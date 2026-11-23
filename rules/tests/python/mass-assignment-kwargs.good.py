from flask import Flask, request

from .models import User, db

app = Flask(__name__)


@app.post("/users")
def create_user():
    data = request.json
    user = User(name=data["name"], email=data["email"])
    db.session.add(user)
    db.session.commit()
    return {"id": user.id}
