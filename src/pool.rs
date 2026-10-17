pub use crate::forkserver::template_main;
use crate::{
    capture,
    forkserver::Template,
    policy::{parse_duration, parse_memory, Cli, Policy},
    preload::TemplateConfig,
    worker::Job,
};
use anyhow::{anyhow, bail, Result};
use clap::Parser;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

const MAX_REQUEST: usize = 1_048_576;

#[cfg(test)]
#[path = "../tests/rust/pool.rs"]
mod tests;

#[derive(Parser)]
#[command(
    name = "pynox pool",
    about = "Persistent CPython pool; one JSON request per stdin line"
)]
pub struct PoolCli {
    /// Maximum concurrent executions; each slot has a clean CPython template.
    #[arg(long, default_value = "2", value_parser = clap::value_parser!(u16).range(1..=32))]
    workers: u16,
    /// Preload trusted modules before forking jobs (comma-separated, repeatable).
    #[arg(long, value_delimiter = ',')]
    preload: Vec<String>,
    /// Trusted package directory, readable by every job in this pool.
    #[arg(long, value_name = "PATH")]
    package_dir: Vec<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    allow_network: bool,
    #[serde(default)]
    id: Value,
    /// Path of a script file to run. Exactly one of `script` and `code` is required.
    #[serde(default)]
    script: Option<PathBuf>,
    /// Inline program text; it needs no file and grants no file access.
    #[serde(default)]
    code: Option<String>,
    /// Display name for inline code in tracebacks and errors (default `<code>`).
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    allow_read: Vec<PathBuf>,
    #[serde(default)]
    allow_write: Vec<PathBuf>,
    #[serde(default)]
    package_dirs: Vec<PathBuf>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default = "default_timeout")]
    timeout: String,
    #[serde(default = "default_memory")]
    memory: String,
    #[serde(default = "default_output")]
    max_output: u64,
}

fn default_timeout() -> String {
    "5s".into()
}
fn default_memory() -> String {
    "512m".into()
}
fn default_output() -> u64 {
    65_536
}

impl Request {
    fn job(self) -> Result<Job> {
        let (program, source) = match (self.script, self.code) {
            (Some(script), None) => {
                if self.name.is_some() {
                    bail!("name applies to code requests only");
                }
                (script.into_os_string(), None)
            }
            (None, Some(code)) => (
                self.name.unwrap_or_else(|| "<code>".into()).into(),
                Some(code),
            ),
            (Some(_), Some(_)) => bail!("Use either script or code, not both"),
            (None, None) => bail!("Request needs script or code"),
        };
        let cli = Cli {
            allow_network: self.allow_network,
            command: std::iter::once(program)
                .chain(self.args.into_iter().map(Into::into))
                .collect(),
            allow_read: self.allow_read,
            allow_write: self.allow_write,
            package_dir: self.package_dirs,
            allow_env: self.env.keys().cloned().collect(),
            timeout: parse_duration(&self.timeout).map_err(|e| anyhow!(e))?,
            memory: parse_memory(&self.memory).map_err(|e| anyhow!(e))?,
            max_output: self.max_output,
        };
        let policy = Policy::from_cli(&cli, source)?;
        if self.env.values().any(|value| value.contains('\0')) {
            bail!("Environment values cannot contain NUL");
        }
        let mut env: BTreeMap<String, String> = [
            ("LANG".into(), "C.UTF-8".into()),
            ("OPENBLAS_NUM_THREADS".into(), "1".into()),
            ("OMP_NUM_THREADS".into(), "1".into()),
        ]
        .into_iter()
        .collect();
        env.extend(self.env);
        Ok(Job { policy, env })
    }
}

struct Pending {
    id: Value,
    request: Request,
    received: Instant,
}

type Writer = Arc<Mutex<std::io::Stdout>>;

