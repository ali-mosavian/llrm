//! Selected DAG to assembly text: the whole chain of this crate against the
//! text of `llc -mtriple=wasm32 -O0 -fast-isel=false` (system LLVM 20.1.8,
//! `tests/golden/*.s`, one function each).
//!
//! The nodes are scheduled in the order given; the pin's scheduler is not bound
//! to a wasm DAG yet.

use llrm_dagisel::dag::{Dag, LoadExt, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::testing::{Node, TestDag};
use llrm_wasm::argument_move::argument_move;
use llrm_wasm::dagisel::generated::ids::isd as wasm_isd;
use llrm_wasm::dagisel::generated::wasm_wasm32 as table;
use llrm_wasm::dagisel::hooks::{Cpu, WasmHooks};
use llrm_wasm::emit::emit;
use llrm_wasm::explicit_locals::explicit_locals;
use llrm_wasm::mir::ValType::I32;
use llrm_wasm::print;
use llrm_wasm::set_p2align::set_p2align_operands;

const ENTRY: SDValue = SDValue { node: 0, res: 0 };

/// `LowerFormalArguments`: `ARGUMENT index`.
fn argument(
    dag: &mut TestDag,
    index: i64,
) -> SDValue {
    let number = dag.target_constant(index, mvt::i32);
    SDValue::new(dag.node(wasm_isd::ARGUMENT, &[mvt::i32], &[number]), 0)
}

fn memory(
    opcode: u16,
    vts: &[llrm_dagisel::matcher::Vt],
    ops: &[SDValue],
    load: Option<LoadExt>,
) -> Node {
    Node { opcode, vts: vts.to_vec(), ops: ops.to_vec(), load, memory_vt: Some(mvt::i32), ..Node::default() }
}

/// `LowerReturn`: `WebAssemblyISD::RETURN chain, values`.
fn ret(
    dag: &mut TestDag,
    chain: SDValue,
    values: &[SDValue],
) -> u32 {
    let ops: Vec<_> = std::iter::once(chain).chain(values.iter().copied()).collect();
    dag.node(wasm_isd::RETURN, &[mvt::Other], &ops)
}

/// The machine nodes under `root`, operands first: a schedule that respects
/// data and chain edges.
fn schedule(
    dag: &TestDag,
    root: u32,
) -> Vec<u32> {
    fn visit(
        dag: &TestDag,
        node: u32,
        seen: &mut Vec<bool>,
        out: &mut Vec<u32>,
    ) {
        if std::mem::replace(&mut seen[node as usize], true) {
            return;
        }
        for i in 0..dag.num_operands(node) {
            visit(dag, dag.operand(node, i).node, seen, out);
        }
        if dag.is_machine(node) {
            out.push(node);
        }
    }
    let mut out = Vec::new();
    visit(dag, root, &mut vec![false; dag.nodes.len() + 8], &mut out);
    out
}

/// Selects every node reachable from `root` (operands first), then prints the
/// function.
fn text(
    mut dag: TestDag,
    name: &str,
    params: usize,
    root: u32,
) -> String {
    let selector = Selector::new(table::TABLE);
    let mut hooks = WasmHooks::new(Cpu::Wasm32, false);
    let mut pending = Vec::new();
    fn collect(
        dag: &TestDag,
        node: u32,
        seen: &mut Vec<bool>,
        out: &mut Vec<u32>,
    ) {
        if std::mem::replace(&mut seen[node as usize], true) {
            return;
        }
        for i in 0..dag.num_operands(node) {
            collect(dag, dag.operand(node, i).node, seen, out);
        }
        out.push(node);
    }
    collect(&dag, root, &mut vec![false; dag.nodes.len()], &mut pending);
    for node in pending {
        if matches!(
            dag.nodes[node as usize].opcode,
            isd::EntryToken | isd::TargetConstant | isd::Constant | isd::CopyFromReg | isd::Register
        ) {
            continue;
        }
        assert_eq!(selector.select(&mut dag, &mut hooks, node), Outcome::Selected, "node {node}");
    }
    let order = schedule(&dag, root);
    let results = usize::from(name != "st");
    let mut f = emit(&dag, name, &vec![I32; params], &vec![I32; results], &order);
    argument_move(&mut f);
    set_p2align_operands(&mut f);
    explicit_locals(&mut f);
    print::function(&f)
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/golden/{name}.s", env!("CARGO_MANIFEST_DIR"))).expect("golden file")
}

#[test]
fn a_load_of_its_argument_is_llcs_text() {
    let mut dag = TestDag::new();
    let p = argument(&mut dag, 0);
    let load =
        dag.add(memory(isd::LOAD, &[mvt::i32, mvt::Other], &[ENTRY, p, SDValue::new(0, 0)], Some(LoadExt::NonExt)));
    let r = ret(&mut dag, SDValue::new(load, 1), &[SDValue::new(load, 0)]);

    assert_eq!(text(dag, "ld", 1, r), golden("ld"));
}

#[test]
fn a_store_through_its_argument_is_llcs_text() {
    let mut dag = TestDag::new();
    let (p, x) = (argument(&mut dag, 0), argument(&mut dag, 1));
    let store = dag.add(memory(isd::STORE, &[mvt::Other], &[ENTRY, x, p, SDValue::new(0, 0)], None));
    let r = ret(&mut dag, SDValue::new(store, 0), &[]);

    assert_eq!(text(dag, "st", 2, r), golden("st"));
}

/// The address 64 folds into the load's offset; the base is `CONST_I32 0`.
#[test]
fn a_load_at_a_constant_address_is_llcs_text() {
    let mut dag = TestDag::new();
    let address = dag.make_constant(64, mvt::i32);
    let load = dag.add(memory(
        isd::LOAD,
        &[mvt::i32, mvt::Other],
        &[ENTRY, address, SDValue::new(0, 0)],
        Some(LoadExt::NonExt),
    ));
    let r = ret(&mut dag, SDValue::new(load, 1), &[SDValue::new(load, 0)]);

    assert_eq!(text(dag, "addr", 0, r), golden("addr"));
}

#[test]
fn two_adds_of_three_arguments_are_llcs_text() {
    let mut dag = TestDag::new();
    let (a, b, c) = (argument(&mut dag, 0), argument(&mut dag, 1), argument(&mut dag, 2));
    let x = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[a, b]), 0);
    let y = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[x, c]), 0);
    let r = ret(&mut dag, ENTRY, &[y]);

    assert_eq!(text(dag, "sum3", 3, r), golden("sum3"));
}
