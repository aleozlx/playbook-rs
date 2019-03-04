extern crate tempfile;
extern crate playbook_api;

#[cfg(feature = "as_switch")]
extern crate handlebars;

#[cfg(test)]
mod test_containers {
    use std::io::prelude::*;
    use std::os::unix::fs::PermissionsExt;
    use playbook_api::{Context, CtxObj};
    use tempfile::{Builder, TempDir};

    fn get_scratch() -> TempDir {
        match Builder::new().tempdir() {
            Ok(tmpdir) => {
                let mut metadata = std::fs::metadata(tmpdir.path().to_str().unwrap())
                    .expect("Failed to read scratch folder metadata.");
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o755);
                return tmpdir;
            },
            Err(e) => panic!("Failed to allocate a scratch folder: {}", e)
        }
    }

    fn get_output(tmpdir: &TempDir, fname: &str) -> String {
        let mut f = std::fs::File::open(tmpdir.path().join(fname)).expect("file not found");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("IO Error while getting the output.");
        return contents;
    }

    #[test]
    #[should_panic]
    fn docker_startn0(){
        let ctx_docker = Context::new();
        match playbook_api::systems::docker::start(ctx_docker, &["true"]) {
            Ok(_docker_cmd) => { }
            Err(e) => { panic!("{}", e); }
        }
    }

    #[test]
    fn docker_start00(){
        let ctx_docker = Context::new()
            .set("image", CtxObj::Str(String::from("aleozlx/playbook-test:test1")));
        match playbook_api::systems::docker::start(ctx_docker, &["true"]) {
            Ok(_docker_cmd) => { }
            Err(e) => { panic!("{}", e); }
        }
    }

    #[test]
    fn docker_start01(){
        let scratch = get_scratch();
        let ctx_docker = Context::new()
            .set("image", CtxObj::Str(String::from("aleozlx/playbook-test:test1")))
            .set("volumes", CtxObj::Array(vec![CtxObj::Str(format!("{}:/scratch:rw", scratch.path().to_str().unwrap()))]))
            .set("interactive", CtxObj::Bool(false));
        match playbook_api::systems::docker::start(ctx_docker, &["bash", "-c", "echo Hello World > /scratch/output.txt"]) {
            Ok(_docker_cmd) => {
                let output = get_output(&scratch, "output.txt");
                assert_eq!(output, String::from("Hello World\n"));
            }
            Err(e) => { panic!("{}", e); }
        }
    }

    #[test]
    fn docker_start02(){
        let scratch = get_scratch();
        let ctx_docker = Context::new()
            .set("image", CtxObj::Str(String::from("aleozlx/playbook-test:test1")))
            .set("volumes", CtxObj::Array(vec![CtxObj::Str(format!("{}:/scratch:rw", scratch.path().to_str().unwrap()))]))
            .set("impersonate", CtxObj::Str(String::from("dynamic")))
            .set("interactive", CtxObj::Bool(false));
        match playbook_api::systems::docker::start(ctx_docker, &["--arg-resume", r#"{"c":1,"p":0,"s":{"data":{}}}"#, "tests/test1/say_hi.yml"]) {
            Ok(_docker_cmd) => {
                let output = get_output(&scratch, "output.txt");
                assert_eq!(output, String::from("Hello World\n"));
            }
            Err(e) => { panic!("{}", e); }
        }
    }

    #[test]
    fn full_play01(){
        let scratch = get_scratch();
        let raw = playbook_api::load_yaml("tests/test1/say_hi.yml").expect("Cannot load test playbook.");
        let playbook = raw.set("docker", CtxObj::Context(raw.subcontext("docker").unwrap()
            .set("volumes", CtxObj::Array(vec![CtxObj::Str(format!("{}:/scratch:rw", scratch.path().to_str().unwrap()))]))
        ));
        let ctx_args = Context::new()
            .set("playbook", CtxObj::Str(String::from("tests/test1/say_hi.yml")));
        match playbook_api::run_playbook(playbook, ctx_args) {
            Ok(()) => {
                let output = get_output(&scratch, "output.txt");
                assert_eq!(output, String::from("Hello World\n"));
            }
            Err(e) => { panic!("Error: exit_code = {:?}", e); }
        }
    }

    #[test]
    fn full_play02_test_sys_vars(){
        let scratch = get_scratch();
        let raw = playbook_api::load_yaml("tests/test1/test_sys_vars.yml").expect("Cannot load test playbook.");
        let playbook = raw.set("docker", CtxObj::Context(raw.subcontext("docker").unwrap()
            .set("volumes", CtxObj::Array(vec![CtxObj::Str(format!("{}:/scratch:rw", scratch.path().to_str().unwrap()))]))
        ));
        let ctx_args = Context::new()
            .set("playbook", CtxObj::Str(String::from("tests/test1/test_sys_vars.yml")));
        match playbook_api::run_playbook(playbook, ctx_args) {
            Ok(()) => {
                let output = get_output(&scratch, "output.txt");
                assert_eq!(output, String::from("Salut!\n"));
            }
            Err(e) => { panic!("Error: exit_code = {:?}", e); }
        }
    }
}

#[cfg(test)]
mod test_parallelism {
    // use std::io::prelude::*;
    // use std::os::unix::fs::PermissionsExt;
    use playbook_api::{Context, CtxObj};

    #[test]
    fn trying_out_sys_fork(){
        let playbook = playbook_api::load_yaml("tests/test2/fork_simple.yml").expect("Cannot load test playbook.");
        let ctx_args = Context::new()
            .set("playbook", CtxObj::Str(String::from("tests/test2/fork_simple.yml")));
        match playbook_api::run_playbook(playbook, ctx_args) {
            Ok(()) => {
                // let output = get_output(&scratch, "output.txt");
                // assert_eq!(output, String::from("Salut!\n"));
            }
            Err(e) => { panic!("Error: exit_code = {:?}", e); }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "as_switch")]
mod test_as_switch {
    use std::collections::BTreeMap;
    use playbook_api::{Context, CtxObj};
    use handlebars::Handlebars;

    #[test]
    fn template0() {
        let mut renderer = Handlebars::new();
        renderer.register_template_string("t0", "Hello {{msg}}").unwrap();
        let mut ctx = BTreeMap::new();
        ctx.insert("msg", String::from("test"));
        let out = renderer.render("t0", &ctx).unwrap();
        assert_eq!(out, String::from("Hello test"))
    }

    #[test]
    fn template1() {
        let mut renderer = Handlebars::new();
        // ref: https://github.com/aleozlx/ymlctx/blob/master/src/lib.rs
        //      https://serde.rs/enum-representations.html        vvv  Enum external tagging
        renderer.register_template_string("t0", "Hello {{data.msg.Str}}").unwrap();
        let ctx = Context::new().set("msg", CtxObj::Str(String::from("test")));
        let out = renderer.render("t0", &ctx).unwrap();
        assert_eq!(out, String::from("Hello test"))
    }
}

#[cfg(test)]
#[cfg(feature = "sys_hotwings")]
mod test_hotwings {
    use playbook_api::systems::hotwings;

    #[test]
    fn hotwings_basic() {
        let raw = playbook_api::load_yaml("tests/test1/say_hi.yml").expect("Cannot load test playbook.");
        let ctx_docker = raw.subcontext("docker").unwrap();
        let cmd = vec![String::from("main.yml")];
        match hotwings::k8s_api(ctx_docker, cmd) {
            Ok(resources) => { assert_eq!(resources[0], include_str!("fixtures/batch-basic.yml")); }
            Err(e) => { panic!("{}", e); }
        }
    }
}
