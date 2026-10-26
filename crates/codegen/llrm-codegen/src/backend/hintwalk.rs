//! The register each value is wanted in because of the copies that lead to it
//! from a pinned value, kept up to date by what a rewrite names.
//!
//! # How it works
//! `hints` says, for a value, the values it is copied to or from; a pinned
//! value (an argument, a return) has its register fixed. A value wants the
//! register of the pinned value that reaches it through the fewest copies, up
//! to `depth` of them, and where two reach it equally far the one of the
//! smaller value number: a property of the graph, not of the order a walk
//! happened to take, so the walk made afresh and the walk kept up to date are
//! the same map.
//!
//! A rewrite names the values whose instructions it changed (`touched`); only
//! those have a hint list that can differ. The walk keeps for each value it
//! has reached the distance, the value it came through (`via`) and the values
//! that came through it, and for each value that is walked its hint list and
//! the walked values that name it. A changed list gives the values it names a
//! new set of ways. A value whose way is gone is marked, with what came
//! through it, and found again from the ways that are held, nearest first and
//! the smaller value among equals (Ramalingam and Reps's update of a shortest
//! path tree); a shorter way offers itself to what it names. A value found
//! again keeps its place among what came through it, so a chain of copies that
//! a rewrite lengthens by one is relabelled in place, not walked and entered
//! again. What is not reached after that is taken out.
//!
//! ```text
//!   pin A ─ a1 ─ a2 ─ a3 ─ a4        a rewrite names a2, giving it a copy t:
//!   pin A ─ a1 ─ a2 ─ t ─ a3 ─ a4    a3 and a4 are marked, found again through t
//! ```
//!
//! `LLRM_CHECK_WANTED=1` makes the walk afresh after every update and fails
//! where the two differ.
//!
//! # Example
//! Pinned `10` is `ax`; `10` hints `1`, and `1` hints `2` and `3`; `2` hints
//! `4`:
//!
//! | value | way | wants |
//! |---|---|---|
//! | 1 | 10 | ax |
//! | 2, 3 | 1 | ax |
//! | 4 | 2 | ax |
//!
//! A rewrite that names `2` and gives it no hint leaves `4` without a way, and
//! `1`, `2` and `3` as they were.
//!
//! From unit test `a_rewrite_that_names_a_value_changes_what_comes_through_it`
//! in `hintwalk_tests.rs`.
//!
//! # Provides
//! `HintWalk`: `fresh`, `update`, `wanted`.
//!
//! # See also
//! [`crate::backend::allocate`], which asks `wanted` of each value it places.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use llrm_lir::registers::RegId;

use crate::support::hash::{HashSet, IndexMap};

const NONE: u32 = u32::MAX;

/// What is kept of a value.
#[derive(Clone, PartialEq)]
struct Node {
    /// Copies from a pin; `NONE` where not reached.
    dist: u32,
    /// The value it was reached through.
    via: u32,
    /// The values reached through it: the first, and, for each of those, the
    /// next and the previous.
    first: u32,
    next: u32,
    prev: u32,
    pin: bool,
    /// Walked on its hint list (`out`).
    walked: bool,
    /// Marked: its way is gone or runs through a value whose way is.
    marked: bool,
}

impl Default for Node {
    fn default() -> Self {
        Self { dist: NONE, via: NONE, first: NONE, next: NONE, prev: NONE, pin: false, walked: false, marked: false }
    }
}

/// The walk over one body's copy hints.
#[derive(Clone, Default, PartialEq)]
pub struct HintWalk {
    depth: u32,
    /// The pinned values, each by its whole register.
    pins: IndexMap<u32, RegId>,
    nodes: Vec<Node>,
    /// The hint list each walked value was walked on.
    out: Vec<Vec<u32>>,
    /// For a value, the walked values that name it.
    named_by: Vec<Vec<u32>>,
    wanted: IndexMap<u32, RegId>,
    /// Values looked at since the walk was made: what a test of its slope
    /// counts.
    work: usize,
}

type Offers = BinaryHeap<Reverse<(u32, u32, u32)>>;

