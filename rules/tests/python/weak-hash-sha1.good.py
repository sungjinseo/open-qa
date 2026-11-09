import hashlib

def digest(data):
    return hashlib.sha512(data).hexdigest()
