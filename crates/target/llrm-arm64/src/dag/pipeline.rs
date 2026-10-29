//! The shared DAG stages run for arm64: combine, legalize, select with the
//! AArch64 table, schedule.
//!
//! DEVIATE: no target combines (`PerformDAGCombine` of AArch64 is not
//! translated), no `PreprocessISelDAG`, and every hook of the matcher says no,
//! so a pattern that needs a complex pattern or a node predicate is not
//! selected. A Custom operation is refused (the legalizer would otherwise
//! expand it). Reason: the first function needs none. Exit: B3.

use llrm_codegen::backend::selectiondag::dag_combiner::CombineHooks;
use llrm_codegen::backend::selectiondag::dag_select::{legalized_with, do_instruction_selection_with, Refusal};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks as LegalizeHooks;
use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::dag::{Dag, Hooks, SDValue as PinValue};
use llrm_dagisel::generated::aarch64::aarch64_apple_m1 as table;
use llrm_dagisel::generated::aarch64::instrinfo::{self, flag};
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::interp::Selector;
use llrm_dagisel::matcher::Vt;
use llrm_lir::registers::RegId;
use llrm_sdag::actions::{Aarch64Features, TargetLowering};

use super::ids::Aarch64Ids;

/// Combine hooks and legalizer hooks of arm64: none. A Custom operation the
/// table asks for is recorded, and the block refused after legalization.
#[derive(Default)]
pub struct Aarch64Lowering {
    pub custom: Option<String>,
}

impl CombineHooks for Aarch64Lowering {}

impl LegalizeHooks for Aarch64Lowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        self.custom.get_or_insert_with(|| format!("custom lowering of {:?}", dag.node(node).opcode));
        None
    }
}

/// The matcher's hooks with nothing decided at selection time.
pub struct Aarch64Hooks;

impl<D: Dag> Hooks<D> for Aarch64Hooks {
    fn pointer_type(&self) -> Vt {
        mvt::i64
    }
    fn check_pattern_predicate(
        &self,
        _: u16,
    ) -> bool {
        false
    }
    fn check_node_predicate(
        &self,
        _: &D,
        _: PinValue,
        _: u16,
    ) -> bool {
        false
    }
    fn check_complex_pattern(
        &mut self,
        _: &mut D,
        _: u32,
        _: Option<u32>,
        _: PinValue,
        _: u8,
        _: &mut Vec<(PinValue, Option<u32>)>,
    ) -> bool {
        false
    }
    fn run_xform(
        &mut self,
        _: &mut D,
        value: PinValue,
        _: u8,
    ) -> PinValue {
        value
    }
    fn is_profitable_to_fold(
        &self,
        _: &D,
        _: PinValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn is_legal_to_fold(
        &self,
        _: &D,
        _: PinValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn check_and_mask(
        &self,
        _: &D,
        _: PinValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn check_or_mask(
        &self,
        _: &D,
        _: PinValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool) {
        instrinfo::memory_flags(opcode)
    }
}

/// What the scheduler reads of the AArch64 instruction descriptions.
pub struct Aarch64Sched;

impl SchedInfo for Aarch64Sched {
    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters::pin_register(&Aarch64Ids::new(), id)
    }

    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        instrinfo::desc(opcode as u16).map_or(0, |desc| usize::from(desc.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        instrinfo::desc(opcode as u16).map_or(&[], |desc| desc.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        instrinfo::desc(opcode as u16).is_some_and(|desc| desc.has(flag::CALL))
    }

    fn source_order(&self) -> bool {
        true
    }

    /// NZCV is copied only through a GPR.
    fn has_cross_copy_class(
        &self,
        reg: u16,
    ) -> bool {
        reg == llrm_dagisel::generated::aarch64::ids::reg::NZCV
    }
}

/// `CodeGenAndEmitDAG` up to the end of scheduling: the units, in order.
pub fn select_block(dag: &mut SelectionDAG) -> Result<Vec<Unit>, Refusal> {
    let tli = TargetLowering::aarch64(Aarch64Features::default());
    let mut lowering = Aarch64Lowering::default();
    let legal = legalized_with(dag, &tli, &mut lowering, None);
    // The reason the legalizer stopped is the Custom operation it could not lower.
    if let Some(why) = lowering.custom {
        return Err(Refusal::Legalize(why));
    }
    legal?;
    let selector = Selector::new(table::TABLE);
    let mut hooks = Aarch64Hooks;
    do_instruction_selection_with(dag, &Aarch64Ids::new(), |adapter, node| selector.select(adapter, &mut hooks, node))?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &Aarch64Sched))
}
