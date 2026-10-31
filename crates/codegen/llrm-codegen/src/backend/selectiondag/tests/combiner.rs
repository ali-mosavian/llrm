use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: vt,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(vt, chain, ptr, info)
}

fn mul_by(
    dag: &mut SelectionDAG,
    x: SDValue,
    c: u128,
) -> SDValue {
    let c = dag.get_constant(c, EVT::I32);
    dag.get_node(ISD::Mul, EVT::I32, &[x, c])
}

fn generic(dag: &mut SelectionDAG) {
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
}

fn x86(
    dag: &mut SelectionDAG,
    level: CombineLevel,
) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    DagCombiner::new(
        dag,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
        level,
    )
    .with_tli(&tli)
    .run();
}

/// `x * 1` is `x` and `x * 8` is `x << 3`: `getNode` keeps the multiply
/// (A0: identity folds are the combiner's), so a matcher sees an `imul`.
#[test]
fn multiply_by_one_and_by_a_power_of_two() {
    let mut dag = dag();
    let x = load(&mut dag, EVT::I32, 1);
    let one = mul_by(&mut dag, x, 1);
    dag.set_root(one);
    generic(&mut dag);
    assert_eq!(dag.get_root(), x);

    let eight = mul_by(&mut dag, x, 8);
    dag.set_root(eight);
    generic(&mut dag);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Shl);
    let amount = dag.operand(root, 1);
    assert_eq!((dag.constant(amount), dag.value_type(amount)), (Some(3), EVT::I8));
}

/// `x * -8` is `0 - (x << 3)`.
#[test]
fn multiply_by_a_negated_power_of_two() {
    let mut dag = dag();
    let x = load(&mut dag, EVT::I32, 1);
    let product = mul_by(&mut dag, x, (-8i32) as u32 as u128);
    dag.set_root(product);
    generic(&mut dag);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Sub);
    assert!(dag.is_null_constant(dag.operand(root, 0)));
    assert_eq!(dag.opcode(dag.operand(root, 1)), ISD::Shl);
}

/// The worklist revisits what a rewrite made: `(x * 2) * 3` is
/// `(x << 1) * 3`, then `x * 6`.
#[test]
fn a_rewrite_is_combined_again() {
    let mut dag = dag();
    let x = load(&mut dag, EVT::I32, 1);
    let twice = mul_by(&mut dag, x, 2);
    let thrice = mul_by(&mut dag, twice, 3);
    dag.set_root(thrice);
    generic(&mut dag);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Mul);
    assert_eq!((dag.operand(root, 0), dag.constant(dag.operand(root, 1))), (x, Some(6)));
}

/// Before types are legal the target leaves a multiply alone (the pin's
/// `isBeforeLegalize`); after, `x * 5` is `MUL_IMM`, one `lea`. The shadow
/// found `x * 5` as an `imul` where the pin has `lea`.
#[test]
fn multiply_by_five_is_a_lea_after_types_are_legal() {
    let mut dag = dag();
    let x = load(&mut dag, EVT::I32, 1);
    let five = mul_by(&mut dag, x, 5);
    dag.set_root(five);
    x86(&mut dag, CombineLevel::BeforeLegalizeTypes);
    assert_eq!(dag.opcode(dag.get_root()), ISD::Mul);
    x86(&mut dag, CombineLevel::AfterLegalizeTypes);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), x86isd::MUL_IMM);
    assert_eq!((dag.operand(root, 0), dag.constant(dag.operand(root, 1))), (x, Some(5)));
}

/// The other constants (root and first operand): 15 is `(x*5)*3`, 10 is
/// `(x<<1)*5`, 11 is `(x*5 << 1) + x`, -5 is `0 - x*5`, 7 is `(x << 3)
/// - x`.
#[test]
fn multiply_by_other_constants_is_leas_shifts_and_adds() {
    let shape = |c: i32| {
        let mut dag = dag();
        let x = load(&mut dag, EVT::I32, 1);
        let product = mul_by(&mut dag, x, c as u32 as u128);
        dag.set_root(product);
        x86(&mut dag, CombineLevel::AfterLegalizeTypes);
        let root = dag.get_root();
        let opcodes = |dag: &SelectionDAG, v: SDValue| (dag.opcode(v), dag.opcode(dag.operand(v, 0)));
        opcodes(&dag, root)
    };
    assert_eq!(shape(15), (x86isd::MUL_IMM, x86isd::MUL_IMM));
    assert_eq!(shape(10), (x86isd::MUL_IMM, ISD::Shl));
    assert_eq!(shape(11), (ISD::Add, ISD::Shl));
    assert_eq!(shape(-5).0, ISD::Sub);
    assert_eq!(shape(7), (ISD::Sub, ISD::Shl));
}

