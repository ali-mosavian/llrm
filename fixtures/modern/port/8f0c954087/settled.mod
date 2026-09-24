fn value(v: &[i16]) -> i16:
    let mut total: i16 = 0
    for i in 0..8:
        total += v[i]
    return total
