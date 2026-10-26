use super::super::legalize_types::TypeLegalizer;
use super::*;

/// `icmp eq i1 a, b` (or `ne`, `ult` ...) stopped the type legalizer at
/// "SetCC: promote operand" (3 functions of the m32 shadow): the operands are
/// promoted and extended in the register to the compare's signedness.
#[test]
fn a_compare_of_i1_operands_is_a_compare_of_promoted_ones() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    for code in [CondCode::SetEq, CondCode::SetNe, CondCode::SetULt, CondCode::SetLt] {
        let mut dag = dag();
        let (a, b) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
        let (bit_a, bit_b) = (dag.get_node(ISD::Truncate, EVT::I1, &[a]), dag.get_node(ISD::Truncate, EVT::I1, &[b]));
        let test = dag.get_setcc(EVT::I8, bit_a, bit_b, code);
        dag.set_root(test);
        let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
        legalizer.run().unwrap_or_else(|why| panic!("{code:?}: {why:?}"));
        assert!(legalizer.all_legal(), "{code:?}");
    }
}
