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
    // The inner select has a second user: `replaceInInstruction` leaves it, the
    // fold reads its arm.
    let out = combined(
        "i1 %c, i16 %x, i16 %y, i16 %w, i16 %z",
        "i16",
        "  %s = select i1 %c, i16 %x, i16 %y\n  %t = add i16 %s, %w\n  %r = select i1 %c, i16 %t, i16 %z\n  %u = add i16 %s, 3\n  %v = xor i16 %r, %u\n  ret i16 %v\n",
    );
    assert!(out.contains("add i16 %x, %w"), "{out}");
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

/// `visitSelectInst` alone over a whole `@f`.
fn combined_function(text: &str) -> String {
    let mut module = parsed(&format!("target datalayout = \"e-p:64:64-i64:64\"\n{text}"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// The loop's early exit and its fall-out reach one block; a select there on
/// the loop test's own condition kept that condition, and the loaded bytes it
/// compares, live out of the loop (early_exit_index z80 -O1: 588 B against
/// 541 B for SDCC, three loops re-testing `a[i] != b[i]` after the exit).
/// The pin writes a phi of the two arms.
#[test]
fn a_select_on_the_condition_of_the_dominating_branch_is_a_phi() {
    let out = combined_function(
        "define i16 @f(ptr %p) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %latch ]
  %c = icmp ult i16 %i, 1200
  br i1 %c, label %body, label %exit
body:
  %v = load i8, ptr %p
  %e = icmp eq i8 %v, 0
  br i1 %e, label %latch, label %exit
latch:
  %j = add i16 %i, 1
  br label %head
exit:
  %r = select i1 %c, i16 %i, i16 -1
  ret i16 %r
}
",
    );
    assert!(
        out.contains("phi i16 [ %i, %body ], [ -1, %head ]") || out.contains("phi i16 [ -1, %head ], [ %i, %body ]"),
        "{out}"
    );
    assert!(!out.contains("select"), "{out}");
}

#[test]
fn a_select_on_the_negated_condition_of_the_dominating_branch_is_a_phi() {
    let out = combined_function(
        "define i16 @f(i1 %c, i16 %a, i16 %b) {
entry:
  %n = xor i1 %c, true
  br i1 %n, label %t, label %e
t:
  br label %m
e:
  br label %m
m:
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}
",
    );
    assert!(out.contains("phi i16 [ %b, %t ], [ %a, %e ]") || out.contains("phi i16 [ %a, %e ], [ %b, %t ]"), "{out}");
    assert!(!out.contains("select"), "{out}");
}

/// An edge the branch does not decide (`%x` reaches `%m` from outside both
/// arms) keeps the select.
#[test]
fn a_select_reached_by_an_edge_the_branch_does_not_decide_stays() {
    let out = combined_function(
        "define i16 @f(i1 %c, i1 %d, i16 %a, i16 %b) {
entry:
  br i1 %d, label %x, label %s
s:
  br i1 %c, label %t, label %e
t:
  br label %m
e:
  br label %m
x:
  br label %m
m:
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}
",
    );
    assert!(out.contains("select i1 %c"), "{out}");
}
