FROM ubuntu:16.04
MAINTAINER Alex Yang <aleozlx@gmail.com>

# System dependencies
RUN apt -y update # 1
RUN apt install -y build-essential wget
RUN apt install -y git sudo

# Rustup & Cargo
RUN wget -P /tmp --quiet https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init && chmod +x /tmp/rustup-init && /tmp/rustup-init --default-toolchain nightly-2019-02-24-x86_64-unknown-linux-gnu -y

