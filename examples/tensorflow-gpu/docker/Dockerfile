FROM tensorflow/tensorflow:1.12.0-devel-gpu-py3
MAINTAINER Alex Yang <aleozlx@gmail.com>

# System dependencies
RUN apt-get -y update && apt-get install -y build-essential gfortran libblas-dev liblapack-dev libatlas-base-dev libssl-dev vim wget python3-dev python3-pip python3-tk

# Sudo
RUN apt-get install sudo gosu
COPY bashrc.sh /bin/tkstack-bashrc.sh
COPY start.sh /bin/tkstack-start.sh

# Python packages
RUN pip3 install --upgrade pip
RUN BLAS=/usr/lib/libblas/libblas.so LAPACK=/usr/lib/lapack/liblapack.so pip3 --no-cache-dir install numpy>=1.15.4 scipy
RUN pip3 install numba
COPY requirements.txt /requirements.txt
RUN pip3 --no-cache-dir install -r /requirements.txt

# Bugfix
COPY util.patch /tmp/util.patch
RUN patch /usr/lib/python3.5/importlib/util.py /tmp/util.patch

# Rustup & Cargo
RUN apt-get -y install file
RUN wget -P /tmp --quiet https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init && chmod +x /tmp/rustup-init && /tmp/rustup-init --default-toolchain nightly -y
RUN /root/.cargo/bin/cargo install playbook --version 0.2.2 && cp /root/.cargo/bin/playbook /usr/bin
ENTRYPOINT ["/bin/tkstack-start.sh"] 
