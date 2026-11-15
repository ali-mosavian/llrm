//! Folds the pin makes for every target, on only for a target that asks
//! (`CombineHooks`), so x86's rows are untouched until the judge has seen them.

use super::super::actions::{LoadExt, TargetLowering};
use super::super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner, NoCombines};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

struct Asks;

impl CombineHooks for Asks {
    fn should_transform_signed_truncation_check(
        &self,
        _vt: EVT,
        _kept_bits: u32,
    ) -> bool {
        true
    }
    fn fold_sra_of_trunc_of_shift(&self) -> bool {
        true
    }
    fn fold_mul_of_add_with_constant(&self) -> bool {
        true
    }
    fn widen_multiply_high_any_width(&self) -> bool {
        true
    }
    fn shift_amount_type(
        &self,
        shifted: EVT,
    ) -> EVT {
        shifted
    }
    fn fold_xor_of_and_with_mask(&self) -> bool {
        true
    }
    fn fold_sext_in_reg_of_load(&self) -> bool {
        true
    }
    fn push_freeze_through_operands(&self) -> bool {
        true
    }
    fn merge_stores_via_trunc_store(&self) -> bool {
        true
    }
    fn fold_or_absorption(&self) -> bool {
        true
    }
    fn reduce_load_width_through_freeze(&self) -> bool {
        true
    }
    fn select_to_select_cc(
        &self,
        vt: EVT,
    ) -> bool {
        vt == EVT::I32
    }
    fn reassociate_compare_pairs(&self) -> bool {
        true
    }
    fn store_fp_constant_as_i64(&self) -> bool {
        true
    }
    fn sub_algebra_folds(&self) -> bool {
        true
    }
    fn rebuild_brcond_condition(&self) -> bool {
        true
    }
    fn fold_bin_op_into_select(&self) -> bool {
        true
    }
}

fn xor_of_masked(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let mask = dag.get_constant(0xf0f0, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
    let root = dag.get_node(ISD::Xor, EVT::I32, &[masked, mask]);
    dag.set_root(root);
}

/// `(a & c) ^ c` stayed an `and` and an `xor`; the pin makes it `(~a) & c`,
/// which AArch64 selects as one `bic`.
#[test]
fn xor_of_a_masked_value_with_its_mask_is_the_masked_complement() {
    let mut dag = dag();
    xor_of_masked(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::And);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Xor, "the complement of a");
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0xf0f0));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_xor_of_the_masked_value() {
    let mut dag = dag();
    xor_of_masked(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Xor);
}

fn sext_in_reg_of_extload(dag: &mut SelectionDAG) {
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(0),
        ext: Some(LoadExt::ExtLoad),
        space: 0,
        ordering: None,
    };
    let load = dag.get_ext_load(LoadExt::ExtLoad, EVT::I32, chain, ptr, info);
    let narrow = dag.get_value_type_node(EVT::I8);
    let root = dag.get_node(ISD::SignExtendInReg, EVT::I32, &[load, narrow]);
    dag.set_root(root);
}

/// A stack byte read as `extload i8` and sign-extended in the register stayed
/// two operations; the pin makes it one `sextload i8`.
#[test]
fn sext_in_reg_of_an_extending_load_is_a_sign_extending_load() {
    let mut dag = dag();
    sext_in_reg_of_extload(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Load);
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a memory node") };
    assert_eq!(info.ext, Some(LoadExt::SExtLoad));
}

/// m32 `stft_fixed`: the extending load also read by an add stayed beside the
/// new `sextload` (a second load of the same word, +18 B); the pin hands the
/// other users the `sextload` too.
#[test]
fn the_other_users_of_an_extending_load_read_the_new_sign_extending_load() {
    let mut dag = dag();
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo {
        mem_vt: EVT::I16,
        align: 2,
        volatile: false,
        base: MemBase::Stack(0),
        ext: Some(LoadExt::ExtLoad),
        space: 0,
        ordering: None,
    };
    let load = dag.get_ext_load(LoadExt::ExtLoad, EVT::I32, chain, ptr, info);
    let narrow = dag.get_value_type_node(EVT::I16);
    let sext = dag.get_node(ISD::SignExtendInReg, EVT::I32, &[load, narrow]);
    let five = dag.get_constant(5, EVT::I32);
    let add = dag.get_node(ISD::Add, EVT::I32, &[load, five]);
    let root = dag.get_node(ISD::Xor, EVT::I32, &[sext, add]);
    dag.set_root(root);
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();
    let root = dag.get_root();
    let (left, right) = (dag.operand(root, 0), dag.operand(dag.operand(root, 1), 0));
    assert_eq!(left.node, right.node, "one load");
}

#[test]
fn a_target_that_does_not_ask_keeps_the_sign_extension_in_the_register() {
    let mut dag = dag();
    sext_in_reg_of_extload(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::SignExtendInReg);
}

fn frozen_compare(dag: &mut SelectionDAG) {
    let a = dag.get_register(EVT::I32, 5);
    // A shift by a variable amount may be poison, so the combiner cannot drop
    // the freeze.
    let amount = dag.get_register(EVT::I32, 6);
    let loaded = dag.get_node(ISD::Shl, EVT::I32, &[a, amount]);
    let setcc = dag.get_setcc(EVT::I32, a, loaded, CondCode::SetLt);
    let root = dag.get_node(ISD::Freeze, EVT::I32, &[setcc]);
    dag.set_root(root);
}

