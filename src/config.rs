//! Optional `pynox.toml`: standing permissions, named profiles and pool
//! defaults. The file is operator configuration, so it is discovered only
//! where the operator put it, never next to the program being run.
use crate::policy::{parse_duration, parse_memory, resolve_paths, Grants, Policy};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[cfg(test)]
#[path = "../tests/rust/config.rs"]
mod tests;

pub const FILE_NAME: &str = "pynox.toml";
pub const ENV_VAR: &str = "PYNOX_CONFIG";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Defaults for sandboxed command-line runs.
    #[serde(default)]
    pub permissions: Grants,
    #[serde(default)]
    pub pool: PoolSection,
    /// Named permission sets, selected with `--profile` or a request's `profile`.
    #[serde(default)]
    pub profiles: BTreeMap<String, Grants>,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolSection {
    pub workers: Option<u16>,
    /// Concurrent persistent sessions (default 16).
    pub max_sessions: Option<u16>,
    /// Close a session idle for this long (default `10m`).
    pub session_idle: Option<String>,
    #[serde(default)]
    pub preload: Vec<String>,
    #[serde(default)]
    pub package_dirs: Vec<PathBuf>,
    /// Ceiling for every request: paths must lie beneath these, limits may not exceed them.
    pub limits: Option<Grants>,
}

impl Config {
    /// Explicit path, then `PYNOX_CONFIG`, then `./pynox.toml` in the current
    /// directory only. A file beside the script is never consulted: a program
    /// with write access to its own directory must not widen its permissions.
    pub fn discover(explicit: Option<&Path>) -> Result<Option<Config>> {
        if let Some(path) = explicit {
            return Config::load(path).map(Some);
        }
        if let Some(path) = std::env::var_os(ENV_VAR).filter(|value| !value.is_empty()) {
            return Config::load(Path::new(&path)).map(Some);
        }
        let local = Path::new(FILE_NAME);
        if local.is_file() {
            return Config::load(local).map(Some);
        }
        Ok(None)
    }

    pub fn load(path: &Path) -> Result<Config> {
        let path = path
            .canonicalize()
            .with_context(|| format!("Cannot read configuration {}", path.display()))?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Cannot read configuration {}", path.display()))?;
        let mut config = Config::parse(&text, &path)
            .with_context(|| format!("Invalid configuration {}", path.display()))?;
        config.path = path;
        Ok(config)
    }

    /// Parse configuration text whose relative paths are anchored at `path`'s directory.
    pub fn parse(text: &str, path: &Path) -> Result<Config> {
        let mut config: Config = toml::from_str(text)?;
        let base = path.parent().unwrap_or(Path::new("."));
        config.permissions = std::mem::take(&mut config.permissions).anchored(base);
        config.profiles = std::mem::take(&mut config.profiles)
            .into_iter()
            .map(|(name, grants)| (name, grants.anchored(base)))
            .collect();
        for directory in &mut config.pool.package_dirs {
            if directory.is_relative() {
                *directory = base.join(&*directory);
            }
        }
        if let Some(idle) = &config.pool.session_idle {
            parse_duration(idle).map_err(|e| anyhow::anyhow!(e))?;
        }
        if let Some(limits) = config.pool.limits.take() {
            let limits = limits.anchored(base);
            if let Some(timeout) = &limits.timeout {
                parse_duration(timeout).map_err(|e| anyhow::anyhow!(e))?;
            }
            if let Some(memory) = &limits.memory {
                parse_memory(memory).map_err(|e| anyhow::anyhow!(e))?;
            }
            config.pool.limits = Some(Grants {
                allow_read: resolve_paths(&limits.allow_read).context("pool.limits.allow_read")?,
                allow_write: resolve_paths(&limits.allow_write)
                    .context("pool.limits.allow_write")?,
                package_dirs: resolve_paths(&limits.package_dirs)
                    .context("pool.limits.package_dirs")?,
                ..limits
            });
        }
        config.path = path.to_path_buf();
        config.check_not_writable()?;
        Ok(config)
    }

    /// The configuration must not lie beneath any write grant it defines.
    fn check_not_writable(&self) -> Result<()> {
        let file = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        let sections = std::iter::once(&self.permissions)
            .chain(self.profiles.values())
            .chain(self.pool.limits.iter());
        for grants in sections {
            for granted in &grants.allow_write {
                let granted = granted.canonicalize().unwrap_or_else(|_| granted.clone());
                if file.starts_with(&granted) {
                    bail!(
                        "Configuration {} is inside writable path {}",
                        file.display(),
                        granted.display()
                    );
                }
            }
        }
        Ok(())
    }

    /// Command-line defaults: `[permissions]` with an optional profile on top.
    pub fn grants(&self, profile: Option<&str>) -> Result<Grants> {
        let mut grants = self.permissions.clone();
        if let Some(name) = profile {
            grants = grants.overlay(self.profile(name)?);
        }
        Ok(grants)
    }

    pub fn profile(&self, name: &str) -> Result<&Grants> {
        self.profiles
            .get(name)
            .with_context(|| format!("Unknown profile {name:?} in {}", self.path.display()))
    }

    /// Enforce `[pool.limits]` on a resolved request policy.
    pub fn check_limits(&self, policy: &Policy) -> Result<()> {
        let Some(limits) = &self.pool.limits else {
            return Ok(());
        };
        let beneath = |requested: &[PathBuf], allowed: &[PathBuf], what: &str| -> Result<()> {
            for path in requested {
                if !allowed.iter().any(|root| path.starts_with(root)) {
                    bail!("{what} {} exceeds pool.limits", path.display());
                }
            }
            Ok(())
        };
        beneath(&policy.allow_read, &limits.allow_read, "allow_read")?;
        beneath(&policy.allow_write, &limits.allow_write, "allow_write")?;
        beneath(&policy.package_dirs, &limits.package_dirs, "package_dirs")?;
        if policy.allow_network && limits.allow_network != Some(true) {
            bail!("allow_network exceeds pool.limits");
        }
        if let Some(timeout) = &limits.timeout {
            let maximum = parse_duration(timeout).map_err(|e| anyhow::anyhow!(e))?;
            if policy.timeout_ms > maximum {
                bail!("timeout exceeds pool.limits ({timeout})");
            }
        }
        if let Some(memory) = &limits.memory {
            let maximum = parse_memory(memory).map_err(|e| anyhow::anyhow!(e))?;
            if policy.memory_bytes > maximum {
                bail!("memory exceeds pool.limits ({memory})");
            }
        }
        if let Some(maximum) = limits.max_output {
            if policy.max_output as u64 > maximum {
                bail!("max_output exceeds pool.limits ({maximum})");
            }
        }
        Ok(())
    }
}
