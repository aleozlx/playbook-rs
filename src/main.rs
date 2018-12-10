#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;

extern crate fern;
extern crate chrono;

extern crate playbook_api;
use std::path::Path;
use playbook_api::{Context, CtxObj};

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
        .set_opt("relocate", _helper(args.value_of("RELOCATE")))
        .set_opt("playbook", _helper(args.value_of("PLAYBOOK")))
        .set_opt("verbose-fern", match args.occurrences_of("VERBOSE") {
            0 => None,
            v => Some(CtxObj::Int(v as i64))
        });
    let mut playbook = Path::new(args.value_of("PLAYBOOK").unwrap()).to_path_buf();
    if let Some(_) = ctx_args.get("docker-step") {
        if !playbook_api::inside_docker() {
            error!("Context error: Not inside of a Docker container.");
            std::process::exit(playbook_api::ERR_APP);
        }
        // * Related issue: https://github.com/aleozlx/playbook-rs/issues/6
        if let Some(relocate) = args.value_of("RELOCATE") {
            playbook = Path::new(relocate).join(playbook.file_name().unwrap());
        }

        if let Ok(ref become_id) = std::env::var("TKSTACK_USER") {
            match impersonate::User::from_id(become_id).unwrap().su() {
                Ok(()) => (),
                Err(e) => {
                    error!("{}", e);
                    std::process::exit(playbook_api::ERR_SYS);
                }
            }
        }
    }
    match playbook_api::run_yaml(&playbook, ctx_args) {
        Ok(()) => (),
        Err(e) => {
            error!("{}: {}", e, playbook.display());
            std::process::exit(playbook_api::ERR_SYS);
        }
    }
}

// extern "C" {
//     fn signal(sig: u32, cb: extern fn(u32)) -> extern fn(u32);
// }

// extern fn just_ignore(_: u32) { }
