//! The matcher interpreter over our own DAG: the C1 address and fold cases.

use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};

use super::dagisel_adapter::DagisAdapter;
use super::dag::{MemBase, MemInfo};
use super::isd::ISD;
use super::mvt::EVT;
use super::{DagConfig, SDValue, SelectionDAG};

fn hooks() -> X86Hooks {
    X86Hooks::new(Options { cpu: Cpu::I486, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: llrm_dagisel::x86_addr::AddrMode::Flat32 })
}

fn mem_info() -> MemInfo {
    MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Unknown }
}

fn input(dag: &mut SelectionDAG, reg: u32) -> SDValue {
    let entry = dag.entry_node();
    dag.get_copy_from_reg(entry, reg, EVT::I32)
}

/// The pin's name of the machine instruction a node became.
fn machine_name(dag: &SelectionDAG, node: u32) -> &'static str {
    let ISD::Machine(code) = dag.nodes()[node as usize].opcode else { panic!("node {node} is not a machine node") };
    table::MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map_or("?", |(_, n)| n)
}

/// `a + ((i + 2) << 2)`: the address of `a[i + 2]`.
fn element_address(dag: &mut SelectionDAG) -> (SDValue, SDValue, SDValue) {
    let a = input(dag, 40);
    let i = input(dag, 41);
    let two = dag.get_constant(2, EVT::I32);
    let shift = dag.get_constant(2, EVT::I8);
    let i2 = dag.get_node(ISD::Add, EVT::I32, &[i, two]);
    let scaled = dag.get_node(ISD::Shl, EVT::I32, &[i2, shift]);
    let address = dag.get_node(ISD::Add, EVT::I32, &[a, scaled]);
    (address, a, i)
}

#[test]
fn a_load_of_a_index_plus_two_is_one_mov32rm_with_the_folded_operands() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (address, a, i) = element_address(&mut dag);
    let entry = dag.entry_node();
    let load = dag.get_load(EVT::I32, entry, address, mem_info());
    let mut adapter = DagisAdapter::new(&mut dag);

    let outcome = Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), load.node);

    assert_eq!(outcome, Outcome::Selected);
    assert_eq!(machine_name(&dag, load.node), "MOV32rm");
    let ops = &dag.nodes()[load.node as usize].ops;
    assert_eq!((ops[0], ops[2]), (a, i), "base and index");
    assert_eq!(dag.constant(ops[1]), Some(4), "scale");
    assert_eq!(dag.constant(ops[3]), Some(8), "the +2, scaled by 4");
}

#[test]
fn add_of_two_registers_is_add32rr() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = input(&mut dag, 40);
    let b = input(&mut dag, 41);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), sum.node), Outcome::Selected);

    assert_eq!(machine_name(&dag, sum.node), "ADD32rr");
}

#[test]
fn a_node_the_table_has_no_pattern_for_cannot_be_selected() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = input(&mut dag, 40);
    let weird = dag.get_node(ISD::Target(9999), EVT::I32, &[a]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), weird.node), Outcome::CannotSelect);
}

#[test]
fn the_select_loop_drops_a_shift_counts_plus_thirty_two_in_our_dag() {
    use llrm_dagisel::x86_select::select_node;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let x = input(&mut dag, 40);
    let entry = dag.entry_node();
    let n = dag.get_copy_from_reg(entry, 41, EVT::I8);
    let thirty_two = dag.get_constant(32, EVT::I8);
    let count = dag.get_node(ISD::Add, EVT::I8, &[n, thirty_two]);
    let shift = dag.get_node(ISD::Shl, EVT::I32, &[x, count]);
    let mut adapter = DagisAdapter::new(&mut dag);

    let outcome = select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), shift.node);

    assert_eq!(outcome, Outcome::Selected);
    let copied = dag.nodes().iter().find_map(|node| {
        (node.opcode == ISD::CopyToReg && matches!(dag.nodes()[node.ops[1].node as usize].data, super::NodeData::Register(r) if r == u32::from(llrm_dagisel::generated::ids::x86::CL)))
            .then(|| node.ops[2])
    });
    assert_eq!(copied, Some(n), "CL gets n, not n + 32");
}

#[test]
fn jump_table_external_symbol_and_register_mask_nodes_have_the_pins_numbers() {
    use super::dagisel_adapter::pin_opcode;
    use llrm_dagisel::generated::ids::isd as pin;
    assert_eq!(pin_opcode(ISD::RegisterMask), pin::RegisterMask);
    assert_eq!(pin_opcode(ISD::JumpTable), pin::JumpTable);
    assert_eq!(pin_opcode(ISD::TargetExternalSymbolNode), pin::TargetExternalSymbol);
}
