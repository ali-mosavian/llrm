type fix = fixed i32, fraction=8
type small = fixed i16, fraction=4
fn value() -> i32:
    return i32(fix(-0.5))
