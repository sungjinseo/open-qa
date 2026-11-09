from flask import request

def greet(user):
    return "Hello {name}".format(name=request.args.get("name"))
