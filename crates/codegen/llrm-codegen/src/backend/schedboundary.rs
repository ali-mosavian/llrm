//! `SchedBoundary`: the clock, the ready queues and the resource counts of one
//! side of a scheduling region (row 65, S2 of ARM64-SCHED-HOIST).
//!
//! # How it works
//! A zone grows from the top or the bottom of the region. It holds the nodes
//! that may be scheduled next (`Available`), the ones that are ready in the
//! dependence sense but would stall (`Pending`), the current cycle, how many
//! micro-ops issued in it, and how many units of each processor resource the
//! scheduled nodes used, scaled so resources with different numbers of units
//! compare (`resource factor`).
//!
//! ```text
//! releaseNode  ready at cycle c > now on an unbuffered machine  -> Pending
//!              hazard (issue width, group, a reserved resource)  -> Pending
//!              else                                              -> Available
//! pickOnlyChoice  release pending, defer new hazards, bump the clock until
//!              something is Available; one node there is the only choice
//! bumpNode     advance the clock for stalls, count micro-ops and resources,
//!              reserve unbuffered resources, move ExpectedLatency
//! ```
//!
//! This is the pin's SchedBoundary (MachineScheduler.cpp#L2492-L3198) and the
//! factors of `TargetSchedModel::init` (TargetSchedule.cpp#L53-L66).
//!
//! # Not translated
//! `ScheduleHazardRecognizer` (the pre-RA AArch64 scheduler has none) and
//! `ResourceSegments` (`enableIntervals`, off for A510 and Cyclone). DEVIATE
//! SB1, owner arm64-phi, exit with the first model that turns them on.

/// `MCProcResourceDesc`.
#[derive(Clone, Debug)]
pub struct ProcResource {
    pub num_units: u32,
    /// -1 where the model does not say.
    pub buffer_size: i32,
    /// The member resources of a group (`SubUnitsIdxBegin`), empty if none.
    pub sub_units: Vec<usize>,
}

/// `MCWriteProcResEntry`.
#[derive(Clone, Copy, Debug)]
pub struct WriteRes {
    pub idx: usize,
    pub release_at_cycle: u32,
    pub acquire_at_cycle: u32,
}

/// A schedule model as the boundary reads it (`TargetSchedModel`).
#[derive(Clone, Debug)]
pub struct Machine {
    pub issue_width: u32,
    pub micro_op_buffer_size: u32,
    /// Resource 0 is the invalid one the pin reserves.
    pub resources: Vec<ProcResource>,
    resource_lcm: u32,
    micro_op_factor: u32,
    resource_factors: Vec<u32>,
}

impl Machine {
    /// `TargetSchedModel::init`: the factors that normalise units, micro-ops
    /// and cycles.
    pub fn new(issue_width: u32, micro_op_buffer_size: u32, resources: Vec<ProcResource>) -> Machine {
        let gcd = |mut a: u32, mut b: u32| {
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        };
        let mut lcm = issue_width;
        for resource in &resources {
            if resource.num_units > 0 {
                lcm = lcm / gcd(lcm, resource.num_units) * resource.num_units;
            }
        }
        let factors = resources.iter().map(|one| if one.num_units == 0 { 0 } else { lcm / one.num_units }).collect();
        Machine {
            issue_width,
            micro_op_buffer_size,
            resources,
            resource_lcm: lcm,
            micro_op_factor: lcm / issue_width,
            resource_factors: factors,
        }
    }

    pub fn latency_factor(&self) -> u32 {
        self.resource_lcm
    }

    pub fn micro_op_factor(&self) -> u32 {
        self.micro_op_factor
    }

    pub fn resource_factor(&self, idx: usize) -> u32 {
        self.resource_factors[idx]
    }
}

/// What the boundary reads of an `SUnit`.
#[derive(Clone, Debug, Default)]
pub struct Unit {
    pub micro_ops: u32,
    pub begin_group: bool,
    pub end_group: bool,
    pub writes: Vec<WriteRes>,
    /// Uses a resource whose buffer is 0 (`SU->isUnbuffered`).
    pub is_unbuffered: bool,
    /// Uses a reserved (unbuffered, grouped) resource (`hasReservedResource`).
    pub has_reserved_resource: bool,
    pub depth: u32,
    pub height: u32,
    pub top_ready_cycle: u32,
    pub bot_ready_cycle: u32,
}

