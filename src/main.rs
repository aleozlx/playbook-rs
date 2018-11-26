#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate fern;
extern crate chrono;
extern crate yaml_rust;
extern crate ymlctx;
extern crate colored;
extern crate regex;
extern crate nix;

#[cfg(feature = "spawner_python")]
extern crate pyo3;

use std::str;
use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::result::Result;
use std::collections::HashSet;
use yaml_rust::{Yaml, YamlLoader};
use colored::*;
use regex::Regex;
use ymlctx::context::{Context, CtxObj};

const SUCCESS: i32 = 0;
const ERR_SYS: i32 = 1;
const ERR_APP: i32 = 2;
const ERR_YML: i32 = 3;
const ERR_JOB: i32 = 4;

fn setup_logger(verbose: bool) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} {} {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(if verbose {log::LevelFilter::Debug} else {log::LevelFilter::Info})
        .chain(std::io::stderr())
        .apply()?;
    Ok(())
}

fn inside_docker() -> bool {
    let status = std::process::Command::new("grep").args(&["-q", "docker", "/proc/1/cgroup"])
        .status().expect("I/O error");
    match status.code() {
        Some(code) => code==0,
        None => unreachable!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobError {}

type BuiltIn = fn(Context) -> !;
type JobSpawner = fn(src: Context, ctx_step: Context) -> Result<(), JobError>;

fn sys_exit(ctx: Context) -> ! {
    std::process::exit(if let Ok(exit_code) = ctx.unpack("exit_code") { exit_code } else { 0 });
}

fn sys_shell(ctx: Context) -> ! {
    if let Some(ctx_docker) = ctx.subcontext("docker") {
        if let Some(CtxObj::Array(bash_cmd)) = ctx.get("bash") {
            let cmd = spawner::format_cmd(bash_cmd.iter().map(|arg| {
                match arg {
                    CtxObj::Str(s) => s.to_owned(),
                    _ => String::from("")
                }
            }));
            match spawner::docker_start(ctx_docker, &["bash", "-c", &cmd]) {
                Ok(()) => {
                    std::process::exit(SUCCESS);
                },
                Err(_) => {
                    error!("Docker crashed.");
                    std::process::exit(ERR_YML);
                }
            }
        }
        else {
            warn!("{}", "Just a bash shell. Here goes nothing.".purple());
            match spawner::docker_start(ctx_docker.set("interactive", CtxObj::Bool(true)), &["bash"]) {
                Ok(()) => {
                    std::process::exit(SUCCESS);
                },
                Err(_) => {
                    error!("Docker crashed.");
                    std::process::exit(ERR_YML);
                }
            }
        }
    }
    else {
        error!("Docker context not found!");
        std::process::exit(ERR_YML);
    }
}

mod spawner;
fn invoke(src: Context, ctx_step: Context) {
    let ref action: String = ctx_step.unpack("action").unwrap();
    let ref src_path_str: String = src.unpack("src").unwrap();
    eprintln!("{}", "== Context ======================".cyan());
    eprintln!("# ctx({}@{}) =\n{}", action.cyan(), src_path_str.dimmed(), ctx_step);
    eprintln!("{}", "== EOF ==========================".cyan());
    let src_path = Path::new(src_path_str);
    if let Some(ext_os) = src_path.extension() {
        let ext = ext_os.to_str().unwrap();
        #[allow(unused_variables)]
        let wrapper = |whichever: JobSpawner| {
            println!("{}", "== Output =======================".blue());
            if let Err(_) = whichever(src, ctx_step) {
                error!("Crash: A task internal error has occurred.");
                std::process::exit(ERR_JOB);
            }
            println!("{}", "== EOF ==========================".blue());
        };
        match ext {
            #[cfg(feature = "spawner_python")]
            "py" => wrapper(spawner::invoke_py),
            _ => warn!("It is not clear how to run {}.", src_path_str)
        }
    }
    else {
        // Possibly a binary?
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
        // if action.starts_with("step_") {
        //     warn!("Action name should not be prefixed by \"step_\": {}", action.cyan());
        // }
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
                                String::from("/usr/bin/env"),
                                String::from("playbook"),
                                format!("--docker-step={}", i_step),
                                ctx_step.unpack("playbook").unwrap()
                            ];
                            let relocate_unpack = ctx_step.unpack::<String>("relocate");
                            if let Ok(relocate) = relocate_unpack {
                                resume_params.push(relocate);
                            }
                            let verbose_unpack = ctx_step.unpack::<i64>("verbose-fern");
                            if let Ok(verbose) = verbose_unpack {
                                if verbose > 0 {
                                    resume_params.push(String::from("-v"));
                                }
                            }
                            match spawner::docker_start(ctx_docker.clone(), resume_params) {
                                Ok(()) => {},
                                Err(_) => {
                                    error!("{}", "Container has crashed".red().bold());
                                    std::process::exit(ERR_JOB);
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
                        eprintln!("{}", "== Context ======================".cyan());
                        eprintln!("# ctx({}) =\n{}", action.cyan(), ctx_sys);
                        eprintln!("{}", "== EOF ==========================".cyan());
                        sys_func(ctx_sys);
                    },
                    (Some(_), None) => {
                        error!("Action not recognized: {}", action);
                        std::process::exit(ERR_YML);
                    },
                    (None, None) => unreachable!()
                }
            },
            (None, None) => {
                error!("Syntax Error: Key `action` must be a string.");
                std::process::exit(ERR_YML);
            }
        }
    }
    else {
        match resolve_builtin(&ctx_step) {
            (Some(action), Some(sys_func)) => {
                let ctx_sys = ctx_step.hide("whitelist").hide("i_step");
                info!("{}: {}", "Built-in".magenta(), action);
                eprintln!("{}", "== Context ======================".cyan());
                eprintln!("# ctx({}) =\n{}", action.cyan(), ctx_sys);
                eprintln!("{}", "== EOF ==========================".cyan());
                sys_func(ctx_sys);
            },
            (Some(action), None) => {
                error!("Action not recognized: {}", action);
                std::process::exit(ERR_YML);
            },
            (None, _) => {
                error!("Syntax Error: Key `whitelist` should be a list of mappings.");
                std::process::exit(ERR_YML);
            }
        }
    }    
}