/// `freeze (setcc a, x)` stayed a freeze of a compare; the pin freezes the
/// operand it cannot prove defined, so the compare is a bare `setcc` again.
#[test]
fn a_frozen_compare_is_a_compare_of_a_frozen_operand() {
    let mut dag = dag();
    frozen_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::SetCC);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_frozen_compare() {
    let mut dag = dag();
    frozen_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Freeze);
}

fn two_byte_stores(dag: &mut SelectionDAG) {
    let (ptr, chain) = (dag.get_register(EVT::I64, 7), dag.entry_node());
    let info = |offset| MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(offset),
        ext: None,
        space: 0,
        ordering: None,
    };
    let (lo, hi) = (dag.get_constant(1, EVT::I8), dag.get_constant(0, EVT::I8));
    let one = dag.get_constant(1, EVT::I64);
    let next = dag.get_node(ISD::Add, EVT::I64, &[ptr, one]);
    let first = dag.get_store(chain, lo, ptr, info(0));
    let second = dag.get_store(first, hi, next, info(1));
    dag.set_root(second);
}

/// A target with no 16-bit register that can still store one from an `i32`.
fn no_halfword_register() -> TargetLowering {
    let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    tli.types = TargetLoweringInfo::new(&[MVT::I8, MVT::I32], false);
    tli.set_trunc_store_action(MVT::I32, MVT::I16, Action::Legal);
    tli
}

fn is_one_halfword_store(dag: &SelectionDAG) -> bool {
    let root = dag.get_root();
    let NodeData::Mem(info) = dag.node(root).data else { return false };
    dag.opcode(root) == ISD::Store && info.mem_vt == EVT::I16 && dag.constant(dag.operand(root, 1)) == Some(1)
}

