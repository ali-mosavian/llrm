//! The matcher interpreter over our own DAG: the C1 address and fold cases.

use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_dagisel::x86_select::select_node;

use super::dag::{MemBase, MemInfo};
use super::dagisel_adapter::DagisAdapter;
use super::isd::ISD;
use super::mvt::EVT;
use super::{DagConfig, SDValue, SelectionDAG};

fn hooks() -> X86Hooks {
    X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode: llrm_dagisel::x86_addr::AddrMode::Flat32,
    })
}

fn mem_info() -> MemInfo {
    MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Unknown, ext: None, space: 0 }
}

fn input(
    dag: &mut SelectionDAG,
    reg: u32,
) -> SDValue {
    let entry = dag.entry_node();
    dag.get_copy_from_reg(entry, reg, EVT::I32)
}

/// The pin's name of the machine instruction a node became.
fn machine_name(
    dag: &SelectionDAG,
    node: u32,
) -> &'static str {
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
    let copied = dag
        .nodes()
        .iter()
        .find_map(
            |node| (node.opcode == ISD::CopyToReg
                && matches!(
                    dag.nodes()[node.ops[1].node as usize].data,
                    super::NodeData::PhysReg(llrm_lir::registers::RegId::CL)
                ))
            .then(|| node.ops[2]),
        );
    assert_eq!(copied, Some(n), "CL gets n, not n + 32");
}

#[test]
fn jump_table_external_symbol_and_register_mask_nodes_have_the_pins_numbers() {
    use llrm_dagisel::generated::ids::isd as pin;

    use super::dagisel_adapter::pin_opcode;
    assert_eq!(pin_opcode(ISD::RegisterMask), pin::RegisterMask);
    assert_eq!(pin_opcode(ISD::JumpTable), pin::JumpTable);
    assert_eq!(pin_opcode(ISD::TargetExternalSymbolNode), pin::TargetExternalSymbol);
}

#[test]
fn a_return_node_is_selected_to_ret() {
    use super::x86isd;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let zero = dag.get_constant_with(0, EVT::I32, true, false);
    let ret = dag.get_node(x86isd::RET_GLUE, EVT::OTHER, &[entry, zero]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), ret.node), Outcome::Selected);

    assert_eq!(machine_name(&dag, ret.node), "RET");
}

#[test]
fn a_target_code_is_named_or_already_the_pins_number() {
    use llrm_dagisel::generated::ids::x86isd as pin;

    use super::dagisel_adapter::pin_opcode;
    assert_eq!(pin_opcode(super::x86isd::CALL), pin::CALL, "sdag-a's private code, by name");
    assert_eq!(pin_opcode(ISD::Target(pin::Wrapper)), pin::Wrapper, "sdag-b's lowering writes the pin's number");
}

/// The adapter numbered the pin's registers like the builder's virtual ones, so
/// EFLAGS (28) read as the builder's 28th virtual register in the emitter.
#[test]
fn the_pins_physical_registers_are_physical_register_nodes() {
    use llrm_dagisel::dag::Dag;
    use llrm_dagisel::generated::ids::x86;
    use llrm_lir::registers::RegId;

    use super::dag::NodeData;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let mut adapter = DagisAdapter::new(&mut dag);
    let eax = adapter.register(x86::EAX, llrm_dagisel::generated::ids::mvt::i32);
    let noreg = adapter.register(0, llrm_dagisel::generated::ids::mvt::i32);
    let flags = adapter.register(x86::EFLAGS, llrm_dagisel::generated::ids::mvt::i32);

    let data = |node: llrm_dagisel::dag::SDValue| dag.nodes()[node.node as usize].data;
    assert!(matches!(data(eax), NodeData::PhysReg(r) if r == RegId::EAX));
    assert!(matches!(data(noreg), NodeData::Register(0)));
    assert!(matches!(data(flags), NodeData::PhysReg(r) if r == super::dagisel_adapter::EFLAGS_ID));
}

