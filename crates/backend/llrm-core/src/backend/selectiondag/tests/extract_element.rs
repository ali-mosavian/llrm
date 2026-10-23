use super::super::legalize_types::TypeLegalizer;
use super::*;

/// A call that returns an `i64` is split by the builder into
/// `extract_element x, 0` and `extract_element x, 1`; the shadow listed 92
/// m16 functions stopped there. The operand's halves are the answer.
#[test]
fn extract_element_of_a_wide_value_is_one_of_its_halves() {
    for (target_bits, half_vt, wide_vt) in [(32, EVT::I32, EVT::I64), (16, EVT::I16, EVT::I32)] {
        let tli = if target_bits == 32 {
            TargetLowering::x86_32(Features::default(), MVT::I32)
        } else {
            TargetLowering::empty(TargetLoweringInfo::x86_16())
        };
        let mut dag = dag();
        let (lo, hi) = (dag.get_register(half_vt, 5), dag.get_register(half_vt, 6));
        let pair = dag.get_node(ISD::BuildPair, wide_vt, &[lo, hi]);
        let (zero, one) = (dag.get_constant(0, EVT::I32), dag.get_constant(1, EVT::I32));
        let low = dag.get_node(ISD::ExtractElement, half_vt, &[pair, zero]);
        let high = dag.get_node(ISD::ExtractElement, half_vt, &[pair, one]);
        let both = dag.get_node(ISD::Sub, half_vt, &[low, high]);
        dag.set_root(both);
        TypeLegalizer::new(&mut dag, &tli).run().unwrap();
        let root = dag.get_root();
        assert_eq!((dag.operand(root, 0), dag.operand(root, 1)), (lo, hi), "{wide_vt:?}");
    }
}
