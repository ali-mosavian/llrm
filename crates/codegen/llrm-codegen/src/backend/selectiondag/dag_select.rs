//! The DAG path of one block: legalize, select, schedule, emit.
//!
//! # How it works
//! [`select_block`] runs the pin's `CodeGenAndEmitDAG` order over a built
//! block: combine, legalize types, combine, legalize the DAG, combine, then
//! `DoInstructionSelection`: every node the root reaches,
//! last created first, goes to `llrm_x86_dagisel::x86_select::select_node`
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

use llrm_dagisel::interp::Outcome;
use llrm_support::hash::HashSet;

use super::actions::TargetLowering;
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::dagisel_adapter::DagisAdapter;
use super::emit_api::{EmitContext, EmitState, Emitted};
use super::isd::ISD;
use super::legalize_dag;
use super::legalize_dag::DagLegalizer;
use super::legalize_types::TypeLegalizer;
use super::schedule::{SchedInfo, Unit, schedule};
use super::{SDValue, SelectionDAG};

/// Why a block did not get through the DAG path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Legalize(String),
    /// No pattern selected the node (`CannotYetSelect`).
    CannotSelect(String),
    /// The selector named a register the target has no id for.
    UnknownRegister(String),
    Emit(super::emit_api::EmitError),
}

/// [`select_block`] for a DAG FastISel already selected: scheduled and emitted.
pub fn select_block_fast(
    dag: &mut SelectionDAG,
    info: &dyn SchedInfo,
    ctx: &EmitContext<'_>,
    state: &mut EmitState,
    first_at: i64,
) -> Result<Emitted, Refusal> {
    let root = dag.get_root();
    let units = crate::support::debug::timed("dag schedule", || schedule(dag, root, info));
    crate::support::debug::timed("dag emit", || emit_units(dag, &units, ctx, state, first_at)).map_err(Refusal::Emit)
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

/// `CodeGenAndEmitDAG` up to selection for any target's lowering hooks:
/// `Combine` at each of three levels around the two legalizers. `arch` is the
/// target whose inline routines the `i128` divide calls; a target without any
/// passes `None`.
pub fn legalized_with<L: CombineHooks + legalize_dag::Hooks>(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    lowering: &mut L,
    arch: Option<&dyn llrm_target::Target>,
    opt_none: bool,
) -> Result<(), Refusal> {
    let combiner = |dag: &mut SelectionDAG, lowering: &mut L, level| {
        let combiner = DagCombiner::new(dag, lowering, level).with_tli(tli);
        if opt_none { combiner.with_opt_none() } else { combiner }
            .run_bounded()
            .map_err(|runaway| Refusal::Legalize(runaway.to_string()))
    };
    combiner(dag, lowering, CombineLevel::BeforeLegalizeTypes)?;
    let mut types = TypeLegalizer::new(dag, tli);
    if let Some(arch) = arch {
        types = types.with_target(arch);
    }
    let types = types.with_hooks(lowering).run();
    types.map_err(|e| refused_legalize(dag, &format!("{e:?}")))?;
    combiner(dag, lowering, CombineLevel::AfterLegalizeTypes)?;
    let legal = DagLegalizer::new(dag, tli, lowering).run();
    legal.map_err(|e| refused_legalize(dag, &format!("{e:?}")))?;
    combiner(dag, lowering, CombineLevel::AfterLegalizeDag)?;
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
    // `insertDAGNode` puts a node before the one being matched, which is
    // an operand of the node being selected: the walk reaches it after that
    // node's users. One whose users are not all selected waits.
    let mut next = |adapter: &DagisAdapter<'_>, made: &mut Vec<u32>| match made
        .iter()
        .rposition(|&m| adapter.is_dead(m) || !adapter.has_pending_user(m))
    {
        Some(at) => Some(made.remove(at)),
        None => rest.next().or_else(|| made.pop()),
    };
    while let Some(node) = next(&adapter, &mut made) {
        // `Node->use_empty()`: a node selection left unused is dead.
        if adapter.is_dead(node) || !(node == adapter.dag.get_root().node || adapter.has_live_users(node)) {
            continue;
        }
        if matches!(adapter.dag.nodes()[node as usize].opcode, ISD::Machine(_)) {
            continue;
        }
        // `SelectCodeCommon` for `AssertSext` and `AssertZext`: each is its
        // operand, once the users that read what it asserts are selected
        // (a pattern predicate sees the assertion; `smull` of a value
        // another block extended). Targets that call `remove_assertions`
        // first have none left.
        if matches!(
            adapter.dag.nodes()[node as usize].opcode,
            ISD::AssertSext | ISD::AssertZext
        ) {
            adapter.drop_assertion(node);
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
            // The identity patterns of `X86InstrVecCompiler.td` (`(v2i64 (bitconvert (v4i32 VR128:$src))) ->
            // $src`), for a target whose matcher has none: a bitcast of a vector to another of the same size
            // changes the type the users matched on, not the register.
            let at = SDValue { node, res: 0 };
            if adapter.dag.nodes()[node as usize].opcode == ISD::BitCast {
                let (to, from) = (adapter.dag.value_type(at), adapter.dag.value_type(adapter.dag.operand(at, 0)));
                if to.is_vector() && from.is_vector() && to.size_in_bits() == from.size_in_bits() {
                    adapter.drop_assertion(node);
                    continue;
                }
            }
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

/// The installed backend's emitter. Without one nothing emits LIR: a
/// refusal, never a skipped block.
pub fn emit_units(
    dag: &SelectionDAG,
    units: &[Unit],
    ctx: &EmitContext<'_>,
    state: &mut EmitState,
    first_at: i64,
) -> Result<Emitted, super::emit_api::EmitError> {
    match crate::backend::hooks::installed() {
        Some(hooks) => hooks.emit_units(dag, units, ctx, state, first_at),
        None => {
            Err(super::emit_api::EmitError::Unsupported("no backend installed (llrm_x86_backend::install)".to_owned()))
        }
    }
}
