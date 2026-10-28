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
use super::dag_combiner::{CombineLevel, DagCombiner};
use super::dagisel_adapter::DagisAdapter;
use super::emit::{EmitContext, EmitState, Emitted, emit_in};
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
    state: &mut EmitState,
    first_at: i64,
    arch: &dyn llrm_target::Target,
    temporaries: &mut dyn super::x86_preprocess::FrameObjects,
) -> Result<Emitted, Refusal> {
    // The scratch slots the legalizers ask for are frame objects numbered from
    // here; they are made as the legalizers finish, before anything else is.
    dag.first_temporary = temporaries.objects();
    legalized(dag, tli, lowering, arch)?;
    for (at, (size, align)) in std::mem::take(&mut dag.temporaries).into_iter().enumerate() {
        let made = temporaries.create_stack_temporary(size, align);
        debug_assert_eq!(made, dag.first_temporary + at as i32);
    }
    // `PreprocessISelDAG`: the rewrites that need no matcher.
    super::x86_preprocess::preprocess(dag, temporaries, lowering.sse2);
    remove_assertions(dag);
    do_instruction_selection(dag, selector, hooks)?;
    let root = dag.get_root();
    let units = schedule(dag, root, info);
    emit_in(dag, &units, ctx, state, first_at).map_err(Refusal::Emit)
}

/// `SelectCodeCommon` for `AssertSext` and `AssertZext`: each is its operand.
/// DEVIATE SD43: the pin drops them while matching; here ahead of the matcher,
/// which has no id for them. Same selected code.
fn remove_assertions(dag: &mut SelectionDAG) {
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
    lowering: &mut X86Lowering,
    arch: &dyn llrm_target::Target,
) -> Result<(), Refusal> {
    DagCombiner::new(dag, lowering, CombineLevel::BeforeLegalizeTypes).with_tli(tli).run();
    let types = TypeLegalizer::new(dag, tli).with_target(arch).with_hooks(lowering).run();
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
pub(crate) fn do_instruction_selection(
    dag: &mut SelectionDAG,
    selector: &Selector,
    hooks: &mut X86Hooks,
) -> Result<(), Refusal> {
    let order = users_first(dag);
    let mut adapter = DagisAdapter::new(dag);
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
        adapter.dag.cur_order = adapter.dag.nodes()[node as usize].order;
        if select_node(selector, &mut adapter, hooks, node) == Outcome::CannotSelect {
            return Err(Refusal::CannotSelect(format!("{:?}", adapter.dag.nodes()[node as usize].opcode)));
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
