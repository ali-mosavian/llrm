//! -O0 allocates with the pin's RegAllocFast (`backend::regallocfast`).

use std::process::Command;

/// A value live across a call, a branch and a loop.
const CROSSING: &str = "extern int printf(const char *, ...);\n\
    static int g(int v) { return v * 3 + 1; }\n\
    int f(int a, int b, int c)\n{\n    int x = a + b;\n    int y = g(x);\n    int i, s = 0;\n\
    for (i = 0; i < c; ++i) s += y * i + x;\n    return c ? s : y + x;\n}\n\
    int main(void)\n{\n    printf(\"%d %d\\n\", f(2, 3, 4), f(5, 7, 0));\n    return 0;\n}\n";

fn cc(
    target: &str,
    level: &str,
    extra: &[&str],
    scratch: &std::path::Path,
) -> std::process::Output {
    std::fs::write(scratch.join("a.c"), CROSSING).unwrap();
    Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_DEBUG", "regalloc")
        .current_dir(scratch)
        .args([level, "a.c"])
        .args(extra)
        .output()
        .unwrap()
}

/// At -O0 the greedy allocator's machinery ran for every function (5.7x the
/// pin's compile time, `o0-phases-vs-pin-2026-11-03`). -O0 must take the fast
/// allocator, and -O1 must keep the greedy one.
#[test]
fn o0_allocates_fast_and_o1_stays_greedy() {
    for target in ["x86_64-linux", "x86_32-linux"] {
        let scratch = tempfile::tempdir().unwrap();
        let o0 = cc(target, "-O0", &["-S", "-o", "a.s"], scratch.path());
        let said = String::from_utf8_lossy(&o0.stderr);
        assert!(o0.status.success(), "{target}: {said}");
        let fast = |line: &&str| line.ends_with("f: fast") || line.ends_with("f_: fast");
        assert!(said.lines().any(|line| fast(&line)), "{target}: -O0 did not allocate f fast: {said}");
        let o1 = cc(target, "-O1", &["-S", "-o", "a.s"], scratch.path());
        let said = String::from_utf8_lossy(&o1.stderr);
        assert!(o1.status.success(), "{target}: {said}");
        assert!(!said.contains(": fast"), "{target}: -O1 took the fast allocator: {said}");
    }
}

/// A value stored after its def and reloaded after the call that took its
/// register still has to be the value the program computes.
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
#[test]
fn o0_fast_allocation_computes_the_same_answer() {
    let scratch = tempfile::tempdir().unwrap();
    let built = cc("x86_64-linux", "-O0", &["-o", "a.out"], scratch.path());
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = Command::new(scratch.path().join("a.out")).output().unwrap();
    // f(2,3,4): x=5, y=16, s = sum(16*i+5) for i<4 = 16*6+20 = 116; f(5,7,0) = 37+12 = 49.
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "116 49\n");
}

/// A value an instruction both reads and writes (`v = shl v, count`) whose
/// register is the one the count must be in (`cl`) stayed there: the shift
/// ran on the wrong register and `shuttle_value.c` printed other numbers.
/// The value takes another register and is copied back after the shift.
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
#[test]
fn o0_tied_value_in_the_register_a_shift_count_needs_is_moved() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .current_dir(scratch.path())
        .args(["-O0", "-o", "a.out"])
        .arg(root.join("tests/run/c/shuttle_value.c"))
        .arg(root.join("tests/cc/report.c"))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = Command::new(scratch.path().join("a.out")).output().unwrap();
    let want = "19088743\n16187254512\n68719476704\n19088743\n6\n26418820992\n4294966272\n-4398046511104\n";
    assert_eq!(String::from_utf8_lossy(&ran.stdout), want);
}
