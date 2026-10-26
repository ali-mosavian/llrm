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

use llrm_lir::registers::{RegId, Regs};
use llrm_target::pressure::PressureSets;

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

    /// The model of a body whose values may be placed as `values` say: each
    /// (value, width in bytes, the registers it may take, as roots). A value
    /// counts the weight and sets of the smallest class holding its registers
    /// at that width (DEVIATE RP1); one no class holds, or with no
    /// registers to take, is not counted. Each set's limit is `limit(set)`.
    pub fn of_values<'v>(
        sets: &PressureSets,
        regs: Regs,
        limit: impl Fn(usize) -> u32,
        values: impl IntoIterator<Item = (u32, i64, &'v [RegId])>,
    ) -> Self {
        let mut model = Self::new((0..sets.sets.len()).map(limit).collect());
        let mut shared: HashMap<String, Rc<[usize]>> = HashMap::default();
        // Thousands of values share a few (width, registers) signatures: the
        // class of each is found once (the pin reads it off the class a value
        // was selected with).
        let mut found: HashMap<(i64, &[RegId]), Option<(u32, Rc<[usize]>)>> = HashMap::default();
        for (value, bytes, allowed) in values {
            let class = found.entry((bytes, allowed)).or_insert_with(|| {
                #[cfg(test)]
                LOOKUPS.with(|count| count.set(count.get() + 1));
                let names: Vec<&str> = allowed.iter().filter_map(|&root| regs.name(regs.named(root, bytes))).collect();
                if names.is_empty() || names.len() != allowed.len() {
                    return None;
                }
                let class = sets.class_holding(&names)?;
                let in_sets = shared.entry(class.name.clone()).or_insert_with(|| Rc::from(class.sets.clone())).clone();
                Some((class.weight, in_sets))
            });
            if let Some((weight, in_sets)) = class {
                model.count(value, *weight, in_sets.clone());
            }
        }
        model
    }

    fn weight(
        &self,
        value: u32,
    ) -> Option<(u32, &[usize])> {
        self.classes.get(&value).map(|(weight, sets)| (*weight, &sets[..]))
    }
}

/// Which candidate a comparison picked (`TryCand.Reason` set, `Cand.Reason`
/// lowered, or neither: the next criterion decides).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pick {
    Try,
    Cand,
    Neither,
}

/// `tryPressure` (MachineScheduler.cpp#L3805), bottom-up: compares what two
/// candidates do to one kind of pressure (`RPDelta.Excess`, `.CriticalMax` or
/// `.CurrentMax`, for `RegExcess`, `RegCritical` and `RegMax`). A change that
/// lowers pressure beats one that does not; for the same set the smaller
/// increase wins; for different sets the one in the set with the higher score
/// (the set's number, `getRegPressureSetScore`) is preferred to increase, the
/// lower to decrease; a candidate that changes nothing counts as the highest.
pub fn try_pressure(
    try_p: Option<PressureChange>,
    cand_p: Option<PressureChange>,
) -> Pick {
    let inc = |change: Option<PressureChange>| change.map_or(0, |change| change.inc);
    let (try_inc, cand_inc) = (inc(try_p), inc(cand_p));
    let greater = |try_val: i64, cand_val: i64| match try_val.cmp(&cand_val) {
        std::cmp::Ordering::Greater => Pick::Try,
        std::cmp::Ordering::Less => Pick::Cand,
        std::cmp::Ordering::Equal => Pick::Neither,
    };
    // If one candidate decreases and the other increases, go with it.
    let decides = greater((try_inc < 0).into(), (cand_inc < 0).into());
    if decides != Pick::Neither {
        return decides;
    }
    // Both affect the same set (or neither any): the smallest increase.
    let set = |change: Option<PressureChange>| change.map_or(usize::from(u16::MAX), |change| change.set);
    if set(try_p) == set(cand_p) {
        return greater(-i64::from(try_inc), -i64::from(cand_inc));
    }
    let rank = |change: Option<PressureChange>| change.map_or(i64::from(i32::MAX), |change| change.set as i64);
    let (mut try_rank, mut cand_rank) = (rank(try_p), rank(cand_p));
    // If the candidates are decreasing pressure, reverse priority.
    if try_inc < 0 {
        std::mem::swap(&mut try_rank, &mut cand_rank);
    }
    greater(try_rank, cand_rank)
}

