import hashlib

def store(password):
    return hashlib.sha256(password.encode()).hexdigest()
