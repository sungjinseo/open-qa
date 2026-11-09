import logging

def parse(s):
    try:
        return int(s)
    except ValueError:
        logging.warning("invalid number: %s", s)
        return 0
