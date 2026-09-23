struct sample:
    tag: i16
    value: i32
    delta: i32

fn total(samples: &[sample]) -> i32:
    var sum: i32 = 0
    for one in &samples:
        sum += one.value
    return sum

fn update(v: &[i32]) -> i32:
    var samples: [sample; 5] = [
        sample { tag: 0, value: v[0], delta: v[1] },
        sample { tag: 0, value: v[1], delta: v[2] },
        sample { tag: 0, value: v[2], delta: v[3] },
        sample { tag: 0, value: v[3], delta: v[4] },
        sample { tag: 0, value: v[4], delta: v[5] },
    ]
    for current in &mut samples:
        current.value += current.delta
    return total(&samples)

fn main() -> i16:
    let v: [i32; 6] = [1, 2, 3, 4, 5, 6]
    update(&v)
    return 0
