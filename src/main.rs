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

}
