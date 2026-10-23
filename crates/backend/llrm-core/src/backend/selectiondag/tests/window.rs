use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::x86isd;
use super::*;

/// The value of `v` when register 7 holds `offset` and register 8 `selector`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    (offset, selector): (u128, u128),
) -> u128 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), (offset, selector));
    let register = |dag: &SelectionDAG| match dag.node(v).data {
        NodeData::Register(7) => offset,
        NodeData::Register(8) => selector,
        other => panic!("register {other:?}"),
    };
    let r = match dag.opcode(v) {
        ISD::Register => register(dag),
        ISD::Constant => dag.constant(v).unwrap(),
        ISD::Srl => arg(0) >> arg(1),
        ISD::And => arg(0) & arg(1),
        ISD::Add => arg(0) + arg(1),
        other => panic!("unexpected {other:?}"),
    };
    r & 0xFFFF
}

/// `llrm.ia16.window`: whole 16-byte steps of the offset move into the
/// selector (DOS: 64K is 0x1000 selector steps). The pointer's address,
/// `selector * 16 + offset`, must not change, and the offset ends below 16.
#[test]
fn a_window_moves_whole_paragraphs_of_the_offset_into_the_selector() {
    let tli = TargetLowering::empty(TargetLoweringInfo::x86_16());
    let mut dag = dag();
    let (offset, selector) = (dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, MVT::Far.into(), &[offset, selector]);
    let window = dag.get_node(x86isd::LLRM_WINDOW, MVT::Far.into(), &[far]);
    let pair = dag.get_node(ISD::BitCast, EVT::I32, &[window]);
    dag.set_root(pair);
    TypeLegalizer::new(&mut dag, &tli).with_target(&llrm_x86_m16::M16).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::BuildPair);
    let (low, high) = (dag.operand(root, 0), dag.operand(root, 1));
    for (off, sel, want_off, want_sel) in
        [(0x1234u128, 0x1000u128, 0x0004u128, 0x1123u128), (0xFFFF, 0x2000, 0x000F, 0x2FFF), (0x000F, 5, 0xF, 5), (0x0010, 0, 0, 1)]
    {
        let (got_off, got_sel) = (eval(&dag, low, (off, sel)), eval(&dag, high, (off, sel)));
        assert_eq!((got_off, got_sel), (want_off, want_sel), "window of {sel:#x}:{off:#x}");
        assert_eq!(got_sel * 16 + got_off, sel * 16 + off, "the address is kept");
    }
}
