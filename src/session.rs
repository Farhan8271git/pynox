//! Persistent sessions: one sandboxed child per session, forked from a
//! template of its own, running successive steps in the same `__main__`.
//! Permissions are fixed when the session starts; every step reuses them.
use crate::{
    capture::{pipe, Stream},
    forkserver::{RunningJob, Template},
    policy::Policy,
    preload::TemplateConfig,
    runner::finish,
    worker::Job,
};
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Write},
    os::{fd::AsRawFd, unix::process::ExitStatusExt},
    process::ExitStatus,
    time::{Duration, Instant},
};

/// One step sent to the session child on its control descriptor.
#[derive(Serialize)]
pub struct Step<'a> {
    pub code: &'a str,
    pub name: &'a str,
    pub args: &'a [String],
}

pub struct Session {
    template: Template,
    child: RunningJob,
    input: File,
    streams: [Stream; 3],
    pub policy: Policy,
    pub steps: u64,
    alive: bool,
}

impl Session {
    /// Start a template, fork the session child and deliver its job line.
    pub fn start(config: &TemplateConfig, job: &Job) -> Result<Session> {
        let mut template = Template::start(config)?;
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
        let mut payload = serde_json::to_vec(job)?;
        payload.push(b'\n');
        input_write.write_all(&payload)?;
        let maximum = job.policy.max_output;
        Ok(Session {
            template,
            child,
            input: input_write,
            streams: [
                Stream::new(out_read, maximum)?,
                Stream::new(err_read, maximum)?,
                Stream::new(report_read, 65_536)?,
            ],
            policy: job.policy.clone(),
            steps: 0,
            alive: true,
        })
    }

    pub fn alive(&self) -> bool {
        self.alive
    }

    /// Run one step and collect its output until the child reports completion,
    /// exits, or the deadline passes. A timeout ends the session.
    pub fn step(&mut self, step: &Step, timeout_ms: u64, max_output: usize) -> Result<Value> {
        if !self.alive {
            bail!("Session has ended");
        }
        let start = Instant::now();
        self.steps += 1;
        for (stream, maximum) in self
            .streams
            .iter_mut()
            .zip([max_output, max_output, 65_536])
        {
            stream.reset(maximum);
        }
        let mut line = serde_json::to_vec(step)?;
        line.push(b'\n');
        if let Err(error) = self.input.write_all(&line) {
            self.alive = false;
            bail!("Session child is not accepting steps: {error}");
        }
        let deadline = start + Duration::from_millis(timeout_ms);
        let mut timed_out = false;
        let mut status: Option<ExitStatus> = None;
        let mut status_bytes = [0u8; 4];
        let mut status_read = 0;
        loop {
            let now = Instant::now();
            if status.is_none() && now >= deadline && !timed_out {
                self.child.kill();
                timed_out = true;
            }
            if timed_out && now > deadline + Duration::from_secs(2) {
                self.alive = false;
                bail!("Template did not acknowledge step termination");
            }
            let report_complete = self.streams[2].data.contains(&b'\n');
            if report_complete || (status.is_some() && self.streams.iter().all(|s| s.eof)) {
                break;
            }
            let mut fds = [libc::pollfd {
                fd: -1,
                events: libc::POLLIN,
                revents: 0,
            }; 4];
            for (slot, stream) in fds.iter_mut().zip(&self.streams) {
                if !stream.eof {
                    slot.fd = stream.file.as_raw_fd();
                }
            }
            if status.is_none() {
                fds[3].fd = self.template.channel.as_raw_fd();
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
            if fds[3].revents != 0 {
                let n = self
                    .template
                    .channel
                    .read(&mut status_bytes[status_read..])?;
                if n == 0 {
                    self.alive = false;
                    bail!("CPython template disconnected");
                }
                status_read += n;
                if status_read == 4 {
                    status = Some(ExitStatus::from_raw(i32::from_ne_bytes(status_bytes)));
                }
            }
            for (fd, stream) in fds.iter().zip(&mut self.streams) {
                if fd.revents != 0 {
                    stream.read_available()?;
                }
            }
        }
        // Output written before the report is already in the pipes; take it.
        for stream in &mut self.streams[..2] {
            if !stream.eof {
                stream.read_available()?;
            }
        }
        if status.is_some() || timed_out {
            self.alive = false;
        }
        let report = {
            let data = &mut self.streams[2].data;
            match data.iter().position(|b| *b == b'\n') {
                Some(end) => {
                    let line = data[..end].to_vec();
                    data.drain(..=end);
                    line
                }
                None => std::mem::take(data),
            }
        };
        let output = [
            (self.streams[0].data.clone(), self.streams[0].truncated),
            (self.streams[1].data.clone(), self.streams[1].truncated),
            (report, false),
        ];
        let mut result = finish(
            &self.policy,
            start,
            status.unwrap_or(ExitStatus::from_raw(0)),
            timed_out,
            output,
        );
        result["runtime"]["mode"] = "session".into();
        result["step"] = json!(self.steps);
        if !self.alive {
            result["session_ended"] = json!(true);
        }
        Ok(result)
    }
}
