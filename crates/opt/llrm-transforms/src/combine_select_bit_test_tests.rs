use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

/// `text` after `visitSelectInst` alone.
fn run(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `@f(i32 %x)` returning i32 after `visitSelectInst` alone.
fn combined(body: &str) -> String {
    run(&format!("define i32 @f(i32 %x) {{\nb0:\n{body}  ret i32 %r\n}}\n"))
}

/// Opt: `(x & 2) == 0 ? 6 : 4` is `6 ^ (x & 2)`. Without the fold the select
/// of two constants stayed, and DFAJumpThreading threaded a state machine of
/// them (cases N=256: 1812 -> 2833 instructions) that the pin never threads,
/// because its state is the `xor`.
#[test]
fn two_constants_that_differ_by_the_tested_bit_are_an_xor() {
    let out = combined("  %a = and i32 %x, 2\n  %c = icmp eq i32 %a, 0\n  %r = select i1 %c, i32 6, i32 4\n");
    assert!(out.contains("xor i32 6, %a") && !out.contains("select"), "{out}");
}

/// `icmp ne` swaps the arms.
#[test]
fn a_ne_test_swaps_the_arms() {
    let out = combined("  %a = and i32 %x, 2\n  %c = icmp ne i32 %a, 0\n  %r = select i1 %c, i32 4, i32 6\n");
    assert!(out.contains("xor i32 6, %a") && !out.contains("select"), "{out}");
}

/// A zero arm and a power of two shifts the bit into place.
#[test]
fn a_zero_arm_shifts_the_bit_to_the_other_arm() {
    let out = combined("  %a = and i32 %x, 8\n  %c = icmp eq i32 %a, 0\n  %r = select i1 %c, i32 0, i32 2\n");
    assert!(out.contains("lshr i32 %a, 2") && !out.contains("select"), "{out}");
}

/// Constants that are no sum, difference, or union of the bit stay a select.
#[test]
fn unrelated_constants_stay_a_select() {
    let out = combined("  %a = and i32 %x, 2\n  %c = icmp eq i32 %a, 0\n  %r = select i1 %c, i32 7, i32 4\n");
    assert!(out.contains("select"), "{out}");
}
