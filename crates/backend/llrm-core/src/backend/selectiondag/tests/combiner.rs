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
    let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0 };
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
    DagCombiner::new(dag, &mut X86Lowering { cmov: false }, level).run();
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

/// A node made after its users (a fold's result replacing an operand) was
/// visited again by every user: visiting a node added its operands whatever
/// their state, so a node was taken once per path to it and QCport's
/// `_scr_draw_hud` (m32 -O2) never finished compiling. A node already visited
/// is not added again as an operand (`SkipIfCombinedBefore`).
#[test]
fn a_node_made_after_its_users_is_visited_once() {
    let mut dag = dag();
    let (placeholder, r5, r6) =
        (dag.get_register(EVT::I32, 7), dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let (c1, c2) = (dag.get_constant(1, EVT::I32), dag.get_constant(2, EVT::I32));
    let x = dag.get_node(ISD::Add, EVT::I32, &[placeholder, c1]);
    let y = dag.get_node(ISD::Xor, EVT::I32, &[placeholder, c2]);
    let root = dag.get_node(ISD::Sub, EVT::I32, &[x, y]);
    dag.set_root(root);
    let made = dag.get_node(ISD::Mul, EVT::I32, &[r5, r6]);
    dag.replace_all_uses_with(placeholder, made);
    let mut hooks = NoCombines;
    let mut combiner = DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes);
    combiner.run();
    // root, x, y, made, r5, r6, c1, c2
    assert_eq!(combiner.visits, 8);
}
