//! Host functions: sandboxed code calls functions the pool's client exposes.
//!
//! The child writes `{"pynox_call": {...}}` lines on its report descriptor.
//! The supervisor checks the name against the task's declared functions, emits
//! a `call` event to the client and remembers where to answer. The client's
//! `reply` request is written back to the child as `<length>\n<json>` on its
//! input pipe. The child only ever reaches functions the host declared.
use crate::policy::Policy;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

#[cfg(test)]
#[path = "../tests/rust/bridge.rs"]
mod tests;

pub const MAX_FUNCTIONS: usize = 64;
pub const MAX_CALLS_PER_TASK: u64 = 1_000;
const MAX_FRAME: usize = 1_048_576;

/// Validate the names a request declares.
pub fn check_names(names: &[String]) -> Result<()> {
    if names.len() > MAX_FUNCTIONS {
        bail!("At most {MAX_FUNCTIONS} host functions per task");
    }
    for name in names {
        let valid = !name.is_empty()
            && name.len() <= 64
            && name.bytes().enumerate().all(|(i, b)| {
                b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
            });
        if !valid {
            bail!("Invalid host function name {name:?}");
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct Reply {
    pub call_id: u64,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct CallFrame {
    pynox_call: CallBody,
}

#[derive(Deserialize)]
struct CallBody {
    function: String,
    #[serde(default)]
    args: Vec<Value>,
    #[serde(default)]
    kwargs: serde_json::Map<String, Value>,
}

/// Calls waiting for a client reply, across every job and session.
#[derive(Default)]
pub struct Bridge {
    next: AtomicU64,
    pending: Mutex<HashMap<u64, File>>,
}

/// Per-task bookkeeping: the child's input pipe and the calls it made.
pub struct TaskCalls<'a> {
    pub bridge: &'a Bridge,
    pub task_id: Value,
    pub input: &'a File,
    pub issued: Vec<u64>,
    pub count: u64,
}

impl<'a> TaskCalls<'a> {
    pub fn new(bridge: &'a Bridge, task_id: Value, input: &'a File) -> Self {
        TaskCalls {
            bridge,
            task_id,
            input,
            issued: Vec::new(),
            count: 0,
        }
    }

    /// Take complete call lines out of the report buffer. Returns the `call`
    /// events to send to the client. Lines that are not calls are dropped.
    /// With `keep_report`, a non-call line ends the scan and stays in the
    /// buffer, because sessions terminate each report with a newline.
    pub fn drain(
        &mut self,
        buffer: &mut Vec<u8>,
        policy: &Policy,
        keep_report: bool,
    ) -> Vec<Value> {
        let mut events = Vec::new();
        while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            let line = &buffer[..end];
            match serde_json::from_slice::<CallFrame>(line) {
                Ok(frame) => {
                    buffer.drain(..=end);
                    if let Some(event) = self.accept(frame.pynox_call, policy) {
                        events.push(event);
                    }
                }
                Err(_) if keep_report => break,
                Err(_) => {
                    buffer.drain(..=end);
                }
            }
        }
        events
    }

    fn accept(&mut self, call: CallBody, policy: &Policy) -> Option<Value> {
        self.count += 1;
        let refusal = if !policy.host_functions.contains(&call.function) {
            Some(format!(
                "Host function {:?} is not available",
                call.function
            ))
        } else if self.count > MAX_CALLS_PER_TASK {
            Some(format!(
                "More than {MAX_CALLS_PER_TASK} host calls in one task"
            ))
        } else {
            None
        };
        if let Some(message) = refusal {
            let _ = answer(self.input, &json!({"error": message}));
            return None;
        }
        let call_id = self.bridge.next.fetch_add(1, Ordering::Relaxed) + 1;
        let Ok(input) = self.input.try_clone() else {
            let _ = answer(self.input, &json!({"error": "Cannot route the host call"}));
            return None;
        };
        self.bridge.pending.lock().unwrap().insert(call_id, input);
        self.issued.push(call_id);
        Some(json!({
            "event": "call", "id": self.task_id, "call_id": call_id,
            "function": call.function, "args": call.args, "kwargs": call.kwargs,
        }))
    }
}

impl Drop for TaskCalls<'_> {
    /// A finished or killed task can no longer receive answers.
    fn drop(&mut self) {
        let mut pending = self.bridge.pending.lock().unwrap();
        for call_id in &self.issued {
            pending.remove(call_id);
        }
    }
}

impl Bridge {
    /// Deliver a client reply to the waiting child.
    pub fn reply(&self, reply: &Reply) -> Result<()> {
        let Some(input) = self.pending.lock().unwrap().remove(&reply.call_id) else {
            bail!("No pending host call {}", reply.call_id);
        };
        let body = match &reply.error {
            Some(message) => json!({"error": message}),
            None => json!({"result": reply.result}),
        };
        answer(&input, &body)
    }
}

/// Frame a reply as `<length>\n<json>` so the child reads it exactly.
pub fn answer(mut input: &File, body: &Value) -> Result<()> {
    let mut payload = serde_json::to_vec(body)?;
    if payload.len() > MAX_FRAME {
        payload = serde_json::to_vec(&json!({"error": "Host reply exceeds 1 MiB"}))?;
    }
    input.write_all(format!("{}\n", payload.len()).as_bytes())?;
    input.write_all(&payload)?;
    Ok(())
}
