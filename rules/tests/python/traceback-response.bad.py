import traceback

def handler():
    try:
        return int("x")
    except ValueError:
        return traceback.format_exc()
