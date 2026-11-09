import ssl

def context():
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    return ctx