/// `SchedRemainder`: what the whole region still has to issue.
#[derive(Clone, Debug, Default)]
pub struct Remainder {
    pub critical_path: u32,
    pub is_acyclic_latency_limited: bool,
    pub rem_issue_count: u32,
    pub remaining_counts: Vec<u32>,
}

impl Remainder {
    /// `SchedRemainder::init`.
    pub fn new(machine: &Machine, units: &[Unit]) -> Remainder {
        let mut rem = Remainder { remaining_counts: vec![0; machine.resources.len()], ..Remainder::default() };
        for unit in units {
            rem.rem_issue_count += unit.micro_ops * machine.micro_op_factor();
            for write in &unit.writes {
                rem.remaining_counts[write.idx] +=
                    machine.resource_factor(write.idx) * (write.release_at_cycle - write.acquire_at_cycle);
            }
        }
        rem
    }
}

const INVALID_CYCLE: u32 = u32::MAX;
/// `-misched-limit`.
const READY_LIST_LIMIT: usize = 256;

/// `checkResourceLimit`.
fn check_resource_limit(l_factor: u32, count: u32, latency: u32, after_sched_node: bool) -> bool {
    let res_cnt_factor = i64::from(count) - i64::from(latency) * i64::from(l_factor);
    if after_sched_node { res_cnt_factor >= i64::from(l_factor) } else { res_cnt_factor > i64::from(l_factor) }
}

#[derive(Clone, Debug)]
pub struct Boundary {
    pub is_top: bool,
    pub available: Vec<usize>,
    pub pending: Vec<usize>,
    check_pending: bool,
    curr_cycle: u32,
    curr_mops: u32,
    min_ready_cycle: u32,
    expected_latency: u32,
    dependent_latency: u32,
    retired_mops: u32,
    executed: Vec<u32>,
    max_executed: u32,
    zone_crit_res_idx: usize,
    is_resource_limited: bool,
    reserved_cycles: Vec<u32>,
    reserved_index: Vec<usize>,
}

impl Boundary {
    /// `SchedBoundary::init`.
    pub fn new(machine: &Machine, is_top: bool) -> Boundary {
        let mut reserved_index = Vec::with_capacity(machine.resources.len());
        let mut units = 0;
        for resource in &machine.resources {
            reserved_index.push(units);
            units += resource.num_units as usize;
        }
        Boundary {
            is_top,
            available: Vec::new(),
            pending: Vec::new(),
            check_pending: false,
            curr_cycle: 0,
            curr_mops: 0,
            min_ready_cycle: u32::MAX,
            expected_latency: 0,
            dependent_latency: 0,
            retired_mops: 0,
            executed: vec![0; machine.resources.len()],
            max_executed: 0,
            zone_crit_res_idx: 0,
            is_resource_limited: false,
            reserved_cycles: vec![INVALID_CYCLE; units],
            reserved_index,
        }
    }

    pub fn curr_cycle(&self) -> u32 {
        self.curr_cycle
    }

    pub fn curr_mops(&self) -> u32 {
        self.curr_mops
    }

    pub fn is_resource_limited(&self) -> bool {
        self.is_resource_limited
    }

    pub fn zone_crit_res_idx(&self) -> usize {
        self.zone_crit_res_idx
    }

    /// `getDependentLatency`.
    pub fn dependent_latency(&self) -> u32 {
        self.dependent_latency
    }

    /// `getScheduledLatency`.
    pub fn scheduled_latency(&self) -> u32 {
        self.expected_latency.max(self.curr_cycle)
    }

    /// `getUnscheduledLatency`.
    pub fn unscheduled_latency(&self, unit: &Unit) -> u32 {
        if self.is_top { unit.height } else { unit.depth }
    }

    /// `getResourceCount`.
    pub fn resource_count(&self, idx: usize) -> u32 {
        self.executed[idx]
    }

    /// `getCriticalCount`.
    pub fn critical_count(&self, machine: &Machine) -> u32 {
        if self.zone_crit_res_idx == 0 {
            self.retired_mops * machine.micro_op_factor()
        } else {
            self.resource_count(self.zone_crit_res_idx)
        }
    }

    /// `getExecutedCount`.
    pub fn executed_count(&self, machine: &Machine) -> u32 {
        (self.curr_cycle * machine.latency_factor()).max(self.max_executed)
    }

