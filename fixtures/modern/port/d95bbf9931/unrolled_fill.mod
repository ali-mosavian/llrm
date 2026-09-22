fn value(k: i16) -> i32:
    var a: [i32; 8, 8] = [0; 8, 8]
    a[k, 1] = 5
    return a[k, 2]
fn main() -> i16:
    return i16(value(3))
