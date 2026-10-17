use crate::policy::{Policy, PythonInfo};
use anyhow::{bail, Context, Result};
use landlock::{
    path_beneath_rules, Access, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus, ABI,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};
use std::{collections::BTreeMap, path::PathBuf};

pub fn apply(policy: &Policy, python: &PythonInfo) -> Result<()> {
    limits(policy)?;
    filesystem(policy, python)
        .context("Landlock unavailable or policy could not be enforced (ABI >= 3 required)")?;
    syscalls(policy.allow_network).context("Cannot install seccomp filter")?;
    Ok(())
}

fn limits(policy: &Policy) -> Result<()> {
    for (resource, maximum) in [
        (libc::RLIMIT_AS, policy.memory_bytes),
        (libc::RLIMIT_CPU, policy.timeout_ms.div_ceil(1000) + 1),
        (libc::RLIMIT_NOFILE, 128),
        (libc::RLIMIT_CORE, 0),
    ] {
        let limit = libc::rlimit {
            rlim_cur: maximum,
            rlim_max: maximum,
        };
        if unsafe { libc::setrlimit(resource, &limit) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn filesystem(policy: &Policy, python: &PythonInfo) -> Result<()> {
    // ABI 3 adds truncate mediation. Never silently accept partial enforcement.
    let abi = ABI::V3;
    let read = AccessFs::ReadFile | AccessFs::ReadDir;
    let write = AccessFs::WriteFile
        | AccessFs::Truncate
        | AccessFs::MakeReg
        | AccessFs::MakeDir
        | AccessFs::RemoveFile
        | AccessFs::RemoveDir;
    let mut readable = vec![python.stdlib.clone(), python.platstdlib.clone()];
    if policy.source.is_none() {
        readable.push(policy.script.clone());
    }
    readable.extend(python.site_packages.iter().filter(|p| p.exists()).cloned());
    // Dynamic extensions need the system shared libraries. These are part of the
    // trusted runtime, and are explicitly disclosed as implicit read permissions.
    readable.extend(
        [
            "/lib",
            "/lib64",
            "/usr/lib",
            "/usr/lib64",
            "/etc/ld.so.cache",
            "/dev/urandom",
        ]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists()),
    );
    readable.extend(policy.allow_read.iter().cloned());
    readable.extend(policy.package_dirs.iter().cloned());
    if policy.allow_network {
        readable.extend(
            [
                "/etc/resolv.conf",
                "/etc/hosts",
                "/etc/nsswitch.conf",
                "/etc/gai.conf",
                "/etc/ssl/certs",
                "/etc/pki/tls/certs",
                "/etc/pki/ca-trust",
            ]
            .into_iter()
            .map(PathBuf::from)
            .filter(|p| p.exists()),
        );
    }
    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))?
        .create()?
        .add_rules(path_beneath_rules(&readable, read))?
        .add_rules(path_beneath_rules(&policy.allow_write, write))?
        .restrict_self()?;
    if status.ruleset != RulesetStatus::FullyEnforced {
        bail!("Landlock did not fully enforce the requested policy");
    }
    Ok(())
}

fn install(rules: BTreeMap<i64, Vec<SeccompRule>>, action: SeccompAction) -> Result<()> {
    let program: BpfProgram = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        action,
        std::env::consts::ARCH.try_into()?,
    )?
    .try_into()?;
    seccompiler::apply_filter(&program)?;
    Ok(())
}