    fn ready_cycle(&self, unit: &Unit) -> u32 {
        if self.is_top { unit.top_ready_cycle } else { unit.bot_ready_cycle }
    }

    /// `getLatencyStallCycles`: the soft stall of a node that reads an
    /// unbuffered resource.
    pub fn latency_stall_cycles(&self, unit: &Unit) -> u32 {
        if !unit.is_unbuffered {
            return 0;
        }
        self.ready_cycle(unit).saturating_sub(self.curr_cycle)
    }

    fn is_reserved_group(machine: &Machine, idx: usize) -> bool {
        let resource = &machine.resources[idx];
        !resource.sub_units.is_empty() && resource.buffer_size == 0
    }

    /// `getNextResourceCycleByInstance`, without intervals.
    fn next_resource_cycle_by_instance(&self, instance: usize, release_at_cycle: u32) -> u32 {
        let next_unreserved = self.reserved_cycles[instance];
        // A resource never used is available now.
        if next_unreserved == INVALID_CYCLE {
            return self.curr_cycle;
        }
        // Bottom-up adds the cycles the operation needs.
        if self.is_top { next_unreserved } else { self.curr_cycle.max(next_unreserved + release_at_cycle) }
    }

    /// `getNextResourceCycle`: the next cycle the resource can be used and
    /// the instance that gives it.
    pub fn next_resource_cycle(
        &self,
        machine: &Machine,
        unit: &Unit,
        idx: usize,
        release_at_cycle: u32,
        acquire_at_cycle: u32,
    ) -> (u32, usize) {
        let start = self.reserved_index[idx];
        let instances = machine.resources[idx].num_units as usize;
        if Self::is_reserved_group(machine, idx) {
            // A node that uses a member of the group is hazarded by the member.
            if unit.writes.iter().any(|entry| machine.resources[idx].sub_units.contains(&entry.idx)) {
                return (self.next_resource_cycle_by_instance(start, release_at_cycle), start);
            }
            let (mut best, mut best_instance) = (INVALID_CYCLE, 0);
            for &member in &machine.resources[idx].sub_units[..instances] {
                let (next, at) = self.next_resource_cycle(machine, unit, member, release_at_cycle, acquire_at_cycle);
                if best > next {
                    best_instance = at;
                    best = next;
                }
            }
            return (best, best_instance);
        }
        let (mut best, mut best_instance) = (INVALID_CYCLE, 0);
        for instance in start..start + instances {
            let next = self.next_resource_cycle_by_instance(instance, release_at_cycle);
            if best > next {
                best_instance = instance;
                best = next;
            }
        }
        (best, best_instance)
    }

    /// `checkHazard`: the node cannot issue in the current cycle.
    pub fn check_hazard(&self, machine: &Machine, unit: &Unit) -> bool {
        let uops = unit.micro_ops;
        if self.curr_mops > 0 && self.curr_mops + uops > machine.issue_width {
            return true;
        }
        if self.curr_mops > 0 && ((self.is_top && unit.begin_group) || (!self.is_top && unit.end_group)) {
            return true;
        }
        if unit.has_reserved_resource {
            for entry in &unit.writes {
                let (next, _) =
                    self.next_resource_cycle(machine, unit, entry.idx, entry.release_at_cycle, entry.acquire_at_cycle);
                if next > self.curr_cycle {
                    return true;
                }
            }
        }
        false
    }

    /// `releaseNode`: a node whose dependences are met enters the zone.
    /// `index` is its place in `pending` when `in_pending_queue`.
    pub fn release_node(
        &mut self,
        machine: &Machine,
        units: &[Unit],
        node: usize,
        ready_cycle: u32,
        in_pending_queue: bool,
        index: usize,
    ) {
        self.min_ready_cycle = self.min_ready_cycle.min(ready_cycle);
        // Interlocks first: to the other heuristics an instruction that cannot
        // issue is not in the queue.
        let is_buffered = machine.micro_op_buffer_size != 0;
        let mut hazard = !is_buffered && ready_cycle > self.curr_cycle;
        if !hazard {
            hazard = self.check_hazard(machine, &units[node]);
        }
        if !hazard && self.available.len() >= READY_LIST_LIMIT {
            hazard = true;
        }
        if !hazard {
            self.available.push(node);
            if in_pending_queue {
                self.pending.remove(index);
            }
            return;
        }
        if !in_pending_queue {
            self.pending.push(node);
        }
    }

