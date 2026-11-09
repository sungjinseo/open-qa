from flask import request
from markupsafe import Markup

def hello():
    return Markup("<h1>Hello " + request.args.get("name", "") + "</h1>")
