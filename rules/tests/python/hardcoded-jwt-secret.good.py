import os
import jwt

def issue(uid):
    return jwt.encode({"uid": uid}, os.environ["JWT_SECRET"], algorithm="HS256")
