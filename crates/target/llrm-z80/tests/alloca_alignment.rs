//! An `alloca` asking more than the frames keep (one byte) is clamped, one asking more than the Z80 accepts is
//! refused (`Machine::kept_stack_align`, `max_alloca_align`).

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8";

fn compiled(align: u32) -> Result<String, String> {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         define i16 @f() {{\n  %a = alloca double, align {align}\n  store double 1.0, ptr %a\n\
         \x20 %i = ptrtoint ptr %a to i16\n  %m = and i16 %i, 7\n  ret i16 %m\n}}\n"
    );
    text(&ir, "f", 0, &Request::new(Level::O2))
}

/// `alloca double, align 8` was believed: `ptr & 7` folded to the constant 0 while SP moves a byte at a time
/// (over_aligned_local_after_even_pushes passed on that fold, not on an aligned frame).
#[test]
fn an_object_up_to_the_accepted_alignment_is_not_assumed_aligned() {
    let asm = compiled(8).expect("compiles");
    assert!(asm.contains("and"), "{asm}");
}

#[test]
fn an_object_past_the_accepted_alignment_is_refused() {
    let why = compiled(16).expect_err("refused");
    assert!(why.contains("@f: an object aligned to 16 needs stack realignment"), "{why}");
}
