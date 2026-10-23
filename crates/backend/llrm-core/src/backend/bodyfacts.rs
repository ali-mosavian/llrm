//! Facts about the definitions of a body's values that the passes around the
//! allocator ask: which values are literals, addresses or copies, which loads
//! of a cell hold until their last use, and what may write a cell.
//! Moved out of `spiller` unchanged.
//!
//! # How it works
//! Each function reads the postings or the instructions of a body and answers
//! for a set of values. `_stable_loads_by` proves, per live range, what LLVM
//! reads from a load's invariance flag (DEVIATE D-I of `greedy::remat`).
//!
//! # Example
//! `v1 := mov 5; v2 := mov v1; ret v2`: `_constants` finds `v1` and `v2` both
//! the literal 5 (a copy of a literal is the literal), `_copies` finds `v2`
//! a copy of `v1`.
//!
//! # Used by
//! `spiller`, `ssaspill`, `constrain`, `floatassign`, `peep`, `greedy::remat`.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::key;
use crate::backend::postings::{At, Postings};
use crate::greedy::remat::group_source;
use crate::greedy::tii::_width;
use crate::model::ir::{AddressRef, Imm, Loc, Mem, Operation, Space};
use crate::model::lir::{Block, Insn, LirBody};
use crate::support::hash::IndexMap;

pub(crate) fn _plain_move(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Move || what.dests.len() != 1 || what.sources.len() != 1 {
        return None;
    }
    let (Loc::Held(dest), Loc::Held(source)) = (&what.dests[0], &what.sources[0]) else {
        return None;
    };
    if dest.width != source.width || !one.requires.is_empty() || !one.delivers.is_empty() {
        return None;
    }
    Some((dest.value, source.value))
}

