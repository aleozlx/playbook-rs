FROM ubuntu:16.04
MAINTAINER Alex Yang <aleozlx@gmail.com>

# System dependencies
RUN apt-get -y update && apt-get install -y build-essential gfortran libblas-dev liblapack-dev libatlas-base-dev libssl-dev python3-dev python3-pip python3-tk
RUN apt-get install -y vim wget file patch sudo gosu

COPY start.sh /bin/tkstack-start.sh
ENTRYPOINT ["/bin/tkstack-start.sh"] 
