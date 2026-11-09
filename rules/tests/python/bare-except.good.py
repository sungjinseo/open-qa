def parse(s):
    try:
        return int(s)
    except ValueError:
        return 0
