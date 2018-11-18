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

fn run_step(num_step: usize, step: Context) {
    if let CtxObj::Str(action) = &step["action"] {
        let action: &str = action;
        println!("{} {}", action, step);
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

fn run_yaml<P: AsRef<Path>>(playbook: P, args: Context) -> Result<(), std::io::Error> {
    // TODO propagate relocated path into ctx
    let mut file = File::open(playbook)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    match YamlLoader::load_from_str(&contents) {
        Ok(config) => {
            let ref config = config[0];
            let raw = Context::from(config.to_owned());
            let ctx_global = raw.hide("steps").hide("docker");

            if inside_docker() {
                if let Some(i_step_str) = ctx_global.get("docker-step") {
                    if let CtxObj::Str(i_step_str) = i_step_str {
                        let i_step: usize = i_step_str.parse().expect("Cannot parse the `--docker-step` flag");
                        match raw.list_contexts("steps") {
                            Some(steps) => {
                                let ref ctx_step = steps[i_step];
                                run_step(i_step, ctx_global.overlay(ctx_step));
                            }
                            None => {
                                error!("Syntax Error: Key `steps` is not an array.");
                                std::process::exit(ERR_YML);
                            }
                        }
                    }
                    else { unreachable!(); }
                }
                else {
                    // .expect("Missing the `--docker-step` flag")
                }
                std::process::exit(SUCCESS);
            }
            else {
                match raw.list_contexts("steps") {
                    Some(steps) => {
                        for (i_step, ctx_step) in steps.iter().enumerate() {
                            run_step(i_step, ctx_global.overlay(ctx_step)); // TODO config overlay
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
            error!("{}", e);
            std::process::exit(ERR_YML);
        }
    }
    Ok(())
}

fn ctx_args_helper(opt: Option<&str>) -> Option<CtxObj> {
    if let Some(s) = opt { Some(CtxObj::Str(s.to_owned())) }
    else { None }
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

    let ctx_args = Context::new()
        .set_opt("docker-step", ctx_args_helper(args.value_of("DOCKER_STEP")))
        .set_opt("container-name", ctx_args_helper(args.value_of("CONTAINER_NAME")))
        .set_opt("relocate", ctx_args_helper(args.value_of("RELOCATE")))
        .set_opt("playbook", ctx_args_helper(args.value_of("PLAYBOOK")));

    let playbook = Path::new(args.value_of("PLAYBOOK").unwrap());
    if inside_docker() && playbook.is_absolute() {
        // Absolute path to the playbook must be self-mounted with relocation specified at cmdline,
        //   because we cannot read any content of the playbook without locating it first.
        if let Err(e) = run_yaml(Path::new(args.value_of("RELOCATE").expect("Missing the `--relocate` flag"))
            .join(playbook.file_name().unwrap()), ctx_args) {
                error!("{}: {}", e, playbook.display());
                std::process::exit(ERR_SYS);
            }
    }
    else {
         if let Err(e) = run_yaml(playbook, ctx_args) {
                error!("{}: {}", e, playbook.display());
                std::process::exit(ERR_SYS);
            }
    }
}
