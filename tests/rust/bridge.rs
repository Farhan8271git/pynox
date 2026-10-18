use super::*;
use crate::policy::{Grants, Program};
use std::io::Read;
use std::os::fd::FromRawFd;

fn policy(functions: &[&str]) -> Policy {
    let program = Program::Code {
        source: "pass".into(),
        label: "<code>".into(),
        argv0: "<code>".into(),
        args: Vec::new(),
    };
    let mut policy = Policy::new(&Grants::default(), program, true).unwrap();
    policy.host_functions = functions.iter().map(|f| f.to_string()).collect();
    policy
}

fn pipe() -> (File, File) {
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
}

fn read_frame(reader: &mut File) -> Value {
    let mut header = Vec::new();
    let mut byte = [0u8];
    while reader.read_exact(&mut byte).is_ok() && byte[0] != b'\n' {
        header.push(byte[0]);
    }
    let length: usize = String::from_utf8(header).unwrap().parse().unwrap();
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn names_must_be_identifiers_and_bounded() {
    check_names(&["search".into(), "get_2".into(), "_x".into()]).unwrap();
    for bad in ["", "2x", "a-b", "a.b", "x y"] {
        assert!(check_names(&[bad.into()]).is_err(), "{bad:?}");
    }
    let many: Vec<String> = (0..=MAX_FUNCTIONS).map(|i| format!("f{i}")).collect();
    assert!(check_names(&many).is_err());
}

#[test]
fn declared_calls_become_events_and_replies_reach_the_child() {
    let bridge = Bridge::default();
    let (mut child_reads, host_writes) = pipe();
    let policy = policy(&["search"]);
    let mut buffer =
        b"{\"pynox_call\":{\"function\":\"search\",\"args\":[\"q\"],\"kwargs\":{\"n\":2}}}\n{\"exit\":0"
            .to_vec();
    let events = {
        let mut task = TaskCalls::new(&bridge, json!("job-1"), &host_writes);
        let events = task.drain(&mut buffer, &policy, false);
        assert_eq!(buffer, b"{\"exit\":0", "the unterminated report stays");
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(
            (&event["event"], &event["id"], &event["function"]),
            (&json!("call"), &json!("job-1"), &json!("search"))
        );
        assert_eq!(
            (&event["args"], &event["kwargs"]),
            (&json!(["q"]), &json!({"n": 2}))
        );
        let call_id = event["call_id"].as_u64().unwrap();
        bridge
            .reply(&Reply {
                call_id,
                result: json!({"hits": 3}),
                error: None,
            })
            .unwrap();
        assert_eq!(read_frame(&mut child_reads), json!({"result": {"hits": 3}}));
        assert!(
            bridge
                .reply(&Reply {
                    call_id,
                    result: Value::Null,
                    error: None,
                })
                .is_err(),
            "each call is answered once"
        );
        events
    };
    assert_eq!(events.len(), 1);
}

#[test]
fn undeclared_functions_and_junk_lines_never_reach_the_client() {
    let bridge = Bridge::default();
    let (mut child_reads, host_writes) = pipe();
    let policy = policy(&["search"]);
    let mut task = TaskCalls::new(&bridge, json!(1), &host_writes);
    let mut buffer = b"not json\n{\"pynox_call\":{\"function\":\"delete_all\"}}\n".to_vec();
    assert!(task.drain(&mut buffer, &policy, false).is_empty());
    assert!(buffer.is_empty());
    let refusal = read_frame(&mut child_reads);
    assert!(
        refusal["error"].as_str().unwrap().contains("not available"),
        "{refusal}"
    );
}

#[test]
fn session_reports_stay_in_the_buffer_and_finished_tasks_forget_their_calls() {
    let bridge = Bridge::default();
    let (_child_reads, host_writes) = pipe();
    let policy = policy(&["search"]);
    let call_id = {
        let mut task = TaskCalls::new(&bridge, json!("s"), &host_writes);
        let mut buffer =
            b"{\"pynox_call\":{\"function\":\"search\"}}\n{\"exit\":0,\"error\":null}\n".to_vec();
        let events = task.drain(&mut buffer, &policy, true);
        assert_eq!(buffer, b"{\"exit\":0,\"error\":null}\n");
        events[0]["call_id"].as_u64().unwrap()
    };
    let late = Reply {
        call_id,
        result: Value::Null,
        error: None,
    };
    assert!(
        bridge.reply(&late).is_err(),
        "a dropped task has no pending calls"
    );
}

#[test]
fn calls_per_task_are_capped() {
    let bridge = Bridge::default();
    let (_child_reads, host_writes) = pipe();
    let policy = policy(&["f"]);
    let mut task = TaskCalls::new(&bridge, json!(1), &host_writes);
    task.count = MAX_CALLS_PER_TASK;
    let mut buffer = b"{\"pynox_call\":{\"function\":\"f\"}}\n".to_vec();
    assert!(task.drain(&mut buffer, &policy, false).is_empty());
}
