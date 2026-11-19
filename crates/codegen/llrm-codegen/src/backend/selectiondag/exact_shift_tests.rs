use super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::*;

/// `shl (srl exact x, 1), 2` kept the inner shift and a shift by 2 when the
/// `srl` had another reader (stft_single -O1 on wasm32: the shifted value
/// lived in a local). The pin's `visitSHL` exact fold makes it `shl x, 1`
/// whatever the other uses.
#[test]
fn a_shl_of_an_exact_srl_with_another_reader_is_one_shl_of_the_source() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let x = dag.get_copy_from_reg(entry, 1000, EVT::I32);
    let (one, two) = (dag.get_constant(1, EVT::I32), dag.get_constant(2, EVT::I32));
    let srl = dag.get_node_flags(ISD::Srl, EVT::I32, &[x, one], SDNodeFlags::EXACT);
    let shl = dag.get_node(ISD::Shl, EVT::I32, &[srl, two]);
    dag.global_names = vec!["g".to_string(), "h".to_string()].into();
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Global(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let (g, h) = (dag.get_global_address(EVT::I32, 0, 0), dag.get_global_address(EVT::I32, 1, 0));
    let chain = SDValue { node: x.node, res: 1 };
    let keep = dag.get_store(chain, srl, g, info);
    let out = dag.get_store(chain, shl, h, MemInfo { base: MemBase::Global(1), ..info });
    let joined = dag.get_token_factor(&[keep, out]);
    dag.set_root(joined);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let mut seen = std::collections::BTreeSet::new();
    let mut work = vec![dag.get_root().node];
    let mut shifted = Vec::new();
    while let Some(node) = work.pop() {
        if !seen.insert(node) {
            continue;
        }
        let value = SDValue { node, res: 0 };
        if dag.opcode(value) == ISD::Shl {
            shifted.push(value);
        }
        work.extend((0..dag.node(value).ops.len()).map(|at| dag.operand(value, at).node));
    }
    assert_eq!(shifted.len(), 1, "{}", dag.dump());
    assert_eq!(dag.constant(dag.operand(shifted[0], 1)), Some(1), "{}", dag.dump());
    assert_eq!(dag.opcode(dag.operand(shifted[0], 0)), ISD::CopyFromReg, "{}", dag.dump());
}

/// `srl (and x, 0xf0), 2`, shift read twice, stayed two nodes
/// (div_constants_short -O2 at m32: `movzwl; shr` where the pin has `shr;
/// and`). The pin's `visitShiftByConstant` makes it `and (srl x, 2), 0x3c` when
/// the operand is a register copy and the shift has another reader.
#[test]
fn a_shift_of_an_and_of_a_shift_is_the_and_of_one_shift() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let x = dag.get_copy_from_reg(entry, 1000, EVT::I32);
    let c = |dag: &mut SelectionDAG, v| dag.get_constant(v, EVT::I32);
    let (three, two, mask) = (c(&mut dag, 3), c(&mut dag, 2), c(&mut dag, 0xf0));
    let _ = three;
    let and = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
    let outer = dag.get_node(ISD::Srl, EVT::I32, &[and, two]);
    dag.global_names = vec!["g".to_string(), "h".to_string()].into();
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Global(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let (g, h) = (dag.get_global_address(EVT::I32, 0, 0), dag.get_global_address(EVT::I32, 1, 0));
    let keep = dag.get_store(SDValue { node: x.node, res: 1 }, outer, g, info);
    let again = dag.get_store(SDValue { node: x.node, res: 1 }, outer, h, MemInfo { base: MemBase::Global(1), ..info });
    let root = dag.get_token_factor(&[keep, again]);
    dag.set_root(root);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    let store = (0..dag.node(root).ops.len())
        .map(|at| dag.operand(root, at))
        .chain([root])
        .find(|&v| dag.opcode(v) == ISD::Store)
        .unwrap_or_else(|| panic!("no store: {}", dag.dump()));
    let value = dag.operand(store, 1);
    assert_eq!(dag.opcode(value), ISD::And, "{}", dag.dump());
    assert_eq!(dag.constant(dag.operand(value, 1)), Some(0x3c), "{}", dag.dump());
    let shift = dag.operand(value, 0);
    assert_eq!(dag.opcode(shift), ISD::Srl, "{}", dag.dump());
    assert_eq!(dag.constant(dag.operand(shift, 1)), Some(2), "{}", dag.dump());
}
