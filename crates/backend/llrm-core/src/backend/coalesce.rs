//! Port of `qbopt/backend/coalesce.py`: a copy whose two values can share a
//! register is not a copy.
//!
//! LLVM's `RegisterCoalescer`, conservative: Briggs and George before the
//! join, since there is no undo.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::{self as ranges, Interval, Segment};
use crate::backend::classes::RegisterClasses;
use crate::backend::neighbours::Neighbours;
use crate::backend::target::{self, Segments};
use crate::backend::{allocate, regclass};
use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, IndexMap};

pub struct Coalescer {
    pub pinned: IndexMap<u32, RegId>,
    pub segments: Segments,
    pub classes: std::rc::Rc<RegisterClasses>,
}

impl Coalescer {
    pub const NAME: &'static str = "coalesce";

    pub fn new(
        pinned: Option<&IndexMap<u32, RegId>>,
        segments: &Segments,
        classes: &std::rc::Rc<RegisterClasses>,
    ) -> Self {
        Self {
            pinned: pinned.cloned().unwrap_or_default(),
            segments: segments.clone(),
            classes: std::rc::Rc::clone(classes),
        }
    }
}

impl LIRTransform for Coalescer {
    fn class_name(&self) -> &'static str {
        "Coalescer"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(joined(&body, Some(&self.pinned), &self.segments, &self.classes))
    }
}

pub type Graph = IndexMap<u32, Neighbours>;

fn _find(
    parent: &mut IndexMap<u32, u32>,
    one: u32,
) -> u32 {
    let mut root = one;
    while parent.get(&root).copied().unwrap_or(root) != root {
        root = parent[&root];
    }
    let mut one = one;
    while parent.get(&one).copied().unwrap_or(one) != one {
        let next = parent[&one];
        parent.insert(one, root);
        one = next;
    }
    root
}

/// `body` with every copy this can prove unnecessary removed.
pub fn joined(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    segments: &Segments,
    classes: &RegisterClasses,
) -> LirBody {
    let mut every = body.pins.clone();
    every.extend(pinned.into_iter().flatten().map(|(value, register)| (*value, *register)));
    let pinned: IndexMap<u32, RegId> = every.into_iter().map(|(value, register)| (value, ir::root(register))).collect();

    let index = ranges::indexed(body);
    let mut live = ranges::intervals(body, Some(&index));
    let masks = allocate::_masks(body, &index, segments);
    let mut widths = allocate::_widest(body);
    let where_of = regclass::classes(body, &BTreeSet::new(), segments, classes);
    let everything: BTreeSet<RegId> = classes.available.iter().copied().collect();
    let mut webs = Webs::new(everything);
    let mut may: IndexMap<u32, BTreeSet<RegId>> = live
        .keys()
        .map(|one| (*one, target::order(where_of.get(one), segments, classes).into_iter().collect()))
        .collect();
    for (value, register) in &pinned {
        if !webs.everything.contains(register) && !where_of.contains_key(value) {
            may.insert(*value, BTreeSet::from([*register]));
        }
    }
    for (value, palette) in may {
        let id = webs.intern(&palette);
        webs.palette_of.insert(value, id);
    }
    let mut held: IndexMap<u32, RegId> = pinned.clone();
    let mut parent: IndexMap<u32, u32> = IndexMap::default();
    // The web a root's values are kept as: the larger of the two joined, so a
    // join costs the smaller's neighbours.
    let mut node_of: IndexMap<u32, u32> = IndexMap::default();
    webs.begin(_interference(body), &held);
    let checking = llrm_support::env_set("LLRM_CHECK_COALESCE");

    for block in &body.blocks {
        for one in &block.insns {
            let Some(pair) = _copy(one) else {
                continue;
            };
            let (mut here, mut there) = (_find(&mut parent, pair.0), _find(&mut parent, pair.1));
            if here == there {
                continue;
            }
            let (mut mine_node, mut theirs_node) =
                (node_of.get(&here).copied().unwrap_or(here), node_of.get(&there).copied().unwrap_or(there));
            // Two pinned to different registers are two registers.
            let (mine_pin, theirs_pin) = (held.get(&mine_node).copied(), held.get(&theirs_node).copied());
            if let (Some(mine), Some(theirs)) = (mine_pin, theirs_pin) {
                if mine != theirs {
                    continue;
                }
            }
            let (Some(mine), Some(theirs)) = (live.get(&mine_node), live.get(&theirs_node)) else {
                continue;
            };
            if webs.near.get(&mine_node).is_some_and(|found| found.contains(&theirs_node)) {
                continue;
            }
            let allowed: BTreeSet<RegId> =
                webs.palette(mine_node).intersection(webs.palette(theirs_node)).copied().collect();
            if allowed.is_empty() {
                continue;
            }
            let merged = _merged(mine, theirs);
            let width =
                widths.get(&mine_node).copied().unwrap_or(0).max(widths.get(&theirs_node).copied().unwrap_or(0)).max(1);
            let allowed: BTreeSet<RegId> = allowed
                .into_iter()
                .filter(|register| !allocate::_clobbered(&merged, *register, &masks, width))
                .collect();
            if allowed.is_empty() {
                continue;
            }
            if [mine_pin, theirs_pin].into_iter().flatten().any(|pin| !allowed.contains(&pin)) {
                continue;
            }
            let k = allowed.len();
            let significant = webs.significant(mine_node, theirs_node, &allowed);
            if checking {
                assert_eq!(
                    significant,
                    webs.significant_by_scan(mine_node, theirs_node, &allowed),
                    "Briggs count of {mine_node} and {theirs_node}"
                );
            }
            if checking {
                for (gone, kept) in [(mine_node, theirs_node), (theirs_node, mine_node)] {
                    assert_eq!(
                        webs.george(gone, kept, &allowed),
                        webs.george_by_scan(gone, kept, &allowed),
                        "George's test of {gone} into {kept}"
                    );
                }
            }
            if significant >= k
                && (held.contains_key(&mine_node)
                    || held.contains_key(&theirs_node)
                    || !webs.either_george(mine_node, theirs_node, &allowed))
            {
                // Briggs and George: the merged class would not be colourable
                continue;
            }
            if mine_pin.is_some() && theirs_pin.is_none() {
                std::mem::swap(&mut here, &mut there);
                std::mem::swap(&mut mine_node, &mut theirs_node);
            }
            // The root that stays is `there`; the web kept is the one with more
            // neighbours.
            let (kept, gone) = if webs.degree(mine_node) > webs.degree(theirs_node) {
                (mine_node, theirs_node)
            } else {
                (theirs_node, mine_node)
            };
            parent.insert(here, there);
            node_of.insert(there, kept);
            node_of.swap_remove(&here);
            live.insert(kept, merged);
            live.swap_remove(&gone);
            widths.insert(kept, width);
            widths.swap_remove(&gone);
            held.swap_remove(&gone);
            let pin = mine_pin.or(theirs_pin);
            if let Some(register) = pin {
                held.insert(kept, register);
            }
            webs.join(gone, kept, &allowed, pin.is_some());
            if checking {
                assert!(webs.consistent(), "the counts after joining {gone} into {kept}");
            }
        }
    }

    // No early return where nothing joined: `_kept` also removes what was
    // already an identity.
    let mut swap: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for insn in &block.insns {
            for one in insn.defines.iter().chain(&insn.uses) {
                let found = _find(&mut parent, *one);
                swap.insert(*one, found);
            }
        }
    }
    let keys: Vec<u32> = parent.keys().copied().collect();
    for one in keys {
        let found = _find(&mut parent, one);
        swap.insert(one, found);
    }
    let swap = |value: u32| swap.get(&value).copied().unwrap_or(value);
    LirBody {
        inputs: body.inputs.iter().map(|value| swap(*value)).collect(),
        ..body.with_blocks(
            body.blocks
                .iter()
                .map(|block| LirBlock {
                    phis: block
                        .phis
                        .iter()
                        .map(|phi| Phi {
                            result: swap(phi.result),
                            incoming: phi.incoming.iter().map(|(at, value)| (*at, swap(*value))).collect(),
                        })
                        .collect(),
                    ..block.with_insns(_kept(block, &swap))
                })
                .collect(),
        )
    }
}

