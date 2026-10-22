//! The interpreter against the generated i486 table, on hand-built DAGs.

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt};
use crate::generated::x86_i486 as table;
use crate::interp::{Outcome, Selector};
use crate::testing::{Node, TestDag, TestHooks};

fn machine_name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

/// The machine opcode of the node `value` now is.
fn selected(dag: &TestDag, node: u32) -> &'static str {
    machine_name(dag.nodes[node as usize].machine.expect("not selected"))
}

#[test]
fn add_of_two_registers_is_add32rr() {
    let mut dag = TestDag::new();
    let a = dag.input(1, mvt::i32);
    let b = dag.input(2, mvt::i32);
    let add = dag.node(isd::ADD, &[mvt::i32], &[a, b]);
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: None };

    assert_eq!(selector.select(&mut dag, &mut hooks, add), Outcome::Selected);

    assert_eq!(selected(&dag, add), "ADD32rr");
    assert_eq!(dag.nodes[add as usize].ops, [a, b]);
}

#[test]
fn a_load_through_an_address_pattern_is_mov32rm_with_five_address_operands() {
    let mut dag = TestDag::new();
    let base = dag.input(1, mvt::i32);
    let index = dag.input(2, mvt::i32);
    let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[base, index]), 0);
    let load = dag.add(Node {
        opcode: isd::LOAD,
        vts: vec![mvt::i32, mvt::Other],
        ops: vec![SDValue::new(0, 0), address, SDValue::new(0, 0)],
        load: Some(crate::dag::LoadExt::NonExt),
        memory_vt: Some(mvt::i32),
        ..Node::default()
    });
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: Some((base, index, 4, 8)) };

    assert_eq!(selector.select(&mut dag, &mut hooks, load), Outcome::Selected);

    assert_eq!(selected(&dag, load), "MOV32rm");
    let ops = &dag.nodes[load as usize].ops;
    assert_eq!(ops.len(), 6, "base, scale, index, disp, segment, chain");
    assert_eq!(ops[0], base);
    assert_eq!(ops[2], index);
    assert_eq!(dag.nodes[ops[1].node as usize].constant, Some(4));
    assert_eq!(dag.nodes[ops[3].node as usize].constant, Some(8));
}

#[test]
fn a_node_no_pattern_covers_cannot_be_selected() {
    let mut dag = TestDag::new();
    let a = dag.input(1, mvt::i32);
    let odd = dag.node(9999, &[mvt::i32], &[a]);
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: None };

    assert_eq!(selector.select(&mut dag, &mut hooks, odd), Outcome::CannotSelect);
}

fn emits(opcodes: &[(u16, &str)], prefix: &str) -> bool {
    opcodes.iter().any(|(_, name)| name.split('|').any(|n| n.starts_with(prefix)))
}

#[test]
fn neither_cpu_emits_cmov_and_only_p5_emits_cmpxchg8b() {
    use crate::generated::x86_p5;
    // `CMOV_GR32` and kin are the pseudos that become a branch; `CMOV32rr` is the instruction.
    for (cpu, opcodes) in [("i486", table::MACHINE_OPCODES), ("P5", x86_p5::MACHINE_OPCODES)] {
        assert!(emits(opcodes, "CMOV_GR32"), "{cpu}: select is the branch pseudo");
        assert!(!emits(opcodes, "CMOV32") && !emits(opcodes, "CMOV16"), "{cpu} has no CMOV instruction");
    }
    assert!(!emits(table::MACHINE_OPCODES, "LCMPXCHG8B"), "i486 has no cx8");
    assert!(emits(x86_p5::MACHINE_OPCODES, "LCMPXCHG8B"), "P5 has cx8");
    assert!(emits(table::MACHINE_OPCODES, "ADD32rr"), "the table must not be empty of the basics");
}
