//! A bit nothing reads is not computed (LLVM's DemandedBits, gcc's match.pd
//! mask rules): x_popcount computed all five stages of a 32-bit bit reversal to
//! read one byte of it, and x_funcptr masked `x & 3` under a sum that is masked
//! by `& 3` again.

use std::process::Command;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// bench/x_popcount at -Os was 202 bytes against gcc's 169 and clang's 149.
const POPCOUNT: &str = "static unsigned rev32(unsigned x) { x = ((x >> 1) & 0x55555555u) | ((x & 0x55555555u) << 1); \
x = ((x >> 2) & 0x33333333u) | ((x & 0x33333333u) << 2); x = ((x >> 4) & 0x0F0F0F0Fu) | ((x & 0x0F0F0F0Fu) << 4); \
x = ((x >> 8) & 0x00FF00FFu) | ((x & 0x00FF00FFu) << 8); return (x >> 16) | (x << 16); }\n\
long f(unsigned s) { return rev32(s) & 255; }\n";

#[test]
fn the_stages_of_a_bit_reversal_a_masked_byte_never_reads_are_not_computed() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = listing(POPCOUNT, level);
        let wide = text
            .lines()
            .filter(|line| line.trim_start().starts_with("shl ") && (line.ends_with(", 16") || line.ends_with(", 8")))
            .count();
        assert_eq!(wide, 0, "{level}: a stage that only the unread bytes depend on stayed:\n{text}");
    }
}

/// bench/x_funcptr: `mov ecx, eax; and ecx, 3; add ecx, edi; and ecx, 3`.
const FUNCPTR: &str = "int (*table[4])(int);\nint f(int i, int x) { return table[(i * 7 + (x & 3)) & 3](x); }\n";

#[test]
fn a_mask_under_a_sum_that_is_masked_again_is_not_applied() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = listing(FUNCPTR, level);
        let ands = text.lines().filter(|line| line.trim_start().starts_with("and ") && line.ends_with(", 3")).count();
        assert_eq!(ands, 1, "{level}: the inner mask stayed:\n{text}");
    }
}
