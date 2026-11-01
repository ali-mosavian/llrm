//! What a target gives the shared DAG path, and the path itself.
//!
//! # How it works
//! [`select_units`] is the pin's `CodeGenAndEmitDAG` up to the end of
//! scheduling: combine and legalize through the target's hooks, make the
//! scratch frame objects they asked for, the target's rewrites that need no
//! matcher ([`DagTarget::preprocess`]), `DoInstructionSelection` through
//! [`DagTarget::select`], the target's rewrites of what it selected
//! ([`DagTarget::postprocess`]), then schedule. What follows is the target's:
//! x86 emits LIR from the units ([`super::dag_select::select_block`]), another
//! target emits into its own machine function.
//!
//! # See also
//! `SelectionDAGISel::CodeGenAndEmitDAG`
//! ([L941](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L941)).

use llrm_dagisel::interp::Outcome;

use super::SelectionDAG;
use super::actions::TargetLowering;
use super::dag_combiner::CombineHooks;
use super::dag_select::{Refusal, do_instruction_selection_with, legalized_with, remove_assertions};
use super::dagisel_adapter::{DagisAdapter, PinRegisters};
use super::legalize_dag;
use super::schedule::{SchedInfo, Unit, schedule};
use super::x86_preprocess::FrameObjects;

/// The borrows legalization needs at once.
pub struct Phases<'a, L> {
    pub tli: &'a TargetLowering,
    pub lowering: &'a mut L,
    /// The target whose inline routines the `i128` divide calls, if it has any.
    pub arch: Option<&'a dyn llrm_target::Target>,
}

/// What picks the instruction for a node: the target's `Select`.
pub trait Select {
    fn select(
        &mut self,
        adapter: &mut DagisAdapter<'_>,
        node: u32,
    ) -> Outcome;
}

/// A target of the DAG path: its lowering, its instruction selector and its
/// register numbering.
pub trait DagTarget {
    /// The custom combines and lowerings (`TargetLowering`'s hooks).
    type Lowering: CombineHooks + legalize_dag::Hooks;

    fn phases(&mut self) -> Phases<'_, Self::Lowering>;

    /// The target's physical registers in the pin's numbering, and its
    /// `Select`; two borrows of the target at once.
    fn selection(&mut self) -> (&dyn PinRegisters, &mut dyn Select);

    /// Before the first combine.
    fn prepare(
        &mut self,
        _dag: &mut SelectionDAG,
    ) {
    }

    /// The reason a legalizer stopped, when the target knows a better one than
    /// `why` (a custom lowering that refused).
    fn legalize_failed(
        &mut self,
        why: Refusal,
    ) -> Refusal {
        why
    }

    /// `PreprocessISelDAG`: the rewrites that need no matcher, after the
    /// legalizers and before the assertion nodes go.
    fn preprocess(
        &mut self,
        _dag: &mut SelectionDAG,
        _temporaries: &mut dyn FrameObjects,
    ) {
    }

    /// `PostprocessISelDAG`: the rewrites of the selected machine nodes.
    fn postprocess(
        &mut self,
        _dag: &mut SelectionDAG,
    ) {
    }
}

/// One block's DAG through the pin's pipeline, to the scheduled units.
pub fn select_units<T: DagTarget>(
    dag: &mut SelectionDAG,
    target: &mut T,
    info: &dyn SchedInfo,
    temporaries: &mut dyn FrameObjects,
) -> Result<Vec<Unit>, Refusal> {
    // The scratch slots the legalizers ask for are frame objects numbered from
    // here; they are made as the legalizers finish, before anything else is.
    dag.first_temporary = temporaries.objects();
    target.prepare(dag);
    let phases = target.phases();
    let legal = legalized_with(dag, phases.tli, phases.lowering, phases.arch);
    legal.map_err(|why| target.legalize_failed(why))?;
    for (at, (size, align)) in std::mem::take(&mut dag.temporaries).into_iter().enumerate() {
        let made = temporaries.create_stack_temporary(size, align);
        debug_assert_eq!(made, dag.first_temporary + at as i32);
    }
    target.preprocess(dag, temporaries);
    remove_assertions(dag);
    let (registers, select) = target.selection();
    do_instruction_selection_with(dag, registers, |adapter, node| select.select(adapter, node))?;
    target.postprocess(dag);
    let root = dag.get_root();
    Ok(schedule(dag, root, info))
}
