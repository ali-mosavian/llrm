//! Port of `qbopt/backend/spiller.py`: a value the allocator would not keep,
//! kept in memory instead.
//!
//! LLVM's `InlineSpiller`: every definition of a spilled value becomes a
//! store into its frame slot, and every use a load into a fresh value that
//! lives only across that one instruction.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::effects;
use crate::analysis::intervals::{self as ranges, Interval, Segment, key};
use crate::analysis::regions;
use crate::backend::allocate::Error;
use crate::backend::coalesce;
use crate::backend::frame::{self as frames, Frame, SlotKey};
use crate::backend::target;
use crate::model::ir::{self, Addr, Address, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::memory::MemoryKind;
use crate::model::mir::{self, MemRef};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::pyset::PySet;

pub struct Spiller {
    pub spilled: BTreeSet<u32>,
    pub frame: Option<Frame>,
}

impl Spiller {
    pub const NAME: &'static str = "spill";

    pub fn new(spilled: BTreeSet<u32>, frame: Option<Frame>) -> Self {
        Self { spilled, frame }
    }
}

impl LIRTransform for Spiller {
    fn class_name(&self) -> &'static str {
        "Spiller"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    /// Python returns `spilled(...)`'s pair; the trait carries the body.
    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        spilled(&body, &self.spilled, self.frame.as_mut()).map(|(body, _made)| body).map_err(|error| error.to_string())
    }

    fn transform_raising(&mut self, body: LirBody) -> Result<LirBody, Exception> {
        spilled(&body, &self.spilled, self.frame.as_mut()).map(|(body, _made)| body).map_err(|error| error.raised())
    }
}


/// `frame.WORD` at the width type this module uses.
const WORD: u32 = frames::WORD as u32;
/// Python duck-types `frame.cell(value, width)` over a `Frame` and a
/// `_Cells`; this is that one method.
pub trait CellOf {
    fn cell_of(&mut self, value: u32, width: u32) -> Result<Option<Mem>, Error>;
}

impl CellOf for Frame {
    fn cell_of(&mut self, value: u32, width: u32) -> Result<Option<Mem>, Error> {
        Ok(Some(self.cell(value, width)?))
    }
}

fn _set(values: &[u32]) -> BTreeSet<u32> {
    values.iter().copied().collect()
}

fn _with<F: FnOnce(&mut Insn)>(one: &Insn, change: F) -> Arc<Insn> {
    let mut made = one.clone();
    change(&mut made);
    Arc::new(made)
}

/// `body` with each of `values` living in a frame slot, and the reloads.
pub fn spilled(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: Option<&mut Frame>,
) -> Result<(LirBody, BTreeSet<u32>), Error> {
    if values.is_empty() {
        return Ok((body.clone(), BTreeSet::new()));
    }
    let mut owned;
    let frame: &mut Frame = match frame {
        Some(frame) => frame,
        None => {
            owned = frames::of(body, None, "", None)?;
            &mut owned
        }
    };
    let mut fresh = _next_value(body);
    let mut made: BTreeSet<u32> = BTreeSet::new();
    let constants = _constants(body, values);
    let addresses = _addresses(body, values);
    let extensions = _extensions(body, values);
    let mut frame_loads = _stable_loads(body, values);
    frame_loads.extend(_frame_loads(body, values));
    let unloaded: BTreeSet<u32> = values.iter().copied().filter(|value| !frame_loads.contains_key(value)).collect();
    let frame_homes = _frame_homes(body, &unloaded);
    let mut rebuilt = frame_loads.clone();
    rebuilt.extend(frame_homes.iter().map(|(value, (home, _at))| (*value, home.clone())));
    let stored: BTreeSet<u32> = values
        .iter()
        .copied()
        .filter(|value| {
            !constants.contains_key(value)
                && !addresses.contains_key(value)
                && !extensions.contains_key(value)
                && !frame_loads.contains_key(value)
                && !frame_homes.contains_key(value)
        })
        .collect();
    // Before any cell names a slot.
    _color_slots(body, &stored, &_widest(body, &stored), frame)?;
    let (body, next, short) = _short_update_runs(body, &stored, frame, fresh)?;
    fresh = next;
    made.extend(short);
    let mut abandoned: BTreeSet<usize> = BTreeSet::new();
    let mut rematerialized_definitions: BTreeSet<usize> = BTreeSet::new();
    let mut identities: BTreeSet<usize> = BTreeSet::new();
    let r#final = _final_uses(&body);
    let rebuilt_values: BTreeSet<u32> = rebuilt.keys().copied().collect();
    let mut cells = _Cells::new(rebuilt.clone());

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for original in &block.insns {
            let mut one = Arc::clone(original);
            if _identity(&one, &stored, frame) {
                identities.insert(key(&one));
                insns.push(one);
                continue;
            }
            // A word index held only for one final memory access.
            if let Some((add, rewritten)) = _indexed_source(&one, &stored, frame, &r#final)? {
                insns.push(add);
                one = rewritten;
            }
            if let Some(source) = _group_source(&one) {
                if let Some(constant) = constants.get(&source.value) {
                    let what = one.what.clone().expect("a group source has semantics");
                    one = _with(&one, |made| {
                        made.what = Some(Semantics { sources: vec![Loc::Imm(constant.clone())], ..what });
                        made.uses = Vec::new();
                        made.symbol = Some(false);
                    });
                }
            }
            // A rebuilt value's cell reads as well as a slot does.
            if !rebuilt.is_empty() {
                if let Some(folded) = _source(&one, &rebuilt_values, &mut cells)? {
                    one = folded;
                }
            }
            // A pure frame address used only as a memory base.
            for value in one.uses.clone() {
                if let Some(address) = addresses.get(&value) {
                    if let Some(folded) = _address_source(&one, value, address) {
                        one = folded;
                    }
                }
            }
            let mut remade: IndexMap<u32, u32> = IndexMap::default();
            for value in one.uses.clone() {
                if (!constants.contains_key(&value)
                    && !addresses.contains_key(&value)
                    && !extensions.contains_key(&value)
                    && !frame_loads.contains_key(&value)
                    && !frame_homes.contains_key(&value))
                    || remade.contains_key(&value)
                    || frame_homes.get(&value).is_some_and(|home| key(&one) == home.1)
                {
                    continue;
                }
                remade.insert(value, fresh);
                let inserted = if let Some(constant) = constants.get(&value) {
                    _inserted(
                        &one,
                        _mov(Loc::Held(Held { value: fresh, width: constant.width }), Loc::Imm(constant.clone())),
                        vec![fresh],
                        Vec::new(),
                    )
                } else if let Some(address) = addresses.get(&value) {
                    _inserted(
                        &one,
                        Semantics {
                            name: Some("lea".to_owned()),
                            dests: vec![Loc::Held(Held { value: fresh, width: _width(&one, value) })],
                            sources: vec![Loc::Address(address.clone())],
                            ..Semantics::new(Operation::Address)
                        },
                        vec![fresh],
                        Vec::new(),
                    )
                } else if let Some(definition) = extensions.get(&value) {
                    let what = definition.what.as_ref().expect("an extension has semantics");
                    let (Loc::Held(destination), Loc::Held(source)) = (&what.dests[0], &what.sources[0]) else {
                        unreachable!("an extension is between two values")
                    };
                    _inserted(
                        &one,
                        Semantics { dests: vec![Loc::Held(Held { value: fresh, width: destination.width })], ..what.clone() },
                        vec![fresh],
                        vec![source.value],
                    )
                } else if let Some(cell) = frame_loads.get(&value) {
                    _reload(&one, fresh, cell)
                } else {
                    _reload(&one, fresh, &frame_homes[&value].0)
                };
                insns.push(_with(&inserted, |made| made.rematerialized = true));
                made.insert(fresh);
                fresh += 1;
            }
            if !remade.is_empty() {
                one = _renamed(&one, &remade);
            }
            if let Some(folded) = _source(&one, &stored, frame)? {
                one = folded;
            }
            let direct = match _in_place(&one, &stored, frame)? {
                Some(direct) => Some(direct),
                None => _tied(&one, &stored, frame)?,
            };
            if let Some(direct) = direct {
                insns.push(direct);
                continue;
            }
            if let Some((before, loaded)) = _memory_source_read_first(&one, &stored, fresh) {
                one = loaded;
                fresh += 1;
                insns.push(before);
                if let Some(direct) = _tied(&one, &stored, frame)? {
                    insns.push(direct);
                    continue;
                }
            }
            let mut before: Vec<Arc<Insn>> = Vec::new();
            let mut after: Vec<Arc<Insn>> = Vec::new();
            let mut rename: IndexMap<u32, u32> = IndexMap::default();
            for value in &one.uses {
                if !stored.contains(value) || rename.contains_key(value) {
                    continue;
                }
                rename.insert(*value, fresh);
                before.push(_reload(&one, fresh, &frame.cell(*value, _width(&one, *value))?));
                fresh += 1;
            }
            for value in &one.defines {
                if !stored.contains(value) {
                    continue;
                }
                if !rename.contains_key(value) {
                    rename.insert(*value, fresh);
                    fresh += 1;
                }
                if !constants.contains_key(value) {
                    after.push(_store(&one, rename[value], &frame.cell(*value, _width(&one, *value))?));
                }
            }
            insns.extend(before);
            let rewritten = if rename.is_empty() { Arc::clone(&one) } else { _renamed(&one, &rename) };
            insns.push(Arc::clone(&rewritten));
            if one.defines.iter().any(|value| frame_loads.contains_key(value)) {
                rematerialized_definitions.insert(key(&rewritten));
            }
            if one.defines.len() == 1
                && (constants.contains_key(&one.defines[0])
                    || addresses.contains_key(&one.defines[0])
                    || extensions.contains_key(&one.defines[0]))
                && !(!one.requires.is_empty() || !one.delivers.is_empty() || !one.clobbers.is_empty())
                && one.group.is_none()
                && one.symbol != Some(true)
            {
                abandoned.insert(key(&rewritten));
            }
            insns.extend(after);
            made.extend(rename.values().copied());
        }
        blocks.push(block.with_insns(insns));
    }
    let mut result = body.with_blocks(blocks);
    let never = None::<fn(&Arc<Insn>) -> Arc<Insn>>;
    if !rematerialized_definitions.is_empty() {
        result = result.with_blocks(result
                .blocks
                .iter()
                .map(|block| block.with_insns(lir::without(&block.insns, |one| rematerialized_definitions.contains(&key(one)), never)))
                .collect());
    }
    if !identities.is_empty() {
        result = result.with_blocks(result
                .blocks
                .iter()
                .map(|block| block.with_insns(lir::without(&block.insns, |one| identities.contains(&key(one)), never)))
                .collect());
    }
    let result = _remove_abandoned(&result, &abandoned);
    let surviving: BTreeSet<u32> =
        result.blocks.iter().flat_map(|block| &block.insns).flat_map(|one| one.defines.iter().copied()).collect();
    let made = made.intersection(&surviving).copied().collect();
    Ok((result, made))
}

/// Keep a just-defined spilled value in a register through one update.
///
/// Only a source that dies at the copy: the update now writes its register.
fn _short_update_runs(
    body: &LirBody,
    stored: &BTreeSet<u32>,
    frame: &mut Frame,
    fresh: u32,
) -> Result<(LirBody, u32, BTreeSet<u32>), Error> {
    let index = ranges::indexed(body);
    let live = ranges::intervals(body, Some(&index));
    let mut made: BTreeSet<u32> = BTreeSet::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        let mut position = 0;
        while position < block.insns.len() {
            let first = &block.insns[position];
            let second = block.insns.get(position + 1);
            let (Some(pair), Some(second)) = (_plain_move(first), second) else {
                insns.push(Arc::clone(first));
                position += 1;
                continue;
            };
            let (into, outof) = pair;
            let width = _width(first, into);
            let slot = index.at[&key(first)];
            let after = Segment { start: slot + ranges::DEF, end: slot + ranges::DEF + 1 };
            let eligible = stored.contains(&into)
                && !stored.contains(&outof)
                && !live[&outof].segments.iter().any(|segment| segment.overlaps(&after))
                && first.covers == Some((first.at, first.at))
                && second.group.is_none()
                && second.what.as_ref().is_some_and(|what| {
                    matches!(what.op, Operation::Binary | Operation::Unary)
                        && second.defines == [into]
                        && second.uses.contains(&into)
                        && second.requires.is_empty()
                        && second.delivers.is_empty()
                        && second.clobbers.is_empty()
                        && second.clobbers_high.is_empty()
                        && !what.dests.iter().chain(&what.sources).any(|operand| matches!(operand, Loc::Mem(_)))
                })
                && {
                    frame.cell(into, width)?;
                    true
                };
            if !eligible {
                insns.push(Arc::clone(first));
                position += 1;
                continue;
            }
            let renamed = IndexMap::from_iter([(into, outof)]);
            let updated = _renamed(second, &renamed);
            insns.push(updated);
            insns.push(_store(second, outof, &frame.cell(into, width)?));
            made.insert(outof);
            position += 2;
        }
        blocks.push(block.with_insns(insns));
    }
    Ok((body.with_blocks(blocks), fresh, made))
}

/// A move between two spilled values that share one slot.
fn _identity(one: &Insn, stored: &BTreeSet<u32>, frame: &Frame) -> bool {
    let Some(pair) = _plain_move(one) else {
        return false;
    };
    if !(stored.contains(&pair.0) && stored.contains(&pair.1)) || pair.0 == pair.1 {
        return false;
    }
    let slots = &frame.slots;
    slots.contains_key(&SlotKey::from(pair.0)) && slots.get(&SlotKey::from(pair.0)) == slots.get(&SlotKey::from(pair.1))
}