/// What a web is to its neighbours' counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Attr {
    /// Its degree reaches its palette's size: Briggs's significant neighbour,
    /// where nothing is pinned.
    significant: bool,
    palette: u32,
    pinned: bool,
}

/// The interference webs the coalescer joins, and for each web what Briggs's
/// test asks of its neighbours as a count kept as webs join, so a join costs
/// the smaller web's neighbours and not the larger's (LLVM keeps no graph: the
/// live ranges of the two answer, in the size of the smaller).
struct Webs {
    everything: BTreeSet<RegId>,
    palettes: Vec<BTreeSet<RegId>>,
    ids: IndexMap<BTreeSet<RegId>, u32>,
    palette_of: IndexMap<u32, u32>,
    near: Graph,
    attr: IndexMap<u32, Attr>,
    /// Per web: its neighbours that are significant, by palette; and how many
    /// are pinned.
    counts: IndexMap<u32, IndexMap<u32, u32>>,
    /// The same for the neighbours that are significant or pinned: the ones
    /// George's test refuses.
    hots: IndexMap<u32, IndexMap<u32, u32>>,
    pinned_near: IndexMap<u32, u32>,
}

impl Webs {
    fn new(everything: BTreeSet<RegId>) -> Self {
        let mut webs = Self {
            everything: everything.clone(),
            palettes: Vec::new(),
            ids: IndexMap::default(),
            palette_of: IndexMap::default(),
            near: Graph::default(),
            attr: IndexMap::default(),
            counts: IndexMap::default(),
            hots: IndexMap::default(),
            pinned_near: IndexMap::default(),
        };
        webs.intern(&everything);
        webs
    }

    fn intern(
        &mut self,
        set: &BTreeSet<RegId>,
    ) -> u32 {
        if let Some(found) = self.ids.get(set) {
            return *found;
        }
        let id = self.palettes.len() as u32;
        self.palettes.push(set.clone());
        self.ids.insert(set.clone(), id);
        id
    }

    /// The registers a web may take: all, where nothing narrowed it.
    fn palette(
        &self,
        node: u32,
    ) -> &BTreeSet<RegId> {
        &self.palettes[self.palette_of.get(&node).copied().unwrap_or(0) as usize]
    }

    fn degree(
        &self,
        node: u32,
    ) -> usize {
        self.near.get(&node).map_or(0, Neighbours::len)
    }

    fn make_attr(
        &self,
        node: u32,
        pinned: bool,
    ) -> Attr {
        let palette = self.palette_of.get(&node).copied().unwrap_or(0);
        Attr { significant: self.degree(node) >= self.palettes[palette as usize].len(), palette, pinned }
    }

    fn begin(
        &mut self,
        near: Graph,
        held: &IndexMap<u32, RegId>,
    ) {
        self.near = near;
        let nodes: BTreeSet<u32> = self.near.keys().chain(self.palette_of.keys()).chain(held.keys()).copied().collect();
        for node in nodes {
            let attr = self.make_attr(node, held.contains_key(&node));
            self.attr.insert(node, attr);
        }
        // Each node's counts are summed over its neighbours and written once,
        // from the attributes in an array: a pair costs a read, not three
        // hash lookups.
        let near = std::mem::take(&mut self.near);
        let span = self.attr.keys().copied().max().map_or(0, |last| last as usize + 1);
        let mut attrs = vec![None; span];
        for (node, attr) in &self.attr {
            attrs[*node as usize] = Some(*attr);
        }
        for (node, neighbours) in &near {
            let (mut counted, mut hot, mut pinned): (IndexMap<u32, u32>, IndexMap<u32, u32>, u32) =
                (IndexMap::default(), IndexMap::default(), 0);
            for other in neighbours.iter() {
                let attr = attrs[other as usize].expect("a neighbour has its attributes");
                if attr.significant {
                    *counted.entry(attr.palette).or_default() += 1;
                }
                if attr.significant || attr.pinned {
                    *hot.entry(attr.palette).or_default() += 1;
                }
                pinned += u32::from(attr.pinned);
            }
            if !counted.is_empty() {
                self.counts.entry(*node).or_default().extend(counted);
            }
            if !hot.is_empty() {
                self.hots.entry(*node).or_default().extend(hot);
            }
            if pinned > 0 {
                *self.pinned_near.entry(*node).or_default() += pinned;
            }
        }
        self.near = near;
    }

