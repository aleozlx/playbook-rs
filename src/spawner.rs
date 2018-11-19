use std::path::Path;
use std::result::Result;
use ymlctx::context::Context;
use pyo3::prelude::*;
use pyo3::types::PyList;
use crate::JobError;

pub fn invoke_py(src: Context, ctx_step: Context) -> Result<(), JobError> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let syspath: &PyList = py.import("sys").unwrap().get("path").unwrap().try_into().unwrap();
    let ref src_path: String = src.unpack("src").unwrap();
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
    let mod_py = py.import(mod_name).unwrap();

    let ref action: String = ctx_step.unpack("action").unwrap();
    match mod_py.call_method1(action, (ctx_step.to_object(py), )) {
        Ok(_) => Ok(()),
        Err(e) => {
            e.print(py);
            Err(JobError {})
        }
    }
}