/// Two adjacent byte stores of constants stayed two stores on AArch64, which
/// has no 16-bit register: the pin joins them into one truncating `i16` store
/// of the promoted `i32` constant (`strh`).
#[test]
fn two_byte_constant_stores_are_one_truncating_halfword_store() {
    let tli = no_halfword_register();
    let mut dag = dag();
    two_byte_stores(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert!(is_one_halfword_store(&dag));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_two_byte_stores() {
    let tli = no_halfword_register();
    let mut dag = dag();
    two_byte_stores(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert!(!is_one_halfword_store(&dag));
}

/// Declines the bitwise form of a logic of compares, as AArch64 does.
struct Declines;

impl CombineHooks for Declines {
    fn convert_setcc_logic_to_bitwise_logic(&self) -> bool {
        false
    }
}

fn and_of_equalities(dag: &mut SelectionDAG) {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let (c1, c2) = (dag.get_constant(65, EVT::I32), dag.get_constant(66, EVT::I32));
    let left = dag.get_setcc(EVT::I1, a, c1, CondCode::SetEq);
    let right = dag.get_setcc(EVT::I1, b, c2, CondCode::SetEq);
    let root = dag.get_node(ISD::And, EVT::I1, &[left, right]);
    dag.set_root(root);
}

/// Two byte compares joined by `and` were one `xor`/`or` compare on AArch64
/// (x86's `convertSetCCLogicToBitwiseLogic`); the pin's AArch64 leaves them two
/// compares.
#[test]
fn a_target_that_declines_keeps_the_and_of_two_equalities() {
    let mut dag = dag();
    and_of_equalities(&mut dag);
    DagCombiner::new(&mut dag, &mut Declines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::And);
}

#[test]
fn a_default_target_joins_the_two_equalities_in_one_compare() {
    let mut dag = dag();
    and_of_equalities(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::SetCC);
}

fn or_of_and_with_one_operand(dag: &mut SelectionDAG) -> SDValue {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let masked = dag.get_node(ISD::And, EVT::I32, &[a, b]);
    let root = dag.get_node(ISD::Or, EVT::I32, &[masked, a]);
    dag.set_root(root);
    a
}

/// `(a & b) | a` stayed an `and` and an `or`; the pin's `visitORCommutative`
/// makes it `a`.
#[test]
fn an_or_of_an_and_with_one_of_its_operands_is_that_operand() {
    let mut dag = dag();
    let a = or_of_and_with_one_operand(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.get_root(), a);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_or_of_the_and() {
    let mut dag = dag();
    or_of_and_with_one_operand(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Or);
}

pub(super) fn masked_frozen_extload(dag: &mut SelectionDAG) {
    // a pointer that may be poison, so the freeze is not dropped
    let (base, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let one = dag.get_constant(1, EVT::I32);
    let ptr = dag.get_node_flags(
        ISD::Add,
        EVT::I32,
        &[base, one],
        SDNodeFlags::default().with(SDNodeFlags::NO_UNSIGNED_WRAP, true),
    );
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(0),
        ext: Some(LoadExt::ExtLoad),
        space: 0,
        ordering: None,
    };
    let load = dag.get_ext_load(LoadExt::ExtLoad, EVT::I32, chain, ptr, info);
    let frozen = dag.get_node(ISD::Freeze, EVT::I32, &[load]);
    let mask = dag.get_constant(0xff, EVT::I32);
    let root = dag.get_node(ISD::And, EVT::I32, &[frozen, mask]);
    dag.set_root(root);
}

/// `and (freeze (extload i8)), 255` kept the load wide and the mask (`LDURWi`
/// and `ANDWri` where the pin has `LDRBBui`): the pin's `ReduceLoadWidth` looks
/// through the freeze, narrows the load to a zero-extending one and puts the
/// freeze, and an `AssertZext`, back on it.
#[test]
fn an_and_mask_of_a_frozen_extending_load_is_a_frozen_zero_extending_load() {
    let mut dag = dag();
    masked_frozen_extload(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::AssertZext);
    let frozen = dag.operand(root, 0);
    assert_eq!(dag.opcode(frozen), ISD::Freeze);
    let load = dag.operand(frozen, 0);
    let NodeData::Mem(info) = dag.node(load).data else { panic!("a memory node") };
    assert_eq!(info.ext, Some(LoadExt::ZExtLoad));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_mask_over_the_frozen_load() {
    let mut dag = dag();
    masked_frozen_extload(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::And);
}

/// An `i64`-multiply target whose `i32` has no high multiply (AArch64's shape).
fn no_high_multiply() -> TargetLowering {
    let mut tli = TargetLowering::x86_64(Features::default(), MVT::I64);
    for op in [ISD::MulHU, ISD::UMulLoHi] {
        tli.set_operation_action(op, MVT::I32, Action::Expand);
    }
    tli
}

fn has_wide_multiply(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    let node = dag.node(value);
    (dag.opcode(value) == ISD::Mul && dag.value_type(value) == EVT::I64)
        || node.ops.iter().any(|&op| has_wide_multiply(dag, op))
}

fn udiv_by_three(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let three = dag.get_constant(3, EVT::I32);
    let root = dag.get_node(ISD::UDiv, EVT::I32, &[x, three]);
    dag.set_root(root);
}

/// `udiv i32 x, 3` took the high half through `MULHU`/`UMUL_LOHI` on a target
/// with a legal `i64` multiply; the pin multiplies in `i64` and shifts right.
#[test]
fn a_32_bit_udiv_by_a_constant_is_a_wide_multiply_where_the_target_asks() {
    let tli = no_high_multiply();
    let mut dag = dag();
    udiv_by_three(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert!(has_wide_multiply(&dag, dag.get_root()));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_32_bit_high_multiply() {
    let tli = no_high_multiply();
    let mut dag = dag();
    udiv_by_three(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert!(!has_wide_multiply(&dag, dag.get_root()));
}

fn mul_of_add(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let (three, five) = (dag.get_constant(3, EVT::I32), dag.get_constant(5, EVT::I32));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[x, three]);
    let root = dag.get_node(ISD::Mul, EVT::I32, &[sum, five]);
    dag.set_root(root);
}

/// `(x + 3) * 5` stayed a multiply of a sum; the pin distributes it to
/// `x * 5 + 15`, which AArch64 selects as a shifted add and an add-immediate.
#[test]
fn a_multiply_of_an_add_with_a_constant_distributes() {
    let mut dag = dag();
    mul_of_add(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(15));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_multiply_of_the_add() {
    let mut dag = dag();
    mul_of_add(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Mul);
}

struct Decomposes;

impl CombineHooks for Decomposes {
    fn decompose_mul_by_constant(
        &self,
        _vt: EVT,
        _c: u128,
    ) -> bool {
        true
    }
}

fn mul_by(
    dag: &mut SelectionDAG,
    constant: u128,
) {
    let x = dag.get_register(EVT::I16, 5);
    let c = dag.get_constant(constant, EVT::I16);
    let root = dag.get_node(ISD::Mul, EVT::I16, &[x, c]);
    dag.set_root(root);
}

/// `x * 33` stayed a multiply, a library call where the target has no
/// multiplier; with `decomposeMulByConstant` it is `(x << 5) + x`, and
/// `x * 15` is `(x << 4) - x`, `x * -33` its negation, `x * 6` shifted once
/// more on both terms.
#[test]
fn a_multiply_by_one_off_a_power_of_two_is_shifts_and_an_add_where_the_target_asks() {
    let shift = |dag: &SelectionDAG, v: SDValue| (dag.opcode(v), dag.constant(dag.operand(v, 1)));
    let mut dag = dag();
    mul_by(&mut dag, 33);
    DagCombiner::new(&mut dag, &mut Decomposes, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(shift(&dag, dag.operand(root, 0)), (ISD::Shl, Some(5)));

    let mut dag = self::dag();
    mul_by(&mut dag, 15);
    DagCombiner::new(&mut dag, &mut Decomposes, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Sub);
    assert_eq!(shift(&dag, dag.operand(root, 0)), (ISD::Shl, Some(4)));

    let mut dag = self::dag();
    mul_by(&mut dag, 6);
    DagCombiner::new(&mut dag, &mut Decomposes, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(shift(&dag, dag.operand(root, 0)), (ISD::Shl, Some(2)));
    assert_eq!(shift(&dag, dag.operand(root, 1)), (ISD::Shl, Some(1)));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_multiply_by_one_off_a_power_of_two() {
    let mut dag = dag();
    mul_by(&mut dag, 33);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Mul);
}

fn sra_of_truncated_high_half(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I64, 5);
    let (thirty_two, three) = (dag.get_constant(32, EVT::I64), dag.get_constant(3, EVT::I32));
    let high = dag.get_node(ISD::Srl, EVT::I64, &[x, thirty_two]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[high]);
    let root = dag.get_node(ISD::Sra, EVT::I32, &[low, three]);
    dag.set_root(root);
}

/// `(i32)(x >> 32) >> 3` stayed a shift, a truncate and a shift; the pin makes
/// it `(i32)(x >>s 35)`, one `asr` on AArch64.
#[test]
fn an_sra_of_a_truncated_high_half_is_one_wide_sra() {
    let mut dag = dag();
    sra_of_truncated_high_half(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Truncate);
    let wide = dag.operand(root, 0);
    assert_eq!(dag.opcode(wide), ISD::Sra);
    assert_eq!(dag.constant(dag.operand(wide, 1)), Some(35));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_sra_of_the_truncated_half() {
    let mut dag = dag();
    sra_of_truncated_high_half(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Sra);
}

fn fits_in_a_byte(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let (half, full) = (dag.get_constant(128, EVT::I32), dag.get_constant(256, EVT::I32));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[x, half]);
    let root = dag.get_setcc(EVT::I8, sum, full, CondCode::SetULt);
    dag.set_root(root);
}

/// `(x + 128) u< 256` stayed an add and an unsigned compare; the pin asks
/// "does `x` survive a sign-extension from `i8`": `sext_inreg x, i8 == x`.
#[test]
fn a_range_check_on_a_biased_value_is_a_sign_extension_compare() {
    let mut dag = dag();
    fits_in_a_byte(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::SetCC);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::SignExtendInReg);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_biased_range_check() {
    let mut dag = dag();
    fits_in_a_byte(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_ne!(dag.opcode(dag.operand(root, 0)), ISD::SignExtendInReg);
}

/// `x * 4` shifted by an `i8` amount whatever the target; AArch64 cannot
/// promote that operand and refused 3 corpus functions (`x * -2`).
#[test]
fn a_multiply_by_a_power_of_two_shifts_by_the_targets_amount_type() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let four = dag.get_constant(4, EVT::I32);
    let root = dag.get_node(ISD::Mul, EVT::I32, &[x, four]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Shl);
    assert_eq!(dag.value_type(dag.operand(root, 1)), EVT::I32);
}

fn select_of_compare(dag: &mut SelectionDAG) {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let c = dag.get_register(EVT::I32, 7);
    let cond = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let root = dag.get_select(EVT::I32, cond, c, b);
    dag.set_root(root);
}

/// A `select` on a compare stayed a `select` beside the compare, which a second
/// user then kept alive; the pin makes it a `select_cc` where `SELECT_CC` is
/// legal or custom.
#[test]
fn a_select_on_a_compare_is_a_select_cc_where_the_target_asks() {
    let mut dag = dag();
    select_of_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::SelectCC);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_select_on_the_compare() {
    let mut dag = dag();
    select_of_compare(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Select);
}

fn or_of_three_compares(dag: &mut SelectionDAG) {
    let r = |dag: &mut SelectionDAG, n| dag.get_register(EVT::I32, n);
    let (a, b, x, c, y) = (r(dag, 1), r(dag, 2), r(dag, 3), r(dag, 4), r(dag, 5));
    let zero = dag.get_constant(0, EVT::I32);
    let eq = dag.get_setcc(EVT::I1, a, zero, CondCode::SetEq);
    let gt1 = dag.get_setcc(EVT::I1, b, x, CondCode::SetGt);
    let gt2 = dag.get_setcc(EVT::I1, c, y, CondCode::SetGt);
    let inner = dag.get_node(ISD::Or, EVT::I1, &[eq, gt1]);
    let root = dag.get_node(ISD::Or, EVT::I1, &[inner, gt2]);
    dag.set_root(root);
}

/// `(a == 0 | b > x) | c > y` stayed in written order; the pin pairs the two
/// `>` first (`reassociateOpsCommutative`).
#[test]
fn an_or_of_compares_pairs_those_of_one_predicate_where_the_target_asks() {
    let mut dag = dag();
    or_of_three_compares(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let code = |dag: &SelectionDAG, v: SDValue| dag.node(dag.operand(v, 2)).data;
    let inner = dag.operand(root, 0);
    assert_eq!(dag.opcode(inner), ISD::Or);
    assert_eq!(code(&dag, dag.operand(inner, 0)), code(&dag, dag.operand(inner, 1)), "the two `>` meet");
}

#[test]
fn a_target_that_does_not_ask_keeps_the_written_order_of_the_compares() {
    let mut dag = dag();
    or_of_three_compares(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let code = |dag: &SelectionDAG, v: SDValue| dag.node(dag.operand(v, 2)).data;
    let inner = dag.operand(root, 0);
    assert_ne!(code(&dag, dag.operand(inner, 0)), code(&dag, dag.operand(inner, 1)));
}

fn masked_add_of_a_high_constant(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let high = dag.get_constant(0xff, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[x, high]);
    let mask = dag.get_constant(0xff, EVT::I32);
    let root = dag.get_node(ISD::And, EVT::I32, &[sum, mask]);
    dag.set_root(root);
}

/// `(x + 0xff) & 0xff` kept the addend: every demanded bit of it is set, so the
/// pin makes it `-1` (`SimplifyDemandedBits`, `ADD`), for every target.
#[test]
fn an_addend_of_all_demanded_ones_is_minus_one() {
    let mut dag = dag();
    masked_add_of_a_high_constant(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let sum = dag.operand(dag.get_root(), 0);
    assert_eq!(dag.constant(dag.operand(sum, 1)), Some(u128::from(u32::MAX)));
}

fn volatile_double_store(dag: &mut SelectionDAG) {
    let (ptr, chain) = (dag.get_register(EVT::I64, 7), dag.entry_node());
    let one = dag.get_constant_fp(u128::from(1.0f64.to_bits()), EVT::Simple(MVT::F64));
    let info = MemInfo {
        mem_vt: EVT::Simple(MVT::F64),
        align: 8,
        volatile: true,
        base: MemBase::Stack(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let root = dag.get_store(chain, one, ptr, info);
    dag.set_root(root);
}

/// `store volatile double 1.0` stayed an FP-constant store (a constant-pool
/// load); the pin stores the bits as an `i64` when that store is legal
/// (`replaceStoreOfFPConstant`).
#[test]
fn a_volatile_double_constant_is_stored_as_an_integer_where_the_target_asks() {
    let tli = TargetLowering::x86_64(Features::default(), MVT::I64);
    let mut dag = dag();
    volatile_double_store(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    assert_eq!(dag.value_type(dag.operand(root, 1)), EVT::I64);
}

#[test]
fn a_target_that_does_not_ask_keeps_the_volatile_double_store() {
    let mut dag = dag();
    volatile_double_store(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.value_type(dag.operand(dag.get_root(), 1)), EVT::Simple(MVT::F64));
}

/// Indexes any load or store at `base + simm9` (a simple AArch64 shape).
struct Indexes;

impl CombineHooks for Indexes {
    fn is_indexed_legal(
        &self,
        _is_load: bool,
        _vt: EVT,
        _post: bool,
    ) -> bool {
        true
    }
    fn indexed_address_parts(
        &self,
        dag: &SelectionDAG,
        _access: SDValue,
        address: SDValue,
        _post: bool,
    ) -> Option<(SDValue, i64)> {
        (dag.opcode(address) == ISD::Add)
            .then(|| (dag.operand(address, 0), dag.constant(dag.operand(address, 1)).unwrap() as i64))
    }
    fn is_legal_addressing_mode(
        &self,
        _vt: EVT,
        _base_offs: i64,
        _scale: i64,
    ) -> bool {
        true
    }
}

/// `store i32 4, p+16` then `store p+16, p`: the address has a second user.
fn store_then_store_of_the_address(dag: &mut SelectionDAG) {
    let (r, chain) = (dag.get_register(EVT::I64, 7), dag.entry_node());
    let q = dag.get_register(EVT::I64, 9);
    // A register base would need a copy (`CombineToPreIndexedLoadStore`, check
    // #1).
    let p = dag.get_node(ISD::Or, EVT::I64, &[r, q]);
    let sixteen = dag.get_constant(16, EVT::I64);
    let a = dag.get_node(ISD::Add, EVT::I64, &[p, sixteen]);
    let info = |vt| MemInfo {
        mem_vt: vt,
        align: 8,
        volatile: false,
        base: MemBase::Stack(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let value = dag.get_register(EVT::I32, 8);
    let first = dag.get_store(chain, value, a, info(EVT::I32));
    let second = dag.get_store(first, a, p, info(EVT::I64));
    dag.set_root(second);
}

fn has_node(
    dag: &SelectionDAG,
    opcode: ISD,
) -> bool {
    dag.nodes().iter().any(|n| n.opcode == opcode)
}

/// A store whose address `p+16` is stored again kept the add beside a plain
/// store; the pin writes the first as a pre-indexed store and stores the
/// writeback (`CombineToPreIndexedLoadStore`).
#[test]
fn a_store_whose_address_is_used_again_is_pre_indexed_where_the_target_asks() {
    let mut dag = dag();
    store_then_store_of_the_address(&mut dag);
    DagCombiner::new(&mut dag, &mut Indexes, CombineLevel::AfterLegalizeDag).run();
    assert!(has_node(&dag, ISD::StorePre));
}

#[test]
fn a_target_with_no_indexed_forms_keeps_the_plain_stores() {
    let mut dag = dag();
    store_then_store_of_the_address(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::AfterLegalizeDag).run();
    assert!(!has_node(&dag, ISD::StorePre) && !has_node(&dag, ISD::StorePost));
}

fn zero_extended_target_node(
    _dag: &SelectionDAG,
    _node: SDValue,
    _operand: &dyn Fn(usize) -> crate::backend::selectiondag::known_bits::KnownBits,
) -> Option<crate::backend::selectiondag::known_bits::KnownBits> {
    Some(crate::backend::selectiondag::known_bits::KnownBits { zero: 0xffff_ff00, one: 0, width: 32 })
}

fn masked_target_node(config: DagConfig) -> (SelectionDAG, SDValue) {
    let mut dag = SelectionDAG::new(config);
    let x = dag.get_register(EVT::I32, 5);
    let node = dag.get_node(ISD::Machine(7), EVT::I32, &[x]);
    let mask = dag.get_constant(0xff, EVT::I32);
    let root = dag.get_node(ISD::And, EVT::I32, &[node, mask]);
    dag.set_root(root);
    (dag, node)
}

/// A target node the target says is a byte: `(and node, 0xff)` is `node`.
/// Without the hook the node is unknown and the mask stays.
#[test]
fn the_known_bits_of_a_target_node_come_from_the_target_where_it_says() {
    let (mut dag, node) = masked_target_node(DagConfig {
        known_bits_for_target_node: Some(zero_extended_target_node),
        ..DagConfig::default()
    });
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.get_root(), node);
}

#[test]
fn a_target_that_says_nothing_of_its_node_keeps_the_mask() {
    let (mut dag, _) = masked_target_node(DagConfig::default());
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::And);
}

/// Keeps a select of two constants a select, as AArch64's `csel` wants.
struct KeepsConstantSelects;

impl CombineHooks for KeepsConstantSelects {
    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }
}

fn select_of_constants(dag: &mut SelectionDAG) {
    let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let (t, f) = (dag.get_constant(8, EVT::I32), dag.get_constant(0, EVT::I32));
    let pick = dag.get_select(EVT::I32, test, t, f);
    dag.set_root(pick);
}

/// `c ? 8 : 0` became `zext(c) << 3` for every target; the pin's default
/// (`convertSelectOfConstantsToMath` false) leaves the select to a `csel`.
#[test]
fn a_target_that_declines_keeps_a_select_of_two_constants() {
    let mut dag = dag();
    select_of_constants(&mut dag);
    DagCombiner::new(&mut dag, &mut KeepsConstantSelects, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Select);
    let mut dag = self::dag();
    select_of_constants(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Shl, "the default is x86's");
}

fn sub_of_a_difference(dag: &mut SelectionDAG) {
    let (a, b, c) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6), dag.get_register(EVT::I32, 7));
    let difference = dag.get_node(ISD::Sub, EVT::I32, &[b, c]);
    let root = dag.get_node(ISD::Sub, EVT::I32, &[a, difference]);
    dag.set_root(root);
}

/// `a - (b - c)` stayed two subtractions; the pin makes it `a + (c - b)`,
/// which AArch64 folds into a shifted operand (`x - (8q - q)`).
#[test]
fn a_value_minus_a_difference_is_the_sum_with_the_flipped_difference() {
    let mut dag = dag();
    sub_of_a_difference(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Add);
}

/// The same fold on x86 moved `div_constants_long` at x86_16 -O2 from 2062 to
/// 2066 instructions (a different spill): off until its rows are judged.
#[test]
fn a_target_that_does_not_ask_keeps_the_value_minus_a_difference() {
    let mut dag = dag();
    sub_of_a_difference(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Sub);
}

fn branch_on_not_of_asserted_boolean(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let one_bit = dag.get_value_type_node(EVT::I1);
    let asserted = dag.get_node(ISD::AssertZext, EVT::I32, &[x, one_bit]);
    let boolean = dag.get_node(ISD::Truncate, EVT::I1, &[asserted]);
    let ones = dag.get_constant(1, EVT::I1);
    let not = dag.get_node(ISD::Xor, EVT::I1, &[boolean, ones]);
    let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, not, dest]);
    dag.set_root(branch);
}

/// `brcond (xor (trunc (assertzext x, i1)), 1)` stayed an `xor` (wasm:
/// `i32.const 1; i32.xor; br_if`, jpeg -O1 +6 bytes); the pin's `rebuildSetCC`
/// makes it `brcond (setcc x, 0, eq)`, an `i32.eqz`.
#[test]
fn a_branch_on_the_not_of_an_asserted_boolean_is_a_compare_with_zero() {
    let mut dag = dag();
    branch_on_not_of_asserted_boolean(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let cond = dag.operand(dag.get_root(), 1);
    assert_eq!(dag.opcode(cond), ISD::SetCC);
    assert_eq!(dag.constant(dag.operand(cond, 1)), Some(0));
    assert!(matches!(dag.node(dag.operand(cond, 2)).data, NodeData::CondCode(CondCode::SetEq)));
}

#[test]
fn a_target_that_does_not_ask_keeps_the_xor_of_the_branch_condition() {
    let mut dag = dag();
    branch_on_not_of_asserted_boolean(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.operand(dag.get_root(), 1)), ISD::Xor);
}

/// `setcc ne (setcc ult x, 32), 1` stayed a compare of a compare (wasm: `x <
/// 32; i32.const 1; i32.ne`); the pin folds it to the inverse compare.
#[test]
fn a_compare_of_a_compare_with_one_is_the_inverse_compare() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let limit = dag.get_constant(32, EVT::I32);
    let below = dag.get_setcc(EVT::I1, x, limit, CondCode::SetULt);
    let one = dag.get_constant(1, EVT::I1);
    let not_below = dag.get_setcc(EVT::I1, below, one, CondCode::SetNe);
    let (chain, dest) = (dag.entry_node(), dag.get_basic_block(1));
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, not_below, dest]);
    dag.set_root(branch);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let cond = dag.operand(dag.get_root(), 1);
    assert_eq!(dag.opcode(cond), ISD::SetCC);
    assert_eq!(dag.operand(cond, 0), x);
    assert!(matches!(dag.node(dag.operand(cond, 2)).data, NodeData::CondCode(CondCode::SetUGe)));
}

/// `shl (select c, 1, 2), 2` stayed a shift of a select; the pin folds the
/// constant into the select, `select c, 4, 8` (bintree -O2, 2200 instructions).
#[test]
fn a_shift_of_a_select_of_constants_is_a_select_of_shifted_constants() {
    let mut dag = dag();
    let c = dag.get_register(EVT::I1, 5);
    let (one, two) = (dag.get_constant(1, EVT::I32), dag.get_constant(2, EVT::I32));
    let sel = dag.get_node(ISD::Select, EVT::I32, &[c, one, two]);
    let root = dag.get_node(ISD::Shl, EVT::I32, &[sel, two]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Select);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(4));
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(8));
}

/// `and (select c, 0, -1), x` took no part of x into the select; the pin
/// makes it `select c, 0, x`.
#[test]
fn an_and_of_a_select_of_zero_and_all_ones_takes_the_operand_into_the_select() {
    let mut dag = dag();
    let (c, x) = (dag.get_register(EVT::I1, 5), dag.get_register(EVT::I32, 6));
    let (zero, ones) = (dag.get_constant(0, EVT::I32), dag.get_constant(u64::MAX as u128, EVT::I32));
    let sel = dag.get_node(ISD::Select, EVT::I32, &[c, zero, ones]);
    let root = dag.get_node(ISD::And, EVT::I32, &[sel, x]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Select);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(0));
    assert_eq!(dag.operand(root, 2), x);
}

/// Declines the fold of a shift of a shared shift.
struct KeepsSharedShift;

impl CombineHooks for KeepsSharedShift {
    fn fold_shift_of_shared_shift(
        &self,
        _vt: EVT,
        _inner: u32,
    ) -> bool {
        false
    }
}

/// `(x >> 3) >> 8` where `x >> 3` has another reader: a word shift that is
/// one instruction for both halves was shifted a second time, as `x >> 11`.
fn shift_of_shift_with_a_second_reader(dag: &mut SelectionDAG) -> SDValue {
    let x = dag.get_register(EVT::I16, 5);
    let (three, eight) = (dag.get_constant(3, EVT::I8), dag.get_constant(8, EVT::I8));
    let inner = dag.get_node(ISD::Srl, EVT::I16, &[x, three]);
    let outer = dag.get_node(ISD::Srl, EVT::I16, &[inner, eight]);
    let low = dag.get_node(ISD::Truncate, EVT::I8, &[inner]);
    let high = dag.get_node(ISD::Truncate, EVT::I8, &[outer]);
    let root = dag.get_node(ISD::BuildPair, EVT::I16, &[low, high]);
    dag.set_root(root);
    inner
}

#[test]
fn a_target_that_declines_keeps_the_shift_of_a_shared_shift() {
    let mut dag = dag();
    let inner = shift_of_shift_with_a_second_reader(&mut dag);
    DagCombiner::new(&mut dag, &mut KeepsSharedShift, CombineLevel::BeforeLegalizeTypes).run();
    let high = dag.operand(dag.get_root(), 1);
    let outer = dag.operand(high, 0);
    assert_eq!(dag.operand(outer, 0), inner, "the high half reads the shift the low half has");
}

#[test]
fn a_default_target_folds_the_shift_of_a_shared_shift() {
    let mut dag = dag();
    let inner = shift_of_shift_with_a_second_reader(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let high = dag.operand(dag.get_root(), 1);
    assert_ne!(dag.operand(dag.operand(high, 0), 0), inner);
}

struct RefusesToCommute;

impl CombineHooks for RefusesToCommute {
    fn is_desirable_to_commute_with_shift(
        &self,
        _dag: &SelectionDAG,
        _shift: SDValue,
        _level: CombineLevel,
    ) -> bool {
        false
    }
}

/// `(x - 1) << 1`, the add read once. A wasm address `p + ((x - 1) << 1)` next
/// to `p + (x << 1)` was rebuilt per use (dot -O2: +32 B on the pin's own IR).
fn shifted_add_of_a_register(dag: &mut SelectionDAG) -> SDValue {
    let x = dag.get_register(EVT::I32, 5);
    let (minus_one, one) = (dag.get_constant(u128::from(u32::MAX), EVT::I32), dag.get_constant(1, EVT::I32));
    let add = dag.get_node(ISD::Add, EVT::I32, &[x, minus_one]);
    let shl = dag.get_node(ISD::Shl, EVT::I32, &[add, one]);
    dag.set_root(shl);
    add
}

#[test]
fn a_shift_of_a_one_use_add_commutes_into_an_add_of_a_shift() {
    let mut dag = dag();
    shifted_add_of_a_register(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Add, "(x << 1) + (-1 << 1)");
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Shl);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(u128::from(u32::MAX - 1)));
}

#[test]
fn a_shift_of_an_add_with_another_reader_keeps_its_add() {
    let mut dag = dag();
    let add = shifted_add_of_a_register(&mut dag);
    let shl = dag.get_root();
    let both = dag.get_node(ISD::Xor, EVT::I32, &[shl, add]);
    dag.set_root(both);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let shl = dag.operand(dag.get_root(), 0);
    assert_eq!(dag.opcode(shl), ISD::Shl);
    assert_eq!(dag.opcode(dag.operand(shl, 0)), ISD::Add, "the add has two readers: the target's default says no");
}

#[test]
fn a_target_that_refuses_keeps_the_shift_of_the_add() {
    let mut dag = dag();
    shifted_add_of_a_register(&mut dag);
    DagCombiner::new(&mut dag, &mut RefusesToCommute, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Shl);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Add);
}

/// A target with an and-not instruction for every non-constant `i32`, as
/// AArch64's `bic`.
struct AndNot;

impl CombineHooks for AndNot {
    fn has_and_not(
        &self,
        vt: EVT,
        constant: bool,
    ) -> bool {
        vt == EVT::I32 && !constant
    }
}

fn masked_merge(dag: &mut SelectionDAG) {
    let (x, y, m) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6), dag.get_register(EVT::I32, 7));
    let diff = dag.get_node(ISD::Xor, EVT::I32, &[x, y]);
    let masked = dag.get_node(ISD::And, EVT::I32, &[diff, m]);
    let root = dag.get_node(ISD::Xor, EVT::I32, &[masked, y]);
    dag.set_root(root);
}

