from flask import Flask, request

app = Flask(__name__)

@app.route("/me")
def me():
    current_user = request.args.get("user")
    return current_user
