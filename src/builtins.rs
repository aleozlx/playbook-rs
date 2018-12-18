use super::container;
use std::path::Path;
use colored::*;
use yaml_rust::YamlLoader;
use ymlctx::context::{Context, CtxObj};
use itertools::Itertools;

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

/// A context labeled as either stateful or stateless,
/// or diverging when neither is applicable, in which case the program must provide an exit code and exit gracefully.
/// 
/// This data structure is only used in the decision making in between steps, therefore it is transient.
/// 
/// A stateful context may affect all following steps by appearing in the ctx_states and arg-resume.
/// On the other hand, a stateless context will still be collected by the playbook for any reason it may need it,
/// then discarded before the next step begins.
pub enum TransientContext {
    Stateful(Context),
    Stateless(Context),
    Diverging(ExitCode)
}

impl From<Result<Context, ExitCode>> for TransientContext {
    fn from(x: Result<Context, ExitCode>) -> Self {
        match x {
            Ok(v) => TransientContext::Stateless(v),
            Err(e) => TransientContext::Diverging(e)
        }
    }
}

type BuiltIn = fn(Context) -> TransientContext;

/// The built-in tasks resolver
pub fn resolve<'step>(ctx_step: &'step Context) -> (Option<&'step str>, Option<BuiltIn>) {
    if let Some(CtxObj::Str(action)) = ctx_step.get("action") {
        let action: &'step str = action;
        match action {
            "sys_exit" => (Some(action), Some(exit)),
            "sys_shell" => (Some(action), Some(shell)),
            "sys_vars" => (Some(action), Some(vars)),
            "sys_fork" => (Some(action), Some(fork)),
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
pub fn exit(ctx: Context) -> TransientContext {
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
pub fn shell(ctx: Context) -> TransientContext {
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

fn single_key(ctx: &Context) -> Option<&str> {
    let keys: Vec<&str> = ctx.keys().into_iter().map(|s| {s as &str}).collect();
    if keys.len() == 1 { Some(&keys[0]) }
    else { None }
}

/// Parallelism!
/// 
/// **Example(s)**
/// ```yaml
/// action: sys_fork
/// states:
///   from: named.yml
///   resource:
///     cuda_devices: ["0", "1", "2", "3"]
///   grid:
///   - param1: [10, 20, 40, 80, 160]
///   - param2: [0.03, 0.01, 0.003, 0.001]
/// ```
pub fn fork(ctx: Context) -> TransientContext {
    let grid = match ctx.list_contexts("grid") {
        Some(params) => params,
        None => {
            error!("Key `grid` is required.");
            return TransientContext::Diverging(ExitCode::ErrYML);
        }
    };
    if let Some(resources) = ctx.subcontext("resource") {
        if let Some(resource_type) = single_key(&resources) {
            if let Some(CtxObj::Array(pool)) = resources.get(resource_type) {
                fork_pool(grid, pool)
            }
            else { TransientContext::Diverging(ExitCode::ErrYML) }
        }
        else { fork_nolimit(grid) }
    }
    else { fork_nolimit(grid) }
}

fn param_space_iter<'a, G>(grid: G) -> impl Iterator<Item = Context> + 'a 
    where G: std::iter::IntoIterator<Item = &'a Context> + Copy
    // ^^^ Really, I am just targeting impl<'a, T> IntoIterator for &'a Vec<T>
{
    let header: Vec<&str> = grid.into_iter().filter_map(single_key).collect();
    grid.into_iter().filter_map(|ctx_param| {
        if let Some(key) = single_key(&ctx_param) {
            if let Some(CtxObj::Array(params)) = ctx_param.get(key) {
                Some(params.iter())
            }
            else { None }
        }
        else { None }
    }).multi_cartesian_product().into_iter().map(move |params| {
        let mut ctx_local = Context::new();
        for (&k, v) in header.iter().zip(params) {
            ctx_local = ctx_local.set(k, v.clone());
        }
        return ctx_local;
    })
}

fn fork_nolimit(grid: Vec<Context>) -> TransientContext {
    for ctx in param_space_iter(&grid) {
        println!("{}", ctx);
    }
    TransientContext::Diverging(ExitCode::Success) // TODO WIP
    // panic!();
}

fn fork_pool(grid: Vec<Context>, pool: &Vec<CtxObj>) -> TransientContext {
    let nproc = pool.len();
    TransientContext::Diverging(ExitCode::ErrSys)
}

/// Dynamically import vars into the `ctx_states` context.
/// This is the only system action that introduces external states to the workflow.
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
pub fn vars(ctx: Context) -> TransientContext {
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
