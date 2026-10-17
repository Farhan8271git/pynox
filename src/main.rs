#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!("Pynox 0.1 requires Linux x86_64 or aarch64");

mod capture;
mod forkserver;
mod native_json;
mod policy;
mod pool;
mod preload;
mod runtime;
mod sandbox;
mod worker;

use anyhow::{Context, Result};
use clap::Parser;
use policy::{python_info, Cli, Policy};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::{CommandExt, ExitStatusExt},
    },
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

fn main() {
    if std::env::args().nth(1).as_deref() == Some("python") {
        std::process::exit(runtime::python());
    }
    if std::env::args().nth(1).as_deref() == Some("doctor") {
        std::process::exit(runtime::doctor());
    }
    if std::env::args().nth(1).as_deref() == Some("--internal-worker") {
        std::process::exit(worker::run(false));
    }
    if std::env::args().nth(1).as_deref() == Some("--internal-template") {
        std::process::exit(pool::template_main());
    }
    if std::env::args().nth(1).as_deref() == Some("pool") {
        let options = pool::PoolCli::parse_from(std::env::args().skip(1));
        if let Err(error) = pool::run(options) {
            println!(
                "{}",
                json!({"event": "error", "message": format!("{error:#}")})
            );
            std::process::exit(125);
        }
        return;
    }
    let cli = if std::env::args().nth(1).as_deref() == Some("run") {
        Cli::parse_from(std::env::args().skip(1))
    } else {
        Cli::parse()
    };
    let start = Instant::now();
    let result = supervise(&cli, start).unwrap_or_else(|error| {
        json!({
            "exit": 125,
            "error": {"type": "LauncherError", "message": format!("{error:#}")},
            "stdout": "", "stderr": "", "duration_ms": start.elapsed().as_millis()
        })
    });
    println!("{}", serde_json::to_string(&result).unwrap());
    std::process::exit(result["exit"].as_i64().unwrap_or(125) as i32);
}

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

fn supervise(cli: &Cli, start: Instant) -> Result<Value> {
    let policy = Policy::from_cli(cli)?;
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
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= Duration::from_millis(policy.timeout_ms) {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(5));
    };
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
        "optimize_json": policy.optimize_json,
        "timeout_ms": policy.timeout_ms, "memory_bytes": policy.memory_bytes,
        "backend": "landlock+seccomp"
    });
    result
}
