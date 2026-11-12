use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;
use crate::backend::selectiondag::MemBase;

/// A `switch` the builder turns into a jump table ends in `br_jt`, which the
/// legalizer refused ("convert node to libcall", 3 functions of the m32
/// shadow). After: `brind (load (table + index << 2))` of the entry, the
/// table address a `Wrapper (TargetJumpTable)`.
#[test]
fn a_jump_table_branch_loads_the_entry_and_branches_to_it() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let chain = dag.entry_node();
    let table = dag.get_jump_table(EVT::I32, 3);
    let index = dag.get_register(EVT::I32, 7);
    let branch = dag.get_node(ISD::BrJT, EVT::OTHER, &[chain, table, index]);
    dag.set_root(branch);
    DagLegalizer::new(
        &mut dag,
        &tli,
        &mut X86Lowering {
            multiply_early_out: None,
            imul_if_smaller: false, hot_chains: false,
            share_products: false,
            cmov: false,
            sse2: false,
            min_size: false,
            three_address_adds: false,
            i16_mul_chains: false,
            sext_load: false,
            zext_free: false,
            branch_constant_selects: false,
            profile: None,
        },
    )
    .run()
    .unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::BrInd);
    let entry = dag.operand(root, 1);
    assert_eq!(dag.opcode(entry), ISD::Load);
    let NodeData::Mem(info) = dag.node(entry).data else { panic!("a load") };
    assert_eq!((info.mem_vt, info.base), (EVT::I32, MemBase::JumpTable));
    assert_eq!(dag.operand(root, 0), SDValue { node: entry.node, res: 1 }, "the branch follows the load");
    let address = dag.operand(entry, 1);
    assert_eq!(dag.opcode(address), ISD::Add);
    let (base, scaled) = (dag.operand(address, 0), dag.operand(address, 1));
    assert_eq!((dag.opcode(base), dag.opcode(dag.operand(base, 0))), (x86isd::WRAPPER, ISD::TargetJumpTable));
    assert_eq!((dag.opcode(scaled), dag.constant(dag.operand(scaled, 1))), (ISD::Shl, Some(2)));
    assert_eq!(dag.operand(scaled, 0), index);
}

/// m64 position-independent code (the pin's clang default) has a jump table
/// of 32-bit offsets from the table: the pin loads the entry sign-extended,
/// adds the table's address and branches (`EK_LabelDifference32`). The
/// absolute form, an `i64` load at `table + index << 3`, needs a 32-bit
/// absolute relocation a PIE refuses.
#[test]
fn an_i64_jump_table_branch_adds_the_sign_extended_offset_to_the_table() {
    let tli = TargetLowering::x86_64(Features::default(), MVT::I64);
    let mut dag = SelectionDAG::new(DagConfig { pointer: EVT::I64, ..DagConfig::default() });
    let chain = dag.entry_node();
    let table = dag.get_jump_table(EVT::I64, 3);
    let index = dag.get_register(EVT::I64, 7);
    let branch = dag.get_node(ISD::BrJT, EVT::OTHER, &[chain, table, index]);
    dag.set_root(branch);
    DagLegalizer::new(
        &mut dag,
        &tli,
        &mut X86Lowering {
            multiply_early_out: None,
            imul_if_smaller: false, hot_chains: false,
            share_products: false,
            cmov: false,
            sse2: false,
            min_size: false,
            three_address_adds: false,
            i16_mul_chains: false,
            sext_load: false,
            zext_free: false,
            branch_constant_selects: false,
            profile: None,
        },
    )
    .run()
    .unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::BrInd);
    let target = dag.operand(root, 1);
    assert_eq!(dag.opcode(target), ISD::Add);
    let (entry, base) = (dag.operand(target, 0), dag.operand(target, 1));
    assert_eq!(dag.opcode(entry), ISD::Load);
    let NodeData::Mem(info) = dag.node(entry).data else { panic!("a load") };
    assert_eq!(
        (info.mem_vt, info.ext, info.base),
        (EVT::I32, Some(crate::backend::selectiondag::actions::LoadExt::SExtLoad), MemBase::JumpTable)
    );
    assert_eq!((dag.opcode(base), dag.opcode(dag.operand(base, 0))), (x86isd::WRAPPER_RIP, ISD::TargetJumpTable));
    let address = dag.operand(entry, 1);
    let scaled = dag.operand(address, 1);
    assert_eq!((dag.opcode(scaled), dag.constant(dag.operand(scaled, 1))), (ISD::Shl, Some(2)));
}