#[cfg(test)]
thread_local! {
    /// How many times `Model::of_values` asked the pressure sets for a class.
    static LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
    /// A model of a body asked the pressure sets for the class of every value
    /// (name strings, a scan of the classes): 8.6 M instructions of d_faces'
    /// 25 M in the scheduler. Values of one width and register list share one
    /// lookup.
    #[test]
    fn values_of_one_signature_cost_one_class_lookup() {
        let sets = llrm_target::pressure::parse("set 0 GPR 4 BIG\nclass BIG 1 4 0 eax,ebx,ecx,edx\n").unwrap();
        let regs = Regs(&llrm_x86_m32::REGISTER_INFO);
        let roots: Vec<RegId> =
            [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX].iter().map(|&reg| regs.root(reg)).collect();
        LOOKUPS.with(|count| count.set(0));
        let model = Model::of_values(&sets, regs, |_| 4, (1..=1000).map(|value| (value, 4, &roots[..])));
        assert_eq!(LOOKUPS.with(std::cell::Cell::get), 1, "one lookup for 1000 values of one signature");
        assert!(model.weight(1000).is_some(), "the values are counted");
    }

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

    /// A word confined to ax and bx has the class that holds just them, a
    /// byte the same two roots a byte class, and a value no class holds is
    /// left out: pressure was read from one count for all of them.
    #[test]
    fn a_value_is_counted_in_the_smallest_class_holding_its_registers() {
        let sets = llrm_target::pressure::parse(
            "set 0 WORDS 4 ALL\nset 1 PAIR 2 PAIR\nclass ALL 2 8 0 ax,bx,cx,dx\nclass PAIR 2 2 0,1 ax,bx\nclass BYTES 1 4 0 al,bl,cl,dl\n",
        )
        .expect("parses");
        let regs = Regs(&llrm_x86_m16::REGISTER_INFO);
        let (ax, bx, cx) = (RegId::AX, RegId::BX, RegId::CX);
        let both: &[RegId] = &[ax, bx];
        let three: &[RegId] = &[ax, bx, cx];
        let model =
            Model::of_values(&sets, regs, |set| sets.sets[set].limit, [(1, 2, both), (2, 2, three), (3, 1, both)]);
        let tracker = Tracker::new(&model, [1, 2, 3], []);
        // value 1: PAIR (weight 2, sets 0 and 1); value 2: ALL (2, set 0);
        // value 3: BYTES (1, set 0).
        assert_eq!(tracker.current(), &[5, 2]);
    }

    fn change(
        set: usize,
        inc: i32,
    ) -> Option<PressureChange> {
        Some(PressureChange { set, inc })
    }

    /// `tryPressure`: a candidate that lowers pressure beats one that raises or
    /// leaves it; in one set the smaller increase wins; across sets the
    /// candidate raising the lower-numbered set loses to the higher, and one
    /// that changes nothing beats any raise. The order is what picks
    /// between equal pressures in the pin, so a flipped comparison reorders
    /// every tie.
    #[test]
    fn try_pressure_compares_as_the_pin_does() {
        assert_eq!(try_pressure(change(3, -1), change(3, 1)), Pick::Try);
        assert_eq!(try_pressure(change(3, 1), change(0, -2)), Pick::Cand);
        assert_eq!(try_pressure(change(2, 1), change(2, 3)), Pick::Try);
        assert_eq!(try_pressure(change(2, 3), change(2, 1)), Pick::Cand);
        assert_eq!(try_pressure(change(5, 1), change(2, 1)), Pick::Try);
        assert_eq!(try_pressure(change(2, 1), change(5, 1)), Pick::Cand);
        // Both lower, different sets: the priority reverses.
        assert_eq!(try_pressure(change(5, -1), change(2, -1)), Pick::Cand);
        assert_eq!(try_pressure(None, change(4, 1)), Pick::Try);
        assert_eq!(try_pressure(change(4, 1), None), Pick::Cand);
        assert_eq!(try_pressure(None, None), Pick::Neither);
        assert_eq!(try_pressure(change(1, 2), change(1, 2)), Pick::Neither);
    }

    #[test]
    fn the_critical_sets_are_those_over_their_limit() {
        let model = model();
        let critical = Tracker::critical_sets(&model, &[3, 3]);
        assert_eq!(critical, [PressureChange { set: 0, inc: 0 }]);
    }
}
