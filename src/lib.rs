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
    ErrTask,
    Any(i32)
}

impl Into<i32> for ExitCode {
    fn into(self) -> i32 {
        match self {
            ExitCode::Success => 0,
            ExitCode::ErrSys => 1,
            ExitCode::ErrApp => 2,
            ExitCode::ErrYML => 3,
            ExitCode::ErrTask => 4,
            ExitCode::Any(x) => x
        }
    }
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

enum TransientContext {
    Stateful(Context),
    Stateless(Context),
    Diverging(ExitCode)
}

fn assume_stateless(x: Result<Context, ExitCode>) -> TransientContext {
    match x {
        Ok(v) => TransientContext::Stateless(v),
        Err(e) => TransientContext::Diverging(e)
    }
}

type BuiltIn = fn(Context) -> TransientContext;
type TaskSpawner = fn(src: Context, ctx_step: Context) -> Result<(), TaskError>;

/// Exit
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_exit
/// ---
/// action: sys_exit
/// exit_code: 1
/// ```
fn sys_exit(ctx: Context) -> TransientContext {
    TransientContext::Diverging(ExitCode::Any(if let Ok(exit_code) = ctx.unpack("exit_code") { exit_code } else { 0 }))
}

/// Enter a shell (this must be in a container context)
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_shell
/// ---
/// action: sys_shell
/// bash: ['echo', 'hi']
/// ```
fn sys_shell(ctx: Context) -> TransientContext {
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
                    TransientContext::Diverging(ExitCode::Success)
                },
                Err(_) => {
                    error!("Docker crashed.");
                    TransientContext::Diverging(ExitCode::ErrYML)
                }
            }
        }
        else {
            warn!("{}", "Just a bash shell. Here goes nothing.".purple());
            match container::docker_start(ctx_docker.set("interactive", CtxObj::Bool(true)).hide("impersonate"), &["bash"]) {
                Ok(_) => {
                    TransientContext::Diverging(ExitCode::Success)
                },
                Err(_) => {
                    error!("Docker crashed.");
                    TransientContext::Diverging(ExitCode::ErrYML)
                }
            }
        }
    }
    else {
        error!("Docker context not found!");
        TransientContext::Diverging(ExitCode::ErrYML)
    }
}

fn sys_fork(ctx: Context) -> TransientContext {
    if let Some(rc) = ctx.subcontext("resource") {

    }
    else {

    }
    TransientContext::Diverging(ExitCode::ErrApp) // TODO
}

/// Dynamically import vars into the `ctx_states` context.
/// This is the only system action that introduces statefulness to the entire operation.
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_var
/// states:
///   from: pipe!
/// ---
/// action: sys_var
/// states:
///   from: another.yml
/// ---
/// action: sys_var
/// states:
///   from: postgresql://user:passwd@host/db
/// ```
fn sys_vars(ctx: Context) -> TransientContext {
    if let Some(CtxObj::Context(ctx_states)) = ctx.get("states") {
        if let Some(CtxObj::Str(url)) = ctx_states.get("from") {
            // * may support both file & database in the future
            let contents = match read_contents(url) {
                Ok(v) => v,
                Err(e) => {
                    error!("IO Error: {}", e);
                    return TransientContext::Diverging(ExitCode::ErrSys);
                }
            };
            return match YamlLoader::load_from_str(&contents) {
                Ok(yml_vars) => {
                    let ctx_pipe = Context::from(yml_vars[0].to_owned());
                    TransientContext::Stateful(ctx_pipe)
                }
                Err(_) => {
                    TransientContext::Diverging(ExitCode::ErrYML)
                }
            };
        }
    }
    TransientContext::Stateless(Context::new())
}

fn invoke(src: Context, ctx_step: Context) -> Result<Context, ExitCode> {
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

fn resolve_builtin<'step>(ctx_step: &'step Context) -> (Option<&'step str>, Option<BuiltIn>) {
    if let Some(CtxObj::Str(action)) = ctx_step.get("action") {
        let action: &'step str = action;
        match action {
            "sys_exit" => (Some(action), Some(sys_exit)),
            "sys_shell" => (Some(action), Some(sys_shell)),
            "sys_vars" => (Some(action), Some(sys_vars)),
            _ => (Some(action), None)
        }
    }
    else { (None, None) }
}

