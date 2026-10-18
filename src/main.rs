#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!("Pynox requires Linux on x86_64 or aarch64");

mod capture;
mod config;
mod forkserver;
mod native_json;
mod policy;
mod pool;
mod preload;
mod runner;
mod runtime;
mod sandbox;
mod session;
mod worker;

use clap::Parser;
use serde_json::json;
use std::ffi::OsString;

const HELP: &str = "\
Pynox: the Python runtime for agents. Sandboxed by default, plain Python on request.

Usage:
  pynox [FLAGS] SCRIPT [ARGS]        Run a script in the sandbox; output and exit code are Python's
  pynox [FLAGS] -c CODE | -m MODULE  Same, for inline code or a module
  pynox --json [FLAGS] ...           Print one JSON result instead (for agents and automation)
  pynox --no-sandbox [PYTHON ARGS]   CPython's own command line, unrestricted
  pynox pool [OPTIONS]               Sandboxed JSONL task pool (`pynox pool --help`)
  pynox doctor                       Check runtime and kernel capabilities
  pynox --version                    Show Pynox and CPython versions

Permissions (deny by default; the script's directory is readable, the current
directory for -m):
  --allow-read=PATH      Read an existing file or directory (repeatable)
  --allow-write=PATH     Write beneath an existing path; does not grant reads
  --package-dir=PATH     Import installed packages from this directory
  --allow-env=NAME       Forward an environment variable
  --allow-network        IPv4/IPv6; subprocesses and host Unix sockets stay denied
  --timeout=5s           Wall-clock limit        --memory=512m   Address-space limit
  --max-output=65536     Captured bytes per stream, with --json
  --max-file-size=SIZE   Largest size a written file may reach, e.g. 100m

Configuration:
  --config=PATH          pynox.toml with permissions, profiles and pool settings
                         (default: $PYNOX_CONFIG, then ./pynox.toml)
  --profile=NAME         Apply a named profile from the configuration";

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    match args.get(1).and_then(|arg| arg.to_str()) {
        Some("doctor") => std::process::exit(runtime::doctor()),
        Some("--version" | "-V") => {
            println!("pynox {}", runtime::cli_version());
            return;
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            return;
        }
        Some("--internal-worker") => std::process::exit(worker::run(false)),
        Some("--internal-template") => std::process::exit(pool::template_main()),
        Some("pool") => {
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
        _ => {}
    }
    let invocation = match policy::parse_invocation(&args[1..]) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("pynox: {error:#}");
            std::process::exit(2);
        }
    };
    if invocation.no_sandbox {
        std::process::exit(runtime::python(invocation.python_args));
    }
    let start = std::time::Instant::now();
    std::process::exit(runner::execute(&invocation, start));
}
