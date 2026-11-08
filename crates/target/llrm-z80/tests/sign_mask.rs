//! The sign mask of a byte.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `(signed char)(x >> 8)` as a long made the high word's sign mask with `sra
/// b` seven times (14 B, 56 T, high_byte -O2 +28 B against SDCC) in a register
/// other than `a`; the byte's bit 7 into the carry and out again is 4 B (`ld a,
/// r; add a, a; sbc a, a; ld r, a`) when `a` is free.
#[test]
fn the_sign_mask_of_a_byte_is_not_seven_arithmetic_shifts() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i32 @f(i16 %x) {\n %s = ashr i16 %x, 8\n %b = trunc i16 %s to i8\n %r = sext i8 %b to i32\n ret i32 %r\n}";
    let asm = text(ir, "f", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!asm.contains("sra "), "{asm}");
}
