#!/usr/bin/env python3
import os, sys
import argparse, yaml, logging
import itertools, functools
from library import steps
logging.basicConfig(level=logging.DEBUG, format='[%(asctime)s] %(message)s', datefmt='%x %H:%M:%S')
logger = logging.getLogger('docker-playbook')
parser = argparse.ArgumentParser(description='YAML driven DNN trainer.')
parser.add_argument('--docker-step', type=int, help='For Docker use only: run a specific step with docker')
parser.add_argument('--container-name', type=str, help='Rename container')
parser.add_argument('--relocate', type=str, help='Relocation of the playbook inside docker')
parser.add_argument('playbook', type=str, help='YAML playbook')
args = parser.parse_args()

class Context(dict):
    def __getattr__(self, name):
        return self[name]

def context(step = {}):
    """ Deduce task execution context
        command line > step config > global config > parser defualts > context initialization """
    ctx = Context()
    ctx.update(dict())
    ctx.update({k:v for k,v in vars(args).items() if v is not None})
    ctx.update({k:v for k,v in config.items() if k != 'steps'})
    ctx.update(dict(step))
    ctx.update({k:v for k,v in vars(args).items() if v is not None and v != parser.get_default(k)})
    if inside_docker() and 'docker_overrides' in ctx.docker:
        ctx.update(ctx.docker['docker_overrides'])
        del ctx['docker']
    return ctx

def inside_docker():
    docker_flag = os.system("grep -q docker /proc/1/cgroup")==0
    assert docker_flag == (args.docker_step is not None)
    return docker_flag

def docker_start(docker, cmd):
    DOCKER = 'docker'
    DOCKER_RUN = [ DOCKER, 'run', '--rm', '-t', '-v', 
        '/home/aleozlx/Code/aml-sub/src:/opt/docker-playbook:ro', # TODO patch in the future 
        '-w', '/home/developer/workspace' ]
    if 'runtime' in docker:
        DOCKER_RUN.append('--runtime={}'.format(docker['runtime']))
    if 'interactive' in docker and docker['interactive'] == True:
        DOCKER_RUN.append('-i')
    if 'ports' in docker:
        for port in docker['ports']:
            DOCKER_RUN.extend(['-p', port])
    if 'volumes' in docker:
        for vol in docker['volumes']:
            src,dst = vol.split(':')
            DOCKER_RUN.extend(['-v', ':'.join([os.path.abspath(src), dst])])
    if 'gui' in docker and docker['gui'] == True:
        DOCKER_RUN.extend([
            '-e', 'DISPLAY', '--net=host',
            '-v', '/tmp/.X11-unix:/tmp/.X11-unix:rw',
            '-v', os.path.expanduser('~/.Xauthority') + ':/home/developer/.Xauthority:ro'
        ])
    if args.container_name:
        DOCKER_RUN.extend([
            '--name', args.container_name
        ])
    DOCKER_RUN.append(docker["image"])
    DOCKER_RUN.extend(cmd)
    logging.debug(' '.join(map(lambda i: ("'%s'"%i) if ' ' in i else i, DOCKER_RUN)))
    if 'interactive' in docker and docker['interactive'] == True:
        # This should be temporary, no join assumed.
        os.execvp(DOCKER, DOCKER_RUN)
    pid = os.fork()
    if pid == 0:
        os.execvp(DOCKER, DOCKER_RUN)
    else:
        _, status = os.waitpid(pid, 0)
        if status != 0:
            exit_code = os.WTERMSIG(status) + 128 if os.WIFSIGNALED(status) else os.WEXITSTATUS(status)
            logging.error('Docker failed. See error message above.')
            sys.exit(exit_code)

def sys_exit(ctx):
    sys.exit(0)

def sys_shell(ctx):
    if 'docker' in ctx and ctx.docker is not None:
        if not inside_docker():
            logging.warning('\033[1;32mJust a plain bash shell. Here goes nothing.\033[0m')
            ctx.docker['interactive'] = True
            docker_start(ctx.docker, ['bash'])
        else:
            logging.warning('This is a potential privilege escalation attack! Shell access is denied.')
            sys.exit(0)
    else:
        logging.error('Docker context not found!')
        sys.exit(1)

def run_step(i_step, step):
    if step["action"] in steps.whitelist:
        if not inside_docker():
            logging.info('\033[1;32mStep {}\033[0m: {}'.format(i_step+1, step["name"]))
        else:
            logging.info('\033[0;36mResuming\033[0m Step {}: {}'.format(i_step+1, step["name"]))
        run = steps.resolve(step["action"])
        ctx = context(step)
        logging.debug(ctx)
        if 'docker' in ctx and ctx.docker is not None:
            if not inside_docker():
                logging.info('Entering Docker image \033[0;36m{}\033[0m'.format(ctx.docker["image"]))
                resume_params = ['/opt/docker-playbook/playbook.py', '--docker-step={}'.format(i_step), ctx.playbook]
                if args.relocate is not None:
                    resume_params.extend(['--relocate', args.relocate])
                docker_start(ctx.docker, resume_params)
                return
        run(ctx)
    elif step["action"] in ['sys_exit', 'sys_shell']:
        logging.info('\033[1;31mBuilt-in\033[0m: {}'.format(step["name"]))
        run = globals()[step["action"]]
        ctx = context(step)
        # logging.debug(ctx)
        run(ctx)
    else:
        logging.warning('Action not recognized: {}'.format(step["action"]))

def main():
    PLAYBOOK = args.playbook
    if inside_docker(): # Resolve playbook location inside docker
        if os.path.isabs(PLAYBOOK): # Custom playbooks must be self-mounted with relocation specified at args
            assert args.relocate is not None
            PLAYBOOK = os.path.join(args.relocate, os.path.basename(PLAYBOOK))
        else: # System provided playbooks are automatically mounted read-only
            PLAYBOOK = os.path.join('/opt/docker-playbook', PLAYBOOK)
    with open(PLAYBOOK, 'r') as f:
        global config
        config = yaml.load(f)
    if inside_docker():
        step = config['steps'][args.docker_step]
        run_step(args.docker_step, step)
        sys.exit(0)
    for i_step, step in enumerate(config['steps']):
        run_step(i_step, step)

if __name__ == '__main__':
    main()