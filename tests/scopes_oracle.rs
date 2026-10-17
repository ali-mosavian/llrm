//! The edges above a loop narrow a value of the loop's `known` as they narrow
//! it of every other (`LLRM_CHECK_SCOPES`). The separable path kept what they
//! narrow from the first `known` it was asked of and read it back for a
//! `known` that lacked the narrowing (an argument `n > 0` fixes before the loop
//! the plain `known` had not been given), so the loop's blocks lost `n >= 1`:
//! 6 of 71 vsgcc programs failed the check (x_switch among them, `n >= 1` in
//! the loop of `bench_x_switch`). The objects were the same; the precision the
//! check names was not there.

use std::process::Command;

const PROGRAM: &str = "extern void report(long value);

static int step(int s, int c)
{
    switch (s) {
    case 0: return c < 64 ? 1 : (c < 128 ? 2 : 0);
    case 1: return c & 1 ? 3 : 0;
    case 2: return c > 200 ? 4 : 1;
    case 3: return (c & 6) == 6 ? 2 : 5;
    case 4: return c ^ 3 ? 0 : 5;
    default: return c < 10 ? 0 : 3;
    }
}
long bench_x_switch(int n)
{
    int i, s = 0; long sum = 0; unsigned r = 9;
    for (i = 0; i < n; ++i) { r = r * 1103515245u + 12345u; s = step(s, (int)((r >> 16) & 255)); sum += s * (i & 3); }
    return sum;
}

int main(void)
{
    report(bench_x_switch(1500));
    return 0;
}
";

#[test]
fn the_edges_above_a_loop_narrow_every_known_of_it() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("x_switch.c"), PROGRAM).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "-o", "a.obj", "x_switch.c"])
        .env("LLRM_CHECK_SCOPES", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
