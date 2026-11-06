import logging

def report(s):
    logging.getLogger(__name__).info("value=%s", s)
