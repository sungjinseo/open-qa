import hashlib

def fingerprint(data):
    return hashlib.sha256(data).hexdigest()
