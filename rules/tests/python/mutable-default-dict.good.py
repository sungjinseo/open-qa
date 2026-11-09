def put(key, value, cache=None):
    if cache is None:
        cache = {}
    cache[key] = value
    return cache
