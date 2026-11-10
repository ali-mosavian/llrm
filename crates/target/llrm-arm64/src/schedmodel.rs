//! The scheduling model of an AArch64 CPU for the pre-RA machine scheduler:
//! `TargetSchedModel` over the pin's `MCSchedModel` (the S1 tables of
//! `llrm_dagisel::sched`) and `AArch64Subtarget::overrideSchedPolicy`.
//!
//! # How it works
//! An instruction's form name is mapped to the pin's opcode
//! (`opcodes.instr`'s last column, then the opcode's number), the opcode to its
//! scheduling class, a variant class resolved by the instruction's own
//! operands ([`SchedPred`]). The class gives the micro-ops, the resources it
//! writes and the latency of its first def. An opcode the model does not state
//! takes `TargetInstrInfo::defaultDefLatency`: 0 for a copy, the model's
//! `LoadLatency` for a load, else 1.
//!
//! # Example
//! ```text
//! generic (Cortex-A510): ADDWri -> 1 micro-op, 1 cycle on CortexA510UnitALU
//! bidirectional, latency heuristic on (only Apple A7 and Neoverse V2 turn
//! it off)
//! ```
//!
//! # Not translated
//! - SB6: `ReadAdvance` (the use's forwarding adjustment of a latency) is not
//!   applied; the edge latency is the def's write latency. Owner arm64-phi,
//!   exit 2026-11-20.
//! - SB7: a form name that is several LLVM opcodes (`[A|B]`, a load's width)
//!   takes the first; `IsScaledAddr` is false. Owner arm64-phi, exit
//!   2026-11-20.
//!
//! # Used by
//! `lir_view`'s `AllocTarget::scheduling_model`.

use std::collections::HashMap;
use std::sync::OnceLock;

use llrm_codegen::backend::genericdriver::RegionPolicy;
use llrm_codegen::backend::machinesched::default_def_latency;
use llrm_codegen::backend::schedboundary::{Machine, ProcResource, Unit, WriteRes};
use llrm_codegen::backend::schedmodel::SchedulingModel;
use llrm_codegen::model::ir::Loc;
use llrm_codegen::model::lir::Insn;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;
use llrm_dagisel::generated::aarch64::sched::CORTEX_A510_MODEL;
use llrm_dagisel::sched::{Modeled, SchedModel, SchedPred};

use crate::opcode_rows::llvm_names;

/// The pin's opcode number of each opcode name the DAG tables emit.
fn opcode_numbers() -> &'static HashMap<&'static str, u16> {
    static NUMBERS: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    NUMBERS.get_or_init(|| MACHINE_OPCODES.iter().map(|&(number, name)| (name, number)).collect())
}

/// An AArch64 CPU's model.
pub struct Arm64Model {
    model: &'static SchedModel,
    /// `FeatureDisableLatencySchedHeuristic` of the CPU's tuning.
    disable_latency: bool,
}

impl Arm64Model {
    /// `-mcpu=generic`: the Cortex-A510 model, whose tuning keeps the latency
    /// heuristic.
    pub fn generic() -> Self {
        Self { model: &CORTEX_A510_MODEL, disable_latency: false }
    }

    /// The model's class data for `insn`, or `None` where the model says
    /// nothing.
    fn class(
        &self,
        insn: &Insn,
    ) -> Option<llrm_dagisel::sched::ClassData> {
        let name = insn.what.as_ref()?.name.as_deref()?;
        let llvm = *llvm_names(name).first()?;
        let opcode = *opcode_numbers().get(llvm)?;
        let shifted = shift_amount(insn).is_some_and(|amount| amount != 0);
        let mut holds = |pred: SchedPred| match pred {
            SchedPred::HasShiftedReg => shifted,
            SchedPred::IsScaledAddr => false,
        };
        match self.model.resolve(opcode, &mut holds)? {
            Modeled::Class(data) => Some(data),
            Modeled::Unmodeled | Modeled::Unsupported => None,
        }
    }
}

/// The shift amount of a shifted-register form: its last source, an
/// immediate.
fn shift_amount(insn: &Insn) -> Option<i64> {
    match insn.what.as_ref()?.sources.last()? {
        Loc::Imm(imm) => Some(imm.value),
        _ => None,
    }
}

