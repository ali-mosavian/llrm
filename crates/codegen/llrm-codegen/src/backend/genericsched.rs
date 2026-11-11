//! `GenericScheduler::tryCandidate`: how the pre-RA machine scheduler picks
//! between two ready nodes (row 65, S0 of ARM64-SCHED-HOIST).
//!
//! # How it works
//! A candidate carries the node, the zone it would extend, the reason it won
//! so far and what it costs: pressure, resources, stall cycles, depth and
//! height. [`try_candidate`] compares a new one with the best so far through
//! the pin's list of heuristics in the pin's order (MachineScheduler.cpp
//! `tryCandidate`, #L3965): the first heuristic that tells the two apart
//! decides, and records its [`Reason`] on the winner (and, as `tryLess` and
//! `tryGreater` do, lowers the reason of a loser that stays).
//!
//! ```text
//! PHYS-REG  REG-EXCESS  REG-CRIT  [same zone: acyclic latency, STALL]
//! CLUSTER   [same zone: WEAK]  REG-MAX
//! [same zone: RES-REDUCE  RES-DEMAND  latency  ORDER]
//! ```
//!
//! The zone is `None` when the two come from different zones (top and bottom
//! of a bidirectional region): only the heuristics that compare across zones
//! apply.
//!
//! # Reasons (pin d1106deb71cc)
//! The enum is [`CandReason`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineScheduler.h#L1099); each reason is set at:
//!
//! | reason | set at |
//! |---|---|
//! | `NoCand`, `Only1`, `FirstValid` | [reset](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineScheduler.h#L1183), [`pickOnlyChoice` picks](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4107), [first candidate](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3970) |
//! | `PhysReg` | [`tryBiasPhysRegs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3895), called at [#L3975](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3975) |
//! | `RegExcess`, `RegCritical` | [#L3981](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3981), [#L3988](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3988) (`tryPressure` [#L3805](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3805)) |
//! | `Stall` | [#L4008](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4008) |
//! | `Cluster`, `Weak` | [#L4026](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4026), [#L4033](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4033) |
//! | `RegMax` | [#L4040](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4040) |
//! | `ResourceReduce`, `ResourceDemand` | [#L4048](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4048), [#L4052](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4052) |
//! | `TopDepthReduce`, `TopPathReduce`, `BotHeightReduce`, `BotPathReduce` | [`tryLatency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3488), called at [#L4003](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4003) and [#L4058](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4058) |
//! | `NodeOrder` | [#L4064](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4064), and in `tryBiasPhysRegs` |
//!
//! # Not here
//! The ready queues, the clock and the resource counts of a zone
//! (`SchedBoundary`) and the choice of zone (`pickNodeBidirectional`) are the
//! callers'; this module is given what they measure.

use crate::backend::regpressure::{Delta, Pick, try_pressure};

/// `GenericSchedulerBase::CandReason`, in the pin's order: a smaller value is
/// a stronger reason (`Cand.Reason > Reason` lowers a losing candidate's).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Reason {
    NoCand,
    Only1,
    PhysReg,
    RegExcess,
    RegCritical,
    Stall,
    Cluster,
    Weak,
    RegMax,
    ResourceReduce,
    ResourceDemand,
    BotHeightReduce,
    BotPathReduce,
    TopDepthReduce,
    TopPathReduce,
    NodeOrder,
    FirstValid,
}