pub fn _plain_move(one: &Insn) -> Option<(u32, u32)> {
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

/// Values copied to and from a spilled one that are cheaper in its slot.
///
/// A group is a Python set whose iteration order decides which new keys
/// `frame.slots` gains first, so it is a `PySet`.
pub fn siblings(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: Option<&mut Frame>,
    fixed: &BTreeSet<u32>,
) -> Result<BTreeSet<u32>, Error> {
    let Some(frame) = frame else {
        return Ok(BTreeSet::new());
    };
    if values.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        let Some(pair) = _plain_move(one) else {
            continue;
        };
        if pair.0 == pair.1 {
            continue;
        }
        adjacent.entry(pair.0).or_default().insert(pair.1);
        adjacent.entry(pair.1).or_default().insert(pair.0);
    }
    if !values.iter().any(|one| adjacent.contains_key(one)) {
        return Ok(BTreeSet::new());
    }
    // A shared slot holds each member at every width it is used, not just moved.
    let widths = _widest(body, &adjacent.keys().copied().collect());

    let near = coalesce::_interference(body);
    let deep = ranges::depths(body);
    let wanted: BTreeSet<u32> = adjacent.keys().copied().collect();
    let mut occurs: IndexMap<u32, Vec<(f64, Arc<Insn>)>> = IndexMap::default();
    for block in &body.blocks {
        let each = ranges::level(deep.get(&block.at).copied().unwrap_or(0));
        for one in &block.insns {
            let named: BTreeSet<u32> = one.defines.iter().chain(&one.uses).copied().collect();
            for value in wanted.intersection(&named) {
                occurs.entry(*value).or_default().push((each, Arc::clone(one)));
            }
        }
    }

    let worth = |candidate: u32, group: &PySet<i64>| -> bool {
        let (mut saved, mut cost) = (0.0, 0.0);
        for (each, one) in occurs.get(&candidate).into_iter().flatten() {
            if let Some(pair) = _plain_move(one) {
                if [pair.0, pair.1]
                    .into_iter()
                    .filter(|one| *one != candidate)
                    .all(|one| group.contains(&i64::from(one)))
                {
                    saved += each;
                }
                continue;
            }
            cost += each;
        }
        saved > cost
    };

    let mut taken: BTreeSet<u32> = BTreeSet::new();
    let mut chosen: BTreeSet<u32> = BTreeSet::new();
    let firsts: Vec<u32> = values.iter().copied().filter(|one| adjacent.contains_key(one)).collect();
    for first in firsts {
        if taken.contains(&first) {
            continue;
        }
        let mut group: PySet<i64> = PySet::new();
        group.add(i64::from(first));
        let mut growing = true;
        while growing {
            growing = false;
            let members: BTreeSet<u32> = group.iter().map(|one| *one as u32).collect();
            let frontier: BTreeSet<u32> = members
                .iter()
                .flat_map(|one| adjacent.get(one).into_iter().flatten().copied())
                .filter(|one| !members.contains(one) && !taken.contains(one) && !fixed.contains(one))
                .collect();
            for candidate in frontier {
                if widths.get(&candidate) != widths.get(&first) {
                    continue;
                }
                if group.iter().any(|one| near.get(&candidate).is_some_and(|found| found.contains(&(*one as u32)))) {
                    continue;
                }
                if values.contains(&candidate) || worth(candidate, &group) {
                    group.add(i64::from(candidate));
                    growing = true;
                }
            }
        }
        let slots: BTreeSet<i64> =
            group.iter().filter_map(|one| frame.slots.get(&SlotKey::Value(*one)).copied()).collect();
        if group.len() < 2 || slots.len() > 1 {
            continue;
        }
        let home = match slots.first() {
            Some(home) => *home,
            None => frame.slot(first, widths[&first])?,
        };
        for one in group.iter() {
            frame.slots.insert(SlotKey::Value(*one), home);
        }
        for one in group.iter() {
            let one = *one as u32;
            taken.insert(one);
            if !values.contains(&one) {
                chosen.insert(one);
            }
        }
    }
    Ok(chosen)
}

/// Assign compatible noninterfering spill values to the same frame slot.
fn _color_slots(
    body: &LirBody,
    values: &BTreeSet<u32>,
    widths: &IndexMap<u32, u32>,
    frame: &mut Frame,
) -> Result<(), Error> {
    let live = ranges::intervals(body, None);
    let mut colors = _existing_colors(body, frame);
    let by_home: IndexMap<i64, usize> = colors.iter().enumerate().map(|(at, (home, _, _))| (*home, at)).collect();
    // `siblings()` may reserve one home for a copy web before this batch.
    for value in values {
        let home = frame.slots.get(&SlotKey::from(*value)).copied();
        let interval = live.get(value);
        let (Some(home), Some(interval)) = (home, interval) else {
            continue;
        };
        let at = by_home[&home];
        let capacity = colors[at].1;
        colors[at].2.push(interval.clone());
        let had = frame.capacities.get(&home).map_or(WORD, |one| *one as u32);
        frame.capacities.insert(home, i64::from(had.max(capacity)));
    }
    let mut pending: Vec<u32> =
        values.iter().copied().filter(|value| !frame.slots.contains_key(&SlotKey::from(*value))).collect();
    pending.sort_by_key(|value| (-i64::from(widths[value].max(WORD)), *value));
    for value in pending {
        let width = widths[&value];
        let capacity = width.max(WORD);
        let Some(interval) = live.get(&value) else {
            frame.slot(value, width)?;
            continue;
        };
        let color = colors
            .iter()
            .position(|one| one.1 >= capacity && one.2.iter().all(|other| !interval.overlaps(other)));
        let Some(color) = color else {
            let home = frame.slot(value, width)?;
            colors.push((home, capacity, vec![interval.clone()]));
            continue;
        };
        let home = colors[color].0;
        frame.slots.insert(SlotKey::from(value), home);
        let had = frame.capacities.get(&home).map_or(WORD, |one| *one as u32);
        frame.capacities.insert(home, i64::from(had.max(capacity)));
        colors[color].2.push(interval.clone());
    }
    Ok(())
}

/// Spill-slot colors already present in the current rewritten body.
///
/// Python numbers each home's pseudo-value below every held value, which is
/// negative; `u32` cannot say that, so they are numbered in the same order
/// at the top of the range instead. Nothing compares them with a real value.
fn _existing_colors(body: &LirBody, frame: &mut Frame) -> Vec<(i64, u32, Vec<Interval>)> {
    let mut homes: Vec<i64> = frame.slots.values().copied().collect::<BTreeSet<i64>>().into_iter().collect();
    homes.sort_unstable();
    if homes.is_empty() {
        return Vec::new();
    }
    let first = u32::MAX - homes.len() as u32;
    let pseudo: IndexMap<i64, u32> =
        homes.iter().enumerate().map(|(index, home)| (*home, first + index as u32)).collect();
    let mut capacities: IndexMap<i64, u32> = homes
        .iter()
        .map(|home| (*home, frame.capacities.get(home).map_or(WORD, |one| *one as u32)))
        .collect();
    let mut unknown = false;
    let floor = frame.floor;

    let mut slot = |operand: &Loc, capacities: &mut IndexMap<i64, u32>| -> Option<u32> {
        let Loc::Mem(cell) = operand else {
            return None;
        };
        let addr = cell.addr?;
        if addr.space != Space::Frame {
            return None;
        }
        let home = addr.disp;
        if let Some(found) = pseudo.get(&home) {
            let had = capacities[&home];
            capacities.insert(home, had.max(cell.width).max(WORD));
            return Some(*found);
        }
        // Anything below the floor is part of spill storage.
        if home < floor {
            unknown = true;
        }
        None
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let Some(what) = &one.what else {
                insns.push(Arc::clone(one));
                continue;
            };
            let defined: BTreeSet<u32> = what.dests.iter().filter_map(|operand| slot(operand, &mut capacities)).collect();
            let used: BTreeSet<u32> = what.sources.iter().filter_map(|operand| slot(operand, &mut capacities)).collect();
            insns.push(_with(one, |made| {
                made.defines = one.defines.iter().copied().chain(defined).collect::<IndexSet<u32>>().into_iter().collect();
                made.uses = one.uses.iter().copied().chain(used).collect::<IndexSet<u32>>().into_iter().collect();
            }));
        }
        blocks.push(block.with_insns(insns));
    }
    let tracked = body.with_blocks(blocks);
    let live = ranges::intervals(&tracked, None);
    let end = ranges::indexed(&tracked).span.values().map(|(_first, last)| *last).max().unwrap_or(1);
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
    colors
}

/// Spill candidates whose value can be reconstructed without a slot.
pub fn rematerializable(body: &LirBody, values: &BTreeSet<u32>) -> BTreeSet<u32> {
    let mut out: BTreeSet<u32> = _constants(body, values).keys().copied().collect();
    out.extend(_addresses(body, values).keys().copied());
    out.extend(_extensions(body, values).keys().copied());
    out.extend(frame_rematerializable(body, values));
    out.extend(_stable_loads(body, values).keys().copied());
    out.extend(_frame_homes(body, values).keys().copied());
    out
}

/// Frame-loaded selectors whose proof permits eager rematerialization.
pub fn frame_rematerializable(body: &LirBody, values: &BTreeSet<u32>) -> BTreeSet<u32> {
    _frame_loads(body, values).keys().copied().collect()
}

/// Values loaded from a cell nothing changes before they are used again.
pub fn _stable_loads(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, Mem> {
    if values.is_empty() {
        return IndexMap::default();
    }
    let mut definitions: IndexMap<u32, Vec<(Arc<Insn>, Option<Mem>)>> = IndexMap::default();
    let mut uses: IndexMap<u32, Vec<Arc<Insn>>> = values.iter().map(|value| (*value, Vec::new())).collect();
    for block in &body.blocks {
        for one in &block.insns {
            for value in values.intersection(&_set(&one.uses)) {
                uses[value].push(Arc::clone(one));
            }
            for value in values.intersection(&_set(&one.defines)) {
                let mut cell = None;
                if let Some(what) = &one.what {
                    if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                        if let ([Loc::Held(dest)], [Loc::Mem(source)]) = (what.dests.as_slice(), what.sources.as_slice()) {
                            if dest.value == *value
                                && dest.width == source.width
                                && source.addr.is_some()
                                && one.defines == [*value]
                                && one.uses.is_empty()
                                && one.clobbers.is_empty()
                                && one.group.is_none()
                                && one.spread.is_empty()
                                && one.symbol != Some(true)
                            {
                                cell = Some(source.clone());
                            }
                        }
                    }
                }
                definitions.entry(*value).or_default().push((Arc::clone(one), cell));
            }
        }
    }

    let mut result: IndexMap<u32, Mem> = IndexMap::default();
    for value in values {
        let Some(found) = definitions.get(value) else {
            continue;
        };
        if found.len() != 1 || uses[value].is_empty() {
            continue;
        }
        let (define, cell) = &found[0];
        let Some(cell) = cell else {
            continue;
        };
        if uses[value].iter().any(|one| one.group.is_some()) {
            continue;
        }
        if _unchanged(body, define, cell, &uses[value]) {
            result.insert(*value, cell.clone());
        }
    }
    result
}

/// `regions.addresses`. The Rust helper refuses a span Python's integers
/// can state and `i64` cannot; overlap is the conservative answer.
fn _addresses_meet(one: Option<Addr>, one_width: u32, other: Option<Addr>, other_width: u32) -> bool {
    regions::addresses(one, one_width, other, other_width, None).unwrap_or(true)
}

/// Whether `cell` still holds what it held at `define` after `one`.
fn _keeps(one: &Arc<Insn>, define: &Arc<Insn>, cell: &Mem, holds: bool) -> bool {
    if Arc::ptr_eq(one, define) {
        return true;
    }
    let written = _written(one, cell);
    holds
        && !_may_write(one, cell)
        && !written
            .iter()
            .any(|dest| dest.addr.is_none() || _addresses_meet(cell.addr, cell.width, dest.addr, dest.width))
}

/// Whether allocated LIR names one fixed BP-relative frame range.
fn _exact_frame(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame)
        && cell.through == Register::BP
        && cell.base.is_none()
        && cell.index.is_none()
}

/// Whether `cell` is a fixed incoming BP-relative word, not a local.
fn _incoming_frame(cell: &Mem) -> bool {
    _exact_frame(cell) && cell.addr.is_some_and(|addr| addr.disp >= 0)
}

/// Whether provenance confines an access to this activation's objects.
fn _proven_local_frame(reference: &MemRef) -> bool {
    reference.provenance.as_ref().is_some_and(|provenance| {
        !provenance.slices.is_empty() && provenance.slices.iter().all(|one| one.object.kind == MemoryKind::Frame)
    })
}

/// Whether the MIR operation may change `cell`.
fn _may_write(one: &Insn, cell: &Mem) -> bool {
    let op = one.op.as_deref();
    let written = _written(one, cell);
    if _exact_frame(cell) && !written.is_empty() && written.iter().all(|dest| _exact_frame(dest)) {
        if written.iter().any(|dest| _addresses_meet(cell.addr, cell.width, dest.addr, dest.width)) {
            return true;
        }
        if op.is_none_or(|op| written.len() >= op.stores.len()) {
            return false;
        }
    }
    let Some(op) = op else {
        return false;
    };
    if effects::unmodeled_write(op) {
        return true;
    }
    for reference in &op.stores {
        if _in_frame(cell) && reference.excludes.contains(&mir::WHOLE_FRAME) {
            continue;
        }
        if _incoming_frame(cell) && _proven_local_frame(reference) {
            continue;
        }
        if reference.addr.is_none() || _addresses_meet(cell.addr, cell.width, reference.addr, reference.width) {
            return true;
        }
    }
    false
}

fn _in_frame(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame)
}

/// The memory this instruction names as written that could be `cell`.
fn _written(one: &Insn, cell: &Mem) -> Vec<Mem> {
    let op = one.op.as_deref();
    let spared = _in_frame(cell)
        && op.is_some_and(|op| {
            !op.stores.is_empty() && op.stores.iter().all(|reference| reference.excludes.contains(&mir::WHOLE_FRAME))
        });
    let written: Vec<Mem> = one
        .what
        .iter()
        .flat_map(|what| &what.dests)
        .filter_map(|dest| match dest {
            Loc::Mem(dest)
                if !(spared && dest.addr.is_some_and(|addr| addr.space != Space::Frame)) =>
            {
                Some(dest.clone())
            }
            _ => None,
        })
        .collect();
    // A proven local object stays proven however selection spelled it.
    if _incoming_frame(cell)
        && op.is_some_and(|op| {
            !op.stores.is_empty() && written.len() <= op.stores.len() && op.stores.iter().all(_proven_local_frame)
        })
    {
        return Vec::new();
    }
    written
}

/// The predecessors of every block, `at`s outside the body ignored.
fn _predecessors(body: &LirBody) -> IndexMap<i64, Vec<i64>> {
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
fn _unchanged(body: &LirBody, define: &Arc<Insn>, cell: &Mem, uses: &[Arc<Insn>]) -> bool {
    let predecessors = _predecessors(body);
    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut into: IndexMap<i64, bool> = blocks.keys().map(|at| (*at, *at != body.entry)).collect();
    let mut outof: IndexMap<i64, bool> = IndexMap::default();
    let mut changing = true;
    while changing {
        changing = false;
        for (at, block) in &blocks {
            let mut holds = into[at];
            for one in &block.insns {
                holds = _keeps(one, define, cell, holds);
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
            holds = _keeps(one, define, cell, holds);
        }
    }
    true
}

/// Stable native arguments that may be loaded again at each use.
fn _frame_loads(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, Mem> {
    let pinned = &body.pins;
    let candidates: BTreeSet<u32> = values
        .iter()
        .copied()
        .filter(|value| pinned.get(value).is_some_and(|register| target::SEGMENTS.contains(register)))
        .collect();
    if candidates.is_empty() {
        return IndexMap::default();
    }

    let mut definitions: IndexMap<u32, (usize, usize, Arc<Insn>, Mem)> = IndexMap::default();
    let mut uses: IndexMap<u32, Vec<(usize, usize)>> = candidates.iter().map(|value| (*value, Vec::new())).collect();
    let mut excluded: BTreeSet<u32> = BTreeSet::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (insn_index, one) in block.insns.iter().enumerate() {
            for value in candidates.intersection(&_set(&one.uses)) {
                uses[value].push((block_index, insn_index));
            }
            for value in candidates.intersection(&_set(&one.defines)) {
                let mut stable = None;
                if let Some(what) = &one.what {
                    if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                        if let ([Loc::Held(dest)], [Loc::Mem(source)]) = (what.dests.as_slice(), what.sources.as_slice()) {
                            if dest.value == *value
                                && dest.width == source.width
                                && source.width == 2
                                && source.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp > 0)
                                && source.base.is_none()
                                && source.through == Register::BP
                                && one.defines == [*value]
                                && one.uses.is_empty()
                            {
                                stable = Some(source.clone());
                            }
                        }
                    }
                }
                match stable {
                    Some(source) if !definitions.contains_key(value) => {
                        definitions.insert(*value, (block_index, insn_index, Arc::clone(one), source));
                    }
                    _ => {
                        excluded.insert(*value);
                    }
                }
            }
        }
    }

    let mut result = IndexMap::default();
    for value in candidates.difference(&excluded) {
        let Some(definition) = definitions.get(value) else {
            continue;
        };
        let locations = &uses[value];
        if locations.is_empty() {
            continue;
        }
        let (block_index, defined_at, _one, source) = definition;
        if locations.iter().any(|(use_block, used_at)| use_block != block_index || used_at <= defined_at) {
            continue;
        }
        let last_use = locations.iter().map(|(_block, used_at)| *used_at).max().expect("not empty");
        let mut safe = true;
        for one in &body.blocks[*block_index].insns[defined_at + 1..=last_use] {
            if _may_write(one, source) {
                safe = false;
                break;
            }
        }
        if safe {
            result.insert(*value, source.clone());
        }
    }
    result
}

