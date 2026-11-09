import re

def year(s):
    m = re.search(r"\d{4}", s)
    return m.group(0) if m else None
