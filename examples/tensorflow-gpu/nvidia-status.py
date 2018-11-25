import subprocess

def gpu_info():
    return subprocess.getoutput('nvidia-smi --query-gpu=gpu_name,pstate,utilization.gpu,utilization.memory,temperature.gpu --format=csv,noheader').strip()

#[playbook(nvidia_status)]
def nvidia_status(ctx):
    print(gpu_info())
