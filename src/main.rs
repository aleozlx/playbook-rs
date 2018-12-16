#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;
extern crate dirs;
extern crate fern;
extern crate chrono;

extern crate playbook_api;
use std::path::Path;
use playbook_api::{Context, CtxObj};
use playbook_api::builtins::ExitCode;

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

macro_rules! map_arg {
    ($args:ident => $name:expr) => {
        if let Some(s) = $args.value_of(stringify!{$name}) { Some(CtxObj::Str(s.to_owned())) }
        else { None }
    }
}

fn main() {
    #[cfg(feature = "agent")]
    let args = clap_app!(playbook =>
            (version: crate_version!())
            (author: crate_authors!())
            (about: crate_description!())
            (@arg RESUME: --("arg-resume") +takes_value "For playbook-rs use ONLY: indicator that we have entered a container")
            (@arg ASSERT_VER: --("arg-version") +takes_value "For playbook-rs use ONLY: to ensure the binary versions match")
            (@arg RELOCATE: --relocate +takes_value "Relocation of the playbook inside docker, required when using abs. path")
            (@arg VERBOSE: --verbose -v ... "Log verbosity")
            (@arg PLAYBOOK: +required "YAML playbook")
        ).get_matches();
    #[cfg(not(feature = "agent"))]
    let args = clap_app!(playbook =>
            (version: crate_version!())
            (author: crate_authors!())
            (about: crate_description!())
            (@arg RELOCATE: --relocate +takes_value "Relocation of the playbook inside docker, required when using abs. path")
            (@arg VERBOSE: --verbose -v ... "Log verbosity")
            (@arg PLAYBOOK: +required "YAML playbook")
        ).get_matches();
    setup_logger(args.occurrences_of("VERBOSE")).expect("Logger Error.");
    if let Some(ver) = args.value_of("ASSERT_VER") {
        if ver != crate_version!() {
            warn!("The playbook binary versions do not match: host => {} vs container => {}", &ver, &crate_version!());
        }
    }
    let ctx_args = Context::new()
        .set_opt("arg-resume", map_arg!(args => RESUME))
        .set_opt("relocate", map_arg!(args => RELOCATE))
        .set_opt("playbook", map_arg!(args => PLAYBOOK))
        .set_opt("verbose-fern", match args.occurrences_of("VERBOSE") {
            0 => None,
            v => Some(CtxObj::Int(v as i64))
        });
    println!("RESUME>>>{}<<<", ctx_args);
    let mut playbook = Path::new(args.value_of("PLAYBOOK").unwrap()).to_path_buf();
    if let Some(_) = ctx_args.get("arg-resume") {
        if !playbook_api::container::inside_docker() {
            error!("Context error: Not inside of a Docker container.");
            exit(ExitCode::ErrApp);
        }
        // * Related issue: https://github.com/aleozlx/playbook-rs/issues/6
        if let Some(relocate) = args.value_of("RELOCATE") {
            playbook = Path::new(relocate).join(playbook.file_name().unwrap());
        }

        if let Ok(ref become_id) = std::env::var("IMPERSONATE") {
            match impersonate::User::from_id(become_id).unwrap().su() {
                Ok(()) => (),
                Err(e) => {
                    error!("{}", e);
                    exit(ExitCode::ErrSys);
                }
            }
        }
    }
    match playbook_api::load_yaml(playbook) {
        Ok(raw) => match playbook_api::run_playbook(raw, ctx_args) {
            Ok(()) => { },
            Err(e) => exit(e)
        },
        Err(e) => exit(e)
    };

    // if let Err(e) = playbook_api::run_yaml(&playbook, ctx_args) {
    //     exit(e);
    // }
}

fn clean_up() {

}

fn exit(code: ExitCode) -> ! {
    clean_up();
    std::process::exit(code.into());
}
