//! A function with `optnone` goes through the pipeline as the pin's
//! `OptNoneInstrumentation` leaves it.

use llrm_arm64::driver::{Level, optimized_at};
use llrm_arm64::machine::Platform;

/// The text of `name` after the arm64 -O2 pipeline, from a module in which
/// `@plain` and `@kept` (`optnone`) are the same -O0-shaped byte copy and
/// `@caller` (`optnone`) calls a one-store `@leaf`.
fn after_o2(name: &str) -> String {
    let mut program =
        llrm_arm64::driver::program(include_str!("data/optnone_backward_copy.ll"), Platform::Linux).expect("parses");
    optimized_at(&mut program, Level::O2).expect("optimizes");
    let text = llrm_mir::print::module(&program.modules[0]);
    let start = text.find(&format!("@{name}(")).unwrap_or_else(|| panic!("no @{name}:\n{text}"));
    let end = text[start..].find("\n}\n").expect("an end") + start;
    text[start..end].to_owned()
}

/// `__attribute__((optnone))` at -O2: the loop of `ocp` was vectorized
/// (`LV: @ocp: vectorized VF 16 IC 2`), as every other function pass ran over
/// it. Its allocas, loads and stores stay, the vector loop is not made, and
/// the same body without the attribute is.
#[test]
fn an_optnone_function_is_left_as_written_and_its_twin_is_not() {
    let plain = after_o2("plain");
    assert!(plain.contains("x i8>") && !plain.contains("alloca"), "the twin is not optimized:\n{plain}");
    let kept = after_o2("kept");
    assert!(
        kept.contains("alloca") && !kept.contains("x i8>") && !kept.contains("phi"),
        "optnone was optimized:\n{kept}"
    );
    assert_eq!(kept.matches("alloca").count(), 3, "{kept}");
}

/// The inliner took `@leaf` into an `optnone` caller: the pin's
/// `getAttributeBasedInliningDecision` refuses ("optnone attribute").
#[test]
fn nothing_is_inlined_into_an_optnone_function() {
    let caller = after_o2("caller");
    assert!(caller.contains("call void @leaf"), "{caller}");
}
