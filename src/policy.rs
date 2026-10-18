//! Command-line options, task policy and their validation.
//!
//! Pynox's own flags are long options that CPython never uses, so the command
//! line splits cleanly: Pynox flags first, then CPython's command line.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[cfg(test)]
#[path = "../tests/rust/policy.rs"]
mod tests;

pub const DEFAULT_TIMEOUT: &str = "5s";
pub const DEFAULT_MEMORY: &str = "512m";
pub const DEFAULT_MAX_OUTPUT: u64 = 65_536;

/// Permissions and limits before resolution. Lists accumulate, scalars are
/// optional so that layers (config, profile, flags, request) can be overlaid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grants {
    #[serde(default)]
    pub allow_read: Vec<PathBuf>,
    #[serde(default)]
    pub allow_write: Vec<PathBuf>,
    #[serde(default)]
    pub allow_env: Vec<String>,
    #[serde(default)]
    pub package_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub allow_network: Option<bool>,
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub max_output: Option<u64>,
    /// Largest size any single file may reach through a write, e.g. `100m`.
    #[serde(default)]
    pub max_file_size: Option<String>,
}

impl Grants {
    /// Apply `layer` on top of `self`: lists concatenate, set scalars override.
    pub fn overlay(mut self, layer: &Grants) -> Grants {
        self.allow_read.extend(layer.allow_read.iter().cloned());
        self.allow_write.extend(layer.allow_write.iter().cloned());
        self.allow_env.extend(layer.allow_env.iter().cloned());
        self.package_dirs.extend(layer.package_dirs.iter().cloned());
        self.allow_network = layer.allow_network.or(self.allow_network);
        self.timeout = layer.timeout.clone().or(self.timeout);
        self.memory = layer.memory.clone().or(self.memory);
        self.max_output = layer.max_output.or(self.max_output);
        self.max_file_size = layer.max_file_size.clone().or(self.max_file_size);
        self
    }

    /// Make relative paths relative to `base` (the configuration file's directory).
    pub fn anchored(mut self, base: &Path) -> Grants {
        for list in [
            &mut self.allow_read,
            &mut self.allow_write,
            &mut self.package_dirs,
        ] {
            for path in list.iter_mut() {
                if path.is_relative() {
                    *path = base.join(&*path);
                }
            }
        }
        self
    }
}

/// What Pynox was asked to run, after its own flags.
#[derive(Debug, Clone, PartialEq)]
pub enum Program {
    Script {
        path: OsString,
        args: Vec<String>,
    },
    Code {
        source: String,
        /// Filename in tracebacks and errors.
        label: String,
        /// `sys.argv[0]`; `-c` for the command line, the label for pool requests.
        argv0: String,
        args: Vec<String>,
    },
    Module {
        name: String,
        args: Vec<String>,
    },
}

impl Program {
    /// Interpret CPython's command line for sandboxed execution. Only the
    /// program selection is supported; interpreter options need `--no-sandbox`.
    pub fn from_python_args(args: &[OsString]) -> Result<Program> {
        let text = |arg: &OsString| -> Result<String> {
            arg.to_str()
                .map(str::to_owned)
                .context("Arguments must be UTF-8")
        };
        let mut index = 0;
        while let Some(arg) = args.get(index) {
            let flag = arg.to_string_lossy();
            if !flag.starts_with('-') || flag == "-" || flag.starts_with("--") {
                break;
            }
            if flag == "-c" || flag == "-m" || flag.starts_with("-c") || flag.starts_with("-m") {
                break;
            }
            // Isolation and buffering options describe what the sandbox does already.
            if flag[1..].chars().all(|c| "uBEsISq".contains(c)) {
                index += 1;
                continue;
            }
            bail!("CPython option {flag} is not available in the sandbox; use --no-sandbox");
        }
        let Some(first) = args.get(index) else {
            bail!(
                "Nothing to run: give a script, -c CODE or -m MODULE (the REPL needs --no-sandbox)"
            );
        };
        let first_text = first.to_string_lossy();
        let rest = |from: usize| -> Result<Vec<String>> { args[from..].iter().map(text).collect() };
        if first_text == "-" {
            bail!("Reading the program from stdin needs --no-sandbox");
        }
        if let Some(inline) = first_text.strip_prefix("-c") {
            let (source, from) = if inline.is_empty() {
                let code = args.get(index + 1).context("-c needs the code to run")?;
                (text(code)?, index + 2)
            } else {
                (inline.to_owned(), index + 1)
            };
            return Ok(Program::Code {
                source,
                label: "<string>".into(),
                argv0: "-c".into(),
                args: rest(from)?,
            });
        }
        if let Some(inline) = first_text.strip_prefix("-m") {
            let (name, from) = if inline.is_empty() {
                let module = args.get(index + 1).context("-m needs the module to run")?;
                (text(module)?, index + 2)
            } else {
                (inline.to_owned(), index + 1)
            };
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                bail!("Invalid module name {name:?}");
            }
            return Ok(Program::Module {
                name,
                args: rest(from)?,
            });
        }
        Ok(Program::Script {
            path: first.clone(),
            args: rest(index + 1)?,
        })
    }
}

