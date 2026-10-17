#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!("Pynox requires Linux on x86_64 or aarch64");

mod capture;
mod forkserver;
mod native_json;
mod policy;
mod pool;
mod preload;
mod runner;
mod runtime;
mod sandbox;
mod worker;

use clap::Parser;
use policy::Cli;
use serde_json::json;

const HELP: &str = "\
Pynox: a Python runtime for agents and scripts, with transparent optimizations and an optional sandbox

Usage:
  pynox [PYTHON_OPTIONS] SCRIPT [ARGS]   Run a script like `python`, with Pynox's optimizations
  pynox -c CODE | -m MODULE              Any CPython command line works here
  pynox sandbox [OPTIONS] SCRIPT [ARGS]  Run with explicit permissions and limits; prints a JSON result
  pynox pool [OPTIONS]                   Sandboxed JSONL task pool for agents and services
  pynox doctor                           Check runtime and kernel capabilities
  pynox --version                        Show Pynox and CPython versions

Run `pynox sandbox --help` or `pynox pool --help` for their options.
`pynox python ...` is accepted as an alias for the default mode.";

fn main() {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("doctor") => std::process::exit(runtime::doctor()),
        Some("--version" | "-V") => {
            println!("pynox {}", runtime::cli_version());
            return;
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            return;
        }
        Some("python") => std::process::exit(runtime::python(2)),
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
        Some("sandbox" | "run") => {}
        // Anything that is not a Pynox command is CPython's own command line.
        _ => std::process::exit(runtime::python(1)),
    }
    let cli = Cli::parse_from(std::env::args().skip(1));
    let start = std::time::Instant::now();
    let result = runner::supervise(&cli, start).unwrap_or_else(|error| {
        json!({
            "exit": 125,
            "error": {"type": "LauncherError", "message": format!("{error:#}")},
            "stdout": "", "stderr": "", "duration_ms": start.elapsed().as_millis()
        })
    });
    println!("{}", result);
    std::process::exit(result["exit"].as_i64().unwrap_or(125) as i32);
}
