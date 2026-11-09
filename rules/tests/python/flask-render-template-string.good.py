from flask import render_template_string, request

def hello():
    return render_template_string("<h1>Hello {{ name }}</h1>", name=request.args.get("name", ""))
