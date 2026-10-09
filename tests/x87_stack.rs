//! x87 stack form: operands are copied from where they are, not exchanged up
//! first.

use std::process::Command;

/// A value squared that stays on the stack below the top was exchanged up and
/// copied (`fxch st(1); fld st(0)`), two instructions where `fld st(1)` and
/// `fmul st(0), st(2)` do it without moving what lies between (nbody's inner
/// loop did it twice a trip: 59085 against 56685 instructions).
#[test]
fn test_a_squared_value_below_the_top_is_copied_from_where_it_is() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "double f(double x, double y, double z) { double a = x * y, b = y * z; return a * a + b * b + a * b; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    let moved_then_copied = lines.windows(2).any(|pair| pair[0].starts_with("fxch") && pair[1] == "fld st(0)");
    assert!(!moved_then_copied, "{listing}");
}

/// A value that stays below the top, read by an instruction with a memory
/// operand (`fsubr m`), was exchanged up and copied (`fxch st(1); fld st(0);
/// fsubr m`): the exchange moved what lay above it, and the join had to put it
/// back (nbody_single and fpbench, unrolled: 3 `fxch` a copy; 1000105 against
/// 900105 instructions).
#[test]
fn test_a_value_below_the_top_that_stays_is_copied_for_a_memory_operand() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "extern float gx, gy;\nfloat f(float a, float b) { float x = gx - a, y = gy - b; return x * y + a * b; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(directory.join("a.s")).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    assert!(!lines.windows(2).any(|pair| pair[0].starts_with("fxch") && pair[1] == "fld st(0)"), "{listing}");
}
