//! A word reloaded for one byte of it loads that byte.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `z` kept in the frame across a call and read only as a byte was reloaded as
/// a word (`ld e, -6(ix)` / `ld d, -5(ix)`, 3 B and 19 T more than the byte
/// alone: wide_shift_by_one, rotate_small, fixed_point).
#[test]
fn a_word_read_in_one_byte_is_reloaded_as_that_byte() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
declare void @ext(ptr)\n\
define i8 @f(i16 %x, i16 %y, i16 %z) {\n  %buf = alloca [4 x i8], align 1\n  call void @ext(ptr %buf)\n\
  %a = trunc i16 %x to i8\n  %b = trunc i16 %y to i8\n  %c = trunc i16 %z to i8\n\
  %s = add i8 %a, %b\n  %r = sub i8 %s, %c\n  ret i8 %r\n}\n";
    for level in [Level::O1, Level::O2, Level::Os] {
        let asm = text(ir, "f", 0, &Request::new(level)).expect("compiles");
        let loads = asm
            .lines()
            .filter(|line| line.trim_start().starts_with("ld ") && line.contains("(ix)") && !line.contains("(ix),"))
            .count();
        assert_eq!(loads, 1, "{level:?}: {loads} frame loads:\n{asm}");
    }
}
