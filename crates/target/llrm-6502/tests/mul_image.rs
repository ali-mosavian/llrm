//! A multiply by a constant in a function tuned for size is a call of
//! `__mulsi3` only while the routine is linked for another reason.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(functions: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{functions}", datalayout());
    assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"))
}

const BY_THREE: &str = "define i32 @by_three(i32 %x) minsize nounwind {\n  %r = mul i32 %x, 3\n  ret i32 %r\n}\n";
const BY_ARGUMENT: &str = "define i32 @by_argument(i32 %x, i32 %y) minsize nounwind {\n  %r = mul i32 %x, %y\n  ret i32 %r\n}\n";

/// `state * 3` at -Os called the 85-byte routine and its 25-byte tail for
/// 3.34 M clocks where the 82-byte shifts and add take a tenth of that
/// (licm_graph_callee_footprint: 467 B against gcc's 296).
#[test]
fn a_constant_multiply_alone_is_shifts_and_an_add() {
    let text = asm(BY_THREE);
    assert!(!text.contains("__mulsi3"), "{text}");
}

/// The routine is linked anyway: the longer expansion would only add bytes.
#[test]
fn a_constant_multiply_beside_a_variable_one_stays_a_call() {
    let text = asm(&format!("{BY_THREE}{BY_ARGUMENT}"));
    let by_three = &text[text.find("by_three:").unwrap()..text.find("by_argument:").unwrap_or(text.len())];
    assert!(by_three.contains("jsr __mulsi3"), "{text}");
}

/// The price is read from the runtime, for any routine: a long division and a
/// variable shift are priced the same way as the multiply.
#[test]
fn a_routine_is_priced_from_the_runtime() {
    let price = llrm_6502::mulimage::library_bytes;
    assert!((100..=120).contains(&price("__mulsi3")), "{}", price("__mulsi3"));
    assert!(price("__udivsi3") > price("__mulsi3"), "{} {}", price("__udivsi3"), price("__mulsi3"));
    assert!(price("__ashlsi3") > 0);
}
