from flask import request

def panel():
    if request.cookies.get("is_admin") == "1":
        return "admin panel"
    return "denied"
