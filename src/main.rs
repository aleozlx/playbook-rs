#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate fern;
extern crate chrono;

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, _record| {
            out.finish(format_args!(
                "{} {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stderr())
        // .chain(fern::log_file("output.log")?)
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

    let playbook = std::path::Path::new(matches.value_of("PLAYBOOK").unwrap());
    let playbook = if inside_docker() && playbook.is_absolute() {
            // Absolute path to the playbook must be self-mounted with relocation specified at cmdline,
            //   because we cannot read any content of the playbook without locating it first.
            std::path::Path::new(matches.value_of("RELOCATE").expect("Missing a `--relocate` flag"))
                .join(std::path::Path::new(matches.value_of("PLAYBOOK").unwrap()).file_name().unwrap())
    }
    else {
        playbook.to_path_buf()
    };

}
