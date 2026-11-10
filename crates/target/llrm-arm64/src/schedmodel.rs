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
use llrm_codegen::backend::schedmodel::{GenericModel, MemBase, MemOp, SchedulingModel};
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
        if std::env::var_os("LLRM_CLASS_DEBUG").is_some() {
            eprintln!("CLASS {name} -> {:?}", llvm_names(name).first());
        }
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

/// The model Darwin keeps (SB8): X86.td's `GenericModel` numbers, on a target
/// with no x87 stack.
pub struct DarwinModel;

impl SchedulingModel for DarwinModel {
    fn machine(&self) -> Machine {
        GenericModel.machine()
    }

    fn policy(&self) -> RegionPolicy {
        GenericModel.policy()
    }

    fn unit(
        &self,
        insn: &Insn,
    ) -> Unit {
        GenericModel.unit(insn)
    }

    fn def_latency(
        &self,
        insn: &Insn,
    ) -> u32 {
        GenericModel.def_latency(insn)
    }

    fn x87_stack(&self) -> bool {
        false
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

    fn x87_stack(&self) -> bool {
        false
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

    /// `AArch64InstrInfo::getMemOperandsWithOffsetWidth` for the base plus
    /// immediate forms (`ui` and a frame slot). SB9: a pre/post-indexed form
    /// reports none (it only exists after `ldst_update`).
    fn mem_op(
        &self,
        insn: &Insn,
    ) -> Option<MemOp> {
        let what = insn.what.as_ref()?;
        let (access, form) = what.name.as_deref()?.split_once('.')?;
        let load = match access.get(..3)? {
            "LDR" => true,
            "STR" => false,
            _ => return None,
        };
        let width = access_bytes(access.get(3..)?)?;
        let Some(Loc::Mem(cell)) = what.sources.last() else { return None };
        let (base, offset) = match form {
            "ui" => (
                match cell.base {
                    Some(held) => MemBase::Virtual(held.value),
                    None => MemBase::Phys(cell.through),
                },
                cell.offset,
            ),
            "fi" => {
                let at = cell.addr?;
                (MemBase::Frame(at.slot_home()?), at.disp)
            }
            _ => return None,
        };
        Some(MemOp { base, offset, width, load })
    }

    /// `AArch64InstrInfo::shouldClusterMemOps`: two accesses that the load
    /// and store optimizer can make a pair.
    fn should_cluster(
        &self,
        (first, a): (&Insn, &MemOp),
        (second, b): (&Insn, &MemOp),
        cluster_size: u32,
        _bytes: u32,
    ) -> bool {
        if std::mem::discriminant(&a.base) != std::mem::discriminant(&b.base) {
            return false;
        }
        if !matches!(a.base, MemBase::Frame(_)) && a.base != b.base {
            return false;
        }
        // Only up to a single pair.
        if cluster_size > 2 {
            return false;
        }
        let (Some(kind_a), Some(kind_b)) = (pair_kind(first), pair_kind(second)) else { return false };
        if !can_pair(kind_a, kind_b) || !candidate_to_pair(first, a) || !candidate_to_pair(second, b) {
            return false;
        }
        // The offsets in units of the access; an unscaled form needs a
        // multiple.
        if a.offset % i64::from(a.width) != 0 || b.offset % i64::from(b.width) != 0 {
            return false;
        }
        let (scaled_a, scaled_b) = (a.offset / i64::from(a.width), b.offset / i64::from(b.width));
        // The pair instructions have a 7-bit signed offset.
        if !(-64..=63).contains(&scaled_a) {
            return false;
        }
        match a.base {
            MemBase::Frame(slot) => matches!(b.base, MemBase::Frame(other) if other == slot),
            _ => scaled_a + 1 == scaled_b,
        }
    }
}

/// The bytes a load or store form moves: its name after `LDR`/`STR`.
fn access_bytes(kind: &str) -> Option<u32> {
    Some(match kind {
        "B" | "SB" | "SBW" | "SBX" => 1,
        "H" | "SH" | "SHW" | "SHX" => 2,
        "W" | "SW" | "F4" => 4,
        "X" | "F8" => 8,
        "F16" => 16,
        _ => return None,
    })
}

/// `isPairableLdStInst`: the form's access kind, if a pair can hold it.
fn pair_kind(insn: &Insn) -> Option<&str> {
    let name = insn.what.as_ref()?.name.as_deref()?;
    let (access, form) = name.split_once('.')?;
    let kind = access.get(3..)?;
    (matches!(form, "ui" | "fi") && matches!(kind, "W" | "X" | "F4" | "F8" | "F16" | "SW")
        && (kind != "SW" || access.starts_with("LDR")))
    .then_some(kind)
}

/// `canPairLdStOpc`: the same kind, or a word with a sign-extended word.
fn can_pair(
    a: &str,
    b: &str,
) -> bool {
    a == b || matches!((a, b), ("W", "SW") | ("SW", "W"))
}

/// `isCandidateToMergeOrPair`: not volatile, and a load does not write its
/// own base register.
fn candidate_to_pair(
    insn: &Insn,
    op: &MemOp,
) -> bool {
    if insn.volatile() {
        return false;
    }
    match op.base {
        MemBase::Virtual(value) => !insn.defines.contains(&value),
        _ => true,
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
