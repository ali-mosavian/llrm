use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

/// A call that returns an `i64` is split by the builder into
/// `extract_element x, 0` and `extract_element x, 1`. The operand's halves are
/// the answer: here a wide load, whose low half is at the address and high half
/// just above it. (`getNode` already folds an extract of a `build_pair`.)
#[test]
fn extract_element_of_a_wide_value_is_one_of_its_halves() {
    let (half_vt, wide_vt, step) = (EVT::I32, EVT::I64, 4);
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    {
        let mut dag = dag();
        let ptr = dag.get_register(EVT::I32, 7);
        let chain = dag.entry_node();
        let info = MemInfo {
            mem_vt: wide_vt,
            align: 4,
            volatile: false,
            base: MemBase::Stack(1),
            ext: None,
            space: 0,
            ordering: None,
        };
        let wide = dag.get_load(wide_vt, chain, ptr, info);
        let (zero, one) = (dag.get_constant(0, EVT::I32), dag.get_constant(1, EVT::I32));
        let low = dag.get_node(ISD::ExtractElement, half_vt, &[wide, zero]);
        let high = dag.get_node(ISD::ExtractElement, half_vt, &[wide, one]);
        let both = dag.get_node(ISD::Sub, half_vt, &[low, high]);
        dag.set_root(both);
        TypeLegalizer::new(&mut dag, &tli).run().unwrap();
        let root = dag.get_root();
        let (lo, hi) = (dag.operand(root, 0), dag.operand(root, 1));
        assert_eq!((dag.opcode(lo), dag.operand(lo, 1)), (ISD::Load, ptr), "the low half is at the address");
        let address = dag.operand(hi, 1);
        assert_eq!(
            (dag.opcode(hi), dag.opcode(address), dag.constant(dag.operand(address, 1))),
            (ISD::Load, ISD::Add, Some(step))
        );
    }
}
