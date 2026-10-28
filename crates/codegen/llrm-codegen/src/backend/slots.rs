//! Lets two values share one frame slot when they are never live together, so
//! a frame holds the most that is live at once, not the sum of all spills.
//!
//! # How it works
//! A slot (`Color`) is its home, the capacity in bytes of the largest holder,
//! and the interval each holder occupies. `fits` accepts a new holder when the
//! slot is big enough and its interval overlaps no holder's. `choose` returns
//! the first slot listed in `partners` (homes the caller prefers) that fits,
//! else the first slot that fits, else none, and the caller opens a new slot.
//! `moved` rewrites a body's frame cells when slots are laid out again, each
//! cell keeping its place in its slot and taking the new slot's tag.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','textColor':'#64748b','titleColor':'#64748b','gridColor':'#94a3b8','sectionBkgColor':'transparent','sectionBkgColor2':'transparent','altSectionBkgColor':'transparent','taskTextColor':'#ffffff','taskTextOutsideColor':'#64748b','taskTextLightColor':'#ffffff','taskTextDarkColor':'#ffffff','doneTaskBkgColor':'#16a34a','doneTaskBorderColor':'#166534','activeTaskBkgColor':'#2563eb','activeTaskBorderColor':'#1e40af','critBkgColor':'#dc2626','critBorderColor':'#991b1b','taskBkgColor':'#475569','taskBorderColor':'#334155','todayLineColor':'transparent'},'gantt':{'barHeight':34,'barGap':12,'topPadding':48,'leftPadding':150,'fontSize':14,'sectionFontSize':14,'numberSectionStyles':2}}}%%
//! gantt
//!   dateFormat X
//!   axisFormat %s
//!   tickInterval 1second
//!   section slot A (4 bytes)
//!   x :done, t10, 1, 2
//!   y∶ shares A with x :done, t11, 4, 5
//!   section slot B (4 bytes)
//!   z overlaps both∶ own slot :crit, t12, 2, 4
//! ```
//!
//! *Slots shared by intervals that do not overlap*
//!
//! # Example
//! None: this is a helper that answers one question per call and has no output
//! of its own. Its effect shows in the spill slots of the `spiller` and
//! `ssaspill` examples.
//!
//! # Used by
//! - [`crate::backend::spiller`], which shares spill slots by the live range of
//!   each spilled value and prefers the slot of a related value, and
//!   `slots_colored`, which does so once over the allocation kept.
//! - [`crate::backend::isel`], which shares the slots of block locals by their
//!   lifetimes.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::{self as ranges, Interval, Segment};
use crate::backend::frame::{self as frames, Frame, SlotKey};
use crate::backend::postings;
use crate::greedy::tii::_with;
use crate::model::ir::{Loc, Operation, Space};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::{IndexMap, IndexSet};

/// A slot: its home, its capacity in bytes, and what each holder occupies.
pub type Color = (i64, u32, Vec<Interval>);

/// Whether `color` can also hold something of `capacity` bytes live over
/// `interval`.
pub fn fits(
    color: &Color,
    interval: &Interval,
    capacity: u32,
) -> bool {
    color.1 >= capacity && color.2.iter().all(|other| !interval.overlaps(other))
}

/// `IntervalSorter`: the homes heaviest first, the order they were made in
/// between equals (`stable_sort`).
pub(crate) fn heaviest_first(
    order: &[i64],
    weight: &IndexMap<i64, f64>,
) -> Vec<i64> {
    let of = |home: &i64| weight.get(home).copied().unwrap_or(0.0);
    let mut sorted = order.to_vec();
    sorted.sort_by(|a, b| of(b).partial_cmp(&of(a)).unwrap_or(std::cmp::Ordering::Equal));
    sorted
}

/// Whether nothing in `color` is live over `interval`
/// (`LiveIntervalUnion::overlaps` of `ColorSlot`): the slot grows to hold the
/// bigger.
pub fn overlaps_none(
    color: &Color,
    interval: &Interval,
) -> bool {
    color.2.iter().all(|other| !interval.overlaps(other))
}

/// The slot to put something in: one of the homes `partners` names that fits,
/// else the first that does; none where a new slot is needed.
pub fn choose(
    colors: &[Color],
    interval: &Interval,
    capacity: u32,
    partners: &[i64],
) -> Option<usize> {
    partners
        .iter()
        .find_map(|home| colors.iter().position(|one| one.0 == *home && fits(one, interval, capacity)))
        .or_else(|| colors.iter().position(|one| fits(one, interval, capacity)))
}

/// `body` with every frame cell of a slot in `moved` put in the slot it is
/// moved to (old first byte -> new first byte): the displacement keeps its
/// place in the slot, and the cell is tagged with the new slot. A cell that
/// names no moved slot is left as it is. None where a cell cannot be moved.
pub fn moved(
    body: &crate::model::lir::LirBody,
    moved: &crate::support::hash::IndexMap<i64, i64>,
) -> Option<crate::model::lir::LirBody> {
    use std::sync::Arc;

    use crate::model::ir::{Addr, AddressRef, Loc, Mem, Semantics, Space};
    use crate::model::lir::{DebugPlace, Insn, LirBlock};
    let addr = |addr: Addr, offset: i64| -> (Addr, i64) {
        let Some(home) = addr.slot_home().filter(|_| addr.space == Space::Frame) else { return (addr, offset) };
        let Some(&new) = moved.get(&home) else { return (addr, offset) };
        let disp = addr.disp - home + new;
        // `offset` repeats the displacement on a cell the spiller made from a
        // folded frame address.
        let offset = if addr.disp != 0 && offset == addr.disp { disp } else { offset };
        (Addr { disp, ..addr }.in_slot(new), offset)
    };
    let loc = |place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) => match cell.addr {
                Some(found) => {
                    let (found, offset) = addr(found, cell.offset);
                    Loc::Mem(Mem { addr: Some(found), offset, ..cell.clone() })
                }
                None => place.clone(),
            },
            Loc::Address(cell) => match cell.addr {
                Some(found) => {
                    let (found, offset) = addr(found, cell.offset);
                    Loc::Address(AddressRef { addr: Some(found), offset, ..cell.clone() })
                }
                None => place.clone(),
            },
            other => other.clone(),
        }
    };
    let touched = |place: &Loc| -> bool {
        let (Loc::Mem(Mem { addr: Some(found), .. }) | Loc::Address(AddressRef { addr: Some(found), .. })) = place
        else {
            return false;
        };
        found.space == Space::Frame && found.slot_home().is_some_and(|home| moved.contains_key(&home))
    };
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let insns: Vec<Arc<Insn>> = block
            .insns
            .iter()
            .map(|one| {
                let Some(what) = &one.what else { return Arc::clone(one) };
                // Most instructions name no cell of a moved home: they are kept
                // as they are, without a copy of their operands to compare.
                if !what.dests.iter().chain(&what.sources).any(&touched) {
                    return Arc::clone(one);
                }
                let (dests, sources): (Vec<Loc>, Vec<Loc>) =
                    (what.dests.iter().map(&loc).collect(), what.sources.iter().map(&loc).collect());
                if dests == what.dests && sources == what.sources {
                    return Arc::clone(one);
                }
                Arc::new(Insn { what: Some(Semantics { dests, sources, ..what.clone() }), ..(**one).clone() })
            })
            .collect();
        blocks.push(LirBlock { insns: insns.into(), ..block.clone().into_inner() }.into());
    }
    let mut out = body.with_blocks(blocks);
    out.homes = Arc::new(
        body.homes
            .iter()
            .map(|(value, cell)| {
                let cell = match cell.addr {
                    Some(found) => {
                        let (found, offset) = addr(found, cell.offset);
                        Mem { addr: Some(found), offset, ..cell.clone() }
                    }
                    None => cell.clone(),
                };
                (*value, cell)
            })
            .collect(),
    );
    out.variables = body
        .variables
        .iter()
        .map(|one| match &one.place {
            DebugPlace::At(found) if found.space == Space::Frame => {
                let (found, _) = addr(*found, 0);
                crate::model::lir::DebugVariable { place: DebugPlace::At(found), ..one.clone() }
            }
            _ => one.clone(),
        })
        .collect();
    Some(out)
}

