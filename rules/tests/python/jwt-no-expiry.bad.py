import jwt


def issue_token(user_id, role, secret_key):
    return jwt.encode({"sub": user_id, "role": role}, secret_key, algorithm="HS256")
