#[playbook(say_hi)]
def say_hi(ctx):
    print("{message}".format(**ctx))