/// Each of `values` that one plain same-width `mov` copies from another value,
/// and that value: made once, so the copy is made again as its source is (the
/// one answer SsaSpill and the allocator both read).
pub(crate) fn _copies(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, u32> {
    let mut defined: IndexMap<u32, usize> = IndexMap::default();
    for one in body.insns() {
        for value in &one.defines {
            *defined.entry(*value).or_default() += 1;
        }
    }
    let mut out = IndexMap::default();
    for one in body.insns() {
        let (Some(what), [value], [source]) = (&one.what, one.defines.as_slice(), one.uses.as_slice()) else {
            continue;
        };
        let ([Loc::Held(dest)], [Loc::Held(from)]) = (what.dests.as_slice(), what.sources.as_slice()) else { continue };
        if what.op == Operation::Move
            && one.group.is_none()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && what.name.as_deref() == Some("mov")
            && dest.value == *value
            && from.value == *source
            && dest.width == from.width
            && values.contains(value)
            && defined.get(value) == Some(&1)
            && defined.get(source) == Some(&1)
        {
            out.insert(*value, *source);
        }
    }
    out
}

#[cfg(test)]
thread_local! {
    /// How many values `_stable_loads_from` has been asked about.
    pub(crate) static STABLE_ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// (On one line: tests/target_facts.rs reads what comes before a line that is
// only the attribute as the code.)
#[cfg(test)]
thread_local! { pub(crate) static KEEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

/// Values loaded from a cell nothing changes before they are used again.
pub(crate) fn _stable_loads(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Mem> {
    _stable_loads_through(body, values, &IndexMap::default())
}

/// `_stable_loads_through` with the postings of the body to hand: where the
/// values occur is read from them, not found by a scan.
pub(crate) fn _stable_loads_by(
    body: &LirBody,
    values: &BTreeSet<u32>,
    copies: &IndexMap<u32, u32>,
    postings: &Postings,
) -> IndexMap<u32, Mem> {
    _stable_loads_from(body, values, copies, Some(postings))
}

/// `_stable_loads`, where a load's copies (`copies`: copy -> source) are made
/// again as it is: the cell must hold until the last use of any of them, not
/// only of the load.
pub(crate) fn _stable_loads_through(
    body: &LirBody,
    values: &BTreeSet<u32>,
    copies: &IndexMap<u32, u32>,
) -> IndexMap<u32, Mem> {
    _stable_loads_from(body, values, copies, None)
}

pub(crate) fn _stable_loads_from(
    body: &LirBody,
    values: &BTreeSet<u32>,
    copies: &IndexMap<u32, u32>,
    postings: Option<&Postings>,
) -> IndexMap<u32, Mem> {
    #[cfg(test)]
    STABLE_ASKED.with(|count| count.set(count.get() + values.len()));
    if values.is_empty() {
        return IndexMap::default();
    }
    let mut definitions: IndexMap<u32, Vec<(Arc<Insn>, Option<Mem>, (usize, usize))>> = IndexMap::default();
    let mut uses: IndexMap<u32, Vec<Arc<Insn>>> = values.iter().map(|value| (*value, Vec::new())).collect();
    // Where each use and each load is: (block, position), in step with `uses`
    // and `definitions`.
    let mut use_places: IndexMap<u32, Vec<(usize, usize)>> = values.iter().map(|value| (*value, Vec::new())).collect();
    // The load a copy comes from, through copies of copies.
    let root = |value: u32| {
        let mut at = value;
        while let Some(source) = copies.get(&at) {
            at = *source;
        }
        at
    };
    // The values that matter are those asked of and their copies; where they
    // occur is all that is read of the body.
    let mut asked: BTreeSet<u32> = values.clone();
    asked.extend(copies.keys().copied().filter(|copy| {
        let owner = root(*copy);
        owner != *copy && values.contains(&owner)
    }));
    let occurrences = match postings {
        Some(postings) => crate::analysis::occurrences::Occurrences::of(postings, &asked),
        None => crate::analysis::occurrences::Occurrences::scan(body, &|value| asked.contains(&value)),
    };
    if postings.is_some() && llrm_support::env_flag!("LLRM_CHECK_OCCURRENCES") {
        occurrences.check_against_scan(body, &|value| asked.contains(&value));
    }
    // Each use of an owner by an instruction, once for every distinct value of
    // it the instruction reads (the owner and its copies).
    let mut reads: std::collections::BTreeMap<(u32, (usize, usize)), usize> = std::collections::BTreeMap::new();
    for value in &asked {
        for made in occurrences.named(*value) {
            let (block_at, position) = made.place;
            let one = &body.blocks[block_at].insns[position];
            if made.used {
                let owner = root(*value);
                if owner != *value && values.contains(&owner) {
                    *reads.entry((owner, made.place)).or_default() += 1;
                }
                if values.contains(value) {
                    *reads.entry((*value, made.place)).or_default() += 1;
                }
            }
            if made.defined && values.contains(value) {
                let mut cell = None;
                if let Some(what) = &one.what {
                    if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                        if let ([Loc::Held(dest)], [Loc::Mem(source)]) =
                            (what.dests.as_slice(), what.sources.as_slice())
                        {
                            if dest.value == *value
                                && dest.width == source.width
                                && source.addr.is_some()
                                && one.defines == [*value]
                                && one.uses.is_empty()
                                && one.clobbers.is_empty()
                                && one.group.is_none()
                                && one.spread.is_empty()
                                && !one.volatile
                                && one.symbol != Some(true)
                            {
                                cell = Some(source.clone());
                            }
                        }
                    }
                    // An x87 load of a fixed cell (a frame cell, a global):
                    // made again where it is read, as the
                    // integer load above.
                    if what.op == Operation::FloatLoad && what.name.as_deref() == Some("fld") {
                        if let ([Loc::Held(dest)], [Loc::Mem(source)]) =
                            (what.dests.as_slice(), what.sources.as_slice())
                        {
                            if dest.value == *value
                                && (_fixed_data(source).is_some() || _incoming_frame(source))
                                && one.uses.is_empty()
                                && one.defines == [*value]
                                && one.clobbers.is_empty()
                                && one.group.is_none()
                                && one.spread.is_empty()
                                && !one.volatile
                                && one.symbol != Some(true)
                            {
                                cell = Some(source.clone());
                            }
                        }
                    }
                }
                definitions.entry(*value).or_default().push((Arc::clone(one), cell, made.place));
            }
        }
    }
    for ((owner, place), times) in reads {
        for _ in 0..times {
            uses[&owner].push(Arc::clone(&body.blocks[place.0].insns[place.1]));
            use_places[&owner].push(place);
        }
    }

    let mut result: IndexMap<u32, Mem> = IndexMap::default();
    // What every value asks of the body's blocks alike, found for the first
    // that asks.
    let flow = std::cell::OnceCell::new();
    let writers = std::cell::OnceCell::new();
    for value in values {
        let Some(found) = definitions.get(value) else {
            continue;
        };
        if found.len() != 1 || uses[value].is_empty() {
            continue;
        }
        let (define, cell, define_at) = &found[0];
        let Some(cell) = cell else {
            continue;
        };
        if uses[value].iter().any(|one| one.group.is_some()) {
            continue;
        }
        let held = if _exact_frame(cell) || _fixed_data(cell).is_some() {
            let flow = flow.get_or_init(|| Flow::of(body));
            // Only what may write the cell is asked of, and the blocks between
            // the load and its uses are gone over.
            let writers = writers.get_or_init(|| Writers::of(body));
            let mut events = Events::default();
            let near = if _exact_frame(cell) {
                writers.near(cell)
            } else {
                let (explicit, background) = writers.near_fixed(cell, body.spares.is_empty());
                events.background = background;
                explicit
            };
            for (block, position) in near {
                if !_keeps(&body.blocks[block].insns[position], define, cell, true, body) {
                    events.own.entry(block).or_default().push((position, false));
                }
            }
            events.own.entry(define_at.0).or_default().push((define_at.1, true));
            for list in events.own.values_mut() {
                list.sort_unstable();
            }
            _holds_at(flow, &events, &use_places[value])
        } else {
            let flow = flow.get_or_init(|| Flow::of(body));
            _unchanged(body, flow, define, cell, &uses[value])
        };
        if llrm_support::env_flag!("LLRM_CHECK_UNCHANGED") {
            assert!(
                held == _unchanged_reference(body, define, cell, &uses[value]),
                "{}: whether the cell holds differs from working it out as before",
                body.name
            );
        }
        if held {
            result.insert(*value, cell.clone());
        }
    }
    result
}

/// The blocks of a body by address, and each one's predecessors.
pub(crate) struct Flow<'a> {
    predecessors: IndexMap<i64, Vec<i64>>,
    blocks: IndexMap<i64, &'a Block>,
    /// The same by position in `body.blocks`.
    parents: Vec<Vec<usize>>,
    entry: usize,
}

impl<'a> Flow<'a> {
    pub(crate) fn of(body: &'a LirBody) -> Self {
        let predecessors = _predecessors(body);
        let blocks: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
        let parents = body
            .blocks
            .iter()
            .map(|block| predecessors[&block.at].iter().filter_map(|at| blocks.get_index_of(at)).collect())
            .collect();
        let entry = blocks.get_index_of(&body.entry).unwrap_or(0);
        Self { predecessors, blocks, parents, entry }
    }
}

/// Where the instructions that may write a frame cell are, found once for a
/// body: the loads asked about are many and an instruction that writes none of
/// their cells is nearly every instruction.
pub(crate) struct Writers {
    /// (block, position) of what may write memory no exact frame cell names and
    /// no fixed data cell: a call, an unmodeled write, a store to a based,
    /// indexed or unplaced cell. (A store to a fixed data cell never meets a
    /// frame cell: other regions in `overlap::may_overlap`.)
    wild: Vec<(usize, usize)>,
    /// The stores to an exact frame cell, by its first byte.
    exact: std::collections::BTreeMap<i64, Vec<(usize, usize)>>,
    /// The widest of those cells.
    widest: i64,
    /// What may write memory no fixed cell of a frame or a data segment names:
    /// a call, an unmodeled write, a store through a register or to an
    /// unplaced cell.
    hard_wild: Vec<(usize, usize)>,
    /// The stores to a fixed data cell, by its space and symbol.
    fixed: crate::support::hash::HashMap<(u8, i64), Group>,
    widest_fixed: i64,
    /// What writes fixed data cells of more than one space and symbol: asked of
    /// every fixed cell one by one.
    mixed: Vec<(usize, usize)>,
    /// The same by block, in position order, of what writes the cells of one
    /// symbol: of every symbol, and of the external symbols alone.
    stores: Lists,
    externals: Lists,
}

/// The stores to the fixed data cells of one space and symbol.
#[derive(Default)]
pub(crate) struct Group {
    /// (first byte, (block, position)) in program order.
    places: Vec<(i64, (usize, usize))>,
    /// The same by first byte, made when asked.
    by_byte: std::cell::OnceCell<Vec<(i64, (usize, usize))>>,
    /// What writes cells of this symbol alone, in program order.
    only: Vec<(usize, usize)>,
}

/// Positions in a block, in order.
pub(crate) type Lists = Vec<Vec<usize>>;

/// What kills a fixed data cell without being named for it (LLVM's
/// `AliasSetTracker` keeps an unknown-alias set for the same reason): a store
/// to another external symbol meets an external symbol's cell, a store to any
/// symbol meets a segment's cell, and a store to another segment's cell meets
/// nothing of it. The answer is the regions' (`overlap::may_overlap`), whatever
/// the displacements.
pub(crate) struct Background<'a> {
    lists: &'a Lists,
    /// What writes the cell's own symbol: asked of by bytes, not here.
    excluded: &'a [(usize, usize)],
}

/// What may change a cell and where it is made to hold.
#[derive(Default)]
pub(crate) struct Events<'a> {
    /// Per block, (position, true for the load, false for a write), in order.
    own: crate::support::hash::HashMap<usize, Vec<(usize, bool)>>,
    /// The writes every block's list of positions stands for.
    background: Option<Background<'a>>,
}

