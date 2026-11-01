//! A byte widened to a word is the word movzx at m16 (SD331).

mod common;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args([level, "-m16", "-march=i386", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// A byte loaded into a word was `movzx ecx, byte ptr` (the pin's `MOVZX32rm8`
/// of `sub_16bit`): an operand-size prefix, +1 byte and +1 clock per load.
#[test]
fn a_byte_widened_to_a_word_is_a_word_movzx() {
    let text = listing("unsigned short f(unsigned char *p, unsigned short s) { return *p * 7 + s; }\n", "-O2");
    assert!(text.contains("movzx ") && !text.contains("movzx e"), "{text}");
}