    /// `bumpCycle`.
    pub fn bump_cycle(&mut self, machine: &Machine, mut next_cycle: u32) {
        if machine.micro_op_buffer_size == 0 {
            assert!(self.min_ready_cycle < u32::MAX, "MinReadyCycle uninitialized");
            next_cycle = next_cycle.max(self.min_ready_cycle);
        }
        // The micro-ops of the cycle left issue in it.
        let dec_mops = machine.issue_width * (next_cycle - self.curr_cycle);
        self.curr_mops = self.curr_mops.saturating_sub(dec_mops);
        let passed = next_cycle - self.curr_cycle;
        self.dependent_latency = self.dependent_latency.saturating_sub(passed);
        self.curr_cycle = next_cycle;
        self.check_pending = true;
        self.is_resource_limited = check_resource_limit(
            machine.latency_factor(),
            self.critical_count(machine),
            self.scheduled_latency(),
            true,
        );
    }

    fn count_resource(
        &mut self,
        machine: &Machine,
        unit: &Unit,
        idx: usize,
        release_at_cycle: u32,
        acquire_at_cycle: u32,
        rem: &mut Remainder,
    ) -> u32 {
        let count = machine.resource_factor(idx) * (release_at_cycle - acquire_at_cycle);
        self.executed[idx] += count;
        self.max_executed = self.max_executed.max(self.executed[idx]);
        assert!(rem.remaining_counts[idx] >= count, "resource double counted");
        rem.remaining_counts[idx] -= count;
        // A resource past the critical one becomes it.
        if self.zone_crit_res_idx != idx && self.resource_count(idx) > self.critical_count(machine) {
            self.zone_crit_res_idx = idx;
        }
        self.next_resource_cycle(machine, unit, idx, release_at_cycle, acquire_at_cycle).0
    }

    /// `bumpNode`: the node is scheduled in this zone.
    pub fn bump_node(&mut self, machine: &Machine, unit: &Unit, rem: &mut Remainder) {
        let inc_mops = unit.micro_ops;
        assert!(
            self.curr_mops == 0 || self.curr_mops + inc_mops <= machine.issue_width,
            "cannot schedule this instruction's micro-ops in the current cycle"
        );
        let ready = self.ready_cycle(unit);
        let mut next_cycle = self.curr_cycle;
        match machine.micro_op_buffer_size {
            0 => assert!(ready <= self.curr_cycle, "broken PendingQueue"),
            1 => {
                if ready > next_cycle {
                    next_cycle = ready;
                }
            },
            _ => {
                // The reorder buffer is not modelled: scheduled micro-ops
                // count as retired; an in-order resource may still stall.
                if unit.is_unbuffered && ready > next_cycle {
                    next_cycle = ready;
                }
            },
        }
        self.retired_mops += inc_mops;
        let dec_rem_issue = inc_mops * machine.micro_op_factor();
        assert!(rem.rem_issue_count >= dec_rem_issue, "micro-ops double counted");
        rem.rem_issue_count -= dec_rem_issue;
        if self.zone_crit_res_idx != 0 {
            // Issue becomes the critical resource once the scaled micro-ops
            // pass the old one by a cycle.
            let scaled = i64::from(self.retired_mops * machine.micro_op_factor());
            if scaled - i64::from(self.resource_count(self.zone_crit_res_idx)) >= i64::from(machine.latency_factor()) {
                self.zone_crit_res_idx = 0;
            }
        }
        for entry in &unit.writes {
            let cycle =
                self.count_resource(machine, unit, entry.idx, entry.release_at_cycle, entry.acquire_at_cycle, rem);
            if cycle > next_cycle {
                next_cycle = cycle;
            }
        }
        if unit.has_reserved_resource {
            // Top-down: the cycle the node issues plus what it reserves;
            // bottom-up: the node's cycle.
            for entry in &unit.writes {
                if machine.resources[entry.idx].buffer_size == 0 {
                    let (until, instance) =
                        self.next_resource_cycle(machine, unit, entry.idx, entry.release_at_cycle, entry.acquire_at_cycle);
                    self.reserved_cycles[instance] =
                        if self.is_top { until.max(next_cycle + entry.release_at_cycle) } else { next_cycle };
                }
            }
        }
        let (top_latency, bot_latency) = if self.is_top {
            (&mut self.expected_latency, &mut self.dependent_latency)
        } else {
            (&mut self.dependent_latency, &mut self.expected_latency)
        };
        if unit.depth > *top_latency {
            *top_latency = unit.depth;
        }
        if unit.height > *bot_latency {
            *bot_latency = unit.height;
        }
        if next_cycle > self.curr_cycle {
            self.bump_cycle(machine, next_cycle);
        } else {
            self.is_resource_limited = check_resource_limit(
                machine.latency_factor(),
                self.critical_count(machine),
                self.scheduled_latency(),
                true,
            );
        }
        // After the bump, which resets the micro-ops of the cycle it leaves.
        self.curr_mops += inc_mops;
        if (self.is_top && unit.end_group) || (!self.is_top && unit.begin_group) {
            next_cycle += 1;
            self.bump_cycle(machine, next_cycle);
        }
        while self.curr_mops >= machine.issue_width {
            next_cycle += 1;
            self.bump_cycle(machine, next_cycle);
        }
    }

