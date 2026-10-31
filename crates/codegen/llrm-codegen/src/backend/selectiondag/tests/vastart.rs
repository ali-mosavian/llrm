use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;

fn legalize(dag: &mut SelectionDAG) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    DagLegalizer::new(dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false })
        .run()
        .unwrap();
}

fn list(
    dag: &mut SelectionDAG,
    index: i32,
) -> SDValue {
    dag.get_frame_index(EVT::I32, index)
}

/// `va_start(ap)` at 32 bits stores the address of the first variadic
/// argument through `ap`; the pin's legalized DAG of `sum(int n, ...)` is
/// `store t0, FrameIndex<-2>, FrameIndex<0>`. Left as a `vastart` the
/// matcher has no pattern for it.
#[test]
fn va_start_stores_the_first_variadic_arguments_address() {
    let mut dag = dag();
    dag.var_args_frame_index = Some(-2);
    let ap = list(&mut dag, 0);
    let chain = dag.entry_node();
    let start = dag.get_node(ISD::VAStart, EVT::OTHER, &[chain, ap]);
    dag.set_root(start);
    legalize(&mut dag);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Store);
    assert_eq!((dag.operand(root, 0), dag.operand(root, 2)), (chain, ap));
    let stored = dag.operand(root, 1);
    assert_eq!(dag.node(stored).data, NodeData::FrameIndex(-2));
    let NodeData::Mem(info) = dag.node(root).data else { panic!("a store") };
    assert_eq!(info.mem_vt, EVT::I32);
}

/// `va_end` does nothing and `va_copy` loads the source list and stores it to
/// the destination (`t19 = load ap; store t19, bp`).
#[test]
fn va_end_is_its_chain_and_va_copy_is_a_load_and_a_store() {
    let mut dag = dag();
    let (src, dest) = (list(&mut dag, 0), list(&mut dag, 1));
    let chain = dag.entry_node();
    let copied = dag.get_node(ISD::VACopy, EVT::OTHER, &[chain, dest, src]);
    dag.set_root(copied);
    legalize(&mut dag);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Store);
    let loaded = dag.operand(root, 1);
    assert_eq!((dag.opcode(loaded), dag.operand(loaded, 1)), (ISD::Load, src));
    assert_eq!(dag.operand(root, 2), dest);

    let mut dag2 = self::dag();
    let ap = list(&mut dag2, 0);
    let chain = dag2.entry_node();
    let end = dag2.get_node(ISD::VAEnd, EVT::OTHER, &[chain, ap]);
    dag2.set_root(end);
    legalize(&mut dag2);
    assert_eq!(dag2.get_root(), chain);
}
