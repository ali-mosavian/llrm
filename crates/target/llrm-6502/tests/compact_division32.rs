//! A long division the module runs once calls the compact 32-step walk at
//! -O1 to -O3 too, as at -Os: the speed family it called was 298 bytes linked
//! (`__divsi3_fast_small` and what it calls) against 212, for clocks the cell
//! does not spend (compound_assignment_types: 1,026 B at -O2, 940 at -Os).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(
    body: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{body}\n", datalayout());
    assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"))
}

/// One division, run once, among the other work of a program (twelve long
/// multiplies: a division is the whole of a small module, which then keeps the
/// speed family).
const COLD: &str = "@v = global i32 3
@w = global i32 0
define i32 @main() {
  %x0 = load volatile i32, ptr @v
  %y0 = load volatile i32, ptr @v
  %m0 = mul i32 %x0, %y0
  store volatile i32 %m0, ptr @w
  %x1 = load volatile i32, ptr @v
  %y1 = load volatile i32, ptr @v
  %m1 = mul i32 %x1, %y1
  store volatile i32 %m1, ptr @w
  %x2 = load volatile i32, ptr @v
  %y2 = load volatile i32, ptr @v
  %m2 = mul i32 %x2, %y2
  store volatile i32 %m2, ptr @w
  %x3 = load volatile i32, ptr @v
  %y3 = load volatile i32, ptr @v
  %m3 = mul i32 %x3, %y3
  store volatile i32 %m3, ptr @w
  %x4 = load volatile i32, ptr @v
  %y4 = load volatile i32, ptr @v
  %m4 = mul i32 %x4, %y4
  store volatile i32 %m4, ptr @w
  %x5 = load volatile i32, ptr @v
  %y5 = load volatile i32, ptr @v
  %m5 = mul i32 %x5, %y5
  store volatile i32 %m5, ptr @w
  %x6 = load volatile i32, ptr @v
  %y6 = load volatile i32, ptr @v
  %m6 = mul i32 %x6, %y6
  store volatile i32 %m6, ptr @w
  %x7 = load volatile i32, ptr @v
  %y7 = load volatile i32, ptr @v
  %m7 = mul i32 %x7, %y7
  store volatile i32 %m7, ptr @w
  %x8 = load volatile i32, ptr @v
  %y8 = load volatile i32, ptr @v
  %m8 = mul i32 %x8, %y8
  store volatile i32 %m8, ptr @w
  %x9 = load volatile i32, ptr @v
  %y9 = load volatile i32, ptr @v
  %m9 = mul i32 %x9, %y9
  store volatile i32 %m9, ptr @w
  %x10 = load volatile i32, ptr @v
  %y10 = load volatile i32, ptr @v
  %m10 = mul i32 %x10, %y10
  store volatile i32 %m10, ptr @w
  %x11 = load volatile i32, ptr @v
  %y11 = load volatile i32, ptr @v
  %m11 = mul i32 %x11, %y11
  store volatile i32 %m11, ptr @w
  %x = load volatile i32, ptr @v
  %y = load volatile i32, ptr @v
  %q = sdiv i32 %x, %y
  ret i32 %q
}";

/// A division by a byte value (the speed family has a short path for it).
const BYTE: &str = "@v = global i32 3
@b = global i8 3
define i32 @main() {
  %x = load volatile i32, ptr @v
  %y8 = load volatile i8, ptr @b
  %y = zext i8 %y8 to i32
  %q = udiv i32 %x, %y
  ret i32 %q
}";

#[test]
fn a_cold_long_division_is_the_compact_walk_at_o2() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let text = asm(COLD, level);
        let compact = text.contains("jsr __divsi3\n") || text.contains("jmp __divsi3\n");
        assert!(compact && !text.contains("__divsi3_fast"), "{level:?}:\n{text}");
    }
}

#[test]
fn a_long_division_by_a_byte_keeps_the_speed_family() {
    let text = asm(BYTE, Level::O2);
    assert!(text.contains("udivsi3_fast"), "{text}");
}