/// Pynox's own command line, parsed before CPython's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Invocation {
    pub grants: Grants,
    pub json: bool,
    pub no_sandbox: bool,
    pub config: Option<PathBuf>,
    pub profile: Option<String>,
    /// Everything after Pynox's flags: CPython's command line.
    pub python_args: Vec<OsString>,
}

const FLAGS_WITH_VALUE: [&str; 11] = [
    "--max-file-size",
    "--allow-read",
    "--allow-write",
    "--allow-env",
    "--package-dir",
    "--timeout",
    "--memory",
    "--max-output",
    "--config",
    "--profile",
    "--allow-network",
];

pub fn parse_invocation(args: &[OsString]) -> Result<Invocation> {
    let mut invocation = Invocation::default();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        let flag = arg.to_string_lossy().into_owned();
        if flag == "--" {
            index += 1;
            break;
        }
        if !flag.starts_with("--") {
            break;
        }
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
            None => (flag.clone(), None),
        };
        match name.as_str() {
            "--json" => invocation.json = true,
            "--no-sandbox" => invocation.no_sandbox = true,
            "--allow-network" => {
                invocation.grants.allow_network = Some(match inline.as_deref() {
                    None | Some("true") => true,
                    Some("false") => false,
                    Some(other) => bail!("--allow-network accepts true or false, not {other:?}"),
                });
            }
            name if FLAGS_WITH_VALUE.contains(&name) => {
                let value = match inline {
                    Some(value) => value,
                    None => {
                        index += 1;
                        args.get(index)
                            .map(|v| v.to_string_lossy().into_owned())
                            .with_context(|| format!("{name} needs a value"))?
                    }
                };
                if value.is_empty() {
                    bail!("{name} needs a value");
                }
                match name {
                    "--allow-read" => invocation.grants.allow_read.push(value.into()),
                    "--allow-write" => invocation.grants.allow_write.push(value.into()),
                    "--allow-env" => invocation.grants.allow_env.push(value),
                    "--package-dir" => invocation.grants.package_dirs.push(value.into()),
                    "--timeout" => {
                        parse_duration(&value).map_err(|e| anyhow::anyhow!(e))?;
                        invocation.grants.timeout = Some(value);
                    }
                    "--memory" => {
                        parse_memory(&value).map_err(|e| anyhow::anyhow!(e))?;
                        invocation.grants.memory = Some(value);
                    }
                    "--max-file-size" => {
                        parse_size(&value).map_err(|e| anyhow::anyhow!(e))?;
                        invocation.grants.max_file_size = Some(value);
                    }
                    "--max-output" => {
                        invocation.grants.max_output =
                            Some(value.parse().context("--max-output must be a number")?);
                    }
                    "--config" => invocation.config = Some(value.into()),
                    "--profile" => invocation.profile = Some(value),
                    _ => unreachable!(),
                }
            }
            // Unknown long options belong to CPython (--help, --version, ...).
            _ => break,
        }
        index += 1;
    }
    invocation.python_args = args[index..].to_vec();
    if invocation.no_sandbox
        && (invocation.json
            || invocation.profile.is_some()
            || invocation.grants != Grants::default())
    {
        bail!("--no-sandbox cannot be combined with permissions, --profile or --json");
    }
    Ok(invocation)
}

