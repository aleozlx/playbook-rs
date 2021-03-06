#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

// #[macro_use]
extern crate itertools;

extern crate yaml_rust;
extern crate ymlctx;
extern crate colored;
extern crate regex;
extern crate nix;
extern crate impersonate;
extern crate serde_json;
extern crate uuid;
extern crate libc;

#[cfg(feature = "lang_python")]
extern crate pyo3;

#[cfg(feature = "as_switch")]
extern crate handlebars;

pub use ymlctx::context::{Context, CtxObj};
pub mod lang;
pub mod builtins;
pub mod systems;

use std::str;
use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Write};
use std::collections::HashMap;
use std::result::Result;
use std::collections::HashSet;
use yaml_rust::YamlLoader;
use colored::*;
use regex::Regex;
use builtins::{TransientContext, ExitCode};
use systems::Infrastructure;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskErrorSource {
    NixError(nix::Error),
    ExitCode(i32),
    Signal(nix::sys::signal::Signal),
    Internal,
    ExternalAPIError
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Closure {
    #[serde(rename = "c")]
    container: u8,
    #[serde(rename = "p")]
    step_ptr: usize,
    #[serde(rename = "s")]
    pub ctx_states: Context,
}

#[test]
fn test_closure_deserialize00() {
    let closure_str = r#"{"c":1,"p":0,"s":{"data":{}}}"#;
    assert_eq!(serde_json::from_str::<Closure>(closure_str).unwrap(), Closure {
        container: 1,
        step_ptr: 0,
        ctx_states: Context::new()
    });
}

#[test]
fn test_closure_deserialize01() {
    let closure_str = r#"{"c":1,"p":0,"s":{"data":{"playbook":{"Str":"tests/test1/say_hi.yml"}}}}"#;
    assert_eq!(serde_json::from_str::<Closure>(closure_str).unwrap(), Closure {
        container: 1,
        step_ptr: 0,
        ctx_states: Context::new().set("playbook", CtxObj::Str(String::from("tests/test1/say_hi.yml")))
    });
}

#[test]
fn test_closure_deserialize02() {
    let closure_str = r#"{"c":1,"p":1,"s":{"data":{"playbook":{"Str":"tests/test1/test_sys_vars.yml"},"message":{"Str":"Salut!"}}}}"#;
    assert_eq!(serde_json::from_str::<Closure>(closure_str).unwrap(), Closure {
        container: 1,
        step_ptr: 1,
        ctx_states: Context::new()
            .set("playbook", CtxObj::Str(String::from("tests/test1/test_sys_vars.yml")))
            .set("message", CtxObj::Str(String::from("Salut!")))
    });
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

fn read_contents<P: AsRef<Path>>(fname: P) -> Result<String, std::io::Error> {
    let mut contents = String::new();
    let mut file = File::open(fname)?;
    file.read_to_string(&mut contents)?;
    return Ok(contents);
}

pub fn format_cmd<I>(cmd: I) -> String
  where I: IntoIterator<Item = String>
{
    cmd.into_iter().map(|s| { if s.contains(" ") { format!("\"{}\"", s) } else { s.to_owned() } }).collect::<Vec<String>>().join(" ")
}

type TaskSpawner = fn(src: Context, ctx_step: Context) -> Result<(), TaskError>;

#[cfg(not(feature = "sandbox"))] // protect the host by removing the entrance to all user codes!
fn invoke(src: Context, ctx_step: Context) -> Result<Context, ExitCode> {
    let ref action: String = ctx_step.unpack("action").unwrap();
    let ref src_path_str: String = src.unpack("src").unwrap();
    if !cfg!(feature = "ci_only") {
        eprintln!("{}", "== Context ======================".cyan());
        eprintln!("# ctx({}@{}) =\n{}", action.cyan(), src_path_str.dimmed(), ctx_step);
        eprintln!("{}", "== EOF ==========================".cyan());
        match std::io::stderr().flush() {
            Ok(_) => {},
            Err(_) => {}
        }
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
                    TaskErrorSource::Internal => Err(None),
                    TaskErrorSource::ExternalAPIError => unreachable!()
                }
            }
            else { Ok(()) };
            #[cfg(not(feature = "ci_only"))]
            println!("{}", "== EOF ==========================".blue());
            match std::io::stdout().flush() {
                Ok(_) => {},
                Err(_) => {}
            }
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
            Err(ExitCode::ErrTask)
        }
        else {
            Ok(Context::new()) // TODO pass return value back as a context
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

fn try_as_builtin(ctx_step: &Context, closure: &Closure) -> TransientContext {
    match builtins::resolve(&ctx_step) {
        (Some(action), Some(sys_func)) => {
            let ctx_sys = ctx_step.overlay(&closure.ctx_states).hide("whitelist");
            info!("{}: {}", "Built-in".magenta(), action);
            if !cfg!(feature = "ci_only") {
                eprintln!("{}", "== Context ======================".cyan());
                eprintln!("# ctx({}) =\n{}", action.cyan(), ctx_sys);
                eprintln!("{}", "== EOF ==========================".cyan());
            }
            sys_func(ctx_sys)
        },
        (Some(action), None) => {
            error!("Action not recognized: {}", action);
            TransientContext::Diverging(ExitCode::ErrYML)
        },
        (None, _) => {
            error!("Syntax Error: Key `whitelist` should be a list of mappings.");
            TransientContext::Diverging(ExitCode::ErrYML)
        }
    }
}

fn run_step(ctx_step: Context, closure: Closure) -> TransientContext {
    if let Some(whitelist) = ctx_step.list_contexts("whitelist") {
        match resolve(&ctx_step, &whitelist) {
            (_, Some(ctx_source)) => {
                let show_step = |for_real: bool| {
                    let step_header = format!("Step {}", closure.step_ptr+1).cyan();
                    if let Some(CtxObj::Str(step_name)) = ctx_step.get("name") {
                        info!("{}: {}", if for_real { step_header } else { step_header.dimmed() }, step_name);
                    }
                    else {
                        info!("{}", if for_real { step_header } else { step_header.dimmed() });
                    }
                };
                if closure.container == 1 {
                    #[cfg(feature = "sandbox")] unreachable!();
                    #[cfg(not(feature = "sandbox"))]
                    {
                        show_step(true);
                        TransientContext::from(invoke(ctx_source, ctx_step.hide("whitelist")))
                    }
                }
                else {
                    if let Some(ctx_docker) = ctx_step.subcontext("docker") {
                        show_step(false);
                        if let Some(CtxObj::Str(image_name)) = ctx_docker.get("image") {
                            info!("Entering Docker: {}", image_name.purple());
                            let mut closure1 = closure.clone();
                            closure1.container = 1;
                            // Register any reassignment of "playbook" to the ctx_states to prolong its lifetime
                            if let Some(ctx_docker_vars) = ctx_docker.subcontext("vars") {
                                closure1.ctx_states = closure1.ctx_states.set_opt("playbook", ctx_docker_vars.get_clone("playbook"));
                            }
                            let mut resume_params = vec! [
                                String::from("--arg-resume"),
                                match serde_json::to_string(&closure1) {
                                    Ok(s) => s,
                                    Err(_) => {
                                        error!("Failed to serialize states.");
                                        return TransientContext::Diverging(ExitCode::ErrApp)
                                    }
                                },
                                ctx_step.unpack("playbook").unwrap()
                            ];
                            let verbose_unpack = ctx_step.unpack("verbose-fern");
                            if let Ok(verbose) = verbose_unpack {
                                if verbose > 0 {
                                    resume_params.push(format!("-{}", "v".repeat(verbose)));
                                }
                            }
                            let infrastructure_str = if let Some(CtxObj::Str(s)) = ctx_step.get("as-switch") { s } else { "docker" };
                            info!("Selected infrastructure: {}", infrastructure_str);
                            if let Some(infrastructure) = systems::abstract_infrastructures(&infrastructure_str) {
                                match infrastructure.start(ctx_docker.set_opt("playbook-from", ctx_step.get_clone("playbook")), resume_params) {
                                    Ok(_docker_cmd) => {
                                        TransientContext::from(Ok(Context::new())) // TODO pass return value back as a context
                                    },
                                    Err(e) => {
                                        match e.src {
                                            TaskErrorSource::NixError(_) | TaskErrorSource::ExitCode(_) | TaskErrorSource::Signal(_) => {
                                                error!("{}: {}", "Container has crashed".red().bold(), e);
                                            },
                                            TaskErrorSource::Internal => {
                                                error!("{}: {}", "InternalError".red().bold(), e);
                                            },
                                            TaskErrorSource::ExternalAPIError => {
                                                error!("{}: {}", "ExternalAPIError".red().bold(), e);
                                            }
                                        }
                                        TransientContext::Diverging(ExitCode::ErrTask)
                                    }
                                }
                            }
                            else {
                                error!("Undefined infrastructure.");
                                TransientContext::Diverging(ExitCode::ErrApp)
                            }
                        }
                        else {
                            error!("Syntax Error: Cannot parse the name of the image.");
                            TransientContext::Diverging(ExitCode::ErrYML)
                        }
                    }
                    else {
                        #[cfg(feature = "sandbox")] unreachable!();
                        #[cfg(not(feature = "sandbox"))]
                        {
                            show_step(true);
                            TransientContext::from(invoke(ctx_source, ctx_step.hide("whitelist")))
                        }
                    }
                }
            },
            (Some(_action), None) => {
                try_as_builtin(&ctx_step, &closure)
            },
            (None, None) => {
                error!("Syntax Error: Key `action` must be a string.");
                TransientContext::Diverging(ExitCode::ErrYML)
            }
        }
    }
    else {
        try_as_builtin(&ctx_step, &closure)
    }    
}

fn deduce_context(ctx_step_raw: &Context, ctx_global: &Context, ctx_args: &Context, closure: &Closure) -> Context {
    let ctx_partial = ctx_global.overlay(ctx_step_raw).overlay(ctx_args).overlay(&closure.ctx_states);
    debug!("ctx({}) =\n{}", "partial".dimmed(), ctx_partial);
    if let Some(CtxObj::Str(_)) = ctx_partial.get("arg-resume") {
        if let Some(ctx_docker_vars) = ctx_partial.subcontext("docker").unwrap().subcontext("vars") {
            ctx_partial.overlay(&ctx_docker_vars).hide("docker")
        }
        else { ctx_partial.hide("docker") }
    }
    else { ctx_partial }
}

fn get_steps(raw: Context) -> Result<(Vec<Context>, Context), ExitCode> {
    let ctx_global = raw.hide("steps");
    if let Some(steps) = raw.list_contexts("steps") {
        Ok((steps, ctx_global))
    }
    else {
        Err(ExitCode::ErrYML)
    }
}

/// Correctly exit from a sys_fork action
fn maybe_exit(exit_code: ExitCode, ctx_states: &Context) -> ExitCode {
    if let Some(CtxObj::Bool(noreturn)) = ctx_states.get("_exit") {
        if *noreturn {
            unsafe { libc::_exit(0); }
        }
    }
    exit_code
}

pub fn run_playbook(raw: Context, ctx_args: Context) -> Result<(), ExitCode> {
    let mut ctx_states = Box::new(Context::new());
    let (steps, ctx_global) = match get_steps(raw) {
        Ok(v) => v,
        Err(e) => {
            error!("Syntax Error: Key `steps` is not an array.");
            return Err(e);
        }
    };
    if let Some(CtxObj::Str(closure_str)) = ctx_args.get("arg-resume") {
        // ^^ Then we must be in a docker container because main() has guaranteed that.
        match serde_json::from_str::<Closure>(closure_str) {
            Ok(closure) => {
                let ctx_step = deduce_context(&steps[closure.step_ptr], &ctx_global, &ctx_args, &closure);
                match run_step(ctx_step, closure) {
                    TransientContext::Stateful(_) | TransientContext::Stateless(_) => Ok(()),
                    TransientContext::Diverging(exit_code) => match exit_code {
                        ExitCode::Success => Ok(()),
                        _ => Err(exit_code)
                    }
                }
            }
            Err(_e) => {
                error!("Syntax Error: Cannot parse the `--arg-resume` flag. {}", closure_str.underline());
                #[cfg(feature = "ci_only")]
                eprintln!("{}", _e);
                Err(ExitCode::ErrApp)
            }
        }
    }
    else {
        for (i, ctx_step_raw) in steps.iter().enumerate() {
            let closure = Closure { container: 0, step_ptr: i, ctx_states: ctx_states.as_ref().clone() };
            let ctx_step = deduce_context(ctx_step_raw, &ctx_global, &ctx_args, &closure);
            match run_step(ctx_step, closure) {
                TransientContext::Stateless(_) => { }
                TransientContext::Stateful(ctx_pipe) => {
                    ctx_states = Box::new(ctx_states.overlay(&ctx_pipe));
                }
                TransientContext::Diverging(exit_code) => match maybe_exit(exit_code, &ctx_states) {
                    ExitCode::Success => { return Ok(()); }
                    exit_code @ _ => { return Err(exit_code); }
                }
            }
        }
        maybe_exit(ExitCode::Success, &ctx_states);
        Ok(())
    }
}

pub fn load_yaml<P: AsRef<Path>>(playbook: P) -> Result<Context, ExitCode> {
    let fname = playbook.as_ref();
    let contents = match read_contents(fname) {
        Ok(v) => v,
        Err(e) => {
            error!("IO Error (while loading the playbook {:?}): {}", playbook.as_ref(), e);
            return Err(ExitCode::ErrSys);
        }
    };
    match YamlLoader::load_from_str(&contents) {
        Ok(yml_global) => {
            Ok(Context::from(yml_global[0].to_owned()))
        },
        Err(e) => {
            error!("{}: {}", e, "Some YAML parsing error has occurred.");
            Err(ExitCode::ErrYML)
        }
    }
}