/// `frame.WORD` at the width type this module uses.
pub(crate) const WORD: u32 = frames::WORD as u32;

/// The colours of the homes from their `LiveStacks` ranges, the allocator's
/// own: the ranges of what was spilled to each, not the liveness of the cell
/// worked out again. None where a home has no range (it was not made by an
/// inline spill).
pub(crate) fn _stacked_colors(frame: &Frame) -> Option<Vec<(i64, u32, Vec<Interval>)>> {
    // The scratch temporaries have no LiveStacks range: `slots_colored` works
    // theirs out from the body.
    let mut homes: Vec<i64> = frame
        .slots
        .values()
        .copied()
        .filter(|home| !frame.scratch.contains(home))
        .collect::<BTreeSet<i64>>()
        .into_iter()
        .collect();
    homes.sort_unstable();
    let first = u32::MAX - homes.len() as u32;
    homes
        .iter()
        .enumerate()
        .map(|(index, home)| {
            let segments = frame.stacks.get(home)?;
            let capacity = frame.capacities.get(home).map_or(WORD, |one| *one as u32).max(WORD);
            let segments = segments.iter().map(|(start, end)| Segment { start: *start, end: *end }).collect();
            Some((*home, capacity, vec![Interval::new(first + index as u32, segments)]))
        })
        .collect()
}

/// The spill slots of `body` shared by what is never live together, once, as
/// the allocation is kept: LLVM's `StackSlotColoring`, which runs after
/// register allocation over the slots `InlineSpiller` gave each spilled
/// register, and GCC's IRA slot reuse (`ira_reuse_stack_slot`), which colours
/// slots after the spill decisions. Colouring each slot as it was made cost
/// every spill the intervals of every home (`intervals homes`, 5% of a large
/// function's compile) for allocations that were then thrown away.
///
/// The homes are coloured most-named first, as LLVM colours by decreasing
/// weight, and in the order they were made too; the colouring with fewer bytes
/// is kept (the first where they tie). A home goes to the slot of a home it is
/// copied to or from, where that fits (`slots::choose`: the copy vanishes),
/// else the first that fits. The slots are laid out again, the most named where
/// a one-byte displacement reaches: from BP the top of the frame, the first
/// slot made, as 16-bit code reaches it; from SP the bottom, where 32-bit code,
/// which omits the frame register, reaches it. Every cell moves with its home.
/// The frame and the body are returned changed; a body with no two homes that
/// can share is returned as it is.
pub(crate) fn slots_colored(
    body: &LirBody,
    frame: &mut Frame,
) -> LirBody {
    if body.returns_twice || frame.native.is_some() || frame.slots.is_empty() {
        return body.clone();
    }
    if llrm_support::env_flag!("LLRM_STACKS_DEBUG") {
        let homes: std::collections::BTreeSet<i64> = frame.slots.values().copied().collect();
        let covered = homes.iter().filter(|home| frame.stacks.contains_key(*home)).count();
        eprintln!("stacks {}: {} homes, {} with a LiveStacks range", body.name, homes.len(), covered);
    }
    let colors = match _stacked_colors(frame) {
        Some(mut colors) if !llrm_support::env_flag!("LLRM_NO_STACKS") => {
            COLOURED.with(|one| one.set((one.get().0 + 1, one.get().1)));
            if !frame.scratch.is_empty() {
                let scratch = frame.scratch.clone();
                colors.extend(_existing_colors(body, frame).0.into_iter().filter(|color| scratch.contains(&color.0)));
            }
            colors
        }
        _ => {
            COLOURED.with(|one| one.set((one.get().0, one.get().1 + 1)));
            _existing_colors(body, frame).0
        }
    };
    let by_home: IndexMap<i64, &(i64, u32, Vec<Interval>)> = colors.iter().map(|color| (color.0, color)).collect();
    // The order the homes were made in.
    let mut order: Vec<i64> = Vec::new();
    for home in frame.slots.values() {
        if !order.contains(home) {
            order.push(*home);
        }
    }
    let mut sorted_homes: Vec<i64> = order.clone();
    sorted_homes.sort_unstable();
    // The home a cell lies in: a cell of the selector's slots lies in none.
    let home_of = |disp: i64| -> Option<i64> {
        let home = sorted_homes.partition_point(|home| *home <= disp).checked_sub(1).map(|at| sorted_homes[at])?;
        by_home.get(&home).filter(|color| disp < home + i64::from(color.1)).map(|_| home)
    };
    let mut home_uses: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        for one in block.insns.iter() {
            for (disp, _, _) in frame_cells(one) {
                if let Some(home) = home_of(disp) {
                    *home_uses.entry(home).or_default() += 1;
                }
            }
        }
    }
    // The homes a loop reads or writes: `loopslots` holds one in a register
    // through the loop unless some access is wider than a word.
    let depths = body.facts.0.get::<crate::analysis::graph::LoopDepths>(body);
    let mut in_loop: BTreeSet<i64> = BTreeSet::new();
    for block in &body.blocks {
        if depths.get(&block.at).copied().unwrap_or(0) == 0 {
            continue;
        }
        for one in block.insns.iter() {
            for (disp, _, _) in frame_cells(one) {
                if let Some(home) = home_of(disp) {
                    in_loop.insert(home);
                }
            }
        }
    }
    // `ScanForSpillSlotRefs`: a slot weighs the frequency of every instruction
    // that names it (`getSpillWeight(false, true, MBFI, MI)`).
    let frequencies = crate::liveness::blockfrequency::MachineBlockFrequencyInfo::of(body);
    let mut home_weight: IndexMap<i64, f64> = IndexMap::default();
    for (at, block) in body.blocks.iter().enumerate() {
        let frequency = frequencies.get_block_freq_relative_to_entry_block(at);
        for one in block.insns.iter() {
            for (disp, _, _) in frame_cells(one) {
                if let Some(home) = home_of(disp) {
                    *home_weight.entry(home).or_insert(0.0) += frequency;
                }
            }
        }
    }
    let colour = |processing: &[i64]| -> (Vec<(i64, u32, Vec<Interval>)>, IndexMap<i64, usize>) {
        let mut shared: Vec<(i64, u32, Vec<Interval>)> = Vec::new();
        let mut looped: Vec<bool> = Vec::new();
        let mut into: IndexMap<i64, usize> = IndexMap::default();
        for home in processing {
            let Some((_, capacity, occupants)) = by_home.get(home).copied() else { continue };
            let mut interval = Interval::new(0, Vec::new());
            interval.segments = occupants.iter().flat_map(|one| one.segments.iter().copied()).collect();
            interval.segments.sort_unstable_by_key(|segment| (segment.start, segment.end));
            // A loop's slot is shared only by what is as wide: a word shares no
            // slot a double word is read from, which no register stands in for
            // (`loopslots`).
            let same = |at: &usize| {
                frame.scratch.contains(&shared[*at].0) == frame.scratch.contains(home)
                    && (shared[*at].1 == *capacity || (!looped[*at] && !in_loop.contains(home)))
            };
            // `ColorSlot`: the first slot in use that nothing overlaps.
            let chosen =
                (0..shared.len()).filter(same).find(|at| crate::backend::slots::overlaps_none(&shared[*at], &interval));
            match chosen {
                Some(at) => {
                    shared[at].2.push(interval);
                    // The slot is as big as the biggest it holds.
                    shared[at].1 = shared[at].1.max(*capacity);
                    looped[at] |= in_loop.contains(home);
                    into.insert(*home, at);
                }
                None => {
                    shared.push((*home, *capacity, vec![interval]));
                    looped.push(in_loop.contains(home));
                    into.insert(*home, shared.len() - 1);
                }
            }
        }
        (shared, into)
    };
    let by_weight = heaviest_first(&order, &home_weight);
    let best = colour(&by_weight);
    let (shared, into) = best;
    if shared.len() == order.len() {
        return _identities_dropped(body);
    }
    let mut uses: Vec<usize> = vec![0; shared.len()];
    for (home, count) in &home_uses {
        if let Some(color) = into.get(home) {
            uses[*color] += count;
        }
    }
    let mut ranked: Vec<usize> = (0..shared.len()).collect();
    ranked.sort_by_key(|color| (std::cmp::Reverse(uses[*color]), *color));
    if body.bits >= 32 {
        ranked.reverse();
    }
    // The homes move: their ranges are of the old ones.
    let mut again = Frame {
        slots: IndexMap::default(),
        capacities: IndexMap::default(),
        stacks: IndexMap::default(),
        ..frame.clone()
    };
    let mut first_key: Vec<Option<SlotKey>> = vec![None; shared.len()];
    for (key, home) in &frame.slots {
        if let Some(color) = into.get(home) {
            first_key[*color].get_or_insert_with(|| key.clone());
        }
    }
    let mut placed: Vec<Option<i64>> = vec![None; shared.len()];
    for color in ranked {
        let Some(key) = first_key[color].clone() else { continue };
        let Ok(new) = again.slot(key, i64::from(shared[color].1)) else { return body.clone() };
        placed[color] = Some(new);
    }
    let mut moved: IndexMap<i64, i64> = IndexMap::default();
    for (key, home) in &frame.slots {
        let new = into.get(home).and_then(|color| placed[*color]).unwrap_or(*home);
        again.slots.insert(key.clone(), new);
        moved.insert(*home, new);
    }
    again.scratch = frame.scratch.iter().filter_map(|home| moved.get(home).copied()).collect();
    let Some(out) = crate::backend::slots::moved(body, &moved) else { return body.clone() };
    *frame = again;
    _identities_dropped(&out)
}