    fn attr_of(
        &self,
        node: u32,
    ) -> Attr {
        self.attr.get(&node).copied().unwrap_or_else(|| self.make_attr(node, false))
    }

    /// `node` gains a neighbour that is `attr`.
    fn add(
        &mut self,
        node: u32,
        attr: Attr,
    ) {
        if attr.significant {
            *self.counts.entry(node).or_default().entry(attr.palette).or_default() += 1;
        }
        if attr.significant || attr.pinned {
            *self.hots.entry(node).or_default().entry(attr.palette).or_default() += 1;
        }
        if attr.pinned {
            *self.pinned_near.entry(node).or_default() += 1;
        }
    }

    /// `node` loses a neighbour that was `attr`.
    fn remove(
        &mut self,
        node: u32,
        attr: Attr,
    ) {
        if attr.significant {
            *self
                .counts
                .get_mut(&node)
                .and_then(|counts| counts.get_mut(&attr.palette))
                .expect("a counted neighbour") -= 1;
        }
        if attr.significant || attr.pinned {
            *self
                .hots
                .get_mut(&node)
                .and_then(|counts| counts.get_mut(&attr.palette))
                .expect("a counted hot neighbour") -= 1;
        }
        if attr.pinned {
            *self.pinned_near.get_mut(&node).expect("a counted pinned neighbour") -= 1;
        }
    }

    fn meets(
        &self,
        palette: u32,
        allowed: &BTreeSet<RegId>,
    ) -> bool {
        self.palettes[palette as usize].intersection(allowed).next().is_some()
    }

    /// How many neighbours of the join of `here` and `there` Briggs counts as
    /// significant for `allowed`.
    fn significant(
        &self,
        here: u32,
        there: u32,
        allowed: &BTreeSet<RegId>,
    ) -> usize {
        let (a, b) = (self.attr_of(here), self.attr_of(there));
        let constrained = a.pinned
            || b.pinned
            || self.pinned_near.get(&here).is_some_and(|n| *n > 0)
            || self.pinned_near.get(&there).is_some_and(|n| *n > 0);
        if constrained {
            return self.significant_by_scan(here, there, allowed);
        }
        let (big, small) = if self.degree(here) >= self.degree(there) { (here, there) } else { (there, here) };
        let mut count: usize = self
            .counts
            .get(&big)
            .into_iter()
            .flatten()
            .filter(|(palette, _)| self.meets(**palette, allowed))
            .map(|(_, n)| *n as usize)
            .sum();
        let empty = Neighbours::default();
        let beside = self.near.get(&big).unwrap_or(&empty);
        count += self
            .near
            .get(&small)
            .unwrap_or(&empty)
            .iter()
            .filter(|other| !beside.contains(other))
            .filter(|other| {
                let attr = self.attr_of(*other);
                attr.significant && self.meets(attr.palette, allowed)
            })
            .count();
        count
    }

    /// `significant` by looking at every neighbour: Briggs as the test is
    /// stated.
    fn significant_by_scan(
        &self,
        here: u32,
        there: u32,
        allowed: &BTreeSet<RegId>,
    ) -> usize {
        let empty = Neighbours::default();
        let neighbours: BTreeSet<u32> = self
            .near
            .get(&here)
            .unwrap_or(&empty)
            .iter()
            .chain(self.near.get(&there).unwrap_or(&empty).iter())
            .filter(|one| *one != here && *one != there)
            .collect();
        let k = allowed.len();
        let constrained = neighbours.iter().chain([&here, &there]).any(|value| self.attr_of(*value).pinned);
        neighbours
            .iter()
            .filter(|o| {
                let palette = self.palette(**o);
                palette.intersection(allowed).next().is_some()
                    && self.degree(**o) >= if constrained { k } else { palette.len() }
            })
            .count()
    }

    /// Whether `gone` can join `kept` without making `kept` harder to colour:
    /// each neighbour of `gone` is one of `kept`'s, or shares no register
    /// with the join, or is neither pinned nor significant. Counted, when
    /// `gone` has the more neighbours, as the hot ones that meet the join
    /// less those `kept` also has.
    fn george(
        &self,
        gone: u32,
        kept: u32,
        allowed: &BTreeSet<RegId>,
    ) -> bool {
        if allowed != self.palette(kept) {
            return false;
        }
        let empty = Neighbours::default();
        let (beside, own) = (self.near.get(&kept).unwrap_or(&empty), self.near.get(&gone).unwrap_or(&empty));
        if own.len() <= beside.len() {
            return own.iter().filter(|other| *other != gone && *other != kept).all(|other| {
                let attr = self.attr_of(other);
                beside.contains(&other) || !self.meets(attr.palette, allowed) || !(attr.significant || attr.pinned)
            });
        }
        let hot: usize = self
            .hots
            .get(&gone)
            .into_iter()
            .flatten()
            .filter(|(palette, _)| self.meets(**palette, allowed))
            .map(|(_, n)| *n as usize)
            .sum();
        let shared = beside
            .iter()
            .filter(|other| own.contains(other))
            .filter(|other| {
                let attr = self.attr_of(*other);
                self.meets(attr.palette, allowed) && (attr.significant || attr.pinned)
            })
            .count();
        hot == shared
    }

    /// `george` by looking at every neighbour of `gone`: the test as it is
    /// stated.
    fn george_by_scan(
        &self,
        gone: u32,
        kept: u32,
        allowed: &BTreeSet<RegId>,
    ) -> bool {
        if allowed != self.palette(kept) {
            return false;
        }
        let empty = Neighbours::default();
        self.near
            .get(&gone)
            .unwrap_or(&empty)
            .iter()
            .filter(|other| *other != gone && *other != kept)
            .all(
                |other| {
                    let palette = self.palette(other);
                    self.near.get(&kept).is_some_and(|found| found.contains(&other))
                        || palette.intersection(allowed).next().is_none()
                        || (!self.attr_of(other).pinned && self.degree(other) < palette.len())
                },
            )
    }

