#![feature(proc_macro, specialization)]

#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate fern;
extern crate chrono;
extern crate yaml_rust;
extern crate linked_hash_map;
extern crate colored;
extern crate pyo3;

use std::str;
use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::collections::{HashSet, BTreeMap};
use std::result::Result;
use yaml_rust::{Yaml, YamlLoader};
use colored::*;

// #[macro_use]
// extern crate serde_derive;
// extern crate serde_yaml;

mod context;
use context::{Context, CtxObj};

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

/** 
 * Creates a whitelist that is based on enumeration of files and symlinks with x permission.
*/
fn white_list() -> HashSet<String> {
    // let stdout = std::process::Command::new("find").args(&[".", "-perm", "/111", "-type", "f", "-o", "-type", "l"])
    //     .output().expect("I/O error").stdout;
    // let output = str::from_utf8(&stdout).unwrap();
    // output.lines().map(|i| { i.to_owned() }).collect()
    ["hi"].iter().map(|&i| {String::from(i)}).collect()
}

type BuiltIn = fn(&Yaml) -> !;

fn sys_exit(ctx: &Yaml) -> ! {
    std::process::exit(0);
}

fn sys_shell(ctx: &Yaml) -> ! {
    unimplemented!()
}

fn run_step(num_step: usize, step: &Context, whitelist: &HashSet<String>) {
    if let CtxObj::Str(action) = &step["action"] {
        let action: &str = action;
        if action.starts_with("step_") {
            warn!("Action name should not be prefixed by \"step_\": {}", action);
        }
        if whitelist.contains(action) {
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
        }
        else{
            let mut whitelist_sys: BTreeMap<&str, BuiltIn> = BTreeMap::new();
            whitelist_sys.insert("sys_exit", sys_exit);
            whitelist_sys.insert("sys_shell", sys_shell);
            if whitelist_sys.contains_key(action) {
                info!("{}: {}", "Built-in".red().bold(), action);
                // TODO context deduction https://doc.rust-lang.org/std/iter/trait.Extend.html
                // whitelist_sys[action](step);
            }
            else {
                warn!("Action not recognized: {}", action);
            }
        }
    }
    else {
        error!("Syntax Error: Key `action` is not a string.");
        std::process::exit(1);
    }
}

fn run_yaml<P: AsRef<Path>>(playbook: P, num_step: Option<usize>) -> Result<(), std::io::Error> {
    // TODO propagate relocated path into ctx
    let mut file = File::open(playbook)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    match YamlLoader::load_from_str(&contents) {
        Ok(config) => {
            // let ref config = config[0];
            // let global_context = Context::from(config);
            // let ref whitelist = white_list();
            // if inside_docker() {
            //     let num_step = num_step.unwrap();
            //     // let ref step = config["steps"][num_step];
            //     let step_context = Context::from(&config["steps"][num_step]);
            //     // run_step(num_step, step, whitelist);
            //     std::process::exit(0);
            // }
            // else {
            //     if let Yaml::Array(steps) = &config["steps"] {
            //         for (i_step, step) in steps.iter().enumerate() {
            //             // run_step(i_step, step, whitelist);
            //         }
            //     }
            //     else {
            //         error!("Syntax Error: Key `steps` is not an array.");
            //         std::process::exit(1);
            //     }
            // }
        },
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}

extern crate rpds;

fn main() {
    let matches = clap_app!(playbook =>
        (version: crate_version!())
        (author: crate_authors!())
        (about: crate_description!())
        (@arg DOCKER_STEP: --("docker-step") "For Docker use ONLY: run a specific step with docker")
        (@arg RELOCATE: --relocate "Relocation of the playbook inside docker, required when using abs. path")
        (@arg PLAYBOOK: +required "YAML playbook")
    ).get_matches();
    setup_logger().unwrap();

    let playbook = Path::new(matches.value_of("PLAYBOOK").unwrap());
    let ret = if inside_docker() {
        let num_step: usize = matches.value_of("DOCKER_STEP")
            .expect("Missing the `--docker-step` flag").parse()
            .expect("Cannot parse the `--docker-step` flag");
        if playbook.is_absolute() {
            // Absolute path to the playbook must be self-mounted with relocation specified at cmdline,
            //   because we cannot read any content of the playbook without locating it first.
            run_yaml(Path::new(matches.value_of("RELOCATE").expect("Missing the `--relocate` flag"))
                .join(playbook.file_name().unwrap()), Some(num_step))
        }
        else {
            run_yaml(playbook, Some(num_step))
        }
    }
    else {
        run_yaml(playbook, None)
    };
    if let Err(e) = ret {
        error!("{}: {}", e, playbook.display());
        std::process::exit(2);
    }
}
