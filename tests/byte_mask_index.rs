//! A byte mask that only indexes memory stays an `and` at m16 (SD330).

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

const HISTO: &str = "unsigned char data[256]; unsigned short counts[256];\n\
void f(int n, short *seed) { for (int i = 0; i < n; i++) ++counts[(data[i] * 7 + *seed) & 255]; }\n";

/// histo's index was built in a/b/c/d and moved by `movzx si, dl` (the pin's
/// `MOVZX32rr8` of `sub_8bit`): 8192 more clocks than `and si, 255`.
#[test]
fn an_index_mask_is_an_and_in_the_index_register() {
    let text = listing(HISTO, "-O2");
    assert!(text.contains(", 255") && !text.contains("movzx si, ") && !text.contains("movzx di, "), "{text}");
}

/// The mask is `movzx` where speed is not the price (a function for size).
#[test]
fn at_os_the_index_mask_stays_a_movzx() {
    let text = listing(HISTO, "-Os");
    assert!(!text.contains(", 255"), "{text}");
}

/// A mask that is returned, not addressed, keeps the three-byte `movzx`.
#[test]
fn a_mask_with_a_non_address_user_stays_a_movzx() {
    let text = listing("unsigned f(unsigned a, unsigned b) { return (a * 7 + b) & 255; }\n", "-O2");
    assert!(!text.contains(", 255"), "{text}");
}
