//! Greedy interval selection before spill materialization.
//!
//! This is the selection half of Python `backend.allocate.allocate`, not its
//! multi-round spill/rewrite coordinator.  It consumes the already weighted
//! Machine IR intervals, assigns whole virtual ranges, and returns every
//! selected spill together.  Splitting and rewriting remain caller work.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::error::Error;
use std::fmt;

use super::{
    LiveInterval, PhysicalRegister, RegisterAssignment, RegisterClass, VirtualRegisterId, PER_INSN,
};

/// Maximum allocation-queue visits, matching Python `backend.allocate.BUDGET`.
pub const BUDGET: usize = 200_000;
/// A reload may not be selected as a spill victim while it spans this few slots.
pub const RELOAD: u64 = 4 * PER_INSN;
const RETRY_PRIORITY: u64 = 1_000_000;

/// The outcome of one greedy selection round.
#[derive(Clone, Debug, PartialEq)]
pub enum GreedyAllocation {
    /// Every named interval received a physical register.
    Complete(RegisterAssignment),
    /// Some intervals need spill materialization before a later allocation round.
    Spills {
        /// The complete assignment for intervals retained in registers.
        assignment: RegisterAssignment,
        /// Every interval selected for this round's spill batch.
        spills: BTreeSet<VirtualRegisterId>,
        /// Sum of the selected intervals' weighted spill prices.
        cost: f64,
    },
}

/// A fact that makes greedy selection impossible or incomplete by contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GreedyAllocationError {
    /// An interval did not retain the declared register class needed for order lookup.
    MissingClass { register: VirtualRegisterId },
    /// Two overlapping hard requirements cannot both be met.
    FixedUnplaceable {
        register: VirtualRegisterId,
        physical: PhysicalRegister,
    },
    /// A protected or short unspillable range has no legal physical register.
    Unspillable { register: VirtualRegisterId },
    /// The deterministic visit guard fired before selection settled.
    VisitBudgetExceeded { visits: usize },
}

impl fmt::Display for GreedyAllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingClass { register } => {
                write!(
                    formatter,
                    "virtual register {register} has no declared class"
                )
            }
            Self::FixedUnplaceable { register, physical } => write!(
                formatter,
                "virtual register {register} cannot be placed in fixed physical register {physical}"
            ),
            Self::Unspillable { register } => write!(
                formatter,
                "virtual register {register} cannot be spilled and no register is free"
            ),
            Self::VisitBudgetExceeded { visits } => write!(
                formatter,
                "greedy allocation exceeded its {visits}-visit budget"
            ),
        }
    }
}

impl Error for GreedyAllocationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Assign,
    Split,
    Done,
}

