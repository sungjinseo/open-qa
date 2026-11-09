import random

def new_reset_token():
    reset_token = "".join(random.choice("abcdef0123456789") for _ in range(32))
    return reset_token