/// `body` with the spill slots no frame cell names any more taken out of
/// `frame`, and the rest laid out again in the order they were, nearest the
/// frame register first: a frame that shrinks where a cell was renamed away
/// (`slotcopies`). The frame and the body are returned changed; with every
/// slot still named, as they are.
pub(crate) fn released(
    body: &LirBody,
    frame: &mut Frame,
) -> LirBody {
    if body.returns_twice || frame.native.is_some() || frame.slots.is_empty() {
        return body.clone();
    }
    let mut named: Vec<i64> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .flat_map(|one| frame_cells(one).map(|(disp, _, _)| disp))
        .collect();
    named.sort_unstable();
    named.dedup();
    let mut homes: Vec<i64> = frame.slots.values().copied().collect::<BTreeSet<i64>>().into_iter().collect();
    homes.sort_unstable_by(|a, b| b.cmp(a));
    let capacity = |home: i64| frame.capacities.get(&home).copied().unwrap_or(i64::from(WORD)).max(i64::from(WORD));
    let live: Vec<i64> = homes
        .iter()
        .copied()
        .filter(|home| {
            named.get(named.partition_point(|disp| *disp < *home)).is_some_and(|disp| *disp < *home + capacity(*home))
        })
        .collect();
    if live.len() == homes.len() {
        return body.clone();
    }
    let mut again = Frame {
        slots: IndexMap::default(),
        capacities: IndexMap::default(),
        stacks: IndexMap::default(),
        ..frame.clone()
    };
    let mut moved: IndexMap<i64, i64> = IndexMap::default();
    for home in &live {
        let Some(key) = frame.slots.iter().find(|(_, at)| *at == home).map(|(key, _)| key.clone()) else { continue };
        let Ok(new) = again.slot(key, capacity(*home)) else { return body.clone() };
        moved.insert(*home, new);
    }
    for (key, home) in &frame.slots {
        if let Some(new) = moved.get(home) {
            again.slots.insert(key.clone(), *new);
        }
    }
    let Some(out) = crate::backend::slots::moved(body, &moved) else { return body.clone() };
    *frame = again;
    out
}

/// A plain move between a value and one frame cell: the value and the cell
/// (displacement, width).
pub(crate) fn _cell_move(
    one: &Insn,
    load: bool,
    floats: bool,
) -> Option<(u32, (i64, u32))> {
    let what = one.what.as_ref()?;
    // An x87 value read from or written to its cell is the same move.
    // `fld`/`fstp` only: `fistp` stores the value as an integer, not as it was.
    let float = floats
        && what.op == if load { Operation::FloatLoad } else { Operation::FloatStore }
        && matches!(what.name.as_deref(), Some("fld" | "fst" | "fstp"));
    if !float && (what.op != Operation::Move || what.name.as_deref() != Some("mov"))
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.clobbers.is_empty()
        || what.dests.len() != 1
        || what.sources.len() != 1
    {
        return None;
    }
    let (held, cell) = if load { (&what.dests[0], &what.sources[0]) } else { (&what.sources[0], &what.dests[0]) };
    let (Loc::Held(held), Loc::Mem(cell)) = (held, cell) else { return None };
    let addr = cell.addr.filter(|addr| addr.space == Space::Frame)?;
    // An element of an array is no slot: the index says which cell it is.
    if cell.index.is_some() || cell.index_through != RegId::None {
        return None;
    }
    (float || held.width == cell.width).then_some((held.value, (addr.disp, cell.width)))
}

