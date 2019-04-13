#![allow(unused_imports)]
#![cfg(feature = "as_switch")]
#![cfg(feature = "sys_hotwings")]

use std::path::Path;
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
        let username = "hotwings";

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
        let home = format!("/home/{}", &username);
        let playbook_from: String = ctx_docker.unpack("playbook-from").unwrap();
        let env_nfs_server = std::env::var("HOTWINGS_NFS_SERVER").expect("Missing environment variable HOTWINGS_NFS_SERVER?");
        let env_currentro_quota = std::env::var("HOTWINGS_CURRENTRO_QUOTA").expect("Missing environment variable HOTWINGS_CURRENTRO_QUOTA?");
        let ctx_modded = ctx_docker
            .set("hotwings_user", CtxObj::Str(username.to_owned()))
            .set("hotwings_task_id", CtxObj::Str(get_task_id(&playbook_from)))
            .set("hotwings_nfs_server", CtxObj::Str(env_nfs_server.to_owned()))
            .set("hotwings_currentro_quota", CtxObj::Str(env_currentro_quota.to_owned())) // ! How to scale up/down?
            .set("hotwings_nvidia", CtxObj::Bool(ctx_docker.unpack("runtime").unwrap_or(String::from("")) == String::from("nvidia")))
            .set("hotwings_gpus", CtxObj::Int(
                if ctx_docker.unpack("runtime").unwrap_or(String::from("")) == String::from("nvidia") {
                    ctx_docker.unpack("gpus").unwrap_or(1i64)
                }
                else { 0 }
            ));

        match k8s_api(ctx_modded, cmd) {
            Ok(resources) => {
                match k8s_provisioner(&resources) {
                    Ok(()) => Ok(String::from(resources.iter().map(|(api, res)| res as &str).collect::<Vec<&str>>().join("\n"))),
                    Err(e) => Err(e)
                }
            },
            Err(e) => Err(TaskError { msg: e.desc, src: TaskErrorSource::Internal })
        }
    }
}

/// Get task id from playbook path
/// 
/// **Example**
/// ```text
/// /data/current-ro/0a6178f6-098d-4059-aaf0-9b6e0ea628d8/hello_inpod.yml
///   => 0a6178f6-098d-4059-aaf0-9b6e0ea628d8
/// ```
fn get_task_id<P: AsRef<Path>>(playbook: P) -> String {
    playbook.as_ref().parent().unwrap().file_name().unwrap().to_string_lossy().to_string()
}

/// Get the renderer with .hbs templates baked into the program
fn get_renderer() -> Handlebars {
    let mut renderer = Handlebars::new();
    renderer.register_template_string("batch-job", include_str!("templates-hotwings/batch.hbs")).unwrap();
    renderer.register_template_string("pv-current-ro", include_str!("templates-hotwings/pv.hbs")).unwrap();
    renderer.register_template_string("pvc-current-ro", include_str!("templates-hotwings/pvc.hbs")).unwrap();
    return renderer;
}

/// Translate ctx_docker into K8s YAMLs
/// 
/// * `ctx_docker` @param a docker context that contains spefications about the container
/// * `cmd` @param the command to run within the container
/// * @returns a series of rendered YAMLs to be provisioned as resources, or a RenderError
pub fn k8s_api<I, S>(ctx_docker: Context, cmd: I) -> Result<Vec<(String, String)>, RenderError>
  where I: IntoIterator<Item = S>, S: AsRef<OsStr>
{
    let renderer = get_renderer();
    let cmd_str: Vec<String> = cmd.into_iter().map(|s| s.as_ref().to_str().unwrap().to_owned()).collect();
    let ctx_modded = ctx_docker
        .set("command_str", CtxObj::Str(format!("[{}]", cmd_str.iter().map(|s| format!("'{}'", s)).collect::<Vec<String>>().join(","))));
    Ok(vec![
        (String::from("api_pv"), renderer.render("pv-current-ro", &ctx_modded)?),
        (String::from("api_pvc"), renderer.render("pvc-current-ro", &ctx_modded)?),
        (String::from("api_job"), renderer.render("batch-job", &ctx_modded)?),
    ])
}

#[cfg(feature = "lang_python")]
use pyo3::prelude::*;

#[cfg(feature = "lang_python")]
pub fn k8s_provisioner(resources: &Vec<(String, String)>) -> Result<(), TaskError> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    let src_k8s_provisioner = include_str!("hotwings_k8s_api.py");
    if let Err(provisioner_err) = py.run(&src_k8s_provisioner, None, None) {
        provisioner_err.print_and_set_sys_last_vars(py);
        Err(TaskError {
            msg: String::from("An internal error has occurred sourcing the k8s provisioner script."),
            src: TaskErrorSource::Internal
        })
    }
    else {
        let provisioner = py.eval("k8s_provisioner", None, None).unwrap();
        let join_job = py.eval("join_job", None, None).unwrap();
        for (api, res) in resources {
            info!("Creating kubernetes resource:");
            info!("{}", res);
            match provisioner.call1((api, res)) {
                Ok(api_return) => {
                    if api == "api_job" { // api_return is actually a job spec obj. Use that to join.
                        if let Err(join_exception) = join_job.call1((api_return, )) {
                            join_exception.print_and_set_sys_last_vars(py);
                            match py.run("sys.stderr.flush()", None, None) {
                                Ok(_) => {}
                                Err(_) => {}
                            }
                            return Err(TaskError {
                                msg: format!("An exception has occurred while joining the job execution."),
                                src: TaskErrorSource::ExternalAPIError
                            });
                        }
                    }
                },
                Err(api_exception) => {
                    api_exception.print_and_set_sys_last_vars(py);
                    return Err(TaskError {
                        msg: format!("An exception has occurred in the k8s provisioner script."),
                        src: TaskErrorSource::ExternalAPIError
                    });
                }
            }


            // TODO shouldn't clean up pv/pvc per step but we will need to do that at some point.
        }
        Ok(())
    }
}
