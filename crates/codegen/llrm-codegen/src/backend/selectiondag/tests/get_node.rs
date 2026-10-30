use super::*;

/// Without folding `x + 0` stays an add node, so a later `store` of it
/// selects a needless `add`; the pin returns `x`.
#[test]
fn adding_zero_gives_the_operand() {
    let mut dag = dag();
    let x = arg(&mut dag, EVT::I32, 1);
    let zero = dag.get_constant(0, EVT::I32);
    assert_eq!(dag.get_node(ISD::Add, EVT::I32, &[x, zero]), x);
    assert_eq!(dag.get_node(ISD::Add, EVT::I32, &[zero, x]), x, "constant moves right first");
}

/// `x * 0` is the zero constant (`getNode`); `x * 1` is the combiner's, so
/// the node stays.
#[test]
fn multiplying_by_zero_folds_and_by_one_does_not() {
    let mut dag = dag();
    let x = arg(&mut dag, EVT::I32, 1);
    let zero = dag.get_constant(0, EVT::I32);
    let one = dag.get_constant(1, EVT::I32);
    assert_eq!(dag.get_node(ISD::Mul, EVT::I32, &[x, zero]), zero);
    let times_one = dag.get_node(ISD::Mul, EVT::I32, &[x, one]);
    assert_eq!(dag.opcode(times_one), ISD::Mul);
}

/// An unfolded `3 + 4` is an add of two constants, which the matcher cannot
/// select as `mov 7`.
#[test]
fn constants_fold_to_one_constant_node() {
    let mut dag = dag();
    let (three, four) = (dag.get_constant(3, EVT::I32), dag.get_constant(4, EVT::I32));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[three, four]);
    assert_eq!(dag.constant(sum), Some(7));
    assert_eq!(sum, dag.get_constant(7, EVT::I32));
}

/// Building `x + y` twice gave two nodes without the CSE map, doubling the
/// instructions selected for a repeated expression.
#[test]
fn equal_requests_give_one_node() {
    let mut dag = dag();
    let (x, y) = (arg(&mut dag, EVT::I16, 1), arg(&mut dag, EVT::I16, 2));
    let first = dag.get_node(ISD::Add, EVT::I16, &[x, y]);
    let before = dag.node_count();
    let second = dag.get_node(ISD::Add, EVT::I16, &[x, y]);
    assert_eq!(first, second);
    assert_eq!(dag.node_count(), before);
    let five = dag.get_constant(5, EVT::I16);
    let right = dag.get_node(ISD::Add, EVT::I16, &[x, five]);
    assert_eq!(dag.get_node(ISD::Add, EVT::I16, &[five, x]), right, "a constant moves right");
    let other = dag.get_node(ISD::Sub, EVT::I16, &[x, y]);
    assert_ne!(first, other);
}

/// CSE keeps the flags both requests share: `add nsw` then plain `add` is
/// one node without `nsw`, or the second user would rely on a flag it never
/// had.
#[test]
fn a_shared_node_keeps_only_the_common_flags() {
    let mut dag = dag();
    let (x, y) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2));
    let flagged = dag.get_node_flags(ISD::Add, EVT::I32, &[x, y], SDNodeFlags::NO_SIGNED_WRAP);
    let plain = dag.get_node(ISD::Add, EVT::I32, &[x, y]);
    assert_eq!(flagged, plain);
    assert!(!dag.node(plain).flags.has(SDNodeFlags::NO_SIGNED_WRAP));
}

/// A glue-typed node is never shared: two `CopyToReg`-glued chains must stay
/// apart.
#[test]
fn glue_results_are_not_shared() {
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &[entry]);
    let b = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &[entry]);
    assert_ne!(a, b);
}

/// Folded at the node's width: a 16-bit `0xffff + 1` wrapped to 0, not 0x10000.
#[test]
fn constant_arithmetic_wraps_at_the_width() {
    let mut dag = dag();
    let (max, one) = (dag.get_constant(0xffff, EVT::I16), dag.get_constant(1, EVT::I16));
    let sum = dag.get_node(ISD::Add, EVT::I16, &[max, one]);
    assert_eq!(dag.constant(sum), Some(0));
    let four = dag.get_constant(4, EVT::I16);
    let shl = dag.get_node(ISD::Shl, EVT::I16, &[max, four]);
    assert_eq!(dag.constant(shl), Some(0xfff0));
    let sra = dag.get_node(ISD::Sra, EVT::I16, &[max, four]);
    assert_eq!(dag.constant(sra), Some(0xffff));
}

