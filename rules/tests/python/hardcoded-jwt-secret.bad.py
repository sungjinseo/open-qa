import jwt

def issue(uid):
    return jwt.encode({"uid": uid}, "my-super-secret", algorithm="HS256")