/// The cells a value is reloaded from and stored back to in the next
/// instruction of `block`: a copy of one slot to another, through a value read
/// only there.
pub(crate) fn _slot_copies(body: &LirBody) -> Vec<((i64, u32), (i64, u32))> {
    let (mut defined, mut used): (IndexMap<u32, usize>, IndexMap<u32, usize>) = Default::default();
    for one in body.insns() {
        for value in &one.defines {
            *defined.entry(*value).or_default() += 1;
        }
        for value in &one.uses {
            *used.entry(*value).or_default() += 1;
        }
    }
    let mut found = Vec::new();
    for block in &body.blocks {
        for pair in block.insns.windows(2) {
            if let (Some((value, from)), Some((stored, to))) =
                (_cell_move(&pair[0], true, false), _cell_move(&pair[1], false, false))
            {
                if value == stored && defined.get(&value) == Some(&1) && used.get(&value) == Some(&1) {
                    found.push((from, to));
                }
            }
        }
    }
    found
}

/// `StackSlotColoring::RemoveDeadStores`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/StackSlotColoring.cpp#L482>):
/// `body` without the store that follows a load of the same value from the
/// same cell (a move between two spilled values that sharing slots put in one
/// cell, or a reload a hoisted spill stores back). The load goes too when the
/// store is the last read of its value.
///
/// DEVIATE D-SSC1: the store may follow copies of the loaded value. LLVM runs
/// this after the rewriter, which has dropped the copies whose two ends got one
/// register; the body here still has them. Exit: run after the rewrite.
pub(crate) fn _identities_dropped(body: &LirBody) -> LirBody {
    let candidates = body.blocks.iter().any(|block| block.insns.iter().any(|one| frame_cells(one).next().is_some()));
    if !candidates {
        return body.clone();
    }
    // How often each value is defined and read: asked only of a store that
    // follows its load at once, so counted when first asked.
    let counts = std::cell::OnceCell::new();
    let counted = || {
        counts.get_or_init(|| {
            let (mut defined, mut used): (IndexMap<u32, usize>, IndexMap<u32, usize>) = Default::default();
            for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
                for value in &one.defines {
                    *defined.entry(*value).or_default() += 1;
                }
                for value in &one.uses {
                    *used.entry(*value).or_default() += 1;
                }
            }
            (defined, used)
        })
    };
    let cell_move = |one: &Insn, load: bool| _cell_move(one, load, true);
    let mut changed = false;
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let mut gone = vec![false; block.insns.len()];
        for at in 0..block.insns.len() {
            let Some((value, cell)) = cell_move(&block.insns[at], true) else { continue };
            // The value and the copies of it before the store.
            let mut same = vec![value];
            let mut next = at + 1;
            while let Some(one) = block.insns.get(next) {
                match one.copy_of() {
                    Some((to, from)) if same.contains(&from) && one.group.is_none() => same.push(to),
                    _ => break,
                }
                next += 1;
            }
            let Some(store) = block.insns.get(next) else { continue };
            if !same.iter().any(|alias| cell_move(store, false) == Some((*alias, cell))) {
                continue;
            }
            changed = true;
            gone[next] = true;
            if next == at + 1 && counted().0.get(&value) == Some(&1) && counted().1.get(&value) == Some(&1) {
                gone[at] = true;
            }
        }
        let insns: Vec<Arc<Insn>> =
            block.insns.iter().zip(&gone).filter(|(_, gone)| !**gone).map(|(one, _)| Arc::clone(one)).collect();
        blocks.push(LirBlock { insns: insns.into(), ..block.clone().into_inner() }.into());
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// Spill-slot colors already present in the current rewritten body, and
/// every value's interval: one liveness pass answers both.
///
/// Python numbers each home's pseudo-value below every held value, which is
/// negative; `u32` cannot say that, so they are numbered in the same order
/// at the top of the range instead. Nothing compares them with a real value.
pub(crate) fn _existing_colors(
    body: &LirBody,
    frame: &mut Frame,
) -> (Vec<(i64, u32, Vec<Interval>)>, Lives) {
    let found = _existing_colors_by(body, frame, false);
    if llrm_support::env_flag!("LLRM_CHECK_COLORS") {
        let whole = _existing_colors_by(body, frame, true);
        // What is asked of a value is where it is live (`slots::fits` overlaps
        // segments of values with those of homes), so the homes and the values
        // are compared together, the weight not read: it counts a value an
        // instruction names twice once more than the body made of the homes
        // does.
        assert!(
            found.1.len() == whole.1.len()
                && found.1.iter().all(|(value, _)| whole.1.get(value).is_some())
                && _same_liveness(&found, &whole),
            "{}: the slot colors or where a value is live differ from working them out whole",
            body.name
        );
    }
    found
}

/// Whether two sets of slot colors, with the values' intervals beside them, say
/// the same of where each is live, whatever numbers their positions carry. A
/// body found by parts numbers its instructions as it remembers them and one
/// worked out whole as it lays them out, so the same liveness comes in other
/// numbers (27 of the 71 vsgcc programs at -O2, the first four segments of a
/// home six instructions apart). What the colors read of a position is where it
/// lies among the others, so the order of all the ends, the homes' and the
/// values' in one order, is compared.
pub(crate) fn _same_liveness(
    one: &(Vec<(i64, u32, Vec<Interval>)>, Lives),
    other: &(Vec<(i64, u32, Vec<Interval>)>, Lives),
) -> bool {
    let shape = |colors: &[(i64, u32, Vec<Interval>)], lives: &Lives| {
        let homes: Vec<(i64, u32, Vec<(u32, usize)>)> = colors
            .iter()
            .map(|(home, color, intervals)| {
                (*home, *color, intervals.iter().map(|one| (one.value, one.segments.len())).collect())
            })
            .collect();
        let values: std::collections::BTreeMap<u32, usize> =
            lives.iter().map(|(value, interval)| (*value, interval.segments.len())).collect();
        (homes, values)
    };
    // Every end of every segment, the homes' then the values' by value number,
    // as its rank among the distinct positions.
    let ranks = |colors: &[(i64, u32, Vec<Interval>)], lives: &Lives| -> Vec<usize> {
        let by_value: std::collections::BTreeMap<u32, &Interval> =
            lives.iter().map(|(value, interval)| (*value, interval)).collect();
        let ends: Vec<i64> = colors
            .iter()
            .flat_map(|c| &c.2)
            .chain(by_value.values().copied())
            .flat_map(|i| &i.segments)
            .flat_map(|s| [s.start, s.end])
            .collect();
        let mut distinct = ends.clone();
        distinct.sort_unstable();
        distinct.dedup();
        ends.iter().map(|end| distinct.binary_search(end).expect("an end is among them")).collect()
    };
    shape(&one.0, &one.1) == shape(&other.0, &other.1) && ranks(&one.0, &one.1) == ranks(&other.0, &other.1)
}

/// The intervals `_existing_colors` reads: the body's own, as remembered and
/// shared, and the homes' beside them.
pub(crate) struct Lives {
    pub(crate) shared: Option<Shared>,
    pub(crate) own: IndexMap<u32, Interval>,
}

/// The body's intervals: the allocator's, where it holds them for this body,
/// else worked out and remembered.
pub(crate) enum Shared {
    Held(std::sync::Arc<crate::backend::live::LiveRanges>),
    Worked(std::sync::Arc<IndexMap<u32, Interval>>),
}

impl Shared {
    pub(crate) fn of(
        body: &LirBody,
        index: Option<&ranges::Indexes>,
    ) -> Self {
        match crate::backend::live::held(body) {
            Some(live) => Self::Held(live),
            None => Self::Worked(ranges::intervals_shared(body, index)),
        }
    }

