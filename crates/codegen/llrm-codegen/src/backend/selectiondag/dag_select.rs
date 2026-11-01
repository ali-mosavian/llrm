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

use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_hooks::X86Hooks;
use llrm_dagisel::x86_select::select_node;
use llrm_support::hash::HashSet;

use super::actions::TargetLowering;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::dag_target::{DagTarget, Phases, Select, select_units};
use super::dagisel_adapter::{DagisAdapter, PinRegisters};
use super::emit::{EmitContext, EmitState, Emitted, emit_in};
use super::isd::ISD;
use super::legalize_dag;
use super::legalize_dag::DagLegalizer;
use super::legalize_types::TypeLegalizer;
use super::schedule::SchedInfo;
use super::x86_lowering::X86Lowering;
use super::{SDValue, SelectionDAG};

/// Why a block did not get through the DAG path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Legalize(String),
    /// No pattern selected the node (`CannotYetSelect`).
    CannotSelect(String),
    /// The selector named a register the target has no id for.
    UnknownRegister(String),
    Emit(super::emit::EmitError),
}

/// x86 as a [`DagTarget`]: its lowering tables, hooks and selector tables.
pub struct X86Target<'a, 'p> {
    pub tli: &'a TargetLowering,
    pub lowering: &'a mut X86Lowering<'p>,
    pub chooser: X86Chooser<'a>,
    pub arch: &'a dyn llrm_target::Target,
}

/// x86's `Select`: the pin's hand matchers, then the generated table.
pub struct X86Chooser<'a> {
    pub selector: &'a Selector,
    pub hooks: &'a mut X86Hooks,
}

impl Select for X86Chooser<'_> {
    fn select(
        &mut self,
        adapter: &mut DagisAdapter<'_>,
        node: u32,
    ) -> Outcome {
        select_node(self.selector, adapter, self.hooks, node)
    }
}

impl<'p> DagTarget for X86Target<'_, 'p> {
    type Lowering = X86Lowering<'p>;

    fn phases(&mut self) -> Phases<'_, X86Lowering<'p>> {
        Phases { tli: self.tli, lowering: &mut *self.lowering, arch: Some(self.arch) }
    }

    fn selection(&mut self) -> (&dyn PinRegisters, &mut dyn Select) {
        (&super::dagisel_adapter::X86Registers, &mut self.chooser)
    }

    fn preprocess(
        &mut self,
        dag: &mut SelectionDAG,
        temporaries: &mut dyn super::x86_preprocess::FrameObjects,
    ) {
        super::x86_preprocess::preprocess(dag, temporaries, self.lowering.sse2);
    }
}

/// The pin's pipeline for one block's DAG.
pub fn select_block(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    lowering: &mut X86Lowering<'_>,
    selector: &Selector,
    hooks: &mut X86Hooks,
    info: &dyn SchedInfo,
    ctx: &EmitContext<'_>,
    state: &mut EmitState,
    first_at: i64,
    arch: &dyn llrm_target::Target,
    temporaries: &mut dyn super::x86_preprocess::FrameObjects,
) -> Result<Emitted, Refusal> {
    let units = select_units(
        dag,
        &mut X86Target { tli, lowering, chooser: X86Chooser { selector, hooks }, arch },
        info,
        temporaries,
    )?;
    emit_in(dag, &units, ctx, state, first_at).map_err(Refusal::Emit)
}

/// `SelectCodeCommon` for `AssertSext` and `AssertZext`: each is its operand.
/// DEVIATE SD43: the pin drops them while matching; here ahead of the matcher,
/// which has no id for them. Same selected code.
pub fn remove_assertions(dag: &mut SelectionDAG) {
    for node in dag.node_ids().collect::<Vec<_>>() {
        let value = SDValue { node, res: 0 };
        if matches!(dag.opcode(value), ISD::AssertSext | ISD::AssertZext) {
            let operand = dag.operand(value, 0);
            dag.replace_all_uses_with(value, operand);
        }
    }
}

/// `CodeGenAndEmitDAG` up to selection: `Combine` at each of three levels
/// around the two legalizers. The select shadow calls this same function.
pub fn legalized(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    lowering: &mut X86Lowering<'_>,
    arch: &dyn llrm_target::Target,
) -> Result<(), Refusal> {
    legalized_with(dag, tli, lowering, Some(arch))
}

