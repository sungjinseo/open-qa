from flask import request

def download():
    name = request.args.get("file")
    with open("/data/uploads/" + name) as f:
        return f.read()
