From rustlang/rust:nightly-slim
RUN apt -y update && apt -y install python3 python3-dev
RUN ln -sf /usr/bin/python3 /usr/bin/python
RUN cargo install --version 0.3.2 playbook --features agent
RUN ln -sT /usr/local/cargo/bin/playbook /usr/bin/playbook