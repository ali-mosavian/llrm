//! `IVDescriptors` on loops written to hit one branch of `IVDescriptors.cpp`
//! each; the expected answers are what the pin's code gives by hand-trace (it
//! has no printer).

use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::AssumptionCache;

use super::{Analyses, is_fp_induction_phi, is_induction_phi, is_reduction_phi};
use crate::scev::{Cx, ScalarEvolution};

/// One line per header phi of the only loop of `@f`: `name: Kind` of its
/// reduction (with `se`) or `name: none`; then `name: induction Kind` lines for
/// the phis that are inductions.
fn describe(
    text: &str,
    with_se: bool,
) -> Vec<String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let (_, _, function) = module.functions().find(|(_, _, f)| !f.layout().is_empty()).expect("a body");
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let the_loop = loops.top_level_loops().next().expect("a loop");
    let mut se = ScalarEvolution::new();
    let mut out = Vec::new();
    for inst in function.iter_block(the_loop.get_header()).collect::<Vec<_>>() {
        if function.instruction(inst).opcode != llrm_mir::opcode::Opcode::Phi {
            continue;
        }
        let name = function.instruction(inst).result.and_then(|v| function.value(v).name.clone()).unwrap_or_default();
        let reduction = is_reduction_phi(&cx, with_se.then_some(&mut se), inst, the_loop, &Analyses::NONE);
        match reduction {
            Some(found) => out.push(format!("{name}: {:?}", found.kind)),
            None => out.push(format!("{name}: none")),
        }
        if let Some(found) = is_fp_induction_phi(&cx, inst, the_loop) {
            out.push(format!("{name}: fp-induction {:?}", found.kind));
        }
        if let Some(found) = is_induction_phi(&cx, &mut se, inst, the_loop) {
            let start = matches!(found.start_value, Operand::Value(_) | Operand::Constant(_));
            assert!(start);
            out.push(format!("{name}: induction {:?}", found.kind));
        }
    }
    out
}

/// A counted loop around `body`, `%acc` the phi under test starting at `init`.
fn reduction_loop(
    ty: &str,
    init: &str,
    body: &str,
    next: &str,
) -> String {
    format!(
        "define {ty} @f({ty} %x, i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %acc = phi {ty} [ {init}, %entry ], [ {next}, %loop ]
{body}
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, %n
  br i1 %done, label %exit, label %loop
exit:
  %r = phi {ty} [ {next}, %loop ]
  ret {ty} %r
}}"
    )
}

/// `%x` is loop invariant, so the accumulator is also the induction `{0,+,%x}`.
#[test]
fn an_integer_add_chain_is_an_add_reduction_and_the_counter_an_induction() {
    let ir = reduction_loop("i32", "0", "  %s = add i32 %acc, %x", "%s");
    assert_eq!(describe(&ir, false), ["i: none", "i: induction Int", "acc: Add", "acc: induction Int"]);
}

#[test]
fn each_integer_operation_is_its_own_kind() {
    for (op, kind) in [("sub", "Sub"), ("mul", "Mul"), ("or", "Or"), ("and", "And"), ("xor", "Xor")] {
        let body = format!("  %v = load i32, ptr %p\n  %s = {op} i32 %acc, %v");
        let ir = reduction_loop("i32", "1", &body, "%s")
            .replace("i32 %x, i32 %n", "ptr %p, i32 %n")
            .replace("define i32 @f(i32 %x", "define i32 @f(ptr %p");
        let got = describe(&ir, false);
        assert!(got.contains(&format!("acc: {kind}")), "{op}: {got:?}");
    }
}

#[test]
fn a_sub_chain_with_the_phi_on_the_right_is_not_a_reduction() {
    let body = "  %v = load i32, ptr %p\n  %s = sub i32 %v, %acc";
    let ir = reduction_loop("i32", "1", body, "%s")
        .replace("define i32 @f(i32 %x", "define i32 @f(ptr %p")
        .replace("i32 %x, i32 %n", "ptr %p, i32 %n");
    assert_eq!(acc_of(&ir), "acc: none");
}