    /// Whether either joins the other under George's test: the one that looks
    /// at the fewer neighbours first, as the answer is the same.
    fn either_george(
        &self,
        one: u32,
        other: u32,
        allowed: &BTreeSet<RegId>,
    ) -> bool {
        let (small, big) = if self.degree(one) <= self.degree(other) { (one, other) } else { (other, one) };
        self.george(small, big, allowed) || self.george(big, small, allowed)
    }

    /// `gone` joined into `kept`, `allowed` what the web may take now and `pin`
    /// whether it is pinned. Costs `gone`'s neighbours, and `kept`'s when
    /// its own count of itself to them changes: its palette, its being pinned,
    /// or its reaching its palette's size, each at most as often as the
    /// palette is large.
    fn join(
        &mut self,
        gone: u32,
        kept: u32,
        allowed: &BTreeSet<RegId>,
        pin: bool,
    ) {
        let (old_gone, old_kept) = (self.attr_of(gone), self.attr_of(kept));
        let gone_near: Vec<u32> = self.near.get(&gone).map(|set| set.iter().collect()).unwrap_or_default();
        let (common, only): (Vec<u32>, Vec<u32>) =
            gone_near.iter().copied().partition(|other| self.near.get(&kept).is_some_and(|set| set.contains(other)));
        // A neighbour of both loses one: it may stop being significant, and its
        // neighbours' counts follow.
        for &other in &common {
            let old = self.attr_of(other);
            let significant = self.degree(other) - 1 >= self.palettes[old.palette as usize].len();
            if significant != old.significant {
                for neighbour in self.near.get(&other).map(|set| set.iter().collect::<Vec<_>>()).unwrap_or_default() {
                    if neighbour != gone {
                        self.remove(neighbour, old);
                        self.add(neighbour, Attr { significant, ..old });
                    }
                }
                self.attr.insert(other, Attr { significant, ..old });
            }
        }
        for &other in &gone_near {
            self.near.get_mut(&other).expect("a neighbour lists its neighbours").remove(&gone);
            self.remove(other, old_gone);
        }
        let palette = self.intern(allowed);
        let degree = self.degree(kept) + only.len();
        let new_kept = Attr { significant: degree >= allowed.len(), palette, pinned: old_kept.pinned || pin };
        if new_kept != old_kept {
            for neighbour in self.near.get(&kept).map(|set| set.iter().collect::<Vec<_>>()).unwrap_or_default() {
                self.remove(neighbour, old_kept);
                self.add(neighbour, new_kept);
            }
        }
        for &other in &only {
            self.near.entry(kept).or_default().insert(other);
            self.near.get_mut(&other).expect("a neighbour lists its neighbours").insert(kept);
            self.add(other, new_kept);
            self.add(kept, self.attr_of(other));
        }
        self.attr.insert(kept, new_kept);
        self.palette_of.insert(kept, palette);
        self.near.entry(kept).or_default();
        self.near.swap_remove(&gone);
        self.attr.swap_remove(&gone);
        self.counts.swap_remove(&gone);
        self.hots.swap_remove(&gone);
        self.pinned_near.swap_remove(&gone);
        self.palette_of.swap_remove(&gone);
    }

    /// Every count kept, against the counts as `begin` makes them: a check for
    /// tests.
    fn consistent(&self) -> bool {
        let mut fresh = self.clone_shape();
        for (node, neighbours) in fresh.near.clone() {
            for other in neighbours.iter() {
                let attr = fresh.attr[&other];
                fresh.add(node, attr);
            }
        }
        let live = |map: &IndexMap<u32, IndexMap<u32, u32>>| -> BTreeSet<(u32, u32, u32)> {
            map.iter()
                .flat_map(|(n, counts)| counts.iter().filter(|(_, c)| **c > 0).map(move |(p, c)| (*n, *p, *c)))
                .collect()
        };
        let pins = |map: &IndexMap<u32, u32>| -> BTreeSet<(u32, u32)> {
            map.iter().filter(|(_, c)| **c > 0).map(|(n, c)| (*n, *c)).collect()
        };
        let degrees_hold = self
            .attr
            .iter()
            .all(|(node, attr)| attr.significant == (self.degree(*node) >= self.palettes[attr.palette as usize].len()));
        degrees_hold
            && live(&self.counts) == live(&fresh.counts)
            && live(&self.hots) == live(&fresh.hots)
            && pins(&self.pinned_near) == pins(&fresh.pinned_near)
    }

    fn clone_shape(&self) -> Self {
        Self {
            everything: self.everything.clone(),
            palettes: self.palettes.clone(),
            ids: self.ids.clone(),
            palette_of: self.palette_of.clone(),
            near: self.near.clone(),
            attr: self.attr.clone(),
            counts: IndexMap::default(),
            hots: IndexMap::default(),
            pinned_near: IndexMap::default(),
        }
    }
}

thread_local! {
    static ASKED: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static NUMBERED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many values the liveness rows of the last interference graph of this
/// thread numbered, for a test that a graph of a few values is not made over
/// every value in the body.
pub fn last_numbered() -> usize {
    NUMBERED.with(std::cell::Cell::get)
}

/// How many values the last interference graph of this thread was asked for,
/// none for all, for a test that `siblings` asks of the webs it grows from
/// only.
pub fn last_asked() -> Option<usize> {
    ASKED.with(std::cell::Cell::get)
}

/// How many instructions the last interference graph of this thread looked at.
pub fn last_visited() -> usize {
    VISITED.with(std::cell::Cell::get)
}

pub fn _interference(body: &LirBody) -> Graph {
    _interference_among(body, None)
}

/// `_interference`, of the values in `only` alone where it is given: a caller
/// that asks of a few values pays for the pairs among them, not for every pair
/// live together. The rows `_interference_among` reads: the walk's, or those
/// found from where the values occur.
enum Rows<'a> {
    Dense(allocate::LiveRows),
    Web(&'a allocate::WebRows),
}

impl Rows<'_> {
    fn numbered(&self) -> usize {
        match self {
            Rows::Dense(rows) => rows.numbered(),
            Rows::Web(rows) => rows.numbered(),
        }
    }

    fn entering(
        &self,
        at: i64,
    ) -> Vec<u32> {
        match self {
            Rows::Dense(rows) => rows.entering(at).collect(),
            Rows::Web(rows) => rows.entering(at).collect(),
        }
    }

    fn leaving(
        &self,
        at: i64,
    ) -> Vec<u32> {
        match self {
            Rows::Dense(rows) => rows.leaving(at).collect(),
            Rows::Web(rows) => rows.leaving(at).collect(),
        }
    }
}