    /// `releasePending`: pending nodes that are ready become visible.
    pub fn release_pending(&mut self, machine: &Machine, units: &[Unit]) {
        // With nothing available the minimum may be reset.
        if self.available.is_empty() {
            self.min_ready_cycle = u32::MAX;
        }
        let mut i = 0;
        let mut end = self.pending.len();
        while i < end {
            let node = self.pending[i];
            let ready = self.ready_cycle(&units[node]);
            self.min_ready_cycle = self.min_ready_cycle.min(ready);
            if self.available.len() >= READY_LIST_LIMIT {
                break;
            }
            self.release_node(machine, units, node, ready, true, i);
            if end != self.pending.len() {
                end -= 1;
                // The next node moved into this place.
                continue;
            }
            i += 1;
        }
        self.check_pending = false;
    }

    /// `removeReady`.
    pub fn remove_ready(&mut self, node: usize) {
        if let Some(at) = self.available.iter().position(|&one| one == node) {
            self.available.remove(at);
        } else {
            let at = self.pending.iter().position(|&one| one == node).expect("bad ready count");
            self.pending.remove(at);
        }
    }

    /// `pickOnlyChoice`: release what is ready, defer what now hits a hazard,
    /// advance the clock until something is available; the node if it is the
    /// only one.
    pub fn pick_only_choice(&mut self, machine: &Machine, units: &[Unit]) -> Option<usize> {
        if self.check_pending {
            self.release_pending(machine, units);
        }
        let mut i = 0;
        while i < self.available.len() {
            let node = self.available[i];
            if self.check_hazard(machine, &units[node]) {
                self.pending.push(node);
                self.available.remove(i);
                continue;
            }
            i += 1;
        }
        while self.available.is_empty() {
            self.bump_cycle(machine, self.curr_cycle + 1);
            self.release_pending(machine, units);
        }
        (self.available.len() == 1).then(|| self.available[0])
    }

