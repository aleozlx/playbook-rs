# pylint: disable=import-error,no-name-in-module
import os, sys, yaml, time
from kubernetes import client, config
from kubernetes.client.rest import ApiException

config.load_incluster_config()

namespace = 'bluecheese'
jobApi = client.BatchV1Api()

def k8s_provisioner(apicall):
    apicall() # passing through any exceptions to playbook-rs
