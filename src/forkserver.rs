//! A sterile, single-threaded CPython template. It receives file descriptors,
//! never request payloads or results: future children cannot inherit old jobs.
use crate::{policy::python_info, preload::TemplateConfig, worker};
use anyhow::{bail, Context, Result};
use pyo3_ffi as py;
use std::{
    fs::File,
    io::{Read, Write},
    mem::{size_of, zeroed},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    process::{Child, Command, Stdio},
    time::Duration,
};

pub struct Template {
    process: Child,
    pub channel: UnixStream,
}

impl Template {
    pub fn start(config: &TemplateConfig) -> Result<Self> {
        let (channel, child_channel) = UnixStream::pair()?;
        let child_fd: OwnedFd = child_channel.into();
        let parent = unsafe { libc::getpid() };
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--internal-template")
            .arg(serde_json::to_string(config)?)
            .env_clear()
            .env("LANG", "C.UTF-8")
            .env("OPENBLAS_NUM_THREADS", "1")
            .env("OMP_NUM_THREADS", "1")
            .env("MKL_NUM_THREADS", "1")
            .env("NUMEXPR_NUM_THREADS", "1")
            .stdin(Stdio::from(child_fd))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(move || parent_death(parent));
        }
        let process = command.spawn().context("Cannot start CPython template")?;
        let mut template = Self { process, channel };
        template
            .channel
            .set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut ready = [0];
        template
            .channel
            .read_exact(&mut ready)
            .context("CPython template did not initialize")?;
        if ready == *b"E" {
            let mut size = [0u8; 4];
            template.channel.read_exact(&mut size)?;
            let size = u32::from_ne_bytes(size).min(8192) as usize;
            let mut message = vec![0; size];
            template.channel.read_exact(&mut message)?;
            bail!("CPython template: {}", String::from_utf8_lossy(&message));
        }
        if ready != *b"R" {
            bail!("Invalid template handshake");
        }
        template.channel.set_read_timeout(None)?;
        Ok(template)
    }

    pub fn fork(&mut self, fds: [RawFd; 4]) -> Result<RunningJob> {
        self.channel
            .set_read_timeout(Some(Duration::from_secs(5)))?;
        send_fds(&self.channel, fds)?;
        let mut bytes = [0; 4];
        self.channel.read_exact(&mut bytes)?;
        let pid = i32::from_ne_bytes(bytes);
        if pid <= 0 {
            bail!("Template could not fork (errno {})", -pid);
        }
        // The template does not reap until this pidfd is acquired and acknowledged.
        // Timeout cancellation therefore cannot signal an unrelated reused PID.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let job = RunningJob(unsafe { File::from_raw_fd(fd as RawFd) });
        self.channel.write_all(b"A")?;
        self.channel.set_read_timeout(None)?;
        Ok(job)
    }
}

impl Drop for Template {
    fn drop(&mut self) {
        // Children carry PDEATHSIG as well. A broken slot cannot orphan a job.
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

pub struct RunningJob(File);

impl RunningJob {
    pub fn kill(&self) {
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGKILL,
                0,
                0,
            );
        }
    }
}

impl Drop for RunningJob {
    fn drop(&mut self) {
        self.kill();
    }
}

pub fn parent_death(parent: libc::pid_t) -> std::io::Result<()> {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::getppid() } != parent {
        return Err(std::io::Error::other(
            "Supervisor exited during process startup",
        ));
    }
    Ok(())
}

pub fn template_main() -> i32 {
    let mut channel = unsafe { UnixStream::from_raw_fd(0) };
    let mut ready = false;
    match template_loop(&mut channel, &mut ready) {
        Ok(()) => 0,
        Err(error) => {
            if ready {
                // Once serving jobs, closure is the failure signal. Do not put
                // a startup error frame into the PID/wait-status protocol.
                return 125;
            }
            // Initialization errors are sent before the ready frame.
            let message = format!("{error:#}");
            let bytes = &message.as_bytes()[..message.len().min(8192)];
            let _ = channel.write_all(b"E");
            let _ = channel.write_all(&(bytes.len() as u32).to_ne_bytes());
            let _ = channel.write_all(bytes);
            125
        }
    }
}