fn acc_of(ir: &str) -> String {
    describe(ir, false).into_iter().find(|l| l.starts_with("acc: ") && !l.contains("induction")).unwrap_or_default()
}

#[test]
fn a_chain_of_adds_and_subs_is_a_chain_with_subs() {
    let body = "  %v = load i32, ptr %p\n  %a = add i32 %acc, %v\n  %s = sub i32 %a, %v";
    let ir = reduction_loop("i32", "1", body, "%s")
        .replace("define i32 @f(i32 %x", "define i32 @f(ptr %p")
        .replace("i32 %x, i32 %n", "ptr %p, i32 %n");
    assert_eq!(acc_of(&ir), "acc: AddChainWithSubs");
}

#[test]
fn a_value_using_the_phi_twice_is_not_a_reduction() {
    let ir = reduction_loop("i32", "1", "  %s = add i32 %acc, %acc", "%s");
    assert_eq!(acc_of(&ir), "acc: none");
}

#[test]
fn the_phi_used_after_the_loop_is_not_a_reduction() {
    let ir = reduction_loop("i32", "0", "  %s = add i32 %acc, %x", "%s")
        .replace("ret i32 %r", "%q = add i32 %r, %acc2\n  ret i32 %q")
        .replace("exit:", "exit:\n  %acc2 = phi i32 [ %acc, %loop ]");
    assert_eq!(acc_of(&ir), "acc: none");
}

#[test]
fn smax_intrinsic_is_a_min_max_reduction() {
    let body = "  %v = load i32, ptr %p\n  %s = call i32 @llvm.smax.i32(i32 %acc, i32 %v)";
    let ir = reduction_loop("i32", "0", body, "%s")
        .replace("define i32 @f(i32 %x", "define i32 @f(ptr %p")
        .replace("i32 %x, i32 %n", "ptr %p, i32 %n")
        + "\ndeclare i32 @llvm.smax.i32(i32, i32)\n";
    assert_eq!(acc_of(&ir), "acc: SMax");
}

#[test]
fn a_select_of_the_phi_and_an_invariant_is_an_any_of() {
    let ir = "define i32 @f(ptr %p, i32 %x, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %s, %loop ]
  %v = load i32, ptr %p
  %c = icmp eq i32 %v, 7
  %s = select i1 %c, i32 %acc, i32 %x
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, %n
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %s, %loop ]
  ret i32 %r
}";
    assert_eq!(acc_of(ir), "acc: AnyOf");
}

#[test]
fn a_float_add_chain_is_an_fadd_reduction() {
    let body = "  %s = fadd float %acc, %x";
    let ir = reduction_loop("float", "0.0", body, "%s");
    assert_eq!(acc_of(&ir), "acc: FAdd");
}

#[test]
fn an_fadd_chain_with_an_fsub_is_a_chain_with_subs() {
    let body = "  %a = fadd float %acc, %x\n  %s = fsub float %a, %x";
    let ir = reduction_loop("float", "0.0", body, "%s");
    assert_eq!(acc_of(&ir), "acc: FAddChainWithSubs");
}

#[test]
fn an_fmul_chain_is_an_fmul_reduction() {
    let ir = reduction_loop("float", "1.0", "  %s = fmul float %acc, %x", "%s");
    assert_eq!(acc_of(&ir), "acc: FMul");
}

#[test]
fn an_integer_phi_is_not_matched_by_a_float_kind() {
    let ir = reduction_loop("i32", "0", "  %s = shl i32 %acc, %x", "%s");
    assert_eq!(acc_of(&ir), "acc: none");
}

#[test]
fn a_pointer_phi_is_an_induction_not_a_reduction() {
    let ir = "define void @f(ptr %a, i32 %n) {
entry:
  br label %loop
loop:
  %p = phi ptr [ %a, %entry ], [ %p.next, %loop ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %p.next = getelementptr inbounds i32, ptr %p, i64 1
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret void
}";
    assert_eq!(describe(ir, false)[0..2], ["p: none", "p: induction Ptr"]);
}