/// Resolved, validated policy for one task. Serialized to the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub allow_network: bool,
    /// Script path, or the label of inline code or the module name.
    pub script: PathBuf,
    pub argv0: String,
    pub source: Option<String>,
    pub module: Option<String>,
    pub args: Vec<String>,
    pub allow_read: Vec<PathBuf>,
    pub allow_write: Vec<PathBuf>,
    /// Granted without a flag: the script's directory, or the current directory for `-m`.
    pub implicit_read: Vec<PathBuf>,
    pub package_dirs: Vec<PathBuf>,
    pub allow_env: Vec<String>,
    pub timeout_ms: u64,
    pub memory_bytes: u64,
    pub max_output: usize,
    /// RLIMIT_FSIZE for the task; `None` leaves file sizes unlimited.
    #[serde(default)]
    pub max_file_size: Option<u64>,
    /// Capture output for a JSON result instead of passing it through.
    pub capture: bool,
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
    // Resolve symlinks so an installed CLI link finds its versioned bundle.
    if let Ok(executable) = std::env::current_exe().and_then(|path| path.canonicalize()) {
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

pub fn resolve_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    paths
        .iter()
        .map(|path| {
            let path = path
                .canonicalize()
                .with_context(|| format!("Permission path must exist: {}", path.display()))?;
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
}

impl Policy {
    pub fn new(grants: &Grants, program: Program, capture: bool) -> Result<Self> {
        let max_output = grants.max_output.unwrap_or(DEFAULT_MAX_OUTPUT);
        if !(1..=16_777_216).contains(&max_output) {
            bail!("max_output must be between 1 and 16777216 bytes");
        }
        let timeout_ms = parse_duration(grants.timeout.as_deref().unwrap_or(DEFAULT_TIMEOUT))
            .map_err(|e| anyhow::anyhow!(e))?;
        let memory_bytes = parse_memory(grants.memory.as_deref().unwrap_or(DEFAULT_MEMORY))
            .map_err(|e| anyhow::anyhow!(e))?;
        let max_file_size = grants
            .max_file_size
            .as_deref()
            .map(parse_size)
            .transpose()
            .map_err(|e| anyhow::anyhow!(e))?;
        for name in &grants.allow_env {
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
        let package_dirs = resolve_paths(&grants.package_dirs)?;
        if package_dirs.iter().any(|path| !path.is_dir()) {
            bail!("Package paths must be directories");
        }
        let (script, argv0, source, module, args, implicit_read) = match program {
            Program::Script { path, args } => {
                let script = PathBuf::from(&path)
                    .canonicalize()
                    .with_context(|| format!("Cannot resolve script {}", path.to_string_lossy()))?;
                if !script.is_file() {
                    bail!("Script must be a regular file");
                }
                let argv0 = script
                    .to_str()
                    .context("Script path must be UTF-8")?
                    .to_owned();
                let directory = script.parent().map(Path::to_path_buf).into_iter().collect();
                (script, argv0, None, None, args, directory)
            }
            Program::Code {
                source,
                label,
                argv0,
                args,
            } => {
                if label.is_empty() || label.contains(['\0', '/']) {
                    bail!("Code name must be a non-empty label without path separators");
                }
                (
                    PathBuf::from(&label),
                    argv0,
                    Some(source),
                    None,
                    args,
                    Vec::new(),
                )
            }
            Program::Module { name, args } => {
                let cwd =
                    std::env::current_dir().context("Cannot resolve the current directory")?;
                (
                    PathBuf::from(&name),
                    name.clone(),
                    None,
                    Some(name),
                    args,
                    vec![cwd],
                )
            }
        };
        Ok(Self {
            allow_network: grants.allow_network.unwrap_or(false),
            script,
            argv0,
            source,
            module,
            args,
            allow_read: resolve_paths(&grants.allow_read)?,
            allow_write: resolve_paths(&grants.allow_write)?,
            implicit_read,
            package_dirs,
            allow_env: grants.allow_env.clone(),
            timeout_ms,
            memory_bytes,
            max_output: max_output as usize,
            max_file_size,
            capture,
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

/// A byte size with a unit (`k`, `m`, `g`), at least 1k.
pub fn parse_size(value: &str) -> std::result::Result<u64, String> {
    scaled(
        &value.to_ascii_lowercase(),
        &[("k", 1024), ("m", 1024 * 1024), ("g", 1024 * 1024 * 1024)],
    )
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