#[test]
fn division_by_zero_is_undef() {
    let mut dag = dag();
    let (x, zero) = (arg(&mut dag, EVT::I32, 1), dag.get_constant(0, EVT::I32));
    let q = dag.get_node(ISD::UDiv, EVT::I32, &[x, zero]);
    assert_eq!(dag.opcode(q), ISD::Undef);
}

#[test]
fn extensions_of_constants_and_chains() {
    let mut dag = dag();
    let minus_one = dag.get_constant(0xff, EVT::I8);
    let sext = dag.get_node(ISD::SignExtend, EVT::I32, &[minus_one]);
    assert_eq!(dag.constant(sext), Some(0xffff_ffff));
    let zext = dag.get_node(ISD::ZeroExtend, EVT::I32, &[minus_one]);
    assert_eq!(dag.constant(zext), Some(0xff));
    let x = arg(&mut dag, EVT::I8, 1);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I16, &[x]);
    let wider = dag.get_node(ISD::ZeroExtend, EVT::I32, &[wide]);
    assert_eq!(dag.operand(wider, 0), x, "zext of zext is one zext");
    let back = dag.get_node(ISD::Truncate, EVT::I8, &[wide]);
    assert_eq!(back, x, "trunc of zext to the source type");
}

#[test]
fn setcc_of_constants_and_equal_operands() {
    let mut dag = dag();
    let (a, b) = (dag.get_constant(0xff, EVT::I8), dag.get_constant(1, EVT::I8));
    let signed_lt = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let unsigned_lt = dag.get_setcc(EVT::I1, a, b, CondCode::SetULt);
    assert_eq!((dag.constant(signed_lt), dag.constant(unsigned_lt)), (Some(1), Some(0)));
    let x = arg(&mut dag, EVT::I32, 1);
    let same = dag.get_setcc(EVT::I1, x, x, CondCode::SetLe);
    assert_eq!(dag.constant(same), Some(1));
    let differ = dag.get_setcc(EVT::I1, x, x, CondCode::SetNe);
    assert_eq!(dag.constant(differ), Some(0));
}

#[test]
fn select_on_a_constant_condition_and_equal_arms() {
    let mut dag = dag();
    let (x, y) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2));
    let (t, f) = (dag.get_bool_constant(true, EVT::I1), dag.get_bool_constant(false, EVT::I1));
    assert_eq!(dag.get_select(EVT::I32, t, x, y), x);
    assert_eq!(dag.get_select(EVT::I32, f, x, y), y);
    let c = arg(&mut dag, EVT::I1, 3);
    assert_eq!(dag.get_select(EVT::I32, c, x, x), x);
}

#[test]
fn a_global_plus_a_constant_folds_into_the_symbol() {
    let mut dag = SelectionDAG::new(DagConfig { offset_folding: true, ..DagConfig::default() });
    let global = dag.get_global_address(EVT::I32, 7, 4);
    let eight = dag.get_constant(8, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[global, eight]);
    assert_eq!(sum, dag.get_global_address(EVT::I32, 7, 12));
    let diff = dag.get_node(ISD::Sub, EVT::I32, &[global, eight]);
    assert_eq!(diff, dag.get_global_address(EVT::I32, 7, -4));
}

#[test]
fn condition_code_bit_tricks_match_the_pin() {
    assert_eq!(CondCode::SetLt.swapped_operands(), CondCode::SetGt);
    assert_eq!(CondCode::SetULe.swapped_operands(), CondCode::SetUGe);
    assert_eq!(CondCode::SetEq.inverse_integer(), CondCode::SetNe);
    assert_eq!(CondCode::SetLt.inverse_integer(), CondCode::SetGe);
    assert_eq!(CondCode::SetUGt.inverse_integer(), CondCode::SetULe);
    assert!(CondCode::SetLe.true_when_equal() && !CondCode::SetLt.true_when_equal());
}

#[test]
fn bit_counts_of_constants() {
    let mut dag = dag();
    let c = dag.get_constant(0b0001_0100, EVT::I8);
    let ctlz = dag.get_node(ISD::CtLz, EVT::I8, &[c]);
    let cttz = dag.get_node(ISD::CtTz, EVT::I8, &[c]);
    let ctpop = dag.get_node(ISD::CtPop, EVT::I8, &[c]);
    assert_eq!((dag.constant(ctlz), dag.constant(cttz), dag.constant(ctpop)), (Some(3), Some(2), Some(2)));
    let bswap = dag.get_constant(0x1234, EVT::I16);
    let swapped = dag.get_node(ISD::BSwap, EVT::I16, &[bswap]);
    assert_eq!(dag.constant(swapped), Some(0x3412));
}

