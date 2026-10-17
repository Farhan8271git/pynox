use super::*;
use clap::Parser;
use std::io::Cursor;

fn policy() -> Policy {
    let cli = Cli::try_parse_from([
        "pynox",
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/hello.py"),
    ])
    .unwrap();
    Policy::from_cli(&cli, None).unwrap()
}

fn result(status: i32, timed_out: bool, report: &[u8], truncated: bool) -> Value {
    finish(
        &policy(),
        Instant::now(),
        ExitStatus::from_raw(status),
        timed_out,
        [
            (Vec::new(), false),
            (Vec::new(), false),
            (report.to_vec(), truncated),
        ],
    )
}

#[test]
fn cold_capture_drains_all_output_while_retaining_only_the_prefix() {
    let mut input = Cursor::new(vec![b'x'; 100_000]);
    let (bytes, truncated) = drain(&mut input, 37);
    assert_eq!(bytes, vec![b'x'; 37]);
    assert!(truncated);
    assert_eq!(input.position(), 100_000);
    assert_eq!(drain(Cursor::new(b"exact"), 5), (b"exact".to_vec(), false));
}

#[test]
fn interrupted_reads_are_retried() {
    struct InterruptedOnce {
        interrupted: bool,
        data: Cursor<Vec<u8>>,
    }
    impl Read for InterruptedOnce {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            self.data.read(buffer)
        }
    }
    let reader = InterruptedOnce {
        interrupted: false,
        data: Cursor::new(b"after signal".to_vec()),
    };
    assert_eq!(drain(reader, 64), (b"after signal".to_vec(), false));
}

#[test]
fn deadline_and_signal_override_a_success_report() {
    let success = br#"{"exit":0,"error":null}"#;
    let timeout = result(libc::SIGKILL, true, success, false);
    assert_eq!(timeout["exit"], 124);
    assert_eq!(timeout["error"]["type"], "TimeoutError");
    let signal = result(libc::SIGKILL, false, success, false);
    assert_eq!(signal["exit"], 128 + libc::SIGKILL);
    assert_eq!(signal["error"]["type"], "SignalError");
}

#[test]
fn invalid_reports_cannot_turn_a_worker_failure_into_success() {
    for report in [
        "",
        "{",
        "[]",
        "null",
        r#"{"exit":-1,"error":null}"#,
        r#"{"exit":256,"error":null}"#,
        r#"{"exit":"0","error":null}"#,
        r#"{"exit":0,"error":"bad"}"#,
        r#"{"error":null}"#,
    ] {
        for (status, expected) in [(0, 125), (7 << 8, 7)] {
            let value = result(status, false, report.as_bytes(), false);
            assert_eq!(value["exit"], expected, "{report}");
            assert_eq!(value["error"]["type"], "WorkerError", "{report}");
        }
    }
}

#[test]
fn truncated_diagnostics_are_rejected_even_when_valid_json() {
    let value = result(0, false, br#"{"exit":0,"error":null}"#, true);
    assert_eq!(value["exit"], 125);
    assert_eq!(value["error"]["type"], "WorkerError");
}

#[test]
fn script_errors_retain_their_location_and_hint() {
    let report = json!({
        "exit": 1,
        "error": {
            "type": "PermissionError",
            "message": "Permission denied",
            "file": "app.py",
            "line": 3,
            "hint": "Grant the input path."
        }
    });
    let value = result(0, false, &serde_json::to_vec(&report).unwrap(), false);
    assert_eq!(value["exit"], 1);
    assert_eq!(value["error"], report["error"]);
}

#[test]
fn captured_output_and_effective_policy_replace_report_metadata() {
    let value = finish(
        &policy(),
        Instant::now(),
        ExitStatus::from_raw(0),
        false,
        [
            (b"real\xffoutput".to_vec(), true),
            (b"warning\n".to_vec(), false),
            (
                br#"{"exit":0,"error":null,"stdout":"fake","policy":{"network":"allow"},"runtime":{"name":"fake"}}"#.to_vec(),
                false,
            ),
        ],
    );
    assert_eq!(value["stdout"], "real\u{fffd}output");
    assert_eq!(value["stderr"], "warning\n");
    assert_eq!(value["stdout_truncated"], true);
    assert_eq!(value["stderr_truncated"], false);
    assert_eq!(value["policy"]["network"], "deny");
    assert_eq!(value["runtime"]["name"], "pynox");
}