fn try_as_builtin(ctx_step: &Context) -> TransientContext {
    match resolve_builtin(&ctx_step) {
        (Some(action), Some(sys_func)) => {
            let ctx_sys = ctx_step.hide("whitelist").hide("i_step");
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

fn run_step(ctx_step: Context) -> TransientContext {
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
                if let Some(CtxObj::Str(_)) = ctx_step.get("arg-resume") {
                    show_step(true);
                    assume_stateless(invoke(ctx_source, ctx_step.hide("whitelist").hide("i_step")))
                }
                else {
                    if let Some(ctx_docker) = ctx_step.subcontext("docker") {
                        show_step(false);
                        if let Some(CtxObj::Str(image_name)) = ctx_docker.get("image") {
                            info!("Entering Docker: {}", image_name.purple());
                            let mut resume_params = vec! [
                                format!("--arg-resume={}", i_step),
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
                                Ok(_docker_cmd) => {
                                    assume_stateless(Ok(Context::new())) // TODO pass return value back as a context
                                },
                                Err(e) => {
                                    match e.src {
                                        TaskErrorSource::NixError(_) | TaskErrorSource::ExitCode(_) | TaskErrorSource::Signal(_) => {
                                            error!("{}: {}", "Container has crashed".red().bold(), e);
                                        },
                                        TaskErrorSource::Internal => ()
                                    }
                                    TransientContext::Diverging(ExitCode::ErrTask)
                                }
                            }
                        }
                        else {
                            error!("Syntax Error: Cannot parse the name of the image.");
                            TransientContext::Diverging(ExitCode::ErrYML)
                        }
                    }
                    else {
                        show_step(true);
                        assume_stateless(invoke(ctx_source, ctx_step.hide("whitelist").hide("i_step")))
                    }
                }
            },
            (Some(_action), None) => {
                try_as_builtin(&ctx_step)
            },
            (None, None) => {
                error!("Syntax Error: Key `action` must be a string.");
                TransientContext::Diverging(ExitCode::ErrYML)
            }
        }
    }
    else {
        try_as_builtin(&ctx_step)
    }    
}

fn deduce_context(ctx_step_raw: &Context, ctx_global: &Context, ctx_states: &Context, i_step: usize) -> Context {
    let ctx_partial = ctx_global.overlay(&ctx_step_raw).overlay(&ctx_states).set("i_step", CtxObj::Int(i_step as i64));
    debug!("ctx({}) =\n{}", "partial".dimmed(), ctx_partial);
    if let Some(CtxObj::Str(_)) = ctx_partial.get("arg-resume") {
        if let Some(ctx_docker) = ctx_partial.subcontext("docker").unwrap().subcontext("vars") {
            ctx_partial.overlay(&ctx_docker).hide("docker")
        }
        else { ctx_partial.hide("docker") }
    }
    else { ctx_partial }
}

fn read_playbook(yml_global: &Yaml) -> Result<(Vec<Context>, Context), ExitCode> {
    let raw = Context::from(yml_global.to_owned());
    let ctx_global = raw.hide("steps");
    if let Some(steps) = raw.list_contexts("steps") {
        Ok((steps, ctx_global))
    }
    else {
        Err(ExitCode::ErrYML)
    }
}

fn check_playbook_fname(fname: &Path) {
    if let Some(playbook_ext) = fname.extension() {
        if playbook_ext != "yml" && playbook_ext != "yaml" {
            warn!("{}", "The playbook file is not YAML based on its extension.".yellow());
        }
    }
}

pub fn run_yaml<P: AsRef<Path>>(playbook: P, ctx_args: Context) -> Result<(), ExitCode> {
    let mut ctx_states = Box::new(ctx_args.clone());
    let fname = playbook.as_ref();
    check_playbook_fname(fname);
    let contents = match read_contents(fname) {
        Ok(v) => v,
        Err(e) => {
            error!("IO Error: {}", e);
            return Err(ExitCode::ErrSys);
        }
    };
    match YamlLoader::load_from_str(&contents) {
        Ok(yml_global) => {
            let (steps, ctx_global) = match read_playbook(&yml_global[0]) {
                Ok(v) => v,
                Err(e) => {
                    error!("Syntax Error: Key `steps` is not an array.");
                    return Err(e);
                }
            };
            if let Some(CtxObj::Str(i_step_str)) = ctx_args.get("arg-resume") {
                // ^^ Then we must be in a docker container because main() has guaranteed that.
                if let Ok(i_step) = i_step_str.parse::<usize>() {
                    let ctx_step = deduce_context(&steps[i_step], &ctx_global, ctx_states.as_ref(), i_step);
                    match run_step(ctx_step) {
                        TransientContext::Stateful(_) | TransientContext::Stateless(_) => Ok(()),
                        TransientContext::Diverging(exit_code) => Err(exit_code)
                    }
                }
                else {
                    error!("Syntax Error: Cannot parse the `--arg-resume` flag.");
                    Err(ExitCode::ErrApp)
                }
            }
            else {
                for (i_step, ctx_step_raw) in steps.iter().enumerate() {
                    let ctx_step = deduce_context(ctx_step_raw, &ctx_global, ctx_states.as_ref(), i_step);
                    match run_step(ctx_step) {
                        TransientContext::Stateless(_) => { }
                        TransientContext::Stateful(ctx_pipe) => {
                            ctx_states = Box::new(ctx_states.overlay(&ctx_pipe));
                        }
                        TransientContext::Diverging(exit_code) => {
                            return Err(exit_code);
                        }
                    }
                }
                Ok(())
            }
        },
        Err(e) => {
            error!("{}: {}", e, "Some YAML parsing error has occurred.");
            Err(ExitCode::ErrYML)
        }
    }
}
