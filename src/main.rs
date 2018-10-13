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
    let matches = clap_app!(myapp =>
        (version: "1.0")
        (author: "Alex Yang <aleozlx@gmail.com>")
        (about: "YAML driven Docker DevOps")
        (@arg DOCKER_STEP: --docker-step "For Docker use only: run a specific step with docker")
        (@arg RELOCATE: --relocate "Relocation of the playbook inside docker")
        (@arg PLAYBOOK: +required "YAML playbook")
    ).get_matches();
    setup_logger();

    if inside_docker() {
        let playbook = std::path::Path::new(matches.value_of("PLAYBOOK").unwrap());
        if playbook.is_absolute() {
            // Custom playbooks must be self-mounted with relocation specified at cmdline.
            //             assert args.relocate is not None
            //             PLAYBOOK = os.path.join(args.relocate, os.path.basename(PLAYBOOK))
        }
        else {
            // System provided playbooks are automatically mounted read-only
            //     PLAYBOOK = os.path.join('/opt/docker-playbook', PLAYBOOK)
        }
    }
}