pub fn _interference_among(
    body: &LirBody,
    only: Option<&BTreeSet<u32>>,
) -> Graph {
    ASKED.with(|asked| asked.set(only.map(BTreeSet::len)));
    let wanted = |value: u32| only.is_none_or(|only| only.contains(&value));
    // Asked of a few values, the rows and widths come from where the values
    // occur, not from a walk of every instruction (in a body with no phis,
    // whose arguments are read in other blocks).
    let among = only.map(|only| {
        crate::backend::postings::following(body, |postings| {
            crate::analysis::occurrences::Occurrences::of(postings, only)
        })
    });
    let web = among.as_ref().and_then(|found| found.rows(body));
    let rows = match &web {
        Some(web) => Rows::Web(web),
        None => Rows::Dense(allocate::live_rows_by(body, wanted)),
    };
    NUMBERED.with(|numbered| numbered.set(rows.numbered()));
    VISITED.with(|visited| visited.set(0));
    // A copy's widths are read only where both its values are asked of (an edge
    // needs both).
    let widths = match (&among, &web) {
        (Some(found), Some(_)) => found.widths(body, &wanted),
        _ => crate::analysis::occurrences::widths_of(body.blocks.iter().flat_map(|block| &block.insns), &wanted),
    };
    let mut graph = Building::default();

    let edge = |graph: &mut Building, one: u32, other: u32| {
        if one != other && wanted(one) && wanted(other) {
            graph.link(one, other);
        }
    };
    let all_pairs = |graph: &mut Building, alive: &BTreeSet<u32>| {
        let named: Vec<u32> = alive.iter().copied().filter(|one| wanted(*one)).collect();
        for value in &named {
            for other in &named {
                edge(graph, *value, *other);
            }
        }
    };

    let targets: BTreeSet<i64> = body.blocks.iter().flat_map(|block| block.succ.iter().copied()).collect();
    let mut entries: BTreeSet<i64> = BTreeSet::from([body.entry]);
    entries.extend(body.blocks.iter().map(|block| block.at).filter(|at| !targets.contains(at)));
    // Asked of a few values, a block that names none of them changes nothing in
    // the walk: what is live in it stays so from its end to its start, so
    // it adds pairs only at the entry, at its phis, and at a parallel copy
    // where two of them are live.
    let touched: Option<BTreeSet<usize>> = only.map(|only| {
        crate::backend::postings::following(body, |postings| {
            only.iter()
                .flat_map(|value| postings.defs(*value).iter().chain(postings.uses(*value)))
                .map(|at| at.0 as usize)
                .collect()
        })
    });
    // Asked of a few values, an instruction that names none of them and is no
    // parallel copy changes nothing in the walk (what it defines or reads is
    // never live here), so the walk is of the instructions that do.
    let naming: Option<HashMap<usize, Vec<usize>>> = only.map(|only| {
        crate::backend::postings::following(body, |postings| {
            let mut found: HashMap<usize, Vec<usize>> = HashMap::default();
            for at in only.iter().flat_map(|value| postings.defs(*value).iter().chain(postings.uses(*value))) {
                found.entry(at.0 as usize).or_default().push(at.1 as usize);
            }
            found
        })
    });
    for (block_index, block) in body.blocks.iter().enumerate() {
        let mut alive: BTreeSet<u32> = rows.leaving(block.at).into_iter().filter(|one| wanted(*one)).collect();
        if let Some(touched) = &touched {
            let quiet = !touched.contains(&block_index)
                && !entries.contains(&block.at)
                && block.phis.is_empty()
                && (alive.len() < 2 || !block.insns.iter().any(|one| one.group.is_some()));
            if quiet {
                continue;
            }
        }
        if entries.contains(&block.at) {
            all_pairs(&mut graph, &rows.entering(block.at).into_iter().filter(|one| wanted(*one)).collect());
        }
        // The positions to walk, last first; every one where not asked of a
        // few.
        let sparse: Option<Vec<usize>> = naming
            .as_ref()
            .map(
                |naming| {
                    let mut at: Vec<usize> = naming.get(&block_index).cloned().unwrap_or_default();
                    at.extend(block.insns.iter().enumerate().filter(|(_, one)| one.group.is_some()).map(|(i, _)| i));
                    at.sort_unstable_by(|a, b| b.cmp(a));
                    at.dedup();
                    at
                },
            );
        let mut next = 0;
        let mut index = block.insns.len() as i64 - 1;
        loop {
            if let Some(sparse) = &sparse {
                while next < sparse.len() && sparse[next] as i64 > index {
                    next += 1;
                }
                let Some(&at) = sparse.get(next) else { break };
                index = at as i64;
            } else if index < 0 {
                break;
            }
            VISITED.with(|visited| visited.set(visited.get() + 1));
            let one = &block.insns[index as usize];
            if one.group.is_some() {
                let mut first = index as usize;
                while first > 0 && block.insns[first - 1].group == one.group {
                    first -= 1;
                }
                let group = &block.insns[first..=index as usize];
                // The values live after a parallel copy all coexist.
                all_pairs(&mut graph, &alive);
                // Every source is live before any destination is written.
                let mut before = alive.clone();
                for item in group {
                    for value in &item.defines {
                        before.remove(value);
                    }
                }
                for item in group {
                    before.extend(item.uses.iter().copied());
                }
                all_pairs(&mut graph, &before);
                for item in group {
                    for value in &item.defines {
                        if wanted(*value) {
                            graph.touch(*value);
                        }
                    }
                }
                alive = before;
                index = first as i64 - 1;
                continue;
            }
            let copy = _copy(one);
            let mut equal = None;
            if let Some(copy) = copy {
                if one.defines == [copy.0] && one.uses == [copy.1] {
                    let what = one.what.as_ref().expect("a copy has semantics");
                    let (Loc::Held(into), Loc::Held(source)) = (&what.dests[0], &what.sources[0]) else {
                        unreachable!("a copy is between two values")
                    };
                    let wide = |value: u32| widths.get(&value).copied();
                    if into.width == source.width && Some(source.width) == wide(copy.0) && wide(copy.0) == wide(copy.1)
                    {
                        equal = Some(copy.1);
                    }
                }
            }
            for value in &one.defines {
                for other in &alive {
                    if Some(*other) != equal {
                        edge(&mut graph, *value, *other);
                    }
                }
            }
            for value in &one.defines {
                alive.remove(value);
            }
            alive.extend(one.uses.iter().copied().filter(|value| wanted(*value)));
            index -= 1;
        }
        for value in block.arrives() {
            for other in &alive {
                edge(&mut graph, value, *other);
            }
        }
    }
    graph.finish()
}

