pub mod docker;

#[cfg(feature = "sys_hotwings")]
pub mod hotwings;

use ymlctx::context::Context;
use crate::TaskError;

pub trait Infrastructure {
    fn start<I>(&self, Context, I) -> Result<String, TaskError>
      where
        I: IntoIterator,
        I::Item: AsRef<std::ffi::OsStr>;
}

pub fn abstract_infrastructures(name: &str) -> Option<impl Infrastructure> {
    match name {
        #[cfg(feature = "sys_hotwings")]
        "hotwings" => Some(SupportedInfrastructure::Hotwings(hotwings::Hotwings {})),
        "docker" => Some(SupportedInfrastructure::Docker(docker::Docker {})),
        _ => None
    }
}

pub enum SupportedInfrastructure {
    Docker(docker::Docker),
    #[cfg(feature = "sys_hotwings")]
    Hotwings(hotwings::Hotwings)
}

impl Infrastructure for SupportedInfrastructure {
    fn start<I>(&self, ctx_docker: Context, cmd: I) -> Result<String, TaskError>
      where I: IntoIterator, I::Item: AsRef<std::ffi::OsStr>
    {
        match self {
            SupportedInfrastructure::Docker(i) => i.start(ctx_docker, cmd),
            #[cfg(feature = "sys_hotwings")]
            SupportedInfrastructure::Hotwings(i) => i.start(ctx_docker, cmd)
        }
    }
}
