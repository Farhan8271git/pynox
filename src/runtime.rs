//! CPython integration shared by sandboxed workers and the default Python mode.
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

/// Embedded Python, precompiled by build.rs with the linked CPython.
pub const BOOTSTRAP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bootstrap.marshal"));
const OPTIMIZATIONS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/optimizations.marshal"));
const JSON_RUNTIME: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/json_runtime.marshal"));

/// Load a precompiled code object. The caller holds the GIL and owns the result.
pub unsafe fn load_code(bytecode: &[u8], what: &str) -> Result<*mut py::PyObject> {
    let code =
        py::PyMarshal_ReadObjectFromString(bytecode.as_ptr().cast(), bytecode.len() as isize);
    if code.is_null() {
        py::PyErr_Print();
        bail!("Cannot load {what}");
    }
    Ok(code)
}

/// Execute precompiled code with `globals` as its module namespace.
pub unsafe fn run_code(bytecode: &[u8], globals: *mut py::PyObject, what: &str) -> Result<()> {
    let code = load_code(bytecode, what)?;
    if py::PyDict_GetItemString(globals, c"__builtins__".as_ptr()).is_null()
        && py::PyDict_SetItemString(globals, c"__builtins__".as_ptr(), py::PyEval_GetBuiltins()) < 0
    {
        py::Py_DECREF(code);
        bail!("Cannot prepare namespace for {what}");
    }
    let result = py::PyEval_EvalCode(code, globals, globals);
    py::Py_DECREF(code);
    if result.is_null() {
        py::PyErr_Print();
        bail!("{what} failed");
    }
    py::Py_DECREF(result);
    Ok(())
}

/// Install the transparent optimizations once, before any user code runs.
/// Patches apply lazily when the application imports their target module, so
/// startup pays only for registering a finder from precompiled bytecode.
pub fn install_optimizations() -> Result<()> {
    unsafe {
        let module = py::PyModule_New(c"_pynox_runtime".as_ptr());
        if module.is_null() {
            bail!("Cannot create Pynox runtime module");
        }
        let outcome = (|| -> Result<()> {
            run_code(
                OPTIMIZATIONS,
                py::PyModule_GetDict(module),
                "Pynox runtime bootstrap",
            )?;
            if py::PyDict_SetItemString(
                py::PyImport_GetModuleDict(),
                c"_pynox_runtime".as_ptr(),
                module,
            ) < 0
            {
                bail!("Cannot register Pynox runtime module");
            }
            let install = py::PyObject_GetAttrString(module, c"install".as_ptr());
            if install.is_null() {
                py::PyErr_Clear();
                bail!("Pynox runtime bootstrap has no install function");
            }
            let result = match load_code(JSON_RUNTIME, "JSON optimization") {
                Ok(json) => {
                    let result = py::PyObject_CallOneArg(install, json);
                    py::Py_DECREF(json);
                    result
                }
                Err(error) => {
                    py::Py_DECREF(install);
                    return Err(error);
                }
            };
            py::Py_DECREF(install);
            if result.is_null() {
                py::PyErr_Print();
                bail!("Cannot install runtime optimizations");
            }
            py::Py_DECREF(result);
            Ok(())
        })();
        py::Py_DECREF(module);
        outcome
    }
}

/// Run CPython's own command line, starting at argument `skip`.
pub fn python(skip: usize) -> i32 {
    match run_python(skip) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("pynox: {error:#}");
            125
        }
    }
}

fn run_python(skip: usize) -> Result<i32> {
    let info = python_info();
    let mut arguments = std::env::args_os().skip(skip).collect::<Vec<_>>();
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
        if let Err(error) = install_optimizations() {
            py::Py_FinalizeEx();
            return Err(error);
        }
        Ok(py::Py_RunMain())
    }
}

pub fn doctor() -> i32 {
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<u8>(),
            0usize,
            1u32,
        )
    };
    let python = python_info();
    let runtime_available = python.stdlib.is_dir() && python.executable.is_file();
    let sandbox_available = abi >= 3;
    let family = if cfg!(target_env = "musl") {
        "linux-musl"
    } else {
        "linux-gnu"
    };
    let note = "Landlock probe only; run a sandboxed script to verify complete sandbox setup.";
    println!(
        "{}",
        serde_json::json!({
            "runtime": "pynox", "version": env!("CARGO_PKG_VERSION"),
            "python": python, "architecture": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "platform": format!("{family}-{}", std::env::consts::ARCH),
            "landlock_abi": abi, "supported": runtime_available,
            "runtime_supported": runtime_available,
            "sandbox_supported": sandbox_available,
            "note": note
        })
    );
    if runtime_available {
        0
    } else {
        125
    }
}