    pub(crate) fn get(
        &self,
        value: &u32,
    ) -> Option<&Interval> {
        match self {
            Self::Held(live) => live.get(value),
            Self::Worked(map) => map.get(value),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Held(live) => live.len(),
            Self::Worked(map) => map.len(),
        }
    }

    pub(crate) fn iter(&self) -> Box<dyn Iterator<Item = (&u32, &Interval)> + '_> {
        match self {
            Self::Held(live) => Box::new(live.iter()),
            Self::Worked(map) => Box::new(map.iter()),
        }
    }
}

impl Lives {
    pub(crate) fn get(
        &self,
        value: &u32,
    ) -> Option<&Interval> {
        self.own.get(value).or_else(|| self.shared.as_ref().and_then(|shared| shared.get(value)))
    }

    pub(crate) fn len(&self) -> usize {
        self.own.len() + self.shared.as_ref().map_or(0, |shared| shared.len())
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&u32, &Interval)> + '_ {
        self.own.iter().chain(self.shared.iter().flat_map(|shared| shared.iter()))
    }
}

/// The intervals of the homes' pseudo-values (`first..`) walked in a body of
/// the instructions that name them alone (whole parallel copies, which share a
/// point), at the slots they have in the body.
pub(crate) fn homes_by_sparse_body(
    body: &LirBody,
    index: &ranges::Indexes,
    changed: &[(usize, usize, Arc<Insn>)],
    first: u32,
) -> IndexMap<u32, Interval> {
    let sparse_blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| LirBlock {
            at: block.at,
            insns: Vec::new().into(),
            succ: block.succ.clone(),
            phis: block.phis.clone(),
            cold: block.cold,
            align: block.align,
            max_skip: block.max_skip,
        })
        .collect();
    let mut sparse_insns: Vec<Vec<Arc<Insn>>> = vec![Vec::new(); body.blocks.len()];
    let mut starts: Vec<Vec<i64>> = vec![Vec::new(); body.blocks.len()];
    let mut by_block: IndexMap<usize, Vec<(usize, Arc<Insn>)>> = IndexMap::default();
    for (block_index, at, made) in changed.iter().cloned() {
        by_block.entry(block_index).or_default().push((at, made));
    }
    for (block_index, mut named) in by_block {
        let original = &body.blocks[block_index].insns;
        // The runs of a parallel copy a named instruction is in, as the walk
        // takes them together.
        let mut wanted: std::collections::BTreeMap<usize, Arc<Insn>> = std::collections::BTreeMap::new();
        for (at, made) in named.drain(..) {
            if let Some(group) = original[at].group {
                let (mut low, mut high) = (at, at);
                while low > 0 && original[low - 1].group == Some(group) {
                    low -= 1;
                }
                while high + 1 < original.len() && original[high + 1].group == Some(group) {
                    high += 1;
                }
                for position in low..=high {
                    wanted.entry(position).or_insert_with(|| Arc::clone(&original[position]));
                }
            }
            wanted.insert(at, made);
        }
        // An instruction's slot, which the body's numbering gives its original;
        // `made` stands where that stood.
        let block = &body.blocks[block_index];
        let mut counted = 0;
        let mut before: Option<usize> = None;
        for (position, one) in wanted {
            // Two instructions of one parallel copy with others between them
            // are not one run here either.
            if let (Some(earlier), Some(group)) = (before, one.group) {
                if position > earlier + 1
                    && sparse_insns[block_index].last().is_some_and(|last| last.group == Some(group))
                {
                    starts[block_index].push(index.slot(block, counted));
                    sparse_insns[block_index].push(Arc::new(Insn::new(0, None, None, Vec::new(), Vec::new())));
                }
            }
            before = Some(position);
            counted = position;
            starts[block_index].push(index.slot(block, position));
            sparse_insns[block_index].push(one);
        }
    }
    let sparse = body.with_blocks(
        sparse_blocks.into_iter().zip(sparse_insns).map(|(block, insns)| block.with_insns(insns)).collect(),
    );
    ranges::intervals_sparse(&sparse, index, &starts, &|value| value >= first)
}

/// The same from where the homes occur: the instructions `changed` name them,
/// so the walk need not look at the others. What the last bodies the homes were
/// asked of left: their blocks, numbering, the homes' intervals (by the home's
/// frame offset) and which instruction names which homes.
#[derive(Default)]
pub(crate) struct HomesHeld(Vec<Arc<HomesState>>);

pub(crate) struct HomesState {
    pub(crate) structure: Arc<HomesStructure>,
    pub(crate) insns: Vec<crate::model::lir::Insns>,
    pub(crate) index: Arc<ranges::Indexes>,
    pub(crate) intervals: IndexMap<i64, Interval>,
    pub(crate) known: BTreeSet<i64>,
    pub(crate) count: usize,
}

/// An earlier body's homes are shifted only while the instructions that changed
/// are fewer than those that name a home: past that, looking at them costs more
/// than finding the homes again (cells N=448: 895 Minstr kept, 316 never kept;
/// d_faces the other way).
pub(crate) const FACTOR: usize = 1;

pub(crate) fn worth_keeping(
    blocks: usize,
    segments: usize,
) -> bool {
    const FIXED: usize = 60_000;
    const KEEP: usize = 40;
    const WALK: usize = 390;
    const SHIFT: usize = 136;
    FIXED + blocks * KEEP < segments * (WALK - SHIFT)
}

/// What of a body's shape the homes' intervals depend on besides its
/// instructions: shared by the states of bodies that keep it.
#[derive(PartialEq)]
pub(crate) struct HomesStructure {
    pub(crate) entry: i64,
    pub(crate) blocks: Vec<(i64, Vec<i64>, Vec<crate::model::lir::Phi>)>,
    pub(crate) odds: crate::model::lir::BlockOdds,
    pub(crate) trips: Vec<(i64, i64)>,
}

impl HomesStructure {
    pub(crate) fn is_of(
        &self,
        body: &LirBody,
    ) -> bool {
        body.facts.0.bump("homes-structures-compared");
        self.entry == body.entry
            && self.blocks.len() == body.blocks.len()
            && self
                .blocks
                .iter()
                .zip(&body.blocks)
                .all(|((at, succ, phis), block)| *at == block.at && *succ == block.succ && *phis == block.phis)
            && self.odds == body.odds
            && self.trips == body.loop_trip_counts
    }

    pub(crate) fn of(body: &LirBody) -> Self {
        Self {
            entry: body.entry,
            blocks: body.blocks.iter().map(|block| (block.at, block.succ.to_vec(), block.phis.to_vec())).collect(),
            odds: body.odds.clone(),
            trips: body.loop_trip_counts.clone(),
        }
    }
}

/// The frame cells `one` names: their displacement, width and whether it
/// writes them.
pub(crate) fn frame_cells(one: &Insn) -> impl Iterator<Item = (i64, u32, bool)> + '_ {
    let cell = |operand: &Loc, defines: bool| -> Option<(i64, u32, bool)> {
        let Loc::Mem(cell) = operand else { return None };
        let addr = cell.addr?;
        (addr.space == Space::Frame).then_some((addr.disp, cell.width, defines))
    };
    one.what
        .iter()
        .flat_map(
            move |what| what.dests
                .iter()
                .filter_map(move |operand| cell(operand, true))
                .chain(what.sources.iter().filter_map(move |operand| cell(operand, false))),
        )
}

