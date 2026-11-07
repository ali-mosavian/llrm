use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, TargetHooks, Visit, combine_for};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

/// `@f(i32 %x, i32 %y)` with `body` returning `%r`, after `visitSelectInst`, on
/// a target that does (`makes`) or does not make `scmp`/`ucmp`.
fn run_on(
    body: &str,
    makes: bool,
) -> String {
    let text = format!("define i32 @f(i32 %x, i32 %y) {{\nb0:\n{body}  ret i32 %r\n}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let hooks = TargetHooks { select_to_cmp: makes, ..Default::default() };
    combine_for(
        context,
        &globals,
        &Default::default(),
        &metadata,
        &DataLayout::default(),
        function,
        visit,
        false,
        &mut declared,
        hooks,
    );
    declared.place(&mut module).unwrap();
    printed(&module)
}

fn run(body: &str) -> String {
    run_on(body, true)
}

/// The pin makes `scmp` on every target; ours only where `Tti` says so (62
/// cells of other targets grew with it on: arm64 sfm16 -Os +500 B, m68k -O2
/// +320 B).
#[test]
fn a_target_that_does_not_make_scmp_keeps_the_select_chain() {
    let body = "  %lt = icmp slt i32 %x, %y\n  %gt = icmp sgt i32 %x, %y\n  %z = zext i1 %gt to i32\n  %r = select i1 %lt, i32 -1, i32 %z\n";
    let out = run_on(body, false);
    assert!(out.contains("select") && !out.contains("scmp"), "{out}");
}

/// Wasm `__eqsf2` kept `select (slt x, y), -1, (zext (sgt x, y))` and a
/// `-1` constant where the pin has `scmp`: 2 B per soft-float compare.
#[test]
fn a_less_than_minus_one_else_greater_than_is_a_signed_three_way_compare() {
    let out = run(
        "  %lt = icmp slt i32 %x, %y\n  %gt = icmp sgt i32 %x, %y\n  %z = zext i1 %gt to i32\n  %r = select i1 %lt, i32 -1, i32 %z\n",
    );
    assert!(out.contains("call i32 @llvm.scmp.i32.i32(i32 %x, i32 %y)") && !out.contains("select"), "{out}");
}

#[test]
fn an_unsigned_compare_makes_ucmp() {
    let out = run(
        "  %lt = icmp ult i32 %x, %y\n  %ne = icmp ne i32 %x, %y\n  %z = zext i1 %ne to i32\n  %r = select i1 %lt, i32 -1, i32 %z\n",
    );
    assert!(out.contains("call i32 @llvm.ucmp.i32.i32(i32 %x, i32 %y)"), "{out}");
}

#[test]
fn a_greater_than_one_else_sext_of_less_than_is_scmp() {
    let out = run(
        "  %gt = icmp sgt i32 %x, %y\n  %lt = icmp slt i32 %x, %y\n  %z = sext i1 %lt to i32\n  %r = select i1 %gt, i32 1, i32 %z\n",
    );
    assert!(out.contains("call i32 @llvm.scmp.i32.i32(i32 %x, i32 %y)"), "{out}");
}

#[test]
fn an_equal_zero_else_a_signed_choice_of_one_and_minus_one_is_scmp() {
    let out = run(
        "  %eq = icmp eq i32 %x, %y\n  %gt = icmp sgt i32 %x, %y\n  %s = select i1 %gt, i32 1, i32 -1\n  %r = select i1 %eq, i32 0, i32 %s\n",
    );
    assert!(out.contains("call i32 @llvm.scmp.i32.i32(i32 %x, i32 %y)"), "{out}");
    let out = run(
        "  %eq = icmp eq i32 %x, %y\n  %lt = icmp ult i32 %x, %y\n  %s = select i1 %lt, i32 -1, i32 1\n  %r = select i1 %eq, i32 0, i32 %s\n",
    );
    assert!(out.contains("call i32 @llvm.ucmp.i32.i32(i32 %x, i32 %y)"), "{out}");
}

#[test]
fn a_constant_bound_with_the_bound_minus_one_is_scmp_of_the_constant() {
    let out = run(
        "  %eq = icmp eq i32 %x, 5\n  %gt = icmp sgt i32 %x, 4\n  %s = select i1 %gt, i32 1, i32 -1\n  %r = select i1 %eq, i32 0, i32 %s\n",
    );
    assert!(out.contains("call i32 @llvm.scmp.i32.i32(i32 %x, i32 5)"), "{out}");
}

/// `zext (sge x, y)` is not `x != y` or the swapped `slt`: the select stays.
#[test]
fn a_greater_or_equal_extension_is_not_a_three_way_compare() {
    let out = run(
        "  %lt = icmp slt i32 %x, %y\n  %ge = icmp sge i32 %x, %y\n  %z = zext i1 %ge to i32\n  %r = select i1 %lt, i32 -1, i32 %z\n",
    );
    assert!(out.contains("select") && !out.contains("scmp"), "{out}");
}
