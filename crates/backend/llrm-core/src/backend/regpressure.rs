//! Register pressure per pressure set, for the pre-RA scheduler:
//! `RegPressureTracker` of the pin (RegisterPressure.cpp) as `GenericScheduler`
//! asks it of a bottom-up region.
//!
//! # How it works
//! Every value counts its class's weight in each pressure set the class is in
//! (`pressure.sets`, `llrm_target::pressure`). The tracker holds the pressure
//! of each set at the point reached, the most each has been, and what passes
//! through the region (`LiveThruPressure`, which raises the limit of the set it
//! is in). [`Tracker::advance_upward`] is `recede`: the instruction's defs stop
//! being live above it and its uses start. [`Tracker::upward_delta`] is
//! `getUpwardPressureDelta`: what placing an instruction next would do, as the
//! first set (in set order) that goes over its limit (`Excess`), over a
//! critical set's max so far (`CriticalMax`), or over the region's max pressure
//! (`CurrentMax`).
//!
//! # Example
//! One set of limit 2, `a` and `b` live: an instruction using a third value `c`
//! and defining nothing live takes the pressure to 3, one over: `Excess` is
//! (set 0, +1). An instruction that defines `a` and uses `c` leaves it at 2.
//!
//! # Not translated
//! DEVIATE RP1: the pin's value class is exact (instruction selection set it);
//! a value's class here is the one its allowed registers pick out. DEVIATE RP2:
//! the dead defs and lane masks the pin tracks (x86 values have one lane; a
//! dead def does not change `getUpwardPressureDelta`, only the max `recede`
//! keeps). The downward direction (`TopRPTracker`) waits for a top-down pass.

use std::rc::Rc;

use crate::model::lir::Insn;
use crate::support::hash::{HashMap, HashSet};

/// A change of one set's pressure (`PressureChange`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PressureChange {
    pub set: usize,
    pub inc: i32,
}

/// `RegPressureDelta`: the first set, in order, for each question; none where
/// no set answers it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Delta {
    pub excess: Option<PressureChange>,
    pub critical_max: Option<PressureChange>,
    pub current_max: Option<PressureChange>,
}

/// What the tracker needs of a target and a body: each set's limit, and the
/// weight and sets of each value that counts.
pub struct Model {
    pub limits: Vec<u32>,
    classes: HashMap<u32, (u32, Rc<[usize]>)>,
}

impl Model {
    pub fn new(limits: Vec<u32>) -> Self {
        Self { limits, classes: HashMap::default() }
    }

    /// `value` counts `weight` in each of `sets`.
    pub fn count(
        &mut self,
        value: u32,
        weight: u32,
        sets: Rc<[usize]>,
    ) {
        self.classes.insert(value, (weight, sets));
    }

    fn weight(
        &self,
        value: u32,
    ) -> Option<(u32, &[usize])> {
        self.classes.get(&value).map(|(weight, sets)| (*weight, &sets[..]))
    }
}

pub struct Tracker<'a> {
    model: &'a Model,
    live: HashSet<u32>,
    /// `CurrSetPressure`.
    current: Vec<u32>,
    /// `P.MaxSetPressure`: the most each set has been since the region's end.
    max: Vec<u32>,
    /// `LiveThruPressure`: raises each set's limit.
    live_thru: Vec<u32>,
}

impl<'a> Tracker<'a> {
    /// A tracker at the bottom of a region with `live_out` live below it
    /// (`closeBottom`); `live_thru` are the values live through the whole
    /// region (`initLiveThru`).
    pub fn new(
        model: &'a Model,
        live_out: impl IntoIterator<Item = u32>,
        live_thru: impl IntoIterator<Item = u32>,
    ) -> Self {
        let sets = model.limits.len();
        let mut tracker = Self {
            model,
            live: HashSet::default(),
            current: vec![0; sets],
            max: vec![0; sets],
            live_thru: vec![0; sets],
        };
        for value in live_out {
            tracker.make_live(value);
        }
        tracker.max.clone_from(&tracker.current);
        for value in live_thru {
            if let Some((weight, in_sets)) = model.weight(value) {
                for &set in in_sets {
                    tracker.live_thru[set] += weight;
                }
            }
        }
        tracker
    }

