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
assert sys.implementation.name == "cpython" and sys.version_info[:2] == (3, 12), "Pynox 0.1 requires CPython 3.12"
assert not sysconfig.get_config_var("Py_GIL_DISABLED"), "Pynox 0.1 requires the GIL build"
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
}))
"#,
        ])
        .output()
        .expect("Cannot inspect CPython; set PYO3_PYTHON to a Python with libpython");
    assert!(output.status.success(), "CPython inspection failed");
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,{}",
        info["libdir"].as_str().unwrap()
    );
    // Release bundles place the matching CPython beside bin/pynox.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../python/lib");
    let destination = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("python.json");
    fs::write(destination, output.stdout).unwrap();
}
