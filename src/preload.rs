//! Operator-selected packages are part of the trusted template, never job input.
use crate::policy::{python_info, Policy};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub modules: Vec<String>,
    pub package_dirs: Vec<PathBuf>,
}

impl TemplateConfig {
    pub fn new(modules: Vec<String>, directories: Vec<PathBuf>) -> Result<Self> {
        if modules.len() > 64 || directories.len() > 64 {
            bail!("At most 64 preload modules and package directories are supported");
        }
        for module in &modules {
            if !module.split('.').all(|part| {
                !part.is_empty()
                    && part.bytes().enumerate().all(|(i, byte)| {
                        byte == b'_'
                            || byte.is_ascii_alphabetic()
                            || (i > 0 && byte.is_ascii_digit())
                    })
            }) {
                bail!("Invalid preload module name: {module}");
            }
        }
        let package_dirs = directories
            .into_iter()
            .map(|path| {
                let path = path.canonicalize().with_context(|| {
                    format!("Cannot resolve package directory {}", path.display())
                })?;
                if !path.is_dir() {
                    bail!("Package path is not a directory: {}", path.display());
                }
                Ok(path)
            })
            .collect::<Result<Vec<_>>>()?;
        let config = Self {
            modules,
            package_dirs,
        };
        if serde_json::to_vec(&config)?.len() > 65_536 {
            bail!("Template configuration exceeds 64 KiB");
        }
        Ok(config)
    }

    pub fn apply_to_job(&self, policy: &mut Policy) -> Result<()> {
        for path in &self.package_dirs {
            if !policy.package_dirs.contains(path) {
                policy.package_dirs.push(path.clone());
            }
        }
        if self.modules.is_empty() {
            return Ok(());
        }
        let python = python_info();
        let mut protected = self.package_dirs.clone();
        protected.extend([python.stdlib, python.platstdlib, std::env::current_exe()?]);
        protected.extend(python.site_packages);
        protected.extend(["/lib", "/lib64", "/usr/lib", "/usr/lib64"].map(PathBuf::from));
        for path in &mut protected {
            if let Ok(resolved) = path.canonicalize() {
                *path = resolved;
            }
        }
        for write in &policy.allow_write {
            if protected
                .iter()
                .any(|path| write.starts_with(path) || path.starts_with(write))
            {
                bail!("Write permission {} overlaps the preloaded runtime; use a separate output directory", write.display());
            }
        }
        Ok(())
    }
}

pub fn check_template() -> Result<()> {
    let threads = std::fs::read_dir("/proc/self/task")?.count();
    if threads != 1 {
        bail!(
            "Preload created {threads} native threads; the fork template must be single-threaded"
        );
    }
    let descriptors: Vec<_> = std::fs::read_dir("/proc/self/fd")?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
        .collect();
    // read_dir's own descriptor is closed before checking the collected numbers.
    for fd in descriptors {
        if fd > 2 && unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0 {
            bail!("Preload left file descriptor {fd} open; template descriptors must not reach future jobs");
        }
    }
    Ok(())
}
