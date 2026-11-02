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

fn volatile_loads_ir(body: &str) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
@vol = dso_local global i8 0
@sink = dso_local global i8 0
define i32 @f() {{
{body}
}}"
    )
}

/// A volatile access happens as often and as wide as the source says: folding
/// the zero-extend into the load must not read it twice (once for the widened
/// copy, once for the byte another user keeps) nor read a word.
#[test]
fn a_volatile_load_keeps_its_count_and_width_when_the_extend_folds() {
    // one load, read both widened and as a byte
    let shared = volatile_loads_ir(
        "  %a = load volatile i8, ptr @vol
  %z = zext i8 %a to i32
  %c = add i8 %a, 1
  store volatile i8 %c, ptr @sink
  ret i32 %z",
    );
    // two loads, each widened
    let twice = volatile_loads_ir(
        "  %a = load volatile i8, ptr @vol
  %b = load volatile i8, ptr @vol
  %x = zext i8 %a to i32
  %y = zext i8 %b to i32
  %s = add i32 %x, %y
  ret i32 %s",
    );
    for (name, ir, loads) in [("shared", shared, 1), ("twice", twice, 2)] {
        for cpu in [Cpu::M68000, Cpu::M68020] {
            for level in [Level::O2, Level::Os] {
                let text = assembly(&ir, cpu, level).expect("compiles");
                // a read is a move whose source is memory; the address may be
                // in a register
                let reads = text.lines().filter(|line| line.trim_start().starts_with("move.b\t(")).count();
                assert_eq!(reads, loads, "{name} {cpu:?} {level:?}: the volatile is read {reads} times:\n{text}");
                let words = text
                    .lines()
                    .filter(|line| ["move.w\t(", "move.l\t("].iter().any(|m| line.trim_start().starts_with(m)))
                    .count();
                assert_eq!(words, 0, "{name} {cpu:?} {level:?}: a volatile byte is read wider than a byte:\n{text}");
            }
        }
    }
}
