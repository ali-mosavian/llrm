//! A call leaves the condition codes undefined, but the descriptions of `CALLb`
//! and the others list only `SP`, and the pin's `RegisterMask` is the preserved
//! set, which leaves `CCR` out. Without `CCR` in the call's mask the scheduler
//! let a compare sit above a call with its branch below it, and the branch read
//! the flags the callee left (arm64 `fib` printed 5148 for 6765; z80 and arm64
//! name their flags in the mask for this).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::{Unit, schedule};
use llrm_codegen::backend::selectiondag::{DagConfig, NodeData, SDValue, SelectionDAG};
use llrm_m68k::dagisel::call::{Ext, M68kCalling, Out};
use llrm_m68k::dagisel::generated::ids::{opcode as ids, reg as pin};
use llrm_m68k::dagisel::pipeline::M68kSched;
use llrm_m68k::reg;

fn machine(
    dag: &mut SelectionDAG,
    op: u16,
    values: &[EVT],
    ops: &[SDValue],
) -> SDValue {
    dag.get_node_multi(ISD::Machine(u32::from(op)), values, ops)
}

fn position(
    order: &[Unit],
    value: SDValue,
) -> usize {
    order.iter().position(|unit| unit.nodes.contains(&value.node)).expect("scheduled")
}

/// `g(a)` lowered by the target: the chain after the call and the registers its
/// `RegisterMask` operand names.
fn lowered_call_mask() -> Vec<u16> {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = dag.get_register(EVT::I32, 1);
    let callee = dag.get_global_address(EVT::I32, 7, 0);
    let argument = Out { value: a, ext: Ext::Any, pointer: false, arg_vt: EVT::I32, split: false, variadic: false, sret: false, byval: None };
    M68kCalling::default().lower_call(&mut dag, entry, callee, &[argument], &[]);
    let masks: Vec<Vec<u16>> = dag
        .nodes()
        .iter()
        .filter(|node| node.opcode == ISD::RegisterMask)
        .map(|node| match node.data {
            NodeData::RegMask(at) => dag.register_masks[at as usize].clone(),
            _ => panic!("a mask without its registers"),
        })
        .collect();
    assert_eq!(masks.len(), 1, "premise: one call, one mask: {masks:?}");
    masks.into_iter().next().unwrap()
}

#[test]
fn a_call_register_mask_names_the_condition_codes() {
    let mask = lowered_call_mask();
    assert!(mask.contains(&pin::CCR), "the call's mask leaves CCR out: {mask:?}");
}

/// `s = a + b; call g(s); beq`: the sum is the call's argument, so it is
/// scheduled before the call, and the branch (chained after the call) reads the
/// flags it set, which the call clobbered: the add is scheduled again after the
/// call. The call's mask is the one the target's lowering makes.
#[test]
fn the_flags_a_branch_reads_are_set_after_the_call_before_it() {
    let mask = lowered_call_mask();
    for source_order in [false, true] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        let a = dag.get_register(EVT::I32, 1);
        let b = dag.get_register(EVT::I32, 2);
        let add = machine(&mut dag, ids::ADD32dd, &[EVT::I32, EVT::I32], &[a, b]);
        let to_flags = dag.get_copy_to_phys_reg(entry, reg::CCR, SDValue { node: add.node, res: 1 }, None);
        let mask = dag.get_register_mask_clobbering(&mask);
        let call = machine(&mut dag, ids::CALLb, &[EVT::OTHER], &[entry, SDValue { node: add.node, res: 0 }, mask]);
        let branch = machine(&mut dag, ids::Beq8, &[EVT::OTHER], &[call, SDValue { node: to_flags.node, res: 1 }]);
        let order = schedule(&mut dag, branch, &M68kSched { source_order });
        let adds: Vec<usize> = (0..order.len()).filter(|&at| order[at].nodes.contains(&add.node)).collect();
        let (at_call, at_branch) = (position(&order, call), position(&order, branch));
        let last_add = *adds.last().expect("the add is scheduled");
        assert!(at_call < at_branch, "premise, source_order {source_order}: {order:?}");
        assert!(adds[0] < at_call, "premise: the add feeds the call, source_order {source_order}: {order:?}");
        assert!(last_add > at_call, "the add's flags are read across the call, source_order {source_order}: {order:?}");
    }
}
