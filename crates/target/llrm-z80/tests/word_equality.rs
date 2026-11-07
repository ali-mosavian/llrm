//! A word compared for equality with a constant.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

fn asm(constant: u32) -> String {
    let ir = format!(
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
         define i16 @f(i16 %n) {{\n %c = icmp eq i16 %n, {constant}\n br i1 %c, label %a, label %b\na:\n ret i16 5\nb:\n ret i16 6\n}}"
    );
    text(&ir, "f", 0, &Request::new(Level::O2)).expect("compiles")
}

/// `n == 2` was two byte compares with a branch between them (`ld a,l; sub
/// a,#2; jr nz; ld a,h; sub a,#0`, 10 B, 7 in SDCC). A constant with a zero
/// byte is `sub` and `or` of the other: no branch, no `sub a, #0`.
/// sibling_call_callees -O2 +12 B against SDCC.
#[test]
fn a_word_equal_to_a_constant_with_a_zero_byte_is_a_sub_and_an_or() {
    for constant in [2, 200, 512, 0x7F00] {
        let asm = asm(constant);
        assert!(!asm.contains("sub a, #0") && !asm.contains("jr nz"), "{constant}: {asm}");
        assert!(asm.contains("or a, h") || asm.contains("or a, l"), "{constant}: {asm}");
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        let sub = lines.iter().position(|line| line.starts_with("sub a, #")).expect("a sub");
        assert!(lines[sub + 1].starts_with("or a, "), "{constant}: {asm}");
    }
}

/// Both bytes nonzero keep the two compares: there is no scratch register for
/// `xor`/`xor`/`or`.
#[test]
fn a_word_equal_to_a_constant_of_two_nonzero_bytes_still_compiles() {
    assert!(asm(0x1234).contains("#18"), "{}", asm(0x1234));
}
