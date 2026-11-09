import os

def prepare(path):
    os.chmod(path, 0o777)
