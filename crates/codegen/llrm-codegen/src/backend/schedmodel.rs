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

use llrm_lir::registers::RegId;

use crate::backend::genericdriver::RegionPolicy;
use crate::backend::machinesched::default_def_latency;
use crate::backend::schedboundary::{Machine, ProcResource, Unit};
use crate::model::lir::Insn;

/// What `TargetInstrInfo::getMemOperandsWithOffsetWidth` reports of a load or
/// store: the one base operand, the byte offset from it and the bytes
/// accessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemOp {
    pub base: MemBase,
    pub offset: i64,
    pub width: u32,
    /// `mayLoad` (else `mayStore`).
    pub load: bool,
}

/// The base operand of a [`MemOp`]: a machine register (`sp`), a virtual one
/// (by value) or a frame index. They order as the pin's register numbers do
/// (machine before virtual) and then the frame indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemBase {
    Phys(RegId),
    Virtual(u32),
    Frame(i64),
}

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

    /// Whether a def that nothing in the region reads holds its latency to
    /// the region end (`ScheduleDAGInstrs::buildSchedGraph`'s edge to `ExitSU`).
    /// DEVIATE SB3: off by default; x86 has not measured it.
    fn exit_edges(&self) -> bool {
        false
    }

    /// Whether the target has an x87 register stack, a pressure set of its own
    /// that the float operations' values are counted in (the pin's `RFP`).
    fn x87_stack(&self) -> bool {
        true
    }

    /// `getMemOperandsWithOffsetWidth`: the access of `insn`, or `None` where
    /// the target does not say (no clustering for it).
    fn mem_op(
        &self,
        _insn: &Insn,
    ) -> Option<MemOp> {
        None
    }

    /// `shouldClusterMemOps`: whether the two accesses, ordered by base and
    /// offset, should stay together (`cluster_size` and `bytes` of the cluster
    /// with the second). A target that clusters nothing says no.
    fn should_cluster(
        &self,
        _first: (&Insn, &MemOp),
        _second: (&Insn, &MemOp),
        _cluster_size: u32,
        _bytes: u32,
    ) -> bool {
        false
    }
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
        default_def_latency(insn, 4)
    }
}
