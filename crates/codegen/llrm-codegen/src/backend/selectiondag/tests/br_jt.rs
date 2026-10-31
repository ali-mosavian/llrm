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
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false },
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
