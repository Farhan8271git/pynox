//! CPython integration shared by restricted workers and explicit Python mode.
use crate::{native_json, policy::python_info};
use anyhow::{bail, Result};
use pyo3_ffi as py;
use std::{
    ffi::{CStr, CString},
    mem::MaybeUninit,
    os::unix::ffi::OsStrExt,
    sync::OnceLock,
};

pub fn cli_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        // CPython exposes a static string, accessible before initialization.
        // Read the loaded engine's version rather than a build-time assumption.
        let python = unsafe { CStr::from_ptr(py::Py_GetVersion()) }.to_string_lossy();
        format!(
            "{} (CPython {})",
            env!("CARGO_PKG_VERSION"),
            python.split_whitespace().next().unwrap_or("unknown")
        )
    })
}

pub unsafe fn register_native() -> Result<()> {
    if py::PyImport_AppendInittab(c"_pynox_json".as_ptr(), Some(native_json::initialize)) < 0 {
        bail!("Cannot register Pynox native module");
    }
    Ok(())
}

pub fn install_json() -> Result<()> {
    let source = serde_json::to_string(include_str!("json_runtime.py"))?;
    let bootstrap = CString::new(format!(
        "import types as _t, sys as _s\n\
         _m = _t.ModuleType('_pynox_json_runtime')\n\
         _s.modules[_m.__name__] = _m\n\
         exec(compile({source}, '<pynox-json>', 'exec'), _m.__dict__)\n\
         _m.install()\ndel _m, _t, _s\n"
    ))?;
    if unsafe { py::PyRun_SimpleString(bootstrap.as_ptr()) } != 0 {
        bail!("Cannot install JSON optimization");
    }
    Ok(())
}

pub fn python() -> i32 {
    match run_python() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("pynox: {error:#}");
            125
        }
    }
}

fn run_python() -> Result<i32> {
    let info = python_info();
    let mut arguments = std::env::args_os().skip(2).collect::<Vec<_>>();
    let optimize = arguments
        .first()
        .is_some_and(|arg| arg == "--optimize-json");
    if optimize {
        arguments.remove(0);
    }
    if arguments.first().is_some_and(|arg| arg == "--") {
        arguments.remove(0);
    }
    arguments.insert(0, info.executable.clone().into_os_string());
    let args = arguments
        .iter()
        .map(|arg| CString::new(arg.as_bytes()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut argv = args.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();
    let home = CString::new(info.home.as_os_str().as_bytes())?;
    let executable = CString::new(info.executable.as_os_str().as_bytes())?;
    unsafe {
        register_native()?;
        let mut config = MaybeUninit::<py::PyConfig>::uninit();
        py::PyConfig_InitPythonConfig(config.as_mut_ptr());
        let mut config = config.assume_init();
        let mut status =
            py::PyConfig_SetBytesString(&mut config, &raw mut config.home, home.as_ptr());
        if py::PyStatus_Exception(status) == 0 {
            status = py::PyConfig_SetBytesString(
                &mut config,
                &raw mut config.executable,
                executable.as_ptr(),
            );
        }
        if py::PyStatus_Exception(status) == 0 {
            status = py::PyConfig_SetBytesArgv(&mut config, argv.len() as isize, argv.as_mut_ptr());
        }
        if py::PyStatus_Exception(status) == 0 {
            status = py::Py_InitializeFromConfig(&config);
        }
        py::PyConfig_Clear(&mut config);
        if py::PyStatus_Exception(status) != 0 {
            py::Py_ExitStatusException(status);
        }
        if optimize {
            if let Err(error) = install_json() {
                py::Py_FinalizeEx();
                return Err(error);
            }
        }
        Ok(py::Py_RunMain())
    }
}

pub fn doctor() -> i32 {
    #[cfg(target_os = "linux")]
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<u8>(),
            0usize,
            1u32,
        )
    };
    #[cfg(target_os = "macos")]
    let abi = -1i64;
    let python = python_info();
    let runtime_available = python.stdlib.is_dir() && python.executable.is_file();
    let sandbox_available = cfg!(target_os = "linux") && abi >= 3;
    let available = runtime_available && (cfg!(target_os = "macos") || sandbox_available);
    let family = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_env = "musl") {
        "linux-musl"
    } else {
        "linux-gnu"
    };
    let note = if cfg!(target_os = "macos") {
        "Native Python mode only. Sandboxed execution and the pool are unavailable."
    } else {
        "Landlock probe only; run a restricted script to verify complete sandbox setup."
    };
    println!(
        "{}",
        serde_json::json!({
            "runtime": "pynox", "version": env!("CARGO_PKG_VERSION"),
            "python": python, "architecture": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "platform": format!("{family}-{}", std::env::consts::ARCH),
            "landlock_abi": abi, "supported": available,
            "runtime_supported": runtime_available,
            "sandbox_supported": sandbox_available,
            "note": note
        })
    );
    if available {
        0
    } else {
        125
    }
}
