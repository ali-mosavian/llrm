//! A mask compared with zero.

use std::process::Command;

fn listing(source: &str, flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c")).current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `if (x & 1)` was `mov ecx, eax; and ecx, 1; jne`: a copy made only to be `and`ed for its flags (x_collatz, 1.32x of gcc's clocks);
/// the mask is `test`'s immediate.
#[test]
fn a_mask_compared_with_zero_is_test_with_an_immediate() {
    let text = listing("int f(unsigned x) { if (x & 1) return 3 * x + 1; return x >> 1; }\n", &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    assert!(text.contains("test eax, 1") && !text.contains("and "), "{text}");
}
