# playbook-rs

[![Build Status](https://travis-ci.org/aleozlx/playbook-rs.svg?branch=master)](https://travis-ci.org/aleozlx/playbook-rs)

YAML driven Docker DevOps

> Allows customization of container environment individually for each step, and forwarding context variables in data structures native to the language of the task being run.

## Requirements

* Docker - make sure you can run `docker images`
* (Optional) Python - use `--no-default-features --features "base"` to waive this dependency
* (Optional) nvidia-docker2 - make sure you can run `nvidia-smi` on host

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

* Designed to work with `nvidia-docker2`
* Runs whitelisted steps in sequential manner (notice the `whitelist` and `#[playbook(say_hi)]`)
* Context deduction: each step can run in a different docker image or on host
* Full access to host network services
* Simple step function siginature `awesome_func(ctx)` - easy to extend
* Minimal command line arguments: `playbook some.yml`
* Colorful logging for readability

To show you what I mean...

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
    docker_overrides:
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

1. Add a function `def something(ctx)`. Current execution context is in `ctx` as dict. Keys are proxied to attributes to save a lot of brackets and quotes.

```python
def something(ctx):
    print(ctx.my_var)
```

2. Whitelist your step function using `#[playbook(...)]` and add the source file path to `whitelist` variable.
3. Add an entry to your YAML file in `steps` where `action` is the step function name:

```yml
steps:
  - name: Some message here
    action: something
    my_var: goes_to_ctx
```

## Context deduction rules

```
docker overrides > step context > global context
```

## How to specify docker environment?

You may add a default docker environment.
And use `docker_overrides` to change context variables when docker is in use.
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
  docker_overrides:
    storage: /workspace/datasets
steps:
  - name: Some message here
    action: something
    storage: /mnt/datasets
```

Or override the docker environment completely per step
```yml
docker:
  # ...
steps:
  - name: Some message here
    action: something
    storage: /mnt/datasets
    docker:
      image: aleozlx/tkstack2:latest
      runtime: nvidia
      volumes:
        - /tmp:/workspace/build
    docker_overrides:
        storage: /workspace/datasets
```

Or use the host
```yml
docker:
  # ...
steps:
  - name: Some message here
    action: something
    storage: /mnt/datasets
    docker: null
```

> Note: When a docker environment is present, the playbook starts docker accordingly and resumes itself inside docker to reuse many of the playbooks features,
> so that context deduction have a consistent behavior.

## Security assumptions

> **Host file system**: volumes specified in your playbook will be mounted read-only by default, which you may override. The current working directory is mounted read-only automatically. Playbook assumes that you use a docker image that uses non-root user whose uid:gid **hopefully** maps to you on host system. Issue [#7](https://github.com/aleozlx/playbook-rs/issues/7) is created to alleviate this.

> **Network**: network services inside docker are **not isolated** from host in non-interactive mode to provide **convenient access** to host databases etc. Playbook assumes whatever you are operationalizing is trusted and that your host should have a proper set of INPUT rules, and that services inside docker should be protected by an independent firewall if necessary. (This will be isolated in the future.)

> **X11**: the recommended docker image does intend to provide isolated X11 access (TODO reference needed here) by creating non-root user that **presumably** maps to you on host and your X authentication files are naturally mounted with proper permissions already in place. Issue [#7](https://github.com/aleozlx/playbook-rs/issues/7) is created to alleviate this.

> **Playbook itself**: the playbook itself is obviously a very capable shell program. It is based on a simple whitelist to establish its symbol namespace and allow any actions to be executed. Predictable behaviors of the program relies on the `playbook` binaries share the same version both on the host and in the container. Issue [#3](https://github.com/aleozlx/playbook-rs/issues/3) is created to alleviate this.


