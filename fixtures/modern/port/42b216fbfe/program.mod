fn at(m: &[i16, 2], i: u8) -> i16:
    return m[i, 1]
fn value() -> i16:
    let mut a: i16[20, 20] = [[0] * 20] * 20
    a[19, 1] = 7
    let i: u8 = 19
    return at(&a, i)
