use super::*;

fn example_cli() -> Cli {
    Cli::try_parse_from([
        "sandbox",
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/hello.py"),
    ])
    .unwrap()
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
fn script_arguments_cannot_turn_into_runtime_permissions() {
    let cli = Cli::try_parse_from([
        "sandbox",
        "--timeout=2s",
        "app.py",
        "--allow-network",
        "--allow-write=/",
        "--timeout=1m",
    ])
    .unwrap();
    assert!(!cli.allow_network);
    assert!(cli.allow_write.is_empty());
    assert_eq!(cli.timeout, 2_000);
    assert_eq!(
        cli.command[1..],
        ["--allow-network", "--allow-write=/", "--timeout=1m"].map(OsString::from)
    );
}

#[test]
fn malformed_and_loader_environment_names_are_rejected() {
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
        let mut cli = example_cli();
        cli.allow_env.push(name.into());
        assert!(Policy::from_cli(&cli, None).is_err(), "{name:?}");
    }
    let mut cli = example_cli();
    cli.allow_env.push("API_KEY".into());
    assert!(Policy::from_cli(&cli, None).is_ok());
}

#[test]
fn write_grants_do_not_implicitly_allow_reads_or_packages() {
    let mut cli = example_cli();
    let script = PathBuf::from(&cli.command[0]);
    cli.allow_write.push(script.clone());
    let policy = Policy::from_cli(&cli, None).unwrap();
    assert_eq!(policy.allow_write, [script.canonicalize().unwrap()]);
    assert!(policy.allow_read.is_empty());
    assert!(policy.package_dirs.is_empty());
    assert!(!policy.allow_network);
}

#[test]
fn script_directories_and_package_files_are_rejected() {
    let mut cli = example_cli();
    cli.package_dir.push(PathBuf::from(&cli.command[0]));
    assert!(Policy::from_cli(&cli, None).is_err());
    cli.package_dir.clear();
    cli.command[0] = env!("CARGO_MANIFEST_DIR").into();
    assert!(Policy::from_cli(&cli, None).is_err());
}

#[test]
fn inline_code_uses_a_label_and_grants_no_script_path() {
    let cli = Cli::try_parse_from(["sandbox", "step.py", "arg"]).unwrap();
    let policy = Policy::from_cli(&cli, Some("print(1)".into())).unwrap();
    assert_eq!(policy.script, PathBuf::from("step.py"));
    assert_eq!(policy.source.as_deref(), Some("print(1)"));
    assert_eq!(policy.args, ["arg"]);
    for name in ["", "../x.py", "a/b.py"] {
        let cli = Cli::try_parse_from(["sandbox", name]).unwrap();
        assert!(
            Policy::from_cli(&cli, Some("print(1)".into())).is_err(),
            "{name:?}"
        );
    }
    // Without inline code the same label must be an existing file.
    assert!(Policy::from_cli(&cli, None).is_err());
}
