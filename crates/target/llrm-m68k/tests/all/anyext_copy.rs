//! `any_extend` of a byte is a register copy (DEVIATE M68K-ANYEXT). bench/tile
//! -Os kept the pin's `MOVZXd32d8` under `and (any_extend i8), 63`: `moveq
//! #0,%d2; move.b %d1,%d2; moveq #63,%d3; and.l %d3,%d2` (8 B) where gcc has
//! `moveq #63,%d2; and.l %d1,%d2` (4 B), 124 B against gcc's 120.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// A byte counter (`i6` after LSR) masked to 63 and scaled to a row index.
const ROWS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @rows(ptr %p, i32 %n) nounwind optsize {
  br label %l
l:
  %i = phi i6 [ -3, %0 ], [ %i1, %l ]
  %k = phi i32 [ 0, %0 ], [ %k1, %l ]
  %z = zext i6 %i to i32
  %s = shl nuw nsw i32 %z, 7
  %q = getelementptr i8, ptr %p, i32 %s
  %v = load i16, ptr %q, align 2
  %e = sext i16 %v to i32
  %k1 = add i32 %k, %e
  %i1 = add nsw i6 %i, 1
  %k2 = add i32 %k1, 1
  %c = icmp eq i32 %k2, %n
  br i1 %c, label %x, label %l
x:
  ret i32 %k1
}";

#[test]
fn a_masked_byte_is_not_zero_extended_first() {
    let asm = assembly(ROWS, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    assert!(!asm.contains("move.b"), "the byte was zero-extended before the mask (+4 B):\n{asm}");
}
