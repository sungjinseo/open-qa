import secrets

def new_reset_token():
    reset_token = secrets.token_hex(16)
    return reset_token
