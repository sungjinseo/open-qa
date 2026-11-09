from flask import request
from markupsafe import Markup, escape

def hello():
    return Markup("<h1>Hello ") + escape(request.args.get("name", "")) + Markup("</h1>")
