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
use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};
use llrm_target::pressure::PressureSets;

use crate::model::lir::Insn;
use crate::support::hash::HashMap;

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
        #[cfg(test)]
        WEIGHTS.with(|count| count.set(count.get() + 1));
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

#[cfg(test)]
thread_local! {
    /// How many times a value's weight was asked of the model.
    static WEIGHTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A read whose def is not in the region (`VNInfo` of the live-in value).
pub const LIVE_IN: u32 = u32::MAX;

/// A region's operands with the def each read comes from. The pin tells the
/// values of a register apart by live interval (`LiveQueryResult::valueIn`), so
/// a tied `c := add c, b` reads the value the copy before it made and writes
/// another; a name is not a value after two-address.
pub struct Region {
    /// Per node, the distinct values it reads with the node that wrote what it
    /// reads (`LIVE_IN` for none in the region).
    reads: Vec<Vec<(u32, u32)>>,
    /// Per node, the distinct values it writes; the version is the node.
    writes: Vec<Vec<u32>>,
    /// The value each name holds at the region's end.
    last_def: HashMap<u32, u32>,
}

impl Region {
    pub fn of(insns: &[Arc<Insn>]) -> Self {
        let mut current: HashMap<u32, u32> = HashMap::default();
        let (mut reads, mut writes) = (Vec::with_capacity(insns.len()), Vec::with_capacity(insns.len()));
        for (node, one) in insns.iter().enumerate() {
            let mut read: Vec<u32> = one.uses.clone();
            read.sort_unstable();
            read.dedup();
            reads
                .push(read.into_iter().map(|value| (value, current.get(&value).copied().unwrap_or(LIVE_IN))).collect());
            let mut write: Vec<u32> = one.defines.clone();
            write.sort_unstable();
            write.dedup();
            for &value in &write {
                current.insert(value, node as u32);
            }
            writes.push(write);
        }
        Self { reads, writes, last_def: current }
    }
}

pub struct Tracker<'a> {
    model: &'a Model,
    region: &'a Region,
    /// The value (the node that wrote it) each live name holds.
    live: HashMap<u32, u32>,
    /// `CurrSetPressure`.
    current: Vec<u32>,
    /// `P.MaxSetPressure`: the most each set has been since the region's end.
    max: Vec<u32>,
    /// `LiveThruPressure`: raises each set's limit.
    live_thru: Vec<u32>,
    /// `PressureDiff`s: per node of the region, in `sets` entries, what placing
    /// it next does to each set. Empty when not kept.
    diffs: Vec<i32>,
    /// The nodes that read each value (name and writer).
    readers: HashMap<(u32, u32), Vec<u32>>,
}