fn emit(writer: &Writer, result: &Value) -> Result<()> {
    let mut writer = writer.lock().unwrap();
    serde_json::to_writer(&mut *writer, result)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn failure(id: Value, kind: &str, message: String) -> Value {
    json!({"id": id, "exit": 125, "error": {"type": kind, "message": message},
        "stdout": "", "stderr": ""})
}

pub fn run(options: PoolCli) -> Result<()> {
    let config = Arc::new(TemplateConfig::new(options.preload, options.package_dir)?);
    let writer = Arc::new(Mutex::new(std::io::stdout()));
    // Templates are separate exec'd processes. The threaded supervisor is
    // never forked and cannot copy previous requests/secrets into a new job.
    let templates = (0..options.workers)
        .map(|_| Template::start(&config))
        .collect::<Result<Vec<_>>>()?;
    let (sender, receiver) = mpsc::sync_channel::<Pending>(options.workers as usize * 2);
    let receiver = Arc::new(Mutex::new(receiver));
    let threads: Vec<_> = templates
        .into_iter()
        .map(|template| {
            let receiver = Arc::clone(&receiver);
            let writer = Arc::clone(&writer);
            let config = Arc::clone(&config);
            thread::spawn(move || -> Result<()> {
                let mut slot = Some(template);
                loop {
                    let pending = match receiver.lock().unwrap().recv() {
                        Ok(request) => request,
                        Err(_) => break,
                    };
                    let queued_us = pending.received.elapsed().as_micros();
                    let job = pending.request.job().and_then(|mut job| {
                        config.apply_to_job(&mut job.policy)?;
                        Ok(job)
                    });
                    let mut result = match job {
                        Err(error) => {
                            failure(pending.id.clone(), "RequestError", format!("{error:#}"))
                        }
                        Ok(job) => {
                            let outcome = (|| -> Result<Value> {
                                let payload = serde_json::to_vec(&job)?;
                                if payload.len() > MAX_REQUEST {
                                    bail!("Request payload exceeds 1 MiB");
                                }
                                if slot.is_none() {
                                    slot = Some(Template::start(&config)?);
                                }
                                capture::run(slot.as_mut().unwrap(), &job.policy, &payload)
                            })();
                            match outcome {
                                Ok(result) => result,
                                Err(error) => {
                                    // Do not retry a task whose side effects might have occurred.
                                    // Replace the broken template before the next request.
                                    slot = None;
                                    failure(
                                        pending.id.clone(),
                                        "PoolWorkerError",
                                        format!("{error:#}"),
                                    )
                                }
                            }
                        }
                    };
                    result["id"] = pending.id;
                    result["queue_us"] = json!(queued_us);
                    result["runtime"]["preloaded_modules"] = json!(config.modules);
                    emit(&writer, &result)?;
                }
                Ok(())
            })
        })
        .collect();
    emit(
        &writer,
        &json!({"event": "ready", "workers": options.workers, "runtime": "pynox",
            "preloaded_modules": config.modules, "package_dirs": config.package_dirs}),
    )?;
    let input_result = (|| -> Result<()> {
        let mut input = std::io::stdin().lock();
        let mut sequence = 0u64;
        loop {
            let Some((bytes, oversized)) = read_line(&mut input)? else {
                break;
            };
            sequence += 1;
            let received = Instant::now();
            if oversized {
                emit(
                    &writer,
                    &failure(
                        json!(sequence),
                        "RequestError",
                        "Request exceeds 1 MiB".into(),
                    ),
                )?;
                continue;
            }
            if bytes.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            // Recover the ID even for unknown fields or invalid option types.
            let raw: Result<Value, _> = serde_json::from_slice(&bytes);
            let id = raw
                .as_ref()
                .ok()
                .and_then(|v| v.get("id"))
                .cloned()
                .unwrap_or(json!(sequence));
            match raw.and_then(serde_json::from_value::<Request>) {
                Ok(mut request) => {
                    request.id = id.clone();
                    sender
                        .send(Pending {
                            id,
                            request,
                            received,
                        })
                        .map_err(|_| anyhow!("Pool workers stopped"))?;
                }
                Err(error) => emit(&writer, &failure(id, "RequestError", error.to_string()))?,
            }
        }
        Ok(())
    })();
    drop(sender);
    for thread in threads {
        thread
            .join()
            .map_err(|_| anyhow!("Pool thread panicked"))??;
    }
    input_result
}

fn read_line(input: &mut impl BufRead) -> Result<Option<(Vec<u8>, bool)>> {
    let mut bytes = Vec::new();
    let mut oversized = false;
    let mut consumed_any = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(consumed_any.then_some((bytes, oversized)));
        }
        consumed_any = true;
        let end = available.iter().position(|byte| *byte == b'\n');
        let n = end.map_or(available.len(), |position| position + 1);
        if !oversized {
            if bytes.len() + n > MAX_REQUEST {
                oversized = true;
                bytes.clear();
            } else {
                bytes.extend_from_slice(&available[..n]);
            }
        }
        input.consume(n);
        if end.is_some() {
            return Ok(Some((bytes, oversized)));
        }
    }
}
