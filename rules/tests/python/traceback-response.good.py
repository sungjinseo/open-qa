import logging

def handler():
    try:
        return int("x")
    except ValueError:
        logging.exception("parse failed")
        return "invalid request", 400
