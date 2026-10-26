use super::super::legalize_types::TypeLegalizer;
use super::*;

/// A shift of an odd width integer (a 33-bit field, `float_in_memory_loop.c`
/// at m16) stopped the type legalizer: "Srl: promote result". The value is
/// promoted with its high bits cleared (srl) or sign-filled (sra).
#[test]
fn a_shift_of_an_odd_width_integer_is_promoted() {
    let odd = EVT::Int(33);
    for (op, tli) in [
        (ISD::Srl, TargetLowering::empty(TargetLoweringInfo::x86_16())),
        (ISD::Sra, TargetLowering::empty(TargetLoweringInfo::x86_16())),
        (ISD::Shl, TargetLowering::x86_32(Features::default(), MVT::I32)),
        (ISD::Srl, TargetLowering::x86_32(Features::default(), MVT::I32)),
    ] {
        let mut dag = dag();
        let word = dag.get_register(EVT::I32, 5);
        let x = dag.get_node(ISD::ZeroExtend, odd, &[word]);
        let by = dag.get_constant(3, EVT::I8);
        let shifted = dag.get_node(op, odd, &[x, by]);
        let low = dag.get_node(ISD::Truncate, EVT::I16, &[shifted]);
        dag.set_root(low);
        let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
        legalizer.run().unwrap_or_else(|why| panic!("{op:?}: {why:?}"));
        assert!(legalizer.all_legal(), "{op:?}");
    }
}
