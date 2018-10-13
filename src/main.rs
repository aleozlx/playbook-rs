#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate fern;
extern crate chrono;
extern crate yaml_rust;

use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::result::Result;
use yaml_rust::{Yaml, YamlLoader};

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

fn run_step(num_step: usize, step: &Yaml) {

}

fn run_yaml<P: AsRef<Path>>(playbook: P, num_step: Option<usize>) -> Result<(), std::io::Error> {
    // TODO propagate relocated path into ctx
    let mut file = File::open(playbook)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    match YamlLoader::load_from_str(&contents) {
        Ok(config) => {
            let ref config = config[0];
            if inside_docker() {
                let num_step = num_step.unwrap();
                let ref step = config["steps"][num_step];
                run_step(num_step, step);
                std::process::exit(0);
            }
            else {
                if let Yaml::Array(steps) = &config["steps"] {
                    for (i_step, step) in steps.iter().enumerate() {
                        run_step(i_step, step);
                    }
                }
                else {
                    error!("Syntax Error: Key `steps` is not an array,");
                    std::process::exit(1);
                }
            }
        },
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}

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