fn template_loop(channel: &mut UnixStream, ready: &mut bool) -> Result<()> {
    if unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0u32) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let config: TemplateConfig = serde_json::from_str(
        &std::env::args()
            .nth(2)
            .unwrap_or_else(|| r#"{"modules":[],"package_dirs":[]}"#.into()),
    )?;
    worker::initialize(&python_info())?;
    worker::preload(&config)?;
    channel.write_all(b"R")?;
    *ready = true;
    while let Some(files) = receive_fds(channel)? {
        let parent = unsafe { libc::getpid() };
        unsafe { py::PyOS_BeforeFork() };
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // None of the Rust destructors inherited from the template may run
            // after remapping descriptors. _exit terminates the disposable copy.
            let code = (|| -> Result<i32> {
                parent_death(parent)?;
                for (target, file) in files.iter().enumerate() {
                    if unsafe { libc::dup2(file.as_raw_fd(), target as RawFd) } < 0 {
                        return Err(std::io::Error::last_os_error().into());
                    }
                }
                if unsafe { libc::syscall(libc::SYS_close_range, 4u32, u32::MAX, 0u32) } != 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                unsafe { py::PyOS_AfterFork_Child() };
                Ok(worker::run(true))
            })()
            .unwrap_or(125);
            unsafe { libc::_exit(code) };
        }
        let fork_error = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EAGAIN);
        unsafe { py::PyOS_AfterFork_Parent() };
        drop(files);
        if pid < 0 {
            channel.write_all(&(-fork_error).to_ne_bytes())?;
            continue;
        }
        // Always reap, including protocol failures / supervisor disconnects.
        let response = (|| -> Result<()> {
            channel.write_all(&pid.to_ne_bytes())?;
            let mut ack = [0];
            channel.read_exact(&mut ack)?;
            if ack != *b"A" {
                bail!("Invalid pidfd acknowledgement");
            }
            Ok(())
        })();
        if response.is_err() {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        let mut status = 0;
        loop {
            if unsafe { libc::waitpid(pid, &mut status, 0) } >= 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
        response?;
        channel.write_all(&status.to_ne_bytes())?;
    }
    Ok(())
}

fn send_fds(channel: &UnixStream, fds: [RawFd; 4]) -> Result<()> {
    let mut byte = b'J';
    let mut iov = libc::iovec {
        iov_base: (&raw mut byte).cast(),
        iov_len: 1,
    };
    // usize storage gives cmsghdr its required alignment.
    let mut control = [0usize; 8];
    let mut message: libc::msghdr = unsafe { zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = unsafe { libc::CMSG_SPACE(size_of::<[RawFd; 4]>() as u32) } as _;
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(size_of::<[RawFd; 4]>() as u32) as _;
        std::ptr::copy_nonoverlapping(
            fds.as_ptr().cast::<u8>(),
            libc::CMSG_DATA(header),
            size_of::<[RawFd; 4]>(),
        );
        if libc::sendmsg(channel.as_raw_fd(), &message, libc::MSG_NOSIGNAL) != 1 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}

fn receive_fds(channel: &UnixStream) -> Result<Option<Vec<File>>> {
    let mut byte = 0u8;
    let mut iov = libc::iovec {
        iov_base: (&raw mut byte).cast(),
        iov_len: 1,
    };
    let mut control = [0usize; 8];
    let mut message: libc::msghdr = unsafe { zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = size_of_val(&control) as _;
    let read = unsafe { libc::recvmsg(channel.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
    if read == 0 {
        return Ok(None);
    }
    if read < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        if byte != b'J'
            || message.msg_flags & libc::MSG_CTRUNC != 0
            || header.is_null()
            || (*header).cmsg_level != libc::SOL_SOCKET
            || (*header).cmsg_type != libc::SCM_RIGHTS
            || (*header).cmsg_len as usize
                != libc::CMSG_LEN(size_of::<[RawFd; 4]>() as u32) as usize
        {
            bail!("Invalid template descriptor message");
        }
        let fds = std::slice::from_raw_parts(libc::CMSG_DATA(header).cast::<RawFd>(), 4);
        // Duplicate above stdio before remapping to avoid descriptor collisions.
        let mut files = Vec::new();
        for fd in fds {
            let original = File::from_raw_fd(*fd);
            let duplicate = libc::fcntl(original.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 4);
            if duplicate < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            files.push(File::from_raw_fd(duplicate));
        }
        Ok(Some(files))
    }
}