impl<'a> Tracker<'a> {
    /// A tracker at the bottom of a region with `live_out` live below it
    /// (`closeBottom`); `live_thru` are the values live through the whole
    /// region (`initLiveThru`). With `diffs`, the `PressureDiff` of each node
    /// is computed once (`buildDAGWithRegPressure`); [`Self::place`] keeps
    /// them current and [`Self::delta`] reads one.
    pub fn over(
        model: &'a Model,
        region: &'a Region,
        live_out: impl IntoIterator<Item = u32>,
        live_thru: impl IntoIterator<Item = u32>,
        diffs: bool,
    ) -> Self {
        let sets = model.limits.len();
        let mut tracker = Self {
            model,
            region,
            live: HashMap::default(),
            current: vec![0; sets],
            max: vec![0; sets],
            live_thru: vec![0; sets],
            diffs: Vec::new(),
            readers: HashMap::default(),
        };
        for value in live_out {
            tracker.make_live(value, region.last_def.get(&value).copied().unwrap_or(LIVE_IN));
        }
        tracker.max.clone_from(&tracker.current);
        for value in live_thru {
            if let Some((weight, in_sets)) = model.weight(value) {
                for &set in in_sets {
                    tracker.live_thru[set] += weight;
                }
            }
        }
        if diffs {
            tracker.diffs = vec![0; region.reads.len() * sets];
            for node in 0..region.reads.len() {
                for (value, version) in region.reads[node].iter().copied() {
                    let Some((weight, in_sets)) = model.weight(value) else { continue };
                    tracker.readers.entry((value, version)).or_default().push(node as u32);
                    if tracker.live.get(&value) != Some(&version) {
                        for &set in in_sets {
                            tracker.diffs[node * sets + set] += weight as i32;
                        }
                    }
                }
                for &value in &region.writes[node] {
                    let Some((weight, in_sets)) = model.weight(value) else { continue };
                    if tracker.live.get(&value) == Some(&(node as u32)) {
                        for &set in in_sets {
                            tracker.diffs[node * sets + set] -= weight as i32;
                        }
                    }
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
        version: u32,
    ) -> bool {
        let Some((weight, sets)) = self.model.weight(value) else { return false };
        if self.live.contains_key(&value) {
            return false;
        }
        self.live.insert(value, version);
        for &set in sets {
            self.current[set] += weight;
        }
        true
    }

    /// `recede` and `updatePressureDiffs`: `node` is placed above everything
    /// placed so far. The values it writes stop being live above it, those it
    /// reads start; a value that just became live stops raising the diff of
    /// every other reader of the same value.
    pub fn place(
        &mut self,
        node: usize,
    ) {
        let region = self.region;
        for &value in &region.writes[node] {
            if let Some((weight, sets)) = self.model.weight(value) {
                if self.live.get(&value) == Some(&(node as u32)) {
                    self.live.remove(&value);
                    for &set in sets {
                        self.current[set] -= weight;
                    }
                }
            }
        }
        let sets = self.current.len();
        for &(value, version) in &region.reads[node] {
            if self.make_live(value, version) && !self.diffs.is_empty() {
                let (weight, in_sets) = self.model.weight(value).expect("made live");
                // Its readers no longer raise pressure, and its writer now ends
                // a live value.
                let writer = (version != LIVE_IN).then_some(&version);
                for &other in self.readers.get(&(value, version)).into_iter().flatten().chain(writer) {
                    for &set in in_sets {
                        self.diffs[other as usize * sets + set] -= weight as i32;
                    }
                }
            }
        }
        for (max, &now) in self.max.iter_mut().zip(&self.current) {
            *max = (*max).max(now);
        }
    }

    /// `getUpwardPressureDelta` for a node of the region from its
    /// `PressureDiff`: the work is the number of sets, whatever the node reads.
    pub fn delta(
        &self,
        node: usize,
        critical: &[PressureChange],
        max_limit: &[u32],
    ) -> Delta {
        let sets = self.current.len();
        self.delta_of(&self.diffs[node * sets..(node + 1) * sets], critical, max_limit)
    }

    /// The same by looking at what `node` reads and writes against what is
    /// live now (`getMaxUpwardPressureDelta`): the check on [`Self::delta`].
    #[cfg(test)]
    fn rescan(
        &self,
        node: usize,
        critical: &[PressureChange],
        max_limit: &[u32],
    ) -> Delta {
        let mut change = vec![0i32; self.current.len()];
        for &(value, version) in &self.region.reads[node] {
            if self.live.get(&value) != Some(&version) {
                if let Some((weight, sets)) = self.model.weight(value) {
                    for &set in sets {
                        change[set] += weight as i32;
                    }
                }
            }
        }
        for &value in &self.region.writes[node] {
            if self.live.get(&value) == Some(&(node as u32)) {
                if let Some((weight, sets)) = self.model.weight(value) {
                    for &set in sets {
                        change[set] -= weight as i32;
                    }
                }
            }
        }
        self.delta_of(&change, critical, max_limit)
    }

    fn delta_of(
        &self,
        change: &[i32],
        critical: &[PressureChange],
        max_limit: &[u32],
    ) -> Delta {
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
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()))
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

    /// The module header's example: pressure 2 of limit 2, a use of a third
    /// value takes it one over; a def of a live value in the same instruction
    /// keeps it at the limit.
    #[test]
    fn an_instruction_that_raises_pressure_past_the_limit_has_excess() {
        let model = model();
        let insns = [insn(&[], &[3]), insn(&[1], &[3])];
        let region = Region::of(&insns);
        let tracker = Tracker::over(&model, &region, [1, 2], [], true);
        assert_eq!(tracker.delta(0, &[], &[2, 0]).excess, Some(PressureChange { set: 0, inc: 1 }));
        assert_eq!(tracker.delta(1, &[], &[2, 0]).excess, None);
    }

    /// A set already over its limit reports the drop an instruction would make,
    /// negative (`Limit - POld`), not zero.
    #[test]
    fn an_instruction_that_brings_pressure_under_the_limit_has_negative_excess() {
        let model = model();
        let insns = [insn(&[1, 2], &[])];
        let region = Region::of(&insns);
        let tracker = Tracker::over(&model, &region, [1, 2, 3], [], true);
        // 3 -> 1 against a limit of 2: the pin reports Limit - POld, not the
        // whole drop.
        assert_eq!(tracker.delta(0, &[], &[3, 0]).excess.map(|change| change.inc), Some(-1));
    }

    /// Values that pass through the region raise every limit of the sets they
    /// are in (`LiveThruPressure`), so a region that only holds them is not in
    /// excess.
    #[test]
    fn live_through_values_raise_the_limit() {
        let model = model();
        let insns = [insn(&[], &[3])];
        let region = Region::of(&insns);
        let tracker = Tracker::over(&model, &region, [1, 2], [1, 2], true);
        assert_eq!(tracker.delta(0, &[], &[2, 0]).excess, None);
    }

    /// A weight-2 value in two sets is counted in both, at its weight.
    #[test]
    fn a_value_counts_its_weight_in_every_set_of_its_class() {
        let model = model();
        let insns = [insn(&[4], &[]), insn(&[], &[4])];
        let region = Region::of(&insns);
        let mut tracker = Tracker::over(&model, &region, [], [], false);
        tracker.place(1);
        assert_eq!(tracker.current(), &[2, 2]);
        tracker.place(0);
        assert_eq!(tracker.current(), &[0, 0]);
        assert_eq!(tracker.max(), &[2, 2], "the most it has been stays");
    }

    /// `CriticalMax` is the pressure the new max passes over what the scheduled
    /// code has reached in a critical set; `CurrentMax` the new max over the
    /// region's own.
    #[test]
    fn the_max_questions_compare_with_the_critical_entry_and_the_region_max() {
        let model = model();
        let insns = [insn(&[], &[2])];
        let region = Region::of(&insns);
        let tracker = Tracker::over(&model, &region, [1], [], true);
        let critical = [PressureChange { set: 0, inc: 1 }];
        let delta = tracker.delta(0, &critical, &[1, 0]);
        assert_eq!(delta.critical_max, Some(PressureChange { set: 0, inc: 1 }));
        assert_eq!(delta.current_max, Some(PressureChange { set: 0, inc: 1 }));
        assert_eq!(tracker.delta(0, &critical, &[2, 0]).current_max, None);
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
        let region = Region::of(&[]);
        let tracker = Tracker::over(&model, &region, [1, 2, 3], [], false);
        // value 1: PAIR (weight 2, sets 0 and 1); value 2: ALL (2, set 0);
        // value 3: BYTES (1, set 0).
        assert_eq!(tracker.current(), &[5, 2]);
    }

    /// Each distinct value of a region's nodes has an operand set; `n` nodes of
    /// `seed`-chosen reads and writes over values 1..=12, a name often both
    /// read and written (a tied def).
    fn region(
        seed: u64,
        n: usize,
    ) -> Vec<Arc<Insn>> {
        let mut state = seed;
        let mut next = move |bound: u64| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 33) % bound
        };
        (0..n)
            .map(|_| {
                let defines: Vec<u32> = (0..next(3)).map(|_| 1 + next(12) as u32).collect();
                let uses: Vec<u32> = (0..next(4)).map(|_| 1 + next(12) as u32).collect();
                insn(&defines, &uses)
            })
            .collect()
    }

