use super::super::legalize_types::TypeLegalizer;
use super::*;

/// `select c, i1 a, i1 b` (a boolean chosen by a condition) stopped the type
/// legalizer at "Select: promote result" (4 of 32 functions of the m32
/// shadow). After: an `i8` select of the promoted arms, nothing illegal left.
#[test]
fn a_select_of_i1_values_is_an_i8_select() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b, c, d) = (
        dag.get_register(EVT::I32, 5),
        dag.get_register(EVT::I32, 6),
        dag.get_register(EVT::I32, 7),
        dag.get_register(EVT::I32, 8),
    );
    let cond = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let (x, y) = (dag.get_setcc(EVT::I1, c, d, CondCode::SetEq), dag.get_setcc(EVT::I1, c, d, CondCode::SetNe));
    let pick = dag.get_select(EVT::I1, cond, x, y);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[pick]);
    dag.set_root(wide);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
}

/// An `i1 undef` operand (QCport `sc_selftest`) stopped the type legalizer at
/// "Undef: promote result". After: an undef of the promoted type.
#[test]
fn an_i1_undef_is_an_undef_of_the_promoted_type() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let a = dag.get_register(EVT::I32, 5);
    let undef = dag.get_undef(EVT::I1);
    let flag = dag.get_node(ISD::Xor, EVT::I1, &[undef, undef]);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[flag]);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[a, wide]);
    dag.set_root(sum);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
}