/// `((x ^ y) & m) ^ y` stayed three operations; with an and-not it is `(x & m)
/// | (y & ~m)`.
#[test]
fn a_masked_merge_is_unfolded_where_the_target_has_and_not() {
    let mut dag = dag();
    masked_merge(&mut dag);
    DagCombiner::new(&mut dag, &mut AndNot, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Or);
}

#[test]
fn a_masked_merge_stays_where_the_target_has_no_and_not() {
    let mut dag = dag();
    masked_merge(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Xor);
}

fn and_with_not_plus(dag: &mut SelectionDAG) {
    let (x, y, z) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6), dag.get_register(EVT::I32, 7));
    let not_y = dag.get_not(y, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[not_y, z]);
    let root = dag.get_node(ISD::And, EVT::I32, &[x, sum]);
    dag.set_root(root);
}

/// `x & (~y + z)` is `x & ~(y - z)`: the `not` moves out so `bic` takes it.
#[test]
fn an_and_with_a_sum_of_a_not_takes_the_not_out_where_the_target_has_and_not() {
    let mut dag = dag();
    and_with_not_plus(&mut dag);
    DagCombiner::new(&mut dag, &mut AndNot, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let rhs = dag.operand(root, 1);
    assert_eq!((dag.opcode(root), dag.opcode(rhs)), (ISD::And, ISD::Xor), "{rhs:?}");
    assert_eq!(dag.opcode(dag.operand(rhs, 0)), ISD::Sub);
}

#[test]
fn an_and_with_a_sum_of_a_not_stays_where_the_target_has_no_and_not() {
    let mut dag = dag();
    and_with_not_plus(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.operand(dag.get_root(), 1)), ISD::Add);
}

