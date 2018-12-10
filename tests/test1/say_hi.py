#[playbook(say_hi)]
def say_hi(ctx):
    with open('/scratch/output.txt', 'w') as f:
        print("{message}".format(**ctx), file=f)
