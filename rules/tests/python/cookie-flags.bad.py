from flask import make_response

def login(sid):
    resp = make_response("ok")
    resp.set_cookie("sid", sid)
    return resp
