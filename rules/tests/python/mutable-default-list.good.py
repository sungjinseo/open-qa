def add(item, bucket=None):
    if bucket is None:
        bucket = []
    bucket.append(item)
    return bucket
