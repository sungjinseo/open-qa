import os
from flask import request, send_from_directory

def static_file():
    return send_from_directory("/data/static", request.args.get("file"))

def download():
    name = os.path.basename(request.args.get("file"))
    with open(os.path.join("/data/uploads", name)) as f:
        return f.read()
