//! What a target tells the pre-RA scheduler about its processor:
//! `TargetSchedModel` of the pin, as the generic scheduler asks it.
//!
//! # How it works
//! A [`SchedulingModel`] gives the zone's machine (issue width, micro-op
//! buffer, resources), the region policy (`overrideSchedPolicy`), each
//! instruction's [`Unit`] (`SUnit`: micro-ops, resource writes) and the
//! latency of what it defines (`computeInstrLatency`, which the dependence
//! graph puts on its data edges). [`GenericModel`] is X86.td's `GenericModel`,
//! which also stands for a target that states none.
//!
//! # Example
//! ```text
//! GenericModel: issue 4, buffer 32, no resources; a load defines in 4
//! cycles, a copy in 0, the rest in 1; bottom-up only.
//! ```
//!
//! # Used by
//! [`machinesched`](super::machinesched).

use crate::backend::genericdriver::RegionPolicy;
use crate::backend::machinesched::default_def_latency;
use crate::backend::schedboundary::{Machine, ProcResource, Unit};
use crate::model::lir::Insn;

/// `TargetSchedModel` for one subtarget.
pub trait SchedulingModel {
    /// `MCSchedModel`: the issue width, the micro-op buffer and the resources.
    fn machine(&self) -> Machine;

    /// `RegionPolicy` after the subtarget's `overrideSchedPolicy`.
    fn policy(&self) -> RegionPolicy;

    /// The `SUnit` of `insn`.
    fn unit(
        &self,
        insn: &Insn,
    ) -> Unit;

    /// The latency of what `insn` defines.
    fn def_latency(
        &self,
        insn: &Insn,
    ) -> u32;
}

/// X86.td's `GenericModel`: issue width 4, a micro-op buffer of 32, no
/// resources and no instruction schedules, so every instruction is one
/// micro-op and a latency is `defaultDefLatency`.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenericModel;

impl SchedulingModel for GenericModel {
    fn machine(&self) -> Machine {
        Machine::new(4, 32, vec![ProcResource { num_units: 0, buffer_size: -1, sub_units: vec![] }])
    }

    fn policy(&self) -> RegionPolicy {
        RegionPolicy::bottom_up()
    }

    fn unit(
        &self,
        _: &Insn,
    ) -> Unit {
        Unit { micro_ops: 1, ..Unit::default() }
    }

    fn def_latency(
        &self,
        insn: &Insn,
    ) -> u32 {
        default_def_latency(insn)
    }
}
