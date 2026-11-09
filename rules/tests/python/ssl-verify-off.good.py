import ssl

def context():
    return ssl.create_default_context()