fn run_yaml<P: AsRef<Path>>(playbook: P, ctx_args: Context) -> Result<(), std::io::Error> {
    let enter_partial = |ctx_partial: Context| {
        debug!("ctx({}) =\n{}", "partial".dimmed(), ctx_partial);
        if let Some(CtxObj::Str(_)) = ctx_partial.get("docker-step") {
            run_step(
                if let Some(ctx_docker) = ctx_partial.subcontext("docker").unwrap().subcontext("docker_overrides") {
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
                std::process::exit(ERR_APP);
            }
            std::process::exit(SUCCESS);
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
            std::process::exit(ERR_YML);
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
            std::process::exit(ERR_YML);
        }
    }
    Ok(())
}

fn main() {
    let args = clap_app!(playbook =>
        (version: crate_version!())
        (author: crate_authors!())
        (about: crate_description!())
        (@arg DOCKER_STEP: --("docker-step") +takes_value "For playbook-rs use ONLY: indicator that we have entered a container")
        (@arg RELOCATE: --relocate +takes_value "Relocation of the playbook inside docker, required when using abs. path")
        (@arg VERBOSE: --verbose -v "Debug log")
        (@arg PLAYBOOK: +required "YAML playbook")
    ).get_matches();
    match args.occurrences_of("VERBOSE") {
        0 => setup_logger(false),
        _ => setup_logger(true)
    }.unwrap();
    fn _helper(opt: Option<&str>) -> Option<CtxObj> {
        if let Some(s) = opt { Some(CtxObj::Str(s.to_owned())) }
        else { None }
    }
    let ctx_args = Context::new()
        .set_opt("docker-step", _helper(args.value_of("DOCKER_STEP")))
        // .set_opt("container-name", _helper(args.value_of("CONTAINER_NAME")))
        .set_opt("relocate", _helper(args.value_of("RELOCATE")))
        .set_opt("playbook", _helper(args.value_of("PLAYBOOK")))
        .set_opt("verbose-fern", match args.occurrences_of("VERBOSE") {
            0 => None,
            v => Some(CtxObj::Int(v as i64))
        });
    let mut playbook = Path::new(args.value_of("PLAYBOOK").unwrap()).to_path_buf();
    if let Some(_) = ctx_args.get("docker-step") {
        if !inside_docker() {
            error!("Context error: Not inside of a Docker container.");
            std::process::exit(ERR_APP);
        }
        // Especially, absolute path to the playbook must be self-mounted with relocation specified at cmdline,
        //   because we cannot read any content of the playbook without locating it first.
        if let Some(relocate) = args.value_of("RELOCATE") {
            playbook = Path::new(relocate).join(playbook.file_name().unwrap());
        }
    }
    match run_yaml(&playbook, ctx_args) {
        Ok(()) => (),
        Err(e) => {
            error!("{}: {}", e, playbook.display());
            std::process::exit(ERR_SYS);
        }
    }
}