/// Existing stable frame stores which can hold a spilled SSA value.
fn _frame_homes(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, (Mem, usize)> {
    if values.is_empty() {
        return IndexMap::default();
    }
    let mut definitions: IndexMap<u32, Vec<(usize, usize)>> = values.iter().map(|value| (*value, Vec::new())).collect();
    let mut uses: IndexMap<u32, Vec<(usize, usize, Arc<Insn>)>> =
        values.iter().map(|value| (*value, Vec::new())).collect();
    let mut candidates: IndexMap<u32, Vec<(usize, usize, Arc<Insn>, Mem)>> =
        values.iter().map(|value| (*value, Vec::new())).collect();

    for (block_index, block) in body.blocks.iter().enumerate() {
        for (insn_index, one) in block.insns.iter().enumerate() {
            for value in values.intersection(&_set(&one.defines)) {
                definitions[value].push((block_index, insn_index));
            }
            for value in values.intersection(&_set(&one.uses)) {
                uses[value].push((block_index, insn_index, Arc::clone(one)));
            }
            let Some(what) = &one.what else {
                continue;
            };
            if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                if let ([Loc::Mem(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) {
                    let original = one.covers.is_some_and(|covers| covers.0 != covers.1);
                    if values.contains(&source.value)
                        && dest.width == source.width
                        && dest.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp < 0)
                        && dest.base.is_none()
                        && dest.through == Register::BP
                        && !dest.stack_argument
                        && original
                        && one.group.is_none()
                    {
                        candidates[&source.value].push((block_index, insn_index, Arc::clone(one), dest.clone()));
                    }
                }
            }
        }
    }

    let mut result = IndexMap::default();
    for value in values {
        let homes = &candidates[value];
        // The store reads the definition kept in a register.
        if homes.is_empty() || definitions[value].len() != 1 {
            continue;
        }
        let mut eligible: Vec<(Mem, usize)> = Vec::new();
        for (_block, _index, store, home) in homes {
            let later: Vec<&(usize, usize, Arc<Insn>)> =
                uses[value].iter().filter(|site| !Arc::ptr_eq(&site.2, store)).collect();
            if later.is_empty() || later.iter().any(|(_b, _i, one)| one.group.is_some()) {
                continue;
            }
            if _home_holds(body, *value, home, store) {
                eligible.push((home.clone(), key(store)));
            }
        }
        if eligible.len() == 1 {
            result.insert(*value, eligible.remove(0));
        }
    }
    result
}

/// Whether `home` still holds `value` after `one`.
///
/// Python's `cell is not home` asks whether a written cell is the store's
/// own operand object; only `store` holds it, and `store` returned above.
fn _holding(one: &Arc<Insn>, value: u32, home: &Mem, store: &Arc<Insn>, holds: bool) -> bool {
    if Arc::ptr_eq(one, store) {
        return true;
    }
    if one.defines.contains(&value) {
        return false;
    }
    let written = _written(one, home);
    holds
        && !_may_write(one, home)
        && !written
            .iter()
            .any(|cell| cell.addr.is_none() || _addresses_meet(home.addr, home.width, cell.addr, cell.width))
}

/// Whether `home` holds `value` at every use of it but the store itself.
fn _home_holds(body: &LirBody, value: u32, home: &Mem, store: &Arc<Insn>) -> bool {
    let predecessors = _predecessors(body);
    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    // A must-analysis: unreached is "holds".
    let mut into: IndexMap<i64, bool> = blocks.keys().map(|at| (*at, *at != body.entry)).collect();
    let mut outof: IndexMap<i64, bool> = IndexMap::default();
    let mut changing = true;
    while changing {
        changing = false;
        for (at, block) in &blocks {
            let mut holds = into[at];
            for one in &block.insns {
                holds = _holding(one, value, home, store, holds);
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

    for block in &body.blocks {
        let mut holds = into[&block.at];
        for one in &block.insns {
            if one.uses.contains(&value) && !Arc::ptr_eq(one, store) && !holds {
                return false;
            }
            holds = _holding(one, value, home, store, holds);
        }
    }
    true
}

pub fn _remove_abandoned(body: &LirBody, abandoned: &BTreeSet<usize>) -> LirBody {
    if abandoned.is_empty() {
        return body.clone();
    }
    let every = || body.blocks.iter().flat_map(|block| &block.insns);
    let mut used: BTreeSet<u32> = every().flat_map(|one| one.uses.iter().copied()).collect();
    used.extend(every().flat_map(|one| one.requires.iter().map(|(held, _)| held.value)));
    used.extend(body.blocks.iter().flat_map(LirBlock::arrives));
    used.extend(body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)));

    let removable =
        |one: &Arc<Insn>| abandoned.contains(&key(one)) && !one.defines.iter().any(|value| used.contains(value));

    let anchor = |one: Arc<Insn>| -> Arc<Insn> {
        if !removable(&one) {
            return one;
        }
        _with(&one, |made| {
            made.what = Some(Semantics { name: Some("nop".to_owned()), ..Semantics::new(Operation::Nothing) });
            made.defines = Vec::new();
            made.uses = Vec::new();
            made.widths = Vec::new();
        })
    };

    body.with_blocks(body
            .blocks
            .iter()
            .map(|block| block.with_insns(lir::without(&block.insns, removable, None::<fn(&Arc<Insn>) -> Arc<Insn>>)
                    .into_iter()
                    .map(anchor)
                    .collect()))
            .collect())
}

/// Where a rebuilt value already is, in the shape `_source` asks a frame.
pub struct _Cells {
    _cells: IndexMap<u32, Mem>,
}

impl _Cells {
    pub fn new(cells: IndexMap<u32, Mem>) -> Self {
        Self { _cells: cells }
    }

    pub fn cell(&self, value: u32, width: u32) -> Option<Mem> {
        self._cells.get(&value).filter(|found| found.width == width).cloned()
    }
}

impl CellOf for _Cells {
    fn cell_of(&mut self, value: u32, width: u32) -> Result<Option<Mem>, Error> {
        Ok(self.cell(value, width))
    }
}

/// Every `(id(insn), value)` where that instruction is the value's last read.
///
/// A destructive index fold needs the base dead after the access. Counting
/// static uses cannot say so: sum_three's loop-invariant base had one use,
/// inside the loop, and `add di,[slot]` moved it on every trip -- 330 for
/// 1110.
fn _final_uses(body: &LirBody) -> BTreeSet<(usize, u32)> {
    use crate::backend::allocate;

    let (_live_in, live_out) = allocate::live(body);
    let mut out: BTreeSet<(usize, u32)> = BTreeSet::new();
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> = live_out[&block.at].clone();
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let one = &block.insns[index as usize];
            let mut first = index as usize;
            if one.group.is_some() {
                while first > 0 && block.insns[first - 1].group == one.group {
                    first -= 1;
                }
            }
            let group = &block.insns[first..=index as usize];
            for item in group {
                for value in &item.defines {
                    alive.remove(value);
                }
            }
            for item in group {
                out.extend(item.uses.iter().filter(|value| !alive.contains(value)).map(|value| (key(item), *value)));
            }
            alive.extend(group.iter().flat_map(|item| item.uses.iter().copied()));
            index = first as i64 - 1;
        }
    }
    out
}

/// Spilled word indexes that can become a direct frame add.
pub fn foldable_indexes(body: &LirBody, values: &BTreeSet<u32>) -> BTreeSet<u32> {
    let r#final = _final_uses(body);
    body.blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter_map(|one| _indexed_pattern(one, values, &r#final))
        .map(|found| found.1.value)
        .collect()
}

/// Expose dying-base address adds so indexes may use any word register.
pub fn unfolded_indexes(body: &LirBody, values: &BTreeSet<u32>) -> (LirBody, BTreeSet<u32>) {
    if values.is_empty() {
        return (body.clone(), BTreeSet::new());
    }
    let r#final = _final_uses(body);
    let mut changed: BTreeSet<u32> = BTreeSet::new();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut rewritten: IndexMap<i64, Arc<Insn>> = IndexMap::default();
        let mut after: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
        let mut definitions: IndexMap<u32, i64> = IndexMap::default();
        for (position, one) in block.insns.iter().enumerate() {
            let position = position as i64;
            definitions.extend(one.defines.iter().map(|value| (*value, position)));
            let Some((base, index, _cells)) = _indexed_pattern(one, values, &r#final) else {
                continue;
            };
            let (add, access) = _unfolded_index(one, base, index);
            let mut placement = position - 1;
            let latest_definition = [base.value, index.value]
                .iter()
                .map(|value| definitions.get(value).copied().unwrap_or(position - 1))
                .max()
                .expect("two values");
            let crossed = &block.insns[(latest_definition + 1) as usize..position as usize];
            if latest_definition < position
                && _flags_overwritten(crossed)
                && !crossed.iter().any(|item| item.uses.contains(&base.value))
            {
                placement = latest_definition;
            }
            rewritten.insert(position, access);
            after.entry(placement).or_default().push(add);
            changed.insert(index.value);
        }
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        insns.extend(after.get(&-1).into_iter().flatten().cloned());
        for (position, one) in block.insns.iter().enumerate() {
            let position = position as i64;
            insns.push(rewritten.get(&position).cloned().unwrap_or_else(|| Arc::clone(one)));
            insns.extend(after.get(&position).into_iter().flatten().cloned());
        }
        blocks.push(block.with_insns(insns));
    }
    if changed.is_empty() {
        return (body.clone(), BTreeSet::new());
    }
    (body.with_blocks(blocks), changed)
}

/// Whether an early inserted add's flags die before anything can read them.
fn _flags_overwritten(crossed: &[Arc<Insn>]) -> bool {
    for one in crossed {
        if one.group.is_some()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
        {
            return false;
        }
        let Some(what) = &one.what else {
            continue;
        };
        if matches!(what.op, Operation::Branch | Operation::Jump | Operation::Call | Operation::Return) {
            return false;
        }
        if matches!(what.name.as_deref(), Some("adc" | "sbb" | "rcl" | "rcr")) {
            return false;
        }
        if matches!(what.name.as_deref(), Some("add" | "sub" | "and" | "or" | "xor" | "cmp" | "test")) {
            return true;
        }
    }
    false
}

/// The base, index and matching cells of one legal direct-index fold.
fn _indexed_pattern(
    one: &Arc<Insn>,
    values: &BTreeSet<u32>,
    r#final: &BTreeSet<(usize, u32)>,
) -> Option<(Held, Held, Vec<Mem>)> {
    let what = one.what.as_ref()?;
    if one.group.is_some() || !one.requires.is_empty() || !one.delivers.is_empty() || !one.clobbers.is_empty() {
        return None;
    }
    let cells: Vec<Mem> = what
        .dests
        .iter()
        .chain(&what.sources)
        .filter_map(|place| match place {
            Loc::Mem(cell)
                if cell.base.is_some_and(|base| base.width == 2)
                    && cell.index.is_some_and(|index| index.width == 2)
                    && cell.scale == 1 =>
            {
                Some(cell.clone())
            }
            _ => None,
        })
        .collect();
    let first = cells.first()?;
    let (base, index) = (first.base.expect("checked"), first.index.expect("checked"));
    if values.contains(&base.value) || !values.contains(&index.value) {
        return None;
    }
    if cells.iter().any(|cell| cell.base != Some(base) || cell.index != Some(index)) {
        return None;
    }
    let participants = BTreeSet::from([base.value, index.value]);
    for place in what.dests.iter().chain(&what.sources) {
        if let Loc::Held(held) = place {
            if participants.contains(&held.value) {
                return None;
            }
        }
        if let Loc::Mem(cell) = place {
            let held: BTreeSet<u32> =
                [cell.base, cell.index, cell.selector].into_iter().flatten().map(|value| value.value).collect();
            if !participants.is_disjoint(&held)
                && (cell.base != Some(base)
                    || cell.index != Some(index)
                    || cell.scale != 1
                    || cell.selector.is_some_and(|selector| participants.contains(&selector.value)))
            {
                return None;
            }
        }
    }
    if !r#final.contains(&(key(one), base.value)) {
        return None;
    }
    Some((base, index, cells))
}

/// One indexed cell as `add base,index` and a base-only cell.
fn _unfolded_index(one: &Insn, base: Held, index: Held) -> (Arc<Insn>, Arc<Insn>) {
    let rebased = |place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) if cell.base == Some(base) && cell.index == Some(index) => Loc::Mem(Mem {
                index: None,
                scale: 1,
                index_through: Register::None,
                ..cell.clone()
            }),
            _ => place.clone(),
        }
    };
    let what = one.what.as_ref().expect("a pattern has semantics");
    let rewritten = _with(one, |made| {
        made.what = Some(Semantics {
            dests: what.dests.iter().map(rebased).collect(),
            sources: what.sources.iter().map(rebased).collect(),
            ..what.clone()
        });
        made.uses = one.uses.iter().copied().filter(|value| *value != index.value).collect();
    });
    let add = _inserted(
        one,
        Semantics {
            name: Some("add".to_owned()),
            dests: vec![Loc::Held(base)],
            sources: vec![Loc::Held(base), Loc::Held(index)],
            ..Semantics::new(Operation::Binary)
        },
        vec![base.value],
        vec![base.value, index.value],
    );
    (add, rewritten)
}

/// Fold a spilled word index into a base the current access kills.
fn _indexed_source(
    one: &Arc<Insn>,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
    r#final: &BTreeSet<(usize, u32)>,
) -> Result<Option<(Arc<Insn>, Arc<Insn>)>, Error> {
    let Some((base, index, _cells)) = _indexed_pattern(one, values, r#final) else {
        return Ok(None);
    };
    let slot = frame.cell(index.value, index.width)?;
    let (add, rewritten) = _unfolded_index(one, base, index);
    let add = _with(&add, |made| {
        let what = made.what.clone().expect("an add has semantics");
        made.what = Some(Semantics { sources: vec![Loc::Held(base), Loc::Mem(slot)], ..what });
        made.uses = vec![base.value];
    });
    Ok(Some((add, rewritten)))
}

