# Rank 2 to 4: nested && repeat literals, views, metadata.
fn trace(m: &[i16, 2]) -> i16:
    let mut total: i16 = 0
    for i in 0..m.dim[0]:
        total += m[i, i]
    return total

fn bump(m: &mut [i16, 2]) -> void:
    m[1, 2] += 100

fn main() -> i16:
    let mut a: i16[3, 3] = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    let mut cube: u8[2, 3, 4] = [[[7] * 4] * 3] * 2
    cube[1, 2, 3] = 1
    let mut t: i32[2, 2, 2, 2] = [[[[0] * 2] * 2] * 2] * 2
    t[1, 0, 1, 0] = 42
    bump(&mut a)
    let mut sum: i16 = 0
    for i in 0..2:
        for j in 0..3:
            for k in 0..4:
                sum += cube[i, j, k]
    print(f"{trace(&a)} {a[1, 2]} {sum} {t[1, 0, 1, 0]} {a.len} {a.dim[1]} {cube.dim[2]}")
    return 0
