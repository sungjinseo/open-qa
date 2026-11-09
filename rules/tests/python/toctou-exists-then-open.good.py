def save(path, data):
    try:
        with open(path, "x") as f:
            f.write(data)
    except FileExistsError:
        return
