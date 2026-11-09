import requests
from flask import request

def proxy():
    url = assert_allowed_host(request.args.get("url"))
    return requests.get(url, timeout=5).text
