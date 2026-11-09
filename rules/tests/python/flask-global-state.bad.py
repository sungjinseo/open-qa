from flask import Flask, request

app = Flask(__name__)
current_user = None

@app.route("/me")
def me():
    global current_user
    current_user = request.args.get("user")
    return current_user
