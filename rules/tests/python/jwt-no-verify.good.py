import os
import jwt

def user_id(token):
    return jwt.decode(token, os.environ["JWT_SECRET"], algorithms=["HS256"])["uid"]