/// A priority-queue entry.  `BinaryHeap` is a max heap, so the reverse ID
/// comparison reproduces Python's ascending virtual-ID final tie-break.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Queued {
    fixed: bool,
    priority: u64,
    register: VirtualRegisterId,
}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fixed
            .cmp(&other.fixed)
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| other.register.cmp(&self.register))
    }
}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Selects a complete assignment or one complete, costed spill batch.
///
/// `intervals` must be the result of [`super::weighted_live_intervals`].
/// Only its named entries participate; undeclared or dead virtual registers
/// are deliberately outside this inner selection policy.  `fixed`,
/// `unspillable`, and `protected` are already-materialized allocation facts.
/// The two callbacks are the sole target input: class allocation order and
/// physical-register overlap.  Register masks, copies, folds, rematerialized
/// siblings, address roles, splitting, rewriting, and retry are deferred.
pub fn allocate_greedy<AllocationOrder, Overlaps>(
    intervals: &BTreeMap<VirtualRegisterId, LiveInterval>,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    fixed: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    unspillable: &BTreeSet<VirtualRegisterId>,
    protected: &BTreeSet<VirtualRegisterId>,
    allocation_order: AllocationOrder,
    overlaps: Overlaps,
) -> Result<GreedyAllocation, GreedyAllocationError>
where
    AllocationOrder: Fn(RegisterClass) -> Vec<PhysicalRegister>,
    Overlaps: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    let mut live = intervals.clone();
    for register in unspillable {
        if let Some(interval) = live.get_mut(register) {
            if interval.size() <= RELOAD {
                interval.weight = f64::INFINITY;
            }
        }
    }
    for register in protected {
        if let Some(interval) = live.get_mut(register) {
            interval.weight = f64::INFINITY;
        }
    }

    let mut assignments = BTreeMap::<VirtualRegisterId, PhysicalRegister>::new();
    let mut stage = BTreeMap::<VirtualRegisterId, Stage>::new();
    let mut spills = BTreeSet::<VirtualRegisterId>::new();
    let mut cascades = BTreeMap::<VirtualRegisterId, u64>::new();
    let mut queue = BinaryHeap::new();
    for &register in live.keys() {
        queue.push(queued(register, fixed, &live, Stage::Assign));
    }

    let mut cost = 0.0;
    let mut visits = 0;
    let mut newest_cascade = 1;
    while let Some(entry) = queue.pop() {
        if visits == BUDGET {
            return Err(GreedyAllocationError::VisitBudgetExceeded { visits });
        }
        visits += 1;
        let register = entry.register;
        if assignments.contains_key(&register) || spills.contains(&register) {
            continue;
        }
        let at = stage.get(&register).copied().unwrap_or(Stage::Assign);
        let Some(interval) = live.get(&register) else {
            continue;
        };
        let order = order_for(register, classes, fixed, &allocation_order)?;

        if let Some(physical) =
            free_register(register, interval, &order, &assignments, &live, &overlaps)
        {
            assignments.insert(register, physical);
            stage.insert(register, Stage::Done);
            continue;
        }

        if at == Stage::Assign {
            if let Some((physical, victims)) = evictable_register(
                register,
                interval,
                &order,
                &assignments,
                &live,
                fixed,
                protected,
                &cascades,
                newest_cascade,
                &allocation_order,
                classes,
                &overlaps,
            )? {
                let cascade = *cascades.entry(register).or_insert_with(|| {
                    let current = newest_cascade;
                    newest_cascade += 1;
                    current
                });
                for victim in victims {
                    assignments.remove(&victim);
                    cascades.insert(victim, cascade);
                    stage.insert(victim, Stage::Assign);
                    queue.push(queued(victim, fixed, &live, Stage::Assign));
                }
                assignments.insert(register, physical);
                stage.insert(register, Stage::Done);
                continue;
            }
            stage.insert(register, Stage::Split);
            queue.push(queued(register, fixed, &live, Stage::Split));
            continue;
        }

        if let Some(&physical) = fixed.get(&register) {
            return Err(GreedyAllocationError::FixedUnplaceable { register, physical });
        }
        if interval.weight.is_infinite() {
            return Err(GreedyAllocationError::Unspillable { register });
        }
        spills.insert(register);
        cost += interval.weight;
        stage.insert(register, Stage::Done);
    }

    let assignment = RegisterAssignment::from_assignments(assignments);
    if spills.is_empty() {
        Ok(GreedyAllocation::Complete(assignment))
    } else {
        Ok(GreedyAllocation::Spills {
            assignment,
            spills,
            cost,
        })
    }
}

fn queued(
    register: VirtualRegisterId,
    fixed: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    live: &BTreeMap<VirtualRegisterId, LiveInterval>,
    stage: Stage,
) -> Queued {
    let size = live.get(&register).map_or(0, LiveInterval::size);
    Queued {
        fixed: fixed.contains_key(&register),
        priority: size.saturating_add(u64::from(stage != Stage::Assign) * RETRY_PRIORITY),
        register,
    }
}

fn class_for(
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    register: VirtualRegisterId,
) -> Result<RegisterClass, GreedyAllocationError> {
    classes
        .get(&register)
        .copied()
        .ok_or(GreedyAllocationError::MissingClass { register })
}

fn order_for<AllocationOrder>(
    register: VirtualRegisterId,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    fixed: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    allocation_order: &AllocationOrder,
) -> Result<Vec<PhysicalRegister>, GreedyAllocationError>
where
    AllocationOrder: Fn(RegisterClass) -> Vec<PhysicalRegister>,
{
    if let Some(&physical) = fixed.get(&register) {
        return Ok(vec![physical]);
    }
    Ok(allocation_order(class_for(classes, register)?))
}