impl Reason {
    /// `getReasonStr`, as `-debug-only=machine-scheduler` prints it (padding
    /// left out).
    pub fn name(self) -> &'static str {
        match self {
            Reason::NoCand => "NOCAND",
            Reason::Only1 => "ONLY1",
            Reason::PhysReg => "PHYS-REG",
            Reason::RegExcess => "REG-EXCESS",
            Reason::RegCritical => "REG-CRIT",
            Reason::Stall => "STALL",
            Reason::Cluster => "CLUSTER",
            Reason::Weak => "WEAK",
            Reason::RegMax => "REG-MAX",
            Reason::ResourceReduce => "RES-REDUCE",
            Reason::ResourceDemand => "RES-DEMAND",
            Reason::BotHeightReduce => "BOT-HEIGHT",
            Reason::BotPathReduce => "BOT-PATH",
            Reason::TopDepthReduce => "TOP-DEPTH",
            Reason::TopPathReduce => "TOP-PATH",
            Reason::NodeOrder => "ORDER",
            Reason::FirstValid => "FIRST",
        }
    }
}

/// `CandPolicy`: what the zone being extended asks for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CandPolicy {
    pub reduce_latency: bool,
    pub reduce_res_idx: usize,
    pub demand_res_idx: usize,
}

/// `SchedResourceDelta`: the critical resources a node needs in the scheduled
/// region, and the ones another region is short of.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceDelta {
    pub crit_resources: u32,
    pub demanded_resources: u32,
}

/// What [`try_candidate`] asks of one node, measured by the caller in the
/// zone the node would extend.
#[derive(Clone, Copy, Debug)]
pub struct Cand {
    pub node: usize,
    pub at_top: bool,
    pub reason: Reason,
    pub policy: CandPolicy,
    /// `RPDelta`: all `None` where pressure is not tracked.
    pub pressure: Delta,
    pub resources: ResourceDelta,
    /// `biasPhysReg` (-1, 0 or 1).
    pub phys_bias: i32,
    /// `getLatencyStallCycles` in the node's zone.
    pub stall: u32,
    /// The node is the successor of the cluster its zone just scheduled
    /// (`isTheSameCluster(ZoneCluster, SU->ParentClusterIdx)`).
    pub cluster_succ: bool,
    /// `getWeakLeft` on the node's side.
    pub weak_left: usize,
    pub depth: u32,
    pub height: u32,
}

impl Cand {
    /// A candidate that has not been compared yet.
    pub fn new(
        node: usize,
        at_top: bool,
        policy: CandPolicy,
    ) -> Cand {
        Cand {
            node,
            at_top,
            reason: Reason::NoCand,
            policy,
            pressure: Delta::default(),
            resources: ResourceDelta::default(),
            phys_bias: 0,
            stall: 0,
            cluster_succ: false,
            weak_left: 0,
            depth: 0,
            height: 0,
        }
    }
}

/// What the zone and the region contribute to the comparison.
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub is_top: bool,
    /// `getCurrMOps`.
    pub curr_mops: u32,
    /// `getScheduledLatency`.
    pub scheduled_latency: u32,
}

/// The region's settings (`Rem`, `RegionPolicy`, `DAG`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Region {
    /// `DAG->isTrackingPressure()`.
    pub tracks_pressure: bool,
    /// `Rem.IsAcyclicLatencyLimited`.
    pub acyclic_latency_limited: bool,
    /// `RegionPolicy.DisableLatencyHeuristic`.
    pub disable_latency: bool,
    /// `RegionPolicy.BiasPRegsExtra`.
    pub bias_pregs_extra: bool,
}

fn try_less(
    try_val: i64,
    cand_val: i64,
    try_cand: &mut Cand,
    cand: &mut Cand,
    reason: Reason,
) -> bool {
    if try_val < cand_val {
        try_cand.reason = reason;
        return true;
    }
    if try_val > cand_val {
        cand.reason = cand.reason.min(reason);
        return true;
    }
    false
}

fn try_greater(
    try_val: i64,
    cand_val: i64,
    try_cand: &mut Cand,
    cand: &mut Cand,
    reason: Reason,
) -> bool {
    if try_val > cand_val {
        try_cand.reason = reason;
        return true;
    }
    if try_val < cand_val {
        cand.reason = cand.reason.min(reason);
        return true;
    }
    false
}