impl Events<'_> {
    /// The last event of `block` before `position` (the last of all when
    /// `None`): its position and whether it is the load.
    pub(crate) fn before(
        &self,
        block: usize,
        position: Option<usize>,
    ) -> Option<(usize, bool)> {
        let own = self.own.get(&block).and_then(|list| {
            list.iter().rev().find(|(at, _)| position.is_none_or(|bound| *at < bound)).map(|(at, load)| (*at, *load))
        });
        let background = self
            .background
            .as_ref()
            .and_then(
                |background| {
                    let list = background.lists.get(block)?;
                    let mut end = position.map_or(list.len(), |bound| list.partition_point(|at| *at < bound));
                    while end > 0 && background.excluded.binary_search(&(block, list[end - 1])).is_ok() {
                        end -= 1;
                    }
                    end.checked_sub(1).map(|last| (list[last], false))
                },
            );
        match (own, background) {
            (Some(own), Some(background)) => Some(if background.0 > own.0 { background } else { own }),
            (one, other) => one.or(other),
        }
    }
}

impl Writers {
    pub(crate) fn of(body: &LirBody) -> Self {
        let mut found = Self {
            wild: Vec::new(),
            exact: Default::default(),
            widest: 1,
            hard_wild: Vec::new(),
            fixed: Default::default(),
            widest_fixed: 1,
            mixed: Vec::new(),
            stores: vec![Vec::new(); body.blocks.len()],
            externals: vec![Vec::new(); body.blocks.len()],
        };
        for (block, one_block) in body.blocks.iter().enumerate() {
            for (position, one) in one_block.insns.iter().enumerate() {
                let dests = one
                    .what
                    .iter()
                    .flat_map(|what| &what.dests)
                    .filter_map(
                        |dest| match dest {
                            Loc::Mem(cell) => Some(cell),
                            _ => None,
                        },
                    );
                let mut wild = one.call.is_some() || one.unmodeled_write();
                let mut hard = wild;
                let mut key: Option<(u8, i64)> = None;
                let mut several = false;
                for cell in dests {
                    if _exact_frame(cell) {
                        let addr = cell.addr.expect("an exact frame cell has an address");
                        found.exact.entry(addr.disp).or_default().push((block, position));
                        found.widest = found.widest.max(i64::from(cell.width));
                    } else if let Some(one_key) = _fixed_data(cell) {
                        let addr = cell.addr.expect("a fixed data cell has an address");
                        found.fixed.entry(one_key).or_default().places.push((addr.disp, (block, position)));
                        found.widest_fixed = found.widest_fixed.max(i64::from(cell.width));
                        if key.is_some_and(|had| had != one_key) {
                            several = true;
                        }
                        key = Some(one_key);
                    } else {
                        wild = true;
                        hard = true;
                    }
                }
                if wild {
                    found.wild.push((block, position));
                }
                if hard {
                    found.hard_wild.push((block, position));
                }
                if let Some(key) = key {
                    if several {
                        found.mixed.push((block, position));
                    } else {
                        let group = found.fixed.get_mut(&key).expect("its group is made");
                        if group.only.last() != Some(&(block, position)) {
                            group.only.push((block, position));
                            found.stores[block].push(position);
                            if key.0 == 1 {
                                found.externals[block].push(position);
                            }
                        }
                    }
                }
            }
        }
        found
    }

