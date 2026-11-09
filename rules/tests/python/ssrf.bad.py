import requests
from flask import request

def proxy():
    return requests.get(request.args.get("url")).text
