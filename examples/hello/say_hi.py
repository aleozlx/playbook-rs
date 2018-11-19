#[playbook(say_hi)]
def say_hi(ctx):
    print("{whoami}: Hi!".format(**ctx))
