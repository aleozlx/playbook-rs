use std::path::Path;
use std::result::Result;
use ymlctx::context::{Context, CtxObj};
use crate::{TaskError, TaskErrorSource};

#[cfg(feature = "lang_python")]
use pyo3::prelude::*;
#[cfg(feature = "lang_python")]
use pyo3::types::PyList;

#[cfg(feature = "lang_python")]
pub fn invoke(src: Context, ctx_step: Context) -> Result<(), TaskError> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let syspath: &PyList = py.import("sys").unwrap().get("path").unwrap().try_into().unwrap();
    let ref src_path: String = src.unpack("src").unwrap();
    if let Some(CtxObj::Array(sys_paths)) = src.get("sys_path") {
        for item in sys_paths {
            if let CtxObj::Str(sys_path) = item {
                syspath.insert(0, sys_path).unwrap();
            }
        }
    }
    let mod_path;
    if let Some(parent) = Path::new(src_path).parent() {
        mod_path = parent;
    }
    else {
        mod_path = Path::new(".");
    }
    syspath.insert(0, mod_path.to_str().unwrap()).unwrap();

    let mod_name;
    if let Some(stem) = Path::new(src_path).file_stem() {
        mod_name = stem.to_str().unwrap();
    }
    else {
        unreachable!();
    }

    if let Ok(mod_py) = py.import(mod_name) {
        let ref action: String = ctx_step.unpack("action").unwrap();
        match mod_py.call_method1(action, (ctx_step.to_object(py), )) {
            Ok(_) => Ok(()),
            Err(e) => {
                e.print(py);
                Err(TaskError { msg: String::from("There was an exception raised by the step action."), src: TaskErrorSource::Internal })
            }
        }
    }
    else {
        Err(TaskError { msg: String::from("Failed to import the step action source module."), src: TaskErrorSource::Internal })
    }
}
