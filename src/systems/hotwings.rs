#![allow(unused_imports)]
#![cfg(feature = "as_switch")]
#![cfg(feature = "sys_hotwings")]

// use std::path::Path;
use std::ffi::OsStr;
// use regex::Regex;
// use colored::Colorize;
use handlebars::{Handlebars, RenderError};
use ymlctx::context::{Context, CtxObj};
use crate::{TaskError, TaskErrorSource};
use super::Infrastructure;

/// Hotwings - a K8s+Celery powered job system
pub struct Hotwings;

impl Infrastructure for Hotwings {
    /// Hotwings - a K8s+Celery powered job system
    /// 
    /// * `ctx_docker` @param a docker context that contains spefications about the container
    /// * `cmd` @param the command to run within the container
    /// * @returns YAML file that provisions the job using the batch/v1 K8s API
    /// 
    /// > Note: the return value is for informational purposes only, the necessary K8s resources
    /// > would already have been provisioned.
    fn start<I>(&self, ctx_docker: Context, cmd: I) -> Result<String, TaskError>
      where I: IntoIterator, I::Item: AsRef<std::ffi::OsStr>
    {
        // TODO get user info by deserializing a file from the submission tgz
        // let username;
        // let output = std::process::Command::new("id").output().unwrap();
        // let mut id_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        // let newline_len = id_stdout.trim_right().len();
        // id_stdout.truncate(newline_len);
        // let rule = Regex::new(r"^uid=(?P<uid>[0-9]+)(\((?P<user>\w+)\))? gid=(?P<gid>[0-9]+)(\((?P<group>\w+)\))?").unwrap();
        // if let Some(caps) = rule.captures(&id_stdout) {
        //     username = caps.name("user").unwrap().as_str().to_owned();
        // }
        // else {
        //     return Err(TaskError { msg: String::from("Failed to identify the user."), src: TaskErrorSource::Internal });
        // }
        // let mut userinfo = HashMap::new();
        // crate::copy_user_info(&mut userinfo, &username);
        // let home = format!("/home/{}", &username);

        match k8s_api(ctx_docker, cmd) {
            Ok(resources) => {
                match k8s_provisioner(&resources) {
                    Ok(()) => Ok(String::from(resources.join("\n"))),
                    Err(e) => Err(e)
                }
            },
            Err(e) => Err(TaskError { msg: e.desc, src: TaskErrorSource::Internal })
        }
    }
}

/// Get the renderer with .hbs templates baked into the program
fn get_renderer() -> Handlebars {
    let mut renderer = Handlebars::new();
    renderer.register_template_string("batch-job", include_str!("templates-hotwings/batch.hbs")).unwrap();
    return renderer;
}

/// Translate ctx_docker into K8s YAMLs
/// 
/// * `ctx_docker` @param a docker context that contains spefications about the container
/// * `cmd` @param the command to run within the container
/// * @returns a series of rendered YAMLs to be provisioned as resources, or a RenderError
pub fn k8s_api<I, S>(ctx_docker: Context, cmd: I) -> Result<Vec<String>, RenderError>
  where I: IntoIterator<Item = S>, S: AsRef<OsStr>
{
    let renderer = get_renderer();
    let cmd_str: Vec<String> = cmd.into_iter().map(|s| s.as_ref().to_str().unwrap().to_owned()).collect();
    let ctx_modded = ctx_docker
        .set("command_str", CtxObj::Str(format!("{:?}", cmd_str)));
    Ok(vec![
        renderer.render("batch-job", &ctx_modded)?
    ])
}

#[cfg(feature = "lang_python")]
use pyo3::prelude::*;

#[cfg(feature = "lang_python")]
pub fn k8s_provisioner(resources: &Vec<String>) -> Result<(), TaskError> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    let src_k8s_provisioner = include_str!("hotwings_k8s_api.py");
    if let Ok(_) = py.run(&src_k8s_provisioner, None, None) {
        let provisioner = py.eval("k8s_provisioner", None, None).unwrap();
        for res in resources {
            info!("Creating kubernetes resource:");
            info!("{}", res);
            if let Ok(apicall) = py.eval(&format!(
                "lambda: jobApi.create_namespaced_job(namespace, body=yaml.safe_load(\"\"\"{}\"\"\"), pretty='true')",
                res
            ), None, None) {
                if let Err(api_exception) = provisioner.call1((apicall, )) {
                    api_exception.print(py);
                    return Err(TaskError {
                        msg: format!("An exception has occurred in the k8s provisioner script."),
                        src: TaskErrorSource::ExternalAPIError
                    });
                }
            }
        }
        Ok(())
    }
    else {
        Err(TaskError {
            msg: String::from("An internal error has occurred sourcing the k8s provisioner script."),
            src: TaskErrorSource::Internal
        })
    }
}
