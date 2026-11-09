import re

def year(s):
    return re.search(r"\d{4}", s).group(0)
