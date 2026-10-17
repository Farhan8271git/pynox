//! Bounded eligibility and encode/write dispatch in Rust, using CPython objects
//! directly. No serde conversion or reimplementation of Python's JSON format.
use pyo3_ffi as py;
use std::{
    ptr,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 1_000_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
static FAST: AtomicU64 = AtomicU64::new(0);
static FALLBACK: AtomicU64 = AtomicU64::new(0);

struct Guard {
    parents: [*mut py::PyObject; MAX_DEPTH],
    nodes: usize,
    bytes: usize,
}

impl Guard {
    fn add(&mut self, amount: usize) -> bool {
        if amount > MAX_BYTES - self.bytes {
            return false;
        }
        self.bytes += amount;
        true
    }

    // The caller holds the GIL; traversal invokes no user code and retains no
    // borrowed reference after returning. Container depth and work are bounded.
    unsafe fn eligible(&mut self, obj: *mut py::PyObject, depth: usize) -> bool {
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth >= MAX_DEPTH {
            return false;
        }
        if obj == py::Py_None() || py::PyBool_Check(obj) != 0 {
            return self.add(5);
        }
        if py::PyLong_CheckExact(obj) != 0 {
            let mut overflow = 0;
            py::PyLong_AsLongLongAndOverflow(obj, &mut overflow);
            return overflow == 0 && py::PyErr_Occurred().is_null() && self.add(24);
        }
        if py::PyFloat_CheckExact(obj) != 0 {
            return self.add(32);
        }
        if py::PyUnicode_CheckExact(obj) != 0 {
            let length = py::PyUnicode_GET_LENGTH(obj) as usize;
            return length <= MAX_BYTES / 12 && self.add(length * 12 + 2);
        }
        let dict = py::PyDict_CheckExact(obj) != 0;
        let list = py::PyList_CheckExact(obj) != 0;
        let tuple = py::PyTuple_CheckExact(obj) != 0;
        if !(dict || list || tuple) || self.parents[..depth].contains(&obj) {
            return false;
        }
        self.parents[depth] = obj;
        if !self.add(2) {
            return false;
        }
        if dict {
            let mut position = 0;
            let mut key = ptr::null_mut();
            let mut value = ptr::null_mut();
            while py::PyDict_Next(obj, &mut position, &mut key, &mut value) != 0 {
                if py::PyUnicode_CheckExact(key) == 0
                    || !self.add(4)
                    || !self.eligible(key, depth + 1)
                    || !self.eligible(value, depth + 1)
                {
                    return false;
                }
            }
        } else {
            let length = if list {
                py::PyList_GET_SIZE(obj)
            } else {
                py::PyTuple_GET_SIZE(obj)
            };
            for index in 0..length {
                let item = if list {
                    py::PyList_GET_ITEM(obj, index)
                } else {
                    py::PyTuple_GET_ITEM(obj, index)
                };
                if !self.add(2) || !self.eligible(item, depth + 1) {
                    return false;
                }
            }
        }
        true
    }
}

unsafe fn recursion_space() -> bool {
    if py::Py_GetRecursionLimit() < 384 {
        return false;
    }
    let mut frame = py::PyThreadState_GetFrame(py::PyThreadState_Get());
    let mut count = 0;
    while !frame.is_null() {
        count += 1;
        if count > 128 {
            py::Py_DECREF(frame.cast());
            return false;
        }
        let parent = py::PyFrame_GetBack(frame);
        py::Py_DECREF(frame.cast());
        frame = parent;
    }
    py::PyErr_Occurred().is_null()
}

unsafe extern "C" fn try_dump(
    _self: *mut py::PyObject,
    args: *mut py::PyObject,
) -> *mut py::PyObject {
    if py::PyTuple_GET_SIZE(args) != 3 {
        py::PyErr_SetString(
            py::PyExc_TypeError,
            c"try_dump expects object, file, encoder".as_ptr(),
        );
        return ptr::null_mut();
    }
    let obj = py::PyTuple_GET_ITEM(args, 0);
    let file = py::PyTuple_GET_ITEM(args, 1);
    let encoder = py::PyTuple_GET_ITEM(args, 2);
    let mut guard = Guard {
        parents: [ptr::null_mut(); MAX_DEPTH],
        nodes: 0,
        bytes: 0,
    };
    if !recursion_space() || !guard.eligible(obj, 0) {
        if !py::PyErr_Occurred().is_null() {
            return ptr::null_mut();
        }
        return py::PyBool_FromLong(0);
    }
    // CPython's existing encoder creates its normal str. Keep its number,
    // escaping, ordering and NaN behavior; no conversion to Rust JSON values.
    let encoded = py::PyObject_CallOneArg(encoder, obj);
    if encoded.is_null() {
        if py::PyErr_ExceptionMatches(py::PyExc_MemoryError) != 0
            || py::PyErr_ExceptionMatches(py::PyExc_RecursionError) != 0
        {
            py::PyErr_Clear();
            return py::PyBool_FromLong(0);
        }
        return ptr::null_mut();
    }
    let write = py::PyObject_GetAttrString(file, c"write".as_ptr());
    if write.is_null() {
        py::Py_DECREF(encoded);
        return ptr::null_mut();
    }
    let result = py::PyObject_CallOneArg(write, encoded);
    py::Py_DECREF(write);
    py::Py_DECREF(encoded);
    if result.is_null() {
        return ptr::null_mut();
    }
    py::Py_DECREF(result);
    FAST.fetch_add(1, Ordering::Relaxed);
    py::PyBool_FromLong(1)
}

unsafe extern "C" fn fallback(
    _self: *mut py::PyObject,
    _unused: *mut py::PyObject,
) -> *mut py::PyObject {
    FALLBACK.fetch_add(1, Ordering::Relaxed);
    py::Py_NewRef(py::Py_None())
}

unsafe extern "C" fn stats(
    _self: *mut py::PyObject,
    _unused: *mut py::PyObject,
) -> *mut py::PyObject {
    py::Py_BuildValue(
        c"{sK,sK}".as_ptr(),
        c"fast".as_ptr(),
        FAST.load(Ordering::Relaxed),
        c"fallback".as_ptr(),
        FALLBACK.load(Ordering::Relaxed),
    )
}

static mut METHODS: [py::PyMethodDef; 4] = [
    py::PyMethodDef {
        ml_name: c"try_dump".as_ptr(),
        ml_meth: py::PyMethodDefPointer {
            PyCFunction: try_dump,
        },
        ml_flags: py::METH_VARARGS,
        ml_doc: c"Guard, encode and write ordinary JSON using CPython's native encoder.".as_ptr(),
    },
    py::PyMethodDef {
        ml_name: c"fallback".as_ptr(),
        ml_meth: py::PyMethodDefPointer {
            PyCFunction: fallback,
        },
        ml_flags: py::METH_NOARGS,
        ml_doc: c"Count a fallback for experiment verification.".as_ptr(),
    },
    py::PyMethodDef {
        ml_name: c"stats".as_ptr(),
        ml_meth: py::PyMethodDefPointer { PyCFunction: stats },
        ml_flags: py::METH_NOARGS,
        ml_doc: c"Return fast/fallback counts.".as_ptr(),
    },
    py::PyMethodDef::zeroed(),
];
static mut MODULE: py::PyModuleDef = py::PyModuleDef {
    m_base: py::PyModuleDef_HEAD_INIT,
    m_name: c"_pynox_json".as_ptr(),
    m_doc: c"Rust implementation of the JSON/open experiment.".as_ptr(),
    m_size: -1,
    m_methods: (&raw mut METHODS).cast(),
    m_slots: ptr::null_mut(),
    m_traverse: None,
    m_clear: None,
    m_free: None,
};

pub unsafe extern "C" fn initialize() -> *mut py::PyObject {
    py::PyModule_Create(&raw mut MODULE)
}