    fn wide_model() -> Model {
        let mut model = Model::new(vec![2, 3]);
        for value in 1..=12 {
            if value % 4 == 0 {
                model.count(value, 2, Rc::from(vec![0, 1]));
            } else {
                model.count(value, 1, Rc::from(vec![0]));
            }
        }
        model
    }

    /// The `PressureDiff`s kept current by `place` must give the delta that
    /// looking at the node against the live values gives, at every step of any
    /// order the graph allows (a node waits for every later node it conflicts
    /// with): an update that missed a value turning live would pick by a stale
    /// count.
    #[test]
    fn the_kept_diffs_give_the_delta_a_rescan_gives_after_each_placement() {
        let model = wide_model();
        for seed in 1..=20u64 {
            let insns = region(seed, 30);
            let region = Region::of(&insns);
            let n = insns.len();
            let touches = |node: usize| -> (Vec<u32>, Vec<u32>) {
                (region.reads[node].iter().map(|read| read.0).collect(), region.writes[node].clone())
            };
            // `later[i]`: the nodes after i that must be placed before it.
            let later: Vec<Vec<usize>> = (0..n)
                .map(|i| {
                    let (reads, writes) = touches(i);
                    (i + 1..n)
                        .filter(|&j| {
                            let (other_reads, other_writes) = touches(j);
                            writes.iter().any(|v| other_reads.contains(v) || other_writes.contains(v))
                                || reads.iter().any(|v| other_writes.contains(v))
                        })
                        .collect()
                })
                .collect();
            let mut kept = Tracker::over(&model, &region, [1, 5, 9], [2], true);
            let critical = [PressureChange { set: 0, inc: 1 }];
            let mut placed = vec![false; n];
            let mut state = seed;
            for _ in 0..n {
                let ready: Vec<usize> = (0..n).filter(|&i| !placed[i] && later[i].iter().all(|&j| placed[j])).collect();
                for &other in &ready {
                    assert_eq!(
                        kept.delta(other, &critical, &[1, 1]),
                        kept.rescan(other, &critical, &[1, 1]),
                        "seed {seed}, node {other}"
                    );
                }
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let node = ready[(state >> 33) as usize % ready.len()];
                placed[node] = true;
                kept.place(node);
            }
        }
    }

