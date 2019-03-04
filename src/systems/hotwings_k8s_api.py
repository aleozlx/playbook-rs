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
    jobApi.create_namespaced_job(namespace, body=yaml.safe_load(body), pretty='true')

def api_pv(body):
    coreV1Api.create_persistent_volume(body=yaml.safe_load(body), pretty='true')

def api_pvc(body):
    coreV1Api.create_namespaced_persistent_volume_claim(namespace, body=yaml.safe_load(body), pretty='true')

def k8s_provisioner(apicall, body):
    globals()[apicall](body) # passing through any exceptions to playbook-rs
