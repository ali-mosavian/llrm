use llrm_analysis::scev::expr::{Scev, Ty, Use};
use llrm_analysis::scev::{Cx, Flags, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Operand};
use llrm_mir::valuetracking::AssumptionCache;

use crate::lsr_formula::{Formula, Immediate, MemAccessTy, RegUseTracker};

const LOOP: &str = "define i32 @f(i32 %a, i32 %b) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %next = add i32 %i, 1
  %c = icmp slt i32 %next, %a
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %b
}
";

/// `body` with the SCEV of the invariants `%a` and `%b` and the loop's header.
fn with_loop(body: impl FnOnce(&Cx, &mut ScalarEvolution, Scev, Scev, BlockId)) {
    let module = llrm_mir::parse::module(LOOP).expect("a module");
    let (_, _, function) = module.functions().next().expect("a function");
    let layout = DataLayout::default();
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
    let mut se = ScalarEvolution::new();
    let a = se.get_unknown(&cx, Operand::Value(function.parameters()[0]));
    let b = se.get_unknown(&cx, Operand::Value(function.parameters()[1]));
    let header = loops.top_level_loops().next().expect("a loop").get_header();
    body(&cx, &mut se, a, b, header);
}

fn recurrence(
    se: &mut ScalarEvolution,
    cx: &Cx,
    start: Scev,
    step: Scev,
    header: BlockId,
) -> Scev {
    se.get_add_rec_expr(cx, vec![Use::of(start), Use::of(step)], header, Flags::default()).scev
}

#[test]
fn an_immediate_adds_and_negates_as_unsigned() {
    assert_eq!(Immediate::FIXED_MAX.add_unsigned(Immediate(1)), Immediate::FIXED_MIN);
    assert_eq!(Immediate::FIXED_MIN.sub_unsigned(Immediate(1)), Immediate::FIXED_MAX);
    assert_eq!(Immediate(3).mul_unsigned(-2), Immediate(-6));
    with_loop(|cx, se, _, _, _| {
        let minus_four = Immediate(4).negative_scev(se, cx, Ty::Int(32));
        assert_eq!(se.as_constant(minus_four).unwrap().sext_value(), -4);
    });
}

#[test]
fn a_register_dropped_from_a_use_is_not_used_by_it() {
    let (r, s) = (Scev(1), Scev(2));
    let mut uses = RegUseTracker::default();
    uses.count_register(r, 0);
    uses.count_register(r, 2);
    uses.count_register(s, 2);
    assert_eq!(uses.registers(), [r, s]);
    assert!(uses.is_reg_used_by_uses_other_than(r, 0));
    assert!(uses.is_reg_used_by_uses_other_than(r, 2));
    assert!(!uses.is_reg_used_by_uses_other_than(s, 2));
    assert!(uses.is_reg_used_by_uses_other_than(s, 0));
    uses.drop_register(r, 2);
    assert!(!uses.is_reg_used_by_uses_other_than(r, 0));
    assert!(!uses.is_reg_used_by_uses_other_than(Scev(9), 0), "an uncounted register is used by none");
}

/// Use 2 takes the place of use 0 and the last slot goes.
#[test]
fn the_last_use_takes_the_place_of_a_dropped_one() {
    let r = Scev(1);
    let mut uses = RegUseTracker::default();
    uses.count_register(r, 2);
    uses.swap_and_drop_use(0, 2);
    let used = uses.used_by_indices(r);
    assert!(used.contains(0) && !used.contains(2), "{used:?}");
    assert_eq!(used.len(), 2);
}

