#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate fern;
extern crate chrono;
extern crate yaml_rust;
extern crate ymlctx;
extern crate colored;
extern crate pyo3;

use std::str;
use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::result::Result;
use yaml_rust::{Yaml, YamlLoader};
use colored::*;

use ymlctx::context::{Context, CtxObj};

const SUCCESS: i32 = 0;
const ERR_SYS: i32 = 1;
const ERR_APP: i32 = 2;
const ERR_YML: i32 = 3;
const ERR_JOB: i32 = 4;

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} {} {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
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

// /** 
//  * Creates a whitelist that is based on enumeration of files and symlinks with x permission.
// */
// fn white_list() -> HashSet<String> {
//     // let stdout = std::process::Command::new("find").args(&[".", "-perm", "/111", "-type", "f", "-o", "-type", "l"])
//     //     .output().expect("I/O error").stdout;
//     // let output = str::from_utf8(&stdout).unwrap();
//     // output.lines().map(|i| { i.to_owned() }).collect()
//     ["hi"].iter().map(|&i| {String::from(i)}).collect()
// }

type BuiltIn = fn(&Yaml) -> !;

fn sys_exit(ctx: &Yaml) -> ! {
    std::process::exit(0);
}

fn sys_shell(ctx: &Yaml) -> ! {
    unimplemented!()
}

fn run_step(num_step: usize, ctx_step: Context) {
    if let Some(CtxObj::Str(action)) = &ctx_step.get("action") {
        let action: &str = action;
        println!("{} {}", action, ctx_step);
        if action.starts_with("step_") {
            warn!("Action name should not be prefixed by \"step_\": {}", action);
        }
        // if whitelist.contains(action) {
            if !inside_docker() {
                // info!("Step {}: {}",
                //     (num_step+1).to_string().green().bold(),
                //     step["name"].as_str().unwrap());
            }
            else {
                // info!("Step {}: {}",
                //     (num_step+1).to_string().green(),
                //     step["name"].as_str().unwrap());
            }
        // }
        // else{
        //     // let whitelist_sys = get_whitelist_sys();
        //     // if whitelist_sys.contains_key(action) {
        //     //     info!("{}: {}", "Built-in".red().bold(), action);
        //     //     // TODO context deduction https://doc.rust-lang.org/std/iter/trait.Extend.html
        //     //     let run = whitelist_sys[action];
        //     //     let mut ctx = Yaml::from_str("");
        //     //     ctx.
        //     //     run(&ctx);
        //     // }
        //     // else {
        //     //     warn!("Action not recognized: {}", action);
        //     // }
        // }
    }
    else {
        error!("Syntax Error: Key `action` is not a string.");
        std::process::exit(1);
    }
}

fn run_yaml<P: AsRef<Path>>(playbook: P, ctx_args: Context) -> Result<(), std::io::Error> {
    let mut file = File::open(playbook)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    match YamlLoader::load_from_str(&contents) {
        Ok(yml_global) => {
            let ref yml_global = yml_global[0];
            let raw = Context::from(yml_global.to_owned());
            let ctx_global = raw.hide("steps").hide("docker");

            if let Some(str_step) = ctx_args.get("docker-step") {
                if !inside_docker() {
                    error!("Context error: Not inside of a Docker container.");
                    std::process::exit(ERR_APP);
                }
                if let CtxObj::Str(i_step_str) = str_step {
                    if let Ok(i_step) = i_step_str.parse::<usize>() {
                        match raw.list_contexts("steps") {
                            Some(steps) => {
                                let ctx_step = &steps[i_step];
                                let ctx_partial = ctx_global.overlay(&ctx_step).overlay(&ctx_args);
                                run_step(i_step, if let Some(ctx_docker) = ctx_partial.subcontext("docker").unwrap().subcontext("docker_overrides") {
                                    ctx_partial.overlay(&ctx_docker).hide("docker")
                                }
                                else {
                                    ctx_partial.hide("docker")
                                });
                            }
                            None => {
                                error!("Syntax Error: Key `steps` is not an array.");
                                std::process::exit(ERR_YML);
                            }
                        }
                    }
                    else {
                        error!("Syntax Error: Cannot parse the `--docker-step` flag.");
                        std::process::exit(ERR_APP);
                    }
                }
                else { unreachable!(); }
                std::process::exit(SUCCESS);
            }
            else {
                match raw.list_contexts("steps") {
                    Some(steps) => {
                        for (i_step, ctx_step) in steps.iter().enumerate() {
                            run_step(i_step, ctx_global.overlay(&ctx_step).overlay(&ctx_args));
                        }
                    }
                    None => {
                        error!("Syntax Error: Key `steps` is not an array.");
                        std::process::exit(ERR_YML);
                    }
                }
            }
        },
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
        (@arg DOCKER_STEP: --("docker-step") "For Docker use ONLY: run a specific step with docker")
        (@arg RELOCATE: --relocate "Relocation of the playbook inside docker, required when using abs. path")
        (@arg PLAYBOOK: +required "YAML playbook")
    ).get_matches();
    setup_logger().unwrap();

    fn _helper(opt: Option<&str>) -> Option<CtxObj> {
        if let Some(s) = opt { Some(CtxObj::Str(s.to_owned())) }
        else { None }
    }
    let ctx_args = Context::new()
        .set_opt("docker-step", _helper(args.value_of("DOCKER_STEP")))
        .set_opt("container-name", _helper(args.value_of("CONTAINER_NAME")))
        .set_opt("relocate", _helper(args.value_of("RELOCATE")))
        .set_opt("playbook", _helper(args.value_of("PLAYBOOK")));

    let mut playbook = Path::new(args.value_of("PLAYBOOK").unwrap()).to_path_buf();
    if let Some(_) = ctx_args.get("docker-step") {
        if !inside_docker() {
            error!("Context error: Not inside of a Docker container.");
            std::process::exit(ERR_APP);
        }

        // Especially, absolute path to the playbook must be self-mounted with relocation specified at cmdline,
        //   because we cannot read any content of the playbook without locating it first.
        playbook = Path::new(args.value_of("RELOCATE").expect("Missing the `--relocate` flag")).join(playbook.file_name().unwrap());
    }

    match run_yaml(&playbook, ctx_args) {
        Ok(()) => (),
        Err(e) => {
            error!("{}: {}", e, playbook.display());
            std::process::exit(ERR_SYS);
        }
    }
}
