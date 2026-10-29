//! Hand-built selected DAGs of four functions and the pipeline that turns one
//! into a [`Function`] after the post-selection passes.

#![allow(dead_code)]

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
use llrm_wasm::mir::Function;
use llrm_wasm::mir::ValType::I32;
use llrm_wasm::reg_stackify::reg_stackify;
use llrm_wasm::set_p2align::set_p2align_operands;

pub const ENTRY: SDValue = SDValue { node: 0, res: 0 };

/// `LowerFormalArguments`: `ARGUMENT index`.
pub fn argument(
    dag: &mut TestDag,
    index: i64,
) -> SDValue {
    let number = dag.target_constant(index, mvt::i32);
    SDValue::new(dag.node(wasm_isd::ARGUMENT, &[mvt::i32], &[number]), 0)
}

pub fn memory(
    opcode: u16,
    vts: &[llrm_dagisel::matcher::Vt],
    ops: &[SDValue],
    load: Option<LoadExt>,
) -> Node {
    Node { opcode, vts: vts.to_vec(), ops: ops.to_vec(), load, memory_vt: Some(mvt::i32), ..Node::default() }
}

/// `LowerReturn`: `WebAssemblyISD::RETURN chain, values`.
pub fn ret(
    dag: &mut TestDag,
    chain: SDValue,
    values: &[SDValue],
) -> u32 {
    let ops: Vec<_> = std::iter::once(chain).chain(values.iter().copied()).collect();
    dag.node(wasm_isd::RETURN, &[mvt::Other], &ops)
}

/// The machine nodes under `root`, operands first: a schedule that respects
/// data and chain edges.
pub fn schedule(
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
pub fn function(
    dag: TestDag,
    name: &str,
    params: usize,
    root: u32,
) -> Function {
    pipeline(dag, name, params, root, false)
}

/// With `stackify`, the pin's -O0 order: RegStackify before ExplicitLocals.
pub fn pipeline(
    mut dag: TestDag,
    name: &str,
    params: usize,
    root: u32,
    stackify: bool,
) -> Function {
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
    if stackify {
        reg_stackify(&mut f);
    }
    explicit_locals(&mut f);
    f
}

/// `ld`, `st`, `addr` and `sum3` of `tests/golden/four.ll`, selected and
/// through the passes.
pub fn four() -> Vec<Function> {
    four_with(false)
}

/// The same four functions through [`pipeline`].
pub fn four_with(stackify: bool) -> Vec<Function> {
    let mut dag = TestDag::new();
    let p = argument(&mut dag, 0);
    let load =
        dag.add(memory(isd::LOAD, &[mvt::i32, mvt::Other], &[ENTRY, p, SDValue::new(0, 0)], Some(LoadExt::NonExt)));
    let r = ret(&mut dag, SDValue::new(load, 1), &[SDValue::new(load, 0)]);
    let ld = pipeline(dag, "ld", 1, r, stackify);

    let mut dag = TestDag::new();
    let (p, x) = (argument(&mut dag, 0), argument(&mut dag, 1));
    let store = dag.add(memory(isd::STORE, &[mvt::Other], &[ENTRY, x, p, SDValue::new(0, 0)], None));
    let r = ret(&mut dag, SDValue::new(store, 0), &[]);
    let st = pipeline(dag, "st", 2, r, stackify);

    let mut dag = TestDag::new();
    let address = dag.make_constant(64, mvt::i32);
    let load = dag.add(memory(
        isd::LOAD,
        &[mvt::i32, mvt::Other],
        &[ENTRY, address, SDValue::new(0, 0)],
        Some(LoadExt::NonExt),
    ));
    let r = ret(&mut dag, SDValue::new(load, 1), &[SDValue::new(load, 0)]);
    let addr = pipeline(dag, "addr", 0, r, stackify);

    let mut dag = TestDag::new();
    let (a, b, c) = (argument(&mut dag, 0), argument(&mut dag, 1), argument(&mut dag, 2));
    let x = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[a, b]), 0);
    let y = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[x, c]), 0);
    let r = ret(&mut dag, ENTRY, &[y]);
    let sum3 = pipeline(dag, "sum3", 3, r, stackify);

    vec![ld, st, addr, sum3]
}
