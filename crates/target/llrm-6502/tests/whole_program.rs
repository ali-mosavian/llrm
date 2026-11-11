//! `-fwhole-program`: an external function nothing calls is not in the object.
//! `float_return` at -O2 was 5945 B against llvm-mos's 90 (2026-11-11): `scale`
//! and `narrow` stayed and pulled the soft-float they use.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const IR: &str = "define i8 @scale(i8 %x) {\n  %y = add i8 %x, 1\n  ret i8 %y\n}\n\
define i8 @used(i8 %x) {\n  %y = add i8 %x, 2\n  ret i8 %y\n}\n\
define i8 @main(i8 %x) {\n  %v = call i8 @used(i8 %x)\n  ret i8 %v\n}\n";

fn text(flags: &[&str], level: Level) -> String {
    let ir = format!("target datalayout = \"{}\"\n{IR}", datalayout());
    let mut request = Request::new(level);
    request.passes = flags.iter().map(|flag| (*flag).to_owned()).collect();
    assembly(&ir, &request).unwrap_or_else(|why| panic!("{why}"))
}

#[test]
fn an_external_function_nothing_calls_is_dropped() {
    for level in [Level::O0, Level::O2, Level::Os] {
        let kept = text(&[], level);
        let swept = text(&["-fwhole-program"], level);
        assert!(kept.contains("scale:"), "{kept}");
        assert!(!swept.contains("scale:"), "{level:?}: {swept}");
        assert!(swept.contains("main:"), "{swept}");
    }
}

#[test]
fn a_function_main_reaches_stays() {
    let swept = text(&["-fwhole-program"], Level::O0);
    assert!(swept.contains("used:"), "{swept}");
}

#[test]
fn the_later_switch_wins() {
    assert!(text(&["-fwhole-program", "-fno-whole-program"], Level::O2).contains("scale:"));
}
