//! Poll all streams and completion together, without a polling sleep or a
//! reader thread per stream. Large output cannot starve the deadline.
use crate::{
    forkserver::{RunningJob, Template},
    policy::Policy,
    runner::finish,
};
use anyhow::{bail, Result};
use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::ExitStatusExt,
    },
    process::ExitStatus,
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "../tests/rust/capture.rs"]
mod tests;

pub fn pipe() -> Result<(File, File)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
}

pub(crate) struct Stream {
    pub(crate) file: File,
    pub(crate) data: Vec<u8>,
    pub(crate) maximum: usize,
    pub(crate) truncated: bool,
    pub(crate) eof: bool,
}

impl Stream {
    pub(crate) fn new(file: File, maximum: usize) -> Result<Self> {
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self {
            file,
            data: Vec::new(),
            maximum,
            truncated: false,
            eof: false,
        })
    }

    /// Forget the previous step's output and apply a new capture limit.
    pub(crate) fn reset(&mut self, maximum: usize) {
        self.data.clear();
        self.truncated = false;
        self.maximum = maximum;
    }

    pub(crate) fn read_available(&mut self) -> Result<()> {
        let mut buffer = [0; 8192];
        for _ in 0..16 {
            match self.file.read(&mut buffer) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(n) => {
                    let keep = n.min(self.maximum.saturating_sub(self.data.len()));
                    self.data.extend_from_slice(&buffer[..keep]);
                    self.truncated |= keep < n;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

pub fn run(template: &mut Template, policy: &Policy, payload: &[u8]) -> Result<Value> {
    let start = Instant::now();
    let (input_read, mut input_write) = pipe()?;
    let (out_read, out_write) = pipe()?;
    let (err_read, err_write) = pipe()?;
    let (report_read, report_write) = pipe()?;
    let child = template.fork([
        input_read.as_raw_fd(),
        out_write.as_raw_fd(),
        err_write.as_raw_fd(),
        report_write.as_raw_fd(),
    ])?;
    drop((input_read, out_write, err_write, report_write));
    input_write.write_all(payload)?;
    drop(input_write);
    let streams = [
        Stream::new(out_read, policy.max_output)?,
        Stream::new(err_read, policy.max_output)?,
        Stream::new(report_read, 65_536)?,
    ];
    collect(template, child, policy, start, streams)
}

fn collect(
    template: &mut Template,
    child: RunningJob,
    policy: &Policy,
    start: Instant,
    mut streams: [Stream; 3],
) -> Result<Value> {
    let deadline = start + Duration::from_millis(policy.timeout_ms);
    let mut timed_out = false;
    let mut status = None;
    let mut status_bytes = [0u8; 4];
    let mut status_read = 0;
    loop {
        let now = Instant::now();
        if status.is_none() && now >= deadline && !timed_out {
            child.kill();
            timed_out = true;
        }
        if timed_out && now > deadline + Duration::from_secs(2) {
            bail!("Template did not acknowledge job termination");
        }
        if status.is_some() && streams.iter().all(|stream| stream.eof) {
            break;
        }
        let mut fds = [libc::pollfd {
            fd: -1,
            events: libc::POLLIN,
            revents: 0,
        }; 4];
        for (slot, stream) in fds.iter_mut().zip(&streams) {
            if !stream.eof {
                slot.fd = stream.file.as_raw_fd();
            }
        }
        if status.is_none() {
            fds[3].fd = template.channel.as_raw_fd();
        }
        let wait = if timed_out || status.is_some() {
            100
        } else {
            deadline
                .saturating_duration_since(now)
                .as_millis()
                .clamp(1, 100) as i32
        };
        if unsafe { libc::poll(fds.as_mut_ptr(), 4, wait) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        // Read completion before evaluating the next deadline.
        if fds[3].revents != 0 {
            let n = template.channel.read(&mut status_bytes[status_read..])?;
            if n == 0 {
                bail!("CPython template disconnected");
            }
            status_read += n;
            if status_read == 4 {
                status = Some(ExitStatus::from_raw(i32::from_ne_bytes(status_bytes)));
            }
        }
        for (fd, stream) in fds.iter().zip(&mut streams) {
            if fd.revents != 0 {
                stream.read_available()?;
            }
        }
    }
    let output = streams.map(|stream| (stream.data, stream.truncated));
    let mut result = finish(policy, start, status.unwrap(), timed_out, output);
    result["runtime"]["mode"] = "pool".into();
    Ok(result)
}
