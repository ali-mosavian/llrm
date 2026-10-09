//! A value live across a call stays in a register the callee keeps.

use std::process::Command;

const KERNEL: &str = "
extern int g(int);
int f(int a)
{
    int x = g(a);
    int y = g(x);
    return x + y;
}
";

/// Every call counted its callee's kept registers as keeping only their 16-bit half (`clobbers_high`, from the
/// contract's `i386` flag, meant for 16-bit code), so on a 32-bit target no 32-bit value crossed a call in a
/// register: `x` was stored to the frame and read back, and fib, queens and frames paid for it in every call.
#[test]
fn test_a_32_bit_value_crosses_a_call_in_a_register_the_callee_keeps() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let stored =
        listing.lines().map(str::trim).any(|line| line.starts_with("mov dword ptr [") && line.ends_with(", eax"));
    assert!(!stored, "x is stored to the frame across the second call: {listing}");
}
