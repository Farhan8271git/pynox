use super::*;
use std::io::{BufReader, Cursor};

fn request(mut fields: Value) -> Request {
    fields["script"] = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/hello.py").into();
    serde_json::from_value(fields).unwrap()
}

#[test]
fn minimal_request_has_no_optional_capabilities() {
    let job = request(json!({})).job().unwrap();
    assert!(!job.policy.allow_network);
    assert!(job.policy.allow_read.is_empty());
    assert!(job.policy.allow_write.is_empty());
    assert!(job.policy.package_dirs.is_empty());
    assert_eq!(job.policy.timeout_ms, 5_000);
    assert_eq!(job.policy.memory_bytes, 512 * 1024 * 1024);
    assert_eq!(job.policy.max_output, 65_536);
}

#[test]
fn unknown_fields_and_mistyped_options_are_rejected() {
    for value in [
        json!({"script": "app.py", "allow_netwrok": true}),
        json!({"script": "app.py", "preload": ["os"]}),
        json!({"script": "app.py", "allow_network": "false"}),
        json!({"script": "app.py", "allow_read": "/tmp"}),
        json!({"script": "app.py", "timeout": 5}),
        json!({"script": "app.py", "max_output": -1}),
        json!({"script": "app.py", "env": {"KEY": 42}}),
        json!({"code": 42}),
    ] {
        assert!(
            serde_json::from_value::<Request>(value.clone()).is_err(),
            "{value}"
        );
    }
}

#[test]
fn json_requests_obey_output_limit_boundaries() {
    for limit in [0, 16_777_217, u64::MAX] {
        assert!(request(json!({"max_output": limit})).job().is_err());
    }
    for limit in [1, 16_777_216] {
        assert_eq!(
            request(json!({"max_output": limit}))
                .job()
                .unwrap()
                .policy
                .max_output,
            limit
        );
    }
}

#[test]
fn environment_is_validated_and_kept_per_request() {
    for env in [
        json!({"API_KEY": "secret\0suffix"}),
        json!({"LD_PRELOAD": "/tmp/inject.so"}),
        json!({"KEY=value": "secret"}),
    ] {
        assert!(request(json!({"env": env})).job().is_err());
    }
    let first = request(json!({"env": {"API_KEY": "first-only"}}))
        .job()
        .unwrap();
    let second = request(json!({})).job().unwrap();
    assert_eq!(first.env["API_KEY"], "first-only");
    assert!(!second.env.contains_key("API_KEY"));
}

#[test]
fn worker_count_stays_within_supported_bounds() {
    for workers in ["0", "33", "-1"] {
        assert!(PoolCli::try_parse_from(["pool", "--workers", workers]).is_err());
    }
    for workers in ["1", "32"] {
        assert!(PoolCli::try_parse_from(["pool", "--workers", workers]).is_ok());
    }
}

#[test]
fn line_limit_includes_the_newline() {
    let mut bytes = vec![b'x'; MAX_REQUEST - 1];
    bytes.push(b'\n');
    let mut input = BufReader::with_capacity(127, Cursor::new(&bytes));
    let (line, oversized) = read_line(&mut input).unwrap().unwrap();
    assert!(!oversized);
    assert_eq!(line, bytes);
    assert!(read_line(&mut input).unwrap().is_none());
}

#[test]
fn oversized_line_is_discarded_without_losing_the_next_request() {
    let next = b"{\"script\":\"next.py\"}\n";
    let mut bytes = vec![b'x'; MAX_REQUEST];
    bytes.push(b'\n');
    bytes.extend_from_slice(next);
    let mut input = BufReader::with_capacity(127, Cursor::new(bytes));
    let (line, oversized) = read_line(&mut input).unwrap().unwrap();
    assert!(oversized);
    assert!(line.is_empty());
    assert_eq!(
        read_line(&mut input).unwrap().unwrap(),
        (next.to_vec(), false)
    );
    assert!(read_line(&mut input).unwrap().is_none());
}

#[test]
fn fragmented_utf8_and_final_line_without_newline_are_preserved() {
    let source = "{\"id\":\"ação\",\"script\":\"app.py\"}";
    let mut input = BufReader::with_capacity(1, Cursor::new(source.as_bytes()));
    assert_eq!(
        read_line(&mut input).unwrap().unwrap(),
        (source.as_bytes().to_vec(), false)
    );
    assert!(read_line(&mut input).unwrap().is_none());
}

#[test]
fn requests_carry_exactly_one_program() {
    assert!(serde_json::from_value::<Request>(json!({"id": "missing"}))
        .unwrap()
        .job()
        .is_err());
    assert!(request(json!({"code": "print(1)"})).job().is_err());
    assert!(request(json!({"name": "step.py"})).job().is_err());
    let inline = serde_json::from_value::<Request>(json!({"code": "print(1)", "args": ["x"]}))
        .unwrap()
        .job()
        .unwrap();
    assert_eq!(inline.policy.script.to_str(), Some("<code>"));
    assert_eq!(inline.policy.source.as_deref(), Some("print(1)"));
    assert_eq!(inline.policy.args, ["x"]);
    let named = serde_json::from_value::<Request>(json!({"code": "print(1)", "name": "step.py"}))
        .unwrap()
        .job()
        .unwrap();
    assert_eq!(named.policy.script.to_str(), Some("step.py"));
}
