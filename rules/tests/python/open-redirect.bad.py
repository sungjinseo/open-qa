from flask import redirect, request

def after_login():
    return redirect(request.args.get("next"))
