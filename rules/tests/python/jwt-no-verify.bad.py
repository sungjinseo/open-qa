import jwt

def user_id(token):
    return jwt.decode(token, options={"verify_signature": False})["uid"]