/// The spilled source arithmetic or a comparison reads as its memory operand, needing no reload.
pub fn folded_source(one: &Insn, values: &BTreeSet<u32>) -> Option<Held> {
    if one.group.is_some() || !one.requires.is_empty() || !one.delivers.is_empty() || !one.clobbers.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    let mut widths: &[u32] = &[2, 4];
    let (left, right) = match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Binary, name, [Loc::Held(dest)], [Loc::Held(left), Loc::Held(right)]) => {
            if !matches!(name, Some("add" | "sub" | "and" | "or" | "xor")) || dest != left {
                return None;
            }
            (*left, *right)
        }
        (Operation::Compare, Some("cmp"), [], [Loc::Held(left), Loc::Held(right)]) => (*left, *right),
        (Operation::Multiply, Some("imul"), [Loc::Held(dest)], [Loc::Held(left), Loc::Held(right)]) => {
            if dest != left {
                return None;
            }
            (*left, *right)
        }
        (Operation::Move, Some("mov"), [Loc::Held(left)], [Loc::Held(right)]) => {
            // A copy is the most useful direct reload.
            widths = &[1, 2, 4];
            (*left, *right)
        }
        _ => return None,
    };
    if !widths.contains(&left.width)
        || right.width != left.width
        || !values.contains(&right.value)
        || left.value == right.value
        || one.defines.contains(&right.value)
        || one.uses.iter().any(|value| values.contains(value) && *value != left.value && *value != right.value)
    {
        return None;
    }
    Some(right)
}

/// Fold one untied spill source into arithmetic or a comparison.
fn _source<F: CellOf>(one: &Insn, values: &BTreeSet<u32>, frame: &mut F) -> Result<Option<Arc<Insn>>, Error> {
    let Some(right) = folded_source(one, values) else {
        return Ok(None);
    };
    let what = one.what.as_ref().expect("a folded source has semantics");
    let left = what.sources[0].clone();
    let Some(cell) = frame.cell_of(right.value, right.width)? else {
        return Ok(None);
    };
    let sources = if what.op == Operation::Move { vec![Loc::Mem(cell)] } else { vec![left, Loc::Mem(cell)] };
    Ok(Some(_with(one, |made| {
        made.what = Some(Semantics { sources, ..what.clone() });
        made.symbol = Some(false);
        made.uses = one.uses.iter().copied().filter(|value| *value != right.value).collect();
    })))
}

/// Fold a rematerializable frame address into every cell that uses it.
fn _address_source(one: &Insn, value: u32, address: &Address) -> Option<Arc<Insn>> {
    let what = one.what.as_ref()?;
    let addr = address.addr?;
    if one.symbol == Some(true)
        || addr.space != Space::Frame
        || address.through != Register::BP
        || address.index != Register::None
        || address.scale != 1
        || one.requires.iter().chain(&one.delivers).any(|(held, _register)| held.value == value)
    {
        return None;
    }

    let mut changed = false;
    let mut invalid = false;

    let mut operand = |place: &Loc| -> Loc {
        let held: Vec<Held> = ir::values(place).into_iter().filter(|one| one.value == value).collect();
        if held.is_empty() {
            return place.clone();
        }
        let Loc::Mem(cell) = place else {
            invalid = true;
            return place.clone();
        };
        let fits = held.len() == 1
            && cell.base == Some(held[0])
            && held[0].width == 2
            && cell.selector.is_none()
            && cell.index.is_none()
            && cell.addr.is_some_and(|found| {
                found.space == Space::Literal
                    && found.index == 0
                    && matches!(found.segment, Register::None | Register::SS)
            });
        if !fits {
            invalid = true;
            return place.clone();
        }
        let displacement = addr.disp + cell.addr.expect("checked").disp;
        changed = true;
        Loc::Mem(Mem {
            addr: Some(Addr::new(Space::Frame, displacement)),
            through: Register::BP,
            offset: displacement,
            disp_width: 0,
            base: None,
            ..cell.clone()
        })
    };

    let dests: Vec<Loc> = what.dests.iter().map(&mut operand).collect();
    let sources: Vec<Loc> = what.sources.iter().map(&mut operand).collect();
    if invalid || !changed {
        return None;
    }
    Some(_with(one, |made| {
        made.what = Some(Semantics { dests, sources, ..what.clone() });
        made.uses = one.uses.iter().copied().filter(|found| *found != value).collect();
        made.symbol = Some(false);
    }))
}

