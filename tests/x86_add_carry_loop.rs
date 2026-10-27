//! A loop that decrements a counter and exits at -1 tests the add's carry.

mod common;

use std::path::Path;

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
///
/// llrm-cc: clang's IR already has the exit as `(n - 1) u> 1`, so no compare
/// with -1 is left for the carry to replace; the pin's `llc` on the same IR
/// runs `dec; cmp 1; ja`, and the loop is held to the pin's length (pin-shape).
#[test]
fn test_a_loop_exit_at_minus_one_reads_the_carry_of_the_add() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("a.c");
    std::fs::write(&source, KERNEL).unwrap();
    let flags = ["--isel=dag", "-m32", "-mabi=sysv", "-march=i486", "-O2"];
    let output =
        common::cc::llrm_c().current_dir(scratch.path()).args(flags).args(["-S", "-o", "-", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    let ours = common::pin::loops(&asm, Some("g"));
    let pin = common::pin::listing(Path::new(&source), &flags);
    let pins = common::pin::loops(&pin, Some("g"));
    assert!(
        pins.len() == 1 && ours.len() == 1 && ours[0] <= pins[0],
        "loops of {ours:?} instructions, the pin's llc has {pins:?}: {asm}"
    );
}
