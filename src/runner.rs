//! Linux process supervision and structured execution results.
use crate::policy::{python_info, Cli, Policy};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::process::{CommandExt, ExitStatusExt},
    },
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "../tests/rust/diagnostics.rs"]
mod tests;

fn drain(mut reader: impl Read, maximum: usize) -> (Vec<u8>, bool) {
    let mut captured = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                let keep = n.min(maximum.saturating_sub(captured.len()));
                captured.extend_from_slice(&buffer[..keep]);
                truncated |= keep < n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    (captured, truncated)
}

/// Block until the worker exits or the deadline passes. A pidfd wakes the
/// supervisor immediately, instead of a polling sleep adding latency to
/// every task.
fn wait_until(child: &mut Child, deadline: Instant) -> Result<(ExitStatus, bool)> {
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, child.id() as libc::pid_t, 0) };
    if pidfd < 0 {
        return Err(std::io::Error::last_os_error()).context("Cannot watch sandbox worker");
    }
    let pidfd = unsafe { File::from_raw_fd(pidfd as RawFd) };
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            return Ok((child.wait()?, true));
        }
        let mut poll = libc::pollfd {
            fd: pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout = remaining.as_millis().min(i32::MAX as u128 - 1) as i32 + 1;
        let ready = unsafe { libc::poll(&mut poll, 1, timeout) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if ready > 0 {
            return Ok((child.wait()?, false));
        }
    }
}

pub fn supervise(cli: &Cli, start: Instant) -> Result<Value> {
    let policy = Policy::from_cli(cli, None)?;
    let mut fds = [-1; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let report_reader = unsafe { File::from_raw_fd(fds[0]) };
    let report_writer = unsafe { File::from_raw_fd(fds[1]) };
    let report_fd = report_writer.as_raw_fd();
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--internal-worker")
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("OMP_NUM_THREADS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in &cli.allow_env {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(report_fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // If the supervisor disappears, do not leave a worker running.
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("Cannot start sandbox worker")?;
    drop(report_writer);
    let maximum = policy.max_output;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let stdout_task = thread::spawn(move || drain(out, maximum));
    let stderr_task = thread::spawn(move || drain(err, maximum));
    let report_task = thread::spawn(move || drain(report_reader, 65_536));
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(serde_json::to_string(&policy)?.as_bytes())?;
    drop(stdin);
    let (status, timed_out) =
        wait_until(&mut child, start + Duration::from_millis(policy.timeout_ms))?;
    let (stdout, stdout_truncated) = stdout_task.join().unwrap();
    let (stderr, stderr_truncated) = stderr_task.join().unwrap();
    let (report, report_truncated) = report_task.join().unwrap();
    Ok(finish(
        &policy,
        start,
        status,
        timed_out,
        [
            (stdout, stdout_truncated),
            (stderr, stderr_truncated),
            (report, report_truncated),
        ],
    ))
}

pub fn finish(
    policy: &Policy,
    start: Instant,
    status: ExitStatus,
    timed_out: bool,
    output: [(Vec<u8>, bool); 3],
) -> Value {
    let [(stdout, stdout_truncated), (stderr, stderr_truncated), (report, report_truncated)] =
        output;
    let failure = |code, kind, message: String| {
        json!({
            "exit": code, "error": {"type": kind, "message": message}
        })
    };
    let mut result = if timed_out {
        failure(
            124,
            "TimeoutError",
            format!("Wall time exceeded {}ms", policy.timeout_ms),
        )
    } else if let Some(signal) = status.signal() {
        failure(
            128 + signal,
            "SignalError",
            format!("Worker terminated by signal {signal}"),
        )
    } else {
        match serde_json::from_slice::<Value>(&report) {
            Ok(value)
                if !report_truncated
                    && value["exit"].as_u64().is_some_and(|n| n <= 255)
                    && (value["error"].is_null() || value["error"].is_object()) =>
            {
                value
            }
            _ => failure(
                if status.success() {
                    125
                } else {
                    status.code().unwrap_or(125)
                },
                "WorkerError",
                "Worker exited without a valid diagnostic report".into(),
            ),
        }
    };
    result["stdout"] = String::from_utf8_lossy(&stdout).into_owned().into();
    result["stderr"] = String::from_utf8_lossy(&stderr).into_owned().into();
    result["stdout_truncated"] = stdout_truncated.into();
    result["stderr_truncated"] = stderr_truncated.into();
    result["duration_ms"] = json!(start.elapsed().as_millis());
    result["duration_us"] = json!(start.elapsed().as_micros());
    result["runtime"] = json!({"name": "pynox", "python": python_info().version});
    result["policy"] = json!({
        "allow_read": policy.allow_read, "allow_write": policy.allow_write,
        "package_dirs": policy.package_dirs,
        "network": if policy.allow_network { "ipv4+ipv6" } else { "deny" },
        "subprocesses": "deny", "host_unix_sockets": "deny",
        "timeout_ms": policy.timeout_ms, "memory_bytes": policy.memory_bytes,
        "backend": "landlock+seccomp"
    });
    result
}
