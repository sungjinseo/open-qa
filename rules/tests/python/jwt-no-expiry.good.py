from datetime import datetime, timedelta, timezone

import jwt


def issue_token(user_id, role, secret_key):
    exp = datetime.now(timezone.utc) + timedelta(minutes=15)
    return jwt.encode({"sub": user_id, "role": role, "exp": exp}, secret_key, algorithm="HS256")