/// What names a home: an instruction, by the frame cells it names.
pub(crate) enum Names<'a> {
    Insn(&'a Insn),
}

/// The homes an instruction names, by place among the homes, as many as it
/// has room for: more, and it is asked of its cells.
#[derive(Clone, Copy, Default)]
pub(crate) struct Places {
    pub(crate) count: u8,
    pub(crate) place: [u32; Places::ROOM],
    /// Bit `i` set: it writes the cell of `place[i]`.
    pub(crate) writes: u8,
}

impl Places {
    const ROOM: usize = 4;

    pub(crate) fn push(
        &mut self,
        place: usize,
        writes: bool,
    ) {
        if let Some(slot) = self.place.get_mut(usize::from(self.count)) {
            *slot = place as u32;
            self.writes |= u8::from(writes) << self.count;
        }
        self.count = self.count.saturating_add(1);
    }

    /// The values it defines and reads among the `asked` homes, ascending and
    /// once, in lists of the caller's emptied first; none where it names more
    /// than it has room for (`false`: ask its cells).
    pub(crate) fn sets_into(
        &self,
        first: u32,
        asked: &[bool],
        defined: &mut Vec<u32>,
        used: &mut Vec<u32>,
    ) -> bool {
        defined.clear();
        used.clear();
        if usize::from(self.count) > Self::ROOM {
            return false;
        }
        for at in 0..usize::from(self.count) {
            let place = self.place[at] as usize;
            if asked[place] {
                if self.writes >> at & 1 == 1 { &mut *defined } else { &mut *used }.push(first + place as u32);
            }
        }
        for list in [defined, used] {
            list.sort_unstable();
            list.dedup();
        }
        true
    }

    /// Whether it may name one of the `wanted` homes (by place); true where it
    /// names more than it has room for.
    pub(crate) fn may_name(
        &self,
        wanted: &[bool],
    ) -> bool {
        usize::from(self.count) > Self::ROOM
            || self.place[..usize::from(self.count)].iter().any(|place| wanted[*place as usize])
    }
}

/// An instruction that names a home.
pub(crate) struct Named<'a> {
    pub(crate) block: usize,
    pub(crate) at: usize,
    pub(crate) names: Names<'a>,
    pub(crate) places: Places,
}

impl Named<'_> {
    /// `sets`, ascending and once, in lists of the caller's that are emptied
    /// first.
    pub(crate) fn sets_into(
        &self,
        homes: &[i64],
        first: u32,
        wanted: &dyn Fn(i64) -> bool,
        defined: &mut Vec<u32>,
        used: &mut Vec<u32>,
    ) {
        defined.clear();
        used.clear();
        match &self.names {
            Names::Insn(one) => {
                for (disp, _, defines) in frame_cells(one) {
                    if !wanted(disp) {
                        continue;
                    }
                    if let Ok(place) = homes.binary_search(&disp) {
                        if defines { &mut *defined } else { &mut *used }.push(first + place as u32);
                    }
                }
            }
        }
        for list in [defined, used] {
            list.sort_unstable();
            list.dedup();
        }
    }

    /// The values it defines and reads among the homes `wanted` allows (the
    /// pseudo-value of a home is `first` and its place among `homes`).
    pub(crate) fn sets(
        &self,
        homes: &[i64],
        first: u32,
        wanted: &dyn Fn(i64) -> bool,
    ) -> (BTreeSet<u32>, BTreeSet<u32>) {
        let (mut defined, mut used) = (BTreeSet::new(), BTreeSet::new());
        match &self.names {
            Names::Insn(one) => {
                for (disp, _, defines) in frame_cells(one) {
                    if !wanted(disp) {
                        continue;
                    }
                    if let Ok(place) = homes.binary_search(&disp) {
                        if defines { &mut defined } else { &mut used }.insert(first + place as u32);
                    }
                }
            }
        }
        (defined, used)
    }
}

