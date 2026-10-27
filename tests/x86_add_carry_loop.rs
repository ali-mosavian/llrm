//! A loop that decrements a counter and exits at -1 tests the add's carry.

mod common;

const KERNEL: &str = "
extern void use(int);
void g(int n, int a)
{
    if (n == 0) return;
    do {
        use(a);
        use(n - 1);
    } while (--n != 0 && n != -1);
}
";

/// `(x - 1) == -1` was `dec ebx; cmp ebx, -1; jne`: a compare the add had
/// already made. The pin reads the carry of `add ebx, -1` (`jb`), one
/// instruction fewer in every such loop (x_hanoi2 m32 -O2 +1.9% clocks).
#[test]
fn test_a_loop_exit_at_minus_one_reads_the_carry_of_the_add() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--isel=dag", "-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "-", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(!asm.contains(", -1\n    jne") && !asm.contains("cmp ebx, -1"), "{asm}");
    assert!(asm.contains("add ebx, -1") && asm.contains("jb "), "{asm}");
}
