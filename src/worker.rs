use crate::{
    policy::{python_info, Policy, PythonInfo},
    preload::TemplateConfig,
    sandbox,
};
use anyhow::{bail, Context, Result};
use pyo3_ffi as py;
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::File,
    io::{Read, Write},
    mem::MaybeUninit,
    os::fd::{AsRawFd, FromRawFd},
};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub policy: Policy,
    pub env: BTreeMap<String, String>,
}

pub fn run(warm: bool) -> i32 {
    match execute(warm) {
        Ok(code) => code,
        Err(error) => {
            let report = serde_json::json!({
                "exit": 125,
                "error": {"type": "SandboxSetupError", "message": format!("{error:#}")}
            });
            let mut pipe = unsafe { File::from_raw_fd(3) };
            let _ = pipe.write_all(report.to_string().as_bytes());
            125
        }
    }
}

fn execute(warm: bool) -> Result<i32> {
    let mut input = String::new();
    std::io::stdin()
        .take(1_048_576)
        .read_to_string(&mut input)?;
    let (policy, env) = if warm {
        let job: Job = serde_json::from_str(&input)?;
        (job.policy, Some(job.env))
    } else {
        (serde_json::from_str(&input)?, None)
    };
    let python = python_info();
    // No caller-provided descriptors or stdin are exposed to user code.
    let null = File::open("/dev/null")?;
    if unsafe { libc::dup2(null.as_raw_fd(), 0) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    drop(null);
    if unsafe { libc::syscall(libc::SYS_close_range, 4u32, u32::MAX, 0u32) } != 0 {
        bail!("close_range failed: {}", std::io::Error::last_os_error());
    }
    sandbox::apply(&policy, &python)?;
    // Cold runs initialize after sandboxing. Pool children inherit trusted
    // CPython startup and optional operator-selected imports; no job code or
    // job data is ever loaded by their template.
    if !warm {
        initialize(&python)?;
    }
    if policy.optimize_json {
        crate::runtime::install_json()?;
    }
    let encoded = serde_json::to_string(&serde_json::to_string(&policy)?)?;
    let packages = serde_json::to_string(&serde_json::to_string(&python.site_packages)?)?;
    if let Some(env) = env {
        let encoded_env = serde_json::to_string(&serde_json::to_string(&env)?)?;
        python_code(&format!(
            "os.environ.clear()\nos.environ.update(json.loads({encoded_env}))"
        ))?;
    }
    python_code(&format!(
        "_pynox_exit = _pynox_run(json.loads({encoded}), json.loads({packages}))"
    ))?;
    let code = unsafe { py::Py_FinalizeEx() };
    if code < 0 {
        bail!("Python finalization failed");
    }
    Ok(0)
}

pub fn initialize(python: &PythonInfo) -> Result<()> {
    unsafe {
        crate::runtime::register_native()?;
        let mut config = MaybeUninit::<py::PyConfig>::uninit();
        py::PyConfig_InitIsolatedConfig(config.as_mut_ptr());
        let mut config = config.assume_init();
        config.site_import = 0;
        config.write_bytecode = 0;
        config.install_signal_handlers = 0;
        config.buffered_stdio = 0;
        let home = CString::new(python.home.to_str().context("Non-UTF-8 Python home")?)?;
        let executable = CString::new(
            python
                .executable
                .to_str()
                .context("Non-UTF-8 Python executable")?,
        )?;
        let initialize = (|| -> Result<()> {
            check(py::PyConfig_SetBytesString(
                &mut config,
                &raw mut config.home,
                home.as_ptr(),
            ))?;
            check(py::PyConfig_SetBytesString(
                &mut config,
                &raw mut config.executable,
                executable.as_ptr(),
            ))?;
            check(py::Py_InitializeFromConfig(&config))
        })();
        py::PyConfig_Clear(&mut config);
        initialize?;
    }
    python_code(include_str!("bootstrap.py"))
}

pub fn preload(config: &TemplateConfig) -> Result<()> {
    if config.modules.is_empty() {
        return Ok(());
    }
    let directories = serde_json::to_string(&serde_json::to_string(&config.package_dirs)?)?;
    let modules = serde_json::to_string(&serde_json::to_string(&config.modules)?)?;
    python_code(&format!(
        "sys.path.extend(json.loads({directories}))\n\
         import importlib, gc\n\
         for _preload_name in json.loads({modules}): importlib.import_module(_preload_name)\n\
         gc.collect()\n\
         gc.freeze()\n"
    ))
    .with_context(|| format!("Could not preload {}", config.modules.join(", ")))?;
    crate::preload::check_template()
}

fn python_code(source: &str) -> Result<()> {
    let code = CString::new(source)?;
    let status = unsafe { py::PyRun_SimpleString(code.as_ptr()) };
    if status != 0 {
        bail!("Python bootstrap failed");
    }
    Ok(())
}

unsafe fn check(status: py::PyStatus) -> Result<()> {
    if py::PyStatus_Exception(status) != 0 {
        let detail = if status.err_msg.is_null() {
            "unknown initialization error".into()
        } else {
            CStr::from_ptr(status.err_msg)
                .to_string_lossy()
                .into_owned()
        };
        bail!("CPython initialization failed: {detail}");
    }
    Ok(())
}