    /// Choosing among candidates asked the model for the weight of every value
    /// each reads or writes, on every pick (3.0 M instructions of d_faces in
    /// `Vec<Delta>` building and 3.7 M more in `bottom_up`). A diff is read in
    /// the work of the sets: 300 reads cost the same as 1.
    #[test]
    fn a_candidates_delta_costs_the_sets_not_what_it_reads() {
        let mut model = Model::new(vec![2]);
        for value in 1..=300 {
            model.count(value, 1, Rc::from(vec![0]));
        }
        let insns = [insn(&[], &(1..=300).collect::<Vec<u32>>()), insn(&[], &[1])];
        let region = Region::of(&insns);
        let tracker = Tracker::over(&model, &region, [], [], true);
        WEIGHTS.with(|count| count.set(0));
        let delta = tracker.delta(0, &[], &[0]);
        assert_eq!(WEIGHTS.with(std::cell::Cell::get), 0, "no weight asked at pick time");
        assert_eq!(delta.excess, Some(PressureChange { set: 0, inc: 300 - 2 }));
        tracker.rescan(0, &[], &[0]);
        assert!(WEIGHTS.with(std::cell::Cell::get) >= 300, "premise: the rescan asks for every read");
    }

    /// After two-address, `c := mov a; c := add c, b; use c` has one name for
    /// two values. Placing the add above the use takes c (the copy's) live and
    /// ends c (its own): pressure goes from {c} to {c, b}, one up. Counting by
    /// name saw c live below and above and reported no rise, so a scheduler
    /// after the coalescer chose by a pressure one too low at every tied
    /// add (lru m32 -Os grew 27 bytes).
    #[test]
    fn a_tied_def_and_use_of_one_name_are_two_values() {
        let mut model = Model::new(vec![1]);
        for value in 1..=3 {
            model.count(value, 1, Rc::from(vec![0]));
        }
        // a = 1, b = 2, c = 3.
        let insns = [insn(&[3], &[1]), insn(&[3], &[3, 2]), insn(&[], &[3])];
        let region = Region::of(&insns);
        let mut tracker = Tracker::over(&model, &region, [], [], true);
        tracker.place(2);
        assert_eq!(tracker.current(), &[1]);
        let delta = tracker.delta(1, &[], &[1]);
        assert_eq!(delta.excess, Some(PressureChange { set: 0, inc: 1 }), "the add raises {{c}} to {{c, b}}");
        assert_eq!(delta, tracker.rescan(1, &[], &[1]));
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
