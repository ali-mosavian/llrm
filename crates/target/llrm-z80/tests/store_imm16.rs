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

/// value_of_valueless -Os: one constant stored to four words was four
/// `ld (hl), #0` pairs (5 B each) where a pair loaded once stores it at 3 B a
/// word: the program grew 109 B to 113 B.
#[test]
fn a_constant_stored_twice_is_loaded_once() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define void @set(ptr %p, ptr %q) nounwind {
  store i16 7, ptr %p
  store i16 7, ptr %q
  ret void
}
"#;
    let asm = text(ir, "set", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("ld (hl), #7"), "each store carries the constant:\n{asm}");
    assert_eq!(asm.matches("#7").count(), 1, "the constant is loaded more than once:\n{asm}");
}

/// local_initializers -O1: a constant word stored to a local went through the
/// frame address (`ld hl, #5; add hl, sp; ld (hl), #9; inc hl; ld (hl), #0`),
/// +7 B and +30 T. With no frame standing the word is pushed (`ld hl, #9; push
/// hl`, 4 B), since #9403; the test read the old ix stores and went red.
#[test]
fn a_constant_word_stored_to_a_local_once_is_pushed_or_two_ix_stores() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare void @use(ptr)
define void @f() nounwind {
  %a = alloca i16
  store i16 9, ptr %a
  call void @use(ptr %a)
  ret void
}
"#;
    let asm = text(ir, "f", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("ld (hl), #9"), "stored through a built address:\n{asm}");
    assert!(asm.contains("push hl") || asm.contains("(ix), #9"), "neither pushed nor stored through ix:\n{asm}");
}

/// A frame that stands anyway (a local array indexed by `ix`) keeps the word's
/// two ix stores: a push would not reach it.
#[test]
fn a_constant_word_stored_to_a_local_in_a_standing_frame_is_two_ix_stores() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare void @use(ptr, ptr)
define void @f() nounwind {
  %a = alloca i16
  %buf = alloca [40 x i8]
  store i16 9, ptr %a
  call void @use(ptr %a, ptr %buf)
  ret void
}
"#;
    let asm = text(ir, "f", 0, &Request::new(Level::Os)).expect("compiles");
    let through_frame =
        |value: &str| asm.contains(&format!("(ix), #{value}")) || asm.contains(&format!("(iy), #{value}"));
    assert!(through_frame("9") && through_frame("0"), "not stored through ix:\n{asm}");
    assert!(!asm.contains("ld (hl), #9"), "stored through a built address:\n{asm}");
}

/// memory_expansion -O1: a constant stored to several locals as `ld d(ix), n`
/// (4 B a byte) instead of from one pair (3 B a byte): +556 B.
#[test]
fn a_constant_stored_to_two_locals_is_loaded_once() {
    let ir = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare void @use(ptr, ptr)
define void @f() nounwind {
  %a = alloca i16
  %b = alloca i16
  store i16 9, ptr %a
  store i16 9, ptr %b
  call void @use(ptr %a, ptr %b)
  ret void
}
"#;
    let asm = text(ir, "f", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("(ix), #9"), "each store carries the constant:\n{asm}");
}

const FILL: &str = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define void @fill(ptr %p, i16 %n) nounwind {
entry:
  %c = icmp eq i16 %n, 0
  br i1 %c, label %done, label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %j, %loop ]
  %q = getelementptr i16, ptr %p, i16 %i
  store i16 4660, ptr %q
  %j = add i16 %i, 1
  %e = icmp eq i16 %j, %n
  br i1 %e, label %done, label %loop
done:
  ret void
}
"#;

/// fill_nest -O2: the constant was stored from two `ld (hl), n` on every trip,
/// 6 T a trip more than from a pair held across the loop: +22144 T.
#[test]
fn at_speed_a_constant_stored_in_a_loop_is_held_across_it() {
    let asm = text(FILL, "fill", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!asm.contains("ld (hl), #52"), "the constant is stored from immediates in the loop:\n{asm}");
}

/// At -Os bytes decide: the two immediates are 4 B, the pair held 5 B.
#[test]
fn at_size_a_constant_stored_in_a_loop_stays_an_immediate() {
    let asm = text(FILL, "fill", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(asm.contains("ld (hl), #52"), "the constant was held for a byte:\n{asm}");
}
