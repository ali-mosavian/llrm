use super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::*;

/// The DAG of `store i8 (or [disjoint] (add (mul x, 7), -21), 3)`, combined
/// before type legalization, and the root's stored value.
fn stored_after_combine(disjoint: bool) -> (SelectionDAG, SDValue) {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let x = dag.get_copy_from_reg(entry, 1000, EVT::I8);
    let c = |dag: &mut SelectionDAG, v| dag.get_constant(v, EVT::I8);
    let (seven, minus21, three) = (c(&mut dag, 7), c(&mut dag, 0xEB), c(&mut dag, 3));
    let mul = dag.get_node(ISD::Mul, EVT::I8, &[x, seven]);
    let add = dag.get_node(ISD::Add, EVT::I8, &[mul, minus21]);
    let flags = if disjoint { SDNodeFlags::DISJOINT } else { SDNodeFlags::default() };
    let or = dag.get_node_flags(ISD::Or, EVT::I8, &[add, three], flags);
    dag.global_names = vec!["g".to_string()].into();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Global(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let g = dag.get_global_address(EVT::I32, 0, 0);
    let store = dag.get_store(SDValue { node: x.node, res: 1 }, or, g, info);
    dag.set_root(store);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    // The root is a token factor over the chain and the rebuilt store.
    let mut seen = std::collections::BTreeSet::new();
    let mut work = vec![dag.get_root().node];
    let mut stores = Vec::new();
    while let Some(node) = work.pop() {
        if !seen.insert(node) {
            continue;
        }
        let value = SDValue { node, res: 0 };
        if dag.opcode(value) == ISD::Store {
            stores.push(value);
        }
        work.extend((0..dag.node(value).ops.len()).map(|at| dag.operand(value, at).node));
    }
    assert_eq!(stores.len(), 1, "{}", dag.dump());
    let stored = dag.operand(stores[0], 1);
    (dag, stored)
}

/// `visitOR` stopped before "if OR can be rewritten into ADD, try combines
/// based on ADD": `or disjoint (add x, -21), 3` stayed an `or` and a second
/// constant (`i32.const -21; i32.add; i32.const 3; i32.or` where the pin has
/// `i32.const -18; i32.add`), 186 times in tests/run/c/memory_expansion.c
/// (+191 B at wasm32 -O2).
#[test]
fn a_disjoint_or_of_an_add_with_a_constant_is_one_add() {
    let (dag, stored) = stored_after_combine(true);
    assert_eq!(dag.opcode(stored), ISD::Add, "{}", dag.dump());
    assert_eq!(dag.constant(dag.operand(stored, 1)), Some(0xEE), "{}", dag.dump());
    assert_eq!(dag.opcode(dag.operand(stored, 0)), ISD::Mul, "{}", dag.dump());
}

/// An `or` that is not add-like is not an add.
#[test]
fn an_or_that_may_share_bits_stays_an_or() {
    let (dag, stored) = stored_after_combine(false);
    assert_eq!(dag.opcode(stored), ISD::Or, "{}", dag.dump());
}
