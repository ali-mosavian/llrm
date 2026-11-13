//! A recursion with one call of itself in a loop is copied into itself at -O3
//! (queens `place`). Needs the Open Watcom tree (tools/ow-fetch.sh) and the m16
//! clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

/// bench/queens, its kernel called through a pointer as the vsgcc wrapper does.
const SOURCE: &str = "extern void report(long value);
static int safe(const int *q, int row, int col)
{
    int r;
    for (r = 0; r < row; ++r) {
        int d = q[r] - col;
        if (d == 0 || d == row - r || d == r - row) return 0;
    }
    return 1;
}
static int place(int *q, int row, int n)
{
    int col, found = 0;
    if (row == n) return 1;
    for (col = 0; col < n; ++col)
        if (safe(q, row, col)) {
            q[row] = col;
            found += place(q, row + 1, n);
        }
    return found;
}
long bench_queens(int n)
{
    int q[12];
    return place(q, 0, n);
}
long (*volatile bench_queens_via)(int n) = bench_queens;
int main(void)
{
    report((*bench_queens_via)(7));
    return 0;
}
";

fn listing_lines(level: &str) -> usize {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "selfloop-{}-{}",
        level.trim_start_matches('-'),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("q.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("q.s"))
        .arg(dir.join("q.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("q.s")).unwrap().lines().count()
}

/// queens -O3 ran 154117 clocks (gcc 94851) with `place` calling itself once:
/// its one call in a loop was no tree, and the bar was read again on the loop
/// that tail-call elimination leaves (22 -> 27 `gcc_insns`). The listing was
/// 135 lines with either missing, 504 with both fixed (-O2: 105).
#[test]
fn a_recursion_with_one_call_in_a_loop_is_copied_into_itself_at_o3_only() {
    let (o2, o3) = (listing_lines("-O2"), listing_lines("-O3"));
    assert!(o3 > 300, "-O3 listing is {o3} lines");
    assert!(o2 < 200, "-O2 listing is {o2} lines");
}
