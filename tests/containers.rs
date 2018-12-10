extern crate tempfile;
extern crate playbook_api;

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
    fn docker_start01(){
        let scratch = get_scratch();
        let ctx_docker = Context::new()
            .set("image", CtxObj::Str(String::from("aleozlx/playbook-test1"))) // TODO use impersonate to mod the image
            .set("volumes", CtxObj::Array(vec![CtxObj::Str(format!("{}:/scratch:rw", scratch.path().to_str().unwrap()))]));
        match playbook_api::spawner::docker_start(ctx_docker, &["bash", "-c", "id && echo Hello World > /scratch/output.txt"]) {
            Ok(_docker_cmd) => {
                let output = get_output(&scratch, "output.txt");
                assert_eq!(output, String::from("Hello World\n"));
            }
            Err(e) => { panic!("{}", e); }
        }
    }
}
