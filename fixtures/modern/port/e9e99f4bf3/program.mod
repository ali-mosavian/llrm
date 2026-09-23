fn boom(zero: i16) -> bool:
    return 1 / zero == 0
fn value() -> i16:
    let zero: i16 = 0
    return i16(false and boom(zero)) + i16(true or boom(zero))
