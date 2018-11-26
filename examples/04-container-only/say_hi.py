#[playbook(say_hi)]
def say_hi(ctx):
    print("{}: Hi!".format(ctx.whoami))