    /// `SchedBoundary::getOtherResourceCount`: the scaled count of the most
    /// used resource of the zone and what remains, and which resource it is
    /// (0 for issue).
    pub fn other_resource_count(&self, machine: &Machine, rem: &Remainder) -> (u32, usize) {
        let mut crit_count = rem.rem_issue_count + self.retired_mops * machine.micro_op_factor();
        let mut crit_idx = 0;
        for idx in 1..machine.resources.len() {
            let count = self.resource_count(idx) + rem.remaining_counts[idx];
            if count > crit_count {
                crit_count = count;
                crit_idx = idx;
            }
        }
        (crit_count, crit_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-wide in-order machine with one unbuffered ALU and one unbuffered
    /// two-unit load pipe: test data only, not a model of any processor.
    fn in_order() -> Machine {
        Machine::new(
            2,
            0,
            vec![
                ProcResource { num_units: 0, buffer_size: -1, sub_units: vec![] },
                ProcResource { num_units: 1, buffer_size: 0, sub_units: vec![] },
                ProcResource { num_units: 2, buffer_size: 0, sub_units: vec![] },
            ],
        )
    }

    fn alu() -> Unit {
        Unit {
            micro_ops: 1,
            writes: vec![WriteRes { idx: 1, release_at_cycle: 1, acquire_at_cycle: 0 }],
            is_unbuffered: true,
            has_reserved_resource: true,
            ..Unit::default()
        }
    }

    #[test]
    fn test_factors_normalise_units_and_micro_ops() {
        let machine = in_order();
        assert_eq!(machine.latency_factor(), 2);
        assert_eq!(machine.micro_op_factor(), 1);
        assert_eq!((machine.resource_factor(1), machine.resource_factor(2)), (2, 1));
        assert_eq!(machine.resource_factor(0), 0);
    }

    #[test]
    fn test_a_node_not_ready_on_an_in_order_machine_waits_pending_and_the_clock_moves_to_it() {
        let machine = in_order();
        let mut ready = alu();
        ready.bot_ready_cycle = 0;
        let mut late = alu();
        late.bot_ready_cycle = 5;
        let units = [ready, late];
        let mut zone = Boundary::new(&machine, false);
        zone.release_node(&machine, &units, 0, 0, false, 0);
        zone.release_node(&machine, &units, 1, 5, false, 0);
        assert_eq!((zone.available.clone(), zone.pending.clone()), (vec![0], vec![1]));
        // The only choice is the ready one.
        assert_eq!(zone.pick_only_choice(&machine, &units), Some(0));
        let mut rem = Remainder::new(&machine, &units);
        zone.bump_node(&machine, &units[0], &mut rem);
        zone.remove_ready(0);
        // Nothing available: the clock goes to the pending node's cycle.
        assert_eq!(zone.pick_only_choice(&machine, &units), Some(1));
        assert_eq!(zone.curr_cycle(), 5);
    }

    #[test]
    fn test_only_a_node_on_an_unbuffered_resource_has_stall_cycles() {
        let machine = in_order();
        let mut zone = Boundary::new(&machine, false);
        let mut one = alu();
        one.bot_ready_cycle = 3;
        assert_eq!(zone.latency_stall_cycles(&one), 3);
        one.is_unbuffered = false;
        assert_eq!(zone.latency_stall_cycles(&one), 0);
        zone.curr_cycle = 4;
        one.is_unbuffered = true;
        assert_eq!(zone.latency_stall_cycles(&one), 0);
    }

    #[test]
    fn test_an_unbuffered_resource_in_use_is_a_hazard_until_it_is_free() {
        let machine = in_order();
        let units = [alu(), alu()];
        let mut rem = Remainder::new(&machine, &units);
        let mut zone = Boundary::new(&machine, true);
        zone.min_ready_cycle = 0;
        zone.bump_node(&machine, &units[0], &mut rem);
        // One micro-op of a width-two machine issued; the ALU is reserved
        // for a cycle, so the second ALU node waits.
        assert!(zone.check_hazard(&machine, &units[1]));
        zone.bump_cycle(&machine, 1);
        assert!(!zone.check_hazard(&machine, &units[1]));
    }

    #[test]
    fn test_scheduled_resources_make_the_most_used_one_critical() {
        let machine = in_order();
        let units = [alu(), alu(), alu()];
        let mut rem = Remainder::new(&machine, &units);
        let mut zone = Boundary::new(&machine, true);
        zone.min_ready_cycle = 0;
        zone.bump_node(&machine, &units[0], &mut rem);
        assert_eq!(zone.zone_crit_res_idx(), 1);
        assert_eq!(zone.resource_count(1), 2);
        assert_eq!(rem.remaining_counts[1], 4);
    }

    #[test]
    fn test_the_issue_width_ends_the_cycle() {
        let machine = in_order();
        let mut a = alu();
        a.writes.clear();
        a.has_reserved_resource = false;
        a.is_unbuffered = false;
        let units = [a.clone(), a.clone(), a];
        let mut rem = Remainder::new(&machine, &units);
        let mut zone = Boundary::new(&machine, true);
        zone.min_ready_cycle = 0;
        zone.bump_node(&machine, &units[0], &mut rem);
        assert_eq!((zone.curr_cycle(), zone.curr_mops()), (0, 1));
        zone.bump_node(&machine, &units[1], &mut rem);
        assert_eq!((zone.curr_cycle(), zone.curr_mops()), (1, 0));
    }
}
