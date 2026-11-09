import os
from flask import request

def download():
    name = os.path.basename(request.args.get("file"))
    with open(os.path.join("/data/uploads", name)) as f:
        return f.read()
