FROM tensorflow/tensorflow:1.12.0-devel-gpu-py3
MAINTAINER Alex Yang <aleozlx@gmail.com>

# System dependencies
RUN apt-get -y update && apt-get install -y build-essential gfortran libblas-dev liblapack-dev libatlas-base-dev libssl-dev python3-dev python3-pip python3-tk
RUN apt-get install -y vim wget file patch sudo

# Rustup & Cargo
RUN wget -P /tmp --quiet https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init && chmod +x /tmp/rustup-init && /tmp/rustup-init --default-toolchain nightly -y

# Python high performance scientific computing
RUN pip3 install --upgrade pip
RUN BLAS=/usr/lib/libblas/libblas.so LAPACK=/usr/lib/lapack/liblapack.so pip3 --no-cache-dir install numpy>=1.15.4 scipy numba

# Playbook-rs DevOps
RUN /root/.cargo/bin/cargo install playbook --version 0.3.2 --features agent && cp /root/.cargo/bin/playbook /usr/bin
#RUN /root/.cargo/bin/cargo install --git https://github.com/aleozlx/playbook-rs.git --branch dev && ln -s /root/.cargo/bin/playbook /usr/bin/playbook #3

COPY bashrc.sh /bin/tkstack-bashrc.sh