    /// The instructions that can fail to keep `cell` (an exact frame cell): any
    /// other writes nothing but exact frame cells that do not meet it, and
    /// keeps it, as `_keeps` finds.
    pub(crate) fn near(
        &self,
        cell: &Mem,
    ) -> Vec<(usize, usize)> {
        let low = cell.addr.expect("an exact frame cell has an address").disp;
        let high = low + i64::from(cell.width);
        let mut found = self.wild.clone();
        for places in self.exact.range(low - self.widest + 1..high).map(|(_, places)| places) {
            found.extend(places.iter().copied());
        }
        found.sort_unstable();
        found.dedup();
        found
    }

    /// `near` for a fixed data cell: what the cell's own symbol is written by
    /// meets it by bytes; what writes another symbol meets it as the regions
    /// say (`Background`) where the body has no `spares` pair (`by_regions`),
    /// else is named one by one.
    pub(crate) fn near_fixed(
        &self,
        cell: &Mem,
        by_regions: bool,
    ) -> (Vec<(usize, usize)>, Option<Background<'_>>) {
        let key = _fixed_data(cell).expect("a fixed data cell");
        let low = cell.addr.expect("a fixed data cell has an address").disp;
        let high = low + i64::from(cell.width.max(1));
        let mut found = self.hard_wild.clone();
        found.extend(self.mixed.iter().copied());
        let group = self.fixed.get(&key);
        if let Some(group) = group {
            let by_byte = group
                .by_byte
                .get_or_init(
                    || {
                        let mut sorted = group.places.clone();
                        sorted.sort_unstable();
                        sorted
                    },
                );
            let first = by_byte.partition_point(|(byte, _)| *byte < low - self.widest_fixed + 1);
            found.extend(by_byte[first..].iter().take_while(|(byte, _)| *byte < high).map(|(_, place)| *place));
        }
        // A `spares` pair (the optimizer's proof that a write is apart from a
        // load) lets a store to another space than the cell's keep it: those
        // are then named one by one. The stores to an external symbol meet an
        // external symbol's cell whatever a pair says.
        let (lists, named) = match (key.0, by_regions) {
            (1, true) => (Some(&self.stores), None),
            (1, false) => (Some(&self.externals), Some(0)),
            (_, true) => (Some(&self.externals), None),
            (_, false) => (None, Some(1)),
        };
        let excluded = group.map_or(&[][..], |group| &group.only);
        let background = lists.map(|lists| Background { lists, excluded });
        if let Some(space) = named {
            for (other, group) in &self.fixed {
                if other.0 == space {
                    found.extend(group.only.iter().copied());
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        (found, background)
    }
}

/// A cell of a data segment or an external symbol at a fixed offset: no
/// register in its address. Its space and symbol.
pub(crate) fn _fixed_data(cell: &Mem) -> Option<(u8, i64)> {
    let addr = cell.addr?;
    let space = match addr.space {
        Space::Segment => 0,
        Space::External => 1,
        _ => return None,
    };
    (addr.base == (RegId::None).iced() && cell.base.is_none() && cell.index.is_none()).then_some((space, addr.index))
}

/// Whether the cell holds at every one of `uses` (block, position), given where
/// it is made to hold and where it is written: `events` per block are
/// (position, true for the load, false for a write) in order. The same answer
/// as `_unchanged`, found over the blocks that lie between the load and the
/// uses and not over the whole body.
pub(crate) fn _holds_at(
    flow: &Flow,
    events: &Events,
    uses: &[(usize, usize)],
) -> bool {
    use crate::support::hash::{HashMap, HashSet};
    // Whether the cell holds out of a block that has an event: after its last.
    let decided = |block: usize| events.before(block, None).map(|(_, load)| load);
    let mut needed: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::default();
    for &(block, position) in uses {
        if events.before(block, Some(position)).is_none() && seen.insert(block) {
            needed.push(block);
        }
    }
    // The blocks whose entry state the uses depend on, through blocks that have
    // no event of their own.
    let mut region = needed.clone();
    let mut next = 0;
    while next < region.len() {
        let block = region[next];
        next += 1;
        for &parent in &flow.parents[block] {
            if decided(parent).is_none() && seen.insert(parent) {
                region.push(parent);
            }
        }
    }
    // Held coming in: not at the entry, and the most held that the blocks
    // leading in allow.
    let mut held: HashMap<usize, bool> = region.iter().map(|&block| (block, block != flow.entry)).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for &block in &region {
            if block == flow.entry || flow.parents[block].is_empty() {
                continue;
            }
            let met = flow.parents[block].iter().all(|&parent| decided(parent).unwrap_or_else(|| held[&parent]));
            if met != held[&block] {
                held.insert(block, met);
                changing = true;
            }
        }
    }
    uses.iter().all(|&(block, position)| match events.before(block, Some(position)) {
        Some((_, load)) => load,
        None => held[&block],
    })
}

/// Whether `cell` still holds what it held at `define` after `one`.
/// Whether `cell`, as the load `define` read it, still holds after `one`, given
/// it held before.
pub(crate) fn _keeps(
    one: &Arc<Insn>,
    define: &Insn,
    cell: &Mem,
    holds: bool,
    body: &LirBody,
) -> bool {
    #[cfg(test)]
    KEEPS.with(|asked| asked.set(asked.get() + 1));
    if std::ptr::eq(Arc::as_ptr(one), define) {
        return true;
    }
    let (sealed, apart) = (body.sealed_arguments, body.spares.contains(&(define.at, one.at)));
    let written = _written(one, cell);
    // Sealed, an incoming argument cell is reached by the frame's own stores
    // alone.
    let meets = |dest: &Mem| {
        if sealed && _incoming_frame(cell) {
            _in_frame(dest) && crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width)
        } else {
            // The optimizer's proof of the pair answers a write whose address
            // LIR cannot place.
            // A register in a data address reaches the whole object, whatever
            // its `addr` says of the bytes it starts at (a frame address says
            // it by its `base`, in `overlap`).
            let reaches = (dest.base.is_some() || dest.index.is_some())
                && dest.addr.is_some_and(|addr| addr.space != Space::Frame);
            let overlaps =
                reaches || crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width);
            match (apart, cell.addr, dest.addr) {
                (false, _, _) => dest.addr.is_none() || overlaps,
                // What it proved is the MIR write's own: a far or unplaced
                // address is its to answer, a place of the cell's own space is
                // not.
                (true, Some(own), Some(there)) => own.space == there.space && overlaps,
                (true, _, _) => false,
            }
        }
    };
    holds && !(!apart && _may_write(one, cell, sealed)) && !written.clone().any(meets)
}