#[test]
fn a_scaled_one_is_the_register_and_two_base_registers_scale_the_last() {
    with_loop(|cx, se, a, b, header| {
        let _ = cx;
        let mut one = Formula { scaled_reg: Some(a), scale: 1, ..Formula::default() };
        one.canonicalize(se, header);
        assert_eq!((one.base_regs.clone(), one.scaled_reg, one.scale), (vec![a], None, 0), "1*reg is reg");

        let mut two = Formula { base_regs: vec![a, b], has_base_reg: true, ..Formula::default() };
        assert!(!two.is_canonical(se, header));
        two.canonicalize(se, header);
        assert_eq!((two.base_regs.clone(), two.scaled_reg, two.scale), (vec![a], Some(b), 1));
        assert_eq!(two.num_regs(), 2);
        assert!(two.unscale());
        assert_eq!((two.base_regs.clone(), two.scaled_reg, two.scale), (vec![a, b], None, 0));
    });
}

/// The register with the loop's recurrence is the scaled one: the invariant
/// sum stays in the base registers and combines there.
#[test]
fn the_loops_recurrence_is_the_scaled_register() {
    with_loop(|cx, se, a, b, header| {
        let one = se.get_constant_of_type(cx, Ty::Int(32), 1);
        let ar = recurrence(se, cx, b, one, header);
        let mut f =
            Formula { base_regs: vec![ar], scaled_reg: Some(a), scale: 1, has_base_reg: true, ..Formula::default() };
        assert!(!f.is_canonical(se, header));
        f.canonicalize(se, header);
        assert_eq!((f.base_regs.clone(), f.scaled_reg), (vec![a], Some(ar)));
    });
}

/// The loop's own recurrence properly dominates its header, as a phi does
/// (`computeBlockDisposition`), so it stays one register with its start. A
/// value made inside the loop does not: it is the other register, and the
/// scaled one.
#[test]
fn an_initial_match_keeps_invariants_together_and_a_loop_value_apart() {
    with_loop(|cx, se, a, b, header| {
        let one = se.get_constant_of_type(cx, Ty::Int(32), 1);
        let ar = recurrence(se, cx, b, one, header);
        let mut whole = Formula::default();
        whole.initial_match(se, cx, ar, header);
        assert_eq!((whole.base_regs.clone(), whole.scaled_reg, whole.has_base_reg), (vec![ar], None, true));

        let next =
            cx.function.walk().filter_map(|(_, inst)| cx.function.instruction(inst).result).nth(1).expect("%next");
        let next = se.get_unknown(cx, Operand::Value(next));
        let sum = se.get_add_expr(cx, vec![Use::of(a), Use::of(next)], Flags::default(), 0).scev;
        let mut f = Formula::default();
        f.initial_match(se, cx, sum, header);
        assert_eq!((f.base_regs.clone(), f.scaled_reg, f.scale), (vec![a], Some(next), 1), "{f:?}");
    });
}

#[test]
fn a_formula_counts_down_to_zero_only_with_one_register_and_a_negative_step() {
    with_loop(|cx, se, _, b, header| {
        let (zero, minus_one) =
            (se.get_constant_of_type(cx, Ty::Int(32), 0), se.get_constant_of_type(cx, Ty::Int(32), -1));
        let down = recurrence(se, cx, b, minus_one, header);
        let plus_one = se.get_constant_of_type(cx, Ty::Int(32), 1);
        let up = recurrence(se, cx, b, plus_one, header);
        let one_reg = |reg| Formula { base_regs: vec![reg], has_base_reg: true, ..Formula::default() };
        assert!(one_reg(down).counts_down_to_zero(se));
        assert!(!one_reg(up).counts_down_to_zero(se));
        assert!(!Formula { base_offset: Immediate(4), ..one_reg(down) }.counts_down_to_zero(se));
        assert!(!Formula { scaled_reg: Some(zero), scale: 2, ..one_reg(down) }.counts_down_to_zero(se));
    });
}

#[test]
fn an_unknown_access_has_no_type() {
    let unknown = MemAccessTy::unknown(MemAccessTy::UNKNOWN_ADDRESS_SPACE);
    assert_eq!(unknown.mem_ty, None);
    assert_ne!(unknown, MemAccessTy::unknown(0));
}