fn syscalls(allow_network: bool) -> Result<()> {
    let denied = [
        // New executables, process inspection, and cross-process signals.
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_getfd,
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_send_signal,
        libc::SYS_kill,
        libc::SYS_tkill,
        libc::SYS_tgkill,
        libc::SYS_rt_sigqueueinfo,
        libc::SYS_rt_tgsigqueueinfo,
        libc::SYS_setrlimit,
        // Alternate I/O paths and namespace / kernel management.
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_open_by_handle_at,
        libc::SYS_name_to_handle_at,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_chroot,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_fsopen,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_move_mount,
        libc::SYS_open_tree,
        libc::SYS_mount_setattr,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_userfaultfd,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
        libc::SYS_reboot,
        libc::SYS_swapon,
        libc::SYS_swapoff,
        libc::SYS_sethostname,
        libc::SYS_setdomainname,
        libc::SYS_acct,
        // Landlock ABI 3 does not mediate these metadata mutations.
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        // __NR_fchmodat2 = 452 on both supported architectures.
        452,
        libc::SYS_fchown,
        libc::SYS_fchownat,
        libc::SYS_utimensat,
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        // IPC can mutate state belonging to another process.
        libc::SYS_shmget,
        libc::SYS_shmat,
        libc::SYS_shmctl,
        libc::SYS_semget,
        libc::SYS_semop,
        libc::SYS_semtimedop,
        libc::SYS_semctl,
        libc::SYS_msgget,
        libc::SYS_msgsnd,
        libc::SYS_msgrcv,
        libc::SYS_msgctl,
        libc::SYS_mq_open,
        libc::SYS_mq_unlink,
        libc::SYS_mq_timedsend,
        libc::SYS_mq_timedreceive,
        libc::SYS_mq_notify,
        libc::SYS_mq_getsetattr,
    ];
    let mut rules: BTreeMap<_, _> = denied.into_iter().map(|nr| (nr, vec![])).collect();
    // asyncio uses an unnamed AF_UNIX stream socketpair for its wakeup channel.
    // Datagram pairs can be reconnected to host sockets, so permit only streams.
    rules.insert(
        libc::SYS_socketpair,
        vec![
            SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::AF_UNIX as u64,
            )?])?,
            SeccompRule::new(
                [
                    libc::SOCK_STREAM,
                    libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                    libc::SOCK_STREAM | libc::SOCK_NONBLOCK,
                    libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
                ]
                .into_iter()
                .map(|value| {
                    SeccompCondition::new(
                        1,
                        SeccompCmpArgLen::Dword,
                        SeccompCmpOp::Ne,
                        value as u64,
                    )
                })
                .collect::<std::result::Result<Vec<_>, _>>()?,
            )?,
        ],
    );
    if allow_network {
        rules.insert(
            libc::SYS_socket,
            vec![SeccompRule::new(vec![
                SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    libc::AF_INET as u64,
                )?,
                SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    libc::AF_INET6 as u64,
                )?,
            ])?],
        );
    } else {
        for nr in [
            libc::SYS_socket,
            libc::SYS_connect,
            libc::SYS_bind,
            libc::SYS_listen,
            libc::SYS_accept,
            libc::SYS_accept4,
        ] {
            rules.insert(nr, vec![]);
        }
    }
    #[cfg(target_arch = "x86_64")]
    for nr in [
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_chmod,
        libc::SYS_chown,
        libc::SYS_lchown,
        libc::SYS_utime,
        libc::SYS_utimes,
        libc::SYS_futimesat,
    ] {
        rules.insert(nr, vec![]);
    }
    // Native libraries can create threads, but clone cannot create a process.
    rules.insert(
        libc::SYS_clone,
        vec![SeccompRule::new(vec![SeccompCondition::new(
            0,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::MaskedEq(libc::CLONE_THREAD as u64),
            0,
        )?])?],
    );
    // Querying our own limits is useful to native libraries; changing limits or
    // inspecting another process is denied.
    rules.insert(
        libc::SYS_prlimit64,
        vec![
            SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                0,
            )?])?,
            SeccompRule::new(vec![SeccompCondition::new(
                2,
                SeccompCmpArgLen::Qword,
                SeccompCmpOp::Ne,
                0,
            )?])?,
        ],
    );
    install(rules, SeccompAction::Errno(libc::EPERM as u32))?;
    #[cfg(target_arch = "x86_64")]
    {
        // x32 shares AUDIT_ARCH_X86_64 but has different syscall numbers.
        // Reject the entire alternate ABI, including future syscalls.
        use seccompiler::sock_filter;
        seccompiler::apply_filter(&[
            sock_filter {
                code: 0x20,
                jt: 0,
                jf: 0,
                k: 0,
            }, // LD nr
            sock_filter {
                code: 0x45,
                jt: 0,
                jf: 1,
                k: 0x4000_0000,
            }, // JSET x32
            sock_filter {
                code: 0x06,
                jt: 0,
                jf: 0,
                k: libc::SECCOMP_RET_KILL_PROCESS,
            },
            sock_filter {
                code: 0x06,
                jt: 0,
                jf: 0,
                k: libc::SECCOMP_RET_ALLOW,
            },
        ])?;
    }
    // libc falls back to clone() when clone3 is unavailable. Its pointed-to
    // arguments cannot be safely inspected by a classic seccomp BPF filter.
    install(
        [(libc::SYS_clone3, vec![])].into_iter().collect(),
        SeccompAction::Errno(libc::ENOSYS as u32),
    )
}
