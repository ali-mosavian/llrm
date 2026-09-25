type fix = fixed i32, fraction=8
fn value(k: i16) -> fix:
    let mut a: fix[8, 8] = [[0] * 8] * 8
    let mut b: fix[8, 8] = [[0] * 8] * 8
    for i in 0..8:
        for j in 0..8:
            a[i, j] = fix(i * 3 + j + 1) / 4
            if i == j:
                b[i, j] = 2
            else:
                b[i, j] = fix((i + j) % 3) / 2
    return a[k, 1] + b[k, 2]
fn main() -> i16:
    value(3)
    return 0
