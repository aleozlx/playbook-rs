use std::ffi::{CString, OsStr};
use std::collections::HashMap;
use std::path::Path;
use regex::Regex;
use nix::unistd::{fork, execvp, ForkResult};
use nix::sys::wait::{waitpid, WaitStatus};
use colored::Colorize;
use ymlctx::context::{Context, CtxObj};
use crate::{TaskError, TaskErrorSource};

pub fn inside_docker() -> bool {
    let status = std::process::Command::new("grep").args(&["-q", "docker", "/proc/1/cgroup"])
        .status().expect("I/O error");
    match status.code() {
        Some(code) => code==0,
        None => unreachable!()
    }
}

pub fn docker_start<I, S>(ctx_docker: Context, cmd: I) -> Result<String, TaskError>
  where I: IntoIterator<Item = S>, S: AsRef<OsStr>
{
    let username;
    let output = std::process::Command::new("id").output().unwrap();
    let mut id_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let newline_len = id_stdout.trim_right().len();
    id_stdout.truncate(newline_len);
    let rule = Regex::new(r"^uid=(?P<uid>[0-9]+)(\((?P<user>\w+)\))? gid=(?P<gid>[0-9]+)(\((?P<group>\w+)\))?").unwrap();
    if let Some(caps) = rule.captures(&id_stdout) {
        username = caps.name("user").unwrap().as_str().to_owned();
    }
    else {
        return Err(TaskError { msg: String::from("Failed to identify the user."), src: TaskErrorSource::Internal });
    }
    let mut userinfo = HashMap::new();
    crate::copy_user_info(&mut userinfo, &username);
    let home = format!("/home/{}", &username);
    let mut docker_run: Vec<String> = ["docker", "run", "--init", "--rm"].iter().map(|&s| {s.to_owned()}).collect();
    if let Some(CtxObj::Bool(interactive)) = ctx_docker.get("interactive") {
        if *interactive {
            docker_run.push(String::from("-it"));
        }
    }
    else {
        docker_run.push(String::from("-it"));
    }
    docker_run.push(String::from("--cap-drop=ALL"));
    if let Some(CtxObj::Str(runtime)) = ctx_docker.get("runtime") {
        docker_run.push(format!("--runtime={}", runtime));
    }
    if let Some(CtxObj::Str(ipc_namespace)) = ctx_docker.get("ipc") {
        docker_run.push(String::from("--ipc"));
        docker_run.push(ipc_namespace.to_owned());
    }
    if let Some(CtxObj::Str(net_namespace)) = ctx_docker.get("network") {
        docker_run.push(String::from("--network"));
        docker_run.push(net_namespace.to_owned());
    }
    docker_run.push(String::from("-v"));
    docker_run.push(format!("{}:{}/current-ro:ro", std::env::current_dir().unwrap().to_str().unwrap(), &home));
    docker_run.push(String::from("-w"));
    docker_run.push(format!("{}/current-ro", &home));
    if let Some(CtxObj::Array(volumes)) = ctx_docker.get("volumes") {
        for v in volumes {
            if let CtxObj::Str(vol) = v {
                if let Some(i) = vol.find(":") {
                    let (src, dst) = vol.split_at(i);
                    let suffix = if dst.ends_with(":ro") || dst.ends_with(":rw") || dst.ends_with(":z") || dst.ends_with(":Z") { "" } else { ":ro" };
                    if let Ok(src) = Path::new(src).canonicalize() {
                        docker_run.push(String::from("-v"));
                        docker_run.push(format!("{}{}{}", src.to_str().unwrap(), dst, suffix));
                    }
                }
            }
        }
    }
    if let Some(CtxObj::Array(ports)) = ctx_docker.get("ports") {
        for p in ports {
            if let CtxObj::Str(port_map) = p {
                docker_run.push(String::from("-p"));
                docker_run.push(port_map.to_owned());
            }
        }
    }
    if let Some(CtxObj::Bool(gui)) = ctx_docker.get("gui") {
        if *gui {
            // TODO verify permissions
            docker_run.extend::<Vec<String>>([
                "--network", "host", "-e", "DISPLAY", "-v", "/tmp/.X11-unix:/tmp/.X11-unix:rw",
                "-v", &format!("{}/.Xauthority:{}/.Xauthority:ro", userinfo["home_dir"], home), 
            ].iter().map(|&s| {s.to_owned()}).collect());
        }
    }
    if let Some(CtxObj::Array(envs)) = ctx_docker.get("environment") {
        for v in envs {
            if let CtxObj::Str(var) = v {
                docker_run.push(String::from("-e"));
                docker_run.push(var.to_owned());
            }
        }
    }
    if let Some(CtxObj::Str(impersonate)) = ctx_docker.get("impersonate") {
        if impersonate == "dynamic" {
            docker_run.push(String::from("--cap-add=SETUID"));
            docker_run.push(String::from("--cap-add=SETGID"));
            docker_run.push(String::from("--cap-add=CHOWN")); // TODO possibility to restrict this?
            docker_run.push(String::from("-u"));
            docker_run.push(String::from("root"));
            docker_run.push(String::from("-e"));
            docker_run.push(format!("IMPERSONATE={}", &id_stdout));
            docker_run.push(String::from("--entrypoint"));
            docker_run.push(String::from("/usr/bin/playbook"));
        }
        else {
            docker_run.push(String::from("-u"));
            docker_run.push(impersonate.to_owned());
        }
    }
    else {
        docker_run.push(String::from("-u"));
        docker_run.push(format!("{}:{}", userinfo["uid"], userinfo["gid"]));
    }
    if let Some(CtxObj::Str(name)) = ctx_docker.get("name") {
        docker_run.push(format!("--name={}", name));
    }
    if let Some(CtxObj::Str(image_name)) = ctx_docker.get("image") {
        docker_run.push(image_name.to_owned());
    }
    else {
        return Err(TaskError {  msg: String::from("The Docker image specification was invalid."), src: TaskErrorSource::Internal });
    }
    docker_run.extend::<Vec<String>>(cmd.into_iter().map(|s| {s.as_ref().to_str().unwrap().to_owned()}).collect());
    let docker_cmd = crate::format_cmd(docker_run.clone());
    info!("{}", &docker_cmd);
    #[cfg(feature = "ci_only")] // Let's see the docker command during testing.
    println!("{}", &docker_cmd);
    let docker_linux: Vec<CString> = docker_run.iter().map(|s| {CString::new(s as &str).unwrap()}).collect();
    match fork() {
        Ok(ForkResult::Child) => {
            match execvp(&CString::new("docker").unwrap(), &docker_linux) {
                Ok(_void) => unreachable!(),
                Err(e) => Err(TaskError { msg: format!("Failed to issue the Docker command. {}", e), src: TaskErrorSource::NixError(e) }),
            }
        },
        Ok(ForkResult::Parent { child, .. }) => {
            match waitpid(child, None) {
                Ok(status) => match status {
                    WaitStatus::Exited(_, exit_code) => {
                        if exit_code == 0 { Ok(docker_cmd) }
                        else {
                            Err(TaskError {
                                msg: format!("The container has returned a non-zero exit code ({}).", exit_code.to_string().red()),
                                src: TaskErrorSource::ExitCode(exit_code)
                            })
                        }
                    },
                    WaitStatus::Signaled(_, sig, _core_dump) => {
                        Err(TaskError {
                            msg: format!("The container has received a signal ({:?}).", sig),
                            src: TaskErrorSource::Signal(sig)
                        })
                    },
                    WaitStatus::Stopped(_, _sig) => unreachable!(),
                    WaitStatus::PtraceEvent(..) => unimplemented!(),
                    WaitStatus::PtraceSyscall(_) => unimplemented!(),
                    WaitStatus::Continued(_) => unreachable!(),
                    WaitStatus::StillAlive => unreachable!()
                },
                Err(e) => Err(TaskError { msg: String::from("Failed to keep track of the child process."), src: TaskErrorSource::NixError(e) })
            }
        },
        Err(e) => Err(TaskError { msg: String::from("Failed to spawn a new process."), src: TaskErrorSource::NixError(e) }),
    }
}
