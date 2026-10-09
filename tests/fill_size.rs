//! A zero fill tuned for size.

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

const CLEAR: &str =
    "int g(int *);\nint f(void) { int t[903]; int i; for (i = 0; i < 903; ++i) t[i] = 0; return g(t); }\n";

/// A frame array of 903 ints cleared at -Os was `mov al, 0; mov ecx, 3612; rep stosb`, 3612 iterations of the string
/// move where `xor eax, eax; mov ecx, 903; rep stosd` is as many bytes and a quarter of the iterations: bintree -Os
/// cost 120145 clocks to gcc's 112830 and clang's 68349.
#[test]
fn a_zero_fill_of_dwords_at_size_in_a_32_bit_segment_is_stosd() {
    let text = listing(CLEAR, &["-m32", "-Os", "-march=i486"]);
    assert!(text.contains("rep stosd") && !text.contains("rep stosb"), "{text}");
}

/// In a 16-bit segment `stosd` and `xor eax, eax` each take an operand-size prefix: the byte fill is shorter and stays.
#[test]
fn a_zero_fill_at_size_in_a_16_bit_segment_stays_bytes() {
    let text = listing(CLEAR, &["-Os"]);
    assert!(text.contains("rep stosb"), "{text}");
}
