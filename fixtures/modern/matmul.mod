# bench/c/matmul.c with 32-bit elements: the language has no integer conversions yet.
fn matmul(seed: i32) -> i32:
    let n: i32 = 8
    var a: [i32; 64] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    var b: [i32; 64] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    var c: [i32; 64] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    for i in 0..n:
        for j in 0..n:
            a[i * 8 + j] = i * 3 + j + 1 + seed
            if i == j:
                b[i * 8 + j] = 2
            else:
                b[i * 8 + j] = (i + j) % 3
    for i in 0..n:
        for j in 0..n:
            var total: i32 = 0
            for k in 0..n:
                total += a[i * 8 + k] * b[k * 8 + j]
            c[i * 8 + j] = total
    var checksum: i32 = 0
    for i in 0..n:
        for j in 0..n:
            checksum += c[i * 8 + j] * (i * 8 + j + 1)
    return checksum

fn main() -> i16:
    let checksum = matmul(1)
    if checksum == 372432:
        print(f"matmul: {checksum}")
        return 0
    print(f"matmul: bad {checksum}")
    return 1