#[test]
fn a_float_phi_stepping_by_an_invariant_is_an_fp_induction() {
    let ir = reduction_loop("float", "0.0", "  %s = fadd float %acc, %x", "%s");
    assert!(describe(&ir, false).contains(&"acc: fp-induction Fp".to_string()));
}

/// `isInductionPHI(Phi, L, SE, D)` stops at `!SE->isSCEVable(PhiTy)`: a float
/// phi is no induction there. It was one, so LoopInterchange took a float
/// accumulator for an inner-loop induction.
#[test]
fn the_scalar_evolution_overload_takes_no_float_phi() {
    let ir = reduction_loop("float", "0.0", "  %s = fadd float %acc, %x", "%s");
    assert!(!describe(&ir, false).contains(&"acc: induction Fp".to_string()));
}

#[test]
fn a_phi_that_is_not_affine_is_not_an_induction() {
    let ir = reduction_loop("i32", "1", "  %s = mul i32 %acc, %acc", "%s");
    assert!(!describe(&ir, false).iter().any(|l| l == "acc: induction Int"));
}

/// The select's other arm is loaded in the loop, so the select is no "any of".
#[test]
fn a_select_with_a_loop_varying_arm_is_not_an_any_of() {
    let ir = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %s, %loop ]
  %v = load i32, ptr %p
  %c = icmp eq i32 %v, 7
  %s = select i1 %c, i32 %acc, i32 %v
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, %n
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %s, %loop ]
  ret i32 %r
}";
    assert_eq!(acc_of(ir), "acc: none");
}

/// `@f`'s `%acc` as `is_reduction_phi` describes it with `analyses`: `Kind` or
/// `none`; the demanded bits are computed here when `demanded`.
fn acc_with(
    text: &str,
    demanded: bool,
) -> String {
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let (_, _, function) = module.functions().find(|(_, _, f)| !f.layout().is_empty()).expect("a body");
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let callees = llrm_mir::memory::callees(&module);
    let environment = crate::demandedbits::Environment {
        context: &module.context,
        layout: &layout,
        callees: &callees,
        globals: &module.globals,
        metadata: &module.metadata,
    };
    let bits = crate::demandedbits::DemandedBits::of(&environment, function);
    let analyses = Analyses { demanded: demanded.then_some((&bits, &environment)), value_tracking: false };
    let the_loop = loops.top_level_loops().next().expect("a loop");
    let acc = function
        .iter_block(the_loop.get_header())
        .find(|&inst| {
            function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi
                && function.instruction(inst).result.and_then(|v| function.value(v).name.as_deref()) == Some("acc")
        })
        .expect("%acc");
    match is_reduction_phi(&cx, None, acc, the_loop, &analyses) {
        Some(found) => format!("{:?} {}", found.kind, module.context.types.int_bits(found.recurrence_type).unwrap()),
        None => "none".to_owned(),
    }
}

/// A reduction `acc = (acc & 255) + x` whose result leaves the loop through
/// `& 255`: the `and` says the type is i8.
fn promoted(exit: &str) -> String {
    format!(
        "define i32 @f(i32 %x, i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %s, %loop ]
  %a = and i32 %acc, 255
  %s = add i32 %a, %x
  %i.next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %i.next, %n
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %s, %loop ]
{exit}
}}"
    )
}

/// Symptom: `computeRecurrenceType` without `DemandedBits` answered the type's
/// width, so a reduction the pin accepts as a type-promoted i8 (the exit value
/// read only through `& 255`) was refused: `none` where the pin says `Add`.
#[test]
fn a_reduction_whose_exit_is_read_through_a_byte_mask_is_the_narrow_type_with_demanded_bits() {
    let ir = promoted("  %o = and i32 %r, 255\n  ret i32 %o");
    assert_eq!(acc_with(&ir, false), "none");
    assert_eq!(acc_with(&ir, true), "Add 8");
}

/// The exit value read whole demands every bit: the computed type is i32, not
/// the `and`'s i8, so the speculative look-through fails as it does in the pin.
#[test]
fn a_reduction_whose_exit_is_read_whole_is_not_the_narrow_type() {
    let ir = promoted("  ret i32 %r");
    assert_eq!(acc_with(&ir, true), "none");
}
