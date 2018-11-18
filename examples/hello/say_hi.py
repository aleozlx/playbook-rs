import os

def inside_container():
    return os.system("grep -q docker /proc/1/cgroup")==0

#[playbook(say_hi)]
def say_hi(ctx):
    if inside_container():
        print("Container: Hi!")
    else:
        print("Host: Hi!")
