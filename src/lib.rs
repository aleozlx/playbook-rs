#[macro_use]
extern crate log;

extern crate yaml_rust;
extern crate ymlctx;
extern crate colored;
extern crate regex;
extern crate nix;
extern crate impersonate;

#[cfg(feature = "lang_python")]
extern crate pyo3;

use std::str;
use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::collections::HashMap;
use std::result::Result;
use std::collections::HashSet;
use yaml_rust::{Yaml, YamlLoader};
use colored::*;
use regex::Regex;
pub use ymlctx::context::{Context, CtxObj};
pub mod container;
pub mod lang;

pub enum ExitCode {
    Success,
    ErrSys,
    ErrApp,
    ErrYML,
    ErrTask
}

pub fn exit(code: ExitCode) -> ! {
    // Any clean up?
    std::process::exit(match code {
        ExitCode::Success => 0,
        ExitCode::ErrSys => 1,
        ExitCode::ErrApp => 2,
        ExitCode::ErrYML => 3,
        ExitCode::ErrTask => 4
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskErrorSource {
    NixError(nix::Error),
    ExitCode(i32),
    Signal(nix::sys::signal::Signal),
    Internal
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskError {
    msg: String,
    src: TaskErrorSource
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", &self.msg)
    }
}

pub fn copy_user_info(facts: &mut HashMap<String, String>, user: &str) {
    if let Some(output) = std::process::Command::new("getent").args(&["passwd", &user]).output().ok() {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let fields: Vec<&str> = stdout.split(":").collect();
        facts.insert(String::from("uid"), String::from(fields[2]));
        facts.insert(String::from("gid"), String::from(fields[3]));
        facts.insert(String::from("full_name"), String::from(fields[4]));
        facts.insert(String::from("home_dir"), String::from(fields[5]));
    }
}

pub fn format_cmd<I>(cmd: I) -> String
  where I: IntoIterator<Item = String>
{
    cmd.into_iter().map(|s| { if s.contains(" ") { format!("\"{}\"", s) } else { s.to_owned() } }).collect::<Vec<String>>().join(" ")
}

type BuiltIn = fn(Context);
type TaskSpawner = fn(src: Context, ctx_step: Context) -> Result<(), TaskError>;

fn sys_exit(ctx: Context) {
    std::process::exit(if let Ok(exit_code) = ctx.unpack("exit_code") { exit_code } else { 0 });
}

fn sys_shell(ctx: Context) {
    if let Some(ctx_docker) = ctx.subcontext("docker") {
        if let Some(CtxObj::Array(bash_cmd)) = ctx.get("bash") {
            let cmd = format_cmd(bash_cmd.iter().map(|arg| {
                match arg {
                    CtxObj::Str(s) => s.to_owned(),
                    _ => String::from("")
                }
            }));
            match container::docker_start(ctx_docker.hide("impersonate"), &["bash", "-c", &cmd]) {
                // Note: it is not secure to transition from the playbook to a shell, so "dynamic" impersonate is not an option
                Ok(_) => {
                    exit(ExitCode::Success);
                },
                Err(_) => {
                    error!("Docker crashed.");
                    exit(ExitCode::ErrYML);
                }
            }
        }
        else {
            warn!("{}", "Just a bash shell. Here goes nothing.".purple());
            match container::docker_start(ctx_docker.set("interactive", CtxObj::Bool(true)).hide("impersonate"), &["bash"]) {
                Ok(_) => {
                    exit(ExitCode::Success);
                },
                Err(_) => {
                    error!("Docker crashed.");
                    exit(ExitCode::ErrYML);
                }
            }
        }
    }
    else {
        error!("Docker context not found!");
        exit(ExitCode::ErrYML);
    }
}

fn invoke(src: Context, ctx_step: Context) {
    let ref action: String = ctx_step.unpack("action").unwrap();
    let ref src_path_str: String = src.unpack("src").unwrap();
    if !cfg!(feature = "ci_only") {
        eprintln!("{}", "== Context ======================".cyan());
        eprintln!("# ctx({}@{}) =\n{}", action.cyan(), src_path_str.dimmed(), ctx_step);
        eprintln!("{}", "== EOF ==========================".cyan());
    }
    let src_path = Path::new(src_path_str);
    if let Some(ext_os) = src_path.extension() {
        let ext = ext_os.to_str().unwrap();
        #[allow(unused_variables)]
        let wrapper = |whichever: TaskSpawner| -> Result<(), Option<String>> {
            let last_words;
            #[cfg(not(feature = "ci_only"))]
            println!("{}", "== Output =======================".blue());
            last_words = if let Err(e) = whichever(src, ctx_step) {
                match e.src {
                    TaskErrorSource::NixError(_) | TaskErrorSource::ExitCode(_) | TaskErrorSource::Signal(_) => {
                        Err(Some(format!("{}", e)))
                    },
                    TaskErrorSource::Internal => Err(None)
                }
            }
            else { Ok(()) };
            #[cfg(not(feature = "ci_only"))]
            println!("{}", "== EOF ==========================".blue());
            return last_words;
        };
        let ret: Result<(), Option<String>> = match ext {
            #[cfg(feature = "lang_python")]
            "py" => wrapper(lang::python::invoke),
            _ => Err(Some(format!("It is not clear how to run {}.", src_path_str)))
        };
        if let Err(last_words) = ret {
            if let Some(msg) = last_words {
                error!("{}", msg);
            }
            exit(ExitCode::ErrTask);
        }
    }
    else {
        // TODO C-style FFI invocation
        unimplemented!();
    }
}

fn symbols<P: AsRef<Path>>(src: P) -> Result<HashSet<String>, std::io::Error> {
    let mut ret = HashSet::new();
    let file = File::open(src)?;
    let re = Regex::new(r"^#\[playbook\((\w+)\)\]").unwrap();
    for line in BufReader::new(file).lines() {
        let ref line = line?;
        if let Some(caps) = re.captures(line){
            ret.insert(caps.get(1).unwrap().as_str().to_owned());
        }
    }
    Ok(ret)
}

fn resolve<'step>(ctx_step: &'step Context, whitelist: &Vec<Context>) -> (Option<&'step str>, Option<Context>) {
    let key_action;
    if let Some(k) = ctx_step.get("action") { key_action = k; }
    else { return (None, None); }
    if let CtxObj::Str(action) = key_action {
        let action: &'step str = action;
        for ctx_source in whitelist {
            if let Some(CtxObj::Str(src)) = ctx_source.get("src") {
                let ref playbook: String = ctx_step.unpack("playbook").unwrap();
                let playbook_dir;
                if let Some(parent) = Path::new(playbook).parent() {
                    playbook_dir = parent;
                }
                else {
                    playbook_dir = Path::new(".");
                }
                let ref src_path = playbook_dir.join(src);
                let src_path_str = src_path.to_str().unwrap();
                debug!("Searching \"{}\" for `{}`.", src_path_str, action);
                if let Ok(src_synbols) = symbols(src_path) {
                    if src_synbols.contains(action) {
                        debug!("Action `{}` has been found.", action);
                        return(Some(action), Some(ctx_source.set("src", CtxObj::Str(src_path_str.to_owned()))));
                    }
                }
                else {
                    warn!("IO Error: {}", src_path_str);
                }
            }
        }
        (Some(action), None)
    }
    else {
        (None, None)
    }
}

fn resolve_builtin<'step>(ctx_step: &'step Context) -> (Option<&'step str>, Option<BuiltIn>) {
    if let Some(CtxObj::Str(action)) = ctx_step.get("action") {
        let action: &'step str = action;
        match action {
            "sys_exit" => (Some(action), Some(sys_exit)),
            "sys_shell" => (Some(action), Some(sys_shell)),
            _ => (Some(action), None)
        }
    }
    else { (None, None) }
}

fn run_step(ctx_step: Context) {
    if let Some(whitelist) = ctx_step.list_contexts("whitelist") {
        match resolve(&ctx_step, &whitelist) {
            (_, Some(ctx_source)) => {
                let i_step: usize = ctx_step.unpack("i_step").unwrap();
                let show_step = |for_real: bool| {
                    let step_header = format!("Step {}", i_step+1).cyan();
                    if let Some(CtxObj::Str(step_name)) = ctx_step.get("name") {
                        info!("{}: {}", if for_real { step_header } else { step_header.dimmed() }, step_name);
                    }
                    else {
                        info!("{}", if for_real { step_header } else { step_header.dimmed() });
                    }
                };
                if let Some(CtxObj::Str(_)) = ctx_step.get("docker-step") {
                    show_step(true);
                    invoke(ctx_source, ctx_step.hide("whitelist").hide("i_step"));
                }
                else {
                    if let Some(ctx_docker) = ctx_step.subcontext("docker") {
                        show_step(false);
                        if let Some(CtxObj::Str(image_name)) = ctx_docker.get("image") {
                            info!("Entering Docker: {}", image_name.purple());
                            let mut resume_params = vec! [
                                format!("--docker-step={}", i_step),
                                ctx_step.unpack("playbook").unwrap()
                            ];
                            let relocate_unpack = ctx_step.unpack("relocate");
                            if let Ok(relocate) = relocate_unpack {
                                resume_params.push(relocate);
                            }
                            let verbose_unpack = ctx_step.unpack("verbose-fern");
                            if let Ok(verbose) = verbose_unpack {
                                if verbose > 0 {
                                    resume_params.push(format!("-{}", "v".repeat(verbose)));
                                }
                            }
                            match container::docker_start(ctx_docker.clone(), resume_params) {
                                Ok(_docker_cmd) => {},
                                Err(e) => {
                                    match e.src {
                                        TaskErrorSource::NixError(_) | TaskErrorSource::ExitCode(_) | TaskErrorSource::Signal(_) => {
                                            error!("{}: {}", "Container has crashed".red().bold(), e);
                                        },
                                        TaskErrorSource::Internal => ()
                                    }
                                    exit(ExitCode::ErrTask);
                                }
                            }
                        }
                    }
                    else {
                        show_step(true);
                        invoke(ctx_source, ctx_step.hide("whitelist").hide("i_step"));
                    }
                }
            },
            (Some(action), None) => {
                match resolve_builtin(&ctx_step) {
                    (_, Some(sys_func)) => {
                        let ctx_sys = ctx_step.hide("whitelist").hide("i_step");
                        info!("{}: {}", "Built-in".magenta(), action);
                        if !cfg!(feature = "ci_only") {
                            eprintln!("{}", "== Context ======================".cyan());
                            eprintln!("# ctx({}) =\n{}", action.cyan(), ctx_sys);
                            eprintln!("{}", "== EOF ==========================".cyan());
                        }
                        sys_func(ctx_sys);
                    },
                    (Some(_), None) => {
                        error!("Action not recognized: {}", action);
                        exit(ExitCode::ErrYML);
                    },
                    (None, None) => unreachable!()
                }
            },
            (None, None) => {
                error!("Syntax Error: Key `action` must be a string.");
                exit(ExitCode::ErrYML);
            }
        }
    }
    else {
        match resolve_builtin(&ctx_step) {
            (Some(action), Some(sys_func)) => {
                let ctx_sys = ctx_step.hide("whitelist").hide("i_step");
                info!("{}: {}", "Built-in".magenta(), action);
                if !cfg!(feature = "ci_only") {
                    eprintln!("{}", "== Context ======================".cyan());
                    eprintln!("# ctx({}) =\n{}", action.cyan(), ctx_sys);
                    eprintln!("{}", "== EOF ==========================".cyan());
                }
                sys_func(ctx_sys);
            },
            (Some(action), None) => {
                error!("Action not recognized: {}", action);
                exit(ExitCode::ErrYML);
            },
            (None, _) => {
                error!("Syntax Error: Key `whitelist` should be a list of mappings.");
                exit(ExitCode::ErrYML);
            }
        }
    }    
}

pub fn run_yaml<P: AsRef<Path>>(playbook: P, ctx_args: Context) -> Result<(), std::io::Error> {
    let enter_partial = |ctx_partial: Context| {
        debug!("ctx({}) =\n{}", "partial".dimmed(), ctx_partial);
        if let Some(CtxObj::Str(_)) = ctx_partial.get("docker-step") {
            run_step(
                if let Some(ctx_docker) = ctx_partial.subcontext("docker").unwrap().subcontext("vars") {
                    ctx_partial.overlay(&ctx_docker).hide("docker")
                }
                else { ctx_partial.hide("docker") });
        }
        else {
            run_step(ctx_partial);
        }
    };
    
    let enter_steps = |steps: Vec<Context>, ctx_global: Context| {
        if let Some(CtxObj::Str(i_step_str)) = ctx_args.get("docker-step") {
            // ^^ Then we must be in a docker container because main() has guaranteed that.
            if let Ok(i_step) = i_step_str.parse::<usize>() {
                let ctx_step = steps[i_step].clone();
                let ctx_partial = ctx_global.overlay(&ctx_step).overlay(&ctx_args);
                enter_partial(ctx_partial.set("i_step", CtxObj::Int(i_step as i64)));
            }
            else {
                error!("Syntax Error: Cannot parse the `--docker-step` flag.");
                exit(ExitCode::ErrApp);
            }
            exit(ExitCode::Success);
        }
        for (i_step, ctx_step) in steps.iter().enumerate() {
            let ctx_partial = ctx_global.overlay(&ctx_step).overlay(&ctx_args);
            enter_partial(ctx_partial.set("i_step", CtxObj::Int(i_step as i64)));
        }
    };

    let enter_global = |yml_global: &Yaml| {
        let raw = Context::from(yml_global.to_owned());
        let ctx_global = raw.hide("steps");
        if let Some(steps) = raw.list_contexts("steps") {
            enter_steps(steps, ctx_global);
        }
        else {
            error!("Syntax Error: Key `steps` is not an array.");
            exit(ExitCode::ErrYML);
        }
    };

    let fname = playbook.as_ref();
    if let Some(playbook_ext) = fname.extension() {
        if playbook_ext != "yml" && playbook_ext != "yaml" {
            warn!("{}", "The playbook file is not YAML based on its extension.".yellow());
        }
    }
    let mut file = File::open(fname)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    match YamlLoader::load_from_str(&contents) {
        Ok(yml_global) => { enter_global(&yml_global[0]); },
        Err(e) => {
            error!("{}: {}", e, "Some YAML parsing error has occurred.");
            exit(ExitCode::ErrYML);
        }
    }
    Ok(())
}