/// An unconstrained full-width parallel copy can take a literal in place.
fn _group_source(one: &Insn) -> Option<Held> {
    if one.group.is_none() || !one.clobbers.is_empty() || !one.requires.is_empty() || !one.delivers.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
        if let ([Loc::Held(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) {
            if dest.width == source.width && one.uses == [source.value] && one.defines == [dest.value] {
                return Some(*source);
            }
        }
    }
    None
}

/// Literal values, including full-width copies with one unambiguous definition.
pub fn _constants(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, Imm> {
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
            if _group_source(one).is_none() {
                excluded.extend(one.uses.iter().copied());
            }
        }
    }
    // Python iterates `definitions.keys() - excluded`, a set; the fixed
    // point below reaches the same answer in any order.
    let mut sources: IndexMap<u32, Loc> = IndexMap::default();
    for (value, defining) in &definitions {
        if excluded.contains(value) || defining.len() != 1 {
            continue;
        }
        let one = &defining[0];
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
        if into.width == source_width && one.uses == source_uses && widths.get(value).copied().unwrap_or(0) <= source_width
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
fn _addresses(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, Address> {
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
        if defining.len() != 1 {
            continue;
        }
        let one = &defining[0];
        let Some(what) = &one.what else {
            continue;
        };
        if what.op != Operation::Address || what.name.as_deref() != Some("lea") {
            continue;
        }
        let ([Loc::Held(destination)], [Loc::Address(source)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
            continue;
        };
        if destination.value == *value
            && matches!(destination.width, 2 | 4)
            && one.defines == [*value]
            && one.uses.is_empty()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && one.group.is_none()
            && one.symbol != Some(true)
            && source.addr.is_some_and(|addr| {
                addr.space == Space::Frame && source.through == Register::BP
                    || matches!(addr.space, Space::Segment | Space::External) && source.through == Register::None
            })
            && source.index == Register::None
        {
            result.insert(*value, source.clone());
        }
    }
    result
}

/// One-use integer extensions that are cheaper to recreate than spill.
fn _extensions(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, Arc<Insn>> {
    let mut definitions: IndexMap<u32, Vec<(usize, usize, Arc<Insn>)>> = IndexMap::default();
    let mut uses: IndexMap<u32, Vec<(usize, usize, Arc<Insn>)>> =
        values.iter().map(|value| (*value, Vec::new())).collect();
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (insn_index, one) in block.insns.iter().enumerate() {
            for value in &one.defines {
                definitions.entry(*value).or_default().push((block_index, insn_index, Arc::clone(one)));
            }
            for value in values.intersection(&_set(&one.uses)) {
                uses[value].push((block_index, insn_index, Arc::clone(one)));
            }
        }
    }

    let mut result = IndexMap::default();
    let none = Vec::new();
    for value in values {
        let found = definitions.get(value).unwrap_or(&none);
        let consumers = uses.get(value).unwrap_or(&none);
        if found.len() != 1 || consumers.len() != 1 {
            continue;
        }
        let (block_index, insn_index, one) = &found[0];
        let (consumer_block, consumer_index, consumer) = &consumers[0];
        if consumer.group.is_some() || consumer_block != block_index || consumer_index <= insn_index {
            continue;
        }
        let Some(what) = &one.what else {
            continue;
        };
        if what.op != Operation::Extend || !matches!(what.name.as_deref(), Some("movsx" | "movzx")) {
            continue;
        }
        let ([Loc::Held(destination)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
            continue;
        };
        if destination.value == *value
            && destination.width > source.width
            && one.defines == [*value]
            && one.uses == [source.value]
            && !values.contains(&source.value)
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && one.clobbers_high.is_empty()
            && one.group.is_none()
            && one.spread.is_empty()
            && one.symbol != Some(true)
        {
            let source_definitions = definitions.get(&source.value).unwrap_or(&none);
            let unchanged = source_definitions.is_empty()
                || (source_definitions.len() == 1
                    && source_definitions[0].0 == *block_index
                    && source_definitions[0].1 < *insn_index);
            if unchanged {
                result.insert(*value, Arc::clone(one));
            }
        }
    }
    result
}

/// How wide each value is read or written anywhere, which is how big its slot has to be.
fn _widest(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, u32> {
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

/// How wide this instruction reads or writes the value, defaulting to a word.
pub fn _width(one: &Insn, value: u32) -> u32 {
    for (held, _register) in one.requires.iter().chain(&one.delivers) {
        if held.value == value {
            return held.width;
        }
    }
    for (named, width) in &one.widths {
        if *named == value {
            return *width;
        }
    }
    let Some(what) = &one.what else {
        return WORD;
    };
    // A cell's base and index too.
    for place in what.dests.iter().chain(&what.sources) {
        for held in ir::values(place) {
            if held.value == value {
                return held.width;
            }
        }
    }
    WORD
}

fn _mov(into: Loc, out_of: Loc) -> Semantics {
    Semantics { name: Some("mov".to_owned()), dests: vec![into], sources: vec![out_of], ..Semantics::new(Operation::Move) }
}

/// The load that puts a spilled value back for one instruction.
fn _reload(beside: &Insn, into: u32, cell: &Mem) -> Arc<Insn> {
    let inserted =
        _inserted(beside, _mov(Loc::Held(Held { value: into, width: cell.width }), Loc::Mem(cell.clone())), vec![into], Vec::new());
    _with(&inserted, |made| made.spill_reload = true)
}

/// The store that puts a spilled value away as soon as it is written.
fn _store(beside: &Insn, out_of: u32, cell: &Mem) -> Arc<Insn> {
    let inserted = _inserted(
        beside,
        _mov(Loc::Mem(cell.clone()), Loc::Held(Held { value: out_of, width: cell.width })),
        Vec::new(),
        vec![out_of],
    );
    _with(&inserted, |made| made.spill_store = true)
}

/// An instruction that stands beside another and claims none of its bytes.
fn _inserted(beside: &Insn, what: Semantics, defines: Vec<u32>, uses: Vec<u32>) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = Insn::new(beside.at, Some((at, at)), Some(what), defines, uses);
    made.op = beside.op.clone();
    Arc::new(made)
}

/// A requirement, naming whichever value now feeds the instruction.
fn _wants(side: &[(Held, Register)], rename: &IndexMap<u32, u32>) -> Vec<(Held, Register)> {
    side.iter()
        .map(|(held, register)| {
            (Held { value: rename.get(&held.value).copied().unwrap_or(held.value), width: held.width }, *register)
        })
        .collect()
}

/// The instruction reading and writing the reload's value instead.
pub fn _renamed(one: &Insn, rename: &IndexMap<u32, u32>) -> Arc<Insn> {
    let swap = |value: &u32| rename.get(value).copied().unwrap_or(*value);
    _with(one, |made| {
        if let Some(what) = &one.what {
            made.what = Some(Semantics {
                dests: what.dests.iter().map(|x| _settled(x, rename)).collect(),
                sources: what.sources.iter().map(|x| _settled(x, rename)).collect(),
                ..what.clone()
            });
        }
        made.defines = one.defines.iter().map(swap).collect();
        made.uses = one.uses.iter().map(swap).collect();
        made.requires = _wants(&one.requires, rename);
        made.delivers = _wants(&one.delivers, rename);
        made.widths = one.widths.iter().map(|(value, width)| (swap(value), *width)).collect();
    })
}

/// One operand with every value it names put through the rename.
fn _settled(place: &Loc, rename: &IndexMap<u32, u32>) -> Loc {
    ir::mapped(place, |one| Held { value: rename.get(&one.value).copied().unwrap_or(one.value), width: one.width })
}

/// A move in a parallel copy with both ends spilled. Retained for callers
/// reporting this legacy refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Simultaneous(pub String);

impl fmt::Display for Simultaneous {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Simultaneous {}

/// One move of a parallel copy, with its spilled end read or written where it lives.
fn _in_place(one: &Insn, values: &BTreeSet<u32>, frame: &mut Frame) -> Result<Option<Arc<Insn>>, Error> {
    let Some(what) = &one.what else {
        return Ok(None);
    };
    if one.group.is_none() || what.op != Operation::Move {
        return Ok(None);
    }
    if what.dests.len() != 1 || what.sources.len() != 1 {
        return Ok(None);
    }
    let into: Vec<u32> = one.defines.iter().copied().filter(|v| values.contains(v)).collect();
    let outof: Vec<u32> = one.uses.iter().copied().filter(|v| values.contains(v)).collect();
    if into.is_empty() && outof.is_empty() {
        return Ok(None);
    }
    if !into.is_empty() && !outof.is_empty() {
        let dest = frame.cell(into[0], _width(one, into[0]))?;
        let source = frame.cell(outof[0], _width(one, outof[0]))?;
        return Ok(Some(_with(one, |made| {
            made.what = Some(Semantics { dests: vec![Loc::Mem(dest)], sources: vec![Loc::Mem(source)], ..what.clone() });
            made.defines = one.defines.iter().copied().filter(|value| !into.contains(value)).collect();
            made.uses = one.uses.iter().copied().filter(|value| !outof.contains(value)).collect();
        })));
    }
    let value = if into.is_empty() { outof[0] } else { into[0] };
    let cell = frame.cell(value, _width(one, value))?;
    if !into.is_empty() {
        return Ok(Some(_with(one, |made| {
            made.what = Some(Semantics { dests: vec![Loc::Mem(cell)], ..what.clone() });
            made.defines = one.defines.iter().copied().filter(|v| *v != value).collect();
        })));
    }
    Ok(Some(_with(one, |made| {
        made.what = Some(Semantics { sources: vec![Loc::Mem(cell)], ..what.clone() });
        made.uses = one.uses.iter().copied().filter(|v| *v != value).collect();
    })))
}

/// The load, and the instruction reading the loaded value instead.
fn _memory_source_read_first(one: &Insn, values: &BTreeSet<u32>, fresh: u32) -> Option<(Arc<Insn>, Arc<Insn>)> {
    let what = one.what.as_ref()?;
    if one.group.is_some() {
        return None;
    }
    let tied: Vec<u32> =
        one.defines.iter().copied().filter(|value| values.contains(value) && one.uses.contains(value)).collect();
    if tied.len() != 1 {
        return None;
    }
    let cells: Vec<&Mem> = what
        .sources
        .iter()
        .filter_map(|x| match x {
            Loc::Mem(cell) => Some(cell),
            _ => None,
        })
        .collect();
    if cells.len() != 1 || what.dests.iter().any(|x| matches!(x, Loc::Mem(_))) {
        return None;
    }
    let cell = cells[0].clone();
    let held = Held { value: fresh, width: cell.width };
    let load = _inserted(
        one,
        _mov(Loc::Held(held), Loc::Mem(cell.clone())),
        vec![fresh],
        ir::values(&Loc::Mem(cell.clone())).iter().map(|place| place.value).collect(),
    );
    // The cell goes to the load, and its fixup with it.
    let load = _with(&load, |made| made.symbol = Some(true));
    let rewritten = _with(one, |made| {
        made.symbol = Some(false);
        made.what = Some(Semantics {
            sources: what
                .sources
                .iter()
                .map(|x| if matches!(x, Loc::Mem(_)) { Loc::Held(held) } else { x.clone() })
                .collect(),
            ..what.clone()
        });
        made.uses = one.uses.iter().copied().chain([fresh]).collect();
    });
    Some((load, rewritten))
}

/// A value an instruction both reads and writes, kept in its slot.
fn _tied(one: &Insn, values: &BTreeSet<u32>, frame: &mut Frame) -> Result<Option<Arc<Insn>>, Error> {
    let Some(what) = &one.what else {
        return Ok(None);
    };
    if one.group.is_some() {
        return Ok(None);
    }
    let tied: Vec<u32> = one.defines.iter().copied().filter(|v| values.contains(v) && one.uses.contains(v)).collect();
    if tied.len() != 1 {
        return Ok(None);
    }
    let value = tied[0];
    // A second source naming it still needs a distinct encoded operand.
    if what.sources.iter().skip(1).any(|arg| matches!(arg, Loc::Held(held) if held.value == value)) {
        return Ok(None);
    }
    // A fixed register is a fixed register: a slot is not one.
    for (place, _register) in target::requirements(what) {
        let side = if place.side == "dest" { &what.dests } else { &what.sources };
        if place.index < side.len() && matches!(&side[place.index], Loc::Held(held) if held.value == value) {
            return Ok(None);
        }
    }
    // One memory operand is all there is, so nothing else may want it.
    for operand in what.dests.iter().chain(&what.sources) {
        if matches!(operand, Loc::Mem(_)) {
            return Ok(None);
        }
        if let Loc::Held(held) = operand {
            if held.value != value && values.contains(&held.value) {
                return Ok(None);
            }
        }
    }
    let cell = frame.cell(value, _width(one, value))?;
    let swap = |x: &Loc| match x {
        Loc::Held(held) if held.value == value => Loc::Mem(cell.clone()),
        _ => x.clone(),
    };
    let made = Semantics {
        dests: what.dests.iter().map(swap).collect(),
        sources: what.sources.iter().map(swap).collect(),
        ..what.clone()
    };
    if !_encodable(&made)? {
        return Ok(None);
    }
    Ok(Some(_with(one, |insn| {
        insn.what = Some(made);
        insn.defines = one.defines.iter().copied().filter(|v| *v != value).collect();
        insn.uses = one.uses.iter().copied().filter(|v| *v != value).collect();
    })))
}

/// Whether this form exists, asked of the one place that knows.
fn _encodable(what: &Semantics) -> Result<bool, Error> {
    let mut taken: IndexMap<u32, Register> = IndexMap::default();
    let rows: IndexMap<u32, Vec<Register>> = [1_u32, 2, 4]
        .into_iter()
        .map(|width| {
            (
                width,
                target::AVAILABLE
                    .into_iter()
                    .filter(|one| target::WIDTHS.get(&target::named(*one, i64::from(width))) == Some(&i64::from(width)))
                    .collect(),
            )
        })
        .collect();

    let mut placed = |operand: &Loc| -> Loc {
        let Loc::Held(held) = operand else {
            return operand.clone();
        };
        if !taken.contains_key(&held.value) {
            let row = rows.get(&held.width).cloned().unwrap_or_default();
            if taken.len() >= row.len() {
                return operand.clone(); // refuses below, which is the safe answer
            }
            taken.insert(held.value, row[taken.len()]);
        }
        Loc::Reg(Reg { register: target::named(taken[&held.value], i64::from(held.width)), width: held.width })
    };

    let probe = Semantics {
        dests: what.dests.iter().map(&mut placed).collect(),
        sources: what.sources.iter().map(&mut placed).collect(),
        ..what.clone()
    };
    Ok(super::select::emit(&probe, 0, None, false, false, None).is_some())
}

/// One past the highest value id this body names.
pub fn _next_value(body: &LirBody) -> u32 {
    let mut seen: BTreeSet<u32> = BTreeSet::from([0]);
    for block in &body.blocks {
        seen.extend(block.arrives());
        for one in &block.insns {
            seen.extend(one.defines.iter().copied());
            seen.extend(one.uses.iter().copied());
        }
    }
    seen.last().copied().expect("seeded with zero") + 1
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_spiller.py`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{_color_slots, _constants, spilled};
    use crate::backend::frame::{Frame, SlotKey};
    use crate::backend::omfwrite;
    use crate::model::ir::{Addr, Address, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
    use crate::model::mir::{self, MemRef, OpCode};

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(value: i64, width: u32) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn set(values: &[u32]) -> BTreeSet<u32> {
        values.iter().copied().collect()
    }

    fn slot(value: u32) -> SlotKey {
        SlotKey::from(value)
    }

    /// `ir.Mem(addr, width, through, offset, disp_width)`.
    fn mem(addr: Addr, width: u32, through: Register, offset: i64, disp_width: u32) -> Mem {
        Mem { through, offset, disp_width, ..Mem::new(Some(addr), width) }
    }

    fn insn(at: i64, covers: (i64, i64), what: Semantics, defines: &[u32], uses: &[u32]) -> Insn {
        Insn::new(at, Some(covers), Some(what), defines.to_vec(), uses.to_vec())
    }

    fn _move(into: u32, out_of: u32, group: Option<i64>, at: i64) -> Insn {
        let what = semantics(Operation::Move, "mov", vec![held(into, 2)], vec![held(out_of, 2)]);
        Insn { group, ..insn(at, (at, at), what, &[into], &[out_of]) }
    }

    fn _add(into: u32, out_of: u32, at: i64) -> Insn {
        let what = semantics(Operation::Binary, "add", vec![held(into, 2)], vec![held(into, 2), held(out_of, 2)]);
        insn(at, (at, at), what, &[into], &[into, out_of])
    }

    fn _body(insns: Vec<Insn>) -> LirBody {
        LirBody::new(
            "one",
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect())],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    fn _out(body: &LirBody, values: &[u32]) -> Vec<Arc<Insn>> {
        let (got, _made) = spilled(body, &set(values), Some(&mut Frame::new(0))).expect("spills");
        got.insns()
    }

    fn what(one: &Insn) -> &Semantics {
        one.what.as_ref().expect("semantics")
    }

    fn name(one: &Insn) -> Option<&str> {
        one.what.as_ref().and_then(|what| what.name.as_deref())
    }

    fn is_mem(place: &Loc) -> bool {
        matches!(place, Loc::Mem(_))
    }

    fn as_mem(place: &Loc) -> &Mem {
        let Loc::Mem(cell) = place else { panic!("not memory: {place:?}") };
        cell
    }

    fn as_held(place: &Loc) -> Held {
        let Loc::Held(one) = place else { panic!("not held: {place:?}") };
        *one
    }

    fn operands(what: &Semantics) -> impl Iterator<Item = &Loc> {
        what.dests.iter().chain(&what.sources)
    }

    fn index_of(insns: &[Arc<Insn>], one: &Arc<Insn>) -> usize {
        insns.iter().position(|other| Arc::ptr_eq(other, one)).expect("present")
    }

    #[test]
    fn test_a_spilled_move_source_loads_straight_into_its_short_successor() {
        let insns = _out(&_body(vec![_move(1, 3, None, 0x10), _move(2, 1, None, 0x11)]), &[1]);
        let copied = insns.iter().find(|one| one.what.as_ref().is_some_and(|w| w.dests == [held(2, 2)])).unwrap();
        assert!(is_mem(&what(copied).sources[0]));
        assert!(copied.uses.is_empty());
    }

    #[test]
    fn test_a_spilled_byte_move_loads_straight_into_its_constrained_child() {
        let made =
            insn(0x10, (0x10, 0x10), semantics(Operation::Move, "mov", vec![held(1, 1)], vec![held(3, 1)]), &[1], &[3]);
        let child =
            insn(0x11, (0x11, 0x11), semantics(Operation::Move, "mov", vec![held(2, 1)], vec![held(1, 1)]), &[2], &[1]);
        let insns = _out(&_body(vec![made, child]), &[1]);
        let copied = insns.iter().find(|one| one.what.as_ref().is_some_and(|w| w.dests == [held(2, 1)])).unwrap();
        assert!(is_mem(&what(copied).sources[0]));
        assert_eq!(as_mem(&what(copied).sources[0]).width, 1);
        assert!(copied.uses.is_empty());
    }

    #[test]
    fn test_a_short_update_is_completed_before_its_result_is_spilled() {
        let copied = _move(2, 1, None, 0x10);
        let shifted = insn(
            0x11,
            (0x11, 0x11),
            semantics(Operation::Binary, "shl", vec![held(2, 2)], vec![held(2, 2), imm(1, 1)]),
            &[2],
            &[2],
        );
        let insns = _out(&_body(vec![copied, shifted]), &[2]);
        let update = insns.iter().find(|one| name(one) == Some("shl")).unwrap();
        let spill = insns.iter().find(|one| one.spill_store).unwrap();
        assert!(matches!(what(update).dests[0], Loc::Held(_)));
        assert!(matches!(what(update).sources[0], Loc::Held(_)));
        assert!(index_of(&insns, spill) > index_of(&insns, update));
    }

    /// nbody's inner loop base `final = start + distance` overwrote `start`.
    ///
    /// The update ran in the copy source's register: `add ax, bx` for
    /// `mov final, start; add final, bx` with `start` still read by the outer
    /// latch, which then advanced the wrong value and the answer drifted.
    #[test]
    fn test_a_short_update_leaves_a_copy_source_that_is_still_live() {
        let copied = _move(2, 1, None, 0x10);
        let shifted = insn(
            0x11,
            (0x11, 0x11),
            semantics(Operation::Binary, "shl", vec![held(2, 2)], vec![held(2, 2), imm(1, 1)]),
            &[2],
            &[2],
        );
        let insns = _out(&_body(vec![copied, shifted, _move(3, 1, None, 0x12)]), &[2]);
        let read = insns.iter().position(|one| one.defines == [3]).unwrap();
        assert!(!insns[..read].iter().any(|one| one.defines.contains(&1)));
    }

    #[test]
    fn test_parameter_is_reloaded_across_what_spares_the_frame() {
        for between in ["call sparing the frame", "call", "call unstated", "pointer store sparing the frame", "pointer store"]
        {
            let param = mem(Addr::new(Space::Frame, 6), 2, Register::BP, 0, 2);
            let load = insn(
                0x100,
                (0x100, 0x100),
                semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(param)]),
                &[1],
                &[],
            );
            let spared = if between.ends_with("sparing the frame") { vec![mir::WHOLE_FRAME] } else { vec![] };
            let middle = if between.starts_with("call") {
                let reach = if between == "call unstated" {
                    vec![]
                } else {
                    vec![MemRef { excludes: spared.clone(), ..MemRef::new(None, 4) }]
                };
                let mut site = mir::Op::new(0x101, OpCode::Operation(Operation::Nothing), "", vec![], vec![]);
                site.kind = mir::Kind::Call;
                site.stores = reach;
                if between != "call unstated" {
                    site.memory_complete = true;
                }
                let mut middle =
                    insn(0x101, (0x101, 0x101), semantics(Operation::Call, "call", vec![], vec![]), &[], &[]);
                middle.op = Some(Arc::new(site));
                middle.clobbers = BTreeSet::from([Register::EAX]);
                middle
            } else {
                let base = mir::Value::new(9, 0);
                let reference = MemRef {
                    base: Some(base),
                    space: Some(Space::Literal),
                    base_width: 2,
                    excludes: spared.clone(),
                    ..MemRef::new(Some(Addr::new(Space::Literal, 10)), 2)
                };
                let mut site = mir::Op::new(0x101, OpCode::Operation(Operation::Nothing), "", vec![], vec![base]);
                site.kind = mir::Kind::Store;
                site.stores = vec![reference];
                let cell = mem(Addr::new(Space::Literal, 10), 2, Register::BX, 10, 2);
                let mut middle = insn(
                    0x101,
                    (0x101, 0x101),
                    semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![imm(0, 2)]),
                    &[],
                    &[],
                );
                middle.op = Some(Arc::new(site));
                middle
            };
            let got = _out(&_body(vec![load, middle, _add(2, 1, 0x102)]), &[1]);
            let slots: Vec<&Loc> = got
                .iter()
                .filter_map(|one| one.what.as_ref())
                .flat_map(|what| &what.dests)
                .filter(|place| matches!(place, Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)))
                .collect();
            assert_eq!(slots.is_empty(), !spared.is_empty(), "{between}: {got:?}");
        }
    }

    fn _parameter_across(store: Insn) -> (LirBody, Frame, Mem) {
        let parameter = mem(Addr::new(Space::Frame, 6), 2, Register::BP, 0, 2);
        let load = insn(
            0x100,
            (0x100, 0x100),
            semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(parameter.clone())]),
            &[1],
            &[],
        );
        let mut frame = Frame::new(0);
        let (result, _made) =
            spilled(&_body(vec![load, store, _add(3, 1, 0x102)]), &set(&[1]), Some(&mut frame)).expect("spills");
        (result, frame, parameter)
    }

    fn _assert_parameter_rematerialized(store: Insn) {
        let (result, frame, parameter) = _parameter_across(store);
        assert!(!frame.slots.contains_key(&slot(1)));
        let insns = result.insns();
        let add = insns.iter().find(|one| name(one) == Some("add")).unwrap();
        assert!(what(add).sources.contains(&Loc::Mem(parameter)));
    }

    #[test]
    fn test_parameter_rematerializes_across_an_exact_disjoint_local_store() {
        let local = mem(Addr::new(Space::Frame, -2), 2, Register::BP, 0, 2);
        let vague_local = MemRef { space: Some(Space::Frame), ..MemRef::new(None, 2) };
        let mut op =
            mir::Op::new(0x101, OpCode::Operation(Operation::Move), "mov", vec![], vec![mir::Value::new(2, 0)]);
        op.kind = mir::Kind::Store;
        op.stores = vec![vague_local];
        let mut store = insn(
            0x101,
            (0x101, 0x101),
            semantics(Operation::Move, "mov", vec![Loc::Mem(local)], vec![held(2, 2)]),
            &[],
            &[2],
        );
        store.op = Some(Arc::new(op));
        _assert_parameter_rematerialized(store);
    }

    #[test]
    fn test_parameter_rematerializes_across_a_proven_local_array_store() {
        let local = Mem {
            index: Some(Held { value: 4, width: 2 }),
            index_through: Register::SI,
            ..mem(Addr::new(Space::Frame, -132), 2, Register::BP, 0, 2)
        };
        let object = MemoryObject {
            identity: Some(Identity::Tuple(vec![Identity::Int(7), Identity::Int(-132), Identity::Int(-4)])),
            extent: Some(128),
            ..MemoryObject::new(MemoryKind::Frame)
        };
        let indexed_local = MemRef {
            base: Some(mir::Value::new(4, 0)),
            space: Some(Space::Frame),
            provenance: Some(Provenance::one_with_slice(object, 0, 128, 2, 2, BTreeSet::new()).unwrap()),
            ..MemRef::new(None, 2)
        };
        let mut op = mir::Op::new(
            0x101,
            OpCode::Operation(Operation::Move),
            "mov",
            vec![],
            vec![mir::Value::new(2, 0), mir::Value::new(4, 0)],
        );
        op.kind = mir::Kind::Store;
        op.stores = vec![indexed_local];
        let mut store = insn(
            0x101,
            (0x101, 0x101),
            semantics(Operation::Move, "mov", vec![Loc::Mem(local)], vec![held(2, 2)]),
            &[],
            &[2, 4],
        );
        store.op = Some(Arc::new(op));
        _assert_parameter_rematerialized(store);
    }

    #[test]
    fn test_untied_spill_source_is_read_directly_by_arithmetic() {
        for width in [2, 4] {
            for op_name in ["add", "sub", "and", "or", "xor"] {
                let op = Insn {
                    what: Some(semantics(
                        Operation::Binary,
                        op_name,
                        vec![held(1, width)],
                        vec![held(1, width), held(2, width)],
                    )),
                    .._add(1, 2, 0x100)
                };
                let result = _out(&_body(vec![op]), &[2]);
                assert_eq!(result.len(), 1, "{op_name} {width}");
                assert_eq!(what(&result[0]).sources[0], held(1, width));
                assert!(is_mem(&what(&result[0]).sources[1]));
                assert_eq!(as_mem(&what(&result[0]).sources[1]).width, width);
                assert_eq!(result[0].uses, [1]);
            }
        }
    }

    #[test]
    fn test_untied_spill_source_is_read_directly_by_multiply() {
        for width in [2, 4] {
            let op = insn(
                0x100,
                (0x100, 0x103),
                semantics(Operation::Multiply, "imul", vec![held(1, width)], vec![held(1, width), held(2, width)]),
                &[1],
                &[1, 2],
            );
            let result = _out(&_body(vec![op]), &[2]);
            assert_eq!(result.len(), 1);
            assert_eq!(what(&result[0]).sources[0], held(1, width));
            assert!(is_mem(&what(&result[0]).sources[1]));
            assert_eq!(result[0].uses, [1]);
        }
    }

    #[test]
    fn test_repeated_tied_operand_does_not_become_memory_to_memory() {
        for op_name in ["xor", "add", "and", "sub"] {
            let op = _add(1, 1, 0x100);
            let op =
                Insn { what: Some(Semantics { name: Some(op_name.to_owned()), ..what(&op).clone() }), uses: vec![1], ..op };
            let result = _out(&_body(vec![op]), &[1]);
            let [reload, arithmetic, store] = result.as_slice() else { panic!("{result:?}") };
            assert!(is_mem(&what(reload).sources[0]));
            assert!(operands(what(arithmetic)).all(|arg| matches!(arg, Loc::Held(_))));
            assert_eq!(what(arithmetic).sources[0], what(arithmetic).sources[1]);
            assert_eq!(what(arithmetic).sources[1], what(arithmetic).dests[0]);
            assert!(is_mem(&what(store).dests[0]));
        }
    }

    #[test]
    fn test_repeated_operand_reloads_a_spill_only_once() {
        let multiply = insn(
            0x57,
            (0x57, 0x5A),
            semantics(Operation::Binary, "imul", vec![held(2, 2)], vec![held(1, 2), held(1, 2), imm(6, 2)]),
            &[2],
            &[1, 1],
        );
        let result = _out(&_body(vec![multiply]), &[1]);
        assert_eq!(result.len(), 2);
        let (reload, product) = (&result[0], &result[1]);
        assert!(is_mem(&what(reload).sources[0]));
        assert_eq!(product.uses, [reload.defines.clone(), reload.defines.clone()].concat());
        assert_eq!(what(product).sources[..2], [what(reload).dests.clone(), what(reload).dests.clone()].concat()[..]);
    }

    #[test]
    fn test_spilled_compare_preserves_order_flags_and_frame_address() {
        for width in [2, 4] {
            for both in [false, true] {
                let op = insn(
                    0,
                    (0, 3),
                    semantics(Operation::Compare, "cmp", vec![], vec![held(1, width), held(2, width)]),
                    &[3],
                    &[1, 2],
                );
                let result = _out(&_body(vec![op]), if both { &[1, 2] } else { &[2] });
                assert_eq!(result.len(), if both { 2 } else { 1 });
                let comparison = result.last().unwrap();
                assert_eq!(comparison.defines, [3]);
                assert_eq!(comparison.symbol, Some(false));
                assert!(matches!(what(comparison).sources[0], Loc::Held(_)));
                assert!(is_mem(&what(comparison).sources[1]));
                if both {
                    assert_eq!(what(comparison).sources[0], what(&result[0]).dests[0]);
                    assert_ne!(what(comparison).sources[1], what(&result[0]).sources[0]);
                } else {
                    assert_eq!(what(comparison).sources[0], held(1, width));
                }
            }
        }
    }

    #[test]
    fn test_spilled_constant_is_rematerialized_without_a_frame_slot() {
        let constant =
            insn(0, (0, 3), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(20, 2)]), &[1], &[]);
        let result = _out(&_body(vec![constant, _add(2, 1, 0x100)]), &[1]);
        assert!(!result.iter().filter_map(|one| one.what.as_ref()).any(|what| operands(what).any(is_mem)));
        let n = result.len();
        assert_eq!(what(&result[n - 2]).sources, [imm(20, 2)]);
        assert!(result[n - 2].rematerialized);
        assert_eq!(as_held(&what(&result[n - 1]).sources[1]).value, result[n - 2].defines[0]);
    }

    fn _frame_address(disp: i64, disp_width: u32) -> Address {
        Address { through: Register::BP, offset: disp, disp_width, ..Address::new(Some(Addr::new(Space::Frame, disp))) }
    }

    fn _lea(at: i64, covers: (i64, i64), source: &Address) -> Insn {
        let what = semantics(Operation::Address, "lea", vec![held(1, 2)], vec![Loc::Address(source.clone())]);
        insn(at, covers, what, &[1], &[])
    }

    fn _recreated(insns: &[Arc<Insn>]) -> Vec<&Arc<Insn>> {
        insns.iter().filter(|one| one.what.as_ref().is_some_and(|w| w.op == Operation::Address)).collect()
    }

    #[test]
    fn test_spilled_frame_address_is_rematerialized_without_a_frame_slot() {
        let source = _frame_address(-132, 2);
        let mut frame = Frame::new(0);
        let body = _body(vec![_lea(0, (0, 3), &source), _add(2, 1, 0x100)]);
        let (result, _made) = spilled(&body, &set(&[1]), Some(&mut frame)).expect("spills");
        assert!(!frame.slots.contains_key(&slot(1)));
        let insns = result.insns();
        let recreated = _recreated(&insns);
        assert_eq!(recreated.len(), 1);
        assert_eq!(what(recreated[0]).sources, [Loc::Address(source)]);
        assert!(recreated[0].rematerialized);
        assert_eq!(as_held(&what(insns.last().unwrap()).sources[1]).value, recreated[0].defines[0]);
    }

    #[test]
    fn test_spilled_frame_address_folds_directly_into_its_only_memory_use() {
        let source = _frame_address(-38, 1);
        let cell = Mem {
            base: Some(Held { value: 1, width: 2 }),
            ..Mem::new(Some(Addr { base: Register::SI, ..Addr::new(Space::Literal, 10) }), 2)
        };
        let load = insn(
            0x14,
            (0x14, 0x17),
            semantics(Operation::Move, "mov", vec![held(2, 2)], vec![Loc::Mem(cell)]),
            &[2],
            &[1],
        );
        let body = _body(vec![_lea(0x10, (0x10, 0x13), &source), load]);
        let (result, made) = spilled(&body, &set(&[1]), Some(&mut Frame::new(0))).expect("spills");
        assert!(made.is_empty());
        let insns = result.insns();
        assert!(_recreated(&insns).is_empty());
        let last = insns.last().unwrap();
        let folded = as_mem(&what(last).sources[0]);
        assert_eq!(folded.addr, Some(Addr::new(Space::Frame, -28)));
        assert!(folded.base.is_none());
        assert!(last.uses.is_empty());
    }

    #[test]
    fn test_spilled_relocatable_address_is_rematerialized_without_a_frame_slot() {
        for space in [Space::Segment, Space::External] {
            let source = Address { disp_width: 2, ..Address::new(Some(Addr { index: 7, ..Addr::new(space, 12) })) };
            let mut frame = Frame::new(0);
            let body = _body(vec![_lea(0, (0, 3), &source), _add(2, 1, 0x100)]);
            let (result, _made) = spilled(&body, &set(&[1]), Some(&mut frame)).expect("spills");
            assert!(!frame.slots.contains_key(&slot(1)));
            let insns = result.insns();
            let recreated = _recreated(&insns);
            assert_eq!(recreated.len(), 1);
            assert_eq!(what(recreated[0]).sources, [Loc::Address(source.clone())]);
            assert!(recreated[0].rematerialized);
            // The rematerialized spelling is not an unrelocated literal zero: fresh
            // OMF emission places the original symbol fixup on its new displacement.
            let lea = semantics(
                Operation::Address,
                "lea",
                vec![Loc::Reg(Reg { register: Register::BX, width: 2 })],
                vec![Loc::Address(source)],
            );
            let names: IndexMap<(Space, i64), String> = IndexMap::from_iter([((space, 7), "_descriptor".to_owned())]);
            let emitted = omfwrite::_encoded(&lea, &names).expect("encodes");
            assert_eq!(emitted.code, [0x8D, 0x1E, 0x0C, 0x00]);
            assert_eq!(emitted.fixups, [omfwrite::Fixup::new(2, omfwrite::OFFSET, "_descriptor")]);
        }
    }

    fn _extension(name: &str, at: i64) -> Insn {
        insn(at, (at, at), semantics(Operation::Extend, name, vec![held(2, 4)], vec![held(1, 2)]), &[2], &[1])
    }

    fn _wide_add(at: i64, into: u32) -> Insn {
        insn(
            at,
            (at, at + 2),
            semantics(Operation::Binary, "add", vec![held(into, 4)], vec![held(into, 4), held(2, 4)]),
            &[into],
            &[into, 2],
        )
    }

    #[test]
    fn test_single_use_extension_is_rematerialized_at_its_use() {
        for op_name in ["movsx", "movzx"] {
            let mut frame = Frame::new(0);
            let body = _body(vec![_move(1, 0, None, 0x0F), _extension(op_name, 0x10), _wide_add(0x14, 3)]);
            let (result, _made) = spilled(&body, &set(&[2]), Some(&mut frame)).expect("spills");
            let insns = result.insns();
            assert!(!frame.slots.contains_key(&slot(2)));
            assert_eq!(insns.iter().map(|one| name(one).unwrap()).collect::<Vec<_>>(), ["mov", op_name, "add"]);
            assert!(insns[1].rematerialized);
            assert_eq!(insns[1].uses, [1]);
            assert_eq!(insns[2].uses, [3, insns[1].defines[0]]);
            assert!(!insns.iter().any(|one| operands(what(one)).any(is_mem)));
        }
    }

    #[test]
    fn test_extension_rematerialization_requires_one_unchanged_ordinary_use() {
        for unsafe_ in ["multiple uses", "source redefined", "parallel copy"] {
            let mut use_ = _wide_add(0x14, 3);
            if unsafe_ == "parallel copy" {
                use_.group = Some(7);
            }
            let mut insns = vec![_extension("movsx", 0x10)];
            if unsafe_ == "source redefined" {
                insns.push(_move(1, 4, None, 0x12));
            }
            insns.push(use_);
            if unsafe_ == "multiple uses" {
                insns.push(insn(
                    0x18,
                    (0x18, 0x1A),
                    semantics(Operation::Binary, "add", vec![held(5, 4)], vec![held(5, 4), held(2, 4)]),
                    &[5],
                    &[5, 2],
                ));
            }
            let mut frame = Frame::new(0);
            spilled(&_body(insns), &set(&[2]), Some(&mut frame)).expect("spills");
            assert!(frame.slots.contains_key(&slot(2)), "{unsafe_}");
        }
    }

    fn _branch(op: Operation, name: &str, at: i64, target: i64) -> Insn {
        let what = Semantics { target: Some(target), ..semantics(op, name, vec![], vec![]) };
        insn(at, (at, at + 2), what, &[], &[])
    }

    #[test]
    fn test_extension_is_not_rematerialized_from_an_entry_into_a_loop() {
        let source = _move(1, 0, None, 0x10);
        let extension = _extension("movsx", 0x12);
        let jump = _branch(Operation::Jump, "jmp", 0x14, 0x20);
        let use_ = _wide_add(0x20, 3);
        let branch = _branch(Operation::Branch, "jne", 0x22, 0x20);
        let body = LirBody::new(
            "loop",
            0x10,
            vec![
                LirBlock {
                    succ: vec![0x20],
                    ..LirBlock::new(0x10, vec![Arc::new(source), Arc::new(extension), Arc::new(jump)])
                },
                LirBlock { succ: vec![0x20, 0x30], ..LirBlock::new(0x20, vec![Arc::new(use_), Arc::new(branch)]) },
                LirBlock::new(0x30, vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let mut frame = Frame::new(0);
        spilled(&body, &set(&[2]), Some(&mut frame)).expect("spills");
        assert!(frame.slots.contains_key(&slot(2)));
    }

    fn _sequential() -> LirBody {
        _body(vec![_move(1, 10, None, 0x10), _add(11, 1, 0x12), _move(2, 20, None, 0x14), _add(21, 2, 0x16)])
    }

    fn _overlapping() -> LirBody {
        _body(vec![_move(1, 10, None, 0x10), _move(2, 20, None, 0x12), _add(11, 1, 0x14), _add(21, 2, 0x16)])
    }

    #[test]
    fn test_nonoverlapping_spills_share_one_compatible_frame_slot() {
        let mut frame = Frame::new(0);
        spilled(&_sequential(), &set(&[1, 2]), Some(&mut frame)).expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 2);
    }

    #[test]
    fn test_nonoverlapping_spills_from_later_rounds_reuse_the_frame_slot() {
        let mut frame = Frame::new(0);
        let (first, _made) = spilled(&_sequential(), &set(&[1]), Some(&mut frame)).expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame)).expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 2);
    }

    #[test]
    fn test_overlapping_spills_from_later_rounds_keep_distinct_slots() {
        let mut frame = Frame::new(0);
        let (first, _made) = spilled(&_overlapping(), &set(&[1]), Some(&mut frame)).expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame)).expect("spills");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
    }

    #[test]
    fn test_cross_round_coloring_includes_current_preassigned_spill_webs() {
        let mut frame = Frame::new(0);
        frame.slots.insert(slot(1), -2);
        frame.capacities.insert(-2, 2);
        _color_slots(&_overlapping(), &set(&[1, 2]), &IndexMap::from_iter([(1, 2), (2, 2)]), &mut frame).expect("colors");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
    }

    fn _wide_pair(into: u32, out_of: u32, user: u32, at: i64) -> [Insn; 2] {
        [
            insn(
                at,
                (at, at),
                semantics(Operation::Move, "mov", vec![held(into, 4)], vec![held(out_of, 4)]),
                &[into],
                &[out_of],
            ),
            insn(
                at + 2,
                (at + 2, at + 2),
                semantics(Operation::Binary, "add", vec![held(user, 4)], vec![held(user, 4), held(into, 4)]),
                &[user],
                &[user, into],
            ),
        ]
    }

    #[test]
    fn test_later_narrow_spill_reuses_a_dead_wider_slot() {
        let mut frame = Frame::new(0);
        let [wide, use_wide] = _wide_pair(1, 10, 11, 0x10);
        let body = _body(vec![wide, use_wide, _move(2, 20, None, 0x14), _add(21, 2, 0x16)]);
        let (first, _made) = spilled(&body, &set(&[1]), Some(&mut frame)).expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame)).expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 4);
    }

    #[test]
    fn test_later_wide_spill_does_not_outgrow_a_narrow_slot() {
        let mut frame = Frame::new(0);
        let [wide, use_wide] = _wide_pair(2, 20, 21, 0x14);
        let body = _body(vec![_move(1, 10, None, 0x10), _add(11, 1, 0x12), wide, use_wide]);
        let (first, _made) = spilled(&body, &set(&[1]), Some(&mut frame)).expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame)).expect("spills");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 6);
    }

    #[test]
    fn test_overlapping_spills_keep_distinct_frame_slots() {
        let mut frame = Frame::new(0);
        spilled(&_overlapping(), &set(&[1, 2]), Some(&mut frame)).expect("spills");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 4);
    }

    #[test]
    fn test_narrow_spill_can_reuse_a_dead_wider_slot() {
        let mut frame = Frame::new(0);
        let [wide, use_wide] = _wide_pair(1, 10, 11, 0x10);
        let body = _body(vec![wide, use_wide, _move(2, 20, None, 0x14), _add(21, 2, 0x16)]);
        spilled(&body, &set(&[1, 2]), Some(&mut frame)).expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 4);
    }

    fn _constant(value: i64, covers: (i64, i64)) -> Insn {
        insn(0, covers, semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(value, 2)]), &[1], &[])
    }

    #[test]
    fn test_grouped_constant_rematerializes_without_splitting_parallel_copy() {
        for destination_spilled in [false, true] {
            let body = _body(vec![_constant(0, (0, 0)), _move(2, 1, Some(7), 0x100), _move(3, 4, Some(7), 0x100)]);
            let mut frame = Frame::new(0);
            let values = if destination_spilled { set(&[1, 2]) } else { set(&[1]) };
            let (done, _) = spilled(&body, &values, Some(&mut frame)).expect("spills");
            let insns = done.insns();
            let group: Vec<&Arc<Insn>> = insns.iter().filter(|one| one.group == Some(7)).collect();
            assert_eq!(group.len(), 2);
            assert_eq!(what(group[0]).sources, [imm(0, 2)]);
            assert!(group[0].uses.is_empty());
            assert_eq!(is_mem(&what(group[0]).dests[0]), destination_spilled);
            let positions: Vec<usize> =
                insns.iter().enumerate().filter(|(_, one)| one.group == Some(7)).map(|(index, _)| index).collect();
            assert_eq!(positions[1], positions[0] + 1);
            assert!(!insns.iter().any(|one| one.spill_reload));
        }
    }

    #[test]
    fn test_parallel_copy_destination_is_not_mistaken_for_a_constant() {
        let body = _body(vec![_constant(0, (0, 0)), _move(1, 4, Some(7), 0x100), _move(2, 1, Some(7), 0x100)]);
        assert!(_constants(&body, &set(&[1])).is_empty());
    }

    #[test]
    fn test_copied_constant_rematerializes_only_with_a_unique_definition() {
        for redefined in [false, true] {
            let mut operations = vec![_constant(20, (0, 3)), _move(2, 1, None, 0x100), _move(3, 2, None, 0x100)];
            if redefined {
                operations.push(_add(1, 4, 0x100));
            }
            operations.push(_add(5, 3, 0x100));
            let result = _out(&_body(operations), &[3]);
            let memory = result.iter().filter_map(|one| one.what.as_ref()).any(|what| operands(what).any(is_mem));
            assert_eq!(memory, redefined);
            if !redefined {
                let n = result.len();
                assert_eq!(what(&result[n - 2]).sources, [imm(20, 2)]);
                assert_eq!(as_held(&what(&result[n - 1]).sources[1]).value, result[n - 2].defines[0]);
            }
        }
    }

    #[test]
    fn test_copy_cycle_is_not_a_constant() {
        let body = _body(vec![_move(1, 2, None, 0x100), _move(2, 1, None, 0x100)]);
        assert!(_constants(&body, &set(&[1, 2])).is_empty());
    }

    #[test]
    fn test_relocated_address_is_not_rematerialized_as_literal_zero() {
        let address =
            Loc::Imm(Imm { value: 0, width: 2, address: Some(Addr { index: 5, ..Addr::new(Space::Segment, 6) }) });
        let defining =
            insn(0, (0, 3), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![address.clone()]), &[1], &[]);
        let result = _out(&_body(vec![defining, _add(2, 1, 0x100)]), &[1]);
        let count =
            result.iter().filter_map(|one| one.what.as_ref()).filter(|what| what.sources == [address.clone()]).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_a_grouped_move_reads_its_spilled_source_where_it_lives() {
        let got = _out(&_body(vec![_move(1, 2, Some(1), 0x100)]), &[2]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].group == Some(1) && got[0].uses.is_empty());
        assert!(is_mem(&what(&got[0]).sources[0]));
        assert_eq!(what(&got[0]).dests, [held(1, 2)]);
    }

    #[test]
    fn test_a_grouped_move_writes_its_spilled_destination_where_it_lives() {
        let got = _out(&_body(vec![_move(1, 2, Some(1), 0x100)]), &[1]);
        assert_eq!(got.len(), 1);
        assert!(got[0].group == Some(1) && got[0].defines.is_empty());
        assert!(is_mem(&what(&got[0]).dests[0]));
        assert_eq!(what(&got[0]).sources, [held(2, 2)]);
    }

    #[test]
    fn test_a_grouped_move_with_both_ends_spilled_stays_grouped() {
        let mut frame = Frame::new(0);
        frame.slots.insert(slot(1), -2);
        frame.slots.insert(slot(2), -4);
        let (got, _) =
            spilled(&_body(vec![_move(1, 2, Some(1), 0x100)]), &set(&[1, 2]), Some(&mut frame)).expect("spills");
        let got = got.insns();
        assert!(got.len() == 1 && got[0].group == Some(1));
        assert!(operands(what(&got[0])).all(is_mem));
        assert!(got[0].defines.is_empty() && got[0].uses.is_empty());
    }

    #[test]
    fn test_a_grouped_move_coalesced_to_one_spill_slot_is_an_identity() {
        let mut frame = Frame::new(0);
        let (got, _) =
            spilled(&_body(vec![_move(1, 2, Some(1), 0x100)]), &set(&[1, 2]), Some(&mut frame)).expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert!(got.insns().is_empty());
    }

    #[test]
    fn test_an_unhandled_arithmetic_form_keeps_its_reload() {
        let add = _add(1, 2, 0x100);
        let adc = Insn { what: Some(Semantics { name: Some("adc".to_owned()), ..what(&add).clone() }), ..add };
        let got = _out(&_body(vec![adc]), &[2]);
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(name(&got[0]) == Some("mov") && is_mem(&what(&got[0]).sources[0]));
        assert_eq!(name(&got[1]), Some("adc"));
    }

    #[test]
    fn test_the_group_comes_out_one_contiguous_run() {
        let body = _body(vec![_move(1, 2, Some(1), 0x100), _move(3, 4, Some(1), 0x100), _move(5, 6, Some(1), 0x100)]);
        let got = _out(&body, &[4]);
        assert_eq!(got.iter().map(|one| one.group).collect::<Vec<_>>(), [Some(1); 3], "{got:?}");
    }

    #[test]
    fn test_a_tied_value_an_instruction_requires_in_a_register_keeps_the_reload() {
        let imul = insn(
            0x200,
            (0x200, 0x202),
            semantics(Operation::Multiply, "imul", vec![held(1, 2), held(2, 2)], vec![held(1, 2), held(3, 2)]),
            &[1, 2],
            &[1, 3],
        );
        let got = _out(&_body(vec![imul]), &[1]);
        assert!(got.len() > 1, "the fixed tie took the in-place path");
        assert!(got.iter().filter(|one| name(one) == Some("mov")).any(|one| is_mem(&what(one).sources[0])));
    }

    #[test]
    fn test_spilling_a_pointer_renames_the_cell_it_is_the_base_of() {
        let where_ = Addr { base: Register::SI, ..Addr::new(Space::Segment, 0x10) };
        let cell = Mem { base: Some(Held { value: 3, width: 2 }), ..mem(where_, 2, Register::None, 0, 2) };
        let load = insn(
            0x20,
            (0x20, 0x22),
            semantics(Operation::Move, "mov", vec![held(5, 2)], vec![Loc::Mem(cell)]),
            &[5],
            &[3],
        );
        let (out, made) = spilled(&_body(vec![load]), &set(&[3]), None).expect("spills");
        let got = out.blocks[0]
            .insns
            .iter()
            .find(|one| what(one).sources.iter().any(|x| matches!(x, Loc::Mem(m) if m.base.is_some())))
            .unwrap();
        let read = as_mem(&what(got).sources[0]);
        let base = read.base.unwrap();
        assert_eq!(got.uses, [base.value], "uses {:?}, cell on {base:?}", got.uses);
        assert_ne!(got.uses, [3], "nothing was spilled; the fixture does not reach the rename");
        assert!(made.contains(&base.value), "{base:?} is not one of the reloads {made:?}");
        assert_eq!(read.through, Register::None, "the rename placed it");
        assert_eq!((read.addr, read.width, read.offset, read.disp_width), (Some(where_), 2, 0, 2));
    }

    #[test]
    fn test_a_dword_index_is_reloaded_as_a_dword() {
        let counter = Insn {
            what: Some(semantics(Operation::Move, "mov", vec![held(1, 4)], vec![held(3, 4)])),
            .._move(1, 3, None, 0x100)
        };
        let cell = Mem {
            base: Some(Held { value: 5, width: 4 }),
            index: Some(Held { value: 1, width: 4 }),
            scale: 2,
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), 2)
        };
        let read = insn(
            0x100,
            (0x100, 0x100),
            semantics(Operation::Move, "mov", vec![held(4, 2)], vec![Loc::Mem(cell)]),
            &[4],
            &[5, 1],
        );
        let out = _out(&_body(vec![counter, read]), &[1]);
        let reloads: Vec<&Arc<Insn>> = out.iter().filter(|one| one.spill_reload).collect();
        assert!(
            !reloads.is_empty() && reloads.iter().all(|one| as_held(&what(one).dests[0]).width == 4),
            "{out:?}"
        );
    }

    #[test]
    fn test_a_stable_load_stored_to_a_local_keeps_its_store_defined() {
        let cell = |disp: i64| Loc::Mem(mem(Addr::new(Space::Frame, disp), 2, Register::BP, disp, 1));
        let load =
            insn(0x10, (0x10, 0x13), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![cell(-0x3C)]), &[1], &[]);
        let store =
            insn(0x13, (0x13, 0x16), semantics(Operation::Move, "mov", vec![cell(-0x4E)], vec![held(1, 2)]), &[], &[1]);
        let limit = insn(
            0x16,
            (0x16, 0x18),
            semantics(Operation::Compare, "cmp", vec![], vec![held(2, 2), held(1, 2)]),
            &[3],
            &[2, 1],
        );
        let out = _out(&_body(vec![load, store, limit]), &[1]);
        let defined: BTreeSet<u32> = out.iter().flat_map(|one| one.defines.iter().copied()).collect();
        assert!(
            out.iter().flat_map(|one| one.uses.iter()).filter(|value| **value != 2).all(|value| defined.contains(value)),
            "{out:?}"
        );
    }

    /// snd_mix_frame stored four bytes of a value first seen as a word, over the saved BP.
    #[test]
    fn test_slot_is_as_wide_as_the_widest_use_of_its_value() {
        let op = |name: &str, into: u32, width: u32, sources: &[u32]| {
            let operation = if name == "mov" { Operation::Move } else { Operation::Binary };
            let what = semantics(
                operation,
                name,
                vec![held(into, width)],
                sources.iter().map(|one| held(*one, width)).collect(),
            );
            let mut uses: Vec<u32> = Vec::new();
            for one in sources {
                if !uses.contains(one) {
                    uses.push(*one);
                }
            }
            insn(0x100, (0x100, 0x100), what, &[into], &uses)
        };
        let body =
            _body(vec![op("mov", 1, 2, &[3]), op("mov", 2, 2, &[3]), op("add", 1, 4, &[1, 3]), op("add", 2, 2, &[2, 3])]);
        let mut frame = Frame::new(0);
        let (got, _made) = spilled(&body, &set(&[1, 2]), Some(&mut frame)).expect("spills");
        let cells: BTreeSet<(i64, u32)> = got
            .insns()
            .iter()
            .filter_map(|one| one.what.clone())
            .flat_map(|what| what.dests.into_iter().chain(what.sources))
            .filter_map(|place| match place {
                Loc::Mem(cell) => cell.addr.filter(|addr| addr.space == Space::Frame).map(|addr| (addr.disp, cell.width)),
                _ => None,
            })
            .collect();
        let size = frame.size();
        assert!(
            !cells.is_empty() && cells.iter().all(|(disp, width)| -size <= *disp && disp + i64::from(*width) <= 0),
            "{cells:?} {size}"
        );
        let spans: IndexMap<i64, i64> = cells
            .iter()
            .map(|(disp, _)| {
                let widest = cells.iter().filter(|(other, _)| other == disp).map(|(_, width)| i64::from(*width)).max();
                (*disp, widest.unwrap())
            })
            .collect();
        for (a, a_span) in &spans {
            for (b, b_span) in &spans {
                if a != b {
                    assert!(a + a_span <= *b || b + b_span <= *a, "{spans:?}");
                }
            }
        }
    }

    /// LNGMXX printed 169330 instead of 142900 after a tied spill discarded its loaded accumulator.
    #[test]
    fn test_two_spilled_operands_keep_the_accumulator_value() {
        for (name, expected) in
            [("add", 15000), ("sub", 9000), ("and", 12000 & 3000), ("or", 12000 | 3000), ("xor", 12000 ^ 3000)]
        {
            let mut frame = Frame::new(0);
            let op = _add(1, 2, 0x100);
            let op = Insn { what: Some(Semantics { name: Some(name.to_owned()), ..what(&op).clone() }), ..op };
            let (body, _) = spilled(&_body(vec![op]), &set(&[1, 2]), Some(&mut frame)).expect("spills");
            let first = Loc::Mem(frame.cell(1u32, 2).unwrap());
            let second = Loc::Mem(frame.cell(2u32, 2).unwrap());
            let mut values: Vec<(Loc, i64)> = vec![(first.clone(), 12000), (second.clone(), 3000)];
            let value = |values: &Vec<(Loc, i64)>, arg: &Loc| {
                values.iter().rev().find(|(key, _)| key == arg).map(|(_, value)| *value).expect("a value")
            };
            for one in body.insns() {
                let args: Vec<i64> = what(&one).sources.iter().map(|arg| value(&values, arg)).collect();
                let result = match name_of(&one) {
                    "mov" => args[0],
                    "add" => args[0] + args[1],
                    "sub" => args[0] - args[1],
                    "and" => args[0] & args[1],
                    "or" => args[0] | args[1],
                    "xor" => args[0] ^ args[1],
                    _ => panic!("{:?}", one.what),
                };
                values.push((what(&one).dests[0].clone(), result & 0xFFFF));
            }
            assert_eq!(value(&values, &first), expected, "{name}");
            assert_eq!(value(&values, &second), 3000, "{name}");
        }
    }

    fn name_of(one: &Insn) -> &str {
        name(one).expect("a name")
    }

    /// LNGMXX's two spilled operands must retain the accumulator but need only one scratch.
    #[test]
    fn test_two_spilled_operands_do_not_need_two_scratch_registers() {
        let result = _out(&_body(vec![_add(1, 2, 0x100)]), &[1, 2]);
        assert_eq!(result.len(), 2);
        assert!(is_mem(&what(result.last().unwrap()).dests[0]));
    }

    #[test]
    fn test_constant_reload_precedes_an_in_place_spilled_update() {
        let constant =
            insn(0, (0, 3), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(20, 2)]), &[1], &[]);
        let result = _out(&_body(vec![constant, _add(2, 1, 0x100)]), &[1, 2]);
        assert!(result.iter().all(|one| !one.uses.contains(&1)));
        let adds: Vec<&Arc<Insn>> = result.iter().filter(|one| name(one) == Some("add")).collect();
        let [add] = adds.as_slice() else { panic!("{} adds", adds.len()) };
        let mut made: IndexMap<u32, &Arc<Insn>> = IndexMap::default();
        for one in &result[..index_of(&result, add)] {
            for value in &one.defines {
                made.insert(*value, one);
            }
        }
        let added: Vec<Loc> = add.uses.iter().map(|value| what(made[value]).sources[0].clone()).collect();
        assert!(added.contains(&imm(20, 2)));
        assert!(is_mem(&what(add).dests[0]));
        assert_eq!(what(add).sources[0], what(add).dests[0]);
        assert!(!result.iter().any(|one| one.spill_store));
    }

    /// The fixup names the operand, so it goes where the operand goes.
    #[test]
    fn test_a_lifted_memory_operand_takes_the_fixup_with_it() {
        let cell = Mem::new(Some(Addr::new(Space::Segment, 0xA)), 2);
        let what_ = semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), Loc::Mem(cell)]);
        let add = insn(0x100, (0x100, 0x104), what_, &[1], &[1]);
        let got = _out(&_body(vec![add]), &[1]);
        let symbolic = |one: &Insn| {
            one.what.as_ref().is_some_and(|what| {
                operands(what).any(|x| {
                    matches!(x, Loc::Mem(Mem { addr: Some(addr), .. }) if addr.space == Space::Segment)
                })
            })
        };
        let lifted: Vec<&Arc<Insn>> = got.iter().filter(|one| symbolic(one)).collect();
        assert_eq!(lifted.len(), 1, "{got:?}");
        assert_eq!(lifted[0].symbol, Some(true), "the load does not claim the operand it now holds");
        let kept: Vec<&Arc<Insn>> =
            got.iter().filter(|one| !Arc::ptr_eq(one, lifted[0]) && name(one) == Some("add")).collect();
        assert!(
            !kept.is_empty() && kept[0].symbol == Some(false),
            "the survivor still claims a fixup for an operand it lost"
        );
    }

    fn _binary(name: &str, into: u32, other: u32, at: i64) -> Insn {
        let what = semantics(Operation::Binary, name, vec![held(into, 2)], vec![held(into, 2), held(other, 2)]);
        insn(at, (at, at + 2), what, &[into], &[into, other])
    }

    /// pressx spilled 207, then 212, then 215, at one `add`, two instructions added every round.
    #[test]
    fn test_a_tied_value_is_spilled_into_the_operand_itself() {
        let got = _out(&_body(vec![_binary("add", 1, 2, 0x200)]), &[1]);
        assert_eq!(got.len(), 1, "{got:?}");
        let what = what(&got[0]);
        assert!(is_mem(&what.dests[0]) && what.dests[0] == what.sources[0]);
        assert_eq!(what.sources[1], held(2, 2));
        assert!(got[0].defines.is_empty() && got[0].uses == [2]);
    }

    /// One memory operand is all an instruction has.
    #[test]
    fn test_a_second_memory_operand_keeps_the_reload() {
        let what = semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), held(2, 2)]);
        let both = insn(0x200, (0x200, 0x202), what, &[1], &[1, 2]);
        let got = _out(&_body(vec![both]), &[1, 2]);
        assert!(got.len() > 1, "two spilled operands took the in-place path");
    }

    /// `ir.Mem(Addr(Space.FAR, disp, segment=Register.ES), 2, base=ir.Held(5, 2), index=...)`.
    fn _far(disp: i64, index: Option<u32>) -> Mem {
        Mem {
            base: Some(Held { value: 5, width: 2 }),
            index: index.map(|value| Held { value, width: 2 }),
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, disp) }), 2)
        }
    }

    fn _read(at: i64, into: u32, cell: Mem, uses: &[u32]) -> Insn {
        insn(at, (at, at + 2), semantics(Operation::Move, "mov", vec![held(into, 2)], vec![Loc::Mem(cell)]), &[into], uses)
    }

    fn _folded_add(one: &Insn) -> bool {
        one.what.as_ref().is_some_and(|what| {
            what.name.as_deref() == Some("add") && what.sources.len() == 2 && is_mem(&what.sources[1])
        })
    }

    /// farloadloop reloaded a carried word index solely to address one cell.
    #[test]
    fn test_a_spilled_word_index_folds_into_a_dead_address_base() {
        let read = _read(0x102, 4, _far(0, Some(1)), &[5, 1]);
        let result = _out(&_body(vec![_add(1, 2, 0x100), read]), &[1]);
        let folded: Vec<&Arc<Insn>> = result.iter().filter(|one| _folded_add(one)).collect();
        assert_eq!(folded.len(), 1, "{result:?}");
        let addressed = result
            .iter()
            .find(|one| {
                name(one) == Some("mov")
                    && matches!(&what(one).sources[0], Loc::Mem(Mem { addr: Some(addr), .. }) if addr.space == Space::Far)
            })
            .unwrap();
        assert!(as_mem(&what(addressed).sources[0]).index.is_none(), "{:?}", addressed.what);
        assert!(!result.iter().any(|one| one.spill_reload), "{result:?}");
    }

    /// The index fold may not move a second access through the same base.
    #[test]
    fn test_a_spilled_word_index_does_not_mutate_a_base_used_later() {
        let first = _read(0x102, 4, _far(0, Some(1)), &[5, 1]);
        let later = _read(0x104, 6, _far(2, None), &[5]);
        let result = _out(&_body(vec![_add(1, 2, 0x100), first, later]), &[1]);
        assert!(!result.iter().any(|one| _folded_add(one)), "{result:?}");
    }

    /// A base used later outside a memory operand was omitted from the death proof.
    #[test]
    fn test_a_spilled_word_index_does_not_mutate_a_base_read_later_as_a_value() {
        let read = _read(0x102, 4, _far(0, Some(1)), &[5, 1]);
        let result = _out(&_body(vec![_add(1, 2, 0x100), read, _move(6, 5, None, 0x100)]), &[1]);
        assert!(!result.iter().any(|one| _folded_add(one)), "{result:?}");
    }

    /// A destructive address fold also changed a base/index data operand.
    #[test]
    fn test_a_spilled_word_index_does_not_mutate_an_address_operand_used_as_data() {
        for data_value in [1, 5] {
            let cell = Loc::Mem(_far(0, Some(1)));
            let write = insn(
                0x102,
                (0x102, 0x104),
                semantics(Operation::Binary, "add", vec![cell.clone()], vec![cell, held(data_value, 2)]),
                &[],
                &[5, 1],
            );
            let result = _out(&_body(vec![_add(1, 2, 0x100), write]), &[1]);
            assert!(!result.iter().any(|one| _folded_add(one)), "{data_value}: {result:?}");
        }
    }

    /// UNWHITEFADE's frame counter, coalesced with its initial zero, lost its increment.
    #[test]
    fn test_a_value_defined_twice_keeps_its_increment_in_its_home() {
        let home = mem(Addr::new(Space::Frame, -0x2A), 2, Register::BP, -0x2A, 1);
        let at = |at: i64, what: Semantics, defines: &[u32], uses: &[u32]| insn(at, (at, at + 2), what, defines, uses);
        let block = |at: i64, insns: Vec<Insn>, succ: Vec<i64>| LirBlock {
            succ,
            ..LirBlock::new(at, insns.into_iter().map(Arc::new).collect())
        };
        let jump = Semantics { target: Some(0x20), ..semantics(Operation::Jump, "jmp", vec![], vec![]) };
        let branch = Semantics { target: Some(0x10), ..semantics(Operation::Branch, "jl", vec![], vec![]) };
        let body = LirBody::new(
            "one",
            0,
            vec![
                block(
                    0,
                    vec![
                        at(0, semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(0, 2)]), &[1], &[]),
                        at(2, jump, &[], &[]),
                    ],
                    vec![0x20],
                ),
                block(
                    0x10,
                    vec![
                        at(0x10, semantics(Operation::Push, "push", vec![], vec![held(1, 2)]), &[], &[1]),
                        at(
                            0x12,
                            semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), imm(1, 2)]),
                            &[1],
                            &[1],
                        ),
                    ],
                    vec![0x20],
                ),
                block(
                    0x20,
                    vec![
                        at(0x20, semantics(Operation::Move, "mov", vec![Loc::Mem(home)], vec![held(1, 2)]), &[], &[1]),
                        at(0x22, semantics(Operation::Compare, "cmp", vec![], vec![held(1, 2), imm(5, 2)]), &[9], &[1]),
                        at(0x24, branch, &[], &[9]),
                    ],
                    vec![0x10, 0x30],
                ),
                block(0x30, vec![at(0x30, semantics(Operation::Return, "ret", vec![], vec![]), &[], &[])], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let (done, _made) = spilled(&body, &set(&[1]), Some(&mut Frame::new(0))).expect("spills");
        let mut held_: IndexMap<u32, i64> = IndexMap::default();
        let mut memory: IndexMap<i64, i64> = IndexMap::default();
        let mut pushed: Vec<i64> = Vec::new();
        let read = |held_: &IndexMap<u32, i64>, memory: &IndexMap<i64, i64>, operand: &Loc| match operand {
            Loc::Imm(one) => one.value,
            Loc::Mem(one) => memory[&one.addr.unwrap().disp],
            Loc::Held(one) => held_[&one.value],
            other => panic!("{other:?}"),
        };
        let blocks: IndexMap<i64, &LirBlock> = done.blocks.iter().map(|block| (block.at, block)).collect();
        for at in [0, 0x20, 0x10, 0x20, 0x10, 0x20] {
            for one in &blocks[&at].insns {
                let what = what(one);
                if matches!(what.op, Operation::Move | Operation::Binary) {
                    let result: i64 = what.sources.iter().map(|source| read(&held_, &memory, source)).sum();
                    match &what.dests[0] {
                        Loc::Mem(dest) => {
                            memory.insert(dest.addr.unwrap().disp, result);
                        }
                        dest => {
                            held_.insert(as_held(dest).value, result);
                        }
                    }
                } else if what.op == Operation::Push {
                    pushed.push(read(&held_, &memory, &what.sources[0]));
                }
            }
        }
        assert_eq!(pushed, [0, 1]);
        assert_eq!(memory[&-0x2A], 2);
    }

    // ------------------------------------------- tests/test_rematerialized_definitions.py

    fn _remat_constant() -> Insn {
        insn(0, (0, 3), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(64, 2)]), &[1], &[])
    }

    fn _push(at: i64, covers: (i64, i64), source: Loc, uses: &[u32]) -> Insn {
        insn(at, covers, semantics(Operation::Push, "push", vec![], vec![source]), &[], uses)
    }

    fn _spilled_one(name: &str, insns: Vec<Insn>) -> LirBody {
        let body = LirBody::new(name, 0, vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect())], IndexMap::default(), IndexMap::default());
        spilled(&body, &set(&[1]), Some(&mut Frame::new(0))).expect("spills").0
    }

    /// MODEL's MOD_OPEN kept mov bx,40h after rematerializing 40h at its call.
    #[test]
    fn test_rematerialization_does_not_keep_the_abandoned_constant() {
        let result = _spilled_one("rematerialized", vec![_remat_constant(), _push(3, (3, 4), held(1, 2), &[1])]);
        let insns = result.insns();
        let moves = insns.iter().filter(|one| one.what.as_ref().is_some_and(|w| w.op == Operation::Move)).count();
        assert_eq!(moves, 1);
        assert_eq!(insns.last().unwrap().covers, Some((0, 4)));
    }

    #[test]
    fn test_reordered_definition_retains_a_byte_ownership_anchor() {
        let prefix = _push(0, (0, 1), imm(0, 2), &[]);
        let constant =
            insn(3, (3, 6), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(64, 2)]), &[1], &[]);
        let moved = Insn { at: 1, covers: Some((1, 3)), ..prefix.clone() };
        let use_ = _push(6, (6, 7), held(1, 2), &[1]);
        let result = _spilled_one("reordered", vec![prefix, constant, moved, use_]);
        let insns = result.insns();
        let owner = insns.iter().find(|one| one.covers == Some((3, 6))).unwrap();
        assert_eq!(owner.what, Some(Semantics { name: Some("nop".to_owned()), ..Semantics::new(Operation::Nothing) }));
        assert!(owner.defines.is_empty() && owner.uses.is_empty());
    }

    #[test]
    fn test_rematerialized_definition_with_unresolved_obligations_is_kept() {
        for constraint in ["requires", "delivers", "symbol"] {
            let mut constant = _remat_constant();
            match constraint {
                "requires" => constant.requires = vec![(Held { value: 1, width: 2 }, Register::AX)],
                "delivers" => constant.delivers = vec![(Held { value: 1, width: 2 }, Register::AX)],
                _ => constant.symbol = Some(true),
            }
            let result = _spilled_one("guarded", vec![constant, _push(3, (3, 4), held(1, 2), &[1])]);
            assert!(
                result
                    .insns()
                    .iter()
                    .any(|one| one.at == 0 && one.what.as_ref().is_some_and(|w| w.op == Operation::Move)),
                "{constraint}"
            );
        }
    }
}
