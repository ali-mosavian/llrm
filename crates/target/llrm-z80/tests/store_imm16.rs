//! A constant word stored through a pointer is two `ld (hl), n`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// inlined_tail_call_keeps_frame_stores: `*p = 5` loaded the constant into a
/// pair first (`ld bc, #5; ld (hl), c; inc hl; ld (hl), b`), 6 B against 5 B.
#[test]
fn a_constant_word_store_loads_no_pair() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define void @set(ptr %p) nounwind {
  store i16 5, ptr %p
  ret void
}
"#;
    let asm = text(ir, "set", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("ld bc") && !asm.contains("ld de"), "the constant went through a pair:\n{asm}");
    assert!(asm.contains("ld (hl), #5") && asm.contains("ld (hl), #0"), "bytes not stored as constants:\n{asm}");
}

/// The high byte is stored as the constant's own high byte, not its low one.
#[test]
fn the_high_byte_of_the_constant_goes_to_the_high_address() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define void @set(ptr %p) nounwind {
  store i16 4660, ptr %p
  ret void
}
"#;
    let asm = text(ir, "set", 0, &Request::new(Level::Os)).expect("compiles");
    let (lo, hi) = (asm.find("ld (hl), #52"), asm.find("ld (hl), #18"));
    assert!(lo.is_some() && hi.is_some() && lo < hi, "0x1234 is #52 then #18:\n{asm}");
}

/// `*p = 5` as the last thing: the pointer is dead, and the `dec hl` that puts
/// it back was a byte and six clocks nothing read.
#[test]
fn a_word_store_does_not_restore_a_pointer_nobody_reads() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define void @set(ptr %p) nounwind {
  store i16 5, ptr %p
  ret void
}
"#;
    let asm = text(ir, "set", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("dec hl"), "the pointer is restored for nobody:\n{asm}");
}

/// The pointer is the return value: it is restored.
#[test]
fn a_word_store_restores_a_pointer_that_is_returned() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define ptr @set(ptr %p) nounwind {
  store i16 5, ptr %p
  ret ptr %p
}
"#;
    let asm = text(ir, "set", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(asm.contains("dec hl"), "the returned pointer is not restored:\n{asm}");
}
