import json
from flask import make_response, request
from markupsafe import Markup, escape

def hello():
    return Markup("<h1>Hello ") + escape(request.args.get("name", "")) + Markup("</h1>")

def data():
    return make_response(json.dumps({"q": request.args.get("q")}))
