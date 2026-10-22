//! The DAG path of one block: legalize, select, schedule, emit.
//!
//! # How it works
//! [`select_block`] runs the pin's `CodeGenAndEmitDAG` order over a built
//! block: combine, legalize types, combine, legalize the DAG, combine, then
//! `DoInstructionSelection`: every node the root reaches,
//! last created first, goes to `llrm_dagisel::x86_select::select_node`
//! (the pin's `Select` hand matchers, then the generated table). The
//! machine DAG is scheduled ([`super::schedule`]) and emitted
//! ([`super::emit`]) as LIR blocks.
//!
//! ```text
//! built DAG -> combine -> TypeLegalizer -> combine -> DagLegalizer -> combine
//!   -> select_node (reverse) -> schedule -> emit
//! ```
//!
//! # Example
//! `ret (a + b)` with both arguments in registers: `CopyFromReg` x2, `ADD32rr`,
//! `CopyToReg EAX`, `RET`; LIR `v3 := add v1, v2` and a move to EAX.
//!
//! # Runs
//! The shadow harness (`--isel=dag`) calls it per block.
//!
//! # See also
//! `SelectionDAGISel::CodeGenAndEmitDAG`
//! ([L941](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L941)),
//! `DoInstructionSelection`
//! ([L1281](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L1281)).

use std::collections::HashSet;

use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_hooks::X86Hooks;
use llrm_dagisel::x86_select::select_node;

use super::actions::TargetLowering;
use super::dag_combiner::{CombineLevel, DagCombiner};
use super::dagisel_adapter::DagisAdapter;
use super::emit::{EmitContext, Emitted, emit};
use super::isd::ISD;
use super::legalize_dag::DagLegalizer;
use super::legalize_types::TypeLegalizer;
use super::schedule::{SchedInfo, schedule};
use super::x86_lowering::X86Lowering;
use super::{SDValue, SelectionDAG};

/// Why a block did not get through the DAG path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Legalize(String),
    /// No pattern selected the node (`CannotYetSelect`).
    CannotSelect(String),
    Emit(super::emit::EmitError),
}

/// The pin's pipeline for one block's DAG.
pub fn select_block(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    lowering: &mut X86Lowering,
    selector: &Selector,
    hooks: &mut X86Hooks,
    info: &dyn SchedInfo,
    ctx: &EmitContext<'_>,
) -> Result<Emitted, Refusal> {
    legalized(dag, tli, lowering)?;
    do_instruction_selection(dag, selector, hooks)?;
    let root = dag.get_root();
    let units = schedule(dag, root, info);
    emit(dag, &units, ctx).map_err(Refusal::Emit)
}

/// `CodeGenAndEmitDAG` up to selection: `Combine` at each of three levels
/// around the two legalizers. The select shadow calls this same function.
pub fn legalized(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    lowering: &mut X86Lowering,
) -> Result<(), Refusal> {
    DagCombiner::new(dag, lowering, CombineLevel::BeforeLegalizeTypes).run();
    TypeLegalizer::new(dag, tli).run().map_err(|e| Refusal::Legalize(format!("{e:?}")))?;
    DagCombiner::new(dag, lowering, CombineLevel::AfterLegalizeTypes).run();
    DagLegalizer::new(dag, tli, lowering).run().map_err(|e| Refusal::Legalize(format!("{e:?}")))?;
    DagCombiner::new(dag, lowering, CombineLevel::AfterLegalizeDag).run();
    Ok(())
}

/// `DoInstructionSelection`: the nodes the root reaches, last created first.
fn do_instruction_selection(
    dag: &mut SelectionDAG,
    selector: &Selector,
    hooks: &mut X86Hooks,
) -> Result<(), Refusal> {
    let reachable = reachable_from_root(dag);
    let count = dag.nodes().len() as u32;
    let mut adapter = DagisAdapter::new(dag);
    for node in (0..count).rev() {
        // `Node->use_empty()`: a node selection left unused is dead.
        if !reachable.contains(&node)
            || adapter.is_dead(node)
            || !(node == adapter.dag.get_root().node || adapter.has_live_users(node))
        {
            continue;
        }
        if matches!(adapter.dag.nodes()[node as usize].opcode, ISD::Machine(_)) {
            continue;
        }
        if select_node(selector, &mut adapter, hooks, node) == Outcome::CannotSelect {
            return Err(Refusal::CannotSelect(format!("{:?}", adapter.dag.nodes()[node as usize].opcode)));
        }
    }
    Ok(())
}

fn reachable_from_root(dag: &SelectionDAG) -> HashSet<u32> {
    let mut seen = HashSet::new();
    let mut work = vec![dag.get_root().node];
    while let Some(n) = work.pop() {
        if seen.insert(n) {
            work.extend(dag.node(SDValue { node: n, res: 0 }).ops.iter().map(|op| op.node));
        }
    }
    seen
}