/// A 486 `imul r16,r16,imm` is 13 clocks: `histo` at -O2 priced `x*7+3` as
/// one (+26% clocks against main's `shl/sub`). The pin leaves an `i16`
/// multiply alone; a target whose multiply is dear (`i16_mul_chains`) gets the
/// `i32` chains (7 is `(x << 3) - x`), and `minsize` keeps the `imul`.
#[test]
fn an_i16_multiply_is_a_chain_where_the_target_asks_and_an_imul_otherwise() {
    let shape = |c: u128, chains: bool, min_size: bool| {
        let mut dag = dag();
        let x = load(&mut dag, EVT::I16, 1);
        let k = dag.get_constant(c, EVT::I16);
        let product = dag.get_node(ISD::Mul, EVT::I16, &[x, k]);
        dag.set_root(product);
        let mut lowering = X86Lowering { cmov: false, sse2: false, min_size, i16_mul_chains: chains, profile: None };
        DagCombiner::new(&mut dag, &mut lowering, CombineLevel::AfterLegalizeTypes).run();
        dag.opcode(dag.get_root())
    };
    // An `i16` has no `lea` pattern for `MUL_IMM`: a chain holding one still
    // ends in an `imul` (recsum c -O2: `x*11` was `imul 5; add; add`).
    let holds = |c: u128, op: ISD| {
        let mut dag = dag();
        let x = load(&mut dag, EVT::I16, 1);
        let k = dag.get_constant(c, EVT::I16);
        let product = dag.get_node(ISD::Mul, EVT::I16, &[x, k]);
        dag.set_root(product);
        let mut lowering =
            X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: true, profile: None };
        DagCombiner::new(&mut dag, &mut lowering, CombineLevel::AfterLegalizeTypes).run();
        let mut todo = vec![dag.get_root()];
        while let Some(value) = todo.pop() {
            if dag.opcode(value) == op {
                return true;
            }
            todo.extend(dag.node(value).ops.iter().copied().filter(|v| dag.opcode(*v) != ISD::Load));
        }
        false
    };
    for c in [3, 5, 9, 10, 11, 12, 15, 24] {
        assert!(!holds(c, x86isd::MUL_IMM), "x*{c} kept a MUL_IMM");
        assert!(!holds(c, ISD::Mul), "x*{c} kept a Mul");
    }
    for c in [7, 10, 12, 24, 3, 5, 9] {
        assert_ne!(shape(c, true, false), ISD::Mul, "x*{c} stayed an imul");
        assert_eq!(shape(c, false, false), ISD::Mul, "x*{c} left the default");
        assert_eq!(shape(c, true, true), ISD::Mul, "x*{c} grew a minsize body");
    }
    assert_eq!(shape(7, true, false), ISD::Sub);
}

/// An `imul` by 75 is 13+ clocks on a 486 where a `lea`/`shl` chain is a few:
/// switch c, huge and qmove kept the `imul` (the pin's `combineMul` has no
/// case for 75). The sheet's chain is built where it is cheaper, never at
/// `minsize` nor where the sheet has none (277167 on the P5).
#[test]
fn a_constant_the_pin_leaves_an_imul_is_the_priced_chain() {
    let shape = |cpu: &crate::backend::cpu::Profile, c: u128, min_size: bool, priced: bool| {
        let mut dag = dag();
        let x = load(&mut dag, EVT::I32, 1);
        let product = mul_by(&mut dag, x, c);
        dag.set_root(product);
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut lowering =
            X86Lowering { cmov: false, sse2: false, min_size, i16_mul_chains: false, profile: priced.then_some(cpu) };
        DagCombiner::new(&mut dag, &mut lowering, CombineLevel::AfterLegalizeTypes).with_tli(&tli).run();
        dag.opcode(dag.get_root())
    };
    let i486 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "486", false).expect("a cpu");
    let p5 = crate::backend::cpu::tuned_for(&llrm_x86_m32::M32, "P5", false).expect("a cpu");
    assert_ne!(shape(i486, 75, false, true), x86isd::MUL_IMM, "75 kept the imul");
    assert_ne!(shape(i486, 75, false, true), ISD::Mul, "75 kept the multiply");
    assert_eq!(shape(i486, 75, false, false), ISD::Mul, "no profile: the pin's imul");
    assert_eq!(shape(i486, 75, true, true), ISD::Mul, "minsize grew");
    assert!(crate::backend::arithmetic::scale(277167, p5).unwrap().is_none(), "premise: no cheaper chain");
    assert_eq!(shape(p5, 277167, false, true), ISD::Mul, "the sheet has no chain: the imul stays");
}

