//! x86's DAG path of one block: the pin's `CodeGenAndEmitDAG` order over
//! `X86Target`, then the installed emitter.
//!
//! Selection goes to `llrm_x86_dagisel::x86_select::select_node` (the pin's
//! `Select` hand matchers, then the generated table); the generic order and
//! the legalizers are `llrm_codegen::backend::selectiondag::dag_select` and
//! `dag_target`.

use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::actions::TargetLowering;
use llrm_codegen::backend::selectiondag::dag_select::{Refusal, do_instruction_selection_with, emit_units};
use llrm_codegen::backend::selectiondag::dag_target::{DagTarget, FrameObjects, Phases, Select, select_units};
use llrm_codegen::backend::selectiondag::dagisel_adapter::{DagisAdapter, PinRegisters};
use llrm_codegen::backend::selectiondag::emit_api::{EmitContext, EmitState, Emitted};
use llrm_codegen::backend::selectiondag::schedule::SchedInfo;
use llrm_codegen::backend::selectiondag::x86_lowering::X86Lowering;
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_x86_dagisel::x86_hooks::X86Hooks;
use llrm_x86_dagisel::x86_select::select_node;

/// x86 as a [`DagTarget`]: its lowering tables, hooks and selector tables.
pub struct X86Target<'a, 'p> {
    /// What the blocks selected before this one learned of the registers they
    /// copy (`FunctionLoweringInfo::LiveOutRegInfo`).
    pub live: &'a mut llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo,
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
        Phases {
            tli: self.tli,
            lowering: &mut *self.lowering,
            arch: Some(self.arch),
            opt_none: self.chooser.hooks.options.opt_none,
        }
    }

    fn selection(&mut self) -> (&dyn PinRegisters, &mut dyn Select) {
        (&llrm_codegen::backend::selectiondag::dagisel_adapter::X86Registers, &mut self.chooser)
    }

    /// `getCopyFromRegs`' assertions: what an earlier block knew of the
    /// registers this one reads.
    fn prepare(
        &mut self,
        dag: &mut SelectionDAG,
    ) {
        llrm_codegen::backend::selectiondag::live_out::assert_live_out_copies(dag, self.live);
    }

    /// `PostprocessISelDAG`; skipped at -O0 like the pin's.
    fn postprocess(
        &mut self,
        dag: &mut SelectionDAG,
    ) {
        if !self.chooser.hooks.options.opt_none {
            super::postprocess::postprocess(dag);
        }
    }

    fn preprocess(
        &mut self,
        dag: &mut SelectionDAG,
        temporaries: &mut dyn FrameObjects,
    ) {
        // `ComputeLiveOutVRegInfo`: after the legalizers, before the assertions
        // go.
        llrm_codegen::backend::selectiondag::live_out::compute_live_out_vreg_info(dag, self.live);
        super::preprocess::preprocess(dag, temporaries, self.lowering.sse2);
        // DEVIATE SD544 (m16): no callee-load move at real mode. Reason: its
        // calls and jumps carry a far callee and a segment, which the pin has
        // no counterpart for. Exit 2026-11-12: the x86-m16 review of far
        // indirect calls.
        let options = &self.chooser.hooks.options;
        if options.address_mode != llrm_dagisel::addr::AddrMode::Real16 {
            let facts = super::preprocess::CalleeLoadFacts {
                optimizing: !options.opt_none,
                is_64: options.cpu == llrm_x86_dagisel::x86_hooks::Cpu::X64,
                position_independent: false,
                slow_two_mem_ops: false,
                slow_indirect_call: false,
                indirect_thunks: false,
            };
            let hooks = &*self.chooser.hooks;
            super::preprocess::move_callee_loads(dag, facts, &mut |dag, node| {
                let adapter = DagisAdapter::new(dag);
                llrm_x86_dagisel::x86_predicates::X86Predicates::check_tc_ret_enough_regs(hooks, &adapter, node)
            });
        }
    }
}

/// The pin's pipeline for one block's DAG.
pub fn select_block(
    live: &mut llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo,
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
    temporaries: &mut dyn FrameObjects,
) -> Result<Emitted, Refusal> {
    let units = select_units(
        dag,
        &mut X86Target { live, tli, lowering, chooser: X86Chooser { selector, hooks }, arch },
        info,
        temporaries,
    )?;
    llrm_codegen::support::debug::timed("dag emit", || emit_units(dag, &units, ctx, state, first_at))
        .map_err(Refusal::Emit)
}

pub fn do_instruction_selection(
    dag: &mut SelectionDAG,
    selector: &Selector,
    hooks: &mut X86Hooks,
) -> Result<(), Refusal> {
    do_instruction_selection_with(
        dag,
        &llrm_codegen::backend::selectiondag::dagisel_adapter::X86Registers,
        |adapter, node| select_node(selector, adapter, hooks, node),
    )
}
