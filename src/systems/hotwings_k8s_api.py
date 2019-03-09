# pylint: disable=import-error,no-name-in-module
import os, sys, yaml, time
from kubernetes import client, config
from kubernetes.client.rest import ApiException

try:
    config.load_incluster_config()
except:
    config.load_kube_config()

namespace = 'bluecheese'
coreV1Api = client.CoreV1Api()
jobApi = client.BatchV1Api()

def api_job(body):
    return jobApi.create_namespaced_job(namespace, body=yaml.safe_load(body), pretty='true')

def api_pv(body):
    return coreV1Api.create_persistent_volume(body=yaml.safe_load(body), pretty='true')

def api_pvc(body):
    return coreV1Api.create_namespaced_persistent_volume_claim(namespace, body=yaml.safe_load(body), pretty='true')

def get_pods(job_spec):
    prefix = job_spec.metadata.labels["job-name"]
    return list(filter(lambda pod: pod.metadata.name.startswith(prefix), coreV1Api.list_namespaced_pod(namespace).items))

def get_pods_status(pods):
    return { pod.metadata.name: pod.status.phase for pod in pods }

def join_job(job_spec):
    # possible pod phases: https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#pod-phase
    terminal_phases = set(['Succeeded', 'Failed', 'Unknown', 'Completed'])
    refresh = lambda: get_pods_status(get_pods(job_spec))
    print('hotwings_k8s_api.py/join_job()', file=sys.stderr, flush=True)
    states = refresh()
    print('init states', states, file=sys.stderr, flush=True)
    while not all((s in terminal_phases) for s in states.values()):
        print(states, file=sys.stderr, flush=True)
        time.sleep(3)
        states = refresh()
    print('final states', states, file=sys.stderr, flush=True)

def k8s_provisioner(apicall, body):
    return globals()[apicall](body) # passing through any exceptions to playbook-rs
