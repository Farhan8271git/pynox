use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};

#[cfg(test)]
#[path = "../tests/rust/policy.rs"]
mod tests;

#[derive(Parser, Debug)]
#[command(
    version = crate::runtime::cli_version(),
    name = "pynox",
    about = "Run Python with kernel-enforced permissions",
    after_help = "Commands:\n  pynox run [OPTIONS] SCRIPT  Restricted execution (also the default)\n  pynox pool [OPTIONS]        Restricted JSONL task pool\n  pynox python [ARGS]         CPython CLI, WITHOUT sandbox\n  pynox doctor                Check runtime and kernel capabilities\n\nExperimental optimization: --optimize-json (also supported immediately after `python`)."
)]
pub struct Cli {
    /// Allow IPv4/IPv6 networking. Host Unix sockets remain denied.
    #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true", default_value = "false")]
    pub allow_network: bool,
    /// Enable experimental bounded json.dump optimization (uses extra memory).
    #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true", default_value = "false")]
    pub optimize_json: bool,
    /// File or directory to make readable (repeat this flag for multiple paths).
    #[arg(long, value_name = "PATH")]
    pub allow_read: Vec<PathBuf>,
    /// Existing file or directory to make writable; does not grant read access.
    #[arg(long, value_name = "PATH")]
    pub allow_write: Vec<PathBuf>,
    /// Environment variable to pass to Python (repeatable).
    #[arg(long, value_name = "NAME")]
    pub allow_env: Vec<String>,
    /// Existing package directory to import from; grants read access to its contents.
    #[arg(long, value_name = "PATH")]
    pub package_dir: Vec<PathBuf>,
    /// Wall-clock limit: e.g. 500ms, 5s, 1m.
    #[arg(long, default_value = "5s", value_parser = parse_duration)]
    pub timeout: u64,
    /// Address-space limit: e.g. 512m, 2g.
    #[arg(long, default_value = "512m", value_parser = parse_memory)]
    pub memory: u64,
    /// Maximum captured bytes per stream; excess output is drained and discarded.
    #[arg(long, default_value = "65536", value_parser = clap::value_parser!(u64).range(1..=16_777_216))]
    pub max_output: u64,
    /// Python script followed by its arguments; only the script is implicitly readable.
    #[arg(required = true, num_args = 1.., trailing_var_arg = true, value_names = ["SCRIPT", "ARGS"])]
    pub command: Vec<OsString>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Policy {
    pub allow_network: bool,
    pub optimize_json: bool,
    pub script: PathBuf,
    pub args: Vec<String>,
    pub allow_read: Vec<PathBuf>,
    pub allow_write: Vec<PathBuf>,
    pub package_dirs: Vec<PathBuf>,
    pub timeout_ms: u64,
    pub memory_bytes: u64,
    pub max_output: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PythonInfo {
    pub home: PathBuf,
    pub executable: PathBuf,
    pub stdlib: PathBuf,
    pub platstdlib: PathBuf,
    pub site_packages: Vec<PathBuf>,
    pub libdir: PathBuf,
    pub version: String,
    pub major_minor: String,
    pub abi: String,
}

pub fn python_info() -> PythonInfo {
    let mut info: PythonInfo =
        serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/python.json"))).unwrap();
    // The running executable's location, not cwd/environment, selects a bundle.
    if let Ok(executable) = std::env::current_exe() {
        if let Some(root) = executable.parent().and_then(|bin| bin.parent()) {
            let home = root.join("python");
            let stdlib = home.join(format!("lib/python{}", info.major_minor));
            if stdlib.join("os.py").is_file() {
                info.home = home.clone();
                info.executable = home.join(format!("bin/python{}", info.major_minor));
                info.stdlib = stdlib;
                info.platstdlib = info.stdlib.clone();
                info.site_packages = vec![info.stdlib.join("site-packages")];
                info.libdir = home.join("lib");
            }
        }
    }
    info
}

impl Policy {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        if !(1..=16_777_216).contains(&cli.max_output) {
            bail!("max_output must be between 1 and 16777216 bytes");
        }
        let (script, args) = cli.command.split_first().context("Script is required")?;
        let script = PathBuf::from(script)
            .canonicalize()
            .context("Cannot resolve script")?;
        if !script.is_file() {
            bail!("Script must be a regular file");
        }
        for name in &cli.allow_env {
            if name.is_empty() || name.contains(['=', '\0']) {
                bail!("Invalid environment variable name");
            }
            // These variables affect the native loader before the sandbox exists.
            if name.starts_with("LD_")
                || name.starts_with("DYLD_")
                || matches!(
                    name.as_str(),
                    "GLIBC_TUNABLES" | "GCONV_PATH" | "LOCPATH" | "NLSPATH"
                )
            {
                bail!("Loader variable {name} cannot be forwarded");
            }
        }
        let resolve = |paths: &[PathBuf]| -> Result<Vec<PathBuf>> {
            paths
                .iter()
                .map(|path| {
                    let path = path.canonicalize().with_context(|| {
                        format!("Permission path must exist: {}", path.display())
                    })?;
                    let metadata = path.metadata()?;
                    if !metadata.is_file() && !metadata.is_dir() {
                        bail!(
                            "Permission paths must be regular files or directories: {}",
                            path.display()
                        );
                    }
                    Ok(path)
                })
                .collect()
        };
        let package_dirs = resolve(&cli.package_dir)?;
        if package_dirs.iter().any(|path| !path.is_dir()) {
            bail!("Package paths must be directories");
        }
        Ok(Self {
            allow_network: cli.allow_network,
            optimize_json: cli.optimize_json,
            script,
            args: args
                .iter()
                .map(|arg| {
                    arg.to_str()
                        .context("Script arguments must be UTF-8")
                        .map(str::to_owned)
                })
                .collect::<Result<_>>()?,
            allow_read: resolve(&cli.allow_read)?,
            allow_write: resolve(&cli.allow_write)?,
            package_dirs,
            timeout_ms: cli.timeout,
            memory_bytes: cli.memory,
            max_output: cli.max_output as usize,
        })
    }
}

fn scaled(value: &str, units: &[(&str, u64)]) -> std::result::Result<u64, String> {
    for (suffix, multiplier) in units {
        if let Some(number) = value.strip_suffix(suffix) {
            return number
                .parse::<u64>()
                .ok()
                .and_then(|n| n.checked_mul(*multiplier))
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("Invalid positive limit: {value}"));
        }
    }
    Err(format!("Invalid unit in {value}"))
}

pub fn parse_duration(value: &str) -> std::result::Result<u64, String> {
    let n = scaled(value, &[("ms", 1), ("s", 1000), ("m", 60_000)])?;
    if n > 86_400_000 {
        return Err("Timeout must be at most 24h".into());
    }
    Ok(n)
}

pub fn parse_memory(value: &str) -> std::result::Result<u64, String> {
    let n = scaled(
        &value.to_ascii_lowercase(),
        &[("k", 1024), ("m", 1024 * 1024), ("g", 1024 * 1024 * 1024)],
    )?;
    if n < 32 * 1024 * 1024 {
        return Err("Memory must be at least 32m for CPython startup".into());
    }
    Ok(n)
}
