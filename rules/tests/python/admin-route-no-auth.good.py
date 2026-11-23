from flask import Flask
from flask_login import login_required

from .auth import admin_required
from .models import User, db

app = Flask(__name__)


@app.route("/admin/users/<int:uid>/delete", methods=["POST"])
@login_required
@admin_required
def delete_user(uid):
    db.session.delete(User.query.get_or_404(uid))
    db.session.commit()
    return {"ok": True}


@app.route("/admin/stats")
@login_required
def stats():
    return {"users": User.query.count()}