impl HintWalk {
    /// The walk of `hints` from `pins`, `depth` copies far, made from nothing.
    pub fn fresh(
        hints: &IndexMap<u32, Vec<u32>>,
        pins: &IndexMap<u32, RegId>,
        depth: usize,
    ) -> Self {
        let mut walk = Self { depth: depth.min((NONE - 1) as usize) as u32, ..Self::default() };
        walk.update(hints, pins, None);
        walk
    }

    /// How many values have been looked at.
    pub fn work(&self) -> usize {
        self.work
    }

    /// The register each value reached is wanted in.
    pub fn wanted(&self) -> &IndexMap<u32, RegId> {
        &self.wanted
    }

    /// Brought up to date with `hints` and `pins` as they are now, where
    /// the values `touched` are all that a rewrite named: `None` walks afresh.
    pub fn update(
        &mut self,
        hints: &IndexMap<u32, Vec<u32>>,
        pins: &IndexMap<u32, RegId>,
        touched: Option<&HashSet<u32>>,
    ) {
        let rebuild = touched.is_none() || self.pins.keys().any(|one| !pins.contains_key(one));
        if rebuild {
            *self = Self { depth: self.depth, work: self.work, ..Self::default() };
        }
        let mut dirty: Vec<u32> = Vec::new();
        let mut changed: Vec<u32> = Vec::new();
        let mut roots: Vec<u32> = Vec::new();
        for (value, register) in pins {
            let whole = crate::backend::allocate::_whole(*register);
            if self.pins.insert(*value, whole) != Some(whole) {
                self.make_pin(*value, whole);
                dirty.push(*value);
                roots.push(*value);
            }
        }
        if let Some(touched) = touched.filter(|_| !rebuild) {
            dirty.extend(touched.iter().copied().filter(|one| self.walked(*one)));
        }
        dirty.sort_unstable();
        dirty.dedup();
        for one in dirty {
            let list = hints.get(&one).cloned().unwrap_or_default();
            // A value that is a pin now is nearer to everything it names.
            if roots.contains(&one) {
                changed.extend(list.iter().copied());
            }
            self.name(one, list, &mut changed);
        }
        changed.sort_unstable();
        changed.dedup();

        // Which values have lost their way, and what came through them.
        let mut marked: Vec<u32> = Vec::new();
        for &value in &changed {
            self.work += 1;
            if self.reached(value) && !self.supported(value) {
                self.mark_below(value, &mut marked);
            }
        }
        // What has no way, or may have a nearer one, is offered from the ways
        // that are held.
        let mut offers = Offers::new();
        for &value in changed.iter().chain(&marked) {
            self.offer_from_named_by(value, &mut offers);
        }
        while let Some(Reverse((distance, way, value))) = offers.pop() {
            self.work += 1;
            if !self.holds(way, distance - 1) || self.nodes[value as usize].pin {
                continue;
            }
            let node = &self.nodes[value as usize];
            if node.dist != NONE && !node.marked && (node.dist, node.via) <= (distance, way) {
                continue;
            }
            self.reach(value, distance, way);
            if distance < self.depth {
                if !self.walked(value) {
                    let list = hints.get(&value).cloned().unwrap_or_default();
                    self.name(value, list, &mut Vec::new());
                }
                let named = self.out[value as usize].clone();
                for target in named {
                    self.offer(target, distance + 1, value, &mut offers);
                }
            }
        }
        // What is left marked has no way.
        for value in marked {
            if self.nodes[value as usize].marked {
                self.take_out(value);
            }
        }
        // Each pin is walked on its hints.
        for pin in self.pins.keys().copied().collect::<Vec<_>>() {
            if !self.walked(pin) {
                let list = hints.get(&pin).cloned().unwrap_or_default();
                self.name(pin, list, &mut Vec::new());
            }
        }
    }

    fn ensure(
        &mut self,
        value: u32,
    ) {
        let need = value as usize + 1;
        if self.nodes.len() < need {
            self.nodes.resize(need, Node::default());
            self.out.resize(need, Vec::new());
            self.named_by.resize(need, Vec::new());
        }
    }

    fn walked(
        &self,
        value: u32,
    ) -> bool {
        self.nodes.get(value as usize).is_some_and(|node| node.walked)
    }