/// The adapter said no store truncates, so a narrow store of a wide value
/// matched the plain store patterns (and `ST_Fp64m32`'s truncating pattern
/// never could).
#[test]
fn a_store_narrower_than_its_value_is_truncating() {
    use llrm_dagisel::dag::Dag;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let (value, ptr) = (input(&mut dag, 1), input(&mut dag, 2));
    let wide = dag.get_store(entry, value, ptr, mem_info());
    let narrow = dag.get_store(entry, value, ptr, MemInfo { mem_vt: EVT::I8, ..mem_info() });
    let adapter = DagisAdapter::new(&mut dag);

    assert!(!adapter.is_truncating_store(wide.node));
    assert!(adapter.is_truncating_store(narrow.node));
}

/// The adapter said no memory node had a segment override, so a load through
/// `fs:` selected as a plain `mov` (the segment operand left at `$noreg`).
#[test]
fn a_load_in_the_fs_space_addresses_through_fs() {
    use llrm_lir::registers::RegId;

    use super::dag::NodeData;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let ptr = input(&mut dag, 1);
    let load = dag.get_load(EVT::I32, entry, ptr, MemInfo { space: 257, ..mem_info() });
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), load.node), Outcome::Selected);

    let segment = dag.nodes()[load.node as usize].ops[4];
    assert!(matches!(dag.node(segment).data, NodeData::PhysReg(r) if r == RegId::FS));
}

/// The segment `les` leaves in ES is address space 259 (llrm's own, after the
/// pin's 256 to 258).
#[test]
fn a_load_in_space_259_addresses_through_es() {
    use llrm_lir::registers::RegId;

    use super::dag::NodeData;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let ptr = input(&mut dag, 1);
    let load = dag.get_load(EVT::I32, entry, ptr, MemInfo { space: 259, ..mem_info() });
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), load.node), Outcome::Selected);

    let segment = dag.nodes()[load.node as usize].ops[4];
    assert!(matches!(dag.node(segment).data, NodeData::PhysReg(r) if r == RegId::ES));
}

/// Every `f80` constant was class `Other`, so `fldz` and `fld1` never matched a
/// long double 0.0 or 1.0 and the `ConstantFP` stayed unselected
/// (long_double_variadic).
#[test]
fn an_f80_zero_and_one_are_selected_as_the_x87_constants() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let zero = dag.get_constant_fp(0, EVT::Simple(super::mvt::MVT::F80));
    let one = dag.get_constant_fp((0x3fff << 64) | (1 << 63), EVT::Simple(super::mvt::MVT::F80));
    let mut adapter = DagisAdapter::new(&mut dag);

    for (value, name) in [(zero, "LD_Fp080"), (one, "LD_Fp180")] {
        assert_eq!(
            select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), value.node),
            Outcome::Selected,
            "{name}"
        );
    }
    assert_eq!(machine_name(&dag, zero.node), "LD_Fp080");
    assert_eq!(machine_name(&dag, one.node), "LD_Fp180");
}

/// Every node of ours that the matcher passes through has an id in
/// `own_ids.txt`: `LLRM_FARCALL` had none, mapped to no opcode, and a call
/// through a function pointer was `CannotSelect(Target(24584))` (7 m16
/// programs: function_table, static_inline, static_pointers).
#[test]
fn every_own_node_has_a_pin_id() {
    use super::dagisel_adapter::pin_opcode;
    use super::x86isd::{LLRM_ASM, LLRM_CODE, LLRM_FARCALL, LLRM_PTRDIFF, LLRM_WINDOW, SEGMENT_OF};
    for node in [LLRM_ASM, LLRM_CODE, LLRM_FARCALL, LLRM_PTRDIFF, LLRM_WINDOW, SEGMENT_OF] {
        assert_ne!(pin_opcode(node), u16::MAX, "{node:?}");
    }
}