/// [`legalized`] for any target's lowering hooks. `arch` is the target whose
/// inline routines the `i128` divide calls; a target without any passes `None`.
pub fn legalized_with<L: CombineHooks + legalize_dag::Hooks>(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    lowering: &mut L,
    arch: Option<&dyn llrm_target::Target>,
) -> Result<(), Refusal> {
    DagCombiner::new(dag, lowering, CombineLevel::BeforeLegalizeTypes).with_tli(tli).run();
    let mut types = TypeLegalizer::new(dag, tli);
    if let Some(arch) = arch {
        types = types.with_target(arch);
    }
    let types = types.with_hooks(lowering).run();
    types.map_err(|e| refused_legalize(dag, &format!("{e:?}")))?;
    DagCombiner::new(dag, lowering, CombineLevel::AfterLegalizeTypes).with_tli(tli).run();
    let legal = DagLegalizer::new(dag, tli, lowering).run();
    legal.map_err(|e| refused_legalize(dag, &format!("{e:?}")))?;
    DagCombiner::new(dag, lowering, CombineLevel::AfterLegalizeDag).with_tli(tli).run();
    // `LLRM_DAGDUMP=all` prints every block's DAG as selection will see it.
    if llrm_support::env_var("LLRM_DAGDUMP").is_some_and(|mode| mode == "all") {
        eprintln!("legalized:\n{}", dag.dump());
    }
    Ok(())
}

/// A legalizer refusal; with `LLRM_DAGDUMP` set, the DAG it stopped on is
/// printed to stderr first.
fn refused_legalize(
    dag: &SelectionDAG,
    why: &str,
) -> Refusal {
    if llrm_support::env_flag!("LLRM_DAGDUMP") {
        eprintln!("legalize refused: {why}\n{}", dag.dump());
    }
    Refusal::Legalize(why.to_owned())
}

/// `DoInstructionSelection`: the nodes the root reaches, users before the nodes
/// they read (`allnodes` in the pin is in topological order; here creation
/// order is not, since a lowering makes the operand it replaces after its
/// user).
#[cfg(test)]
pub(crate) fn do_instruction_selection(
    dag: &mut SelectionDAG,
    selector: &Selector,
    hooks: &mut X86Hooks,
) -> Result<(), Refusal> {
    do_instruction_selection_with(dag, &super::dagisel_adapter::X86Registers, |adapter, node| {
        select_node(selector, adapter, hooks, node)
    })
}

/// `DoInstructionSelection` for any target: `select` picks the instruction for
/// one node, as the target's `Select` does, and `registers` names its physical
/// registers in the pin's numbering.
pub fn do_instruction_selection_with(
    dag: &mut SelectionDAG,
    registers: &dyn super::dagisel_adapter::PinRegisters,
    mut select: impl FnMut(&mut DagisAdapter<'_>, u32) -> Outcome,
) -> Result<(), Refusal> {
    let order = users_first(dag);
    let mut adapter = DagisAdapter::with_registers(dag, registers);
    // Nodes a selection makes (a shift amount rewritten, a mask shrunk) are
    // visited next, latest first: the pin inserts them before the node
    // being selected (`insertDAGNode`), and its walk runs toward the front
    // of `allnodes`. They were left as generic nodes, which the emitter
    // refused ("operand Truncate before its definition").
    let mut made: Vec<u32> = Vec::new();
    let mut rest = order.into_iter();
    while let Some(node) = made.pop().or_else(|| rest.next()) {
        // `Node->use_empty()`: a node selection left unused is dead.
        if adapter.is_dead(node) || !(node == adapter.dag.get_root().node || adapter.has_live_users(node)) {
            continue;
        }
        if matches!(adapter.dag.nodes()[node as usize].opcode, ISD::Machine(_)) {
            continue;
        }
        let before = adapter.dag.nodes().len();
        // The pin's `Select` keeps these as they are and marks them selected
        // (`SelectionDAGISel.cpp`, "Mark selected").
        if matches!(
            adapter.dag.nodes()[node as usize].opcode,
            ISD::TokenFactor | ISD::CopyFromReg | ISD::CopyToReg
        ) {
            adapter.mark_selected(node);
        }
        adapter.dag.cur_order = adapter.dag.nodes()[node as usize].order;
        if select(&mut adapter, node) == Outcome::CannotSelect {
            return Err(Refusal::CannotSelect(format!("{:?}", adapter.dag.nodes()[node as usize].opcode)));
        }
        if let Some(why) = adapter.refusal() {
            return Err(Refusal::UnknownRegister(why.to_owned()));
        }
        made.extend(before as u32..adapter.dag.nodes().len() as u32);
    }
    Ok(())
}

/// The nodes the root reaches in reverse topological order: a node comes before
/// everything it reads.
pub(crate) fn users_first(dag: &SelectionDAG) -> Vec<u32> {
    let mut order = Vec::new();
    let mut seen = HashSet::default();
    let mut stack = vec![(dag.get_root().node, false)];
    while let Some((n, expanded)) = stack.pop() {
        if expanded {
            order.push(n);
        } else if seen.insert(n) {
            stack.push((n, true));
            stack.extend(dag.node(SDValue { node: n, res: 0 }).ops.iter().map(|op| (op.node, false)));
        }
    }
    order.reverse();
    order
}
