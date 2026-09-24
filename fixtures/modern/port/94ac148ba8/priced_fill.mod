fn value(v: &[i16]) -> i32:
    let mut a: i32[64] = [0] * 64
    let mut total: i16 = 0
    for i in 0..4:
        total += v[i]
    a[total] = 5
    return a[1]
fn main() -> i16:
    let v: i16[4] = [1, 2, 3, 4]
    return i16(value(&v))
