from flask import redirect, request, url_for

def after_login():
    return redirect(url_for("home"))
