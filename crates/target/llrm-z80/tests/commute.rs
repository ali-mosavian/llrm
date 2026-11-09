//! `add hl, rr` takes the value that dies there in `hl`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// switch_peeled_table -Os: `x - 197 <u 22` with `x` read again and `fall` in
/// `de`. The constant went to `bc` and `add hl, bc` destroyed `x`, whose copy
/// had no pair left: a frame (`push ix`, `push hl`, `pop bc`), pick 110 B and
/// 255 T a call against 92 B without.
#[test]
fn a_value_read_again_stays_out_of_hl_when_the_constant_can_take_it() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i16 @f(i16 %x, i16 %y) nounwind {
  %d = add i16 %x, -197
  %c = icmp ult i16 %d, 22
  br i1 %c, label %a, label %b
a:
  ret i16 %y
b:
  %e = icmp eq i16 %x, 219
  %r = select i1 %e, i16 1, i16 0
  ret i16 %r
}
"#;
    let asm = text(ir, "f", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("(ix)") && !asm.contains("push"), "x went to the frame:\n{asm}");
}

/// call_clobbers_flags: `n` is in the frame across the call either way, and
/// the swap copied it into `bc` first (`ld c,l; ld b,h; push bc`): +2 B, +24 T
/// at every level.
#[test]
fn a_value_that_goes_to_the_frame_across_a_call_is_not_copied_first() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare i16 @callee(i16)
define i16 @pick(i16 %n) nounwind {
  %d = sub i16 %n, 12
  %r = call i16 @callee(i16 %d)
  %c = icmp eq i16 %n, 12
  %s = select i1 %c, i16 1, i16 2
  %t = add i16 %r, %s
  ret i16 %t
}
"#;
    let asm = text(ir, "pick", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("ld c, l\n\tld b, h\n\tpush bc"), "copied before the push:\n{asm}");
}

/// long_step_by_one: the carry chain's copy of the high word was read by
/// nothing, and counted as a read after the add: `ld c,l; ld b,h` and the
/// `push de` / `pop hl` pair became four more bytes.
#[test]
fn a_copy_nothing_reads_is_not_a_later_read() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i32 @add_ones(i32 %x) nounwind {
  %r = add i32 %x, 2
  ret i32 %r
}
"#;
    let asm = text(ir, "add_ones", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("ld c, l"), "hl copied to bc for nothing:\n{asm}");
}
