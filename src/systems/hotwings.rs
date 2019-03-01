use std::ffi::{CString, OsStr};
use std::collections::HashMap;
use std::path::Path;
use regex::Regex;
use nix::unistd::{fork, execvp, ForkResult};
use nix::sys::wait::{waitpid, WaitStatus};
use colored::Colorize;
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
#[cfg(feature = "as_switch")]
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

// * Sample yaml
// ---
// apiVersion: batch/v1
// kind: Job
// metadata:
//   generateName: test-job-
// spec:
//   template:
//     metadata:
//       name: test_job
//     spec:
//       containers:
//         - name: test
//           image: busybox
//           command: ["hostname"]
//       restartPolicy: Never

    

}
