import logging
import traceback

logger = logging.getLogger(__name__)

def handler():
    try:
        return int("x")
    except ValueError:
        logging.exception("parse failed")
        logging.error("trace: %s", traceback.format_exc())
        logger.error(traceback.format_exc())
        return "invalid request", 400
