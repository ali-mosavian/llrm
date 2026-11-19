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
    // f(2,3,4): x=5, y=16, s = sum(16*i+5) for i<4 = 16*6+20 = 116; f(5,7,0) =
    // 37+12 = 49.
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

/// EarlyTailDuplicate ran at -O0 (102M instructions:u of the card corpus); the
/// pin adds it only when optimizing. It must run at -O1 and not at -O0.
#[test]
fn early_tail_duplication_runs_at_o1_and_not_at_o0() {
    for (level, ran) in [("-O0", false), ("-O1", true)] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), CROSSING).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", "x86_64-linux")
            .env("LLRM_DEBUG", "time")
            .env("LLRM_TIME_TOP", "100000")
            .current_dir(scratch.path())
            .args([level, "-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{level}: {said}");
        assert_eq!(said.contains("lir earlytaildup"), ran, "{level}: {said}");
    }
}

/// An argument was copied out of its register before it was stored
/// (`mov eax, edi; mov [rbp-4], eax`): RegAllocFast had no copy hints
/// (DEVIATE R1), so the pin's `mov [rbp-4], edi` cost a `mov` per parameter.
#[test]
fn o0_stores_an_argument_straight_from_its_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "int f(int a, long b, int c)\n{\n    return a + (int)b + c;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let stored = |register: &str| asm.lines().any(|line| line.contains("ptr [rbp-") && line.trim_end().ends_with(&format!(", {register}")));
    assert!(stored("edi") && stored("rsi") && stored("edx"), "{asm}");
}

/// A value computed for a call argument went to `eax` and was copied to
/// `edi`/`esi` (`add eax, 3; mov edi, eax`, +16 B on idiv_loop_os): the copy
/// into the argument register hinted only the value it copied, not the value
/// that value was copied from.
#[test]
fn o0_computes_a_call_argument_in_its_register() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "extern int g(int, int);\nint f(int n)\n{\n    int s = 0;\n    for (int i = 1; i <= n; ++i) s += g(7 * i + 3, 5 * i + 2);\n    return s;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let moved = |register: &str| asm.lines().any(|line| line.trim() == format!("mov {register}, eax"));
    assert!(!moved("edi") && !moved("esi"), "{asm}");
}

/// The register order was worked out for every value (1,484 times in
/// div_constants_short, 2.8 M of 23.3 M Ir) though it depends only on the set
/// the value is confined to: a function with many values and one set must
/// work it out once.
#[test]
fn o0_works_out_the_register_order_once_per_confinement() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.c"),
        "extern int g(int, int);\nint f(int n)\n{\n    int s = 0;\n    for (int i = 1; i <= n; ++i) s += g(7 * i + 3, 5 * i + 2);\n    return s;\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_DEBUG", "time")
        .env("LLRM_TIME_TOP", "100000")
        .current_dir(scratch.path())
        .args(["-O0", "-S", "a.c", "-o", "a.s"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let said = String::from_utf8_lossy(&out.stderr);
    let row = said.lines().find(|line| line.ends_with("fast register order")).unwrap_or_else(|| panic!("{said}"));
    let numbers: Vec<u32> = row.split_whitespace().filter_map(|word| word.parse().ok()).collect();
    let (hits, misses) = (numbers[0], numbers[1]);
    assert!(hits > misses, "{row}");
}