/// The `i16` multiply chain is built where `i16` is not promoted and nowhere
/// else: one predicate (`promotes_i16`) decides both, so changing SD441's rule
/// moves them together.
#[test]
fn the_i16_chain_is_built_exactly_where_i16_is_not_promoted() {
    use super::super::dag_combiner::CombineHooks;
    use super::super::x86_lowering::promotes_i16;
    let m16 = TargetLowering::x86_16(Features::default(), MVT::I16);
    let m32 = TargetLowering::x86_32(Features::default(), MVT::I32);
    for (tli, promotes) in [(m16, false), (m32, true)] {
        let mut dag = dag();
        // no foldable load: the pin's rule keeps an op beside one in i16
        let x = dag.get_register(EVT::I16, 2);
        let y = dag.get_register(EVT::I16, 3);
        let sum = dag.get_node(ISD::Add, EVT::I16, &[x, y]);
        let lowering = X86Lowering {
            cmov: false,
            sse2: false,
            min_size: false,
            i16_mul_chains: !promotes_i16(&tli),
            profile: None,
        };
        assert_eq!(promotes_i16(&tli), promotes);
        assert_eq!(lowering.promote_type(&dag, &tli, sum).is_some(), promotes);
        assert_eq!(lowering.i16_mul_chains, !promotes);
    }
}

/// A lowering that returns two values (a double-width shift) leaves them as one
/// `MERGE_VALUES`; nothing selects it, so 17 execution programs stopped at
/// "unselected node MergeValues". `visitMERGE_VALUES` makes each user read the
/// operands.
#[test]
fn a_merge_values_is_replaced_by_its_operands() {
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let pair = dag.get_node_multi(ISD::MergeValues, &[EVT::I32, EVT::I32], &[a, b]);
    let sum =
        dag.get_node(ISD::Add, EVT::I32, &[SDValue { node: pair.node, res: 0 }, SDValue { node: pair.node, res: 1 }]);
    dag.set_root(sum);
    generic(&mut dag);
    let root = dag.get_root();
    assert_eq!((dag.operand(root, 0), dag.operand(root, 1)), (a, b));
}

/// A branch on `!(a == b)` made `xor (setcc eq), true`, which selected as a
/// `setcc`, an `xor` and a compare against zero (8 m32 shadow functions:
/// `SETCCr` the pin does not have). `not (setcc x, y, cc)` is `setcc x, y,
/// !cc`.
#[test]
fn a_not_of_a_compare_is_the_inverse_compare() {
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let eq = dag.get_setcc(EVT::I1, a, b, CondCode::SetEq);
    let one = dag.get_constant(1, EVT::I1);
    let not = dag.get_node(ISD::Xor, EVT::I1, &[eq, one]);
    let chain = dag.entry_node();
    let dest = dag.get_basic_block(1);
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, not, dest]);
    dag.set_root(branch);
    generic(&mut dag);
    let root = dag.get_root();
    let cond = dag.operand(root, 1);
    assert_eq!(dag.opcode(cond), ISD::SetCC);
    assert!(matches!(dag.node(dag.operand(cond, 2)).data, NodeData::CondCode(CondCode::SetNe)));
}

