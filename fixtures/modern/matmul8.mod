# matmul_fixed.mod as 8x8 matrices through rank-2 views; same inputs, same checksum.
type fix = fixed i32, fraction=8

fn multiply(a: &[fix, 2], b: &[fix, 2], c: &mut [fix, 2]) -> void:
    for i in 0..a.dim[0]:
        for j in 0..b.dim[1]:
            let mut total: fix = 0
            for k in 0..a.dim[1]:
                total += a[i, k] * b[k, j]
            c[i, j] = total

fn main() -> i16:
    let seed: i16 = 1
    let mut a: fix[8, 8] = [[0] * 8] * 8
    let mut b: fix[8, 8] = [[0] * 8] * 8
    let mut c: fix[8, 8] = [[0] * 8] * 8
    for i in 0..8:
        for j in 0..8:
            a[i, j] = fix(i * 3 + j + 1 + seed) / 4
            if i == j:
                b[i, j] = 2
            else:
                b[i, j] = fix((i + j) % 3) / 2
    multiply(&a, &b, &mut c)
    let mut checksum: fix = 0
    for i in 0..8:
        for j in 0..8:
            checksum += c[i, j] * fix(i * 8 + j + 1)
    print(f"matmul: {checksum}")
    return 0
