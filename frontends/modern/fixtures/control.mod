fn step(value: i16) -> i16:
    return value + 1

fn count(limit: i16) -> i16:
    var value: i16 = 0
    while value < limit:
        value = step(value)
        if value == 3:
            continue
        if value > 10:
            break
    return value
