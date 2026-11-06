//! `narrow_setcc_by_known_bits` (DEVIATE, m68k): a compare of a value whose
//! upper bits are known zero against a constant that fits is made at `i8` or
//! `i16`, for eq/ne and the unsigned conditions. The pin narrows only from an
//! explicit extension; hex tested `v & 15 < 10` with `cmpi.l`.

use super::super::dag_combiner::{CombineHooks, CombineLevel, NoCombines, combine};
use super::*;

struct Narrow;
impl CombineHooks for Narrow {
    fn narrow_setcc_by_known_bits(&self) -> bool {
        true
    }
}

/// The type the compare `(x & mask) cond bound` ends up comparing in.
fn compared_in(
    on: bool,
    mask: u128,
    cond: CondCode,
    bound: u128,
) -> EVT {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let x = dag.get_register(EVT::I32, 7);
    let m = dag.get_constant(mask, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[x, m]);
    let c = dag.get_constant(bound, EVT::I32);
    let test = dag.get_setcc(EVT::I1, masked, c, cond);
    dag.set_root(test);
    if on {
        combine(&mut dag, &mut Narrow, CombineLevel::BeforeLegalizeTypes);
    } else {
        combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    }
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::SetCC, "premise: the root is still a compare");
    dag.value_type(dag.operand(root, 0))
}

#[test]
fn a_masked_byte_below_a_constant_is_compared_as_a_byte() {
    for cond in [CondCode::SetULt, CondCode::SetULe, CondCode::SetUGt, CondCode::SetUGe, CondCode::SetNe] {
        assert_eq!(compared_in(true, 15, cond, 10), EVT::I8, "{cond:?}");
    }
    assert_eq!(compared_in(false, 15, CondCode::SetULt, 10), EVT::I32, "premise: without the hook it stays i32");
}

#[test]
fn a_constant_too_big_for_a_byte_is_compared_as_a_word() {
    assert_eq!(compared_in(true, 511, CondCode::SetULt, 300), EVT::I16);
}

#[test]
fn a_constant_the_known_bits_do_not_hold_stays_wide() {
    assert_eq!(compared_in(true, 15, CondCode::SetULt, 0x1_0000), EVT::I32);
    assert_eq!(compared_in(true, 0xFFFF_FFFF, CondCode::SetULt, 10), EVT::I32, "the upper bits are not known zero");
}

/// A signed order of the narrow values would need both below 2^(w-1), which
/// the pin's own rules leave alone ("todo, be more careful with signed
/// comparisons"). The IR level cannot show this: InstCombine turns a signed
/// compare of a non-negative value into an unsigned one before the backend.
#[test]
fn a_signed_condition_is_not_narrowed() {
    for cond in [CondCode::SetLt, CondCode::SetLe, CondCode::SetGt, CondCode::SetGe] {
        assert_eq!(compared_in(true, 15, cond, 10), EVT::I32, "{cond:?}");
    }
}
