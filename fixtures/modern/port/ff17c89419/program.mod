type fix = fixed i32, fraction=8
type small = fixed i16, fraction=4
fn value() -> i16:
    return i16(fix(3)) + i16(fix(2.75))
