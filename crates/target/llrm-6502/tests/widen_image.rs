//! 6502-WIDEN-IMAGE: at -Os a product of two words widened to 32 bits is a
//! `__umulhisi3` call only where the module's bytes, the routines linked
//! counted, do not rise. A per-product rule grew idioms +93 B and
//! int64_division +49 B (the routine linked for four sites that do not pay it);
//! no rule at all left loop_versioning's 16 x 8 product a `__mulsi3` call: 882
//! B, 1.66M clocks against 865 B, 586k (2026-11-12).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn body(ir: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    text.split(".segment").next().unwrap().to_owned()
}

/// The only product of the module: the widening routines replace `__mulsi3` in
/// the image.
#[test]
fn a_lone_widening_product_is_not_a_long_multiply() {
    let text = body(
        "define i32 @f(i16 %a, i8 %b) {\n  %x = sext i16 %a to i32\n  %y = zext i8 %b to i32\n  %m = mul i32 %x, %y\n  ret i32 %m\n}\n",
    );
    assert!(!text.contains("__mulsi3"), "{text}");
}

/// A long multiply stays in the module, so the widening routine would be linked
/// beside it for one site that saves fewer bytes than the routine costs.
#[test]
fn a_widening_product_beside_a_long_multiply_stays_a_long_multiply() {
    let text = body(
        "define i32 @f(i16 %a, i16 %b) {\n  %x = zext i16 %a to i32\n  %y = zext i16 %b to i32\n  %m = mul i32 %x, %y\n  ret i32 %m\n}\n\
         define i32 @g(i32 %a, i32 %b) {\n  %m = mul i32 %a, %b\n  ret i32 %m\n}\n",
    );
    assert!(!text.contains("__umulhisi3"), "{text}");
}
