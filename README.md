# playbook-rs

[![master](https://travis-ci.org/aleozlx/playbook-rs.svg?branch=master)](https://travis-ci.org/aleozlx/playbook-rs)
[![nightly](https://travis-ci.org/aleozlx/playbook-rs.svg?branch=dev)](https://travis-ci.org/aleozlx/playbook-rs)

YAML driven Docker DevOps

This is designed to containerize and migrate any function (language agnostic too) in your workflow by:

> 1. YAML Task Specification => Context
> 2. Deduced Context <=> Container Environment
> 3. Context => Task Arguments (with a native data structure)

## Dependencies

* Docker CE
* (Optional) Python - use `--no-default-features --features "base"` to waive this dependency
* (Optional) nvidia-docker2

## Installation

```sh
cargo +nightly install playbook
```

or without Python:

```sh
cargo install playbook --no-default-features --features "base"
```

## Usage example

```sh
playbook say_hi.yml
```

**Features**

* Designed to work with `nvidia-docker2` and operationalize very complex GPU workflows
* Language agnostic symbol namespace (notice the `whitelist` and `#[playbook(say_hi)]`)
* Static/dynamic impersonation (setuid and optionally create&reference host user) to ensure correct privileges
* Internal security hardening by allowing a small subset of super user capabilities
  * SETUID, SETGID, CHOWN, MKNOD
* X11 Graphics `gui: ture`
* Support specifying IPC and (TODO network) namespace
* Each step can run in a different container (or on host) to support imcompatible dependencies in the same workflow
* Simple step function API: `awesome_func(ctx)`
* Minimal command line arguments to launch a workflow: `playbook some.yml`
* Colorful logging for readability

## Examples

### A task to be operationalized

```python
#[playbook(say_hi)]
def say_hi(ctx):
    print("{whoami}: Hi!".format(**ctx))
```

### Description of the resources needed in a YAML file
```yml
---
whitelist:
- src: say_hi.py
whoami: Host
steps:
- name: Running in a container
  action: say_hi
  docker:
    image: aleozlx/playbook-hello
    vars:
      whoami: Container
- name: Running on host
  action: say_hi

```

### It works!
Notice how the playbook driver spawns containers when necessary, and supply appropriate context variables to the task in its native data structure.
```
$ playbook say_hi.yml
[2018-11-19 10:51:15] INFO Step 1: Running in a container
[2018-11-19 10:51:15] INFO Entering Docker: aleozlx/playbook-hello
[2018-11-19 10:51:15] INFO ["docker", "run", "--rm", "-t", "--net=host", "-v", "/home/alex/Code/playbook-rs/examples/hello:/home/alex/current-ro", "-w", "/home/alex/current-ro", "aleozlx/playbook-hello", "/usr/bin/env", "playbook", "--docker-step=0", "say_hi.yml"]
[2018-11-19 16:51:15] INFO Step 1: Running in a container
== Context ======================
# ctx(say_hi@say_hi.py) =
---
docker-step: "0"
playbook: say_hi.yml
whoami: Container
name: Running in a container
action: say_hi
== EOF ==========================
== Output =======================
Container: Hi!
== EOF ==========================
[2018-11-19 10:51:15] INFO Step 2: Running on host
== Context ======================
# ctx(say_hi@say_hi.py) =
---
name: Running on host
whoami: Host
action: say_hi
playbook: say_hi.yml
== EOF ==========================
== Output =======================
Host: Hi!
== EOF ==========================
```

## How to add steps?

1. Add a function `def something(ctx)`. Current execution context is in `ctx` as dict. Keys are proxied to attributes to save a lot of brackets and quotes. And be sure to declare/export this function to playbook as a symbol.

```python
## say_hi.py ##

#[playbook(say_hi)]
def something(ctx):
    print(ctx.my_var)
```

2. Add the source file path to the `whitelist`.

```yml
whitelist:
- src: say_hi.py
```

3. Add an entry to your YAML file in `steps` where `action` is the step function name

```yml
steps:
  - name: Some message here
    action: something
    my_var: goes_to_ctx
```
4. The function will receive a deduced context by its native data structure.

```python
# Equivalent to calling the following after entering an appropriate environment and re-computing context
something({'my_var': 'goes_to_ctx'})
```

## How to specify docker environment?

You may add a default docker environment.
And use `vars` to change context variables when docker is in use.
```yml
docker:
  image: aleozlx/tkstack2:latest
  runtime: nvidia
  gui: False
  ports:
    - 6006:6006
  volumes:
    - /tmp:/workspace/build
    - /mnt/datasets:/workspace/datasets
  vars:
    storage: /workspace/datasets
steps:
  - name: Some message here
    action: something
    storage: /mnt/datasets
```

Or override the docker environment completely per step
```yml
docker:
  # ... snippet omitted ...
steps:
  - name: Some message here
    action: something
    storage: /mnt/datasets
    docker:
      image: aleozlx/tkstack2:latest
      runtime: nvidia
      volumes:
        - /tmp:/workspace/build
    vars:
        storage: /workspace/datasets
```

Or use the host
```yml
docker:
  # ... snippet omitted ...
steps:
  - name: Some message here
    action: something
    storage: /mnt/datasets
    docker: null
```

> Note: When a docker environment is present, the playbook starts docker accordingly and resumes itself inside docker to reuse many of the playbooks features,
> so that context deduction have a consistent behavior.