/// `tryPressure` (#L3805) of one kind of pressure, through
/// [`try_pressure`]'s answer.
fn pressure(
    try_p: Option<crate::backend::regpressure::PressureChange>,
    cand_p: Option<crate::backend::regpressure::PressureChange>,
    try_cand: &mut Cand,
    cand: &mut Cand,
    reason: Reason,
) -> bool {
    match try_pressure(try_p, cand_p, try_cand.at_top == cand.at_top) {
        Pick::Try => {
            try_cand.reason = reason;
            true
        }
        Pick::Cand => {
            cand.reason = cand.reason.min(reason);
            true
        }
        Pick::Neither => false,
    }
}

/// `tryLatency` (#L3488).
fn try_latency(
    try_cand: &mut Cand,
    cand: &mut Cand,
    zone: &Zone,
) -> bool {
    let (try_depth, cand_depth) = (i64::from(try_cand.depth), i64::from(cand.depth));
    let (try_height, cand_height) = (i64::from(try_cand.height), i64::from(cand.height));
    if zone.is_top {
        if try_depth.max(cand_depth) > i64::from(zone.scheduled_latency)
            && try_less(try_depth, cand_depth, try_cand, cand, Reason::TopDepthReduce)
        {
            return true;
        }
        try_greater(try_height, cand_height, try_cand, cand, Reason::TopPathReduce)
    } else {
        if try_height.max(cand_height) > i64::from(zone.scheduled_latency)
            && try_less(try_height, cand_height, try_cand, cand, Reason::BotHeightReduce)
        {
            return true;
        }
        try_greater(try_depth, cand_depth, try_cand, cand, Reason::BotPathReduce)
    }
}

/// `tryBiasPhysRegs` (#L3895): a node that defines a physical register its
/// user copies out, or copies one in, goes next to that copy.
fn try_bias_phys_regs(
    try_cand: &mut Cand,
    cand: &mut Cand,
    zone: Option<&Zone>,
    extra: bool,
) -> bool {
    let (try_bias, cand_bias) = (try_cand.phys_bias, cand.phys_bias);
    if try_greater(i64::from(try_bias), i64::from(cand_bias), try_cand, cand, Reason::PhysReg) {
        return true;
    }
    if extra
        && let Some(zone) = zone
        && try_bias != 0
        && try_bias == cand_bias
    {
        // Both biased the same way: keep their input order.
        let (try_node, cand_node) = (try_cand.node as i64, cand.node as i64);
        if zone.is_top {
            try_less(try_node, cand_node, try_cand, cand, Reason::NodeOrder);
        } else {
            try_greater(try_node, cand_node, try_cand, cand, Reason::NodeOrder);
        }
        return true;
    }
    false
}

