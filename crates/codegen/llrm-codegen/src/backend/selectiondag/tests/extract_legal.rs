use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;

/// `extract_element x:i32, 0/1` of a register type (an `i32` at m16 is legal)
/// reached selection as `CannotSelect(ExtractElement)` (ia16_abi, wc16_mixed,
/// register_upper_half of the m16 run): the node lies about being legal, and
/// is a truncate, of a shift for the high half.
#[test]
fn extracting_a_half_of_a_legal_wide_value_is_a_truncate_of_a_shift() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    for (which, shifted) in [(0u128, false), (1, true)] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let index = dag.get_constant(which, EVT::I16);
        let half = dag.get_node(ISD::ExtractElement, EVT::I16, &[x, index]);
        dag.set_root(half);
        DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false })
            .run()
            .unwrap();
        let root = dag.get_root();
        assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Truncate, EVT::I16), "half {which}");
        let inner = dag.operand(root, 0);
        if shifted {
            assert_eq!((dag.opcode(inner), dag.operand(inner, 0)), (ISD::Srl, x));
            assert_eq!(dag.constant(dag.operand(inner, 1)), Some(16));
        } else {
            assert_eq!(inner, x);
        }
    }
}
