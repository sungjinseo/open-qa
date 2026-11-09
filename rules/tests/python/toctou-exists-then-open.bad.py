import os

def save(path, data):
    if not os.path.exists(path):
        with open(path, "w") as f:
            f.write(data)
