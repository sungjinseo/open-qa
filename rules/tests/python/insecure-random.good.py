import random
import secrets

def new_reset_token():
    reset_token = secrets.token_hex(16)
    color_code = random.choice(["red", "green"])
    status_code = random.randint(200, 204)
    return reset_token, color_code, status_code
