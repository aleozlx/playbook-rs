# playbook-rs

[![Build Status](https://img.shields.io/travis/aleozlx/playbook-rs/master.svg?style=flat-square&label=master)](https://travis-ci.org/aleozlx/playbook-rs)
[![Build Status](https://img.shields.io/travis/aleozlx/playbook-rs/dev.svg?style=flat-square&label=nightly)](https://travis-ci.org/aleozlx/playbook-rs)
[![Docs](https://img.shields.io/badge/docs.rs-playbook-blue.svg?style=flat-square)](https://docs.rs/playbook)
[![Version](https://img.shields.io/crates/v/playbook.svg?style=flat-square)](https://crates.io/crates/playbook)
[![CI Base Image](https://img.shields.io/docker/automated/aleozlx/playbook-test.svg?style=flat-square)](https://hub.docker.com/r/aleozlx/playbook-test/tags/)
![Language](https://img.shields.io/github/languages/top/aleozlx/playbook-rs.svg?style=flat-square)


YAML driven container workflow orchestration

## Features

* Designed to work with `nvidia-docker2` and operationalize very complex GPU workflows
* Language agnostic symbol namespace (notice the `whitelist` and `#[playbook(something)]`)
* Static/dynamic impersonation (setuid and optionally create&reference host user) to ensure correct privileges
* Internal security hardening by whitelisting a small subset of super user capabilities
  * SETUID, SETGID, CHOWN
* X11 Graphics by setting `gui: ture`
* Each step can run in a different container (or on host) to support imcompatible dependencies in the same workflow
* Support specifying IPC and network namespace
* Simple action call convention: `awesome_func(ctx)`
* Minimal command line arguments to launch a workflow: `playbook some.yml`
* Colorful logging for readability

## Dependencies

* (Optional) Docker CE

## In a nutshell

1. Add a function `def something(ctx)`. Current execution context is in `ctx` as dict. Keys can also be accessed as attributes to save a lot of brackets and quotes. And be sure to declare this function to playbook as a symbol.

```python
## hello_world.py ##

#[playbook(something)]
def something(ctx):
    print(ctx.message)
```

2. Add the source file path to the `whitelist`.

```yml
## main.yml ##
whitelist:
- src: hello_world.py
```

3. Add an entry to your YAML file in `steps` where `action` is the step function name

```yml
## main.yml ##
whitelist:
- src: hello_world.py
steps:
- name: Some description here
  action: something
  message: Hello World!
```

4. Run it!

```bash
$ playbook main.yml
```

Check Wiki for more details and examples.

# FAQ

## What's the difference between this and _____?

### Docker-compose

There are features such as impersonate (aka container user provisioning) and others security enhancements,
which make this a better level of abstraction for research & development use.
Try and use `-vv` to see for yourself how many options we are using in the docker commands / API invocations to run things more carefully and properly.
Viewing these as repetitive work that needs to be automated, this project is created!
All of it can be done absolutely with just docker or docker-compose (which is exactly how this works), but perhaps not without more boilerplates and scripting in every single project to be containerized.

Besides, we also aim to assist global system resource coordination, especially in a shared system.
In the long term, we aim to support Kubernetes, SLURM, HTCondor or any other system we are interested in using.

### Kubernetes

Kubernetes focuses on workload, this focuses on workflow.

## Why Rust?

Firstly, any scripting language is out of the question because it is much more difficult to build a system with high reliability and sustainability requirements with languages so dynamic and tolerant to errors.
We want the system to crash hard when there is the slightest amount of outdated codes or data structures and it better does not even compile, so that we become aware of it in the earliest time possible, as opposed to well after built into containers and delivered to users.

Secondly, the best AOT compiled commitment-free languages out there, imo, are C++, Rust and Go.

Lastly, we want to have a nice package manager, so Rust is the only option here.