/// Replacing a value rewrites its users; two users that become equal are one
/// node after (the pin's `AddModifiedNodeToCSEMaps`), or a later `getNode` of
/// the rewritten expression would build a second copy.
#[test]
fn replacing_a_value_rewrites_users_and_merges_equal_ones() {
    let mut dag = dag();
    let (x, y, z) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2), arg(&mut dag, EVT::I32, 3));
    let xz = dag.get_node(ISD::Add, EVT::I32, &[x, z]);
    let yz = dag.get_node(ISD::Add, EVT::I32, &[y, z]);
    let outer_x = dag.get_node(ISD::Mul, EVT::I32, &[xz, z]);
    let outer_y = dag.get_node(ISD::Mul, EVT::I32, &[yz, z]);
    assert_eq!(dag.users(xz).collect::<Vec<_>>(), vec![outer_x.node]);
    dag.replace_all_uses_with(x, y);
    assert_eq!(dag.users(x).count(), 0);
    assert_eq!(dag.get_node(ISD::Add, EVT::I32, &[y, z]), yz, "x+z became y+z, the existing node");
    assert_eq!(dag.get_node(ISD::Mul, EVT::I32, &[yz, z]), outer_y, "and (x+z)*z the existing (y+z)*z");
    assert_eq!(dag.node(outer_x).ops[0], yz);
    assert_eq!(dag.node_ids().count(), dag.node_count());
}

/// `users` and `use_count` walk the use list in place while it is sorted and
/// sort a copy when it is not (a node re-entered after an operand update, a
/// node reading one value twice): in creation order, once each, none deleted.
#[test]
fn users_are_distinct_and_in_creation_order_whatever_the_use_list() {
    let mut dag = dag();
    let a = dag.get_register(EVT::I32, 1);
    let b = dag.get_register(EVT::I32, 2);
    let first = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    let second = dag.get_node(ISD::Mul, EVT::I32, &[a, a]);
    // `first` is entered in `a`'s use list again, after `second`.
    dag.update_node_operands(first.node, &[a, b]);
    let users: Vec<u32> = dag.users(a).collect();
    assert_eq!(users, [first.node, second.node]);
    assert_eq!(dag.use_count(a.node), 3, "first once, second twice");
    let third = dag.get_node(ISD::Sub, EVT::I32, &[a, b]);
    dag.delete_dead(third.node);
    assert_eq!(dag.users(a).collect::<Vec<u32>>(), [first.node, second.node]);
}

/// Where a function evaluates floats at the x87's extended precision, a
/// quotient the single cannot hold is not folded: `0.5 * (-1 / 7)` stayed
/// `-0.0714285746` (single bits `bd924925`) where BC prints the double
/// `bfb2492492492492` (tests/differential/qb-bc/single-constants.bas).
#[test]
fn an_inexact_single_quotient_is_not_folded_under_extended_evaluation() {
    let divide = |fp_extended: bool| {
        let mut dag = SelectionDAG::new(DagConfig { fp_extended, ..DagConfig::default() });
        let one = dag.get_constant_fp(u128::from(1.0f32.to_bits()), EVT::Simple(MVT::F32));
        let seven = dag.get_constant_fp(u128::from(7.0f32.to_bits()), EVT::Simple(MVT::F32));
        let quotient = dag.get_node(ISD::FDiv, EVT::Simple(MVT::F32), &[one, seven]);
        let exact = dag.get_node(ISD::FDiv, EVT::Simple(MVT::F32), &[one, one]);
        (dag.opcode(quotient) == ISD::ConstantFP, dag.opcode(exact) == ISD::ConstantFP)
    };
    assert_eq!(divide(false), (true, true));
    assert_eq!(divide(true), (false, true));
}

/// Two loads of one address on one chain that differ only in their atomic
/// ordering are two nodes: merged, an `acquire` load would be selected as the
/// plain load its twin is.
#[test]
fn loads_that_differ_in_ordering_are_not_merged() {
    use crate::backend::selectiondag::{MemBase, MemInfo};
    use llrm_mir::opcode::AtomicOrdering;
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = |ordering| MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Unknown,
        ext: None,
        space: 0,
        ordering,
    };
    let plain = dag.get_load(EVT::I32, chain, ptr, info(None));
    let acquire = dag.get_load(EVT::I32, chain, ptr, info(Some(AtomicOrdering::Acquire)));
    let again = dag.get_load(EVT::I32, chain, ptr, info(Some(AtomicOrdering::Acquire)));
    assert_ne!(plain.node, acquire.node);
    assert_eq!(acquire.node, again.node, "equal accesses still share one node");
}