    pub fn current(&self) -> &[u32] {
        &self.current
    }

    pub fn max(&self) -> &[u32] {
        &self.max
    }

    fn make_live(
        &mut self,
        value: u32,
    ) {
        if let Some((weight, sets)) = self.model.weight(value) {
            if self.live.insert(value) {
                for &set in sets {
                    self.current[set] += weight;
                }
            }
        }
    }

    fn make_dead(
        &mut self,
        value: u32,
    ) {
        if let Some((weight, sets)) = self.model.weight(value) {
            if self.live.remove(&value) {
                for &set in sets {
                    self.current[set] -= weight;
                }
            }
        }
    }

    /// `recede`: `one` is placed above everything placed so far.
    pub fn advance_upward(
        &mut self,
        one: &Insn,
    ) {
        for &value in &one.defines {
            self.make_dead(value);
        }
        for &value in &one.uses {
            self.make_live(value);
        }
        for (max, &now) in self.max.iter_mut().zip(&self.current) {
            *max = (*max).max(now);
        }
    }

    /// `getUpwardPressureDelta`: the set of each kind that placing `one` next
    /// would raise first. `critical` is the region's critical sets with the
    /// most each has reached while scheduling (`RegionCriticalPSets`), in
    /// set order, and `max_limit` the region's max pressure before
    /// scheduling (`getRegPressure().MaxSetPressure`).
    pub fn upward_delta(
        &self,
        one: &Insn,
        critical: &[PressureChange],
        max_limit: &[u32],
    ) -> Delta {
        let mut change = vec![0i32; self.current.len()];
        let mut seen: Vec<u32> = Vec::new();
        for &value in &one.uses {
            if !self.live.contains(&value) && !seen.contains(&value) {
                seen.push(value);
                if let Some((weight, sets)) = self.model.weight(value) {
                    for &set in sets {
                        change[set] += weight as i32;
                    }
                }
            }
        }
        let mut defs: Vec<u32> = Vec::new();
        for &value in &one.defines {
            if self.live.contains(&value) && !defs.contains(&value) {
                defs.push(value);
                if let Some((weight, sets)) = self.model.weight(value) {
                    for &set in sets {
                        change[set] -= weight as i32;
                    }
                }
            }
        }
        let mut delta = Delta::default();
        for (set, &inc) in change.iter().enumerate() {
            if inc == 0 {
                continue;
            }
            let limit = self.model.limits[set] + self.live_thru[set];
            let (old, max_old) = (self.current[set], self.max[set]);
            let new = (old as i32 + inc) as u32;
            let max_new = max_old.max(new);
            if delta.excess.is_none() {
                // `ExcessInc` is unsigned in the pin and narrowed to the
                // change's signed field: the drop back to the
                // limit comes out negative.
                let over = if new > limit {
                    if old > limit { new as i32 - old as i32 } else { (new - limit) as i32 }
                } else if old > limit {
                    limit as i32 - old as i32
                } else {
                    0
                };
                if over != 0 {
                    delta.excess = Some(PressureChange { set, inc: over });
                }
            }
            if max_new == max_old {
                continue;
            }
            if delta.critical_max.is_none() {
                if let Some(entry) = critical.iter().find(|entry| entry.set == set) {
                    let over = max_new as i32 - entry.inc;
                    if over > 0 {
                        delta.critical_max = Some(PressureChange { set, inc: over });
                    }
                }
            }
            if delta.current_max.is_none() && max_new > max_limit[set] {
                delta.current_max = Some(PressureChange { set, inc: (max_new - max_old) as i32 });
            }
        }
        delta
    }

