//! long_shift_by_bits -O1/-O2: `x >> 5` of a 32-bit value was 54 B of inline
//! `srl`/`rr` steps (SDCC 13 B; our -Os call `ld a, #5; jp __lshrsi3`, 5 B),
//! +58 B linked over SDCC's best. A function with a wide constant shift is also
//! selected with the shift routine's call and the module price picks the
//! smaller.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const IR: &str = r#"target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i32 @r5(i32 %x) nounwind { %s = lshr i32 %x, 5
  ret i32 %s }
define i32 @r7(i32 %x) nounwind { %s = lshr i32 %x, 7
  ret i32 %s }
define i32 @l5(i32 %x) nounwind { %s = shl i32 %x, 5
  ret i32 %s }
"#;

#[test]
fn a_wide_constant_shift_is_the_routine_call_where_that_is_smaller_at_o1_and_o2() {
    for level in [Level::O1, Level::O2] {
        for (name, routine) in [("r5", "__lshrsi3"), ("r7", "__lshrsi3"), ("l5", "__ashlsi3")] {
            let asm = text(IR, name, 0, &Request::new(level)).expect("compiles");
            assert!(asm.contains(routine), "{level:?} {name} has no {routine}:\n{asm}");
        }
    }
}