/// A far pointer's null test compared the joined dword with zero: `shl;
/// movzx; add; je` (a third of the extra instructions of `_part_frame`).
/// A pair is zero when `lo | hi` is, as `ExpandSetCCOperands` does.
#[test]
fn a_far_pointer_null_test_is_the_or_of_its_halves() {
    for cc in [CondCode::SetEq, CondCode::SetNe] {
        let mut dag = dag();
        let (offset, selector) = (load(&mut dag, EVT::I16, 1), load(&mut dag, EVT::I16, 2));
        let pair = dag.get_node(ISD::BuildPair, EVT::I32, &[offset, selector]);
        let zero = dag.get_constant(0, EVT::I32);
        let test = dag.get_setcc(EVT::I1, pair, zero, cc);
        let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
        let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, test, dest]);
        dag.set_root(branch);
        generic(&mut dag);
        let cond = dag.operand(dag.get_root(), 1);
        let joined = dag.operand(cond, 0);
        assert_eq!((dag.opcode(cond), dag.opcode(joined), dag.value_type(joined)), (ISD::SetCC, ISD::Or, EVT::I16));
        assert!(matches!(dag.node(dag.operand(cond, 2)).data, NodeData::CondCode(c) if c == cc));
    }
}

/// A clamp `x > 1.0 ? 1.0 : x` laid out with the true block next branched on
/// `xor (setcc ogt), true`: `seta; xor 1; test 1; jne` where the old selector
/// had one `jbe`. The not of a float compare is the inverse float compare.
#[test]
fn a_not_of_a_float_compare_is_the_inverse_float_compare() {
    for (cc, not_cc) in [
        (CondCode::SetOGt, CondCode::SetULe),
        (CondCode::SetOLt, CondCode::SetUGe),
        (CondCode::SetULe, CondCode::SetOGt),
        (CondCode::SetUNe, CondCode::SetOEq),
    ] {
        let mut dag = dag();
        let (a, b) = (load(&mut dag, EVT::Simple(MVT::F64), 1), load(&mut dag, EVT::Simple(MVT::F64), 2));
        let cmp = dag.get_setcc(EVT::I1, a, b, cc);
        let one = dag.get_constant(1, EVT::I1);
        let not = dag.get_node(ISD::Xor, EVT::I1, &[cmp, one]);
        let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
        let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, not, dest]);
        dag.set_root(branch);
        generic(&mut dag);
        let cond = dag.operand(dag.get_root(), 1);
        assert_eq!(dag.opcode(cond), ISD::SetCC, "{cc:?}");
        assert!(matches!(dag.node(dag.operand(cond, 2)).data, NodeData::CondCode(c) if c == not_cc), "{cc:?}");
    }
}

/// After legalization `oeq` and `une` are not legal condition codes (two flags
/// on x86): `not (setcc une)` stays a `xor`, or an illegal code reaches the
/// matcher. Before it the fold is made.
#[test]
fn a_not_of_a_float_compare_does_not_make_an_illegal_code_after_legalization() {
    for (level, folds) in [(CombineLevel::BeforeLegalizeTypes, true), (CombineLevel::AfterLegalizeDag, false)] {
        let mut dag = dag();
        let (a, b) = (load(&mut dag, EVT::Simple(MVT::F64), 1), load(&mut dag, EVT::Simple(MVT::F64), 2));
        let cmp = dag.get_setcc(EVT::I1, a, b, CondCode::SetUNe);
        let one = dag.get_constant(1, EVT::I1);
        let not = dag.get_node(ISD::Xor, EVT::I1, &[cmp, one]);
        let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
        let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, not, dest]);
        dag.set_root(branch);
        x86(&mut dag, level);
        let cond = dag.operand(dag.get_root(), 1);
        assert_eq!(dag.opcode(cond) == ISD::SetCC, folds, "{level:?}");
    }
}

/// The check was x86's `oeq`/`une` by name, so a target that expands another
/// code (wasm has no `ule`) got `not (setcc ogt)` folded into the expanded
/// `ule`, which legalization had just undone. The table decides.
#[test]
fn a_not_of_a_float_compare_does_not_make_a_code_the_target_expands() {
    let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    tli.set_cond_code_action(CondCode::SetULe, MVT::F64, Action::Expand);
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::Simple(MVT::F64), 1), load(&mut dag, EVT::Simple(MVT::F64), 2));
    let cmp = dag.get_setcc(EVT::I1, a, b, CondCode::SetOGt);
    let one = dag.get_constant(1, EVT::I1);
    let not = dag.get_node(ISD::Xor, EVT::I1, &[cmp, one]);
    let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, not, dest]);
    dag.set_root(branch);
    DagCombiner::new(
        &mut dag,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
        CombineLevel::AfterLegalizeDag,
    )
    .with_tli(&tli)
    .run();
    let cond = dag.operand(dag.get_root(), 1);
    assert_eq!(dag.opcode(cond), ISD::Xor, "the not stays; ule is expanded");
}
