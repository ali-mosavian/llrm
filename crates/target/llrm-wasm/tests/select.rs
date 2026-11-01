//! The wasm32 matcher table (tblgen's `WebAssemblyGenDAGISel.inc`) against
//! hand-built DAGs. The oracle is `llc -mtriple=wasm32 -O0 -fast-isel=false
//! -print-after=finalize-isel` (system LLVM 20.1.8).

use llrm_dagisel::dag::{LoadExt, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::testing::{Node, TestDag};
use llrm_wasm::dagisel::generated::wasm_wasm32 as table;
use llrm_wasm::dagisel::hooks::{Cpu, WasmHooks};

fn machine_name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

fn select(
    dag: &mut TestDag,
    node: u32,
) -> &'static str {
    let selector = Selector::new(table::TABLE);
    assert_eq!(selector.select(dag, &mut WasmHooks::new(Cpu::Wasm32, false), node), Outcome::Selected);
    machine_name(dag.nodes[node as usize].machine.expect("not selected"))
}

/// The machine node `value` is, as its name and the constant operands (`None`
/// for an operand that is not a constant).
fn describe(
    dag: &TestDag,
    value: SDValue,
) -> (&'static str, Vec<Option<i64>>) {
    let node = &dag.nodes[value.node as usize];
    let name = node.machine.map_or("?", machine_name);
    (name, node.ops.iter().map(|op| dag.nodes[op.node as usize].constant).collect())
}

/// Before the target seam the only table the selector could read was x86's: a
/// wasm DAG had none. `add` of two registers is `ADD_I32` or `ADD_I64`.
#[test]
fn add_of_two_registers_is_add_of_its_width() {
    for (vt, name) in [(mvt::i32, "ADD_I32"), (mvt::i64, "ADD_I64")] {
        let mut dag = TestDag::new();
        let a = dag.input(1, vt);
        let b = dag.input(2, vt);
        let add = dag.node(isd::ADD, &[vt], &[a, b]);

        assert_eq!(select(&mut dag, add), name);
    }
}

fn load(
    dag: &mut TestDag,
    address: SDValue,
) -> u32 {
    dag.add(Node {
        opcode: isd::LOAD,
        vts: vec![mvt::i32, mvt::Other],
        ops: vec![SDValue::new(0, 0), address, SDValue::new(0, 0)],
        load: Some(LoadExt::NonExt),
        memory_vt: Some(mvt::i32),
        ..Node::default()
    })
}

/// The selected load's `(offset, base)`: the operands that follow `p2align`.
fn load_address(address: impl FnOnce(&mut TestDag) -> SDValue) -> (Option<i64>, (&'static str, Vec<Option<i64>>)) {
    let mut dag = TestDag::new();
    let address = address(&mut dag);
    let node = load(&mut dag, address);
    assert_eq!(select(&mut dag, node), "LOAD_I32_A32");
    let ops = dag.nodes[node as usize].ops.clone();
    let base = ops[2];
    (dag.nodes[ops[1].node as usize].constant, describe(&dag, base))
}

/// The oracle selects `load i32, ptr %p` as `LOAD_I32_A32 _, 0, %p`.
#[test]
fn a_load_through_a_register_has_offset_zero() {
    let (offset, base) = load_address(|dag| dag.input(1, mvt::i32));
    assert_eq!(offset, Some(0));
    assert_eq!(base.0, "?", "the base is the register itself");
}

/// Oracle: `load i32, ptr inttoptr (i32 64 to ptr)` is `CONST_I32 0` as the
/// base and 64 as the offset.
#[test]
fn a_load_at_a_constant_address_puts_it_in_the_offset() {
    let (offset, base) = load_address(|dag| dag.make_constant(64, mvt::i32));
    assert_eq!(offset, Some(64));
    assert_eq!(base, ("CONST_I32", vec![Some(0)]));
}

/// A store takes the same address operands: `STORE_I32_A32 _, 0, %p, %x`.
#[test]
fn a_store_through_a_register_is_store_with_offset_zero() {
    let mut dag = TestDag::new();
    let (p, x) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
    let store = dag.add(Node {
        opcode: isd::STORE,
        vts: vec![mvt::Other],
        ops: vec![SDValue::new(0, 0), x, p, SDValue::new(0, 0)],
        memory_vt: Some(mvt::i32),
        ..Node::default()
    });

    assert_eq!(select(&mut dag, store), "STORE_I32_A32");
    let ops = &dag.nodes[store as usize].ops;
    assert_eq!(dag.nodes[ops[1].node as usize].constant, Some(0));
}

/// Oracle: `gep inbounds i8, %p, 8` is an `add nuw`, folded into offset 8.
#[test]
fn an_add_nuw_folds_its_constant_into_the_offset() {
    let (offset, base) = load_address(|dag| {
        let (p, eight) = (dag.input(1, mvt::i32), dag.make_constant(8, mvt::i32));
        let add = dag.node(isd::ADD, &[mvt::i32], &[p, eight]);
        dag.nodes[add as usize].no_unsigned_wrap = true;
        SDValue::new(add, 0)
    });
    assert_eq!(offset, Some(8));
    assert_eq!(base.0, "?", "the base is the register itself");
}

/// The offset is unsigned with infinite precision: an add that may wrap does
/// not fold. Before the nuw query the fold fired never, so this pins the
/// other side of it.
#[test]
fn an_add_that_may_wrap_keeps_its_offset_zero() {
    let (offset, _) = load_address(|dag| {
        let (p, eight) = (dag.input(1, mvt::i32), dag.make_constant(8, mvt::i32));
        SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[p, eight]), 0)
    });
    assert_eq!(offset, Some(0));
}

/// `or p, 8` where `p` is masked to `0x100`: the bits are disjoint, so it is
/// an add and the offset folds.
#[test]
fn an_or_of_disjoint_bits_folds_the_offset() {
    let (offset, _) = load_address(|dag| {
        let (p, mask) = (dag.input(1, mvt::i32), dag.make_constant(0x100, mvt::i32));
        let and = dag.node(isd::AND, &[mvt::i32], &[p, mask]);
        let eight = dag.make_constant(8, mvt::i32);
        SDValue::new(dag.node(isd::OR, &[mvt::i32], &[SDValue::new(and, 0), eight]), 0)
    });
    assert_eq!(offset, Some(8));
}

/// A shift by a register of its own width: `llc` gives `SHL_I32`, `SHR_S_I32`,
/// `SHR_U_I32` and the `_I64` forms.
#[test]
fn a_shift_is_the_shift_of_its_width() {
    for (opcode, vt, name) in [
        (isd::SHL, mvt::i32, "SHL_I32"),
        (isd::SRA, mvt::i32, "SHR_S_I32"),
        (isd::SRL, mvt::i32, "SHR_U_I32"),
        (isd::SHL, mvt::i64, "SHL_I64"),
    ] {
        let mut dag = TestDag::new();
        let a = dag.input(1, vt);
        let b = dag.input(2, vt);
        let shift = dag.node(opcode, &[vt], &[a, b]);

        assert_eq!(select(&mut dag, shift), name);
    }
}
