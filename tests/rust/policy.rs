use super::*;

fn hello() -> OsString {
    concat!(env!("CARGO_MANIFEST_DIR"), "/examples/hello.py").into()
}

fn args(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

#[test]
fn duration_units_and_boundary_are_consistent() {
    for (value, milliseconds) in [
        ("1ms", 1),
        ("500ms", 500),
        ("5s", 5_000),
        ("2m", 120_000),
        ("1440m", 86_400_000),
        ("86400s", 86_400_000),
    ] {
        assert_eq!(parse_duration(value).unwrap(), milliseconds, "{value}");
    }
}

#[test]
fn invalid_or_overflowing_durations_are_rejected() {
    for value in [
        "",
        "5",
        "0ms",
        "-1s",
        "1.5s",
        "1h",
        "5ss",
        "86400001ms",
        "18446744073709551615m",
        "18446744073709551616ms",
    ] {
        assert!(parse_duration(value).is_err(), "{value}");
    }
}

#[test]
fn memory_units_respect_the_minimum_and_case() {
    for (value, bytes) in [
        ("32768k", 33_554_432),
        ("32m", 33_554_432),
        ("32M", 33_554_432),
        ("2g", 2_147_483_648),
        ("2G", 2_147_483_648),
    ] {
        assert_eq!(parse_memory(value).unwrap(), bytes, "{value}");
    }
}

#[test]
fn invalid_or_overflowing_memory_limits_are_rejected() {
    for value in [
        "32767k",
        "31m",
        "0g",
        "-32m",
        "1.5g",
        "32mb",
        "33554432",
        "18446744073709551615g",
    ] {
        assert!(parse_memory(value).is_err(), "{value}");
    }
}

#[test]
fn pynox_flags_stop_at_the_python_command_line() {
    let invocation = parse_invocation(&args(&[
        "--timeout=2s",
        "--allow-read",
        "data",
        "--allow-network",
        "--json",
        "app.py",
        "--allow-network",
        "--allow-write=/",
        "--timeout=1m",
    ]))
    .unwrap();
    assert_eq!(invocation.grants.timeout.as_deref(), Some("2s"));
    assert_eq!(invocation.grants.allow_read, [PathBuf::from("data")]);
    assert_eq!(invocation.grants.allow_network, Some(true));
    assert!(invocation.grants.allow_write.is_empty());
    assert!(invocation.json);
    assert_eq!(
        invocation.python_args,
        args(&[
            "app.py",
            "--allow-network",
            "--allow-write=/",
            "--timeout=1m"
        ])
    );
}

#[test]
fn python_options_and_unknown_long_options_pass_through() {
    let invocation = parse_invocation(&args(&["-I", "-c", "print(1)"])).unwrap();
    assert_eq!(invocation.python_args, args(&["-I", "-c", "print(1)"]));
    let version = parse_invocation(&args(&["--no-sandbox", "--version"])).unwrap();
    assert!(version.no_sandbox);
    assert_eq!(version.python_args, args(&["--version"]));
    let dashes = parse_invocation(&args(&["--allow-network", "--", "--weird.py"])).unwrap();
    assert_eq!(dashes.python_args, args(&["--weird.py"]));
}

#[test]
fn flag_values_are_validated_while_parsing() {
    for bad in [
        &["--timeout=1h", "app.py"][..],
        &["--memory=1m", "app.py"],
        &["--max-output=lots", "app.py"],
        &["--allow-network=maybe", "app.py"],
        &["--allow-read=", "app.py"],
        &["--config"],
        &["--no-sandbox", "--allow-network", "app.py"],
        &["--no-sandbox", "--json", "app.py"],
    ] {
        assert!(parse_invocation(&args(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn programs_are_selected_like_python() {
    assert_eq!(
        Program::from_python_args(&args(&["app.py", "x"])).unwrap(),
        Program::Script {
            path: "app.py".into(),
            args: vec!["x".into()]
        }
    );
    assert_eq!(
        Program::from_python_args(&args(&["-I", "-u", "-c", "print(1)", "y"])).unwrap(),
        Program::Code {
            source: "print(1)".into(),
            label: "<string>".into(),
            argv0: "-c".into(),
            args: vec!["y".into()]
        }
    );
    assert_eq!(
        Program::from_python_args(&args(&["-mjson.tool", "--help"])).unwrap(),
        Program::Module {
            name: "json.tool".into(),
            args: vec!["--help".into()]
        }
    );
    for bad in [
        &[][..],
        &["-"],
        &["-O", "app.py"],
        &["-X", "dev", "app.py"],
        &["-c"],
        &["-m", "../evil"],
    ] {
        assert!(Program::from_python_args(&args(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn malformed_and_loader_environment_names_are_rejected() {
    let program = || Program::Script {
        path: hello(),
        args: Vec::new(),
    };
    for name in [
        "",
        "KEY=value",
        "KEY\0",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "GLIBC_TUNABLES",
        "GCONV_PATH",
        "LOCPATH",
        "NLSPATH",
    ] {
        let grants = Grants {
            allow_env: vec![name.into()],
            ..Grants::default()
        };
        assert!(Policy::new(&grants, program(), true).is_err(), "{name:?}");
    }
    let grants = Grants {
        allow_env: vec!["API_KEY".into()],
        ..Grants::default()
    };
    assert!(Policy::new(&grants, program(), true).is_ok());
}

#[test]
fn scripts_grant_their_directory_and_nothing_else() {
    let program = Program::Script {
        path: hello(),
        args: Vec::new(),
    };
    let policy = Policy::new(&Grants::default(), program, false).unwrap();
    let script = PathBuf::from(hello()).canonicalize().unwrap();
    assert_eq!(
        policy.implicit_read,
        [script.parent().unwrap().to_path_buf()]
    );
    assert!(policy.allow_read.is_empty());
    assert!(policy.allow_write.is_empty());
    assert!(!policy.allow_network);
    assert!(!policy.capture);
    assert_eq!(policy.timeout_ms, 5_000);
    assert_eq!(policy.memory_bytes, 512 * 1024 * 1024);
    assert_eq!(policy.argv0, script.to_str().unwrap());
}

#[test]
fn write_grants_do_not_implicitly_allow_reads_or_packages() {
    let script = PathBuf::from(hello());
    let grants = Grants {
        allow_write: vec![script.clone()],
        ..Grants::default()
    };
    let program = Program::Script {
        path: hello(),
        args: Vec::new(),
    };
    let policy = Policy::new(&grants, program, true).unwrap();
    assert_eq!(policy.allow_write, [script.canonicalize().unwrap()]);
    assert!(policy.allow_read.is_empty());
    assert!(policy.package_dirs.is_empty());
}

#[test]
fn script_directories_and_package_files_are_rejected() {
    let grants = Grants {
        package_dirs: vec![PathBuf::from(hello())],
        ..Grants::default()
    };
    let program = || Program::Script {
        path: hello(),
        args: Vec::new(),
    };
    assert!(Policy::new(&grants, program(), true).is_err());
    let directory = Program::Script {
        path: env!("CARGO_MANIFEST_DIR").into(),
        args: Vec::new(),
    };
    assert!(Policy::new(&Grants::default(), directory, true).is_err());
}

#[test]
fn inline_code_uses_a_label_and_grants_no_script_path() {
    let code = |label: &str| Program::Code {
        source: "print(1)".into(),
        label: label.into(),
        argv0: label.into(),
        args: vec!["arg".into()],
    };
    let policy = Policy::new(&Grants::default(), code("step.py"), true).unwrap();
    assert_eq!(policy.script, PathBuf::from("step.py"));
    assert_eq!(policy.source.as_deref(), Some("print(1)"));
    assert!(policy.implicit_read.is_empty());
    assert_eq!(policy.args, ["arg"]);
    for label in ["", "../x.py", "a/b.py"] {
        assert!(
            Policy::new(&Grants::default(), code(label), true).is_err(),
            "{label:?}"
        );
    }
}

#[test]
fn modules_grant_the_current_directory() {
    let program = Program::Module {
        name: "json.tool".into(),
        args: Vec::new(),
    };
    let policy = Policy::new(&Grants::default(), program, false).unwrap();
    assert_eq!(policy.module.as_deref(), Some("json.tool"));
    assert_eq!(policy.implicit_read, [std::env::current_dir().unwrap()]);
    assert_eq!(policy.argv0, "json.tool");
}

#[test]
fn overlay_concatenates_lists_and_overrides_scalars() {
    let base = Grants {
        allow_read: vec!["a".into()],
        allow_network: Some(false),
        timeout: Some("1s".into()),
        ..Grants::default()
    };
    let layer = Grants {
        allow_read: vec!["b".into()],
        allow_network: Some(true),
        memory: Some("1g".into()),
        ..Grants::default()
    };
    let merged = base.overlay(&layer);
    assert_eq!(merged.allow_read, [PathBuf::from("a"), PathBuf::from("b")]);
    assert_eq!(merged.allow_network, Some(true));
    assert_eq!(merged.timeout.as_deref(), Some("1s"));
    assert_eq!(merged.memory.as_deref(), Some("1g"));
}
