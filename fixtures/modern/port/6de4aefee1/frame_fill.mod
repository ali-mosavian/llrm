fn value(k: i16) -> i32:
    var a: [i32; 64] = [0; 64]
    a[k] = 5
    return a[k] + a[k + 1]
fn main() -> i16:
    return i16(value(3))
