FROM ubuntu:16.04
MAINTAINER Alex Yang <aleozlx@gmail.com>

# System dependencies
RUN apt -y update && apt install -y build-essential python3-dev python3-pip
RUN apt install -y git vim wget file patch sudo

# Rustup & Cargo
RUN wget -P /tmp --quiet https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init && chmod +x /tmp/rustup-init && /tmp/rustup-init --default-toolchain nightly -y

# Install the dev version of playbook-rs
RUN ln -sf /usr/bin/python3 /usr/bin/python
RUN /root/.cargo/bin/cargo install --git https://github.com/aleozlx/playbook-rs.git --branch dev && ln -s /root/.cargo/bin/playbook /usr/bin/playbook #3