impl SchedulingModel for Arm64Model {
    fn machine(&self) -> Machine {
        let resources = self
            .model
            .resources
            .iter()
            .map(|one| ProcResource {
                num_units: u32::from(one.num_units),
                buffer_size: one.buffer_size,
                sub_units: one.sub_units.iter().map(|&unit| usize::from(unit)).collect(),
            })
            .collect();
        Machine::new(self.model.issue_width, self.model.micro_op_buffer_size.max(0) as u32, resources)
    }

    /// `AArch64Subtarget::overrideSchedPolicy`.
    fn policy(&self) -> RegionPolicy {
        RegionPolicy::bidirectional(self.disable_latency)
    }

    /// `initSUnits`: the micro-ops and resource writes of the class, and
    /// whether it uses a buffer-1 (unbuffered) or buffer-0 (reserved)
    /// resource.
    fn unit(
        &self,
        insn: &Insn,
    ) -> Unit {
        let Some(class) = self.class(insn) else { return Unit { micro_ops: 1, ..Unit::default() } };
        let mut unit = Unit {
            micro_ops: u32::from(class.num_micro_ops),
            begin_group: class.begin_group,
            end_group: class.end_group,
            ..Unit::default()
        };
        for write in class.write_proc_res {
            unit.writes.push(WriteRes {
                idx: usize::from(write.resource),
                release_at_cycle: u32::from(write.release_at_cycle),
                acquire_at_cycle: u32::from(write.acquire_at_cycle),
            });
            match self.model.resources[usize::from(write.resource)].buffer_size {
                0 => unit.has_reserved_resource = true,
                1 => unit.is_unbuffered = true,
                _ => {}
            }
        }
        unit
    }

    /// `computeOperandLatency` without the use's `ReadAdvance` (SB6).
    fn def_latency(
        &self,
        insn: &Insn,
    ) -> u32 {
        match self.class(insn).and_then(|class| class.write_latency.first().copied()) {
            // `capLatency`: a variable latency (-1) is 1000.
            Some(entry) => u32::try_from(entry.cycles).unwrap_or(1000),
            None => default_def_latency(insn, self.model.load_latency),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_codegen::model::ir::{Held, Imm, Operation, Semantics};

    use super::*;

    fn form(
        name: &str,
        sources: Vec<Loc>,
    ) -> Insn {
        let held = |value| Loc::Held(Held { value, width: 4, offset: 0 });
        let what =
            Semantics { name: Some(name.into()), dests: vec![held(1)], sources, ..Semantics::new(Operation::Binary) };
        Insn::new(0, Some((0, 0)), Some(what), vec![1], vec![2])
    }

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    /// The module header's example, read off the pin's Cortex-A510 tables:
    /// an add of an immediate is one micro-op of one cycle.
    #[test]
    fn an_add_immediate_is_one_micro_op_of_one_cycle_on_the_generic_cpu() {
        let model = Arm64Model::generic();
        let add = form("ADDWri", vec![held(2), imm(1), imm(0)]);
        assert_eq!((model.unit(&add).micro_ops, model.def_latency(&add)), (1, 1));
        assert!(!model.unit(&add).writes.is_empty(), "it writes a resource of the model");
    }

    /// An opcode the model does not state is priced as the pin prices it by
    /// default: the unit is one micro-op and the latency is
    /// `defaultDefLatency`.
    #[test]
    fn a_form_the_model_does_not_know_takes_the_default_latency() {
        let model = Arm64Model::generic();
        let unknown = Arc::new(form("NoSuchForm", vec![held(2)]));
        assert_eq!(
            (model.unit(&unknown).micro_ops, model.def_latency(&unknown)),
            (1, default_def_latency(&unknown, 3))
        );
    }

    /// AArch64 schedules from both ends and keeps the latency heuristic on
    /// the generic CPU.
    #[test]
    fn the_policy_is_bidirectional_with_the_latency_heuristic() {
        let policy = Arm64Model::generic().policy();
        assert_eq!((policy.only_top_down, policy.only_bottom_up, policy.disable_latency), (false, false, false));
    }
}
