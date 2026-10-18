use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=PYO3_PYTHON");
    let python = env::var("PYO3_PYTHON").unwrap_or_else(|_| "python3".into());
    let output = Command::new(&python)
        .args([
            "-I",
            "-S",
            "-c",
            r#"
import json, sys, sysconfig
assert sys.implementation.name == "cpython" and (3, 11) <= sys.version_info[:2] <= (3, 15), "Pynox requires CPython 3.11 through 3.15"
assert not sysconfig.get_config_var("Py_GIL_DISABLED"), "Pynox 0.1 requires the GIL build"
assert sysconfig.get_config_var("Py_ENABLE_SHARED"), "Pynox requires a shared libpython"
print(json.dumps({
    "home": sys.base_prefix,
    "executable": sys.executable,
    "stdlib": sysconfig.get_path("stdlib"),
    "platstdlib": sysconfig.get_path("platstdlib"),
    "site_packages": list(dict.fromkeys([
        sysconfig.get_path("purelib"), sysconfig.get_path("platlib")
    ])),
    "libdir": sysconfig.get_config_var("LIBDIR"),
    "version": sys.version.split()[0],
    "major_minor": f"{sys.version_info.major}.{sys.version_info.minor}",
    "abi": f"cp{sys.version_info.major}{sys.version_info.minor}",
}))
"#,
        ])
        .output()
        .expect("Cannot inspect CPython; set PYO3_PYTHON to a Python with libpython");
    assert!(
        output.status.success(),
        "CPython inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,{}",
        info["libdir"].as_str().unwrap()
    );
    // Release bundles place the matching CPython beside bin/pynox.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../python/lib");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out_dir.join("python.json"), output.stdout).unwrap();
    // Embedded Python is compiled here, by the CPython the binary links, and
    // loaded at runtime with marshal like a .pyc. Compiling source at startup
    // would be the interpreter's first use of the compiler and costs about a
    // millisecond per process; loading bytecode costs microseconds.
    for (name, filename) in [
        ("bootstrap", "<pynox-bootstrap>"),
        ("optimizations", "<pynox-runtime>"),
        ("json_runtime", "<pynox-json>"),
    ] {
        let source = format!("python/pynox/_embedded/{name}.py");
        println!("cargo:rerun-if-changed={source}");
        let compiled = Command::new(&python)
            .args([
                "-I",
                "-S",
                "-c",
                "import marshal, sys\n\
                 source = open(sys.argv[1], encoding='utf-8').read()\n\
                 sys.stdout.buffer.write(marshal.dumps(compile(source, sys.argv[2], 'exec')))",
                &source,
                filename,
            ])
            .output()
            .expect("Cannot compile embedded Python");
        assert!(
            compiled.status.success(),
            "Compiling {source} failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        fs::write(out_dir.join(format!("{name}.marshal")), compiled.stdout).unwrap();
    }
}
