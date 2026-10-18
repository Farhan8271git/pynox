use super::*;
use crate::policy::Program;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn parse(text: &str) -> Result<Config> {
    Config::parse(text, &root().join("pynox.toml"))
}

#[test]
fn relative_paths_are_anchored_at_the_configuration_file() {
    let config = parse(
        r#"
[permissions]
allow_read = ["examples", "/tmp"]
allow_network = true

[pool]
workers = 4
preload = ["json"]
package_dirs = ["examples"]

[profiles.analysis]
allow_write = ["examples/out"]
timeout = "30s"
"#,
    )
    .unwrap();
    assert_eq!(
        config.permissions.allow_read,
        [root().join("examples"), PathBuf::from("/tmp")]
    );
    assert_eq!(config.pool.workers, Some(4));
    assert_eq!(config.pool.package_dirs, [root().join("examples")]);
    let analysis = config.grants(Some("analysis")).unwrap();
    assert_eq!(
        analysis.allow_read,
        [root().join("examples"), PathBuf::from("/tmp")]
    );
    assert_eq!(analysis.allow_write, [root().join("examples/out")]);
    assert_eq!(analysis.allow_network, Some(true));
    assert_eq!(analysis.timeout.as_deref(), Some("30s"));
    assert!(config.grants(Some("missing")).is_err());
}

#[test]
fn unknown_keys_and_invalid_limits_are_rejected() {
    for text in [
        "[permissions]\nallow_net = true\n",
        "[pool]\nworker = 2\n",
        "[pool.limits]\ntimeout = \"1h\"\n",
        "[pool.limits]\nallow_read = [\"/definitely/missing\"]\n",
        "[sandbox]\nallow_read = []\n",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn configuration_inside_a_writable_path_is_rejected() {
    assert!(parse("[permissions]\nallow_write = [\".\"]\n").is_err());
    assert!(parse("[profiles.loose]\nallow_write = [\"..\"]\n").is_err());
    assert!(parse("[permissions]\nallow_write = [\"examples\"]\n").is_ok());
}

/// A scratch tree with `data/` and `out/`; `examples/out` is not in the checkout.
fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pynox-config-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::create_dir_all(root.join("out")).unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn pool_limits_cap_every_request_dimension() {
    let root = scratch();
    let config = Config::parse(
        r#"
[pool.limits]
allow_read = ["data"]
allow_write = ["out"]
allow_network = false
timeout = "10s"
memory = "1g"
max_output = 4096
"#,
        &root.join("pynox.toml"),
    )
    .unwrap();
    let code = || Program::Code {
        source: "pass".into(),
        label: "<code>".into(),
        argv0: "<code>".into(),
        args: Vec::new(),
    };
    let within = Grants {
        allow_read: vec![root.join("data")],
        allow_write: vec![root.join("out")],
        timeout: Some("10s".into()),
        memory: Some("1g".into()),
        max_output: Some(4096),
        ..Grants::default()
    };
    config
        .check_limits(&mut Policy::new(&within, code(), true).unwrap())
        .unwrap();
    for exceeding in [
        Grants {
            allow_read: vec![root.clone()],
            ..Grants::default()
        },
        Grants {
            allow_write: vec![root.join("data")],
            ..Grants::default()
        },
        Grants {
            package_dirs: vec![root.join("data")],
            ..Grants::default()
        },
        Grants {
            allow_network: Some(true),
            ..Grants::default()
        },
        Grants {
            timeout: Some("11s".into()),
            ..Grants::default()
        },
        Grants {
            memory: Some("2g".into()),
            ..Grants::default()
        },
        Grants {
            max_output: Some(4097),
            ..Grants::default()
        },
    ] {
        let mut policy = Policy::new(&exceeding, code(), true).unwrap();
        assert!(config.check_limits(&mut policy).is_err(), "{exceeding:?}");
    }
    let unlimited = parse("").unwrap();
    let wide = Grants {
        allow_network: Some(true),
        ..Grants::default()
    };
    unlimited
        .check_limits(&mut Policy::new(&wide, code(), true).unwrap())
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn file_size_limit_caps_requests_and_becomes_their_default() {
    let config = parse("[pool.limits]\nmax_file_size = \"10m\"\n").unwrap();
    let code = || Program::Code {
        source: "pass".into(),
        label: "<code>".into(),
        argv0: "<code>".into(),
        args: Vec::new(),
    };
    let mut unset = Policy::new(&Grants::default(), code(), true).unwrap();
    config.check_limits(&mut unset).unwrap();
    assert_eq!(unset.max_file_size, Some(10 * 1024 * 1024));
    let smaller = Grants {
        max_file_size: Some("1m".into()),
        ..Grants::default()
    };
    let mut policy = Policy::new(&smaller, code(), true).unwrap();
    config.check_limits(&mut policy).unwrap();
    assert_eq!(policy.max_file_size, Some(1024 * 1024));
    let larger = Grants {
        max_file_size: Some("11m".into()),
        ..Grants::default()
    };
    let mut policy = Policy::new(&larger, code(), true).unwrap();
    assert!(config.check_limits(&mut policy).is_err());
    assert!(parse("[pool.limits]\nmax_file_size = \"lots\"\n").is_err());
}
