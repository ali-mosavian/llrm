//! A callee-saved register's first use is its `push` and `pop` at the CPU's
//! prices (`Tuning::csr_priced`, DEVIATE CSR3 of the greedy allocator).

use std::process::Command;

/// fib's `n` and first result live across a call. On a 486 `pop` is 4 clocks
/// and a spill access 1, so `push si; push di; pop di; pop si` (10) lost to
/// the four stack accesses: fib bq16 -O2 ran 10.7% more clocks than before the
/// pin's rule (`si` and `di` kept, 24 instructions an inner node, not 22).
fn fib(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .arg(format!("{}/bench/fib/fib.bas", env!("CARGO_MANIFEST_DIR")))
        .args(["--dialect", "qb45", "--runtime", "qb45", "-m16", "-S", "-o", "a.s"])
        .args(flags)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let start = text.find("\nFIB proc").expect("FIB");
    text[start..].split("FIB endp").next().unwrap().to_owned()
}

#[test]
fn test_fib_spills_on_a_486_where_a_pop_is_four_clocks() {
    let body = fib(&["-O2", "-march=i486"]);
    assert!(!body.contains("push si"), "{body}");
}

/// Premise: where a pop is one clock the registers are still the cheaper way.
#[test]
fn test_fib_keeps_si_and_di_on_a_pentium_where_a_pop_is_one_clock() {
    let body = fib(&["-O2", "-march=pentium"]);
    assert!(body.contains("push si"), "{body}");
}

/// Under -Os bytes decide: a push and a pop are 2 bytes, a spill's accesses 3
/// or 4 each.
#[test]
fn test_fib_keeps_si_and_di_at_os_where_the_pair_is_the_smaller_code() {
    let body = fib(&["-Os", "-march=i486"]);
    assert!(body.contains("push si"), "{body}");
}
