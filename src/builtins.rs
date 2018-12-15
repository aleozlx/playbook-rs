use super::container;
use std::path::Path;
use colored::*;
use yaml_rust::YamlLoader;
use ymlctx::context::{Context, CtxObj};

#[derive(Clone)]
pub enum ExitCode {
    Success,
    ErrSys,
    ErrApp,
    ErrYML,
    ErrTask,
    Any(i32)
}

impl std::fmt::Debug for ExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let exit_code: i32 = self.to_owned().into();
        write!(f, "{}", exit_code)
    }
}

impl Into<i32> for ExitCode {
    fn into(self) -> i32 {
        match self {
            ExitCode::Success => 0,
            ExitCode::ErrSys => 1,
            ExitCode::ErrApp => 2,
            ExitCode::ErrYML => 3,
            ExitCode::ErrTask => 4,
            ExitCode::Any(x) => x
        }
    }
}

pub enum TransientContext {
    Stateful(Context),
    Stateless(Context),
    Diverging(ExitCode)
}

impl TransientContext {
    pub fn assume_stateless(x: Result<Context, ExitCode>) -> TransientContext {
        match x {
            Ok(v) => TransientContext::Stateless(v),
            Err(e) => TransientContext::Diverging(e)
        }
    }
}

type BuiltIn = fn(Context) -> TransientContext;
pub fn resolve<'step>(ctx_step: &'step Context) -> (Option<&'step str>, Option<BuiltIn>) {
    if let Some(CtxObj::Str(action)) = ctx_step.get("action") {
        let action: &'step str = action;
        match action {
            "sys_exit" => (Some(action), Some(exit)),
            "sys_shell" => (Some(action), Some(shell)),
            "sys_vars" => (Some(action), Some(vars)),
            _ => (Some(action), None)
        }
    }
    else { (None, None) }
}

/// Exit
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_exit
/// ---
/// action: sys_exit
/// exit_code: 1
/// ```
fn exit(ctx: Context) -> TransientContext {
    TransientContext::Diverging(ExitCode::Any(if let Ok(exit_code) = ctx.unpack("exit_code") { exit_code } else { 0 }))
}

/// Enter a shell (this must be in a container context)
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_shell
/// ---
/// action: sys_shell
/// bash: ['echo', 'hi']
/// ```
fn shell(ctx: Context) -> TransientContext {
    if let Some(ctx_docker) = ctx.subcontext("docker") {
        if let Some(CtxObj::Array(bash_cmd)) = ctx.get("bash") {
            let cmd = super::format_cmd(bash_cmd.iter().map(|arg| {
                match arg {
                    CtxObj::Str(s) => s.to_owned(),
                    _ => String::from("")
                }
            }));
            match container::docker_start(ctx_docker.hide("impersonate"), &["bash", "-c", &cmd]) {
                // Note: it is not secure to transition from the playbook to a shell, so "dynamic" impersonate is not an option
                Ok(_) => {
                    TransientContext::Diverging(ExitCode::Success)
                },
                Err(_) => {
                    error!("Docker crashed.");
                    TransientContext::Diverging(ExitCode::ErrYML)
                }
            }
        }
        else {
            warn!("{}", "Just a bash shell. Here goes nothing.".purple());
            match container::docker_start(ctx_docker.set("interactive", CtxObj::Bool(true)).hide("impersonate"), &["bash"]) {
                Ok(_) => {
                    TransientContext::Diverging(ExitCode::Success)
                },
                Err(_) => {
                    error!("Docker crashed.");
                    TransientContext::Diverging(ExitCode::ErrYML)
                }
            }
        }
    }
    else {
        error!("Docker context not found!");
        TransientContext::Diverging(ExitCode::ErrYML)
    }
}

fn fork(ctx: Context) -> TransientContext {
    if let Some(rc) = ctx.subcontext("resource") {

    }
    else {

    }
    TransientContext::Diverging(ExitCode::ErrApp) // TODO
}

/// Dynamically import vars into the `ctx_states` context.
/// This is the only system action that introduces statefulness to the entire operation.
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_var
/// states:
///   from: pipe!
/// ---
/// action: sys_var
/// states:
///   from: another.yml
/// ---
/// action: sys_var
/// states:
///   from: postgresql://user:passwd@host/db
/// ```
fn vars(ctx: Context) -> TransientContext {
    if let Some(CtxObj::Context(ctx_states)) = ctx.get("states") {
        if let Some(CtxObj::Str(url)) = ctx_states.get("from") {
            // * may support both file & database in the future
            let ref playbook: String = ctx.unpack("playbook").unwrap();
            let playbook_dir;
            if let Some(parent) = Path::new(playbook).parent() {
                playbook_dir = parent;
            }
            else {
                playbook_dir = Path::new(".");
            }
            let ref src_path = playbook_dir.join(url);

            let contents = match super::read_contents(src_path) {
                Ok(v) => v,
                Err(e) => {
                    error!("IO Error: {}", e);
                    return TransientContext::Diverging(ExitCode::ErrSys);
                }
            };
            return match YamlLoader::load_from_str(&contents) {
                Ok(yml_vars) => {
                    let ctx_pipe = Context::from(yml_vars[0].to_owned());
                    TransientContext::Stateful(ctx_pipe)
                }
                Err(_) => {
                    TransientContext::Diverging(ExitCode::ErrYML)
                }
            };
        }
    }
    TransientContext::Stateless(Context::new())
}