/// An interference graph as it is built: a row of bits for each value, indexed
/// by its number, so a pair costs two bit writes and no hash lookup, and the
/// graph's nodes in the order the first pair named them.
#[derive(Default)]
struct Building {
    rows: Vec<Neighbours>,
    seen: Vec<bool>,
    order: Vec<u32>,
}

impl Building {
    fn touch(
        &mut self,
        value: u32,
    ) {
        let at = value as usize;
        if at >= self.rows.len() {
            self.rows.resize_with(at + 1, Neighbours::default);
            self.seen.resize(at + 1, false);
        }
        if !self.seen[at] {
            self.seen[at] = true;
            self.order.push(value);
        }
    }

    fn link(
        &mut self,
        one: u32,
        other: u32,
    ) {
        self.touch(one);
        self.touch(other);
        self.rows[one as usize].insert(other);
        self.rows[other as usize].insert(one);
    }

    fn finish(mut self) -> Graph {
        self.order.iter().map(|value| (*value, std::mem::take(&mut self.rows[*value as usize]))).collect()
    }
}

/// One block's instructions, with a joined copy's bytes given away.
fn _kept(
    block: &LirBlock,
    swap: &dyn Fn(u32) -> u32,
) -> Vec<Arc<Insn>> {
    let identity = |one: &Arc<Insn>| _copy(one).is_some_and(|pair| pair.0 == pair.1);
    lir::without(&block.insns, identity, Some(|one: &Arc<Insn>| _renamed(one, swap)))
}

/// One interval covering both, which is what the joined value occupies.
pub fn _merged(
    one: &Interval,
    other: &Interval,
) -> Interval {
    let mut runs: Vec<Segment> = one.segments.iter().chain(&other.segments).copied().collect();
    runs.sort_by_key(|x| (x.start, x.end));
    let mut out = vec![runs[0]];
    for seg in &runs[1..] {
        let last = out.last_mut().expect("seeded with the first");
        if seg.start <= last.end {
            *last = Segment { start: last.start, end: last.end.max(seg.end) };
            continue;
        }
        out.push(*seg);
    }
    let weight = if other.weight > one.weight { other.weight } else { one.weight };
    Interval { value: one.value, segments: out, weight }
}

/// The (written, read) pair this instruction is a plain move of.
pub fn _copy(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Move {
        return None;
    }
    if what.dests.len() != 1 || what.sources.len() != 1 {
        return None;
    }
    match (&what.dests[0], &what.sources[0]) {
        (Loc::Held(into), Loc::Held(out_of)) => Some((into.value, out_of.value)),
        _ => None,
    }
}

/// A requirement, naming the value that survived the join.
fn _wants(
    side: &[(Held, RegId)],
    swap: &dyn Fn(u32) -> u32,
) -> Vec<(Held, RegId)> {
    side.iter().map(|(held, r)| (Held { value: swap(held.value), width: held.width }, *r)).collect()
}

/// One instruction with every joined value naming its survivor.
pub(crate) fn _renamed(
    one: &Arc<Insn>,
    swap: &dyn Fn(u32) -> u32,
) -> Arc<Insn> {
    let mut made = (**one).clone();
    if let Some(what) = &one.what {
        made.what = Some(Semantics {
            dests: what.dests.iter().map(|x| _settled(x, swap)).collect(),
            sources: what.sources.iter().map(|x| _settled(x, swap)).collect(),
            ..what.clone()
        });
    }
    made.defines = one.defines.iter().map(|v| swap(*v)).collect();
    made.uses = one.uses.iter().map(|v| swap(*v)).collect();
    made.requires = _wants(&one.requires, swap);
    made.delivers = _wants(&one.delivers, swap);
    made.widths = one.widths.iter().map(|(v, w)| (swap(*v), *w)).collect();
    Arc::new(made)
}

