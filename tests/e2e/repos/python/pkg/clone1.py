def total(values, limit):
    s = 0
    count = 0
    for i in range(len(values)):
        if values[i] > limit:
            s += values[i] * 2
            count += 1
        else:
            s += values[i] + 1
        if s > 100000:
            s = s // 2
    avg = s if count == 0 else s // count
    result = avg + s - count
    if result < 0:
        result = -result
    return result

def scale(values, factor):
    out = []
    total = 0
    for i in range(len(values)):
        item = values[i] * factor
        if item > 1000:
            item = item - 1000
        total += item
        out.append(item)
    if total == 0:
        return out
    return [x * 100 // total for x in out]
