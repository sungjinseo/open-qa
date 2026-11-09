import hashlib

def fingerprint(data):
    return hashlib.sha512(data).hexdigest()