fn free_register<Overlaps>(
    register: VirtualRegisterId,
    interval: &LiveInterval,
    order: &[PhysicalRegister],
    assignments: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    live: &BTreeMap<VirtualRegisterId, LiveInterval>,
    overlaps: &Overlaps,
) -> Option<PhysicalRegister>
where
    Overlaps: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    order.iter().copied().find(|candidate| {
        assignments.iter().all(|(&other, &assigned)| {
            other == register
                || !physical_conflicts(*candidate, assigned, overlaps)
                || !live
                    .get(&other)
                    .is_some_and(|other_interval| interval.overlaps(other_interval))
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn evictable_register<AllocationOrder, Overlaps>(
    register: VirtualRegisterId,
    interval: &LiveInterval,
    order: &[PhysicalRegister],
    assignments: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    live: &BTreeMap<VirtualRegisterId, LiveInterval>,
    fixed: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    protected: &BTreeSet<VirtualRegisterId>,
    cascades: &BTreeMap<VirtualRegisterId, u64>,
    newest_cascade: u64,
    allocation_order: &AllocationOrder,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    overlaps: &Overlaps,
) -> Result<Option<(PhysicalRegister, Vec<VirtualRegisterId>)>, GreedyAllocationError>
where
    AllocationOrder: Fn(RegisterClass) -> Vec<PhysicalRegister>,
    Overlaps: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    let cascade = cascades.get(&register).copied().unwrap_or(newest_cascade);
    let mut best: Option<(f64, PhysicalRegister, Vec<VirtualRegisterId>)> = None;
    for &candidate in order {
        let victims = assignments
            .iter()
            .filter_map(|(&other, &assigned)| {
                (physical_conflicts(candidate, assigned, overlaps)
                    && live
                        .get(&other)
                        .is_some_and(|other_interval| interval.overlaps(other_interval)))
                .then_some(other)
            })
            .collect::<Vec<_>>();
        if victims.is_empty()
            || victims
                .iter()
                .any(|victim| fixed.contains_key(victim) || protected.contains(victim))
            || victims
                .iter()
                .any(|victim| cascades.get(victim).copied().unwrap_or(0) >= cascade)
        {
            continue;
        }
        let mut bill = 0.0;
        for &victim in &victims {
            let victim_interval = &live[&victim];
            if !movable(
                victim,
                candidate,
                assignments,
                live,
                fixed,
                allocation_order,
                classes,
                overlaps,
            )? {
                bill += victim_interval.weight;
            }
        }
        if bill >= interval.weight {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(best_bill, _, _)| bill < *best_bill)
        {
            best = Some((bill, candidate, victims));
        }
    }
    Ok(best.map(|(_, physical, victims)| (physical, victims)))
}

#[allow(clippy::too_many_arguments)]
fn movable<AllocationOrder, Overlaps>(
    register: VirtualRegisterId,
    occupied: PhysicalRegister,
    assignments: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    live: &BTreeMap<VirtualRegisterId, LiveInterval>,
    fixed: &BTreeMap<VirtualRegisterId, PhysicalRegister>,
    allocation_order: &AllocationOrder,
    classes: &BTreeMap<VirtualRegisterId, RegisterClass>,
    overlaps: &Overlaps,
) -> Result<bool, GreedyAllocationError>
where
    AllocationOrder: Fn(RegisterClass) -> Vec<PhysicalRegister>,
    Overlaps: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    if fixed.contains_key(&register) {
        return Ok(false);
    }
    let order = allocation_order(class_for(classes, register)?);
    let elsewhere = order
        .into_iter()
        .filter(|candidate| !physical_conflicts(*candidate, occupied, overlaps))
        .collect::<Vec<_>>();
    Ok(free_register(
        register,
        &live[&register],
        &elsewhere,
        assignments,
        live,
        overlaps,
    )
    .is_some())
}

fn physical_conflicts<Overlaps>(
    left: PhysicalRegister,
    right: PhysicalRegister,
    overlaps: &Overlaps,
) -> bool
where
    Overlaps: Fn(PhysicalRegister, PhysicalRegister) -> bool,
{
    left == right || overlaps(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{LiveSegment, RegisterClass};

    const GENERAL: RegisterClass = RegisterClass::new(0);
    const FIRST: PhysicalRegister = PhysicalRegister::new(0);
    const SECOND: PhysicalRegister = PhysicalRegister::new(1);
    const THIRD: PhysicalRegister = PhysicalRegister::new(2);

    fn ranges(items: &[(u32, u64, u64, f64)]) -> BTreeMap<VirtualRegisterId, LiveInterval> {
        items
            .iter()
            .map(|&(register, start, end, weight)| {
                let register = VirtualRegisterId::new(register);
                (
                    register,
                    LiveInterval {
                        register,
                        segments: vec![LiveSegment { start, end }],
                        weight,
                    },
                )
            })
            .collect()
    }

    fn classes(registers: &[u32]) -> BTreeMap<VirtualRegisterId, RegisterClass> {
        registers
            .iter()
            .map(|&register| (VirtualRegisterId::new(register), GENERAL))
            .collect()
    }

    fn no_alias(_: PhysicalRegister, _: PhysicalRegister) -> bool {
        false
    }

    #[test]
    fn conflicting_hard_register_assignments_are_unplaceable_not_spills() {
        // Python tests/test_allocation.py::test_conflicting_hard_register_assignments_are_unplaceable_not_spills.
        let intervals = ranges(&[(1, 0, 8, 1.0), (2, 0, 8, 1.0)]);
        let fixed = BTreeMap::from([
            (VirtualRegisterId::new(1), FIRST),
            (VirtualRegisterId::new(2), FIRST),
        ]);

        assert_eq!(
            allocate_greedy(
                &intervals,
                &classes(&[1, 2]),
                &fixed,
                &BTreeSet::new(),
                &BTreeSet::new(),
                |_| vec![FIRST],
                no_alias,
            ),
            Err(GreedyAllocationError::FixedUnplaceable {
                register: VirtualRegisterId::new(2),
                physical: FIRST,
            })
        );
    }

    #[test]
    fn a_live_fixed_range_uses_its_pin_outside_the_target_order() {
        // Python `allocate()` makes a fixed range's order exactly its pin.
        let intervals = ranges(&[(1, 0, 8, 1.0)]);
        let fixed = BTreeMap::from([(VirtualRegisterId::new(1), FIRST)]);
        let result = allocate_greedy(
            &intervals,
            &classes(&[1]),
            &fixed,
            &BTreeSet::new(),
            &BTreeSet::new(),
            |_| vec![SECOND],
            no_alias,
        )
        .expect("a fixed range does not need to be in the flexible target order");

        match result {
            GreedyAllocation::Complete(assignment) => {
                assert_eq!(assignment.get(VirtualRegisterId::new(1)), Some(FIRST));
            }
            GreedyAllocation::Spills { .. } => panic!("the fixed range has its pinned register"),
        }
    }

    #[test]
    fn a_hard_register_assignment_is_not_an_eviction_victim() {
        // Python tests/test_allocation.py::test_a_hard_register_assignment_is_not_an_eviction_victim.
        let intervals = ranges(&[(1, 0, 10, 0.1), (2, 1, 9, 10.0)]);
        let fixed = BTreeMap::from([(VirtualRegisterId::new(1), FIRST)]);

        let result = allocate_greedy(
            &intervals,
            &classes(&[1, 2]),
            &fixed,
            &BTreeSet::new(),
            &BTreeSet::new(),
            |_| vec![FIRST],
            no_alias,
        )
        .expect("the flexible interval, not the hard one, is selected for spilling");
        assert!(matches!(
            result,
            GreedyAllocation::Spills { spills, cost, .. }
                if spills == BTreeSet::from([VirtualRegisterId::new(2)]) && cost == 10.0
        ));
    }

    #[test]
    fn an_unspillable_range_without_a_register_is_unplaceable() {
        // Python tests/test_allocation.py::test_an_unspillable_range_without_a_register_is_unplaceable.
        let intervals = ranges(&[(1, 0, 8, 1.0), (2, 0, 8, 1.0)]);
        let fixed = BTreeMap::from([(VirtualRegisterId::new(1), FIRST)]);
        let unspillable = BTreeSet::from([VirtualRegisterId::new(2)]);

        assert_eq!(
            allocate_greedy(
                &intervals,
                &classes(&[1, 2]),
                &fixed,
                &unspillable,
                &BTreeSet::new(),
                |_| vec![FIRST],
                no_alias,
            ),
            Err(GreedyAllocationError::Unspillable {
                register: VirtualRegisterId::new(2),
            })
        );
    }

    #[test]
    fn larger_ranges_place_first_then_costed_eviction_returns_one_complete_batch() {
        let intervals = ranges(&[(1, 0, 10, 1.0), (2, 1, 9, 10.0)]);
        let result = allocate_greedy(
            &intervals,
            &classes(&[1, 2]),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            |_| vec![FIRST],
            no_alias,
        )
        .expect("the hotter short interval evicts the larger cheap interval");

        match result {
            GreedyAllocation::Spills {
                assignment,
                spills,
                cost,
            } => {
                assert_eq!(assignment.get(VirtualRegisterId::new(2)), Some(FIRST));
                assert_eq!(spills, BTreeSet::from([VirtualRegisterId::new(1)]));
                assert_eq!(cost, 1.0);
            }
            GreedyAllocation::Complete(_) => {
                panic!("the cheap victim must be returned for spilling")
            }
        }
    }

    #[test]
    fn alias_overlaps_block_two_live_ranges_from_register_views_in_one_family() {
        let intervals = ranges(&[(1, 0, 8, 1.0), (2, 0, 8, 1.0)]);
        let result = allocate_greedy(
            &intervals,
            &classes(&[1, 2]),
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            |_| vec![FIRST, SECOND, THIRD],
            |left, right| (left == FIRST && right == SECOND) || (left == SECOND && right == FIRST),
        )
        .expect("the third non-aliasing register is legal");

        match result {
            GreedyAllocation::Complete(assignment) => {
                assert_eq!(assignment.get(VirtualRegisterId::new(1)), Some(FIRST));
                assert_eq!(assignment.get(VirtualRegisterId::new(2)), Some(THIRD));
            }
            GreedyAllocation::Spills { .. } => panic!("a non-aliasing fallback exists"),
        }
    }
}