/// Whether allocated LIR names one fixed BP-relative frame range.
pub(crate) fn _exact_frame(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame)
        && cell.through == RegId::BP
        && cell.base.is_none()
        && cell.index.is_none()
}

/// Whether `cell` is a fixed incoming BP-relative word, not a local.
pub(crate) fn _incoming_frame(cell: &Mem) -> bool {
    _exact_frame(cell) && cell.addr.is_some_and(|addr| addr.disp >= 0)
}

/// Whether the MIR operation may change `cell`; in a `sealed` body an
/// incoming argument changes only where a frame store names it.
pub(crate) fn _may_write(
    one: &Insn,
    cell: &Mem,
    sealed: bool,
) -> bool {
    let call = one.call.as_deref();
    let written = _written(one, cell);
    if sealed && _incoming_frame(cell) {
        return written.clone().any(|dest| {
            _in_frame(dest) && crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width)
        });
    }
    if _exact_frame(cell) && written.clone().next().is_some() && written.clone().all(|dest| _exact_frame(dest)) {
        if written
            .clone()
            .any(|dest| crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width))
        {
            return true;
        }
        if call.is_none_or(|call| written.count() >= usize::from(call.writes())) {
            return false;
        }
    }
    if one.unmodeled_write() {
        return true;
    }
    call.is_some_and(|call| call.writes() && !(_in_frame(cell) && call.spares(_frame_disp(cell), cell.width)))
}