/// `select_cc` on a compare of two constants made `get_setcc` fold the compare to a constant, and `visit_setcc` read
/// the operands of that constant: `tests/fixtures/c/bitfield` panicked (`index out of bounds`, `dag.rs:1202`) at O1 on
/// arm64. The pin's `SimplifySelectCC` takes the arm the constant compare names.
#[test]
fn a_select_cc_on_a_constant_compare_is_the_arm_it_names() {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let (one, two) = (dag.get_constant(1, EVT::I32), dag.get_constant(2, EVT::I32));
    let cc = dag.get_cond_code(CondCode::SetLt);
    let root = dag.get_node(ISD::SelectCC, EVT::I32, &[one, two, x, y, cc]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.get_root(), x, "1 < 2 picks the true arm");
}

fn frozen_exact_shift(dag: &mut SelectionDAG) {
    let x = dag.get_register(EVT::I32, 5);
    let amount = dag.get_constant(24, EVT::I32);
    let shifted = dag.get_node_flags(ISD::Sra, EVT::I32, &[x, amount], SDNodeFlags::EXACT);
    let root = dag.get_node(ISD::Freeze, EVT::I32, &[shifted]);
    dag.set_root(root);
}

/// `freeze (sra exact x, 24)` of a register stayed a freeze, which kept the
/// `sext_inreg` of a byte divide's operand (`div8_os` -O3 on wasm, one
/// `extend16_s` per iteration); the pin drops the flag and the freeze.
#[test]
fn a_frozen_shift_by_a_constant_is_the_shift_of_a_defined_register() {
    let mut dag = dag();
    frozen_exact_shift(&mut dag);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Sra, "{}", dag.dump());
}

#[test]
fn a_target_that_does_not_ask_keeps_the_frozen_shift() {
    let mut dag = dag();
    frozen_exact_shift(&mut dag);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::Freeze);
}

/// `freeze (trunc nsw (assertsext x))`: the assertion may be poison, so the
/// freeze moves under the truncation, which is no longer `nsw`.
#[test]
fn a_freeze_of_a_truncation_moves_to_its_operand() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let ty = dag.get_value_type_node(EVT::I8);
    let asserted = dag.get_node(ISD::AssertSext, EVT::I32, &[x, ty]);
    let narrow = dag.get_node_flags(ISD::Truncate, EVT::I16, &[asserted], SDNodeFlags::NO_SIGNED_WRAP);
    let root = dag.get_node(ISD::Freeze, EVT::I16, &[narrow]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut Asks, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Truncate, "{}", dag.dump());
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Freeze, "{}", dag.dump());
    assert_eq!(dag.node(root).flags, SDNodeFlags::default(), "the flag is dropped");
}
