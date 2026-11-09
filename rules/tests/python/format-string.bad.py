from flask import request

def greet(user):
    template = request.args.get("tpl")
    return template.format(user=user)