/// `GenericScheduler::tryCandidate` (#L3965): true when `try_cand` beats
/// `cand`. `zone` is the zone both extend, `None` when they are in different
/// ones. On a win `try_cand.reason` is the heuristic that decided; on a loss
/// that a heuristic decided, `cand.reason` is lowered to it.
pub fn try_candidate(
    cand: &mut Cand,
    try_cand: &mut Cand,
    zone: Option<&Zone>,
    region: &Region,
) -> bool {
    if cand.reason == Reason::NoCand {
        try_cand.reason = Reason::FirstValid;
        return true;
    }
    if try_bias_phys_regs(try_cand, cand, zone, region.bias_pregs_extra) {
        return try_cand.reason != Reason::NoCand;
    }
    if region.tracks_pressure {
        let (t, c) = (try_cand.pressure, cand.pressure);
        if pressure(t.excess, c.excess, try_cand, cand, Reason::RegExcess)
            || pressure(t.critical_max, c.critical_max, try_cand, cand, Reason::RegCritical)
        {
            return try_cand.reason != Reason::NoCand;
        }
    }
    if let Some(zone) = zone {
        // A loop limited by its acyclic path schedules for latency; within a
        // cycle that already issued something, the other heuristics go first.
        if region.acyclic_latency_limited && zone.curr_mops == 0 && try_latency(try_cand, cand, zone) {
            return try_cand.reason != Reason::NoCand;
        }
        if try_less(i64::from(try_cand.stall), i64::from(cand.stall), try_cand, cand, Reason::Stall) {
            return try_cand.reason != Reason::NoCand;
        }
    }
    if try_greater(i64::from(try_cand.cluster_succ), i64::from(cand.cluster_succ), try_cand, cand, Reason::Cluster) {
        return try_cand.reason != Reason::NoCand;
    }
    if zone.is_some() && try_less(try_cand.weak_left as i64, cand.weak_left as i64, try_cand, cand, Reason::Weak) {
        return try_cand.reason != Reason::NoCand;
    }
    if region.tracks_pressure
        && pressure(try_cand.pressure.current_max, cand.pressure.current_max, try_cand, cand, Reason::RegMax)
    {
        return try_cand.reason != Reason::NoCand;
    }
    let Some(zone) = zone else {
        return false;
    };
    let (t, c) = (try_cand.resources, cand.resources);
    if try_less(i64::from(t.crit_resources), i64::from(c.crit_resources), try_cand, cand, Reason::ResourceReduce) {
        return try_cand.reason != Reason::NoCand;
    }
    if try_greater(
        i64::from(t.demanded_resources),
        i64::from(c.demanded_resources),
        try_cand,
        cand,
        Reason::ResourceDemand,
    ) {
        return try_cand.reason != Reason::NoCand;
    }
    if !region.disable_latency
        && try_cand.policy.reduce_latency
        && !region.acyclic_latency_limited
        && try_latency(try_cand, cand, zone)
    {
        return try_cand.reason != Reason::NoCand;
    }
    // Fall through to the original instruction order.
    if (zone.is_top && try_cand.node < cand.node) || (!zone.is_top && try_cand.node > cand.node) {
        try_cand.reason = Reason::NodeOrder;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::regpressure::PressureChange;

    const BOTTOM: Zone = Zone { is_top: false, curr_mops: 0, scheduled_latency: 0 };
    const TOP: Zone = Zone { is_top: true, curr_mops: 0, scheduled_latency: 0 };

    fn pair(
        try_node: usize,
        cand_node: usize,
    ) -> (Cand, Cand) {
        let mut cand = Cand::new(cand_node, false, CandPolicy::default());
        cand.reason = Reason::FirstValid;
        (cand, Cand::new(try_node, false, CandPolicy::default()))
    }

    #[test]
    fn test_the_first_candidate_is_valid_whatever_it_is() {
        let mut cand = Cand::new(0, false, CandPolicy::default());
        let mut try_cand = Cand::new(3, false, CandPolicy::default());
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &Region::default()));
        assert_eq!(try_cand.reason, Reason::FirstValid);
    }

    #[test]
    fn test_the_later_node_wins_a_tie_bottom_up_and_the_earlier_top_down() {
        let region = Region::default();
        let (mut cand, mut try_cand) = pair(5, 2);
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &region));
        assert_eq!(try_cand.reason, Reason::NodeOrder);
        let (mut cand, mut try_cand) = pair(2, 5);
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &region));
        let (mut cand, mut try_cand) = pair(2, 5);
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&TOP), &region));
    }

    #[test]
    fn test_nodes_of_different_zones_never_fall_to_node_order() {
        let (mut cand, mut try_cand) = pair(5, 2);
        assert!(!try_candidate(&mut cand, &mut try_cand, None, &Region::default()));
    }

    #[test]
    fn test_a_stall_beats_node_order_and_lowers_the_losers_reason() {
        let (mut cand, mut try_cand) = pair(5, 2);
        cand.stall = 3;
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &Region::default()));
        assert_eq!(try_cand.reason, Reason::Stall);
        // The other way round, the best stays and learns why.
        let (mut cand, mut try_cand) = pair(5, 2);
        try_cand.stall = 3;
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &Region::default()));
        assert_eq!(cand.reason, Reason::Stall);
    }

    #[test]
    fn test_physical_register_bias_comes_before_pressure() {
        let region = Region { tracks_pressure: true, ..Region::default() };
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.phys_bias = 1;
        cand.pressure.excess = None;
        try_cand.pressure.excess = Some(PressureChange { set: 0, inc: 2 });
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &region));
        assert_eq!(try_cand.reason, Reason::PhysReg);
    }

    #[test]
    fn test_equal_bias_keeps_input_order_only_when_asked() {
        let (mut cand, mut try_cand) = pair(5, 2);
        cand.phys_bias = 1;
        try_cand.phys_bias = 1;
        // Not asked: falls through to the later heuristics.
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &Region::default()));
        assert_eq!(try_cand.reason, Reason::NodeOrder);
        // Asked, and the earlier node does not win bottom-up.
        let region = Region { bias_pregs_extra: true, ..Region::default() };
        let (mut cand, mut try_cand) = pair(2, 5);
        cand.phys_bias = 1;
        try_cand.phys_bias = 1;
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &region));
    }

    #[test]
    fn test_pressure_is_not_looked_at_unless_tracked() {
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.pressure.excess = Some(PressureChange { set: 0, inc: -1 });
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &Region::default()));
        let region = Region { tracks_pressure: true, ..Region::default() };
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.pressure.excess = Some(PressureChange { set: 0, inc: -1 });
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&BOTTOM), &region));
        assert_eq!(try_cand.reason, Reason::RegExcess);
    }

    #[test]
    fn test_the_latency_heuristic_needs_the_policy_and_not_a_disabled_region() {
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.height = 9;
        cand.height = 4;
        let bottom = Zone { scheduled_latency: 1, ..BOTTOM };
        // Without ReduceLatency the heights are not compared; the later node
        // wins by order, which here is the candidate.
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&bottom), &Region::default()));
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.height = 9;
        cand.height = 4;
        try_cand.policy.reduce_latency = true;
        // The lesser height wins bottom-up: the candidate.
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&bottom), &Region::default()));
        assert_eq!(cand.reason, Reason::BotHeightReduce);
        let region = Region { disable_latency: true, ..Region::default() };
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.height = 9;
        cand.height = 4;
        try_cand.policy.reduce_latency = true;
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&bottom), &region));
        assert_eq!(cand.reason, Reason::FirstValid);
    }

    #[test]
    fn test_an_acyclic_latency_limited_loop_decides_by_latency_before_stalls() {
        let region = Region { acyclic_latency_limited: true, ..Region::default() };
        let bottom = Zone { scheduled_latency: 1, ..BOTTOM };
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.height = 2;
        cand.height = 7;
        cand.stall = 0;
        try_cand.stall = 5;
        assert!(try_candidate(&mut cand, &mut try_cand, Some(&bottom), &region));
        assert_eq!(try_cand.reason, Reason::BotHeightReduce);
        // A cycle that already issued: the stall decides first.
        let busy = Zone { curr_mops: 1, ..bottom };
        let (mut cand, mut try_cand) = pair(1, 2);
        try_cand.height = 2;
        cand.height = 7;
        try_cand.stall = 5;
        assert!(!try_candidate(&mut cand, &mut try_cand, Some(&busy), &region));
        assert_eq!(cand.reason, Reason::Stall);
    }

    #[test]
    fn test_reasons_are_ordered_as_the_pins_enum() {
        assert!(Reason::Stall < Reason::Cluster);
        assert!(Reason::ResourceDemand < Reason::BotHeightReduce);
        assert!(Reason::TopPathReduce < Reason::NodeOrder);
        assert_eq!(Reason::PhysReg.name(), "PHYS-REG");
    }
}
