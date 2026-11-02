//! A zero-extended volatile byte load.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

fn zext_of_volatile_load(width: &str) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
@zero = dso_local global i8 0
define {width} @f() {{
  %a = load volatile i8, ptr @zero
  %z = zext i8 %a to {width}
  ret {width} %z
}}"
    )
}

/// The pin's `tryToFoldExtOfLoad` folds a non-simple load too when the target's
/// extending load is legal, so `zext (load volatile i8)` is one `zextload`:
/// the destination is cleared and the byte moved in. The combiner left the
/// volatile load alone and masked afterwards (`move.b; and.l #255`, 4 B more,
/// on 80% of the seeds the pin is smaller on).
#[test]
fn a_zero_extended_volatile_byte_load_is_cleared_then_moved_not_masked() {
    for width in ["i16", "i32"] {
        for cpu in [Cpu::M68000, Cpu::M68020] {
            for level in [Level::O2, Level::Os] {
                let text = assembly(&zext_of_volatile_load(width), cpu, level).expect("compiles");
                assert!(!text.contains("and.l"), "{width} {cpu:?} {level:?}:\n{text}");
                assert!(text.contains("moveq\t#0") || text.contains("clr.w"), "{width} {cpu:?} {level:?}:\n{text}");
            }
        }
    }
}
