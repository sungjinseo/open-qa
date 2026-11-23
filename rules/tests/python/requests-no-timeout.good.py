import requests


def fetch_profile(url):
    return requests.get(url, timeout=(3, 10)).json()
