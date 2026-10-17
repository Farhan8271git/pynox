#[cfg(not(all(
    any(target_os = "linux", target_os = "macos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!("Pynox requires Linux or macOS on x86_64 or aarch64");

mod native_json;
mod policy;
mod runtime;

#[cfg(target_os = "linux")]
mod capture;
#[cfg(target_os = "linux")]
mod forkserver;
#[cfg(target_os = "linux")]
mod pool;
#[cfg(target_os = "linux")]
mod preload;
#[cfg(target_os = "linux")]
mod runner;
#[cfg(target_os = "linux")]
mod sandbox;
#[cfg(target_os = "linux")]
mod worker;

use clap::Parser;
use policy::Cli;
use serde_json::json;

fn main() {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("python") => std::process::exit(runtime::python()),
        Some("doctor") => std::process::exit(runtime::doctor()),
        _ => {}
    }
    #[cfg(target_os = "linux")]
    match command.as_deref() {
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
    #[cfg(target_os = "macos")]
    if matches!(
        command.as_deref(),
        Some("pool" | "--internal-worker" | "--internal-template")
    ) {
        sandbox_unavailable();
    }
    let cli = if command.as_deref() == Some("run") {
        Cli::parse_from(std::env::args().skip(1))
    } else {
        Cli::parse()
    };
    #[cfg(target_os = "linux")]
    {
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
    #[cfg(target_os = "macos")]
    {
        let _ = cli;
        sandbox_unavailable();
    }
}

#[cfg(target_os = "macos")]
fn sandbox_unavailable() -> ! {
    println!(
        "{}",
        json!({
            "exit": 125, "stdout": "", "stderr": "",
            "error": {
                "type": "SandboxUnavailable",
                "message": "Sandboxed execution and the pool are not available on macOS yet.",
                "hint": "Use Linux for sandboxed tasks, or explicitly choose `pynox python` for unrestricted execution."
            }
        })
    );
    std::process::exit(125);
}
