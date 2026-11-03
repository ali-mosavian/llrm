//! `AssertZext` / `AssertSext`: what an argument's `zeroext`/`signext` says
//! about the register it arrives in.

use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::known_bits::{known_bits, num_sign_bits};
use super::super::x86_lowering::X86Lowering;
use super::*;

fn asserted(
    dag: &mut SelectionDAG,
    op: ISD,
    from: EVT,
) -> SDValue {
    let x = dag.get_register(EVT::I32, 5);
    let ty = dag.get_value_type_node(from);
    dag.get_node(op, EVT::I32, &[x, ty])
}

#[test]
fn an_assert_zext_knows_its_high_bits_are_zero_and_an_assert_sext_counts_its_copies() {
    let mut dag = dag();
    let zero = asserted(&mut dag, ISD::AssertZext, EVT::I16);
    assert_eq!(known_bits(&dag, zero).zero, 0xFFFF_0000);
    assert_eq!(num_sign_bits(&dag, zero), 16);
    let sign = asserted(&mut dag, ISD::AssertSext, EVT::I8);
    assert_eq!(num_sign_bits(&dag, sign), 25);
}

#[test]
fn a_mask_of_what_an_assert_zext_holds_is_dropped() {
    let mut dag = dag();
    let zero = asserted(&mut dag, ISD::AssertZext, EVT::I16);
    let mask = dag.get_constant(0xFFFF, EVT::I32);
    let and = dag.get_node(ISD::And, EVT::I32, &[zero, mask]);
    dag.set_root(and);
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(
        dag.opcode(dag.get_root()),
        ISD::AssertZext,
        "the `and` was the zero extension the assert already holds"
    );
}
