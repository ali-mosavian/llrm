//! A pair copied while `a` is live goes through a free index register, not
//! through the stack.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `lower` copied the string pointer to the walking pair as `pha; lda rc2; sta
/// rc4; lda rc3; sta rc5; pla` (10 B, 2 B over `ldx rc2; stx rc4; ldx rc3; stx
/// rc5`): `x` is free and `a` holds the loaded byte.
#[test]
fn a_pointer_copy_beside_a_live_a_uses_a_free_x() {
    let ir = format!(
        "target datalayout = \"{}\"
define ptr @lower(ptr %s) {{
entry:
  %c0 = load i8, ptr %s
  %z = icmp eq i8 %c0, 0
  br i1 %z, label %done, label %loop
loop:
  %p = phi ptr [ %s, %entry ], [ %p1, %latch ]
  %c = phi i8 [ %c0, %entry ], [ %c1, %latch ]
  %off = add i8 %c, -65
  %up = icmp ult i8 %off, 26
  br i1 %up, label %fix, label %latch
fix:
  %l = or i8 %c, 32
  store i8 %l, ptr %p
  br label %latch
latch:
  %p1 = getelementptr inbounds i8, ptr %p, i16 1
  %c1 = load i8, ptr %p1
  %nz = icmp eq i8 %c1, 0
  br i1 %nz, label %done, label %loop
done:
  ret ptr %s
}}",
        datalayout()
    );
    let asm = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let lower = asm.split(".segment").next().unwrap();
    assert!(!lower.contains("pha") && !lower.contains("pla"), "{lower}");
}
