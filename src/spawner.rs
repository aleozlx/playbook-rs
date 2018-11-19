use std::path::Path;
use std::result::Result;
use std::ffi::{CString, OsStr};
use std::collections::HashMap;
use ymlctx::context::{Context, CtxObj};
use pyo3::prelude::*;
use pyo3::types::PyList;
use regex::Regex;
use nix::unistd::{fork, execvp, ForkResult};
use nix::sys::wait::{waitpid, WaitStatus};
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

fn copy_user_info(facts: &mut HashMap<String, String>, user: &str) {
    if let Some(output) = std::process::Command::new("getent").args(&["passwd", &user]).output().ok() {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let fields: Vec<&str> = stdout.split(":").collect();
        facts.insert(String::from("uid"), String::from(fields[2]));
        facts.insert(String::from("gid"), String::from(fields[3]));
        facts.insert(String::from("full_name"), String::from(fields[4]));
        facts.insert(String::from("home_dir"), String::from(fields[5]));
    }
}

pub fn docker_start<I, S>(ctx_docker: Context, cmd: I) -> Result<(), JobError>
  where I: IntoIterator<Item = S>, S: AsRef<OsStr>
{
    let username;
    let output = std::process::Command::new("id").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let rule = Regex::new(r"^uid=(?P<uid>[0-9]+)(\((?P<user>\w+)\))? gid=(?P<gid>[0-9]+)(\((?P<group>\w+)\))?").unwrap();
    if let Some(caps) = rule.captures(&stdout) {
        username = caps.name("user").unwrap().as_str().to_owned();
    }
    else { return Err(JobError {}); }
    let mut userinfo = HashMap::new();
    copy_user_info(&mut userinfo, &username);
    let home = format!("/home/{}", &username);
    let mut docker_run: Vec<String> = ["docker", "run", "--rm", "-t", "--net=host"].iter().map(|&s| {s.to_owned()}).collect();
    // docker_run.push(String::from("-v"));
    // docker_run.push(format!("{}:/usr/bin/playbook", std::env::current_exe().unwrap().to_str().unwrap()));
    docker_run.push(String::from("-v"));
    docker_run.push(format!("{}:{}/current-ro", std::env::current_dir().unwrap().to_str().unwrap(), &home));
    docker_run.push(String::from("-w"));
    docker_run.push(format!("{}/current-ro", &home));
    // docker_run.push(format!("--user={}", username)); // TODO add gid
    if let Some(CtxObj::Str(runtime)) = ctx_docker.get("runtime") {
        docker_run.push(format!("--runtime={}", runtime));
    }
    if let Some(CtxObj::Bool(interactive)) = ctx_docker.get("interactive") {
        if *interactive {
            docker_run.push(String::from("-i"));
        }
    }
    if let Some(CtxObj::Array(volumes)) = ctx_docker.get("volumes") {
        for v in volumes {
            if let CtxObj::Str(vol) = v {
                if let Some(i) = vol.find(":") {
                    let (src, dst) = vol.split_at(i);
                    if let Ok(src) = Path::new(src).canonicalize() {
                        docker_run.push(String::from("-v"));
                        docker_run.push(format!("{}:{}", src.to_str().unwrap(), dst));
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
            docker_run.extend::<Vec<String>>([
                "-e", "DISPLAY", "-v", "/tmp/.X11-unix:/tmp/.X11-unix:rw",
                "-v", &format!("{}/.Xauthority:{}/.Xauthority:ro", userinfo["home_dir"], home), 
            ].iter().map(|&s| {s.to_owned()}).collect());
        }
    }
    if let Some(CtxObj::Str(container_name)) = ctx_docker.get("container_name") {
        docker_run.push(format!("--name={}", container_name));
    }
    if let Some(CtxObj::Str(image_name)) = ctx_docker.get("image") {
        docker_run.push(image_name.to_owned());
    }
    else { return Err(JobError {}); }
    docker_run.extend::<Vec<String>>(cmd.into_iter().map(|s| {s.as_ref().to_str().unwrap().to_owned()}).collect());
    info!("{:?}", &docker_run);
    let docker_linux: Vec<CString> = docker_run.iter().map(|s| {CString::new(s as &str).unwrap()}).collect();
    match fork() {
        Ok(ForkResult::Child) => {
            match execvp(&CString::new("docker").unwrap(), &docker_linux) {
                Ok(_) => Ok(()),
                Err(_) => Err(JobError {}),
            }
        },
        Ok(ForkResult::Parent { child, .. }) => {
            match waitpid(child, None) {
                Ok(status) => match status {
                    WaitStatus::Exited(_, exit_code) => {
                        if exit_code == 0 { Ok(()) }
                        else { Err(JobError {}) } // TODO return exit_code
                    },
                    WaitStatus::Signaled(_, _sig, _core_dump) => {
                        Err(JobError {}) // TODO report signal
                    },
                    WaitStatus::Stopped(_, _sig) => unreachable!(),
                    WaitStatus::PtraceEvent(..) => unimplemented!(),
                    WaitStatus::PtraceSyscall(_) => unimplemented!(),
                    WaitStatus::Continued(_) => unreachable!(),
                    WaitStatus::StillAlive => unreachable!()
                },
                Err(_) => Err(JobError {})
            }
        },
        Err(_) => Err(JobError {}),
    }
}


