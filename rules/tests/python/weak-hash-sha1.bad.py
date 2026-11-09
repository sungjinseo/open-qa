import hashlib

def fingerprint(data):
    return hashlib.sha1(data).hexdigest()
