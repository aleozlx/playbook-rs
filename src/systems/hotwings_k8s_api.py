# pylint: disable=import-error,no-name-in-module
import os, sys, yaml, time
from kubernetes import client, config
from kubernetes.client.rest import ApiException

config.load_incluster_config()

namespace = 'bluecheese'
jobApi = client.BatchV1Api()

def k8s_provisioner(apicall):
    try:
        apicall()
    except ApiException as e:
        print('ApiException:', e.reason, file=sys.stderr)
        print(e.body, file=sys.stderr)

# TODO k8s_provisioner(lambda: jobApi.create_namespaced_job(namespace, body=yaml.safe_load(""""""), pretty='true'))
