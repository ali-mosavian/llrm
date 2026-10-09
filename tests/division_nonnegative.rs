//! A division by a constant of a value that is never negative.

use std::process::Command;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const LOOP: &str = "int f(int n) { int i, s = 0; for (i = 0; i < n; ++i) s += (i * 97) % 211; return s; }\n";

/// `s += (i * 97) % 211` over `i = 0..n` was `cdq; idiv`, 43 clocks: signed, as `i * 97` was not known to be positive.
/// gcc multiplies by the reciprocal. (bench x_hash: 1.38x of gcc's clocks.)
#[test]
fn a_remainder_of_a_counter_s_product_is_a_multiply_not_a_division() {
    let text = listing(LOOP, &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    assert!(!text.contains("idiv") && !text.contains("div "), "{text}");
}

/// Tuned for size the division stays: `xor edx, edx; div` is a byte over `cdq; idiv`, and the reciprocal is never taken
/// for size.
#[test]
fn tuned_for_size_the_division_stays_signed() {
    let text = listing(LOOP, &["-m32", "-mabi=sysv", "-Os", "-march=i486"]);
    assert!(text.contains("idiv"), "{text}");
}
