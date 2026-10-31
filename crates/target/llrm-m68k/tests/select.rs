//! The m68k matcher table (tblgen's `M68kGenDAGISel.inc`) against hand-built
//! DAGs.

use llrm_dagisel::dag::{LoadExt, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::testing::{Node, TestDag};
use llrm_m68k::dagisel::generated::m68k_m68000 as table;
use llrm_m68k::dagisel::hooks::{Cpu, M68kHooks};

fn machine_name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

/// Before the target seam, the only table the selector could read was x86's: an
/// m68k DAG had no table at all. `i32 add` of two registers is `ADD32dr`, as
/// the pin's llc picks (`-print-after=finalize-isel`).
#[test]
fn add_of_two_registers_is_add32dr() {
    let mut dag = TestDag::new();
    let a = dag.input(1, mvt::i32);
    let b = dag.input(2, mvt::i32);
    let add = dag.node(isd::ADD, &[mvt::i32], &[a, b]);
    let selector = Selector::new(table::TABLE);

    assert_eq!(selector.select(&mut dag, &mut M68kHooks::new(Cpu::M68000, false), add), Outcome::Selected);

    assert_eq!(machine_name(dag.nodes[add as usize].machine.expect("not selected")), "ADD32dr");
}

/// An `i32` load from `address`.
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

fn select_load(address: impl FnOnce(&mut TestDag) -> SDValue) -> &'static str {
    let mut dag = TestDag::new();
    let address = address(&mut dag);
    let node = load(&mut dag, address);
    let selector = Selector::new(table::TABLE);
    assert_eq!(selector.select(&mut dag, &mut M68kHooks::new(Cpu::M68000, false), node), Outcome::Selected);
    machine_name(dag.nodes[node as usize].machine.expect("not selected"))
}

/// The pin's llc (`-O2`, `-print-after=finalize-isel`) selects `load i32, ptr
/// %p` as `MOV32rj` (address register indirect).
#[test]
fn a_load_through_a_register_is_mov32rj() {
    assert_eq!(select_load(|dag| dag.input(1, mvt::i32)), "MOV32rj");
}

/// Same oracle: `load (gep i8, %p, 8)` is `MOV32rp`, base and 16-bit
/// displacement.
#[test]
fn a_load_at_base_plus_constant_is_mov32rp() {
    let name = select_load(|dag| {
        let base = dag.input(1, mvt::i32);
        let eight = dag.make_constant(8, mvt::i32);
        SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[base, eight]), 0)
    });
    assert_eq!(name, "MOV32rp");
}

/// Same oracle: `load (gep i32, %p, %i)` after the shift is `MOV32rf`, base
/// plus index with a zero displacement (a load parent allows it).
#[test]
fn a_load_at_base_plus_index_is_mov32rf() {
    let name = select_load(|dag| {
        let (base, index) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[base, index]), 0)
    });
    assert_eq!(name, "MOV32rf");
}
