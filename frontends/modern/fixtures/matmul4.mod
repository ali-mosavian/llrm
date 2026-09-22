# A 4x4 matrix product in 24.8 fixed point through rank-2 views.
type fix = fixed i32, fraction=8

fn multiply(a: &[fix, 2], b: &[fix, 2], c: &mut [fix, 2]) -> void:
    for i in 0..a.dim(0):
        for j in 0..b.dim(1):
            var total: fix = 0
            for k in 0..a.dim(1):
                total += a[i, k] * b[k, j]
            c[i, j] = total

fn main() -> i16:
    let a: [fix; 4, 4] = [[1, 0.5, 0, -2], [0.25, 3, 1, 0], [-1, 2, 0.75, 1], [0, 0, 1.5, 4]]
    let b: [fix; 4, 4] = [[2, 0, 1, 0], [0, 1, 0.5, 0], [1, 1, 1, 1], [0.5, -0.25, 0, 2]]
    var c: [fix; 4, 4] = [0; 4, 4]
    multiply(&a, &b, &mut c)
    for i in 0..4:
        print(f"{c[i, 0]} {c[i, 1]} {c[i, 2]} {c[i, 3]}")
    return 0
