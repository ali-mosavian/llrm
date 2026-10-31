use llrm_analysis::scev::expr::{Scev, Ty, Use as Operand32};
use llrm_analysis::scev::{Cx, Flags, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::tti::{
    AddressingMode, ArithOp, CastKind, CmpSel, Cost as TtiCost, CostKind, ImmUser, LsrCost, MemoryOp, OperandInfo, Tti,
    Ty as TtiTy,
};
use llrm_mir::valuetracking::AssumptionCache;
use llrm_support::hash::HashSet;
use llrm_x86::tti::{RealMode, X86};

use crate::lsr_formula::{Formula, Immediate, MemAccessTy, RegUseTracker};
use crate::lsr_use::*;

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

/// `body` with the SCEV of the invariants `%a` and `%b`, the loop's header and
/// its recurrence `{0,+,1}`.
fn with_loop(body: impl FnOnce(&Cx, &mut ScalarEvolution, Scev, Scev, BlockId, Scev)) {
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
    let (zero, one) = (se.get_constant_of_type(&cx, Ty::Int(32), 0), se.get_constant_of_type(&cx, Ty::Int(32), 1));
    let ar = se.get_add_rec_expr(&cx, vec![Operand32::of(zero), Operand32::of(one)], header, Flags::default()).scev;
    body(&cx, &mut se, a, b, header, ar);
}

fn rated(
    cx: &Cx,
    se: &mut ScalarEvolution,
    header: BlockId,
    lu: &Use,
    f: &Formula,
    visited: &HashSet<Scev>,
) -> Cost {
    let tti = X86::of_cpu("core2").unwrap();
    let mut rating = Rating { se, cx, tti: &tti, l: header, hardware_loop_profitable: false, as_written: false };
    let (mut cost, mut regs) = (Cost::new(), HashSet::default());
    cost.rate_formula(&mut rating, f, &mut regs, visited, lu, None);
    cost
}

fn basic_use(offsets: &[i64]) -> Use {
    use_of(Kind::Basic, offsets)
}

/// A use of `kind` with a fixup at each offset: a use with none has an empty
/// range (min above max) and folds nothing.
fn use_of(
    kind: Kind,
    offsets: &[i64],
) -> Use {
    let mut lu = Use::new(kind, MemAccessTy::unknown(0));
    for &offset in offsets {
        let mut fixup = Fixup::new(llrm_mir::module::InstId(0), Operand::Value(llrm_mir::module::ValueId(0)));
        fixup.offset = Immediate(offset);
        lu.push_fixup(fixup);
    }
    lu
}

/// The loop's own recurrence is one register and one recurrence, and a start of
/// 0 needs no setup: `1 instruction, 1 reg, with addrec cost 1`.
#[test]
fn an_induction_variable_costs_a_register_and_a_recurrence() {
    with_loop(|cx, se, _, _, header, ar| {
        let f = Formula { base_regs: vec![ar], has_base_reg: true, ..Formula::default() };
        let cost = rated(cx, se, header, &basic_use(&[0]), &f, &HashSet::default());
        assert_eq!(cost.c, LsrCost { insns: 1, num_regs: 1, add_rec_cost: 1, ..LsrCost::default() });
    });
}

/// An unknown invariant is one register and one setup instruction; `a + b` is
/// canonically a base and a scaled register, and an address folds the pair, so
/// there is no add.
#[test]
fn two_invariants_in_an_address_fold_to_no_add() {
    with_loop(|cx, se, a, b, header, _| {
        let mut f = Formula { base_regs: vec![a, b], has_base_reg: true, ..Formula::default() };
        f.canonicalize(se, header);
        let lu = use_of(Kind::Address, &[0]);
        let cost = rated(cx, se, header, &lu, &f, &HashSet::default());
        // X86 prices any scale in an address at one.
        assert_eq!(cost.c, LsrCost { num_regs: 2, setup_cost: 2, scale_cost: 1, ..LsrCost::default() });
        // A basic use cannot fold the second register: one base add.
        let basic = use_of(Kind::Basic, &[0]);
        let cost = rated(cx, se, header, &basic, &f, &HashSet::default());
        assert_eq!(cost.c, LsrCost { insns: 1, num_regs: 2, num_base_adds: 1, setup_cost: 2, ..LsrCost::default() });
    });
}

/// DEVIATE LU2: `a + b + 8` for a Basic use is one `lea` on x86, so the offset
/// beside the two registers costs no add of its own: before, it was charged
/// one, and PLASMA's zero-ending counter lost the tie to `cmp bx,642` (4
/// against 4).
#[test]
fn an_offset_beside_two_registers_is_the_displacement_of_the_lea() {
    with_loop(|cx, se, a, b, header, _| {
        let mut f = Formula { base_regs: vec![a, b], has_base_reg: true, ..Formula::default() };
        f.unfolded_offset = Immediate(8);
        f.canonicalize(se, header);
        let basic = use_of(Kind::Basic, &[0]);
        let cost = rated(cx, se, header, &basic, &f, &HashSet::default());
        assert_eq!(cost.c.num_base_adds, 1, "the add of the registers only");
        // One register and an offset is an add of the offset, as before.
        let one =
            Formula { base_regs: vec![a], has_base_reg: true, unfolded_offset: Immediate(8), ..Formula::default() };
        let cost = rated(cx, se, header, &basic, &one, &HashSet::default());
        assert_eq!(cost.c.num_base_adds, 1);
    });
}

/// A register a chosen formula already holds is not counted twice, and one that
/// the caller marked visited makes the formula lose.
#[test]
fn a_visited_register_loses_and_a_chosen_one_is_free() {
    with_loop(|cx, se, a, _, header, _| {
        let f = Formula { base_regs: vec![a], has_base_reg: true, ..Formula::default() };
        let lu = basic_use(&[0]);
        let visited: HashSet<Scev> = [a].into_iter().collect();
        assert!(rated(cx, se, header, &lu, &f, &visited).is_loser());

        let tti = X86::of_cpu("core2").unwrap();
        let mut rating = Rating { se, cx, tti: &tti, l: header, hardware_loop_profitable: false, as_written: false };
        let (mut cost, mut regs) = (Cost::new(), [a].into_iter().collect::<HashSet<Scev>>());
        cost.rate_formula(&mut rating, &f, &mut regs, &HashSet::default(), &lu, None);
        assert_eq!(cost.c, LsrCost::default(), "a chosen register costs nothing more");
    });
}

/// Each fixup's offset adds the bits it takes, signed: 4 is `100` and a sign
/// bit, -1 is one bit; a global is 64.
#[test]
fn immediates_cost_their_significant_bits() {
    with_loop(|cx, se, a, _, header, _| {
        let f = Formula { base_regs: vec![a], has_base_reg: true, ..Formula::default() };
        let cost = rated(cx, se, header, &basic_use(&[4, -1, 0]), &f, &HashSet::default());
        assert_eq!(cost.c.imm_cost, 4 + 1);
    });
}

/// Fewer instructions win before registers do; then the target's order.
#[test]
fn fewer_instructions_win_before_fewer_registers() {
    let tti = X86::of_cpu("core2").unwrap();
    let (mut few, mut many) = (Cost::new(), Cost::new());
    few.c = LsrCost { insns: 1, num_regs: 5, ..LsrCost::default() };
    many.c = LsrCost { insns: 2, num_regs: 1, ..LsrCost::default() };
    assert!(few.is_less(&tti, &many) && !many.is_less(&tti, &few));
    few.lose();
    assert!(few.is_loser() && many.is_less(&tti, &few));
}

/// `Cost::isLess` ranks by instructions first only when `-lsr-insns-cost` is on
/// the command line, which nothing passes: a target with the default order
/// (registers first) kept a solution of 2 instructions and 4 registers over the
/// pin's 3 and 3, and a copy loop got a shared index instead of two pointers.
#[test]
fn the_default_order_ranks_registers_before_instructions() {
    let tti = llrm_mir::tti::Basic;
    let (mut few, mut many) = (Cost::new(), Cost::new());
    few.c = LsrCost { insns: 2, num_regs: 4, ..LsrCost::default() };
    many.c = LsrCost { insns: 3, num_regs: 3, ..LsrCost::default() };
    assert!(many.is_less(&tti, &few) && !few.is_less(&tti, &many));
}

#[test]
fn what_an_address_mode_folds_by_kind() {
    let tti = X86::of_cpu("core2").unwrap();
    let fold = |kind, gv, offset, base, scale| {
        is_am_completely_folded_at(&tti, kind, MemAccessTy::unknown(0), gv, Immediate(offset), base, scale)
    };
    assert!(fold(Kind::Address, false, 8, true, 4));
    assert!(!fold(Kind::Address, false, 1 << 40, true, 1), "a displacement is 32 bits");
    assert!(fold(Kind::Basic, false, 0, true, 0) && !fold(Kind::Basic, false, 0, true, 1));
    assert!(fold(Kind::Special, false, 0, true, -1) && !fold(Kind::Special, true, 0, true, 0));
    assert!(fold(Kind::ICmpZero, false, 0, true, -1), "a -1 scale goes to the other side");
    assert!(!fold(Kind::ICmpZero, false, 0, true, 2));
    assert!(!fold(Kind::ICmpZero, false, 5, true, -1), "two parts and an immediate");
    assert!(fold(Kind::ICmpZero, false, 5, true, 0), "cmp reg, -5");
}

/// An offset range whose ends overflow with the base is not folded.
#[test]
fn offsets_that_wrap_with_the_base_do_not_fold() {
    let tti = X86::of_cpu("core2").unwrap();
    let range = |min, max, base| {
        is_am_completely_folded_between(
            &tti,
            Immediate(min),
            Immediate(max),
            Kind::Address,
            MemAccessTy::unknown(0),
            false,
            Immediate(base),
            true,
            1,
        )
    };
    assert!(range(-8, 8, 100));
    assert!(!range(0, 8, i64::MAX - 4));
    assert!(!range(i64::MIN, 0, -8));
    assert!(!range(0, 1 << 32, 0), "the far end is past 32 bits");
}

/// In an address X86 prices a scale at one, whatever it is, and none at
/// nothing; where the use does not fold it (a basic use takes no scale), a
/// scale of 1 is free and any other costs one.
#[test]
fn the_cost_of_a_scale_follows_whether_it_folds() {
    with_loop(|_, se, a, b, header, _| {
        let tti = X86::of_cpu("core2").unwrap();
        let lu = use_of(Kind::Address, &[0]);
        let at = |scale| {
            let f =
                Formula { base_regs: vec![a], scaled_reg: Some(b), scale, has_base_reg: true, ..Formula::default() };
            scaling_factor_cost(&tti, se, &lu, &f, header)
        };
        assert_eq!(at(2), TtiCost::of(1));
        assert_eq!(at(1), TtiCost::of(1));
        let basic = use_of(Kind::Basic, &[0]);
        let in_basic = |scale| {
            let f =
                Formula { base_regs: vec![a], scaled_reg: Some(b), scale, has_base_reg: true, ..Formula::default() };
            scaling_factor_cost(&tti, se, &basic, &f, header)
        };
        assert_eq!((in_basic(1), in_basic(2)), (TtiCost::FREE, TtiCost::of(1)));
        let none = Formula { base_regs: vec![a], has_base_reg: true, ..Formula::default() };
        assert_eq!(scaling_factor_cost(&tti, se, &lu, &none, header), TtiCost::FREE);
    });
}

/// A formula with the same registers in another order is the same formula; a
/// rigid use keeps its first; deleting one moves the last into its place; and
/// the tracker is told of registers no formula uses any more.
#[test]
fn a_use_keeps_one_formula_per_set_of_registers() {
    with_loop(|_, se, a, b, header, _| {
        let mut lu = Use::new(Kind::Address, MemAccessTy::unknown(0));
        let mut ab =
            Formula { base_regs: vec![a], scaled_reg: Some(b), scale: 1, has_base_reg: true, ..Formula::default() };
        let ba = Formula { base_regs: vec![b], scaled_reg: Some(a), scale: 1, has_base_reg: true, ..ab.clone() };
        assert!(lu.insert_formula(se, &ab, header));
        assert!(lu.has_formula_with_same_regs(&ba));
        assert!(!lu.insert_formula(se, &ba, header));
        assert_eq!(lu.not_selected_probability(a), 0.0);

        let only_a = Formula { base_regs: vec![a], has_base_reg: true, ..Formula::default() };
        assert!(lu.insert_formula(se, &only_a, header));
        assert_eq!(lu.not_selected_probability(b), 0.5);
        let mut reg_uses = RegUseTracker::default();
        for &reg in &lu.regs {
            reg_uses.count_register(reg, 0);
        }
        lu.delete_formula(0);
        assert_eq!(lu.formulae, [only_a.clone()], "the last took its place");
        lu.recompute_regs(0, &mut reg_uses);
        assert!(lu.regs.contains(&a) && !lu.regs.contains(&b));
        assert!(!reg_uses.used_by_indices(b).contains(0) && reg_uses.used_by_indices(a).contains(0));

        let mut rigid = Use::new(Kind::Basic, MemAccessTy::unknown(0));
        rigid.rigid_formula = true;
        ab.scale = 1;
        assert!(rigid.insert_formula(se, &only_a, header));
        assert!(!rigid.insert_formula(se, &ab, header), "a rigid use takes one formula");
    });
}

/// The offsets of the fixups bound the range an address mode must hold.
#[test]
fn fixups_widen_the_offset_range() {
    let lu = basic_use(&[8, -4, 0]);
    assert_eq!((lu.min_offset, lu.max_offset), (Immediate(-4), Immediate(8)));
    assert_eq!(lu.fixups.len(), 3);
}

/// A user inside the loop is not outside it; the `ret` in the exit block is; a
/// phi uses a value where it comes from.
#[test]
fn a_fixup_is_outside_the_loop_where_its_user_is() {
    with_loop(|cx, _, _, _, header, _| {
        let user = |opcode| {
            cx.function
                .walk()
                .map(|(_, inst)| inst)
                .find(|&inst| cx.function.instruction(inst).opcode == opcode)
                .unwrap()
        };
        let ret = Fixup::new(user(Opcode::Ret), Operand::Value(cx.function.parameters()[1]));
        let add = Fixup::new(user(Opcode::Binary(BinaryOp::Add)), Operand::Value(cx.function.parameters()[1]));
        assert!(ret.is_use_fully_outside_loop(cx, header));
        assert!(!add.is_use_fully_outside_loop(cx, header));
        // `%next` flows into `%i` from the loop's own block.
        let next = cx.function.instruction(user(Opcode::Binary(BinaryOp::Add))).result.unwrap();
        let phi = Fixup::new(user(Opcode::Phi), Operand::Value(next));
        assert!(!phi.is_use_fully_outside_loop(cx, header));
    });
}

/// DEVIATE LU1: real mode pays the prefixed index once for each scaled
/// register. Six uses of `base + 2 * i` cost 2 instructions beyond the
/// recurrence, not 12 (particle's ADVANCE took six pointer recurrences for want
/// of that).
#[test]
fn a_scaled_index_costs_its_instructions_once_for_each_scaled_register() {
    with_loop(|cx, se, a, b, header, ar| {
        let real = RealMode {
            scale_costs: vec![(1, 0), (2, 2)],
            registers: 6,
            scaled_index_insns: 2,
            huge_space: None,
            inline_size_guard: false,
            float_code_size_percent: 100,
            unroll_size_few_trips: (0, 0),
            recursive_inlining: None,
        };
        let real = X86::of_cpu("i486").unwrap().in_real_mode(real);
        let flat = X86::of_cpu("i486").unwrap();
        let lu = use_of(Kind::Address, &[0]);
        let scaled = |base| {
            let mut f = Formula {
                base_regs: vec![base],
                scaled_reg: Some(ar),
                scale: 2,
                has_base_reg: true,
                ..Formula::default()
            };
            f.canonicalize(se, header);
            f
        };
        let (fa, fb) = (scaled(a), scaled(b));
        for (tti, first, both) in [(&real as &dyn llrm_mir::tti::Tti, 3, 3), (&flat, 1, 1)] {
            let mut rating = Rating { se, cx, tti, l: header, hardware_loop_profitable: false, as_written: false };
            let (mut cost, mut regs, visited) = (Cost::new(), HashSet::default(), HashSet::default());
            cost.rate_formula(&mut rating, &fa, &mut regs, &visited, &lu, None);
            assert_eq!(cost.c.insns, first, "the recurrence and the prefixed index");
            cost.rate_formula(&mut rating, &fb, &mut regs, &visited, &lu, None);
            assert_eq!(cost.c.insns, both, "the same scaled register is not paid for again");
        }
    });
}

/// A target that favors post-indexed addressing and has `(An)+` for every type,
/// on the basic cost model.
struct PostInc {
    favored: bool,
}

impl Tti for PostInc {
    fn cmp_sel_cost(
        &self,
        _: CmpSel,
        _: TtiTy,
        _: CostKind,
    ) -> TtiCost {
        TtiCost::BASIC
    }

    fn memory_op_cost(
        &self,
        _: MemoryOp,
        _: TtiTy,
        _: CostKind,
        _: bool,
    ) -> TtiCost {
        TtiCost::BASIC
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> TtiCost {
        if phi && kind != CostKind::RecipThroughput { TtiCost::FREE } else { TtiCost::BASIC }
    }

    fn int_imm_cost(
        &self,
        _: i128,
        _: u32,
    ) -> TtiCost {
        TtiCost::BASIC
    }

    fn int_imm_cost_inst(
        &self,
        _: ImmUser,
        _: u32,
        _: i128,
        _: u32,
    ) -> TtiCost {
        TtiCost::FREE
    }

    fn is_truncate_free(
        &self,
        _: TtiTy,
        _: TtiTy,
    ) -> bool {
        false
    }

    /// `d16(An)` and `(An)`.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        !base_gv && has_base_reg && scale == 0 && i16::try_from(base_offset).is_ok()
    }

    fn cast_cost(
        &self,
        _: CastKind,
        _: TtiTy,
        _: TtiTy,
        _: CostKind,
    ) -> TtiCost {
        TtiCost::BASIC
    }

    fn arithmetic_cost(
        &self,
        _: ArithOp,
        _: TtiTy,
        _: CostKind,
        _: OperandInfo,
        _: OperandInfo,
    ) -> TtiCost {
        TtiCost::BASIC
    }

    fn preferred_addressing_mode(&self) -> AddressingMode {
        if self.favored { AddressingMode::PostIndexed } else { AddressingMode::None }
    }

    fn is_post_inc_load_legal(
        &self,
        _ty: TtiTy,
    ) -> bool {
        true
    }

    fn is_post_inc_store_legal(
        &self,
        _ty: TtiTy,
    ) -> bool {
        true
    }
}

/// A pointer `{%a,+,1}` that a store walks is the access's own increment where
/// the target favors post-indexed addressing and has `(An)+` for the access:
/// it costs no recurrence. Without that, or without `(An)+`, it costs one.
#[test]
fn a_pointer_stepping_by_a_constant_is_free_where_post_indexed_access_is_favored() {
    with_loop(|cx, se, a, _, header, _| {
        let one = se.get_constant_of_type(cx, Ty::Int(32), 1);
        let walk = se.get_add_rec_expr(cx, vec![Operand32::of(a), Operand32::of(one)], header, Flags::default()).scev;
        let f = Formula { base_regs: vec![walk], has_base_reg: true, ..Formula::default() };
        let lu = use_of(Kind::Address, &[0]);
        let rate = |tti: &dyn Tti, se: &mut ScalarEvolution| {
            let mut rating = Rating { se, cx, tti, l: header, hardware_loop_profitable: false, as_written: false };
            let (mut cost, mut regs) = (Cost::new(), HashSet::default());
            cost.rate_formula(&mut rating, &f, &mut regs, &HashSet::default(), &lu, None);
            cost.c.add_rec_cost
        };
        assert_eq!(rate(&PostInc { favored: false }, se), 1, "premise: the recurrence costs one");
        assert_eq!(rate(&PostInc { favored: true }, se), 0);
    });
}