    fn reached(
        &self,
        value: u32,
    ) -> bool {
        self.nodes.get(value as usize).is_some_and(|node| node.dist != NONE)
    }

    /// Whether `value` is pinned or reached, unmarked, `distance` copies from a
    /// pin.
    fn holds(
        &self,
        value: u32,
        distance: u32,
    ) -> bool {
        match self.nodes.get(value as usize) {
            Some(node) if node.pin => distance == 0,
            Some(node) => node.dist == distance && !node.marked,
            None => false,
        }
    }

    /// The distance of a value that is pinned or reached and not marked.
    fn distance(
        &self,
        value: u32,
    ) -> Option<u32> {
        let node = self.nodes.get(value as usize)?;
        if node.pin {
            Some(0)
        } else if node.dist != NONE && !node.marked {
            Some(node.dist)
        } else {
            None
        }
    }

    /// Whether the way a reached value came by is still a way.
    fn supported(
        &self,
        value: u32,
    ) -> bool {
        let node = &self.nodes[value as usize];
        node.via != NONE
            && self.holds(node.via, node.dist - 1)
            && self.out.get(node.via as usize).is_some_and(|list| list.contains(&value))
    }

    /// The register of a value that is pinned or reached.
    fn register(
        &self,
        value: u32,
    ) -> RegId {
        self.pins.get(&value).copied().unwrap_or_else(|| self.wanted[&value])
    }

    /// `value` is a pin now: if it was reached it is not, and what came
    /// through it is offered its register afresh.
    fn make_pin(
        &mut self,
        value: u32,
        register: RegId,
    ) {
        self.ensure(value);
        self.unlink(value);
        let node = &mut self.nodes[value as usize];
        node.pin = true;
        node.dist = NONE;
        node.via = NONE;
        node.marked = false;
        self.wanted.swap_remove(&value);
        self.set_below(value, register);
    }

    /// Everything reached through `value`, unmarked, wanted in `register`.
    fn set_below(
        &mut self,
        value: u32,
        register: RegId,
    ) {
        let mut below = self.kids(value);
        while let Some(one) = below.pop() {
            self.work += 1;
            if !self.nodes[one as usize].marked {
                self.wanted.insert(one, register);
                below.extend(self.kids(one));
            }
        }
    }

    fn kids(
        &self,
        value: u32,
    ) -> Vec<u32> {
        let mut out = Vec::new();
        let mut at = self.nodes[value as usize].first;
        while at != NONE {
            out.push(at);
            at = self.nodes[at as usize].next;
        }
        out
    }

    /// `value` taken out of the list of those reached through its way.
    fn unlink(
        &mut self,
        value: u32,
    ) {
        let (via, next, prev) = {
            let node = &self.nodes[value as usize];
            (node.via, node.next, node.prev)
        };
        if via == NONE {
            return;
        }
        if prev == NONE {
            self.nodes[via as usize].first = next;
        } else {
            self.nodes[prev as usize].next = next;
        }
        if next != NONE {
            self.nodes[next as usize].prev = prev;
        }
        let node = &mut self.nodes[value as usize];
        node.via = NONE;
        node.next = NONE;
        node.prev = NONE;
    }

    /// `value` among those reached through `way`.
    fn link(
        &mut self,
        value: u32,
        way: u32,
    ) {
        self.unlink(value);
        let first = self.nodes[way as usize].first;
        {
            let node = &mut self.nodes[value as usize];
            node.via = way;
            node.next = first;
            node.prev = NONE;
        }
        if first != NONE {
            self.nodes[first as usize].prev = value;
        }
        self.nodes[way as usize].first = value;
    }

    /// `value` and everything that came through it marked, without a way until
    /// one is found.
    fn mark_below(
        &mut self,
        value: u32,
        marked: &mut Vec<u32>,
    ) {
        let mut below = vec![value];
        while let Some(one) = below.pop() {
            self.work += 1;
            if self.nodes[one as usize].marked {
                continue;
            }
            self.nodes[one as usize].marked = true;
            marked.push(one);
            below.extend(self.kids(one));
        }
    }

