from flask import session

def panel():
    if session.get("role") == "admin":
        return "admin panel"
    return "denied"
