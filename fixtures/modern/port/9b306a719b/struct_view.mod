struct sample:
    tag: i16
    value: i32
    delta: i32
fn total(samples: &[sample]) -> i32:
    var sum: i32 = 0
    for one in &samples:
        sum += one.value
    return sum
fn main() -> i16:
    let s: [sample; 2] = [sample { tag: 0, value: 1, delta: 2 }, sample { tag: 0, value: 2, delta: 3 }]
    return i16(total(&s))