/// A fixed frame cell's displacement; none for one indexed or based.
pub(crate) fn _frame_disp(cell: &Mem) -> Option<i64> {
    _exact_frame(cell).then(|| cell.addr.expect("a frame cell").disp)
}

pub(crate) fn _in_frame(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame)
}

/// The memory this instruction names as written that could be `cell`.
pub(crate) fn _written<'a>(
    one: &'a Insn,
    cell: &Mem,
) -> impl Iterator<Item = &'a Mem> + Clone {
    let spared = _in_frame(cell) && one.call.as_ref().is_some_and(|call| call.writes() && call.spares_the_frame());
    one.what
        .iter()
        .flat_map(|what| &what.dests)
        .filter_map(
            move |dest| match dest {
                Loc::Mem(dest) if !(spared && dest.addr.is_some_and(|addr| addr.space != Space::Frame)) => Some(dest),
                _ => None,
            },
        )
}

/// The predecessors of every block, `at`s outside the body ignored.
pub(crate) fn _predecessors(body: &LirBody) -> IndexMap<i64, Vec<i64>> {
    let mut predecessors: IndexMap<i64, Vec<i64>> = body.blocks.iter().map(|block| (block.at, Vec::new())).collect();
    for block in &body.blocks {
        for at in &block.succ {
            if let Some(found) = predecessors.get_mut(at) {
                found.push(block.at);
            }
        }
    }
    predecessors
}

/// Whether every use of the loaded value sees the cell the load saw.
///
/// A block takes the cell in as it leaves its predecessors alike and, one
/// instruction after another, keeps it unless that instruction may write it:
/// `_keeps(one, ..., holds) = holds && K(one)` for every instruction but the
/// load, which makes it hold. So a block's answer is that of its instructions
/// after its last load, or of all of them: asked of each instruction once, not
/// once for each time the blocks are gone over until they settle.
pub(crate) fn _unchanged(
    body: &LirBody,
    flow: &Flow,
    define: &Arc<Insn>,
    cell: &Mem,
    uses: &[Arc<Insn>],
) -> bool {
    let (predecessors, blocks) = (&flow.predecessors, &flow.blocks);
    // Per block: whether the cell holds out of it whatever came in, and where
    // it is made to hold from.
    let mut gives: IndexMap<i64, (bool, bool)> = IndexMap::default();
    let mut kept: IndexMap<i64, Vec<bool>> = IndexMap::default();
    let wanted: BTreeSet<usize> = uses.iter().map(key).collect();
    for (at, block) in blocks {
        let mut after_load = None;
        let mut every = true;
        let mut sure = true;
        let mut per_insn: Vec<bool> = Vec::with_capacity(block.insns.len());
        for one in &block.insns {
            // Whether the instruction leaves a held cell held.
            let keeps = std::ptr::eq(Arc::as_ptr(one), Arc::as_ptr(define)) || _keeps(one, define, cell, true, body);
            per_insn.push(keeps);
            if std::ptr::eq(Arc::as_ptr(one), Arc::as_ptr(define)) {
                after_load = Some(true);
                sure = true;
            } else if !keeps {
                every = false;
                sure = false;
            }
        }
        // (holds out when nothing held coming in, holds out when it did)
        gives.insert(*at, (after_load.is_some() && sure, if after_load.is_some() { sure } else { every }));
        if block.insns.iter().any(|one| wanted.contains(&key(one))) {
            kept.insert(*at, per_insn);
        }
    }
    let mut into: IndexMap<i64, bool> = blocks.keys().map(|at| (*at, *at != body.entry)).collect();
    let mut outof: IndexMap<i64, bool> = IndexMap::default();
    let mut changing = true;
    while changing {
        changing = false;
        for at in blocks.keys() {
            let (empty, full) = gives[at];
            let holds = if into[at] { full } else { empty };
            if outof.get(at) != Some(&holds) {
                outof.insert(*at, holds);
                changing = true;
            }
        }
        for at in blocks.keys() {
            if *at == body.entry || predecessors[at].is_empty() {
                continue;
            }
            let met = predecessors[at].iter().all(|parent| outof.get(parent).copied().unwrap_or(true));
            if met != into[at] {
                into.insert(*at, met);
                changing = true;
            }
        }
    }
    for block in &body.blocks {
        let Some(per_insn) = kept.get(&block.at) else { continue };
        let mut holds = into[&block.at];
        for (position, one) in block.insns.iter().enumerate() {
            if wanted.contains(&key(one)) && !holds {
                return false;
            }
            holds =
                if std::ptr::eq(Arc::as_ptr(one), Arc::as_ptr(define)) { true } else { holds && per_insn[position] };
        }
    }
    true
}

