# bench/c/matmul.c in 24.8 fixed point: quarter-step inputs, so every product is exact.
type fix = fixed i32, fraction=8

fn matmul(seed: i32) -> fix:
    let n: i16 = 8
    let mut a: fix[64] = [0] * 64
    let mut b: fix[64] = [0] * 64
    let mut c: fix[64] = [0] * 64
    for i in 0..n:
        for j in 0..n:
            a[i * 8 + j] = fix(i * 3 + j + 1 + seed) / 4
            if i == j:
                b[i * 8 + j] = 2
            else:
                b[i * 8 + j] = fix((i + j) % 3) / 2
    for i in 0..n:
        for j in 0..n:
            let mut total: fix = 0
            for k in 0..n:
                total += a[i * 8 + k] * b[k * 8 + j]
            c[i * 8 + j] = total
    let mut checksum: fix = 0
    for i in 0..n:
        for j in 0..n:
            checksum += c[i * 8 + j] * fix(i * 8 + j + 1)
    return checksum

fn main() -> i16:
    print(f"matmul: {matmul(1)}")
    return 0
