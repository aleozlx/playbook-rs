#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate fern;
extern crate chrono;
extern crate yaml_rust;

use std::path::Path;
use std::fs::File;
use std::io::prelude::*;
use std::result::Result;
use std::collections::BTreeMap;
use yaml_rust::YamlLoader;

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

fn run_yaml<P: AsRef<Path>>(playbook: P) -> Result<(), std::io::Error> {
    // TODO propagate relocated path into ctx
    let mut file = File::open(playbook)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config = YamlLoader::load_from_str(&contents);
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

    if let Err(e) = setup_logger() {
        panic!("ERROR {}", e);
    }

    let playbook = Path::new(matches.value_of("PLAYBOOK").unwrap());
    let ret = if inside_docker() && playbook.is_absolute() {
        // Absolute path to the playbook must be self-mounted with relocation specified at cmdline,
        //   because we cannot read any content of the playbook without locating it first.
        run_yaml(Path::new(matches.value_of("RELOCATE").expect("Missing a `--relocate` flag"))
            .join(playbook.file_name().unwrap()))
    }
    else {
        run_yaml(playbook)
    };
    if let Err(e) = ret {
        error!("{}: {}", e, playbook.display());
        std::process::exit(2);
    }
}
