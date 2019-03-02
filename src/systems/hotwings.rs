use std::ffi::{CString, OsStr};
use std::collections::HashMap;
use std::path::Path;
use regex::Regex;
use nix::unistd::{fork, execvp, ForkResult};
use nix::sys::wait::{waitpid, WaitStatus};
use colored::Colorize;
use handlebars::Handlebars;
use ymlctx::context::{Context, CtxObj};
use crate::{TaskError, TaskErrorSource};

/// Hotwings - a K8s+Celery powered job system
/// 
/// * `ctx_docker` is a docker context that contains spefications about the container
/// * `cmd` is the command to run within the container
/// * returns YAML file that provisions the job using the batch/v1 K8s API
/// 
/// > Note: the return value is for informational purposes only, the necessary K8s resources
/// > would already have been provisioned.
#[cfg(feature = "sys_hotwings")]
pub fn hotwings_start<I, S>(ctx_docker: Context, cmd: I) -> Result<String, TaskError>
  where I: IntoIterator<Item = S>, S: AsRef<OsStr>
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
    Ok(String::from("dummy"))
}

fn get_renderer() -> Handlebars {
    let mut renderer = Handlebars::new();
    renderer.register_template_string("batch-job", include_str!("templates-hotwings/batch.hbs")).unwrap();
    return renderer;
}

#[cfg(feature = "sys_hotwings")]
pub fn k8s_api<I, S>(ctx_docker: Context, cmd: I) -> Vec<String>
  where I: IntoIterator<Item = S>, S: AsRef<OsStr>
{
    let mut renderer = get_renderer();
    let cmd_str: Vec<String> = cmd.into_iter().map(|s| s.as_ref().to_str().unwrap().to_owned()).collect();
    let a = renderer.render("batch-job", &ctx_docker
        .set("command_str", CtxObj::Str(format!("{:?}", cmd_str)))).unwrap();
    let mut ret = Vec::new();
    ret.push(a);
    return ret;
}
