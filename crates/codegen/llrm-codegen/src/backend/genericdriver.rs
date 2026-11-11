//! `GenericScheduler` over a dependence graph: the loop that picks a node from
//! the top or the bottom zone, schedules it and releases its neighbours (row
//! 65, S2 of ARM64-SCHED-HOIST).
//!
//! # How it works
//! Roots with no predecessor start in the top zone, roots with no successor in
//! the bottom zone. Each step picks a node ([`Scheduler::pick_node`]): in a
//! one-direction region from that zone; in a bidirectional one the zones'
//! best candidates are compared across zones. The node's zone advances its
//! clock (`bumpNode`) and the node's neighbours in that direction are
//! released once all of their other neighbours are scheduled, with the ready
//! cycle the edge's latency gives. The order is the top nodes in the order
//! picked, then the bottom nodes in the reverse.
//!
//! This is `GenericScheduler::pickNode`, `pickNodeBidirectional`,
//! `pickNodeFromQueue`, `setPolicy`, `schedNode` (MachineScheduler.cpp#L3254-
//! L4300) and `ScheduleDAGMI::schedule`, `releaseSucc`, `releasePred`.
//!
//! # Not translated
//! - SB3: `ExitSU` as a node. The terminator stays a schedulable node, so it
//!   takes an issue slot the pin's `ExitSU` does not. The edge it holds for an
//!   unread def (`buildSchedGraph`, latency less one) is translated for a model
//!   that asks (`SchedulingModel::exit_edges`, arm64); the x86 models do not
//!   and keep the deepest root as the critical path.
//! - SB4: `computeCyclicCriticalPath`: `IsAcyclicLatencyLimited` stays off. It
//!   is only asked of a model with a micro-op buffer (Cyclone), not the
//!   in-order A510.
//! - SB5b: the fast-cluster path of `BaseMemOpClusterMutation` (threshold 1000).
//! - SB5: macro fusion, and
//!   `tryBiasPhysRegs`/`reschedulePhysReg` (no region holds a physical
//!   register, MS4).
//!
//! - SB9: a pre/post-indexed access reports no memory operand to the cluster
//!   mutation (`getMemOperandsWithOffsetWidth`); it exists only after
//!   `ldst_update`.
//!
//! Owner arm64-phi; exit with the model branch that first needs each.

use crate::backend::genericsched::{Cand, CandPolicy, Reason, Region, ResourceDelta, Zone, try_candidate};
use crate::backend::machinesched::Graph;
use crate::backend::regpressure::Delta;
use crate::backend::schedboundary::{Boundary, Machine, Remainder, Unit};

/// What follows the pressure of the two zones (`TopRPTracker` and
/// `BotRPTracker`).
pub trait ZonePressure {
    /// `RPDelta` of placing `node` at the top or the bottom.
    fn delta(
        &self,
        node: usize,
        at_top: bool,
    ) -> Delta;
    /// `node` is placed at the top or the bottom.
    fn place(
        &mut self,
        node: usize,
        at_top: bool,
    );
}

/// `RegionPolicy` (what `initPolicy` and the subtarget's `overrideSchedPolicy`
/// set).
#[derive(Clone, Copy, Debug)]
pub struct RegionPolicy {
    pub only_top_down: bool,
    pub only_bottom_up: bool,
    /// `DisableLatencyHeuristic`.
    pub disable_latency: bool,
}

impl RegionPolicy {
    /// The generic default: bottom-up only.
    pub fn bottom_up() -> RegionPolicy {
        RegionPolicy { only_top_down: false, only_bottom_up: true, disable_latency: false }
    }

    /// AArch64's `overrideSchedPolicy`.
    pub fn bidirectional(disable_latency: bool) -> RegionPolicy {
        RegionPolicy { only_top_down: false, only_bottom_up: false, disable_latency }
    }
}

/// `checkResourceLimit` before a node is scheduled.
fn resource_limited(
    l_factor: u32,
    count: u32,
    latency: u32,
) -> bool {
    i64::from(count) - i64::from(latency) * i64::from(l_factor) > i64::from(l_factor)
}

