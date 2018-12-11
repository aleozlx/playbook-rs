#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;
extern crate dirs;
extern crate fern;
extern crate chrono;

extern crate playbook_api;
use std::path::Path;
use playbook_api::{Context, CtxObj, ExitCode};

fn setup_logger(verbose: u64) -> Result<(), fern::InitError> {
    let ref log_dir = dirs::home_dir().expect("Cannot determine the HOME directory.").join(".playbook-rs");
    if !Path::new(log_dir).exists() { std::fs::create_dir(log_dir)?; }
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} {} {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            3 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Trace
        })
        .chain(fern::log_file(log_dir.join("playbook.log"))?)
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
        (@arg VERBOSE: --verbose -v ... "Log verbosity")
        (@arg PLAYBOOK: +required "YAML playbook")
    ).get_matches();
    setup_logger(args.occurrences_of("VERBOSE")).expect("Logger Error.");
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
        if !playbook_api::container::inside_docker() {
            error!("Context error: Not inside of a Docker container.");
            playbook_api::exit(ExitCode::ErrApp);
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
                    playbook_api::exit(ExitCode::ErrSys);
                }
            }
        }
    }
    match playbook_api::run_yaml(&playbook, ctx_args) {
        Ok(()) => (),
        Err(e) => {
            error!("{}: {}", e, playbook.display());
            playbook_api::exit(ExitCode::ErrSys);
        }
    }
}



// extern "C" {
//     fn signal(sig: u32, cb: extern fn(u32)) -> extern fn(u32);
// }

// extern fn just_ignore(_: u32) { }