    /// `RegionCriticalPSets`: the sets whose region pressure `max_set_pressure`
    /// is over their limit, each with the pressure so far as its unit
    /// increment.
    pub fn critical_sets(
        model: &Model,
        max_set_pressure: &[u32],
    ) -> Vec<PressureChange> {
        max_set_pressure
            .iter()
            .enumerate()
            .filter(|&(set, &most)| most > model.limits[set])
            .map(|(set, _)| PressureChange { set, inc: 0 })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insn(
        defines: &[u32],
        uses: &[u32],
    ) -> Insn {
        Insn::new(0, None, None, defines.to_vec(), uses.to_vec())
    }

    /// Values 1..=3 weigh 1 in set 0 (limit 2); value 4 weighs 2 in sets 0 and
    /// 1 (limit 3).
    fn model() -> Model {
        let mut model = Model::new(vec![2, 3]);
        for value in 1..=3 {
            model.count(value, 1, Rc::from(vec![0]));
        }
        model.count(4, 2, Rc::from(vec![0, 1]));
        model
    }

    /// The module header's example: pressure 2 of limit 2, a use of a third
    /// value takes it one over; a def of a live value in the same instruction
    /// keeps it at the limit.
    #[test]
    fn an_instruction_that_raises_pressure_past_the_limit_has_excess() {
        let model = model();
        let tracker = Tracker::new(&model, [1, 2], []);
        let over = tracker.upward_delta(&insn(&[], &[3]), &[], &[2, 0]);
        assert_eq!(over.excess, Some(PressureChange { set: 0, inc: 1 }));
        let level = tracker.upward_delta(&insn(&[1], &[3]), &[], &[2, 0]);
        assert_eq!(level.excess, None);
    }

    /// A set already over its limit reports the drop an instruction would make,
    /// negative (`Limit - POld`), not zero.
    #[test]
    fn an_instruction_that_brings_pressure_under_the_limit_has_negative_excess() {
        let model = model();
        let tracker = Tracker::new(&model, [1, 2, 3], []);
        let under = tracker.upward_delta(&insn(&[1, 2], &[]), &[], &[3, 0]);
        // 3 -> 1 against a limit of 2: the pin reports Limit - POld, not the
        // whole drop.
        assert_eq!(under.excess.map(|change| change.inc), Some(-1));
    }

    /// Values that pass through the region raise every limit of the sets they
    /// are in (`LiveThruPressure`), so a region that only holds them is not in
    /// excess.
    #[test]
    fn live_through_values_raise_the_limit() {
        let model = model();
        let tracker = Tracker::new(&model, [1, 2], [1, 2]);
        assert_eq!(tracker.upward_delta(&insn(&[], &[3]), &[], &[2, 0]).excess, None);
    }

    /// A weight-2 value in two sets is counted in both, at its weight.
    #[test]
    fn a_value_counts_its_weight_in_every_set_of_its_class() {
        let model = model();
        let mut tracker = Tracker::new(&model, [], []);
        tracker.advance_upward(&insn(&[], &[4]));
        assert_eq!(tracker.current(), &[2, 2]);
        tracker.advance_upward(&insn(&[4], &[]));
        assert_eq!(tracker.current(), &[0, 0]);
        assert_eq!(tracker.max(), &[2, 2], "the most it has been stays");
    }

    /// `CriticalMax` is the pressure the new max passes over what the scheduled
    /// code has reached in a critical set; `CurrentMax` the new max over the
    /// region's own.
    #[test]
    fn the_max_questions_compare_with_the_critical_entry_and_the_region_max() {
        let model = model();
        let tracker = Tracker::new(&model, [1], []);
        let critical = [PressureChange { set: 0, inc: 1 }];
        let delta = tracker.upward_delta(&insn(&[], &[2]), &critical, &[1, 0]);
        assert_eq!(delta.critical_max, Some(PressureChange { set: 0, inc: 1 }));
        assert_eq!(delta.current_max, Some(PressureChange { set: 0, inc: 1 }));
        let quiet = tracker.upward_delta(&insn(&[], &[2]), &critical, &[2, 0]);
        assert_eq!(quiet.current_max, None);
    }

    #[test]
    fn the_critical_sets_are_those_over_their_limit() {
        let model = model();
        let critical = Tracker::critical_sets(&model, &[3, 3]);
        assert_eq!(critical, [PressureChange { set: 0, inc: 0 }]);
    }
}
