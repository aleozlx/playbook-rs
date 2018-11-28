FROM aleozlx/playbook-tensorflow:latest
MAINTAINER Alex Yang <aleozlx@gmail.com>

COPY requirements.txt /requirements.txt
RUN pip3 --no-cache-dir install -r /requirements.txt

# Bugfix
COPY util.patch /tmp/util.patch
RUN patch /usr/lib/python3.5/importlib/util.py /tmp/util.patch

ENTRYPOINT ["/bin/tkstack-start.sh"] 
