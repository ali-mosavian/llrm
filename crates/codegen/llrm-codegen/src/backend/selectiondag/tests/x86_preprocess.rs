use super::super::x86_preprocess::{FrameObjects, preprocess};
use super::*;
use crate::backend::selectiondag::MemBase;

struct Frame(Vec<(u32, u32)>);

impl FrameObjects for Frame {
    fn objects(&self) -> i32 {
        self.0.len() as i32
    }

    fn create_stack_temporary(
        &mut self,
        size: u32,
        align: u32,
    ) -> i32 {
        self.0.push((size, align));
        self.0.len() as i32 - 1
    }
}

fn double_in_a_register(dag: &mut SelectionDAG) -> SDValue {
    dag.get_register(EVT::Simple(MVT::F64), 5)
}

/// `(float)x` without SSE: an x87 register holds the double at full width, so
/// `fp_round` must go through a four-byte slot to round; left as a node it
/// reached the matcher as a register copy and `16777217.0` stayed itself.
#[test]
fn an_x87_fp_round_is_a_store_and_a_reload_of_the_narrower_type() {
    let mut dag = dag();
    let x = double_in_a_register(&mut dag);
    let zero = dag.get_constant_with(0, EVT::I32, true, false);
    let narrow = dag.get_node(ISD::FpRound, EVT::Simple(MVT::F32), &[x, zero]);
    dag.set_root(narrow);
    let mut frame = Frame(Vec::new());
    assert_eq!(preprocess(&mut dag, &mut frame, false), 1);
    assert_eq!(frame.0, vec![(4, 4)]);
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::Simple(MVT::F32)));
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a load") };
    assert_eq!((info.mem_vt, info.base), (EVT::Simple(MVT::F32), MemBase::Stack(0)));
    let store = SDValue { node: dag.operand(root, 0).node, res: 0 };
    assert_eq!(dag.opcode(store), ISD::Store);
    assert_eq!(dag.operand(store, 1), x, "the double is what is stored");
    let NodeData::Mem(stored) = dag.node(store).data else { panic!("a store") };
    assert_eq!(stored.mem_vt, EVT::Simple(MVT::F32), "narrowed by the store");
}

/// A rounding the front end says is exact, and an extension, stay.
#[test]
fn an_exact_round_and_an_extension_are_left_alone() {
    let mut dag = dag();
    let x = double_in_a_register(&mut dag);
    let one = dag.get_constant_with(1, EVT::I32, true, false);
    let exact = dag.get_node(ISD::FpRound, EVT::Simple(MVT::F32), &[x, one]);
    let single = dag.get_register(EVT::Simple(MVT::F32), 6);
    let wide = dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F64), &[single]);
    let both = dag.get_node(ISD::FAdd, EVT::Simple(MVT::F64), &[wide, x]);
    let widened = dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F64), &[exact]);
    let sum = dag.get_node(ISD::FAdd, EVT::Simple(MVT::F64), &[both, widened]);
    dag.set_root(sum);
    assert_eq!(preprocess(&mut dag, &mut Frame(Vec::new()), false), 0);
}

/// `fpext float to x86_fp80` of a value in an xmm register at x86-64: no
/// pattern moves an xmm register to an x87 one, and the matcher refused it
/// (`CannotSelect("FpExtend")`, every QB program that mixes the two). The pin
/// stores the float and reloads it with an extending load.
#[test]
fn an_sse_float_extended_to_an_x87_value_goes_through_a_slot() {
    let mut dag = dag();
    let single = dag.get_register(EVT::Simple(MVT::F32), 6);
    let wide = dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F80), &[single]);
    dag.set_root(wide);
    let mut frame = Frame(Vec::new());
    assert_eq!(preprocess(&mut dag, &mut frame, true), 1);
    assert_eq!(frame.0, vec![(4, 4)]);
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Load, EVT::Simple(MVT::F80)));
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a load") };
    assert_eq!((info.mem_vt, info.ext.is_some()), (EVT::Simple(MVT::F32), true), "an extending load of the float");
    // Both in xmm registers it is a legal conversion; without SSE it is a
    // no-op.
    let mut dag = dag_with_f32_to_f64();
    assert_eq!(preprocess(&mut dag, &mut Frame(Vec::new()), true), 0);
}

fn dag_with_f32_to_f64() -> SelectionDAG {
    let mut dag = dag();
    let single = dag.get_register(EVT::Simple(MVT::F32), 6);
    let wide = dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F64), &[single]);
    dag.set_root(wide);
    dag
}
