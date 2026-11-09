from flask import make_response, request

def lang():
    resp = make_response("ok")
    resp.headers["X-Lang"] = request.args.get("lang")
    return resp
