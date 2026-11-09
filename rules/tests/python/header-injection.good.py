from urllib.parse import quote
from flask import make_response, request

def lang():
    resp = make_response("ok")
    resp.headers["X-Lang"] = quote(request.args.get("lang", ""))
    return resp