/// The homes' intervals: those of the homes the instructions that changed since
/// an earlier body do not name are that body's, shifted to the new numbering;
/// the others are found from where they occur. A spill changes a few blocks and
/// names one home or two; the intervals of the rest, across the whole body,
/// were found afresh for each (2.0 G of compiling d_faces, 616 times).
pub(crate) fn homes_kept(
    body: &LirBody,
    index: &Arc<ranges::Indexes>,
    named: &[Named<'_>],
    homes: &[i64],
    first: u32,
) -> IndexMap<u32, Interval> {
    let pseudo = |home: usize| first + home as u32;
    let fresh_for = |wanted: &[usize]| -> IndexMap<u32, Interval> {
        let values: Vec<u32> = wanted.iter().map(|home| pseudo(*home)).collect();
        // Only the instructions that name a wanted home, and only the wanted
        // among what they name: the others' intervals are not asked.
        let mut disps: Vec<i64> = wanted.iter().map(|home| homes[*home]).collect();
        disps.sort_unstable();
        let mut asked = vec![false; homes.len()];
        for home in wanted {
            asked[*home] = true;
        }
        let mut plan = crate::analysis::occurrences::Occurrences::default();
        let (mut defined, mut used): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
        for one in named.iter().filter(|one| one.places.may_name(&asked)) {
            if !one.places.sets_into(first, &asked, &mut defined, &mut used) {
                one.sets_into(homes, first, &|disp| disps.binary_search(&disp).is_ok(), &mut defined, &mut used);
            }
            if !defined.is_empty() || !used.is_empty() {
                plan.plan(one.block, one.at, &defined, &used);
            }
        }
        plan.ranges(body, index, &values)
    };
    // The state with the fewest blocks that differ from this body's, of those
    // whose structure is this body's: the blocks are told apart by identity,
    // and the structure, which is compared block by block, only for the states
    // taken in that order until one is.
    let kept = body
        .facts
        .0
        .stash(
            |held: &mut HomesHeld| {
                let mut by_change: Vec<(usize, &Arc<HomesState>)> = held
                    .0
                    .iter()
                    .map(|state| {
                        let differing = state
                            .insns
                            .iter()
                            .zip(&body.blocks)
                            .filter(|(insns, block)| !insns.same_insns(&block.insns))
                            .count();
                        (differing, state)
                    })
                    .collect();
                by_change.sort_by_key(|(differing, _)| *differing);
                by_change.into_iter().find(|(_, state)| state.structure.is_of(body)).map(|(_, state)| Arc::clone(state))
            },
        );
    let mut result: Option<IndexMap<u32, Interval>> = None;
    if let Some(state) = &kept {
        let blocks: Vec<(i64, &crate::model::lir::Insns)> =
            state.structure.blocks.iter().zip(&state.insns).map(|((at, _, _), insns)| (*at, insns)).collect();
        let differing = ranges::differing_blocks(&blocks, body);
        let mut touched: BTreeSet<i64> = BTreeSet::new();
        let changed = ranges::changed_runs(
            &blocks,
            &state.index,
            body,
            index,
            &differing,
            ((state.count + 16) / 4).min((named.len() + 16) / FACTOR),
            &mut |run| {
                // The homes it named then and the homes it names now.
                for one in run {
                    for (disp, _, _) in frame_cells(one) {
                        if state.known.contains(&disp) || homes.binary_search(&disp).is_ok() {
                            touched.insert(disp);
                        }
                    }
                }
            },
        );
        if changed * 4 <= state.count + 16 && changed * FACTOR <= named.len() + 16 {
            let shift = ranges::Shift::between(&blocks, &state.index, body, index, &differing);
            let redone: Vec<usize> = homes
                .iter()
                .enumerate()
                .filter(|(_, home)| touched.contains(home) || !state.known.contains(home))
                .map(|(at, _)| at)
                .collect();
            for _ in &redone {
                body.facts.0.bump("homes-redone");
            }
            let fresh = fresh_for(&redone);
            let mut out: IndexMap<u32, Interval> = IndexMap::default();
            for (at, home) in homes.iter().enumerate() {
                let value = pseudo(at);
                if redone.binary_search(&at).is_ok() {
                    if let Some(found) = fresh.get(&value) {
                        out.insert(value, found.clone());
                    }
                } else if let Some(old) = state.intervals.get(home) {
                    out.insert(
                        value,
                        Interval::new(
                            value,
                            old.segments
                                .iter()
                                .map(|segment| ranges::Segment {
                                    start: shift.start(segment.start),
                                    end: shift.end(segment.end),
                                })
                                .collect(),
                        ),
                    );
                }
            }
            result = Some(out);
        }
    }
    let result = result.unwrap_or_else(|| fresh_for(&(0..homes.len()).collect::<Vec<_>>()));
    if kept.is_some() && llrm_support::env_flag!("LLRM_CHECK_OCCURRENCES") {
        assert!(
            result == fresh_for(&(0..homes.len()).collect::<Vec<_>>()),
            "{}: the homes' intervals kept from an earlier body differ from finding them afresh",
            body.name
        );
    }
    // A state kept is one whose structure is this body's.
    let structure = match &kept {
        Some(state) => Arc::clone(&state.structure),
        None => Arc::new(HomesStructure::of(body)),
    };
    if !worth_keeping(body.blocks.len(), result.values().map(|found| found.segments.len()).sum()) {
        return result;
    }
    let state = HomesState {
        structure,
        insns: body.blocks.iter().map(|block| block.insns.clone()).collect(),
        index: Arc::clone(index),
        intervals: homes
            .iter()
            .enumerate()
            .filter_map(|(at, home)| result.get(&pseudo(at)).map(|found| (*home, found.clone())))
            .collect(),
        known: homes.iter().copied().collect(),
        count: body.blocks.iter().map(|block| block.insns.len()).sum(),
    };
    body.facts
        .0
        .stash(
            |held: &mut HomesHeld| {
                held.0.insert(0, Arc::new(state));
                held.0.truncate(3);
            },
        );
    result
}

pub(crate) fn _existing_colors_by(
    body: &LirBody,
    frame: &mut Frame,
    whole: bool,
) -> (Vec<(i64, u32, Vec<Interval>)>, Lives) {
    let mut homes: Vec<i64> = frame.slots.values().copied().collect::<BTreeSet<i64>>().into_iter().collect();
    homes.sort_unstable();
    if homes.is_empty() {
        return (Vec::new(), Lives { shared: Some(Shared::of(body, None)), own: IndexMap::default() });
    }
    let first = u32::MAX - homes.len() as u32;
    let pseudo: IndexMap<i64, u32> =
        homes.iter().enumerate().map(|(index, home)| (*home, first + index as u32)).collect();
    let mut capacities: IndexMap<i64, u32> =
        homes.iter().map(|home| (*home, frame.capacities.get(home).map_or(WORD, |one| *one as u32))).collect();
    let mut unknown = false;
    let (floor, hole) = (frame.floor, frame.hole);
    let frame_spills = |disp: i64| disp < floor || (-hole..0).contains(&disp);

    // Only an instruction that names a home is made again with it among its
    // values; the others are the body's. Only those with a frame cell among
    // their operands can, which are carried from the body this one was made
    // from.
    let mut flipped = false;
    // The instructions that name a home: made again with them among their
    // values only where the whole body is walked.
    let mut named: Vec<Named<'_>> = Vec::new();
    postings::following(body, |postings| {
        for (block_index, block) in body.blocks.iter().enumerate() {
            for at in postings.frames(block_index) {
                let one = &block.insns[at as usize];
                let mut names = false;
                let mut places = Places::default();
                for (disp, width, writes) in frame_cells(one) {
                    if let Some((place, _, had)) = capacities.get_full_mut(&disp) {
                        *had = (*had).max(width).max(WORD);
                        places.push(place, writes);
                        names = true;
                    } else if frame_spills(disp) {
                        unknown = true;
                    }
                }
                if !names && !whole {
                    continue;
                }
                // A value changes whether an instruction is a mark, and with it
                // the slots after it: it is one made of nothing.
                flipped |= names && one.is_meta();
                named.push(Named { block: block_index, at: at as usize, names: Names::Insn(one), places });
            }
        }
    });
    if llrm_support::env_flag!("LLRM_CHECK_NAMED") {
        // The body read whole, instruction by instruction.
        let mut walked: Vec<(usize, usize, BTreeSet<u32>, BTreeSet<u32>)> = Vec::new();
        let (mut walked_capacities, mut walked_unknown, mut walked_flipped) = (
            homes
                .iter()
                .map(|home| (*home, frame.capacities.get(home).map_or(WORD, |one| *one as u32)))
                .collect::<IndexMap<i64, u32>>(),
            false,
            false,
        );
        for (block_index, block) in body.blocks.iter().enumerate() {
            for (at, one) in block.insns.iter().enumerate() {
                let Some(what) = &one.what else { continue };
                let mut slot_of = |operand: &Loc| -> Option<u32> {
                    let Loc::Mem(cell) = operand else { return None };
                    let addr = cell.addr?;
                    if addr.space != Space::Frame {
                        return None;
                    }
                    if let Some(found) = pseudo.get(&addr.disp) {
                        let had = walked_capacities[&addr.disp];
                        walked_capacities.insert(addr.disp, had.max(cell.width).max(WORD));
                        return Some(*found);
                    }
                    if frame_spills(addr.disp) {
                        walked_unknown = true;
                    }
                    None
                };
                let framed = what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .any(
                        |operand| matches!(
                            operand,
                            Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                        ),
                    );
                if !framed {
                    continue;
                }
                let defined: BTreeSet<u32> = what.dests.iter().filter_map(&mut slot_of).collect();
                let used: BTreeSet<u32> = what.sources.iter().filter_map(&mut slot_of).collect();
                if defined.is_empty() && used.is_empty() && !whole {
                    continue;
                }
                walked_flipped |= (!defined.is_empty() || !used.is_empty()) && one.is_meta();
                walked.push((block_index, at, defined, used));
            }
        }
        let carried: Vec<(usize, usize, BTreeSet<u32>, BTreeSet<u32>)> = named
            .iter()
            .map(|one| {
                let (defined, used) = one.sets(&homes, first, &|_| true);
                (one.block, one.at, defined, used)
            })
            .collect();
        assert!(
            carried == walked
                && capacities == walked_capacities
                && unknown == walked_unknown
                && flipped == walked_flipped,
            "{}: the instructions that name a home, carried from the body this one was made from, differ from reading the body whole",
            body.name
        );
    }
    let changed_insns = || -> Vec<(usize, usize, Arc<Insn>)> {
        MADE.with(|count| count.set(count.get() + named.len()));
        named
            .iter()
            .map(|one| {
                let (block_index, at) = (&one.block, &one.at);
                let (defined, used) = one.sets(&homes, first, &|_| true);
                let one = &body.blocks[*block_index].insns[*at];
                let made = _with(one, |made| {
                    made.defines = one
                        .defines
                        .iter()
                        .copied()
                        .chain(defined.iter().copied())
                        .collect::<IndexSet<u32>>()
                        .into_iter()
                        .collect();
                    made.uses = one
                        .uses
                        .iter()
                        .copied()
                        .chain(used.iter().copied())
                        .collect::<IndexSet<u32>>()
                        .into_iter()
                        .collect();
                });
                (*block_index, *at, made)
            })
            .collect()
    };
    let shared;
    let owned;
    let index: &ranges::Indexes;
    let live;
    if whole || flipped || llrm_support::env_flag!("LLRM_CHECK_RANGES") {
        let mut tracked = body.clone();
        for (block_index, at, made) in changed_insns() {
            let mut insns = tracked.blocks[block_index].insns.to_vec();
            insns[at] = made;
            tracked.blocks[block_index].insns = insns.into();
        }
        owned = ranges::indexed(&tracked);
        index = &owned;
        let busy = crate::analysis::frequency::Frequency::of(&tracked);
        live = Lives {
            shared: None,
            own: if whole || flipped {
                ranges::intervals_over(&tracked, Some(index), &busy)
            } else {
                let mut live = ranges::intervals(body, None);
                // The homes' pseudo-values are the `first..` ids.
                live.extend(ranges::intervals_where(&tracked, index, &busy, &|value| value >= first));
                live
            },
        };
    } else {
        // The slots are the body's own, remembered; the homes' pseudo-values
        // are walked in a body of the instructions that name them alone
        // (whole parallel copies, which share a point), at the slots they have
        // in the body.
        // The numbering the allocator's intervals of this body are in, where it
        // holds them.
        shared = crate::backend::live::held(body)
            .map(|held| Arc::clone(held.index()))
            .unwrap_or_else(|| ranges::indexed_shared(body));
        index = &*shared;
        let homes_found =
            llrm_support::debug::timed("intervals homes", || homes_kept(body, &shared, &named, &homes, first));
        if llrm_support::env_flag!("LLRM_CHECK_OCCURRENCES") {
            let walked = homes_by_sparse_body(body, index, &changed_insns(), first);
            assert!(
                homes_found == walked,
                "{}: the homes' intervals by occurrences differ from the walk of the body of their instructions",
                body.name
            );
        }
        live = Lives { shared: Some(Shared::of(body, None)), own: homes_found };
    }
    let end = index.span.values().map(|(_first, last)| *last).max().unwrap_or(1);
    let mut colors = Vec::new();
    for home in homes {
        let interval = live.get(&pseudo[&home]);
        let mut occupants: Vec<Interval> = interval.into_iter().cloned().collect();
        if unknown {
            occupants = vec![Interval::new(pseudo[&home], vec![Segment { start: 0, end }])];
        }
        frame.capacities.insert(home, i64::from(capacities[&home]));
        colors.push((home, capacities[&home], occupants));
    }
    (colors, live)
}

// `_existing_colors`, with the intervals of the homes' pseudo-values found
// among themselves and the body's own remembered, or, `whole`, as the body
// with the homes in it is worked out at once.
thread_local! {
    pub(crate) static MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// (colourings from the allocator's `LiveStacks`, colourings that worked
    /// the homes' liveness out of the body) on this thread.
    pub(crate) static COLOURED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::Held;

    use super::{heaviest_first, slots_colored};
    use crate::backend::frame::Frame;
    use crate::model::ir::{Loc, Mem, Operation, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::objectfile::module::Addr;

    fn mov(
        at: i64,
        dest: Loc,
        source: Loc,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![dest],
            sources: vec![source],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    /// `fld dword [slot]; fistp dword [slot]` converts a float to an integer in
    /// place; dropping it as an identity made fpdeep print 1125122048, not 144.
    #[test]
    fn test_a_float_load_stored_back_as_an_integer_is_kept() {
        let mut frame = Frame::new(0);
        let at = frame.slot(1i64, 4).expect("slot");
        let cell = Loc::Mem(Mem {
            through: llrm_lir::registers::RegId::BP,
            disp_width: 4,
            ..Mem::new(Some(Addr::new(Space::Frame, at).in_slot(at)), 4)
        });
        let held = Loc::Held(Held { value: 7, width: 4 });
        let x87 = |at: i64,
                   op: Operation,
                   name: &str,
                   dests: Vec<Loc>,
                   sources: Vec<Loc>,
                   defines: Vec<u32>,
                   uses: Vec<u32>| {
            let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
        };
        let body = LirBody::new(
            "f",
            0,
            vec![
                LirBlock::new(
                    0,
                    vec![
                        x87(0, Operation::FloatLoad, "fld", vec![held.clone()], vec![cell.clone()], vec![7], vec![]),
                        x87(1, Operation::FloatStore, "fistp", vec![cell], vec![held], vec![], vec![7]),
                    ],
                )
                .into(),
            ],
            Default::default(),
            Default::default(),
        );
        let out = slots_colored(&body, &mut frame);
        assert_eq!(out.insns().len(), 2, "fistp converts, it is no store back");
    }

    /// The colouring tried four orders of the homes and kept the fewest bytes;
    /// the pin colours once, the heaviest slot first (`IntervalSorter`), the
    /// homes' own order between equals.
    #[test]
    fn test_slots_are_coloured_heaviest_first_and_in_their_own_order_between_equals() {
        let weight: crate::support::hash::IndexMap<i64, f64> =
            [(-4, 1.0), (-8, 32.0), (-12, 1.0), (-16, 32.0)].into_iter().collect();
        assert_eq!(heaviest_first(&[-4, -8, -12, -16], &weight), vec![-8, -16, -4, -12]);
    }

    /// x_ll_arith kept `ecx := [slot]; [slot] := ecx` when the colouring had
    /// no two homes to share: the early return skipped the identity drop.
    #[test]
    fn test_reload_stored_back_to_its_slot_is_dropped_when_no_homes_share() {
        let mut frame = Frame::new(0);
        let at = frame.slot(1i64, 4).expect("slot");
        let cell = Loc::Mem(Mem {
            through: llrm_lir::registers::RegId::BP,
            disp_width: 4,
            ..Mem::new(Some(Addr::new(Space::Frame, at).in_slot(at)), 4)
        });
        let held = Loc::Held(Held { value: 7, width: 4 });
        let body = LirBody::new(
            "f",
            0,
            vec![
                LirBlock::new(
                    0,
                    vec![mov(0, held.clone(), cell.clone(), vec![7], vec![]), mov(1, cell, held, vec![], vec![7])],
                )
                .into(),
            ],
            Default::default(),
            Default::default(),
        );
        let out = slots_colored(&body, &mut frame);
        assert_eq!(out.insns().len(), 0, "a reload stored straight back is a no-op");
    }
}