pub struct Scheduler<'a> {
    graph: &'a Graph,
    machine: &'a Machine,
    units: Vec<Unit>,
    policy: RegionPolicy,
    pressure: Option<&'a mut dyn ZonePressure>,
    top: Boundary,
    bot: Boundary,
    rem: Remainder,
    /// Edges to successors with their latency, by node.
    succ_edges: Vec<Vec<(usize, u32)>>,
    preds_left: Vec<usize>,
    succs_left: Vec<usize>,
    weak_preds_left: Vec<usize>,
    weak_succs_left: Vec<usize>,
    /// `TopClusterID` and `BotClusterID`: the cluster of the node each zone
    /// scheduled last.
    top_cluster: Option<u32>,
    bot_cluster: Option<u32>,
    scheduled: Vec<bool>,
    top_cand: Cand,
    bot_cand: Cand,
}

impl<'a> Scheduler<'a> {
    /// `units` gives each node's micro-ops, resources and flags; its depth and
    /// height are computed here from the graph's edge latencies.
    pub fn new(
        graph: &'a Graph,
        machine: &'a Machine,
        mut units: Vec<Unit>,
        policy: RegionPolicy,
        pressure: Option<&'a mut dyn ZonePressure>,
    ) -> Scheduler<'a> {
        let n = units.len();
        let mut succ_edges = vec![Vec::new(); n];
        for (node, preds) in graph.preds.iter().enumerate() {
            for (at, &pred) in preds.iter().enumerate() {
                succ_edges[pred].push((node, graph.latencies[node][at]));
            }
        }
        for node in 0..n {
            units[node].depth = graph.preds[node]
                .iter()
                .enumerate()
                .map(|(at, &pred)| units[pred].depth + graph.latencies[node][at])
                .max()
                .unwrap_or(0);
        }
        for node in (0..n).rev() {
            units[node].height = succ_edges[node]
                .iter()
                .map(|&(succ, lat)| units[succ].height + lat)
                .max()
                .unwrap_or(0)
                .max(graph.exit_latency[node]);
        }
        let rem = if machine.has_instr_sched_model() { Remainder::new(machine, &units) } else { Remainder::default() };
        let idle = Cand::new(0, false, CandPolicy::default());
        Scheduler {
            graph,
            machine,
            policy,
            pressure,
            top: Boundary::new(machine, true),
            bot: Boundary::new(machine, false),
            rem,
            preds_left: graph.preds.iter().map(Vec::len).collect(),
            succs_left: graph.succs.iter().map(Vec::len).collect(),
            weak_preds_left: graph.weak_preds.iter().map(Vec::len).collect(),
            weak_succs_left: graph.weak_succs.iter().map(Vec::len).collect(),
            top_cluster: None,
            bot_cluster: None,
            scheduled: vec![false; n],
            succ_edges,
            units,
            top_cand: idle,
            bot_cand: idle,
        }
    }

    fn region(&self) -> Region {
        Region {
            tracks_pressure: self.pressure.is_some(),
            acyclic_latency_limited: self.rem.is_acyclic_latency_limited,
            disable_latency: self.policy.disable_latency,
            bias_pregs_extra: false,
        }
    }

    /// `computeRemLatency`.
    fn rem_latency(
        &self,
        zone: &Boundary,
    ) -> u32 {
        let longest =
            |nodes: &[usize]| nodes.iter().map(|&node| zone.unscheduled_latency(&self.units[node])).max().unwrap_or(0);
        zone.dependent_latency().max(longest(&zone.available)).max(longest(&zone.pending))
    }

    /// `shouldReduceLatency`.
    fn should_reduce_latency(
        &self,
        zone: &Boundary,
        rem_latency: &mut Option<u32>,
    ) -> bool {
        if zone.curr_cycle() > self.rem.critical_path {
            return true;
        }
        if zone.curr_cycle() == 0 {
            return false;
        }
        let rem = *rem_latency.get_or_insert_with(|| self.rem_latency(zone));
        rem + zone.curr_cycle() > self.rem.critical_path
    }

    /// `setPolicy`, not post-RA.
    fn set_policy(
        &self,
        zone: &Boundary,
        other: &Boundary,
    ) -> CandPolicy {
        let mut policy = CandPolicy::default();
        let (other_count, other_crit) = if self.machine.has_instr_sched_model() {
            other.other_resource_count(self.machine, &self.rem)
        } else {
            (0, 0)
        };
        let mut rem_latency = None;
        let mut other_limited = false;
        if self.machine.has_instr_sched_model() && other_count != 0 {
            let rem = *rem_latency.get_or_insert_with(|| self.rem_latency(zone));
            other_limited = resource_limited(self.machine.latency_factor(), other_count, rem);
        }
        if !other_limited && self.should_reduce_latency(zone, &mut rem_latency) {
            policy.reduce_latency = true;
        }
        if zone.zone_crit_res_idx() == other_crit {
            return policy;
        }
        if zone.is_resource_limited() && policy.reduce_res_idx == 0 {
            policy.reduce_res_idx = zone.zone_crit_res_idx();
        }
        if other_limited {
            policy.demand_res_idx = other_crit;
        }
        policy
    }

    fn init_candidate(
        &self,
        node: usize,
        at_top: bool,
        policy: CandPolicy,
    ) -> Cand {
        let zone = if at_top { &self.top } else { &self.bot };
        let unit = &self.units[node];
        let mut cand = Cand::new(node, at_top, policy);
        if let Some(pressure) = &self.pressure {
            cand.pressure = pressure.delta(node, at_top);
        }
        cand.phys_bias = self.bias_phys_reg(node, at_top);
        cand.stall = zone.latency_stall_cycles(unit);
        // `isTheSameCluster(ZoneCluster, SU->ParentClusterIdx)`.
        let zone_cluster = if at_top { self.top_cluster } else { self.bot_cluster };
        cand.cluster_succ = zone_cluster.is_some() && zone_cluster == self.graph.parent_cluster[node];
        cand.weak_left = if at_top { self.weak_preds_left[node] } else { self.weak_succs_left[node] };
        cand.depth = unit.depth;
        cand.height = unit.height;
        cand
    }

    /// `biasPhysReg` for a copy: schedule it at once when the physical end is
    /// already scheduled, and a copy of a physical register in (out) at the
    /// bottom (top) at once unless it is at the region's boundary, where it
    /// waits. `ExitSU` edges (`exit_latency`) count as successors. The
    /// pin's `isMoveImmediate` case needs a physical def, which no node has
    /// before allocation, and `BiasPRegsExtra` is SystemZ's.
    fn bias_phys_reg(
        &self,
        node: usize,
        at_top: bool,
    ) -> i32 {
        let Some((dest, source)) = self.units[node].copy_physical else { return 0 };
        let (scheduled, unscheduled) = if at_top { (source, dest) } else { (dest, source) };
        if scheduled {
            return 1;
        }
        if !unscheduled {
            return 0;
        }
        let at_boundary =
            if at_top { self.succs_left[node] == 0 && self.graph.exit_latency[node] == 0 } else { self.preds_left[node] == 0 };
        if at_boundary { -1 } else { 1 }
    }

    /// `initResourceDelta`.
    fn init_resource_delta(
        &self,
        cand: &mut Cand,
    ) {
        if cand.policy.reduce_res_idx == 0 && cand.policy.demand_res_idx == 0 {
            return;
        }
        let mut delta = ResourceDelta::default();
        for write in &self.units[cand.node].writes {
            if write.idx == cand.policy.reduce_res_idx {
                delta.crit_resources += write.release_at_cycle;
            }
            if write.idx == cand.policy.demand_res_idx {
                delta.demanded_resources += write.release_at_cycle;
            }
        }
        cand.resources = delta;
    }

    fn zone_view(zone: &Boundary) -> Zone {
        Zone { is_top: zone.is_top, curr_mops: zone.curr_mops(), scheduled_latency: zone.scheduled_latency() }
    }

    /// `pickNodeFromQueue`: the best of a zone's available nodes.
    fn pick_from_queue(
        &self,
        at_top: bool,
        policy: CandPolicy,
    ) -> Cand {
        let zone = if at_top { &self.top } else { &self.bot };
        let view = Self::zone_view(zone);
        let region = self.region();
        let mut best = Cand::new(usize::MAX, at_top, policy);
        for &node in &zone.available {
            let mut try_cand = self.init_candidate(node, at_top, policy);
            self.init_resource_delta(&mut try_cand);
            if try_candidate(&mut best, &mut try_cand, Some(&view), &region) {
                best = try_cand;
            }
        }
        if llrm_support::env_flag!("LLRM_MS_TRACE") {
            eprintln!(
                "MSC top={at_top} available={} pending={} best={} reason={:?} excess={:?} critical={:?} current={:?}",
                zone.available.len(),
                zone.pending.len(),
                best.node,
                best.reason,
                best.pressure.excess,
                best.pressure.critical_max,
                best.pressure.current_max
            );
        }
        best
    }

    fn pick_bidirectional(&mut self) -> (usize, bool, Reason) {
        if let Some(node) = self.bot.pick_only_choice(self.machine, &self.units) {
            return (node, false, Reason::Only1);
        }
        if let Some(node) = self.top.pick_only_choice(self.machine, &self.units) {
            return (node, true, Reason::Only1);
        }
        let bot_policy = self.set_policy(&self.bot, &self.top);
        let top_policy = self.set_policy(&self.top, &self.bot);
        // A candidate stays valid while its node is unscheduled and the policy
        // is the same.
        let stale = |cand: &Cand, scheduled: &[bool], policy: CandPolicy| {
            cand.reason == Reason::NoCand || scheduled[cand.node] || cand.policy != policy
        };
        if stale(&self.bot_cand, &self.scheduled, bot_policy) {
            self.bot_cand = self.pick_from_queue(false, bot_policy);
        }
        if stale(&self.top_cand, &self.scheduled, top_policy) {
            self.top_cand = self.pick_from_queue(true, top_policy);
        }
        let mut best = self.bot_cand;
        let mut try_cand = self.top_cand;
        try_cand.reason = Reason::NoCand;
        if try_candidate(&mut best, &mut try_cand, None, &self.region()) {
            best = try_cand;
        }
        (best.node, best.at_top, best.reason)
    }

    /// `pickNode`.
    fn pick_node(&mut self) -> (usize, bool, Reason) {
        let (node, at_top, reason) = if self.policy.only_top_down {
            match self.top.pick_only_choice(self.machine, &self.units) {
                Some(node) => (node, true, Reason::Only1),
                None => {
                    let cand = self.pick_from_queue(true, CandPolicy::default());
                    (cand.node, true, cand.reason)
                }
            }
        } else if self.policy.only_bottom_up {
            match self.bot.pick_only_choice(self.machine, &self.units) {
                Some(node) => (node, false, Reason::Only1),
                None => {
                    let cand = self.pick_from_queue(false, CandPolicy::default());
                    (cand.node, false, cand.reason)
                }
            }
        } else {
            self.pick_bidirectional()
        };
        // A node ready on both sides is in both queues (or pending).
        if self.preds_left[node] == 0 {
            self.top.remove_ready(node);
        }
        if self.succs_left[node] == 0 {
            self.bot.remove_ready(node);
        }
        (node, at_top, reason)
    }

    /// `schedNode` and `updateQueues`.
    fn sched_node(
        &mut self,
        node: usize,
        at_top: bool,
    ) {
        self.scheduled[node] = true;
        if at_top {
            self.top_cluster = self.graph.parent_cluster[node];
        } else {
            self.bot_cluster = self.graph.parent_cluster[node];
        }
        if at_top {
            let cycle = self.top.curr_cycle();
            self.units[node].top_ready_cycle = self.units[node].top_ready_cycle.max(cycle);
            self.top.bump_node(self.machine, &self.units[node], &mut self.rem);
            if let Some(pressure) = &mut self.pressure {
                pressure.place(node, true);
            }
            for &weak in &self.graph.weak_succs[node] {
                self.weak_preds_left[weak] -= 1;
            }
            for at in 0..self.succ_edges[node].len() {
                let (succ, latency) = self.succ_edges[node][at];
                let ready = self.units[node].top_ready_cycle + latency;
                self.units[succ].top_ready_cycle = self.units[succ].top_ready_cycle.max(ready);
                self.preds_left[succ] -= 1;
                if self.preds_left[succ] == 0 && !self.scheduled[succ] {
                    let ready = self.units[succ].top_ready_cycle;
                    self.top.release_node(self.machine, &self.units, succ, ready, false, 0);
                }
            }
        } else {
            let cycle = self.bot.curr_cycle();
            self.units[node].bot_ready_cycle = self.units[node].bot_ready_cycle.max(cycle);
            self.bot.bump_node(self.machine, &self.units[node], &mut self.rem);
            if let Some(pressure) = &mut self.pressure {
                pressure.place(node, false);
            }
            for &weak in &self.graph.weak_preds[node] {
                self.weak_succs_left[weak] -= 1;
            }
            for at in 0..self.graph.preds[node].len() {
                let pred = self.graph.preds[node][at];
                let latency = self.graph.latencies[node][at];
                let ready = self.units[node].bot_ready_cycle + latency;
                self.units[pred].bot_ready_cycle = self.units[pred].bot_ready_cycle.max(ready);
                self.succs_left[pred] -= 1;
                if self.succs_left[pred] == 0 && !self.scheduled[pred] {
                    let ready = self.units[pred].bot_ready_cycle;
                    self.bot.release_node(self.machine, &self.units, pred, ready, false, 0);
                }
            }
        }
    }

    /// `ScheduleDAGMI::schedule`: the order of the region, with the reason of
    /// each pick in the order made: the node, whether from the top, and why
    /// (`-debug-only=machine-scheduler`'s `Pick`).
    pub fn schedule(mut self) -> (Vec<usize>, Vec<(usize, bool, Reason)>) {
        let n = self.units.len();
        for node in 0..n {
            if self.preds_left[node] == 0 {
                self.top.release_node(self.machine, &self.units, node, 0, false, 0);
            }
        }
        // Bottom roots in reverse, so the higher priority nodes appear first.
        // The region end releases the defs that hold a latency to it
        // (`ExitSU`), ready that many cycles into the bottom zone.
        for node in 0..n {
            self.units[node].bot_ready_cycle = self.graph.exit_latency[node];
        }
        for node in (0..n).rev() {
            if self.succs_left[node] == 0 {
                self.bot.release_node(self.machine, &self.units, node, self.graph.exit_latency[node], false, 0);
            }
        }
        // `registerRoots`: `ExitSU`'s depth is the deepest of what reaches it.
        let exit_depth = (0..n).map(|node| self.units[node].depth + self.graph.exit_latency[node]).max().unwrap_or(0);
        self.rem.critical_path = self
            .bot
            .available
            .iter()
            .map(|&node| self.units[node].depth)
            .max()
            .unwrap_or(0)
            .max(exit_depth);
        let (mut top, mut bottom, mut reasons) = (Vec::new(), Vec::new(), Vec::new());
        while top.len() + bottom.len() < n {
            let (node, at_top, reason) = self.pick_node();
            reasons.push((node, at_top, reason));
            self.sched_node(node, at_top);
            if at_top {
                top.push(node);
            } else {
                bottom.push(node);
            }
        }
        bottom.reverse();
        top.extend(bottom);
        (top, reasons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::schedboundary::{ProcResource, WriteRes};

    /// Test data, not a model of any processor: a one-wide machine with a
    /// load/store pipe and a divider, both one unit; unbuffered (in order) or
    /// with a micro-op buffer.
    fn machine(buffer: u32) -> Machine {
        let size = if buffer == 0 { 0 } else { -1 };
        Machine::new(
            1,
            buffer,
            vec![
                ProcResource { num_units: 0, buffer_size: -1, sub_units: vec![] },
                ProcResource { num_units: 1, buffer_size: size, sub_units: vec![] },
                ProcResource { num_units: 1, buffer_size: size, sub_units: vec![] },
            ],
        )
    }

    fn unit(
        resource: usize,
        buffer: u32,
    ) -> Unit {
        Unit {
            micro_ops: 1,
            writes: vec![WriteRes { idx: resource, release_at_cycle: 1, acquire_at_cycle: 0 }],
            is_unbuffered: buffer == 0,
            has_reserved_resource: buffer == 0,
            ..Unit::default()
        }
    }

    /// Six stores, a load, and two dependent divides on the load:
    /// `0..=5` stores, `6` the load (3 cycles to the first divide), `7` and
    /// `8` the divides (6 cycles between them). The stores have no edge to
    /// the rest.
    fn diamond(buffer: u32) -> (Graph, Machine, Vec<Unit>) {
        let graph = Graph::from_edges(9, &[(6, 7, 3), (7, 8, 6)]);
        let mut units: Vec<Unit> = (0..6).map(|_| unit(1, buffer)).collect();
        units.push(unit(1, buffer));
        units.push(unit(2, buffer));
        units.push(unit(2, buffer));
        (graph, machine(buffer), units)
    }

    fn order(
        buffer: u32,
        policy: RegionPolicy,
    ) -> Vec<usize> {
        let (graph, machine, units) = diamond(buffer);
        Scheduler::new(&graph, &machine, units, policy, None).schedule().0
    }

    /// pin: a def nothing in the region reads holds `latency - 1` to the
    /// region end (`ExitSU`), so on an in-order machine the bottom zone
    /// leaves it pending and places the independent stores first: the def
    /// is issued first in the region. Without the edge the three tie and
    /// source order stands, the def last of the stores.
    #[test]
    fn test_a_live_out_def_with_latency_is_issued_before_independent_stores() {
        let mut graph = Graph::from_edges(3, &[]);
        let units = vec![unit(2, 0), unit(1, 0), unit(1, 0)];
        let machine = machine(0);
        let held = Scheduler::new(&graph, &machine, units.clone(), RegionPolicy::bottom_up(), None).schedule().0;
        assert_eq!(held, vec![0, 1, 2], "source order without the exit edge");
        graph.exit_latency[2] = 3;
        let got = Scheduler::new(&graph, &machine, units, RegionPolicy::bottom_up(), None).schedule().0;
        assert_eq!(got[0], 2, "the live-out def stayed behind the stores: {got:?}");
    }

    /// pin: `biasPhysReg`. A copy of a physical register in (`COPY $wzr`) is
    /// scheduled first from the top (the physical end is scheduled) and
    /// deferred at the bottom (the register is at the boundary), so the
    /// bidirectional zones start it from the top. nbody O2's 22-node
    /// preheader began with the bottom's pick of another node and the
    /// constants it held lived through the loop.
    #[test]
    fn test_a_copy_of_a_physical_register_is_started_from_the_top() {
        let graph = Graph::from_edges(3, &[]);
        let machine = machine(0);
        let plain = vec![unit(1, 0), unit(1, 0), unit(1, 0)];
        let mut biased = plain.clone();
        biased[2].copy_physical = Some((false, true));
        let policy = RegionPolicy::bidirectional(false);
        let without = Scheduler::new(&graph, &machine, plain, policy, None).schedule().0;
        let with = Scheduler::new(&graph, &machine, biased, policy, None).schedule().0;
        assert_ne!(without[0], 2, "no bias: the copy is not first ({without:?})");
        assert_eq!(with[0], 2, "the copy of a physical register starts from the top ({with:?})");
    }

    /// pin: `llc -mcpu=generic` (in order) hoists the load and the divides
    /// above stores that do not feed them; `-mcpu=apple-m1` (out of order)
    /// keeps source order. The stall of the second divide on the first lets
    /// the bottom zone place stores meanwhile.
    #[test]
    fn test_an_in_order_machine_hoists_a_load_chain_above_independent_stores() {
        for policy in [RegionPolicy::bottom_up(), RegionPolicy::bidirectional(false)] {
            let got = order(0, policy);
            let at = |node: usize| got.iter().position(|&one| one == node).unwrap();
            assert!(at(6) < at(5), "the load stays after the stores: {got:?}");
        }
    }

    /// By hand, bottom-up, one-wide: divide 8 is placed first (latest node);
    /// divide 7 is ready 6 cycles later, so stores 5..1 fill cycles 1-5; at
    /// cycle 6 divide 7 beats store 0 by order, store 0 follows, and the
    /// load (ready at 9) comes last. Placed 8,5,4,3,2,1,7,0,6; the order is
    /// that reversed.
    #[test]
    fn test_the_stall_of_a_divide_chain_interleaves_stores_between_its_links() {
        assert_eq!(order(0, RegionPolicy::bottom_up()), vec![6, 0, 7, 1, 2, 3, 4, 5, 8]);
    }

    #[test]
    fn test_a_machine_with_a_buffer_keeps_source_order_where_nothing_stalls() {
        let got = order(192, RegionPolicy::bottom_up());
        assert_eq!(got, (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn test_every_node_is_placed_once_whatever_the_policy() {
        for policy in [
            RegionPolicy::bottom_up(),
            RegionPolicy::bidirectional(false),
            RegionPolicy { only_top_down: true, only_bottom_up: false, disable_latency: false },
        ] {
            for buffer in [0, 192] {
                let mut got = order(buffer, policy);
                got.sort_unstable();
                assert_eq!(got, (0..9).collect::<Vec<_>>());
            }
        }
    }
}
