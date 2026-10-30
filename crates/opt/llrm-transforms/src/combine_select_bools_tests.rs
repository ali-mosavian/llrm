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

/// `@f(params)` returning `i1` after `visitSelectInst` alone.
fn combined(
    params: &str,
    body: &str,
) -> String {
    let text = format!("define i1 @f({params}) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

const CMPS: &str = "  %a = icmp ugt i16 %x, 2\n  %b = icmp ult i16 %x, 9\n";

/// Where the false arm is poison only if the condition is, `select c, true, x`
/// is `or`.
#[test]
fn a_select_of_true_over_a_value_the_condition_implies_is_an_or() {
    let out = combined("i16 %x", &format!("{CMPS}  %r = select i1 %a, i1 true, i1 %b\n  ret i1 %r\n"));
    assert!(out.contains("or i1 %a, %b") && !out.contains("select"), "{out}");
}

#[test]
fn a_select_of_a_value_the_condition_implies_over_false_is_an_and() {
    let out = combined("i16 %x", &format!("{CMPS}  %r = select i1 %a, i1 %b, i1 false\n  ret i1 %r\n"));
    assert!(out.contains("and i1 %a, %b") && !out.contains("select"), "{out}");
}

/// An arm that may be poison on its own keeps the select: the `or` would leak
/// it.
#[test]
fn a_select_over_an_unrelated_value_is_not_an_or() {
    let out = combined("i1 %a, i1 %y", "  %r = select i1 %a, i1 true, i1 %y\n  ret i1 %r\n");
    assert!(out.contains("select i1 %a, i1 true, i1 %y"), "{out}");
}

#[test]
fn a_false_true_arm_moves_the_negation_to_the_condition() {
    let out = combined("i1 %a, i1 %y", "  %r = select i1 %a, i1 false, i1 %y\n  ret i1 %r\n");
    assert!(out.contains("xor i1 %a, true") && out.contains("select i1 %"), "{out}");
    assert!(out.contains(", i1 %y, i1 false"), "{out}");
}

#[test]
fn a_true_false_arm_moves_the_negation_to_the_condition() {
    let out = combined("i1 %a, i1 %y", "  %r = select i1 %a, i1 %y, i1 true\n  ret i1 %r\n");
    assert!(out.contains("xor i1 %a, true") && out.contains(", i1 true, i1 %y"), "{out}");
}

#[test]
fn de_morgan_turns_a_negated_logical_and_into_a_negated_or() {
    let out = combined(
        "i1 %a, i1 %b",
        "  %na = xor i1 %a, true\n  %nb = xor i1 %b, true\n  %r = select i1 %na, i1 %nb, i1 false\n  ret i1 %r\n",
    );
    assert!(out.contains("select i1 %a, i1 true, i1 %b") && out.contains("xor i1 %"), "{out}");
}

#[test]
fn de_morgan_turns_a_negated_logical_or_into_a_negated_and() {
    let out = combined(
        "i1 %a, i1 %b",
        "  %na = xor i1 %a, true\n  %nb = xor i1 %b, true\n  %r = select i1 %na, i1 true, i1 %nb\n  ret i1 %r\n",
    );
    assert!(out.contains("select i1 %a, i1 %b, i1 false") && out.contains("xor i1 %"), "{out}");
}

#[test]
fn a_select_that_repeats_its_condition_s_select_uses_the_inner_condition() {
    let out = combined(
        "i1 %a, i1 %b",
        "  %s = select i1 %a, i1 true, i1 %b\n  %r = select i1 %s, i1 true, i1 %b\n  ret i1 %r\n",
    );
    assert!(out.matches("select").count() == 1 && out.contains("select i1 %a, i1 true, i1 %b"), "{out}");
    let out = combined(
        "i1 %a, i1 %b",
        "  %s = select i1 %a, i1 %b, i1 false\n  %r = select i1 %s, i1 %b, i1 false\n  ret i1 %r\n",
    );
    assert!(out.matches("select").count() == 1 && out.contains("select i1 %a, i1 %b, i1 false"), "{out}");
}