/// `_unchanged` as it was written: the blocks gone over, each instruction asked
/// of again, until they settle.
pub(crate) fn _unchanged_reference(
    body: &LirBody,
    define: &Arc<Insn>,
    cell: &Mem,
    uses: &[Arc<Insn>],
) -> bool {
    let predecessors = _predecessors(body);
    let blocks: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut into: IndexMap<i64, bool> = blocks.keys().map(|at| (*at, *at != body.entry)).collect();
    let mut outof: IndexMap<i64, bool> = IndexMap::default();
    let mut changing = true;
    while changing {
        changing = false;
        for (at, block) in &blocks {
            let mut holds = into[at];
            for one in &block.insns {
                holds = _keeps(one, define, cell, holds, body);
            }
            if outof.get(at) != Some(&holds) {
                outof.insert(*at, holds);
                changing = true;
            }
        }
        for at in blocks.keys() {
            if *at == body.entry || predecessors[at].is_empty() {
                continue;
            }
            let met = predecessors[at].iter().all(|parent| outof.get(parent).copied().unwrap_or(true));
            if met != into[at] {
                into.insert(*at, met);
                changing = true;
            }
        }
    }

    let wanted: BTreeSet<usize> = uses.iter().map(key).collect();
    for block in &body.blocks {
        let mut holds = into[&block.at];
        for one in &block.insns {
            if wanted.contains(&key(one)) && !holds {
                return false;
            }
            holds = _keeps(one, define, cell, holds, body);
        }
    }
    true
}

/// The definition every one of `defining` repeats, if they all do the same.
/// A split remakes a value where each piece ends, so a remakeable value can
/// have several definitions that are one.
pub(crate) fn _one_definition(defining: &[Arc<Insn>]) -> Option<&Arc<Insn>> {
    let first = defining.first()?;
    let alike = |one: &Insn| {
        one.what == first.what
            && one.defines == first.defines
            && one.uses == first.uses
            && one.clobbers == first.clobbers
            && one.requires == first.requires
            && one.delivers == first.delivers
            && one.group == first.group
    };
    defining.iter().all(|one| alike(one)).then_some(first)
}

/// Literal values, including full-width copies with one unambiguous definition.
pub(crate) fn _constants(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Imm> {
    _literals(body, values, false)
}

pub(crate) fn _at<'b>(
    body: &'b LirBody,
    at: At,
) -> &'b Arc<Insn> {
    &body.blocks[at.0 as usize].insns[at.1 as usize]
}

/// `_constants`, and also those some instruction reads wider than the literal:
/// those are remade only where read at its width.
pub(crate) fn _literals(
    body: &LirBody,
    values: &BTreeSet<u32>,
    any_width: bool,
) -> IndexMap<u32, Imm> {
    let mut definitions: IndexMap<u32, Vec<Arc<Insn>>> = IndexMap::default();
    let mut excluded: BTreeSet<u32> = BTreeSet::new();
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in &one.defines {
            definitions.entry(*value).or_default().push(Arc::clone(one));
        }
        for value in &one.uses {
            let had = widths.get(value).copied().unwrap_or(0);
            widths.insert(*value, had.max(_width(one, *value)));
        }
        if one.group.is_some() {
            excluded.extend(one.defines.iter().copied());
            if group_source(one).is_none() {
                excluded.extend(one.uses.iter().copied());
            }
        }
    }
    // Python iterates `definitions.keys() - excluded`, a set; the fixed
    // point below reaches the same answer in any order.
    let mut sources: IndexMap<u32, Loc> = IndexMap::default();
    for (value, defining) in &definitions {
        if excluded.contains(value) {
            continue;
        }
        let Some(one) = _one_definition(defining) else {
            continue;
        };
        let Some(what) = &one.what else {
            continue;
        };
        if what.op != Operation::Move
            || what.dests.len() != 1
            || what.sources.len() != 1
            || one.defines != [*value]
            || !one.clobbers.is_empty()
        {
            continue;
        }
        let (into, source) = (&what.dests[0], &what.sources[0]);
        let Loc::Held(into) = into else {
            continue;
        };
        let (source_width, source_uses) = match source {
            Loc::Imm(imm) => (imm.width, Vec::new()),
            Loc::Held(held) => (held.width, vec![held.value]),
            _ => continue,
        };
        if into.width == source_width
            && one.uses == source_uses
            && (any_width || widths.get(value).copied().unwrap_or(0) <= source_width)
        {
            sources.insert(*value, source.clone());
        }
    }
    let mut result: IndexMap<u32, Imm> = IndexMap::default();
    loop {
        let before = result.len();
        for (value, source) in &sources {
            let constant = match source {
                Loc::Held(held) => result.get(&held.value).cloned(),
                Loc::Imm(imm) => Some(imm.clone()),
                _ => None,
            };
            if let Some(constant) = constant {
                result.insert(*value, constant);
            }
        }
        if result.len() == before {
            return result.into_iter().filter(|(value, _constant)| values.contains(value)).collect();
        }
    }
}

