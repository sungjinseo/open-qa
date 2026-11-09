import logging

def load(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        logging.exception("cannot read %s", path)
        raise