fn _settled(
    place: &Loc,
    swap: &dyn Fn(u32) -> u32,
) -> Loc {
    ir::mapped(place, |value| Held { value: swap(value.value), width: value.width })
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_coalesce.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{_interference, _merged, joined};
    use crate::analysis::intervals;
    use crate::backend::{allocate, cpu::ProfileOrName, select, target};
    use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn _move(
        at: i64,
        into: u32,
        out_of: u32,
    ) -> Insn {
        let what = semantics(Operation::Move, "mov", vec![held(into, 2)], vec![held(out_of, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![into], vec![out_of])
    }

    fn _use(
        at: i64,
        reads: u32,
    ) -> Insn {
        let what = semantics(Operation::Push, "push", vec![], vec![held(reads, 2)]);
        Insn::new(at, Some((at, at + 1)), Some(what), vec![], vec![reads])
    }

    fn _define(
        at: i64,
        makes: u32,
    ) -> Insn {
        let what = semantics(
            Operation::Move,
            "mov",
            vec![held(makes, 2)],
            vec![Loc::Imm(Imm { value: i64::from(makes), width: 2, address: None })],
        );
        Insn::new(at, Some((at, at + 3)), Some(what), vec![makes], vec![])
    }

    fn _jump(
        at: i64,
        to: i64,
    ) -> Insn {
        let what = Semantics { name: Some("jmp".to_owned()), target: Some(to), ..Semantics::new(Operation::Jump) };
        Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![])
    }

    fn body(
        name: &str,
        insns: Vec<Insn>,
        pins: &[(u32, RegId)],
    ) -> LirBody {
        LirBody::new(
            name,
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect())],
            IndexMap::default(),
            pins.iter().copied().collect(),
        )
    }

    fn grouped(
        one: Insn,
        group: i64,
    ) -> Insn {
        Insn { group: Some(group), ..one }
    }

    fn allocated(
        body: &LirBody,
        pins: &IndexMap<u32, RegId>,
    ) -> allocate::Assignment {
        allocate::allocate(
            body,
            Some(pins),
            None,
            None,
            None,
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates")
    }

    /// HARR's hoisted selector copy became unencodable mov es,es across a
    /// coverage gap.
    #[test]
    fn test_retained_resource_identity_has_a_legal_encoding() {
        let body = body("resource-copy", vec![_move(3, 1, 1)], &[(1, RegId::ES)]);
        let result =
            allocate::applied(&body, &allocated(&body, &body.pins), &crate::backend::classes::RegisterClasses::m16())
                .expect("applies");
        let insns = result.insns();
        assert_eq!(insns.len(), 1);
        assert_eq!(insns[0].covers, body.insns()[0].covers);
        let emitted = select::emit(insns[0].what.as_ref().expect("semantics"), 3, None, false, false, None);
        assert!(emitted.is_some());
        assert!(emitted.unwrap().code.is_empty());
    }

    // ---------------------------------------------------- tests/test_flow.py

    /// lngmix joins v3 with v9 and then, through the rename, v9 with v20.
    #[test]
    fn test_the_coalescer_joins_the_intervals_it_merges() {
        let segment = |start, end| intervals::Segment { start, end };
        let one = intervals::Interval::new(1, vec![segment(15, 16), segment(59, 60)]);
        let other = intervals::Interval::new(2, vec![segment(0, 15)]);
        assert!(!one.overlaps(&other), "these abut and must not read as overlapping");

        let both = _merged(&one, &other);
        assert_eq!(both.segments, [segment(0, 16), segment(59, 60)]);
        // And a third value inside the union is now correctly refused.
        let third = intervals::Interval::new(3, vec![segment(4, 9)]);
        assert!(!one.overlaps(&third), "the original said nothing about this range");
        assert!(both.overlaps(&third), "the merged interval must cover what it swallowed");
    }

    #[test]
    fn test_equal_resource_values_coalesce_without_consuming_a_gpr() {
        let mut load = _define(0, 1);
        load.what = Some(semantics(
            Operation::Move,
            "mov",
            vec![held(1, 2)],
            vec![Loc::Mem(Mem { through: RegId::BP, offset: 0, disp_width: 2, ..Mem::new(None, 2) })],
        ));
        let mut insns = vec![load];
        insns.extend((0..6).map(|index| _define(3 + index * 3, 10 + index as u32)));
        insns.extend([_move(21, 2, 1), _use(26, 1), _use(28, 2)]);
        insns.extend((0..6).map(|index| _use(30 + index * 3, 10 + index as u32)));
        let body = body("resources", insns, &[(1, RegId::ES), (2, RegId::ES)]);
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        assert_eq!(done.insns().len(), body.insns().len() - 1);
        let result = allocated(&done, &done.pins);
        assert!(result.spilled.is_empty());
        assert!(result.r#where.values().any(|register| *register == RegId::ES));
        let wholes: BTreeSet<RegId> = result.r#where.values().map(|register| allocate::_whole(*register)).collect();
        assert!(
            crate::backend::classes::RegisterClasses::m16().available.iter().all(|register| wholes.contains(register))
        );
    }

    #[test]
    fn test_resource_constraints_survive_coalescing() {
        for other in ["different_resource", "clobber"] {
            let mut insns = vec![_define(0, 1), _move(3, 2, 1), _use(6, 2)];
            let pins = [(1, RegId::ES), (2, if other == "different_resource" { RegId::FS } else { RegId::ES })];
            if other == "clobber" {
                let mut call = Insn::new(
                    5,
                    Some((5, 5)),
                    Some(semantics(Operation::Call, "call", vec![], vec![])),
                    vec![],
                    vec![],
                );
                call.clobbers = BTreeSet::from([RegId::ES]);
                insns.insert(2, call);
            }
            let body = body("resource-safety", insns, &pins);
            let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
            assert_eq!(done.insns().len(), body.insns().len(), "{other}");
        }
    }

    #[test]
    fn test_a_copy_can_share_a_register_while_its_equal_source_is_still_read() {
        let body = body("equal", vec![_define(0, 1), _move(3, 2, 1), _use(5, 1), _use(6, 2)], &[]);
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let insns = done.insns();
        assert_eq!(insns.len(), 3);
        assert_eq!(insns[2].uses, insns[1].uses);
    }

    #[test]
    fn test_a_source_redefined_while_its_copy_is_live_cannot_share() {
        let insns = vec![_define(0, 1), _move(3, 2, 1), _define(5, 1), _use(8, 1), _use(9, 2)];
        let count = insns.len();
        let done = joined(
            &body("different", insns, &[]),
            None,
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let insns = done.insns();
        assert_eq!(insns.len(), count);
        assert_ne!(insns[count - 1].uses, insns[count - 2].uses);
    }

    #[test]
    fn test_parallel_copy_sources_interfere_before_any_destination_is_written() {
        let insns = vec![
            _define(0, 1),
            _define(1, 3),
            _move(3, 2, 1),
            _define(5, 2),
            grouped(_move(8, 1, 2), 1),
            grouped(_move(8, 3, 1), 1),
            _use(9, 1),
            _use(10, 3),
        ];
        let body = body("parallel-sources", insns, &[]);
        assert!(_interference(&body)[&1].contains(&2));
        let live = intervals::intervals(&body, None);
        assert!(live[&1].overlaps(&live[&2]));
    }

    #[test]
    fn test_parallel_copy_destinations_interfere_after_all_are_written() {
        let insns = vec![_define(0, 1), grouped(_move(1, 2, 1), 1), grouped(_move(1, 3, 1), 1), _use(4, 2), _use(5, 3)];
        let body = body("parallel-destinations", insns, &[]);
        assert!(_interference(&body)[&2].contains(&3));
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let insns = done.insns();
        assert_ne!(insns[insns.len() - 2].uses, insns[insns.len() - 1].uses);
    }

    #[test]
    fn test_different_entry_values_cannot_share_even_if_copied_later() {
        let insns = vec![_use(0, 1), _use(1, 2), _move(2, 2, 1), _use(4, 2)];
        let count = insns.len();
        assert_eq!(
            joined(
                &body("inputs", insns, &[]),
                None,
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .insns()
            .len(),
            count
        );
    }

    #[test]
    fn test_a_narrow_copy_is_not_equality_of_a_wide_source() {
        let mut wide = _use(5, 1);
        wide.what = Some(semantics(Operation::Push, "push", vec![], vec![held(1, 4)]));
        let insns = vec![_define(0, 1), _move(3, 2, 1), wide, _use(6, 2)];
        let count = insns.len();
        assert_eq!(
            joined(
                &body("partial", insns, &[]),
                None,
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .insns()
            .len(),
            count
        );
    }

    #[test]
    fn test_a_coalesced_address_keeps_its_memory_operand_defined() {
        let memory = Mem { base: Some(Held { value: 2, width: 2 }), ..Mem::new(None, 2) };
        let load = Insn::new(
            5,
            Some((5, 7)),
            Some(semantics(Operation::Move, "mov", vec![held(3, 2)], vec![Loc::Mem(memory)])),
            vec![3],
            vec![2],
        );
        let insns = vec![_define(0, 1), _move(3, 2, 1), load, _use(7, 1), _use(8, 3)];
        let done = joined(
            &body("address", insns, &[]),
            None,
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let made: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.defines.clone()).collect();
        for one in done.insns() {
            let what = one.what.as_ref().expect("semantics");
            for operand in what.dests.iter().chain(&what.sources) {
                assert!(ir::values(operand).iter().all(|value| made.contains(&value.value)));
            }
        }
    }

    #[test]
    fn test_pinned_return_cannot_absorb_an_incompatible_address_class() {
        let load = Insn::new(
            5,
            Some((5, 7)),
            Some(semantics(
                Operation::FloatLoad,
                "fld",
                vec![Loc::st(0)],
                vec![Loc::Mem(Mem { base: Some(Held { value: 2, width: 2 }), ..Mem::new(None, 8) })],
            )),
            vec![],
            vec![2],
        );
        let body = body("pointer", vec![_define(0, 1), _move(3, 2, 1), load], &[]);
        let pins: IndexMap<u32, RegId> = IndexMap::from_iter([(1, RegId::EAX)]);
        assert_eq!(
            joined(&body, Some(&pins), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16())
                .insns()
                .len(),
            3
        );
        let pinned = LirBody { pins: pins.clone(), ..body };
        assert_eq!(
            joined(&pinned, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16()).insns().len(),
            3
        );
    }

    #[test]
    fn test_coalescing_keeps_the_pinned_return_as_representative() {
        let regs = crate::backend::registerinfo::test_regs();
        let body = body("return", vec![_define(0, 1), _move(3, 2, 1), _use(5, 2)], &[]);
        let done = joined(
            &body,
            Some(&IndexMap::from_iter([(2, RegId::EAX)])),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let insns = done.insns();
        assert_eq!(insns[0].defines, vec![2]);
        assert_eq!(insns[insns.len() - 1].uses, vec![2]);
        for register in [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX] {
            let pins = IndexMap::from_iter([(2, register)]);
            let joined =
                joined(&body, Some(&pins), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
            let emitted = allocate::applied(
                &joined,
                &allocated(&joined, &pins),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("applies");
            let insns = emitted.insns();
            assert_eq!(
                insns[insns.len() - 1].what.as_ref().expect("semantics").sources,
                vec![Loc::Reg(Reg { register: regs.named(register, 2), width: 2 })]
            );
        }
    }

    #[test]
    fn test_two_copies_into_one_value_leave_every_read_defined() {
        let arm = |at: i64, value: u32| {
            let mut block = LirBlock::new(
                at,
                vec![_define(at, value), _move(at + 3, 2, value), _jump(at + 5, 0x20)]
                    .into_iter()
                    .map(Arc::new)
                    .collect(),
            );
            block.succ = vec![0x20];
            block
        };
        let last = LirBlock::new(0x20, vec![Arc::new(_use(0x20, 2))]);
        let body = LirBody::new(
            "two arms",
            0,
            vec![arm(0, 61), arm(0x10, 63), last],
            IndexMap::default(),
            IndexMap::default(),
        );
        let done = joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let made: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.defines.clone()).collect();
        let read: BTreeSet<u32> = done.insns().iter().flat_map(|one| one.uses.clone()).collect();
        let missing: Vec<u32> = read.difference(&made).copied().collect();
        assert!(missing.is_empty(), "read with nothing defining it: {missing:?}");
    }

    #[test]
    fn test_a_pinned_neighbour_does_not_stop_georges_join() {
        for pinned in [false, true] {
            let long_lived: Vec<u32> = (10..16).collect();
            let mut insns: Vec<Insn> =
                long_lived.iter().enumerate().map(|(at, one)| _define(at as i64, *one)).collect();
            insns.extend([_define(0x10, 50), _define(0x13, 1), _move(0x16, 2, 1), _use(0x18, 2)]);
            insns.extend(long_lived.iter().chain([&50]).enumerate().map(|(at, one)| _use(0x20 + at as i64, *one)));
            let count = insns.len();
            let pins: Vec<(u32, RegId)> = if pinned { vec![(50, RegId::BX)] } else { vec![] };
            let body = body("counter", insns, &pins);
            assert_eq!(
                joined(&body, None, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16()).insns().len(),
                count - 1,
                "pinned={pinned}"
            );
        }
    }
}
