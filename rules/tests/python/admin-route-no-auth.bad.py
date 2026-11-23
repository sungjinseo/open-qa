from flask import Flask

from .models import User, db

app = Flask(__name__)


@app.route("/admin/users/<int:uid>/delete", methods=["POST"])
def delete_user(uid):
    db.session.delete(User.query.get_or_404(uid))
    db.session.commit()
    return {"ok": True}
