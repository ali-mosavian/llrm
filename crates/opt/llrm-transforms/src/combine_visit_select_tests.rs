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

/// `@f(params)` returning `ret` after `visitSelectInst` alone.
fn combined(
    params: &str,
    ret: &str,
    body: &str,
) -> String {
    let text = format!(
        "target datalayout = \"e-p:64:64-i64:64\"\ndeclare i16 @llvm.ctpop.i16(i16)\ndefine {ret} @f({params}) {{\nb0:\n{body}}}\n"
    );
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

#[test]
fn a_select_of_one_and_zero_is_a_zero_extension() {
    let out = combined("i1 %c", "i16", "  %r = select i1 %c, i16 1, i16 0\n  ret i16 %r\n");
    assert!(out.contains("%0 = zext i1 %c to i16") || out.contains("zext i1 %c to i16"), "{out}");
    assert!(!out.contains("select"), "{out}");
}

#[test]
fn a_select_of_all_ones_and_zero_is_a_sign_extension() {
    let out = combined("i1 %c", "i16", "  %r = select i1 %c, i16 -1, i16 0\n  ret i16 %r\n");
    assert!(out.contains("sext i1 %c to i16") && !out.contains("select"), "{out}");
}

#[test]
fn a_select_of_zero_and_one_extends_the_negated_condition() {
    let out = combined("i1 %c", "i16", "  %r = select i1 %c, i16 0, i16 1\n  ret i16 %r\n");
    assert!(out.contains("xor i1 %c, true") && out.contains("zext i1 %"), "{out}");
}

#[test]
fn other_constants_stay_a_select() {
    let body = "  %r = select i1 %c, i16 4, i16 0\n  ret i16 %r\n";
    let out = combined("i1 %c", "i16", body);
    assert!(out.contains("select i1 %c, i16 4, i16 0"), "{out}");
}

#[test]
fn a_select_of_a_gep_and_its_base_selects_the_index() {
    let out = combined(
        "i1 %c, ptr %p, i64 %i",
        "ptr",
        "  %g = getelementptr i32, ptr %p, i64 %i\n  %r = select i1 %c, ptr %g, ptr %p\n  ret ptr %r\n",
    );
    assert!(out.contains("select i1 %c, i64 %i, i64 0") && out.contains("getelementptr i32, ptr %p, i64 %"), "{out}");
}

#[test]
fn a_nested_select_on_the_false_arm_is_a_logical_and() {
    let out = combined(
        "i1 %c, i1 %d, i16 %a, i16 %b",
        "i16",
        "  %s = select i1 %d, i16 %a, i16 %b\n  %r = select i1 %c, i16 %s, i16 %b\n  ret i16 %r\n",
    );
    assert!(out.contains("select i1 %c, i1 %d, i1 false") && out.contains("i16 %a, i16 %b"), "{out}");
}

#[test]
fn a_nested_select_on_the_true_arm_is_a_logical_or() {
    let out = combined(
        "i1 %c, i1 %d, i16 %a, i16 %b",
        "i16",
        "  %s = select i1 %d, i16 %a, i16 %b\n  %r = select i1 %c, i16 %a, i16 %s\n  ret i16 %r\n",
    );
    assert!(out.contains("select i1 %c, i1 true, i1 %d"), "{out}");
}

#[test]
fn a_negated_condition_swaps_the_arms() {
    let out = combined(
        "i1 %c, i16 %a, i16 %b",
        "i16",
        "  %n = xor i1 %c, true\n  %r = select i1 %n, i16 %a, i16 %b\n  ret i16 %r\n",
    );
    assert!(out.contains("select i1 %c, i16 %b, i16 %a"), "{out}");
}

#[test]
fn a_binop_between_selects_on_one_condition_drops_the_inner_select() {
    let out = combined(
        "i1 %c, i16 %x, i16 %y, i16 %w, i16 %z",
        "i16",
        "  %s = select i1 %c, i16 %x, i16 %y\n  %t = add i16 %s, %w\n  %r = select i1 %c, i16 %t, i16 %z\n  ret i16 %r\n",
    );
    assert!(out.contains("add i16 %x, %w") && !out.contains("select i1 %c, i16 %x, i16 %y"), "{out}");
}

#[test]
fn the_ctpop_guard_is_dropped() {
    let out = combined(
        "i16 %x",
        "i16",
        "  %c = icmp ult i16 %x, 2\n  %p = call i16 @llvm.ctpop.i16(i16 %x)\n  %r = select i1 %c, i16 %x, i16 %p\n  ret i16 %r\n",
    );
    assert!(out.contains("ret i16 %p") && !out.contains("select"), "{out}");
}

#[test]
fn a_nested_select_the_outer_condition_decides_is_its_arm() {
    let out = combined(
        "i1 %c, i16 %a, i16 %b",
        "i16",
        "  %s = select i1 %c, i16 %a, i16 %b\n  %r = select i1 %c, i16 %s, i16 7\n  ret i16 %r\n",
    );
    assert!(out.contains("select i1 %c, i16 %a, i16 7"), "{out}");
}
