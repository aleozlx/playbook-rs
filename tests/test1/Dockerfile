FROM aleozlx/playbook-test:base
MAINTAINER Alex Yang <aleozlx@gmail.com>
RUN apt-get install -y python3-dev && ln -sf /usr/bin/python3 /usr/bin/python

# Install the dev version of playbook-rs
ARG GUEST_FEATURES
RUN /root/.cargo/bin/cargo install --git https://github.com/aleozlx/playbook-rs.git --branch dev --features ${GUEST_FEATURES} && ln -s /root/.cargo/bin/playbook /usr/bin/playbook #9
