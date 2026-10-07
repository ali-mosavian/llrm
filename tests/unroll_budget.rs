//! A completely unrolled loop is bounded by its target's `unroll_budget`, read from the description.

use std::process::Command;

/// Eight trips of four multiply-xor steps: under the 200 of x86-m16 once the unrolled work boosts it, over x86-m32's 150.
const KERNEL: &str = "
int f(const int *a)
{
    int i, s = 0;
    for (i = 0; i < 8; i++) {
        s += (a[i] ^ 3) * (s | 1); s ^= s >> 1;
        s += (a[i] ^ 4) * (s | 3); s ^= s >> 2;
        s += (a[i] ^ 5) * (s | 5); s ^= s >> 3;
        s += (a[i] ^ 6) * (s | 7); s ^= s >> 4;
    }
    return s;
}
";

/// The number of conditional jumps left in `f` at `flags`: a loop that stayed rolled has one.
fn jumps(flags: &[&str]) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    text.lines().map(str::trim).filter(|line| line.starts_with('j') && !line.starts_with("jmp")).count()
}

/// Both targets took GCC's 200 under LLVM's 400% boost, a limit over either reference's: x86-m32's matmul
/// unrolled to 2633 B where clang's is 1789 and gcc's 402.
#[test]
fn test_each_target_unrolls_to_the_budget_its_description_states() {
    assert_eq!(jumps(&["-m32", "-O2", "-march=i486"]), 1, "x86-m32 states 150: the loop stays");
    assert_eq!(jumps(&["-m32", "-O3", "-march=i486"]), 0, "-O3 is twice the target's budget");
    assert_eq!(jumps(&["-O2"]), 0, "x86-m16 states 200: the loop is copied");
}
