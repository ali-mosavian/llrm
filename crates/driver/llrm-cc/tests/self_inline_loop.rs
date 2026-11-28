//! A recursion with one call of itself in a loop is copied into itself at -O3
//! when asked with `-fipa-cp-clone` (a search that recurses on `row + 1`);
//! x86-m32 -O3 makes no copy by default, as the pin (IPACP-O3 ended
//! 2026-11-26). Needs the Open Watcom tree (tools/ow-fetch.sh) and the m16
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

fn listing_lines(
    level: &str,
    extra: &[&str],
) -> usize {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "selfloop-{}-{}-{}",
        level.trim_start_matches('-'),
        extra.len(),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("q.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv"])
        .args(extra)
        .arg("-o")
        .arg(dir.join("q.s"))
        .arg(dir.join("q.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("q.s")).unwrap().lines().count()
}

/// A recursion calling itself once in a loop was no tree, and the bar was read
/// again on the loop that tail-call elimination leaves (22 -> 27 `gcc_insns`).
/// With `-fipa-cp-clone` the listing is 2,583 lines, with the copy missing
/// about 130; by default (no clone, as the pin) -O3 makes no copy and is no
/// longer than -O2 (243).
#[test]
fn a_recursion_with_one_call_in_a_loop_is_copied_into_itself_at_o3_only_when_asked() {
    let (o2, o3, asked) =
        (listing_lines("-O2", &[]), listing_lines("-O3", &[]), listing_lines("-O3", &["-fipa-cp-clone"]));
    assert!(asked > 1000, "-O3 -fipa-cp-clone listing is {asked} lines");
    assert!(o3 <= o2, "-O3 listing is {o3} lines by default, -O2 {o2}");
}