    /// A marked value that found no way: not reached, not walked.
    fn take_out(
        &mut self,
        value: u32,
    ) {
        self.unlink(value);
        let node = &mut self.nodes[value as usize];
        node.dist = NONE;
        node.marked = false;
        node.first = NONE;
        self.wanted.swap_remove(&value);
        self.name(value, Vec::new(), &mut Vec::new());
        self.nodes[value as usize].walked = false;
    }

    /// `value` walked on `list`: the values it named and no longer names, and
    /// those it names newly, are pushed on `changed`.
    fn name(
        &mut self,
        value: u32,
        list: Vec<u32>,
        changed: &mut Vec<u32>,
    ) {
        self.ensure(value);
        // A value named twice is named.
        let mut list = list;
        let mut seen: Vec<u32> = Vec::with_capacity(list.len());
        list.retain(|one| {
            if seen.contains(one) {
                false
            } else {
                seen.push(*one);
                true
            }
        });
        for target in &list {
            self.ensure(*target);
        }
        let before = std::mem::take(&mut self.out[value as usize]);
        for target in &before {
            if !list.contains(target) {
                let held = &mut self.named_by[*target as usize];
                if let Some(at) = held.iter().position(|one| *one == value) {
                    held.swap_remove(at);
                }
                changed.push(*target);
            }
        }
        for target in &list {
            if !before.contains(target) {
                self.named_by[*target as usize].push(value);
                changed.push(*target);
            }
        }
        self.out[value as usize] = list;
        self.nodes[value as usize].walked = true;
    }

    /// The nearest way to `value` among the values that name it and are held,
    /// offered where it is better than what `value` has.
    fn offer_from_named_by(
        &self,
        value: u32,
        offers: &mut Offers,
    ) {
        let Some(node) = self.nodes.get(value as usize) else { return };
        if node.pin {
            return;
        }
        let best = self.named_by[value as usize]
            .iter()
            .filter_map(|way| Some((self.distance(*way)? + 1, *way)))
            .filter(|(distance, _)| *distance <= self.depth)
            .min();
        if let Some((distance, way)) = best
            && (node.dist == NONE || node.marked || (distance, way) < (node.dist, node.via))
        {
            offers.push(Reverse((distance, way, value)));
        }
    }

    /// `way` reaches `value` at `distance`, offered if that is better.
    fn offer(
        &self,
        value: u32,
        distance: u32,
        way: u32,
        offers: &mut Offers,
    ) {
        if distance > self.depth {
            return;
        }
        let Some(node) = self.nodes.get(value as usize) else { return };
        if !node.pin && (node.dist == NONE || node.marked || (distance, way) < (node.dist, node.via)) {
            offers.push(Reverse((distance, way, value)));
        }
    }

    /// `value` reached at `distance` through `way`; what came through it takes
    /// its register if that changed.
    fn reach(
        &mut self,
        value: u32,
        distance: u32,
        way: u32,
    ) {
        self.link(value, way);
        let node = &mut self.nodes[value as usize];
        node.dist = distance;
        node.marked = false;
        let register = self.register(way);
        let changed = self.wanted.get(&value) != Some(&register);
        self.wanted.insert(value, register);
        if changed {
            self.set_below(value, register);
        }
    }
}

#[cfg(test)]
impl HintWalk {
    /// What must hold of the tables between updates.
    fn invariants(&self) -> Result<(), String> {
        for (at, node) in self.nodes.iter().enumerate() {
            let value = at as u32;
            if node.marked {
                return Err(format!("{value} is marked between updates"));
            }
            if node.dist != NONE {
                if !self.wanted.contains_key(&value) {
                    return Err(format!("{value} is reached and not wanted"));
                }
                if node.via == NONE {
                    return Err(format!("{value} is reached with no way"));
                }
                if !self.holds(node.via, node.dist - 1) {
                    return Err(format!("{value} came through {} which is not at {}", node.via, node.dist - 1));
                }
                if !self.out[node.via as usize].contains(&value) {
                    return Err(format!("{value} came through {} which no longer names it", node.via));
                }
                if !self.kids(node.via).contains(&value) {
                    return Err(format!("{value} is not among those reached through {}", node.via));
                }
            } else if self.wanted.contains_key(&value) {
                return Err(format!("{value} is wanted and not reached"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "hintwalk_tests.rs"]
mod tests;
