import requests


def fetch_profile(url):
    return requests.get(url).json()
