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
        common::cc::llrm_cc().current_dir(scratch.path()).args(flags).args(["-S", "-o", "-", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    let ours = common::pin::loops(&asm, Some("g"));
    let pin = common::pin::listing(Path::new(&source), &flags);
    let pins = common::pin::loops(&pin, Some("g"));
    // The pin stores each argument into the reserved call frame (`movl %edi,
    // (%esp)`); ours pushes it and cleans up with `add esp, 4` (DEVIATE CF1,
    // docs/working/ci.md call-frame-reserve): a push and an add per call, two
    // calls here.
    let calls = 2;
    assert!(
        pins.len() == 1 && ours.len() == 1 && ours[0] <= pins[0] + 2 * calls,
        "loops of {ours:?} instructions, the pin's llc has {pins:?} (+{} for CF1): {asm}",
        2 * calls
    );
}

const HANOI: &str = "
static int hanoi(int n, int a, int b, int c)
{
    if (n == 0) return 0;
    return hanoi(n - 1, a, c, b) + a + hanoi(n - 1, c, b, a);
}
int f(int n) { return hanoi(n, 1, 3, 2); }
";

fn hanoi_listing(selector: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), HANOI).unwrap();
    let flags = [selector, "-m32", "-mabi=sysv", "-march=i486", "-O2", "-fno-inline-functions"];
    let output =
        common::cc::llrm_cc().current_dir(scratch.path()).args(flags).args(["-S", "-o", "-", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// hanoi's exit at -1 was `dec ebx; cmp ebx, -1; jne`; the pin reads the carry
/// of `add ebx, -1` (`jb`). The old selector still emits the compare, so this
/// is gated on the DAG selector becoming the default (#4512): it runs
/// `--isel=dag` and the old selector's shape is asserted to differ.
///
/// llrm-cc: the kernel is the original (tests/return_duplication.rs); clang's
/// IR for it keeps the `-1` exit that the loop above lost.
#[test]
fn test_hanoi_exit_at_minus_one_reads_the_carry_of_the_add() {
    let reads_carry = |asm: &str| {
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        let add = |line: &&str| line.starts_with("add e") && line.ends_with(", -1");
        lines
            .iter()
            .enumerate()
            .any(|(at, line)| add(line) && lines[at + 1..].iter().take(4).any(|next| next.starts_with("jb ")))
    };
    let dag = hanoi_listing("--isel=dag");
    assert!(reads_carry(&dag), "{dag}");
    assert!(!dag.contains(", -1\n    jne"), "a compare with -1 stayed: {dag}");
    assert!(!reads_carry(&hanoi_listing("--isel=old")), "the old selector reads the carry: the gate is open");
}