/// Pure addresses cheap enough to recreate at every use.
pub(crate) fn _addresses(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, AddressRef> {
    let mut definitions: IndexMap<u32, Vec<Arc<Insn>>> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in &one.defines {
            if values.contains(value) {
                definitions.entry(*value).or_default().push(Arc::clone(one));
            }
        }
    }

    let mut result = IndexMap::default();
    for (value, defining) in &definitions {
        if let Some(source) = _address_of(*value, defining) {
            result.insert(*value, source);
        }
    }
    result
}

/// The address `value` is made again from, where its definitions are one `lea`
/// of a frame cell or a symbol.
pub(crate) fn _address_of(
    value: u32,
    defining: &[Arc<Insn>],
) -> Option<AddressRef> {
    let one = _one_definition(defining)?;
    let what = one.what.as_ref()?;
    if what.op != Operation::Address || what.name.as_deref() != Some("lea") {
        return None;
    }
    // A cell's address is an address like any other once nothing held is in it.
    let source = match what.sources.as_slice() {
        [Loc::Address(source)] => source.clone(),
        [Loc::Mem(cell)] if cell.base.is_none() && cell.index.is_none() => AddressRef {
            addr: cell.addr,
            through: cell.through,
            index_through: RegId::None,
            scale: 1,
            offset: cell.offset,
            disp_width: cell.disp_width,
        },
        _ => return None,
    };
    let [Loc::Held(destination)] = what.dests.as_slice() else { return None };
    (destination.value == value
        && matches!(destination.width, 2 | 4)
        && one.defines == [value]
        && one.uses.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.group.is_none()
        && one.symbol != Some(true)
        && source.addr.is_some_and(|addr| {
            addr.space == Space::Frame && source.through == RegId::BP
                || matches!(addr.space, Space::Segment | Space::External) && source.through == RegId::None
        })
        && source.index_through == RegId::None)
        .then_some(source)
}

pub(crate) fn _widest(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, u32> {
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        let named: BTreeSet<u32> = one.defines.iter().chain(&one.uses).copied().collect();
        for value in values.intersection(&named) {
            let had = widths.get(value).copied().unwrap_or(0);
            widths.insert(*value, had.max(_width(one, *value)));
        }
    }
    widths
}

/// The instruction reading and writing the reload's value instead.
pub(crate) fn _renamed(
    one: &Arc<Insn>,
    rename: &IndexMap<u32, u32>,
) -> Arc<Insn> {
    one.renamed(&|value| rename.get(&value).copied().unwrap_or(value))
}

#[cfg(test)]
mod keeps_tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Addr, Held, Imm, Operation, Semantics};

    /// A store through a register to `_a` reaches all of `_a`, whatever byte
    /// its `addr` starts at: `_a[v+v] = 7` was taken to keep the cell `_a+2`,
    /// so a load of it was made again after the loop (loop_fusion -Omax read
    /// the value the loop had stored).
    #[test]
    fn test_a_store_through_a_register_does_not_keep_a_cell_of_its_symbol() {
        let symbol = |disp| Addr { index: 3, ..Addr::new(Space::Segment, disp) };
        let held = |value| Held { value, width: 4 };
        let cell = Mem::new(Some(symbol(2)), 2);
        let load = Arc::new(Insn::new(
            1,
            None,
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Held(Held { value: 9, width: 2 })],
                sources: vec![Loc::Mem(cell.clone())],
                ..Semantics::new(Operation::Move)
            }),
            vec![9],
            vec![],
        ));
        let through = Mem { base: Some(held(5)), index: Some(held(5)), ..Mem::new(Some(symbol(0)), 2) };
        let store = Arc::new(Insn::new(
            2,
            None,
            Some(Semantics {
                name: Some("mov".into()),
                dests: vec![Loc::Mem(through)],
                sources: vec![Loc::Imm(Imm { value: 7, width: 2, address: None })],
                ..Semantics::new(Operation::Move)
            }),
            vec![],
            vec![5, 5],
        ));
        let body = LirBody::new(
            "k",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(
                0,
                vec![Arc::clone(&load), Arc::clone(&store)],
            ))],
            Default::default(),
            Default::default(),
        );
        assert!(!_keeps(&store, &load, &cell, true, &body));
    }
}
