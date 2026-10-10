//! Port of `qbopt/backend/spiller.py`: a value the allocator would not keep,
//! kept in memory instead.
//!
//! LLVM's `InlineSpiller`: every definition of a spilled value becomes a
//! store into its frame slot, and every use a load into a fresh value that
//! lives only across that one instruction.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;
use llrm_lir::registers::Regs;

use crate::analysis::intervals::{self as ranges, Interval, Segment, key};
use crate::backend::allocate::Error;
use crate::backend::classes::RegisterClasses;
use crate::backend::coalesce;
use crate::backend::frame::{self as frames, Frame, SlotKey};
use crate::backend::postings::{self, At, Postings};
use crate::model::ir::{self, Addr, AddressRef, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::support::hash::{IndexMap, IndexSet};
use crate::support::pyset::PySet;

/// `frame.WORD` at the width type this module uses.
const WORD: u32 = frames::WORD as u32;
/// Python duck-types `frame.cell(value, width)` over a `Frame` and a
/// `_Cells`; this is that one method.
pub trait CellOf {
    fn cell_of(
        &mut self,
        value: u32,
        width: u32,
    ) -> Result<Option<Mem>, Error>;
}

impl CellOf for Frame {
    fn cell_of(
        &mut self,
        value: u32,
        width: u32,
    ) -> Result<Option<Mem>, Error> {
        Ok(Some(self.cell(value, width)?))
    }
}

fn _set(values: &[u32]) -> BTreeSet<u32> {
    values.iter().copied().collect()
}

fn _with<F: FnOnce(&mut Insn)>(
    one: &Insn,
    change: F,
) -> Arc<Insn> {
    let mut made = one.clone();
    change(&mut made);
    Arc::new(made)
}

/// `body` with each of `values` living in a frame slot, and the reloads.
pub fn spilled(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: Option<&mut Frame>,
    classes: &RegisterClasses,
) -> Result<(LirBody, BTreeSet<u32>), Error> {
    spilled_from(body, values, frame, 0, classes)
}

/// How each value of a set is spilled: the decision, made before any
/// instruction is written. A value is made again where it is read (a constant,
/// an address, an extension, a stable load of a frame cell), or stored to a
/// slot of the frame.
pub struct Plan {
    constants: IndexMap<u32, Imm>,
    addresses: IndexMap<u32, AddressRef>,
    extensions: IndexMap<u32, Arc<Insn>>,
    frame_loads: IndexMap<u32, Mem>,
    /// The cell of each value made again from the frame.
    rebuilt: IndexMap<u32, Mem>,
    stored: BTreeSet<u32>,
    narrow: IndexMap<u32, Imm>,
}

impl Plan {}

/// `values` of `body` decided: how each is spilled, and the slots of those
/// stored.
pub fn planned(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
) -> Result<Plan, Error> {
    postings::following(body, |postings| planned_with(body, values, frame, postings))
}

fn planned_with(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
    postings: &Postings,
) -> Result<Plan, Error> {
    // A plain copy of a value made again is made again the same way.
    let copies = llrm_support::debug::timed("spill copies", || _copies_by(body, values, postings));
    let wide: BTreeSet<u32> = values.iter().chain(copies.values()).copied().collect();
    let constants = _through_copies(
        llrm_support::debug::timed("spill constants", || _literals_by(body, &wide, false, postings)),
        values,
        &copies,
    );
    let addresses = _through_copies(
        llrm_support::debug::timed("spill addresses", || _addresses_by(body, &wide, postings)),
        values,
        &copies,
    );
    let extensions = llrm_support::debug::timed("spill extensions", || _extensions_by(body, values, postings));
    let mut frame_loads = llrm_support::debug::timed("spill stable loads", || {
        _stable_loads_by(body, values, &IndexMap::default(), postings)
    });
    frame_loads.extend(llrm_support::debug::timed("spill frame loads", || _frame_loads(body, values)));
    // A phi's value the program also stores to a cell the body has not written
    // since is read from there.
    if !body.homes.is_empty() {
        let widths: IndexMap<u32, u32> = body
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .filter_map(|one| one.what.as_ref())
            .flat_map(|what| what.dests.iter().chain(&what.sources))
            .filter_map(|place| if let Loc::Held(held) = place { Some((held.value, held.width)) } else { None })
            .collect();
        let homed = crate::backend::storedhomes::held(body, &|value, cell| {
            values.contains(&value) && !frame_loads.contains_key(&value) && widths.get(&value) == Some(&cell.width)
        });
        frame_loads.extend(homed);
    }
    // A copy of a load is made again as that load.
    if !copies.is_empty() {
        let apart: BTreeSet<u32> = values.iter().copied().filter(|value| !frame_loads.contains_key(value)).collect();
        let copied = _through_copies(
            llrm_support::debug::timed("spill stable loads", || _stable_loads_by(body, &wide, &copies, postings)),
            &apart,
            &copies,
        );
        for (value, cell) in copied {
            frame_loads.entry(value).or_insert(cell);
        }
    }
    let rebuilt = frame_loads.clone();
    let stored: BTreeSet<u32> = values
        .iter()
        .copied()
        .filter(|value| {
            !constants.contains_key(value)
                && !addresses.contains_key(value)
                && !extensions.contains_key(value)
                && !frame_loads.contains_key(value)
        })
        .collect();
    // Before any cell names a slot.
    llrm_support::debug::timed("spill color slots", || {
        _color_slots(body, &stored, &_widest_by(body, &stored, postings), frame)
    })?;
    let narrow = llrm_support::debug::timed("spill literals", || _literals_by(body, &stored, true, postings));
    Ok(Plan { constants, addresses, extensions, frame_loads, rebuilt, stored, narrow })
}

/// `spilled`, numbering the values it makes from `floor` at least: an
/// allocation in progress still knows values the body no longer names.
pub fn spilled_from(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: Option<&mut Frame>,
    floor: u32,
    classes: &RegisterClasses,
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
    let plan = planned(body, values, frame)?;
    let (body, made, merged) = materialized(body, &plan, frame, floor, classes, &BTreeSet::new())?;
    Ok((body, made.union(&merged).copied().collect()))
}

/// `spilled_from`, with `plain` kept out of the merging of updates into one
/// register value, and what that merging made told apart from the rest: (the
/// body, the values made to live for one use, the values made to live across
/// several).
///
/// A merged value that is spilled in its turn is merged again, as long as it is
/// not told to be plain: a spill that makes what it spills, without end
/// (`chain` at N=7 with -m16 never finished). The caller names each merged
/// value plain once it is spilled.
pub fn spilled_apart(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
    floor: u32,
    classes: &RegisterClasses,
    plain: &BTreeSet<u32>,
) -> Result<(LirBody, BTreeSet<u32>, BTreeSet<u32>), Error> {
    if values.is_empty() {
        return Ok((body.clone(), BTreeSet::new(), BTreeSet::new()));
    }
    let plan = planned(body, values, frame)?;
    materialized(body, &plan, frame, floor, classes, plain)
}

/// `body` with `plan` written: a reload before each read of a stored value, a
/// store after each write, and every other value made again where it is read.
/// The values made are returned; each lives for one use.
pub fn materialized(
    body: &LirBody,
    plan: &Plan,
    frame: &mut Frame,
    floor: u32,
    classes: &RegisterClasses,
    plain: &BTreeSet<u32>,
) -> Result<(LirBody, BTreeSet<u32>, BTreeSet<u32>), Error> {
    let regs = body.regs();
    let mut fresh = crate::backend::splitkit::_next_value_following(body).max(floor);
    let mut made: BTreeSet<u32> = BTreeSet::new();
    let Plan { constants, addresses, extensions, frame_loads, rebuilt, stored, narrow } = plan;
    let merging: BTreeSet<u32> = stored.difference(plain).copied().collect();
    let first = fresh;
    let (body, next) =
        llrm_support::debug::timed("spill short updates", || _short_update_runs(body, &merging, frame, fresh))?;
    fresh = next;
    let (body, next) =
        llrm_support::debug::timed("spill local updates", || _local_updates(&body, &merging, frame, fresh))?;
    fresh = next;
    let merged: BTreeSet<u32> = (first..fresh).collect();
    let mut abandoned: BTreeSet<usize> = BTreeSet::new();
    let mut rematerialized_definitions: BTreeSet<usize> = BTreeSet::new();
    let mut identities: BTreeSet<usize> = BTreeSet::new();
    let body = llrm_support::debug::timed("spill sunk copies", || {
        _sunk_from_copies(&body, &addresses.keys().copied().collect())
    });
    let r#final = FinalUses::new(&body);
    let _rewrite = llrm_support::debug::span("spill rewrite");
    let rebuilt_values: BTreeSet<u32> = rebuilt.keys().copied().collect();
    let mut cells = _Cells::new(rebuilt.clone());

    // The blocks holding an instruction the cleanup after this loop removes or
    // anchors.
    let mut marked: BTreeSet<usize> = BTreeSet::new();
    // An instruction that names none of the values below is left as it is, so a
    // block with none is not looked at.
    let relevant: BTreeSet<u32> = stored
        .iter()
        .chain(rebuilt.keys())
        .chain(constants.keys())
        .chain(addresses.keys())
        .chain(extensions.keys())
        .chain(frame_loads.keys())
        .copied()
        .collect();
    // The positions, in each block, of the instructions that name one.
    let mut touching: Vec<Vec<u32>> = vec![Vec::new(); body.blocks.len()];
    postings::following(&body, |postings| {
        for value in &relevant {
            for at in postings.defs(*value).iter().chain(postings.uses(*value)).chain(postings.needs(*value)) {
                touching[at.0 as usize].push(at.1);
            }
        }
    });
    for positions in &mut touching {
        positions.sort_unstable();
        positions.dedup();
    }
    let named: BTreeSet<usize> = (0..touching.len()).filter(|block| !touching[*block].is_empty()).collect();
    let mut blocks = Vec::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        if !named.contains(&block_index) && !check_postings() {
            blocks.push(block.clone());
            continue;
        }
        LOOKED.with(|looked| looked.set(looked.get() + 1));
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        // Where the parallel copy being copied begins in `insns`: what its
        // moves read is made before all of them, not between two.
        let mut copy: Option<(i64, usize)> = None;
        for (position, original) in block.insns.iter().enumerate() {
            let mut one = Arc::clone(original);
            copy = match (one.group, copy) {
                (Some(group), Some((open, at))) if group == open => Some((open, at)),
                (Some(group), _) => Some((group, insns.len())),
                (None, _) => None,
            };
            // One that names none of the values is left as it is.
            let untouched = touching[block_index].binary_search(&(position as u32)).is_err();
            if untouched && !check_postings() {
                insns.push(one);
                continue;
            }
            if _identity(&one, &stored, frame) {
                marked.insert(block_index);
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
                    if let Some(folded) = _address_source(regs, &one, value, address) {
                        one = folded;
                    }
                }
            }
            let mut remade: IndexMap<u32, u32> = IndexMap::default();
            for value in one.uses.clone() {
                let constant =
                    constants.get(&value).or(narrow.get(&value).filter(|imm| _width(&one, value) <= imm.width));
                if (constant.is_none()
                    && !addresses.contains_key(&value)
                    && !extensions.contains_key(&value)
                    && !frame_loads.contains_key(&value))
                    || remade.contains_key(&value)
                {
                    continue;
                }
                remade.insert(value, fresh);
                let inserted = if let Some(constant) = constant {
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
                        Semantics {
                            dests: vec![Loc::Held(Held { value: fresh, width: destination.width })],
                            ..what.clone()
                        },
                        vec![fresh],
                        vec![source.value],
                    )
                } else {
                    _reload(&one, fresh, &frame_loads[&value])
                };
                let product = _with(&inserted, |made| made.rematerialized = true);
                match &mut copy {
                    Some((_, at)) => {
                        insns.insert(*at, product);
                        *at += 1;
                    }
                    None => insns.push(product),
                }
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
                None => _tied(body.bits, &one, &stored, frame, classes)?,
            };
            if let Some(direct) = direct {
                insns.push(direct);
                continue;
            }
            if let Some((read, loaded)) = _memory_source_read_first(&one, &stored, fresh) {
                one = loaded;
                fresh += 1;
                // The read names the cell's values: the spilled ones come back
                // first.
                let (reloads, renamed) = _reloaded(&read, &stored, frame, &mut fresh)?;
                insns.extend(reloads);
                insns.push(if renamed.is_empty() { read } else { _renamed(&read, &renamed) });
                if let Some(direct) = _tied(body.bits, &one, &stored, frame, classes)? {
                    insns.push(direct);
                    continue;
                }
            }
            let mut after: Vec<Arc<Insn>> = Vec::new();
            let (before, mut rename) = _reloaded(&one, &stored, frame, &mut fresh)?;
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
                marked.insert(block_index);
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
                marked.insert(block_index);
                abandoned.insert(key(&rewritten));
            }
            insns.extend(after);
            made.extend(rename.values().copied());
        }
        if check_postings() && !named.contains(&block_index) {
            assert!(
                insns.len() == block.insns.len()
                    && insns.iter().zip(&block.insns).all(|(made, was)| Arc::ptr_eq(made, was)),
                "{}: the block at {:#x} names none of the values spilled and was rewritten",
                body.name,
                block.at
            );
        }
        if check_postings() {
            let kept: crate::support::hash::HashSet<*const Insn> = insns.iter().map(Arc::as_ptr).collect();
            for (position, one) in block.insns.iter().enumerate() {
                if touching[block_index].binary_search(&(position as u32)).is_err() {
                    assert!(
                        kept.contains(&Arc::as_ptr(one))
                            && !identities.contains(&key(one))
                            && !abandoned.contains(&key(one)),
                        "{}: the instruction at {position} of block {block_index} names none of the values spilled and was changed",
                        body.name
                    );
                }
            }
        }
        blocks.push(block.with_insns(insns));
    }
    drop(_rewrite);
    let _cleanup = llrm_support::debug::span("spill cleanup");
    let mut result = body.with_blocks(blocks);
    let never = None::<fn(&Arc<Insn>) -> Arc<Insn>>;
    // Only a block that held one of those instructions has any to remove.
    for &at in &marked {
        if !rematerialized_definitions.is_empty() {
            result.blocks[at] = result
                .blocks[at]
                .with_insns(
                    lir::without(
                        &result.blocks[at].insns,
                        |one| rematerialized_definitions.contains(&key(one)),
                        never,
                    ),
                );
        }
        if !identities.is_empty() {
            result.blocks[at] = result
                .blocks[at]
                .with_insns(
                    lir::without(
                        &result.blocks[at].insns,
                        |one| identities.contains(&key(one)),
                        never,
                    ),
                );
        }
    }
    let result = _remove_abandoned_in(result, &abandoned, &marked);
    // The values made that something still defines.
    let made = postings::following(&result, |postings| {
        made.iter().copied().filter(|value| !postings.defs(*value).is_empty()).collect::<BTreeSet<u32>>()
    });
    Ok((result, made, merged))
}

/// The reloads of the spilled values `one` reads, and the value each is read
/// as.
fn _reloaded(
    one: &Insn,
    stored: &BTreeSet<u32>,
    frame: &mut Frame,
    fresh: &mut u32,
) -> Result<(Vec<Arc<Insn>>, IndexMap<u32, u32>), Error> {
    let mut rename: IndexMap<u32, u32> = IndexMap::default();
    let mut before: Vec<Arc<Insn>> = Vec::new();
    for value in &one.uses {
        if !stored.contains(value) || rename.contains_key(value) {
            continue;
        }
        rename.insert(*value, *fresh);
        before.push(_reload(one, *fresh, &frame.cell(*value, _width(one, *value))?));
        *fresh += 1;
    }
    Ok((before, rename))
}

/// `body` with each move of a parallel copy that reads a value in `reading`
/// made a plain move after the copy. The value is then made again for that
/// one move, beside it, rather than for the copy: it would live across all of
/// them. A move that reads one value and writes another is as correct there,
/// since a copy reads before it writes.
fn _sunk_from_copies<'b>(
    body: &'b LirBody,
    reading: &BTreeSet<u32>,
) -> Cow<'b, LirBody> {
    let moved = |one: &Insn| _group_source(one).is_some_and(|source| reading.contains(&source.value));
    // A copy of a value in `reading` reads it, so it is among that value's
    // readers.
    let any = postings::following(body, |postings| {
        reading.iter().any(|value| postings.uses(*value).iter().any(|at| moved(_at(body, *at))))
    });
    if check_postings() {
        assert_eq!(
            any,
            body.insns().iter().any(|one| moved(one)),
            "{}: the copies to sink from the postings differ",
            body.name
        );
    }
    if !any {
        return Cow::Borrowed(body);
    }
    Cow::Owned(_sunk_whole(body, &moved))
}

fn _sunk_whole(
    body: &LirBody,
    moved: &dyn Fn(&Insn) -> bool,
) -> LirBody {
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns: Vec<Arc<Insn>> = Vec::new();
            let mut sunk: Vec<Arc<Insn>> = Vec::new();
            let mut copy: Option<i64> = None;
            for one in &block.insns {
                if one.group != copy {
                    insns.append(&mut sunk);
                }
                copy = one.group;
                if moved(one) {
                    sunk.push(_with(one, |made| made.group = None));
                } else {
                    insns.push(Arc::clone(one));
                }
            }
            insns.append(&mut sunk);
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}

/// Keep a just-defined spilled value in a register through one update.
///
/// Only a source that dies at the copy: the update now writes its register.
fn _short_update_runs<'b>(
    body: &'b LirBody,
    stored: &BTreeSet<u32>,
    frame: &mut Frame,
    fresh: u32,
) -> Result<(Cow<'b, LirBody>, u32), Error> {
    // A run is a plain move into a stored value followed by something: only a
    // definition of one can start it.
    let possible = postings::following(body, |postings| {
        stored
            .iter()
            .any(
                |&value| postings
                    .defs(value)
                    .iter()
                    .any(
                        |at| {
                            let insns = &body.blocks[at.0 as usize].insns;
                            matches!(
                                _plain_move(&insns[at.1 as usize]),
                                Some((into, _)) if into == value
                            ) && insns.get(at.1 as usize + 1).is_some()
                        },
                    ),
            )
    });
    if !possible {
        return Ok((Cow::Borrowed(body), fresh));
    }
    let (made, next) = _short_update_runs_whole(body, stored, frame, fresh)?;
    Ok((Cow::Owned(made), next))
}

fn _short_update_runs_whole(
    body: &LirBody,
    stored: &BTreeSet<u32>,
    frame: &mut Frame,
    fresh: u32,
) -> Result<(LirBody, u32), Error> {
    let (index, live) = match crate::backend::live::held(body) {
        Some(held) => (Arc::clone(held.index()), Shared::Held(held)),
        None => {
            let index = ranges::indexed_shared(body);
            let live = Shared::Worked(ranges::intervals_shared(body, Some(&index)));
            (index, live)
        }
    };
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
            let after = Segment { start: ranges::def_point(slot), end: ranges::def_point(slot) + 1 };
            let eligible = stored.contains(&into)
                && !stored.contains(&outof)
                && !live.get(&outof).expect("live").segments.iter().any(|segment| segment.overlaps(&after))
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
            position += 2;
        }
        blocks.push(block.with_insns(insns));
    }
    Ok((body.with_blocks(blocks), fresh))
}

/// An update of a spilled value whose result is read again in its block,
/// as LLVM's local split: reloaded once, updated and read in a register,
/// and stored only if still live after its last read there. Spilled whole,
/// the update ran in memory and each read reloaded.
fn _local_updates<'b>(
    body: &'b LirBody,
    stored: &BTreeSet<u32>,
    frame: &mut Frame,
    fresh: u32,
) -> Result<(Cow<'b, LirBody>, u32), Error> {
    // An update defines a stored value and reads it.
    let possible = postings::following(body, |postings| {
        stored.iter().any(|&value| postings.defs(value).iter().any(|at| _at(body, *at).uses.contains(&value)))
    });
    if !possible {
        return Ok((Cow::Borrowed(body), fresh));
    }
    let (made, next) = _local_updates_whole(body, stored, frame, fresh)?;
    Ok((Cow::Owned(made), next))
}

fn _local_updates_whole(
    body: &LirBody,
    stored: &BTreeSet<u32>,
    frame: &mut Frame,
    mut fresh: u32,
) -> Result<(LirBody, u32), Error> {
    let (index, live) = match crate::backend::live::held(body) {
        Some(held) => (Arc::clone(held.index()), Shared::Held(held)),
        None => {
            let index = ranges::indexed_shared(body);
            let live = Shared::Worked(ranges::intervals_shared(body, Some(&index)));
            (index, live)
        }
    };
    let register_only = |one: &Insn| {
        one.group.is_none()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && one.clobbers_high.is_empty()
            && one.what.as_ref().is_some_and(|what| {
                !what.dests.iter().chain(&what.sources).any(|operand| matches!(operand, Loc::Mem(_)))
            })
    };
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut position = 0;
        while position < insns.len() {
            let update = Arc::clone(&insns[position]);
            let Some(&value) = update.defines.first() else {
                position += 1;
                continue;
            };
            let is_update = stored.contains(&value)
                && update.defines.len() == 1
                && update.uses.contains(&value)
                && register_only(&update)
                && update.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Binary | Operation::Unary));
            if !is_update {
                position += 1;
                continue;
            }
            // The reads that follow, up to a redefinition or anything a
            // register may not survive.
            let mut reads = Vec::new();
            for (at, one) in insns.iter().enumerate().skip(position + 1) {
                if one.defines.contains(&value) || !register_only(one) {
                    break;
                }
                if one.uses.contains(&value) {
                    reads.push(at);
                }
            }
            let Some(&last) = reads.last() else {
                position += 1;
                continue;
            };
            let slot = index.at[&key(&insns[last])];
            let after = Segment { start: ranges::def_point(slot), end: ranges::def_point(slot) + 1 };
            let kept = live.get(&value).expect("live").segments.iter().any(|segment| segment.overlaps(&after));
            // Worth it when it saves a memory operand: the update's own
            // and each read's, against one reload and perhaps one store.
            if reads.len() + 1 <= 1 + usize::from(kept) {
                position += 1;
                continue;
            }
            let width = _width(&update, value);
            let cell = frame.cell(value, width)?;
            let register = fresh;
            fresh += 1;
            let renamed = IndexMap::from_iter([(value, register)]);
            for at in &reads {
                insns[*at] = _renamed(&insns[*at], &renamed);
            }
            let mut run = vec![_reload(&update, register, &cell), _renamed(&update, &renamed)];
            if kept {
                run.push(_store(&update, register, &cell));
            }
            let length = run.len();
            insns.splice(position..=position, run);
            position += length;
        }
        blocks.push(block.with_insns(insns));
    }
    Ok((body.with_blocks(blocks), fresh))
}

/// A move between two spilled values that share one slot.
fn _identity(
    one: &Insn,
    stored: &BTreeSet<u32>,
    frame: &Frame,
) -> bool {
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
    postings::following(body, |postings| siblings_over(body, values, frame, fixed, postings))
}

thread_local! {
    static EXAMINED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many instructions this thread has looked at for the copies of a value,
/// for a test that a spill asks of the web it is in and not of every move in
/// the body.
pub fn examined() -> usize {
    EXAMINED.with(std::cell::Cell::get)
}

/// The values a plain move copies to or from `value`, from the instructions
/// that name it alone.
fn _copied_by(
    body: &LirBody,
    postings: &Postings,
    value: u32,
) -> BTreeSet<u32> {
    let mut at: Vec<At> = postings.defs(value).iter().chain(postings.uses(value)).copied().collect();
    at.sort_unstable();
    at.dedup();
    EXAMINED.with(|examined| examined.set(examined.get() + at.len()));
    at.iter()
        .filter_map(|one| _plain_move(_at(body, *one)))
        .filter(|pair| pair.0 != pair.1)
        .filter_map(|pair| {
            if pair.0 == value {
                Some(pair.1)
            } else if pair.1 == value {
                Some(pair.0)
            } else {
                None
            }
        })
        .collect()
}

/// The plain-move neighbours of the values asked about, worked out as they are
/// asked from the occurrences of each: a spill asks of one copy web, not of
/// every move in the body.
struct Adjacent<'b> {
    body: &'b LirBody,
    postings: &'b Postings,
    known: std::cell::RefCell<IndexMap<u32, BTreeSet<u32>>>,
}

impl Adjacent<'_> {
    fn of(
        &self,
        value: u32,
    ) -> BTreeSet<u32> {
        self.known.borrow_mut().entry(value).or_insert_with(|| _copied_by(self.body, self.postings, value)).clone()
    }

    fn has(
        &self,
        value: u32,
    ) -> bool {
        !self.of(value).is_empty()
    }
}

fn siblings_over(
    body: &LirBody,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
    fixed: &BTreeSet<u32>,
    postings: &Postings,
) -> Result<BTreeSet<u32>, Error> {
    let adjacency = llrm_support::debug::span("siblings adjacency");
    let adjacent = Adjacent { body, postings, known: Default::default() };
    if !values.iter().any(|one| adjacent.has(*one)) {
        return Ok(BTreeSet::new());
    }
    drop(adjacency);
    // Only the copy webs that hold a value of `values` are grown from: the
    // others are asked of by no one.
    let wanted: BTreeSet<u32> = {
        let mut web: BTreeSet<u32> = values.iter().copied().filter(|one| adjacent.has(*one)).collect();
        let mut work: Vec<u32> = web.iter().copied().collect();
        while let Some(one) = work.pop() {
            for next in adjacent.of(one) {
                if web.insert(next) {
                    work.push(next);
                }
            }
        }
        web
    };
    // A shared slot holds each member at every width it is used, not just
    // moved.
    let widths = llrm_support::debug::timed("siblings widths", || _widest_by(body, &wanted, postings));

    // Only pairs among the values of those webs are asked of.
    let near =
        llrm_support::debug::timed("siblings interference", || coalesce::_interference_among(body, Some(&wanted)));
    if llrm_support::env_set("LLRM_CHECK_SIBLINGS") {
        let whole = coalesce::_interference(body);
        for value in &wanted {
            let among = |graph: &coalesce::Graph| {
                graph
                    .get(value)
                    .map(|near| near.iter().filter(|one| wanted.contains(one)).collect::<BTreeSet<u32>>())
                    .unwrap_or_default()
            };
            assert!(
                among(&near) == among(&whole),
                "{}: the interference of value#{value} among its web differs from the whole graph's",
                body.name
            );
        }
        // The copies, the web and the widths from the occurrences are those of
        // a walk of the whole body.
        let mut everywhere: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            let Some(pair) = _plain_move(one) else { continue };
            if pair.0 != pair.1 {
                everywhere.entry(pair.0).or_default().insert(pair.1);
                everywhere.entry(pair.1).or_default().insert(pair.0);
            }
        }
        for value in wanted.iter().chain(values) {
            assert!(
                adjacent.of(*value) == everywhere.get(value).cloned().unwrap_or_default(),
                "{}: the copies of value#{value} from its occurrences differ from a walk of the body",
                body.name
            );
        }
        assert!(
            widths.iter().eq(_widest(body, &wanted).iter()),
            "{}: the widths of a copy web from its occurrences differ from a walk of the body",
            body.name
        );
    }
    let deep = llrm_support::debug::timed("siblings depths", || ranges::depths(body));
    let occurring = llrm_support::debug::span("siblings occurs");
    let mut occurs: IndexMap<u32, Vec<(f64, Arc<Insn>)>> = IndexMap::default();
    for &value in &wanted {
        let mut at: Vec<At> = postings.defs(value).iter().chain(postings.uses(value)).copied().collect();
        at.sort_unstable();
        at.dedup();
        if at.is_empty() {
            continue;
        }
        let each = |block: u32| ranges::level(deep.get(&body.blocks[block as usize].at).copied().unwrap_or(0));
        occurs.insert(value, at.iter().map(|one| (each(one.0), Arc::clone(_at(body, *one)))).collect());
    }
    if llrm_support::env_set("LLRM_CHECK_SIBLINGS") {
        let mut everywhere: IndexMap<u32, Vec<(f64, Arc<Insn>)>> = IndexMap::default();
        for block in &body.blocks {
            let each = ranges::level(deep.get(&block.at).copied().unwrap_or(0));
            for one in &block.insns {
                let named: BTreeSet<u32> = one.defines.iter().chain(&one.uses).copied().collect();
                for value in wanted.intersection(&named) {
                    everywhere.entry(*value).or_default().push((each, Arc::clone(one)));
                }
            }
        }
        assert!(
            occurs.len() == everywhere.len()
                && occurs
                    .iter()
                    .all(|(value, found)| everywhere.get(value).is_some_and(|other| other.len() == found.len()
                        && other.iter().zip(found).all(|(a, b)| a.0 == b.0 && Arc::ptr_eq(&a.1, &b.1)))),
            "{}: the occurrences of a copy web from the postings differ from a walk of the body",
            body.name
        );
    }
    drop(occurring);
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
    let firsts: Vec<u32> = values.iter().copied().filter(|one| adjacent.has(*one)).collect();
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
                .flat_map(|one| adjacent.of(*one))
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
    let (mut colors, live) = _existing_colors(body, frame);
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
    // After a second return from `setjmp` a slot another value used holds that
    // value: none is shared.
    if body.returns_twice {
        for value in pending {
            frame.slot(value, widths[&value])?;
        }
        return Ok(());
    }
    pending.sort_by_key(|value| (-i64::from(widths[value].max(WORD)), *value));
    let copies = _copied_with(body, &pending.iter().copied().collect());
    for value in pending {
        let width = widths[&value];
        let capacity = width.max(WORD);
        let Some(interval) = live.get(&value) else {
            frame.slot(value, width)?;
            continue;
        };
        // A slot the value is copied to or from makes that copy vanish.
        let partners: Vec<i64> = copies
            .get(&value)
            .into_iter()
            .flatten()
            .filter_map(|partner| match partner {
                Err(home) => Some(*home),
                Ok(other) => frame.slots.get(&SlotKey::from(*other)).copied(),
            })
            .collect();
        let color = crate::backend::slots::choose(&colors, interval, capacity, &partners);
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

/// What each of `values` is copied to or from: another value, or a frame cell's
/// home. Only the instructions that name one of `values` are looked at: those
/// the postings of the body give, not every instruction of it.
fn _copied_with(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Vec<Result<u32, i64>>> {
    let mut at: Vec<At> = postings::following(body, |postings| {
        values.iter().flat_map(|value| postings.defs(*value).iter().chain(postings.uses(*value))).copied().collect()
    });
    at.sort_unstable();
    at.dedup();
    let found = _copied_among(
        at.iter().map(|&(block, position)| &body.blocks[block as usize].insns[position as usize]),
        values,
    );
    if check_postings() {
        let walked = _copied_among(body.blocks.iter().flat_map(|block| &block.insns), values);
        assert!(found == walked, "{}: the copies of the values from the postings differ from the walk", body.name);
    }
    found
}

fn _copied_among<'a>(
    insns: impl Iterator<Item = &'a Arc<Insn>>,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Vec<Result<u32, i64>>> {
    let mut copies: IndexMap<u32, Vec<Result<u32, i64>>> = IndexMap::default();
    let side = |loc: &Loc| match loc {
        Loc::Held(held) => Some(Ok(held.value)),
        Loc::Mem(Mem { addr: Some(addr), base: None, index: None, .. }) if addr.space == Space::Frame => {
            Some(Err(addr.disp))
        }
        _ => None,
    };
    for one in insns {
        let Some(what) = &one.what else { continue };
        let ([into], [from]) = (what.dests.as_slice(), what.sources.as_slice()) else { continue };
        if what.op != Operation::Move || what.name.as_deref() != Some("mov") {
            continue;
        }
        let (Some(into), Some(from)) = (side(into), side(from)) else { continue };
        for (this, other) in [(into, from), (from, into)] {
            if let Ok(value) = this {
                if values.contains(&value) {
                    copies.entry(value).or_default().push(other);
                }
            }
        }
    }
    copies
}

/// Spill-slot colors already present in the current rewritten body, and
/// every value's interval: one liveness pass answers both.
///
/// Python numbers each home's pseudo-value below every held value, which is
/// negative; `u32` cannot say that, so they are numbered in the same order
/// at the top of the range instead. Nothing compares them with a real value.
fn _existing_colors(
    body: &LirBody,
    frame: &mut Frame,
) -> (Vec<(i64, u32, Vec<Interval>)>, Lives) {
    let found = _existing_colors_by(body, frame, false);
    if llrm_support::env_set("LLRM_CHECK_COLORS") {
        let whole = _existing_colors_by(body, frame, true);
        assert!(_same_colors(&found.0, &whole.0), "{}: the slot colors differ from working them out whole", body.name);
        // What is asked of a value is where it is live (`slots::fits` overlaps
        // segments). Its weight is not read, and counts a value an
        // instruction names twice once more than the body made of the homes
        // does.
        assert!(
            found
                .1
                .iter()
                .all(|(value, interval)| whole.1.get(value).is_some_and(|other| other.segments == interval.segments)),
            "{}: where a value is live differs from working it out whole",
            body.name
        );
        assert!(
            found.1.len() == whole.1.len(),
            "{}: a value has no interval where working it out whole gives one",
            body.name
        );
    }
    found
}

/// Whether two sets of slot colors are the same where each home is live: the
/// weights of the occupants are not read.
fn _same_colors(
    one: &[(i64, u32, Vec<Interval>)],
    other: &[(i64, u32, Vec<Interval>)],
) -> bool {
    one.len() == other.len()
        && one.iter().zip(other).all(|(left, right)| {
            left.0 == right.0
                && left.1 == right.1
                && left.2.len() == right.2.len()
                && left.2.iter().zip(&right.2).all(|(a, b)| a.value == b.value && a.segments == b.segments)
        })
}

// `_existing_colors`, with the intervals of the homes' pseudo-values found
// among themselves and the body's own remembered, or, `whole`, as the body
// with the homes in it is worked out at once.
thread_local! {
    static MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many instructions this thread has made again with the homes among their
/// values, for a test that the usual ask does not.
pub fn made_for_homes() -> usize {
    MADE.with(std::cell::Cell::get)
}

/// The intervals `_existing_colors` reads: the body's own, as remembered and
/// shared, and the homes' beside them.
struct Lives {
    shared: Option<Shared>,
    own: IndexMap<u32, Interval>,
}

/// The body's intervals: the allocator's, where it holds them for this body,
/// else worked out and remembered.
enum Shared {
    Held(std::sync::Arc<crate::backend::live::LiveRanges>),
    Worked(std::sync::Arc<IndexMap<u32, Interval>>),
}

impl Shared {
    fn of(
        body: &LirBody,
        index: Option<&ranges::Indexes>,
    ) -> Self {
        match crate::backend::live::held(body) {
            Some(live) => Self::Held(live),
            None => Self::Worked(ranges::intervals_shared(body, index)),
        }
    }

    fn get(
        &self,
        value: &u32,
    ) -> Option<&Interval> {
        match self {
            Self::Held(live) => live.get(value),
            Self::Worked(map) => map.get(value),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Held(live) => live.len(),
            Self::Worked(map) => map.len(),
        }
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&u32, &Interval)> + '_> {
        match self {
            Self::Held(live) => Box::new(live.iter()),
            Self::Worked(map) => Box::new(map.iter()),
        }
    }
}

impl Lives {
    fn get(
        &self,
        value: &u32,
    ) -> Option<&Interval> {
        self.own.get(value).or_else(|| self.shared.as_ref().and_then(|shared| shared.get(value)))
    }

    fn len(&self) -> usize {
        self.own.len() + self.shared.as_ref().map_or(0, |shared| shared.len())
    }

    fn iter(&self) -> impl Iterator<Item = (&u32, &Interval)> + '_ {
        self.own.iter().chain(self.shared.iter().flat_map(|shared| shared.iter()))
    }
}

/// The intervals of the homes' pseudo-values (`first..`) walked in a body of
/// the instructions that name them alone (whole parallel copies, which share a
/// point), at the slots they have in the body.
fn homes_by_sparse_body(
    body: &LirBody,
    index: &ranges::Indexes,
    changed: &[(usize, usize, Arc<Insn>)],
    first: u32,
) -> IndexMap<u32, Interval> {
    let sparse_blocks: Vec<LirBlock> = body
        .blocks
        .iter()
        .map(|block| LirBlock {
            at: block.at,
            insns: Vec::new().into(),
            succ: block.succ.clone(),
            phis: block.phis.clone(),
            cold: block.cold,
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
struct HomesHeld(Vec<Arc<HomesState>>);

struct HomesState {
    structure: Arc<HomesStructure>,
    insns: Vec<crate::model::lir::Insns>,
    index: Arc<ranges::Indexes>,
    intervals: IndexMap<i64, Interval>,
    known: BTreeSet<i64>,
    count: usize,
}

/// How many homes' intervals this body's facts have found afresh from their
/// occurrences, for a test that an edit finds those it touched only.
pub fn homes_redone(body: &LirBody) -> usize {
    body.facts.0.counted("homes-redone")
}

/// An earlier body's homes are shifted only while the instructions that changed
/// are fewer than those that name a home: past that, looking at them costs more
/// than finding the homes again (cells N=448: 895 Minstr kept, 316 never kept;
/// d_faces the other way).
const FACTOR: usize = 1;

/// Whether the homes' intervals of a body are worth keeping for the next:
/// keeping them costs a fixed `FIXED` instructions a call (the state, the
/// lookups) and `KEEP` for each block of the body, and saves, for each segment
/// the intervals hold, what finding it from the occurrences cost (`WALK`) less
/// shifting it (`SHIFT`). Measured: QCport d_faces found 5.2 M segments in 2.05
/// G (390 each) and shifted 6.1 M in 0.83 G (136 each); x_transpose, 23 calls
/// of 41 intervals, lost 1.4 M kept (60 000 a call); a block of state is about
/// 40.

fn worth_keeping(
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
struct HomesStructure {
    entry: i64,
    blocks: Vec<(i64, Vec<i64>, Vec<crate::model::lir::Phi>)>,
    odds: crate::model::lir::BlockOdds,
    trips: Vec<(i64, i64)>,
}

impl HomesStructure {
    fn is_of(
        &self,
        body: &LirBody,
    ) -> bool {
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

    fn of(body: &LirBody) -> Self {
        Self {
            entry: body.entry,
            blocks: body.blocks.iter().map(|block| (block.at, block.succ.clone(), block.phis.clone())).collect(),
            odds: body.odds.clone(),
            trips: body.loop_trip_counts.clone(),
        }
    }
}

/// The frame cells `one` names: their displacement, width and whether it
/// writes them.
fn frame_cells(one: &Insn) -> impl Iterator<Item = (i64, u32, bool)> + '_ {
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
enum Names<'a> {
    Insn(&'a Insn),
}

/// An instruction that names a home.
struct Named<'a> {
    block: usize,
    at: usize,
    names: Names<'a>,
}

impl Named<'_> {
    /// The values it defines and reads among the homes `wanted` allows (the
    /// pseudo-value of a home is `first` and its place among `homes`).
    fn sets(
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
fn homes_kept(
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
        let tuples: Vec<(usize, usize, BTreeSet<u32>, BTreeSet<u32>)> = named
            .iter()
            .filter_map(|one| {
                let (defined, used) = one.sets(homes, first, &|disp| disps.binary_search(&disp).is_ok());
                (!defined.is_empty() || !used.is_empty()).then_some((one.block, one.at, defined, used))
            })
            .collect();
        crate::analysis::occurrences::Occurrences::planned(&tuples).ranges(body, index, &values)
    };
    let kept = body.facts.0.stash(|held: &mut HomesHeld| {
        held.0
            .iter()
            .filter(|state| state.structure.is_of(body))
            .min_by_key(|state| {
                state.insns.iter().zip(&body.blocks).filter(|(insns, block)| !insns.same_insns(&block.insns)).count()
            })
            .cloned()
    });
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
    if kept.is_some() && llrm_support::env_set("LLRM_CHECK_OCCURRENCES") {
        assert!(
            result == fresh_for(&(0..homes.len()).collect::<Vec<_>>()),
            "{}: the homes' intervals kept from an earlier body differ from finding them afresh",
            body.name
        );
    }
    let structure = match &kept {
        Some(state) if state.structure.is_of(body) => Arc::clone(&state.structure),
        _ => Arc::new(HomesStructure::of(body)),
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

fn _existing_colors_by(
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
            for &at in postings.frames(block_index) {
                let one = &block.insns[at as usize];
                let mut names = false;
                for (disp, width, _) in frame_cells(one) {
                    if let Some(had) = capacities.get_mut(&disp) {
                        *had = (*had).max(width).max(WORD);
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
                named.push(Named { block: block_index, at: at as usize, names: Names::Insn(one) });
            }
        }
    });
    if llrm_support::env_set("LLRM_CHECK_NAMED") {
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
    if whole || flipped || llrm_support::env_set("LLRM_CHECK_RANGES") {
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
        if llrm_support::env_set("LLRM_CHECK_OCCURRENCES") {
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

/// The instruction that remakes `value` anywhere: its only definition, when
/// that reads nothing (a constant or an address).
pub fn recomputed(
    body: &LirBody,
    value: u32,
) -> Option<Arc<Insn>> {
    let only = BTreeSet::from([value]);
    // The value's own occurrences, from the postings the body is followed with,
    // not a walk of every instruction (a carve asks this of its value).
    let defining: Vec<Arc<Insn>> = postings::following(body, |postings| {
        if !_literals_by(body, &only, false, postings).contains_key(&value)
            && !_addresses_by(body, &only, postings).contains_key(&value)
        {
            return None;
        }
        let mut at: Vec<At> = postings.defs(value).to_vec();
        at.dedup();
        Some(at.iter().map(|at| Arc::clone(_at(body, *at))).collect())
    })?;
    body.facts.0.bump_by("recomputed-definitions", defining.len());
    if check_postings() {
        let walked: Vec<Arc<Insn>> = body
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .filter(|one| one.defines.contains(&value))
            .cloned()
            .collect();
        assert!(
            walked.len() == defining.len() && walked.iter().zip(&defining).all(|(a, b)| Arc::ptr_eq(a, b)),
            "{}: the definitions of value {value} from the postings differ from the walk",
            body.name
        );
    }
    let one = _one_definition(&defining)?;
    let alone = one.defines == [value]
        && one.uses.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.group.is_none();
    alone.then(|| Arc::clone(one))
}

/// Each of `values` that one plain same-width `mov` copies from another value,
/// and that value: made once, so the copy is made again as its source is (the
/// one answer SsaSpill and the allocator both read).
pub fn _copies(
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

/// `found` (made for the values and their copies' sources) for `values`: a copy
/// takes its source's.
fn _through_copies<T: Clone>(
    found: IndexMap<u32, T>,
    values: &BTreeSet<u32>,
    copies: &IndexMap<u32, u32>,
) -> IndexMap<u32, T> {
    values
        .iter()
        .filter_map(|value| {
            found
                .get(value)
                .or_else(|| copies.get(value).and_then(|source| found.get(source)))
                .map(|one| (*value, one.clone()))
        })
        .collect()
}

thread_local! {
    static FOLDED_READ_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many bodies `_folded_reads` has read on this thread.
pub fn folded_read_scans() -> usize {
    FOLDED_READ_SCANS.with(std::cell::Cell::get)
}

/// Each of `values` that is loaded from a cell holding until its one reader,
/// which runs no more often than the load (frequencies are products of floats:
/// two blocks that run alike differ in the last digits) and takes that cell as
/// its memory operand: read there it costs the same one memory operand and no
/// instruction, and no register is held from the load to it, so holding it
/// never pays.
pub fn _folded_reads(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    FOLDED_READ_SCANS.with(|count| count.set(count.get() + 1));
    let stable = _stable_loads(body, values);
    if stable.is_empty() {
        return BTreeSet::new();
    }
    let busy = crate::analysis::frequency::Frequency::of(body);
    let mut readers: IndexMap<u32, Vec<(i64, &Arc<Insn>)>> = IndexMap::default();
    let mut loads: IndexMap<u32, i64> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in one.uses.iter().filter(|value| stable.contains_key(*value)) {
                readers.entry(*value).or_default().push((block.at, one));
            }
            for value in one.defines.iter().filter(|value| stable.contains_key(*value)) {
                loads.insert(*value, block.at);
            }
        }
    }
    // A cell the body also writes is a variable: its loads and stores share
    // frame homes and slots with other values.
    let written: Vec<Mem> = body
        .insns()
        .iter()
        .flat_map(|one| one.what.iter().flat_map(|what| what.dests.iter()))
        .filter_map(|place| if let Loc::Mem(cell) = place { Some(cell.clone()) } else { None })
        .collect();
    stable
        .iter()
        .filter(|(value, cell)| {
            if written.iter().any(|dest| {
                dest.addr.is_none()
                    || (dest.addr.zip(cell.addr).is_some_and(|(there, own)| there.space == own.space)
                        && crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width))
            }) {
                return false;
            }
            let ([(at, one)], Some(loaded)) =
                (readers.get(*value).map(Vec::as_slice).unwrap_or_default(), loads.get(*value))
            else {
                return false;
            };
            let Some(what) = &one.what else { return false };
            let held = |place: &Loc| matches!(place, Loc::Held(held) if held.value == **value);
            let (read, written) = (
                what.sources.iter().filter(|place| held(place)).count(),
                what.dests.iter().filter(|place| held(place)).count(),
            );
            if one.group.is_some()
                || read != 1
                || written != 0
                || one.defines.contains(value)
                || busy.block(*at) > busy.block(*loaded) * (1.0 + 1e-9)
            {
                return false;
            }
            // The forms a reader takes a memory operand in: the spiller's own
            // folds, and the push that peephole folds.
            let only = BTreeSet::from([**value]);
            let pushed = what.op == Operation::Push
                && what.name.as_deref() == Some("push")
                && one.defines.is_empty()
                && one.requires.is_empty()
                && one.delivers.is_empty();
            // A copy is the coalescer's: the load it copies is the load it
            // becomes.
            pushed
                || (what.op != Operation::Move
                    && folded_source_in(one, &only, false).is_some_and(|folded| folded.value == **value))
        })
        .map(|(value, _)| *value)
        .collect()
}

/// Values loaded from a cell nothing changes before they are used again.
pub fn _stable_loads(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Mem> {
    _stable_loads_through(body, values, &IndexMap::default())
}

/// `_stable_loads_through` with the postings of the body to hand: where the
/// values occur is read from them, not found by a scan.
pub fn _stable_loads_by(
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
pub fn _stable_loads_through(
    body: &LirBody,
    values: &BTreeSet<u32>,
    copies: &IndexMap<u32, u32>,
) -> IndexMap<u32, Mem> {
    _stable_loads_from(body, values, copies, None)
}

fn _stable_loads_from(
    body: &LirBody,
    values: &BTreeSet<u32>,
    copies: &IndexMap<u32, u32>,
    postings: Option<&Postings>,
) -> IndexMap<u32, Mem> {
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
    if postings.is_some() && llrm_support::env_set("LLRM_CHECK_OCCURRENCES") {
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
        let flow = flow.get_or_init(|| Flow::of(body));
        let held = if _exact_frame(cell) {
            // Only what may write the cell is asked of, and the blocks between
            // the load and its uses are gone over.
            let writers = writers.get_or_init(|| Writers::of(body));
            let mut events: crate::support::hash::HashMap<usize, Vec<(usize, bool)>> =
                crate::support::hash::HashMap::default();
            for (block, position) in writers.near(cell) {
                if !_keeps(&body.blocks[block].insns[position], define, cell, true, body) {
                    events.entry(block).or_default().push((position, false));
                }
            }
            events.entry(define_at.0).or_default().push((define_at.1, true));
            for list in events.values_mut() {
                list.sort_unstable();
            }
            _holds_at(flow, &events, &use_places[value])
        } else {
            _unchanged(body, flow, define, cell, &uses[value])
        };
        if llrm_support::env_set("LLRM_CHECK_UNCHANGED") {
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

// (On one line: tests/target_facts.rs reads what comes before a line that is
// only the attribute as the code.)
#[cfg(test)]
thread_local! { static KEEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

/// The blocks of a body by address, and each one's predecessors.
struct Flow<'a> {
    predecessors: IndexMap<i64, Vec<i64>>,
    blocks: IndexMap<i64, &'a LirBlock>,
    /// The same by position in `body.blocks`.
    parents: Vec<Vec<usize>>,
    entry: usize,
}

impl<'a> Flow<'a> {
    fn of(body: &'a LirBody) -> Self {
        let predecessors = _predecessors(body);
        let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
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
struct Writers {
    /// (block, position) of what may write memory no exact frame cell names: a
    /// call, an unmodeled write, a store to a based, indexed or unplaced
    /// cell.
    wild: Vec<(usize, usize)>,
    /// The stores to an exact frame cell, by its first byte.
    exact: std::collections::BTreeMap<i64, Vec<(usize, usize)>>,
    /// The widest of those cells.
    widest: i64,
}

impl Writers {
    fn of(body: &LirBody) -> Self {
        let mut found = Self { wild: Vec::new(), exact: Default::default(), widest: 1 };
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
                for cell in dests {
                    if _exact_frame(cell) {
                        let addr = cell.addr.expect("an exact frame cell has an address");
                        found.exact.entry(addr.disp).or_default().push((block, position));
                        found.widest = found.widest.max(i64::from(cell.width));
                    } else {
                        wild = true;
                    }
                }
                if wild {
                    found.wild.push((block, position));
                }
            }
        }
        found
    }

    /// The instructions that can fail to keep `cell` (an exact frame cell): any
    /// other writes nothing but exact frame cells that do not meet it, and
    /// keeps it, as `_keeps` finds.
    fn near(
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
}

/// Whether the cell holds at every one of `uses` (block, position), given where
/// it is made to hold and where it is written: `events` per block are
/// (position, true for the load, false for a write) in order. The same answer
/// as `_unchanged`, found over the blocks that lie between the load and the
/// uses and not over the whole body.
fn _holds_at(
    flow: &Flow,
    events: &crate::support::hash::HashMap<usize, Vec<(usize, bool)>>,
    uses: &[(usize, usize)],
) -> bool {
    use crate::support::hash::{HashMap, HashSet};
    // Whether the cell holds out of a block that has an event: after its last.
    let decided = |block: usize| events.get(&block).and_then(|list| list.last()).map(|(_, load)| *load);
    let mut needed: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::default();
    for &(block, position) in uses {
        let before = events.get(&block).and_then(|list| list.iter().rev().find(|(at, _)| *at < position));
        if before.is_none() && seen.insert(block) {
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
    uses.iter().all(|&(block, position)| {
        match events.get(&block).and_then(|list| list.iter().rev().find(|(at, _)| *at < position)) {
            Some((_, load)) => *load,
            None => held[&block],
        }
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
            let overlaps = crate::backend::overlap::may_overlap(cell.addr, cell.width, dest.addr, dest.width);
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

fn _in_frame(cell: &Mem) -> bool {
    cell.addr.is_some_and(|addr| addr.space == Space::Frame)
}

/// The memory this instruction names as written that could be `cell`.
fn _written<'a>(
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
///
/// A block takes the cell in as it leaves its predecessors alike and, one
/// instruction after another, keeps it unless that instruction may write it:
/// `_keeps(one, ..., holds) = holds && K(one)` for every instruction but the
/// load, which makes it hold. So a block's answer is that of its instructions
/// after its last load, or of all of them: asked of each instruction once, not
/// once for each time the blocks are gone over until they settle.
fn _unchanged(
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
fn _unchanged_reference(
    body: &LirBody,
    define: &Arc<Insn>,
    cell: &Mem,
    uses: &[Arc<Insn>],
) -> bool {
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

/// Stable native arguments that may be loaded again at each use.
fn _frame_loads(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Mem> {
    let regs = body.regs();
    let pinned = &body.pins;
    let candidates: BTreeSet<u32> = values
        .iter()
        .copied()
        .filter(|value| pinned.get(value).is_some_and(|register| regs.is_segment(*register)))
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
                        if let ([Loc::Held(dest)], [Loc::Mem(source)]) =
                            (what.dests.as_slice(), what.sources.as_slice())
                        {
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
            if _may_write(one, source, body.sealed_arguments) {
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

/// `_remove_abandoned` of a body in which only the blocks in `marked` hold an
/// abandoned instruction: the others are left as they are, and what is read is
/// asked of the occurrences, not of a scan of the body.
fn _remove_abandoned_in(
    body: LirBody,
    abandoned: &BTreeSet<usize>,
    marked: &BTreeSet<usize>,
) -> LirBody {
    if abandoned.is_empty() {
        return body;
    }
    let reference = check_postings().then(|| _remove_abandoned(&body, abandoned));
    // What else reads a value: a block's arrivals and a phi's inputs.
    let mut elsewhere: BTreeSet<u32> = body.blocks.iter().flat_map(LirBlock::arrives).collect();
    elsewhere.extend(
        body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)),
    );
    let used = |value: u32, postings: &Postings| {
        !postings.uses(value).is_empty() || !postings.needs(value).is_empty() || elsewhere.contains(&value)
    };
    let result = postings::following(&body, |postings| {
        let removable =
            |one: &Arc<Insn>| abandoned.contains(&key(one)) && !one.defines.iter().any(|value| used(*value, postings));
        let anchor = |one: Arc<Insn>| -> Arc<Insn> {
            if !removable(&one) {
                return one;
            }
            _with(&one, |made| {
                made.what = Some(lir::inert());
                made.defines = Vec::new();
                made.uses = Vec::new();
                made.widths = Vec::new();
            })
        };
        let mut blocks = body.blocks.clone();
        for &at in marked {
            blocks[at] = blocks[at].with_insns(
                lir::without(&blocks[at].insns, removable, None::<fn(&Arc<Insn>) -> Arc<Insn>>)
                    .into_iter()
                    .map(anchor)
                    .collect(),
            );
        }
        body.with_blocks(blocks)
    });
    if let Some(reference) = reference {
        assert!(
            result.blocks == reference.blocks,
            "{}: the abandoned instructions removed using the postings differ",
            result.name
        );
    }
    result
}

pub fn _remove_abandoned(
    body: &LirBody,
    abandoned: &BTreeSet<usize>,
) -> LirBody {
    if abandoned.is_empty() {
        return body.clone();
    }
    let every = || body.blocks.iter().flat_map(|block| &block.insns);
    let mut used: BTreeSet<u32> = every().flat_map(|one| one.uses.iter().copied()).collect();
    used.extend(every().flat_map(|one| one.requires.iter().map(|(held, _)| held.value)));
    used.extend(body.blocks.iter().flat_map(LirBlock::arrives));
    used.extend(
        body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)),
    );

    let removable =
        |one: &Arc<Insn>| abandoned.contains(&key(one)) && !one.defines.iter().any(|value| used.contains(value));

    let anchor = |one: Arc<Insn>| -> Arc<Insn> {
        if !removable(&one) {
            return one;
        }
        _with(&one, |made| {
            made.what = Some(lir::inert());
            made.defines = Vec::new();
            made.uses = Vec::new();
            made.widths = Vec::new();
        })
    };

    body.with_blocks(
        body.blocks
            .iter()
            .map(|block| {
                block.with_insns(
                    lir::without(&block.insns, removable, None::<fn(&Arc<Insn>) -> Arc<Insn>>)
                        .into_iter()
                        .map(anchor)
                        .collect(),
                )
            })
            .collect(),
    )
}

/// Where a rebuilt value already is, in the shape `_source` asks a frame.
pub struct _Cells {
    _cells: IndexMap<u32, Mem>,
}

impl _Cells {
    pub fn new(cells: IndexMap<u32, Mem>) -> Self {
        Self { _cells: cells }
    }

    pub fn cell(
        &self,
        value: u32,
        width: u32,
    ) -> Option<Mem> {
        self._cells.get(&value).filter(|found| found.width == width).cloned()
    }
}

impl CellOf for _Cells {
    fn cell_of(
        &mut self,
        value: u32,
        width: u32,
    ) -> Result<Option<Mem>, Error> {
        Ok(self.cell(value, width))
    }
}

/// Every `(id(insn), value)` where that instruction is the value's last read.
///
/// A destructive index fold needs the base dead after the access. Counting
/// static uses cannot say so: sum_three's loop-invariant base had one use,
/// inside the loop, and `add di,[slot]` moved it on every trip -- 330 for
/// 1110.
pub(crate) fn _final_uses(body: &LirBody) -> BTreeSet<(usize, u32)> {
    use crate::backend::allocate;
    FINALS.with(|runs| runs.set(runs.get() + 1));

    // Only what leaves each block is read: rows, not the sets of every block's
    // entry and exit.
    let rows = allocate::live_rows(body);
    let mut out: BTreeSet<(usize, u32)> = BTreeSet::new();
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> = rows.leaving(block.at).collect();
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

thread_local! {
    static FINALS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LOOKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks this thread has rewritten instruction by instruction, for a
/// test that a block naming none of the values spilled is not looked at.
pub fn blocks_looked_at() -> usize {
    LOOKED.with(std::cell::Cell::get)
}

/// How many times this thread has worked out the final uses of a body, for a
/// test that a spill nothing folds does not.
pub fn final_use_runs() -> usize {
    FINALS.with(std::cell::Cell::get)
}

/// `_final_uses` of a body, worked out when first asked: only an index fold
/// reads it, and it takes the liveness of the whole body, for each spill.
pub(crate) struct FinalUses<'a> {
    body: &'a LirBody,
    found: std::cell::OnceCell<BTreeSet<(usize, u32)>>,
}

impl<'a> FinalUses<'a> {
    pub(crate) fn new(body: &'a LirBody) -> Self {
        Self { body, found: std::cell::OnceCell::new() }
    }

    fn contains(
        &self,
        pair: &(usize, u32),
    ) -> bool {
        self.found.get_or_init(|| _final_uses(self.body)).contains(pair)
    }
}

/// Spilled word indexes that can become a direct frame add.
pub fn foldable_indexes(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    let r#final = FinalUses::new(body);
    body.blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter_map(|one| _indexed_pattern(one, values, &r#final))
        .map(|found| found.1.value)
        .collect()
}

/// Expose dying-base address adds so indexes may use any word register.
pub fn unfolded_indexes(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> (LirBody, BTreeSet<u32>) {
    if values.is_empty() {
        return (body.clone(), BTreeSet::new());
    }
    let r#final = FinalUses::new(body);
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
        if matches!(
            what.op,
            Operation::Branch | Operation::Jump | Operation::Call | Operation::Return
        ) {
            return false;
        }
        if what.name.as_deref().is_some_and(crate::backend::peephole::_reads_flags) {
            return false;
        }
        if matches!(
            what.name.as_deref(),
            Some("add" | "sub" | "and" | "or" | "xor" | "cmp" | "test")
        ) {
            return true;
        }
    }
    false
}

/// The base, index and matching cells of one legal direct-index fold.
fn _indexed_pattern(
    one: &Arc<Insn>,
    values: &BTreeSet<u32>,
    r#final: &FinalUses<'_>,
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
fn _unfolded_index(
    one: &Insn,
    base: Held,
    index: Held,
) -> (Arc<Insn>, Arc<Insn>) {
    let rebased = |place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) if cell.base == Some(base) && cell.index == Some(index) => {
                Loc::Mem(Mem { index: None, scale: 1, index_through: Register::None, ..cell.clone() })
            }
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
    r#final: &FinalUses<'_>,
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

/// The spilled source arithmetic or a comparison reads as its memory operand,
/// needing no reload.
pub fn folded_source(
    one: &Insn,
    values: &BTreeSet<u32>,
) -> Option<Held> {
    folded_source_in(one, values, true)
}

/// `folded_source`, for an instruction that `tied` writes the register of its
/// first source (after two-address lowering) or, not tied, names its result
/// apart (SSA).
pub fn folded_source_in(
    one: &Insn,
    values: &BTreeSet<u32>,
    tied: bool,
) -> Option<Held> {
    folded_source_among(one, &|value| values.contains(&value), tied)
}

/// `folded_source_in` for the values `among` says are spilled, so that a caller
/// that asks of one value at a time builds no set to ask with.
pub fn folded_source_among(
    one: &Insn,
    among: &dyn Fn(u32) -> bool,
    tied: bool,
) -> Option<Held> {
    let (left, right) = folded_pair(one, tied)?;
    if !among(right.value)
        || one.uses.iter().any(|value| among(*value) && *value != left.value && *value != right.value)
    {
        return None;
    }
    Some(right)
}

/// The two values `one` could fold the second of into the first's operation,
/// whichever are spilled: what `folded_source_among` finds before it asks which
/// are. The second is the only value that can fold.
pub fn folded_pair(
    one: &Insn,
    tied: bool,
) -> Option<(Held, Held)> {
    if one.group.is_some() || !one.requires.is_empty() || !one.delivers.is_empty() || !one.clobbers.is_empty() {
        return None;
    }
    let what = one.what.as_ref()?;
    let mut widths: &[u32] = &[2, 4];
    let (left, right) = match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Binary, name, [Loc::Held(dest)], [Loc::Held(left), Loc::Held(right)]) => {
            if !matches!(name, Some("add" | "sub" | "and" | "or" | "xor")) || (tied && dest != left) {
                return None;
            }
            (*left, *right)
        }
        (Operation::Compare, Some("cmp"), [], [Loc::Held(left), Loc::Held(right)]) => (*left, *right),
        (Operation::Multiply, Some("imul"), [Loc::Held(dest)], [Loc::Held(left), Loc::Held(right)]) => {
            if tied && dest != left {
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
        || left.value == right.value
        || one.defines.contains(&right.value)
    {
        return None;
    }
    Some((left, right))
}

/// Fold one untied spill source into arithmetic or a comparison.
fn _source<F: CellOf>(
    one: &Insn,
    values: &BTreeSet<u32>,
    frame: &mut F,
) -> Result<Option<Arc<Insn>>, Error> {
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
fn _address_source(
    regs: Regs,
    one: &Insn,
    value: u32,
    address: &AddressRef,
) -> Option<Arc<Insn>> {
    let what = one.what.as_ref()?;
    let addr = address.addr?;
    if one.symbol == Some(true)
        || addr.space != Space::Frame
        || address.through != Register::BP
        || address.index_through != Register::None
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
                    && (found.segment == Register::None || regs.is_stack_segment(found.segment))
            });
        if !fits {
            invalid = true;
            return place.clone();
        }
        let displacement = addr.disp + cell.addr.expect("checked").disp;
        changed = true;
        Loc::Mem(Mem {
            // The folded address's slot: the cell is that frame address plus a
            // constant.
            addr: Some(Addr { index: addr.index, ..Addr::new(Space::Frame, displacement) }),
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
        match (what.dests.as_slice(), what.sources.as_slice()) {
            ([Loc::Held(dest)], [Loc::Held(source)])
                if dest.width == source.width && one.uses == [source.value] && one.defines == [dest.value] =>
            {
                return Some(*source);
            }
            // Into a spilled value's slot.
            ([Loc::Mem(dest)], [Loc::Held(source)])
                if dest.width == source.width && one.uses == [source.value] && one.defines.is_empty() =>
            {
                return Some(*source);
            }
            _ => {}
        }
    }
    None
}

/// The definition every one of `defining` repeats, if they all do the same.
/// A split remakes a value where each piece ends, so a remakeable value can
/// have several definitions that are one.
pub fn _one_definition(defining: &[Arc<Insn>]) -> Option<&Arc<Insn>> {
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
pub fn _constants(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Imm> {
    _literals(body, values, false)
}

fn check_postings() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_POSTINGS"))
}

fn _at<'b>(
    body: &'b LirBody,
    at: At,
) -> &'b Arc<Insn> {
    &body.blocks[at.0 as usize].insns[at.1 as usize]
}

/// `_copies`, from the occurrences of `values` and of their sources alone.
fn _copies_by(
    body: &LirBody,
    values: &BTreeSet<u32>,
    postings: &Postings,
) -> IndexMap<u32, u32> {
    let mut found: Vec<(At, u32, u32)> = Vec::new();
    for &value in values {
        let [at] = postings.defs(value) else { continue };
        let one = _at(body, *at);
        let (Some(what), [defined], [source]) = (&one.what, one.defines.as_slice(), one.uses.as_slice()) else {
            continue;
        };
        let ([Loc::Held(dest)], [Loc::Held(from)]) = (what.dests.as_slice(), what.sources.as_slice()) else { continue };
        if what.op == Operation::Move
            && one.group.is_none()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && what.name.as_deref() == Some("mov")
            && dest.value == *defined
            && from.value == *source
            && dest.width == from.width
            && *defined == value
            && postings.defs(*source).len() == 1
        {
            found.push((*at, value, *source));
        }
    }
    found.sort_unstable();
    let out: IndexMap<u32, u32> = found.into_iter().map(|(_, value, source)| (value, source)).collect();
    if check_postings() {
        assert!(out.iter().eq(_copies(body, values).iter()), "{}: the copies from the postings differ", body.name);
    }
    out
}

/// `_addresses`, from the definitions of `values` alone.
fn _addresses_by(
    body: &LirBody,
    values: &BTreeSet<u32>,
    postings: &Postings,
) -> IndexMap<u32, AddressRef> {
    let mut order: Vec<(At, usize, u32)> = Vec::new();
    for &value in values {
        let Some(&first) = postings.defs(value).first() else { continue };
        let at = body.blocks[first.0 as usize].insns[first.1 as usize]
            .defines
            .iter()
            .position(|one| *one == value)
            .unwrap_or(0);
        order.push((first, at, value));
    }
    order.sort_unstable();
    let mut result = IndexMap::default();
    for (_, _, value) in order {
        let defining: Vec<Arc<Insn>> = postings.defs(value).iter().map(|at| Arc::clone(_at(body, *at))).collect();
        if let Some(source) = _address_of(value, &defining) {
            result.insert(value, source);
        }
    }
    if check_postings() {
        assert!(
            result.iter().eq(_addresses(body, values).iter()),
            "{}: the addresses from the postings differ",
            body.name
        );
    }
    result
}

/// `_extensions`, from the occurrences of `values` and of their sources alone.
fn _extensions_by(
    body: &LirBody,
    values: &BTreeSet<u32>,
    postings: &Postings,
) -> IndexMap<u32, Arc<Insn>> {
    let mut result = IndexMap::default();
    for value in values {
        let found = postings.defs(*value);
        let mut consumers: Vec<At> = postings.uses(*value).to_vec();
        consumers.dedup();
        if found.len() != 1 || consumers.len() != 1 {
            continue;
        }
        let (block_index, insn_index) = found[0];
        let one = _at(body, found[0]);
        let consumer_at = consumers[0];
        let consumer = _at(body, consumer_at);
        if consumer.group.is_some() || consumer_at.0 != block_index || consumer_at.1 <= insn_index {
            continue;
        }
        let defs_of = |source: u32| -> Vec<(usize, usize)> {
            postings.defs(source).iter().map(|at| (at.0 as usize, at.1 as usize)).collect()
        };
        if let Some(extended) =
            _extension_of(*value, one, values, (block_index as usize, insn_index as usize), &defs_of)
        {
            result.insert(*value, extended);
        }
    }
    if check_postings() {
        assert!(
            result
                .iter()
                .map(|(value, one)| (*value, key(one)))
                .eq(_extensions(body, values).iter().map(|(value, one)| (*value, key(one)))),
            "{}: the extensions from the postings differ",
            body.name
        );
    }
    result
}

/// `_widest`, from the occurrences of `values` alone.
fn _widest_by(
    body: &LirBody,
    values: &BTreeSet<u32>,
    postings: &Postings,
) -> IndexMap<u32, u32> {
    let mut order: Vec<(At, u32, u32)> = Vec::new();
    for &value in values {
        let mut at: Vec<At> = postings.defs(value).iter().chain(postings.uses(value)).copied().collect();
        at.sort_unstable();
        at.dedup();
        let Some(&first) = at.first() else { continue };
        let width = at.iter().map(|one| _width(_at(body, *one), value)).max().unwrap_or(0);
        order.push((first, value, width));
    }
    order.sort_unstable();
    let out: IndexMap<u32, u32> = order.into_iter().map(|(_, value, width)| (value, width)).collect();
    if check_postings() {
        assert!(out.iter().eq(_widest(body, values).iter()), "{}: the widths from the postings differ", body.name);
    }
    out
}

/// `_literals` of the values in `values`, from their definitions and those of
/// what they are copies of: the answer for a value reads its own definition
/// and, for a copy, its source's, and so down the chain.
fn _literals_by(
    body: &LirBody,
    values: &BTreeSet<u32>,
    any_width: bool,
    postings: &Postings,
) -> IndexMap<u32, Imm> {
    enum Step {
        Visiting,
        Is(Option<Imm>),
    }
    // What a value is made from, where it is one single plain move of a
    // constant or of another value.
    let source_of = |value: u32| -> Option<Loc> {
        let defining: Vec<Arc<Insn>> = postings.defs(value).iter().map(|at| Arc::clone(_at(body, *at))).collect();
        if defining.is_empty() || defining.iter().any(|one| one.group.is_some() && (one.defines.contains(&value))) {
            return None;
        }
        if postings.uses(value).iter().any(|at| {
            let one = _at(body, *at);
            one.group.is_some() && _group_source(one).is_none()
        }) {
            return None;
        }
        let one = _one_definition(&defining)?;
        let what = one.what.as_ref()?;
        if what.op != Operation::Move
            || what.dests.len() != 1
            || what.sources.len() != 1
            || one.defines != [value]
            || !one.clobbers.is_empty()
        {
            return None;
        }
        let (into, source) = (&what.dests[0], &what.sources[0]);
        let Loc::Held(into) = into else { return None };
        let (source_width, source_uses) = match source {
            Loc::Imm(imm) => (imm.width, Vec::new()),
            Loc::Held(held) => (held.width, vec![held.value]),
            _ => return None,
        };
        let widest = postings.uses(value).iter().map(|at| _width(_at(body, *at), value)).max().unwrap_or(0);
        (into.width == source_width && one.uses == source_uses && (any_width || widest <= source_width))
            .then(|| source.clone())
    };
    let mut steps: IndexMap<u32, Step> = IndexMap::default();
    fn resolve(
        value: u32,
        source_of: &dyn Fn(u32) -> Option<Loc>,
        steps: &mut IndexMap<u32, Step>,
    ) -> Option<Imm> {
        match steps.get(&value) {
            Some(Step::Is(known)) => return known.clone(),
            Some(Step::Visiting) => return None,
            None => {}
        }
        steps.insert(value, Step::Visiting);
        let found = match source_of(value) {
            Some(Loc::Imm(imm)) => Some(imm),
            Some(Loc::Held(held)) => resolve(held.value, source_of, steps),
            _ => None,
        };
        steps.insert(value, Step::Is(found.clone()));
        found
    }
    let mut result: IndexMap<u32, Imm> = IndexMap::default();
    for &value in values {
        if let Some(constant) = resolve(value, &source_of, &mut steps) {
            result.insert(value, constant);
        }
    }
    if check_postings() {
        let reference = _literals(body, values, any_width);
        assert!(
            result.len() == reference.len() && result.iter().all(|(value, one)| reference.get(value) == Some(one)),
            "{}: the literals from the postings differ",
            body.name
        );
    }
    result
}

/// `_constants`, and also those some instruction reads wider than the literal:
/// those are remade only where read at its width.
fn _literals(
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
            if _group_source(one).is_none() {
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
pub fn _addresses(
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
fn _address_of(
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
            index_through: Register::None,
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
            addr.space == Space::Frame && source.through == Register::BP
                || matches!(addr.space, Space::Segment | Space::External) && source.through == Register::None
        })
        && source.index_through == Register::None)
        .then_some(source)
}

fn _extensions(
    body: &LirBody,
    values: &BTreeSet<u32>,
) -> IndexMap<u32, Arc<Insn>> {
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
        let defs_of = |source: u32| -> Vec<(usize, usize)> {
            definitions.get(&source).unwrap_or(&none).iter().map(|(block, at, _)| (*block, *at)).collect()
        };
        if let Some(extended) = _extension_of(*value, one, values, (*block_index, *insn_index), &defs_of) {
            result.insert(*value, extended);
        }
    }
    result
}

/// The instruction `value` is made again by, where it is the one definition of
/// a zero or sign extension of a value not spilled with it, read once after it
/// in its block, its source defined at most once before it.
fn _extension_of(
    value: u32,
    one: &Arc<Insn>,
    values: &BTreeSet<u32>,
    at: (usize, usize),
    definitions_of: &dyn Fn(u32) -> Vec<(usize, usize)>,
) -> Option<Arc<Insn>> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Extend || !matches!(what.name.as_deref(), Some("movsx" | "movzx")) {
        return None;
    }
    let ([Loc::Held(destination)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
        return None;
    };
    if destination.value == value
        && destination.width > source.width
        && one.defines == [value]
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
        let source_definitions = definitions_of(source.value);
        let unchanged = source_definitions.is_empty()
            || (source_definitions.len() == 1 && source_definitions[0].0 == at.0 && source_definitions[0].1 < at.1);
        if unchanged {
            return Some(Arc::clone(one));
        }
    }
    None
}

pub fn _widest(
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

/// How wide this instruction reads or writes the value, defaulting to a word.
pub fn _width(
    one: &Insn,
    value: u32,
) -> u32 {
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

fn _mov(
    into: Loc,
    out_of: Loc,
) -> Semantics {
    Semantics {
        name: Some("mov".to_owned()),
        dests: vec![into],
        sources: vec![out_of],
        ..Semantics::new(Operation::Move)
    }
}

/// The load that puts a spilled value back for one instruction.
pub fn _reload(
    beside: &Insn,
    into: u32,
    cell: &Mem,
) -> Arc<Insn> {
    let inserted = _inserted(
        beside,
        _mov(Loc::Held(Held { value: into, width: cell.width }), Loc::Mem(cell.clone())),
        vec![into],
        Vec::new(),
    );
    _with(&inserted, |made| made.spill_reload = true)
}

/// The store that puts a spilled value away as soon as it is written.
pub fn _store(
    beside: &Insn,
    out_of: u32,
    cell: &Mem,
) -> Arc<Insn> {
    let inserted = _inserted(
        beside,
        _mov(Loc::Mem(cell.clone()), Loc::Held(Held { value: out_of, width: cell.width })),
        Vec::new(),
        vec![out_of],
    );
    _with(&inserted, |made| made.spill_store = true)
}

/// An instruction that stands beside another and claims none of its bytes.
fn _inserted(
    beside: &Insn,
    what: Semantics,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = Insn::new(beside.at, Some((at, at)), Some(what), defines, uses);
    made.call = beside.call.clone();
    Arc::new(made)
}

/// A requirement, naming whichever value now feeds the instruction.
fn _wants(
    side: &[(Held, Register)],
    rename: &IndexMap<u32, u32>,
) -> Vec<(Held, Register)> {
    side.iter()
        .map(|(held, register)| {
            (Held { value: rename.get(&held.value).copied().unwrap_or(held.value), width: held.width }, *register)
        })
        .collect()
}

/// The instruction reading and writing the reload's value instead.
pub fn _renamed(
    one: &Insn,
    rename: &IndexMap<u32, u32>,
) -> Arc<Insn> {
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
fn _settled(
    place: &Loc,
    rename: &IndexMap<u32, u32>,
) -> Loc {
    ir::mapped(place, |one| Held { value: rename.get(&one.value).copied().unwrap_or(one.value), width: one.width })
}

/// One move of a parallel copy, with its spilled end read or written where it
/// lives.
fn _in_place(
    one: &Insn,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
) -> Result<Option<Arc<Insn>>, Error> {
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
            made.what =
                Some(Semantics { dests: vec![Loc::Mem(dest)], sources: vec![Loc::Mem(source)], ..what.clone() });
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
fn _memory_source_read_first(
    one: &Insn,
    values: &BTreeSet<u32>,
    fresh: u32,
) -> Option<(Arc<Insn>, Arc<Insn>)> {
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
fn _tied(
    bits: u32,
    one: &Insn,
    values: &BTreeSet<u32>,
    frame: &mut Frame,
    classes: &RegisterClasses,
) -> Result<Option<Arc<Insn>>, Error> {
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
    for (place, _register) in classes.requirements(what) {
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
    if !_encodable(bits, &made, classes)? {
        return Ok(None);
    }
    Ok(Some(_with(one, |insn| {
        insn.what = Some(made);
        insn.defines = one.defines.iter().copied().filter(|v| *v != value).collect();
        insn.uses = one.uses.iter().copied().filter(|v| *v != value).collect();
    })))
}

/// Whether this form exists, asked of the one place that knows.
fn _encodable(
    bits: u32,
    what: &Semantics,
    classes: &RegisterClasses,
) -> Result<bool, Error> {
    let regs = classes.registers;
    let mut taken: IndexMap<u32, Register> = IndexMap::default();
    let rows: IndexMap<u32, Vec<Register>> = [1_u32, 2, 4]
        .into_iter()
        .map(|width| {
            (
                width,
                classes
                    .available
                    .iter()
                    .copied()
                    .filter(|one| {
                        regs.width_of(regs.named(*one, i64::from(width)))
                            .filter(|_| regs.integer(regs.named(*one, i64::from(width))))
                            == Some(i64::from(width))
                    })
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
                // refuses below, which is the safe answer
                return operand.clone();
            }
            taken.insert(held.value, row[taken.len()]);
        }
        Loc::Reg(Reg { register: regs.named(taken[&held.value], i64::from(held.width)), width: held.width })
    };

    let probe = Semantics {
        dests: what.dests.iter().map(&mut placed).collect(),
        sources: what.sources.iter().map(&mut placed).collect(),
        ..what.clone()
    };
    Ok(super::select::priced_in(bits, &probe, 0, None, false, false, None).is_some())
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

    use super::{_color_slots, _constants, planned, spilled, spilled_from};
    use crate::backend::frame::{Frame, SlotKey};
    use crate::backend::{objbuild, select};
    use crate::model::ir::{Addr, AddressRef, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
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

    fn imm(
        value: i64,
        width: u32,
    ) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn set(values: &[u32]) -> BTreeSet<u32> {
        values.iter().copied().collect()
    }

    fn slot(value: u32) -> SlotKey {
        SlotKey::from(value)
    }

    /// `ir.Mem(addr, width, through, offset, disp_width)`.
    fn mem(
        addr: Addr,
        width: u32,
        through: Register,
        offset: i64,
        disp_width: u32,
    ) -> Mem {
        Mem { through, offset, disp_width, ..Mem::new(Some(addr), width) }
    }

    fn insn(
        at: i64,
        covers: (i64, i64),
        what: Semantics,
        defines: &[u32],
        uses: &[u32],
    ) -> Insn {
        Insn::new(at, Some(covers), Some(what), defines.to_vec(), uses.to_vec())
    }

    fn _move(
        into: u32,
        out_of: u32,
        group: Option<i64>,
        at: i64,
    ) -> Insn {
        let what = semantics(Operation::Move, "mov", vec![held(into, 2)], vec![held(out_of, 2)]);
        Insn { group, ..insn(at, (at, at), what, &[into], &[out_of]) }
    }

    fn _add(
        into: u32,
        out_of: u32,
        at: i64,
    ) -> Insn {
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

    fn _out(
        body: &LirBody,
        values: &[u32],
    ) -> Vec<Arc<Insn>> {
        let (got, _made) =
            spilled(body, &set(values), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
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

    fn index_of(
        insns: &[Arc<Insn>],
        one: &Arc<Insn>,
    ) -> usize {
        insns.iter().position(|other| Arc::ptr_eq(other, one)).expect("present")
    }

    /// A hoisted index add may not land before a SETcc, which reads the
    /// flags an earlier compare left: `setl` was read as neither reading
    /// nor writing them, so the add behind it looked safe to hoist.
    #[test]
    fn test_an_index_add_is_not_hoisted_above_a_flag_reader() {
        let set = insn(0x10, (0x10, 0x10), semantics(Operation::Unary, "setl", vec![held(1, 1)], vec![]), &[1], &[]);
        let add = _add(2, 3, 0x11);
        assert!(!super::_flags_overwritten(&[Arc::new(set), Arc::new(add)]));
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

    /// A spilled value updated and then read in its block keeps that local
    /// run in a register: N$PQ4's `x - q*d` then `*10` ran `sub [slot]`,
    /// then read the slot twice, three memory operands for one reload.
    #[test]
    fn test_an_update_read_again_in_its_block_stays_in_a_register() {
        let binary = |name: &str, into: u32, left: u32, right: u32, at: i64| {
            let what = semantics(Operation::Binary, name, vec![held(into, 2)], vec![held(left, 2), held(right, 2)]);
            insn(at, (at, at), what, &[into], &[left, right])
        };
        let made = binary("imul", 1, 8, 9, 0x10);
        let updated = binary("sub", 1, 1, 2, 0x11);
        let read = _move(3, 1, None, 0x12);
        let added = binary("add", 3, 3, 1, 0x13);
        let pushed = _push(0x14, (0x14, 0x14), held(3, 2), &[3]);
        let insns = _out(&_body(vec![made, updated, read, added, pushed]), &[1]);
        let memory = insns
            .iter()
            .filter(|one| {
                one.what
                    .as_ref()
                    .is_some_and(|what| what.dests.iter().chain(&what.sources).any(|loc| matches!(loc, Loc::Mem(_))))
            })
            .count();
        // The product stored, and read back once.
        assert_eq!(memory, 2, "{insns:#?}");
    }

    /// The short and local update passes numbered the body again (and asked its
    /// intervals unshared) each time they found something to do, though the
    /// spill had just numbered it: d_faces -O1, 634 spills, 249 + 54 M of 24 G
    /// instructions. They read the remembered numbering.
    #[test]
    fn test_the_update_passes_read_the_remembered_numbering_of_the_body() {
        let copied = _move(2, 1, None, 0x10);
        let shifted = insn(
            0x11,
            (0x11, 0x11),
            semantics(Operation::Binary, "shl", vec![held(2, 2)], vec![held(2, 2), imm(1, 1)]),
            &[2],
            &[2],
        );
        let body = _body(vec![copied, shifted]);
        crate::analysis::intervals::indexed_shared(&body);
        let mut frame = crate::backend::frame::Frame::new(0);
        super::_short_update_runs(&body, &BTreeSet::from([2]), &mut frame, 100).expect("runs");
        super::_local_updates(&body, &BTreeSet::from([2]), &mut frame, 100).expect("updates");
        // One ask above, then one by each pass.
        assert_eq!(body.facts.0.counted("indexed-remembered"), 2, "the passes numbered the body again");
        assert_eq!(body.facts.0.counted("indexed"), 1);
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
        for between in ["call sparing the frame", "call", "call listing nothing"] {
            let param = mem(Addr::new(Space::Frame, 6), 2, Register::BP, 0, 2);
            let load = insn(
                0x100,
                (0x100, 0x100),
                semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(param)]),
                &[1],
                &[],
            );
            let spared = between.ends_with("sparing the frame");
            let mut middle = insn(0x101, (0x101, 0x101), semantics(Operation::Call, "call", vec![], vec![]), &[], &[]);
            // A call listing nothing it touches may write anything.
            middle.call = (between != "call listing nothing").then(|| {
                Arc::new(crate::model::lir::CallMemory {
                    effects: llrm_mir::memory::Effects::ANY,
                    private: if spared { vec![crate::model::lir::WHOLE_FRAME] } else { vec![] },
                    disturbs: BTreeSet::new(),
                })
            });
            middle.clobbers = BTreeSet::from([Register::EAX]);
            let got = _out(&_body(vec![load, middle, _add(2, 1, 0x102)]), &[1]);
            let slots: Vec<&Loc> = got
                .iter()
                .filter_map(|one| one.what.as_ref())
                .flat_map(|what| &what.dests)
                .filter(|place| {
                    matches!(
                        place,
                        Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                    )
                })
                .collect();
            assert_eq!(slots.is_empty(), spared, "{between}: {got:?}");
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
        let (result, _made) = spilled(
            &_body(vec![load, store, _add(3, 1, 0x102)]),
            &set(&[1]),
            Some(&mut frame),
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("spills");
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
        let store = insn(
            0x101,
            (0x101, 0x101),
            semantics(Operation::Move, "mov", vec![Loc::Mem(local)], vec![held(2, 2)]),
            &[],
            &[2],
        );
        _assert_parameter_rematerialized(store);
    }

    #[test]
    fn test_parameter_rematerializes_across_a_local_array_store() {
        let local = Mem {
            index: Some(Held { value: 4, width: 2 }),
            index_through: Register::SI,
            ..mem(Addr::new(Space::Frame, -132), 2, Register::BP, 0, 2)
        };
        let store = insn(
            0x101,
            (0x101, 0x101),
            semantics(Operation::Move, "mov", vec![Loc::Mem(local)], vec![held(2, 2)]),
            &[],
            &[2, 4],
        );
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
            let op = Insn {
                what: Some(Semantics { name: Some(op_name.to_owned()), ..what(&op).clone() }),
                uses: vec![1],
                ..op
            };
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

    /// A split remakes a constant where its piece ends, so the rest of the
    /// value has one identical definition per piece; the spiller gave it a
    /// stack slot, and COPPER reloaded 0 inside a loop.
    #[test]
    fn test_a_constant_defined_twice_alike_is_rematerialized_without_a_frame_slot() {
        let result = _out(
            &_body(vec![_constant(20, (0, 3)), _add(2, 1, 0x100), _constant(20, (4, 7)), _add(3, 1, 0x100)]),
            &[1],
        );
        assert!(
            !result.iter().filter_map(|one| one.what.as_ref()).any(|what| operands(what).any(is_mem)),
            "{result:?}"
        );
    }

    /// One wide read of a constant made every read of it a reload: COPPER's
    /// zero, copied once into a 32-bit value, was loaded from its slot in a
    /// loop.
    #[test]
    fn test_a_constant_read_wider_once_is_remade_at_its_narrow_reads() {
        let wide = insn(4, (4, 4), semantics(Operation::Move, "mov", vec![held(3, 4)], vec![held(1, 4)]), &[3], &[1]);
        let result = _out(&_body(vec![_constant(20, (0, 3)), wide, _add(2, 1, 0x100)]), &[1]);
        let reads = result.iter().filter(|one| what(one).sources.iter().any(is_mem)).count();
        assert_eq!(reads, 1, "{result:?}");
        let n = result.len();
        assert_eq!(what(&result[n - 2]).sources, [imm(20, 2)], "{result:?}");
    }

    /// A parallel copy into an already spilled value hid the constant it
    /// copies: COPPER's zero took a slot and was reloaded in a loop.
    #[test]
    fn test_a_constant_copied_into_a_slot_by_a_parallel_copy_is_remade() {
        let slot = Frame::new(0).cell(9u32, 2).expect("a slot");
        let into = semantics(Operation::Move, "mov", vec![Loc::Mem(slot)], vec![held(1, 2)]);
        let grouped = Insn { group: Some(7), ..insn(4, (4, 4), into, &[], &[1]) };
        let result = _out(&_body(vec![_constant(20, (0, 3)), grouped, _add(2, 1, 0x100)]), &[1]);
        let reads = result.iter().filter(|one| what(one).sources.iter().any(is_mem)).count();
        assert_eq!(reads, 0, "{result:?}");
        assert!(result.iter().any(|one| one.group == Some(7) && what(one).sources == [imm(20, 2)]), "{result:?}");
    }

    /// The allocator names values the body no longer holds; a reload
    /// numbered from the body alone took one of those names, and the
    /// allocator left the reload unplaced.
    #[test]
    fn test_a_spill_numbers_its_reloads_above_the_floor_it_is_given() {
        let body = _body(vec![_move(1, 10, None, 0x10), _add(11, 1, 0x12), _add(12, 1, 0x14)]);
        let (_, made) = spilled_from(
            &body,
            &set(&[1]),
            Some(&mut Frame::new(0)),
            100,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("spills");
        assert!(!made.is_empty() && made.iter().all(|one| *one >= 100), "{made:?}");
    }

    fn _frame_address(
        disp: i64,
        disp_width: u32,
    ) -> AddressRef {
        AddressRef {
            through: Register::BP,
            offset: disp,
            disp_width,
            ..AddressRef::new(Some(Addr::new(Space::Frame, disp)))
        }
    }

    fn _lea(
        at: i64,
        covers: (i64, i64),
        source: &AddressRef,
    ) -> Insn {
        let what = semantics(Operation::Address, "lea", vec![held(1, 2)], vec![Loc::Address(source.clone())]);
        insn(at, covers, what, &[1], &[])
    }

    fn _recreated(insns: &[Arc<Insn>]) -> Vec<&Arc<Insn>> {
        insns.iter().filter(|one| one.what.as_ref().is_some_and(|w| w.op == Operation::Address)).collect()
    }

    /// `p = *p` with `p` spilled: the load read its own base from a register
    /// nothing had reloaded, and the allocator emitted `mov cx, [bx]` with bx
    /// never set (found by the allocator fuzz lane, seed 25).
    #[test]
    fn test_a_spilled_value_read_as_the_base_of_its_own_load_is_reloaded_first() {
        let cell = Mem { base: Some(Held { value: 1, width: 2 }), ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2) };
        let first = insn(0, (0, 3), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(8, 2)]), &[1], &[]);
        let walk =
            insn(4, (4, 6), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(cell)]), &[1], &[1]);
        let store = insn(8, (8, 10), semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)]), &[2], &[1]);
        let result = _out(&_body(vec![first, walk, store]), &[1]);
        let mut defined: BTreeSet<u32> = BTreeSet::new();
        for one in &result {
            for read in &one.uses {
                assert!(defined.contains(read), "value#{read} is read before anything defines it: {one:?}");
            }
            defined.extend(one.defines.iter().copied());
        }
    }

    /// A rematerialized address inserted between two moves of a parallel
    /// copy cut the copy in two, and lived across the rest of it (#104:
    /// ten slots, spilled again without end; with more moves than registers,
    /// a value for each, at once). The move that reads it leaves the copy and
    /// follows it, with the address made beside it.
    #[test]
    fn test_a_move_of_a_parallel_copy_that_reads_a_remade_value_follows_the_copy() {
        let source = _frame_address(-132, 2);
        let body = _body(vec![
            _lea(0, (0, 3), &source),
            _move(10, 5, Some(1), 0x100),
            _move(11, 1, Some(1), 0x100),
            _move(12, 6, Some(1), 0x100),
            _add(2, 10, 0x110),
        ]);
        let result = _out(&body, &[1]);
        let copy: Vec<usize> = (0..result.len()).filter(|at| result[*at].group == Some(1)).collect();
        assert_eq!(copy.len(), 2);
        assert_eq!(copy[1] - copy[0], 1, "one parallel copy, in one piece: {copy:?}");
        let made = result.iter().position(|one| one.rematerialized).expect("the address is made again");
        assert!(made > copy[1] && result[made + 1].defines == [11], "made at {made}, the copy ends at {}", copy[1]);
    }

    #[test]
    fn test_spilled_frame_address_is_rematerialized_without_a_frame_slot() {
        let source = _frame_address(-132, 2);
        let mut frame = Frame::new(0);
        let body = _body(vec![_lea(0, (0, 3), &source), _add(2, 1, 0x100)]);
        let (result, _made) =
            spilled(&body, &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
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
        let (result, made) =
            spilled(&body, &set(&[1]), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
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
        let regs = crate::backend::registerinfo::test_regs();
        for space in [Space::Segment, Space::External] {
            let source =
                AddressRef { disp_width: 2, ..AddressRef::new(Some(Addr { index: 7, ..Addr::new(space, 12) })) };
            let mut frame = Frame::new(0);
            let body = _body(vec![_lea(0, (0, 3), &source), _add(2, 1, 0x100)]);
            let (result, _made) =
                spilled(&body, &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                    .expect("spills");
            assert!(!frame.slots.contains_key(&slot(1)));
            let insns = result.insns();
            let recreated = _recreated(&insns);
            assert_eq!(recreated.len(), 1);
            assert_eq!(what(recreated[0]).sources, [Loc::Address(source.clone())]);
            assert!(recreated[0].rematerialized);
            // The rematerialized spelling is not an unrelocated literal zero:
            // fresh OMF emission places the original symbol fixup
            // on its new displacement.
            let lea = semantics(
                Operation::Address,
                "lea",
                vec![Loc::Reg(Reg { register: Register::BX, width: 2 })],
                vec![Loc::Address(source)],
            );
            let names: IndexMap<(Space, i64), String> = IndexMap::from_iter([((space, 7), "_descriptor".to_owned())]);
            let emitted = objbuild::_encoded(regs, &lea, &names, 16).expect("encodes");
            assert_eq!(emitted.code, [0x8D, 0x1E, 0x0C, 0x00]);
            assert_eq!(emitted.fixups, [objbuild::Fixup::new(2, objbuild::OFFSET, "_descriptor")]);
        }
    }

    fn _extension(
        name: &str,
        at: i64,
    ) -> Insn {
        insn(at, (at, at), semantics(Operation::Extend, name, vec![held(2, 4)], vec![held(1, 2)]), &[2], &[1])
    }

    fn _wide_add(
        at: i64,
        into: u32,
    ) -> Insn {
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
            let (result, _made) =
                spilled(&body, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                    .expect("spills");
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
            spilled(&_body(insns), &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
            assert!(frame.slots.contains_key(&slot(2)), "{unsafe_}");
        }
    }

    fn _branch(
        op: Operation,
        name: &str,
        at: i64,
        target: i64,
    ) -> Insn {
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
        spilled(&body, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16()).expect("spills");
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
        spilled(&_sequential(), &set(&[1, 2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 2);
    }

    #[test]
    fn test_nonoverlapping_spills_from_later_rounds_reuse_the_frame_slot() {
        let mut frame = Frame::new(0);
        let (first, _made) =
            spilled(&_sequential(), &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 2);
    }

    #[test]
    fn test_overlapping_spills_from_later_rounds_keep_distinct_slots() {
        let mut frame = Frame::new(0);
        let (first, _made) =
            spilled(&_overlapping(), &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
    }

    /// A value copied from one spilled earlier took the first free slot, not
    /// its source's, so the copy stayed as a load and a store: CYCLEBLOBS
    /// shifted six slots down a chain at each loop entry.
    #[test]
    fn test_a_later_spill_shares_the_slot_it_is_copied_from() {
        let body = _body(vec![
            _move(3, 30, None, 0x10),
            _move(1, 10, None, 0x12),
            _add(31, 3, 0x14),
            _move(2, 1, None, 0x16),
            _add(21, 2, 0x18),
        ]);
        let mut frame = Frame::new(0);
        let (first, _made) =
            spilled(&body, &set(&[1, 3]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        _color_slots(&first, &set(&[2]), &IndexMap::from_iter([(2, 2)]), &mut frame).expect("colors");
        assert_eq!(frame.slots[&slot(2)], frame.slots[&slot(1)]);
    }

    /// The interference of every value live together was built to ask of the
    /// pairs in the copy webs a spilled value is in (14.8 s of compiling
    /// `d_faces`, #559). Another web is asked of by no one.
    #[test]
    fn test_siblings_ask_the_webs_of_the_spilled_values_only() {
        let body = _body(vec![
            _move(1, 10, None, 0x10),
            _move(2, 1, None, 0x12),
            _move(11, 20, None, 0x14),
            _move(12, 11, None, 0x16),
            _add(31, 2, 0x18),
            _add(32, 12, 0x1a),
        ]);
        let mut frame = Frame::new(0);
        super::siblings(&body, &set(&[1]), Some(&mut frame), &BTreeSet::new()).expect("sibling slots");
        assert_eq!(crate::backend::coalesce::last_asked(), Some(3), "the other web's values were asked of");
    }

    /// `siblings` walked every instruction of the body for its plain moves,
    /// numbered every value of it for the liveness of the graph, and walked
    /// it again for the widths and the occurrences: 3.6 + 1.2 + 1.0 G of the
    /// 104 G of compiling `d_faces` (6%), for a web of three values. It
    /// looks at the instructions that name the web's values.
    #[test]
    fn test_siblings_cost_the_size_of_the_web_not_the_size_of_the_body() {
        let mut insns = vec![_move(1, 10, None, 0x10), _move(2, 1, None, 0x12), _add(31, 2, 0x14)];
        for other in 0..60u32 {
            insns.push(_move(100 + 2 * other, 101 + 2 * other, None, 0x20 + 2 * other as i64));
        }
        let body = _body(insns);
        let mut frame = Frame::new(0);
        let before = super::examined();
        super::siblings(&body, &set(&[1]), Some(&mut frame), &BTreeSet::new()).expect("sibling slots");
        let looked = super::examined() - before;
        assert!(
            looked <= 12,
            "{looked} instructions looked at for the copies of a web of three values in a body of 63"
        );
        assert!(
            crate::backend::coalesce::last_numbered() <= 4,
            "{} values numbered for a graph of {:?}",
            crate::backend::coalesce::last_numbered(),
            crate::backend::coalesce::last_asked()
        );
    }

    /// Every spill made a body of every instruction with the slots in it as
    /// values, and numbered it and found every interval again to colour the
    /// slots: 16 s of compiling `d_faces` (#559). The slots are found among
    /// themselves and the body's own intervals are remembered; the answers are
    /// the same.
    #[test]
    fn test_the_slots_colors_are_the_same_found_among_themselves_as_found_whole() {
        let body = _body(vec![
            _move(3, 30, None, 0x10),
            _move(1, 10, None, 0x12),
            _add(31, 3, 0x14),
            _move(2, 1, None, 0x16),
            _add(21, 2, 0x18),
        ]);
        let mut frame = Frame::new(0);
        let (spilt, _made) =
            spilled(&body, &set(&[1, 3]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        assert!(!frame.slots.is_empty(), "the body has homes to find");
        let (among, whole) = (
            super::_existing_colors_by(&spilt, &mut frame, false),
            super::_existing_colors_by(&spilt, &mut frame, true),
        );
        assert!(super::_same_colors(&among.0, &whole.0), "{:?} {:?}", among.0, whole.0);
        assert!(
            among.1.len() == whole.1.len()
                && among
                    .1
                    .iter()
                    .all(|(value, one)| whole.1.get(value).is_some_and(|other| other.segments == one.segments))
        );
    }

    /// The homes are walked in a body of the instructions that name them: two
    /// of one parallel copy with another instruction between them were one
    /// run there, which moved the point the first is read at (weapons.c: a home
    /// live from 415, not 411).
    #[test]
    fn test_two_instructions_of_one_copy_apart_are_not_one_run_among_the_homes() {
        let body = _body(vec![
            _move(3, 30, Some(7), 0x10),
            _move(40, 41, None, 0x12),
            _move(1, 10, Some(7), 0x14),
            _add(31, 3, 0x16),
            _move(2, 1, None, 0x18),
            _add(21, 2, 0x1a),
            _add(22, 1, 0x1c),
        ]);
        let mut frame = Frame::new(0);
        let (spilt, _made) =
            spilled(&body, &set(&[1, 3]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        assert!(!frame.slots.is_empty(), "the body has homes to find");
        let (among, whole) = (
            super::_existing_colors_by(&spilt, &mut frame, false),
            super::_existing_colors_by(&spilt, &mut frame, true),
        );
        assert!(super::_same_colors(&among.0, &whole.0), "{:?} {:?}", among.0, whole.0);
    }

    #[test]
    fn test_cross_round_coloring_includes_current_preassigned_spill_webs() {
        let mut frame = Frame::new(0);
        frame.slots.insert(slot(1), -2);
        frame.capacities.insert(-2, 2);
        _color_slots(&_overlapping(), &set(&[1, 2]), &IndexMap::from_iter([(1, 2), (2, 2)]), &mut frame)
            .expect("colors");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
    }

    fn _wide_pair(
        into: u32,
        out_of: u32,
        user: u32,
        at: i64,
    ) -> [Insn; 2] {
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
        let (first, _made) =
            spilled(&body, &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 4);
    }

    /// A body that calls `setjmp` shares no slot: after the second return one a
    /// dead value used holds that value. Another value's dead slot is
    /// otherwise reused, as the test above shows.
    #[test]
    fn test_no_spill_slot_is_shared_in_a_body_that_calls_setjmp() {
        let shared = |twice: bool| {
            let mut frame = Frame::new(0);
            let [wide, use_wide] = _wide_pair(1, 10, 11, 0x10);
            let mut body = _body(vec![wide, use_wide, _move(2, 20, None, 0x14), _add(21, 2, 0x16)]);
            body.returns_twice = twice;
            let (first, _made) =
                spilled(&body, &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                    .expect("spills");
            spilled(&first, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
            frame.slots[&slot(1)] == frame.slots[&slot(2)]
        };
        assert_eq!((shared(false), shared(true)), (true, false));
    }

    #[test]
    fn test_later_wide_spill_does_not_outgrow_a_narrow_slot() {
        let mut frame = Frame::new(0);
        let [wide, use_wide] = _wide_pair(2, 20, 21, 0x14);
        let body = _body(vec![_move(1, 10, None, 0x10), _add(11, 1, 0x12), wide, use_wide]);
        let (first, _made) =
            spilled(&body, &set(&[1]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        spilled(&first, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 6);
    }

    #[test]
    fn test_overlapping_spills_keep_distinct_frame_slots() {
        let mut frame = Frame::new(0);
        spilled(&_overlapping(), &set(&[1, 2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_ne!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 4);
    }

    #[test]
    fn test_narrow_spill_can_reuse_a_dead_wider_slot() {
        let mut frame = Frame::new(0);
        let [wide, use_wide] = _wide_pair(1, 10, 11, 0x10);
        let body = _body(vec![wide, use_wide, _move(2, 20, None, 0x14), _add(21, 2, 0x16)]);
        spilled(&body, &set(&[1, 2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_eq!(frame.slots[&slot(1)], frame.slots[&slot(2)]);
        assert_eq!(frame.size(), 4);
    }

    fn _constant(
        value: i64,
        covers: (i64, i64),
    ) -> Insn {
        insn(0, covers, semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(value, 2)]), &[1], &[])
    }

    #[test]
    fn test_grouped_constant_rematerializes_without_splitting_parallel_copy() {
        for destination_spilled in [false, true] {
            let body = _body(vec![_constant(0, (0, 0)), _move(2, 1, Some(7), 0x100), _move(3, 4, Some(7), 0x100)]);
            let mut frame = Frame::new(0);
            let values = if destination_spilled { set(&[1, 2]) } else { set(&[1]) };
            let (done, _) = spilled(&body, &values, Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
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

    /// `mov v1, [global]` read back into `v2`, which is read once more: v2 is
    /// the load made again, not a slot.
    #[test]
    fn test_a_copy_of_a_global_load_is_made_again_not_stored() {
        let cell = crate::model::ir::Mem {
            addr: Some(crate::model::ir::Addr::new(Space::Segment, 0)),
            ..crate::model::ir::Mem::new(None, 2)
        };
        let load =
            insn(0, (0, 0), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(cell)]), &[1], &[]);
        let body = _body(vec![load, _move(2, 1, None, 0x10), _add(21, 2, 0x12), _add(22, 1, 0x14)]);
        let mut frame = Frame::new(0);
        let (done, _) = spilled(&body, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert_eq!(frame.size(), 0, "a slot for a copy of a load");
        assert!(!done.insns().iter().any(|one| one.spill_store || one.spill_reload));
    }

    /// A copy of a load is made again as the load only while the cell holds
    /// until the copy's last use: the fuzz (seed
    /// 31) stored an address through a copy whose cell had been written since.
    #[test]
    fn test_a_copy_of_a_load_is_not_made_again_after_its_cell_changes() {
        let cell = crate::model::ir::Mem {
            addr: Some(crate::model::ir::Addr::new(Space::Segment, 0)),
            ..crate::model::ir::Mem::new(None, 2)
        };
        let load = insn(
            0,
            (0, 0),
            semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(cell.clone())]),
            &[1],
            &[],
        );
        let write = insn(
            0x20,
            (0x20, 0x20),
            semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![imm(9, 2)]),
            &[],
            &[],
        );
        let body = _body(vec![load, _move(2, 1, None, 0x10), _add(21, 1, 0x12), write, _add(22, 2, 0x30)]);
        let mut frame = Frame::new(0);
        let (done, _) = spilled(&body, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills");
        assert!(
            done.insns().iter().any(|one| one.spill_store || one.spill_reload),
            "the copy was made again from a cell that changed"
        );
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
        let (got, _) = spilled(
            &_body(vec![_move(1, 2, Some(1), 0x100)]),
            &set(&[1, 2]),
            Some(&mut frame),
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("spills");
        let got = got.insns();
        assert!(got.len() == 1 && got[0].group == Some(1));
        assert!(operands(what(&got[0])).all(is_mem));
        assert!(got[0].defines.is_empty() && got[0].uses.is_empty());
    }

    #[test]
    fn test_a_grouped_move_coalesced_to_one_spill_slot_is_an_identity() {
        let mut frame = Frame::new(0);
        let (got, _) = spilled(
            &_body(vec![_move(1, 2, Some(1), 0x100)]),
            &set(&[1, 2]),
            Some(&mut frame),
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("spills");
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
        let (out, made) =
            spilled(&_body(vec![load]), &set(&[3]), None, &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
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
        assert!(!reloads.is_empty() && reloads.iter().all(|one| as_held(&what(one).dests[0]).width == 4), "{out:?}");
    }

    /// Whether the cell a load read holds at its uses was found by going over
    /// every block, each instruction asked of again, until the blocks
    /// settled: three passes at least, so a body of 40 blocks asked of its
    /// instructions well over a hundred times for one load. Each is asked
    /// of once, and the load is found stable all the same.
    #[test]
    fn test_whether_a_loaded_cell_holds_asks_of_each_instruction_once() {
        let cell = Loc::Mem(mem(Addr::new(Space::Frame, 4), 2, Register::BP, 4, 1));
        let load = insn(0, (0, 1), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![cell]), &[1], &[]);
        let mut blocks = vec![LirBlock { succ: vec![1], ..LirBlock::new(0, vec![Arc::new(load)]) }];
        for at in 1..=40_i64 {
            let work = insn(
                at * 10,
                (at * 10, at * 10 + 1),
                semantics(
                    Operation::Binary,
                    "add",
                    vec![held(100 + at as u32, 2)],
                    vec![held(100 + at as u32 - 1, 2), Loc::Imm(Imm { value: 1, width: 2, address: None })],
                ),
                &[100 + at as u32],
                &[100 + at as u32 - 1],
            );
            blocks.push(LirBlock {
                succ: if at < 40 { vec![at + 1] } else { vec![] },
                ..LirBlock::new(at, vec![Arc::new(work)])
            });
        }
        let last = blocks.last_mut().expect("a block");
        let used = insn(1000, (1000, 1001), semantics(Operation::Push, "push", vec![], vec![held(1, 2)]), &[], &[1]);
        last.insns.edit(|insns| insns.push(Arc::new(used)));
        let body = LirBody::new("chain", 0, blocks, IndexMap::default(), IndexMap::default());
        let before = super::KEEPS.with(std::cell::Cell::get);
        let stable = super::_stable_loads(&body, &set(&[1]));
        let asked = super::KEEPS.with(std::cell::Cell::get) - before;
        assert_eq!(stable.len(), 1, "the load of a cell nothing writes is stable");
        assert!(asked <= 2 * 43, "{asked} questions of `_keeps` for a body of 43 instructions");
    }

    /// A function with n values loaded from the frame asked of every
    /// instruction for each: a by-value struct of 2,048 words spent 41% of
    /// its 6.1 s in `_keeps`, `_may_write` and the walk around them (#924).
    /// Only what may write a cell is asked about it, and a store that does
    /// overlap one cell still makes that one unstable.
    #[test]
    fn test_a_loaded_cell_is_asked_about_only_what_may_write_it() {
        let cell = |disp: i64| Loc::Mem(mem(Addr::new(Space::Frame, disp), 2, Register::BP, disp, 1));
        let n = 60_i64;
        let mut insns = Vec::new();
        for i in 0..n {
            insns.push(insn(
                i,
                (i, i + 1),
                semantics(Operation::Move, "mov", vec![held(1 + i as u32, 2)], vec![cell(-2 * (i + 1))]),
                &[1 + i as u32],
                &[],
            ));
        }
        // Stores to cells of their own, far from every load's, and one onto the
        // cell of the load numbered 7.
        for i in 0..n {
            let at = 100 + i;
            insns.push(insn(
                at,
                (at, at + 1),
                semantics(
                    Operation::Move,
                    "mov",
                    vec![cell(-1000 - 2 * i)],
                    vec![Loc::Imm(Imm { value: 0, width: 2, address: None })],
                ),
                &[],
                &[],
            ));
        }
        insns.push(insn(
            300,
            (300, 301),
            semantics(
                Operation::Move,
                "mov",
                vec![cell(-16)],
                vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
            ),
            &[],
            &[],
        ));
        for i in 0..n {
            let at = 400 + i;
            insns.push(insn(
                at,
                (at, at + 1),
                semantics(Operation::Push, "push", vec![], vec![held(1 + i as u32, 2)]),
                &[],
                &[1 + i as u32],
            ));
        }
        let total = insns.len() as i64;
        let body = _body(insns);
        let values: Vec<u32> = (1..=n as u32).collect();
        let before = super::KEEPS.with(std::cell::Cell::get);
        let stable = super::_stable_loads(&body, &set(&values));
        let asked = super::KEEPS.with(std::cell::Cell::get) - before;
        assert_eq!(stable.len() as i64, n - 1, "every load but the one whose cell is stored to");
        assert!(!stable.contains_key(&8), "value 8 was loaded from [bp-16], which a store overwrites");
        assert!(asked as i64 <= 2 * n, "{asked} questions of `_keeps` for {n} loads in a body of {total} instructions");
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
            out.iter()
                .flat_map(|one| one.uses.iter())
                .filter(|value| **value != 2)
                .all(|value| defined.contains(value)),
            "{out:?}"
        );
    }

    /// snd_mix_frame stored four bytes of a value first seen as a word, over
    /// the saved BP.
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
        let body = _body(vec![
            op("mov", 1, 2, &[3]),
            op("mov", 2, 2, &[3]),
            op("add", 1, 4, &[1, 3]),
            op("add", 2, 2, &[2, 3]),
        ]);
        let mut frame = Frame::new(0);
        let (got, _made) =
            spilled(&body, &set(&[1, 2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        let cells: BTreeSet<(i64, u32)> = got
            .insns()
            .iter()
            .filter_map(|one| one.what.clone())
            .flat_map(|what| what.dests.into_iter().chain(what.sources))
            .filter_map(|place| match place {
                Loc::Mem(cell) => {
                    cell.addr.filter(|addr| addr.space == Space::Frame).map(|addr| (addr.disp, cell.width))
                }
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

    /// LNGMXX printed 169330 instead of 142900 after a tied spill discarded its
    /// loaded accumulator.
    #[test]
    fn test_two_spilled_operands_keep_the_accumulator_value() {
        for (name, expected) in
            [("add", 15000), ("sub", 9000), ("and", 12000 & 3000), ("or", 12000 | 3000), ("xor", 12000 ^ 3000)]
        {
            let mut frame = Frame::new(0);
            let op = _add(1, 2, 0x100);
            let op = Insn { what: Some(Semantics { name: Some(name.to_owned()), ..what(&op).clone() }), ..op };
            let (body, _) = spilled(
                &_body(vec![op]),
                &set(&[1, 2]),
                Some(&mut frame),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("spills");
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

    /// LNGMXX's two spilled operands must retain the accumulator but need only
    /// one scratch.
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
            one.what
                .as_ref()
                .is_some_and(
                    |what| operands(what).any(|x| {
                        matches!(
                            x,
                            Loc::Mem(Mem { addr: Some(addr), .. }) if addr.space == Space::Segment
                        )
                    }),
                )
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

    fn _binary(
        name: &str,
        into: u32,
        other: u32,
        at: i64,
    ) -> Insn {
        let what = semantics(Operation::Binary, name, vec![held(into, 2)], vec![held(into, 2), held(other, 2)]);
        insn(at, (at, at + 2), what, &[into], &[into, other])
    }

    /// pressx spilled 207, then 212, then 215, at one `add`, two instructions
    /// added every round.
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

    /// `ir.Mem(Addr(Space.FAR, disp, segment=Register.ES), 2, base=ir.Held(5,
    /// 2), index=...)`.
    fn _far(
        disp: i64,
        index: Option<u32>,
    ) -> Mem {
        Mem {
            base: Some(Held { value: 5, width: 2 }),
            index: index.map(|value| Held { value, width: 2 }),
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, disp) }), 2)
        }
    }

    fn _read(
        at: i64,
        into: u32,
        cell: Mem,
        uses: &[u32],
    ) -> Insn {
        insn(
            at,
            (at, at + 2),
            semantics(Operation::Move, "mov", vec![held(into, 2)], vec![Loc::Mem(cell)]),
            &[into],
            uses,
        )
    }

    fn _folded_add(one: &Insn) -> bool {
        one.what
            .as_ref()
            .is_some_and(
                |what| what.name.as_deref() == Some("add") && what.sources.len() == 2 && is_mem(&what.sources[1]),
            )
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
                    && matches!(
                        &what(one).sources[0],
                        Loc::Mem(Mem { addr: Some(addr), .. }) if addr.space == Space::Far
                    )
            })
            .unwrap();
        assert!(as_mem(&what(addressed).sources[0]).index.is_none(), "{:?}", addressed.what);
        assert!(!result.iter().any(|one| one.spill_reload), "{result:?}");
    }

    /// Each spill took the liveness of the whole body for `_final_uses`, which
    /// only an index fold reads: 4% of compiling d_alias (#559). It is
    /// worked out when a fold asks, and once for the body.
    #[test]
    fn test_final_uses_are_worked_out_only_for_a_body_with_an_index_to_fold() {
        let before = super::final_use_runs();
        let result = _out(&_body(vec![_add(1, 2, 0x100), _move(3, 1, None, 0x102)]), &[1]);
        assert!(!result.is_empty());
        assert_eq!(super::final_use_runs() - before, 0, "a body with no memory index asked for the final uses");
        let read = _read(0x102, 4, _far(0, Some(1)), &[5, 1]);
        let before = super::final_use_runs();
        let result = _out(&_body(vec![_add(1, 2, 0x100), read]), &[1]);
        assert_eq!(result.iter().filter(|one| _folded_add(one)).count(), 1, "the fold is still made");
        assert_eq!(super::final_use_runs() - before, 1, "worked out once");
    }

    /// Three blocks, a value in the middle one, a value in the last.
    fn _three_blocks() -> LirBody {
        let block = |at: i64, insns: Vec<Insn>, succ: Vec<i64>| LirBlock {
            succ,
            ..LirBlock::new(at, insns.into_iter().map(Arc::new).collect())
        };
        LirBody::new(
            "three",
            0,
            vec![
                block(0, vec![_move(10, 11, None, 0x10)], vec![1]),
                block(1, vec![_add(1, 2, 0x100), _move(3, 1, None, 0x102)], vec![2]),
                block(2, vec![_move(20, 21, None, 0x200), _move(22, 20, None, 0x202)], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    /// Each spill scanned the body for the values it spills (copies, constants,
    /// addresses, extensions, widths): 8% of compiling d_alias (#559). The
    /// occurrences follow the body, redoing only the blocks a rewrite changed.
    #[test]
    fn test_the_homes_are_not_made_into_instructions_unless_the_whole_body_is_walked() {
        let body = _three_blocks();
        let (spilt, _) =
            spilled(&body, &set(&[1]), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        let mut frame = Frame::new(0);
        let (again, _) =
            spilled(&spilt, &set(&[2]), Some(&mut frame), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        let before = super::made_for_homes();
        _color_slots(&again, &set(&[3]), &IndexMap::from_iter([(3, 2)]), &mut frame).expect("colours");
        assert_eq!(super::made_for_homes() - before, 0, "an instruction was made again for each that names a home");
    }

    #[test]
    fn test_the_plan_of_a_spill_reads_where_values_occur_from_the_postings_not_a_scan_of_the_body() {
        use crate::analysis::occurrences::Occurrences;
        let body = _three_blocks();
        let before = Occurrences::scans(&body);
        planned(&body, &set(&[1]), &mut Frame::new(0)).expect("plans");
        assert_eq!(Occurrences::scans(&body), before, "the body was scanned for the stable loads of the values");
    }

    /// The homes' intervals were found afresh for every home after every spill,
    /// across the whole body (2.0 G of compiling d_faces, 616 times),
    /// though a spill names one or two of them. An earlier body's are shifted,
    /// and only the homes the changed instructions name are found again;
    /// the answer is the same as finding all of them.
    #[test]
    fn test_a_spill_finds_the_intervals_of_the_homes_it_names_only() {
        use crate::analysis::intervals as ranges;
        use crate::model::ir::Semantics;
        use crate::model::lir::{Insn, LirBlock};
        let nop = |at: i64| {
            Arc::new(Insn::new(
                at,
                Some((at, at)),
                Some(Semantics { name: Some("nop".to_owned()), ..Semantics::new(Operation::Nothing) }),
                vec![],
                vec![],
            ))
        };
        let homes = [-6i64, -4, -2];
        let first = 1000u32;
        let names = |extra: bool| {
            let one = |home: usize| first + home as u32;
            // A run of its own for each short life, enough of them to be worth
            // keeping.
            let short = (0..600usize).flat_map(|pair| {
                let (block, home) = (400 + 2 * pair, pair % 3);
                [
                    (block, 0, BTreeSet::from([one(home)]), BTreeSet::new()),
                    (block + 1, 0, BTreeSet::new(), BTreeSet::from([one(home)])),
                ]
            });
            let mut named = vec![
                (5, 0, BTreeSet::from([one(0)]), BTreeSet::new()),
                (350, 1, BTreeSet::new(), BTreeSet::from([one(0)])),
                (10, 0, BTreeSet::from([one(1)]), BTreeSet::new()),
                (300, 1, BTreeSet::new(), BTreeSet::from([one(1)])),
                (30, usize::from(extra), BTreeSet::from([one(2)]), BTreeSet::new()),
                (380, 1, BTreeSet::new(), BTreeSet::from([one(2)])),
            ];
            named.extend(short);
            named
        };
        let sorted = |mut named: Vec<(usize, usize, BTreeSet<u32>, BTreeSet<u32>)>| {
            named.sort_by_key(|(block, at, _, _)| (*block, *at));
            named
        };
        // The instruction that stores to, or loads from, the home a name says.
        let naming = |at: i64, defined: &BTreeSet<u32>, used: &BTreeSet<u32>| {
            let cell = |values: &BTreeSet<u32>| {
                let home = homes[(*values.iter().next().expect("a home") - first) as usize];
                Loc::Mem(mem(Addr::new(Space::Frame, home), 2, Register::BP, home, 1))
            };
            let what = if defined.is_empty() {
                semantics(Operation::Move, "mov", vec![held(1, 2)], vec![cell(used)])
            } else {
                semantics(Operation::Move, "mov", vec![cell(defined)], vec![imm(0, 2)])
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
        };
        let chain = |extra_at_30: bool| {
            let plan = names(extra_at_30);
            let blocks: Vec<LirBlock> = (0..1600i64)
                .map(|at| {
                    let mut insns = if at == 30 && extra_at_30 {
                        vec![nop(0x1000), nop(0x1001 + at), nop(0x2000 + at)]
                    } else {
                        vec![nop(0x1001 + at), nop(0x2000 + at)]
                    };
                    for (block, position, defined, used) in &plan {
                        if *block as i64 == at {
                            insns[*position] = naming(0x3000 + 4 * at + *position as i64, defined, used);
                        }
                    }
                    LirBlock { succ: if at < 1599 { vec![at + 1] } else { vec![] }, ..LirBlock::new(at, insns) }
                })
                .collect();
            LirBody::new("chain", 0, blocks, IndexMap::default(), IndexMap::default())
        };
        // What names the homes in `body`, as the colouring finds it.
        fn named_in(
            body: &LirBody,
            plan: Vec<(usize, usize, BTreeSet<u32>, BTreeSet<u32>)>,
        ) -> Vec<super::Named<'_>> {
            plan.into_iter()
                .map(|(block, at, _, _)| super::Named {
                    block,
                    at,
                    names: super::Names::Insn(&body.blocks[block].insns[at]),
                })
                .collect()
        }
        let before = chain(false);
        let first_found = super::homes_kept(
            &before,
            &ranges::indexed_shared(&before),
            &named_in(&before, sorted(names(false))),
            &homes,
            first,
        );
        assert_eq!(first_found.len(), 3, "premise: all three homes are live somewhere");
        // A spill puts an instruction in block 30, which names the third home.
        let mut after = before.clone();
        let replaced = chain(true).blocks[30].insns.clone();
        after = after.with_blocks(
            after
                .blocks
                .iter()
                .map(|block| if block.at == 30 { block.with_insns(replaced.to_vec()) } else { block.clone() })
                .collect(),
        );
        let redone = super::homes_redone(&after);
        let kept = super::homes_kept(
            &after,
            &ranges::indexed_shared(&after),
            &named_in(&after, sorted(names(true))),
            &homes,
            first,
        );
        assert_eq!(super::homes_redone(&after) - redone, 1, "the homes the change does not name were found again");
        let index = ranges::indexed_shared(&after);
        let values: Vec<u32> = (first..first + 3).collect();
        let afresh =
            crate::analysis::occurrences::Occurrences::planned(&sorted(names(true))).ranges(&after, &index, &values);
        assert!(kept == afresh, "the kept intervals differ from finding them afresh");
    }

    /// A rewrite's edited block was compared with its parent by looking up
    /// every instruction of both in a hash (`facts intervals`, `intervals
    /// homes`: 380 + 71 Minstr became 625 + 74 on cells N=224). The
    /// instructions it kept are the same allocations, so runs of them are
    /// found by pointer; every run must hold one instruction in both, in
    /// order, and a list edited by insertions and removals alone has every
    /// instruction it shares in a run.
    /// The postings of a long block were made again, entry by entry, after
    /// every spill: 2978 of cells N=448's 28 700 Minstr. They are patched
    /// from what the rewrite changed, and must be what working them out
    /// gives, for insertions, removals and moves, and for values the edit
    /// adds and drops.
    #[test]
    fn test_the_postings_of_a_long_edited_block_are_those_of_working_them_out() {
        use crate::model::ir::Semantics;
        use crate::model::lir::{Insn, LirBlock};
        let make = |at: i64, defines: Vec<u32>, uses: Vec<u32>| {
            Arc::new(Insn::new(
                at,
                Some((at, at)),
                Some(Semantics { name: Some("nop".to_owned()), ..Semantics::new(Operation::Nothing) }),
                defines,
                uses,
            ))
        };
        let mut seed = 11u64;
        let mut next = |bound: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % bound
        };
        let mut insns: Vec<Arc<Insn>> =
            (0..120).map(|at| make(at, vec![at as u32 % 9], vec![(at as u32 * 7) % 11, at as u32 % 5])).collect();
        let block = |insns: &[Arc<Insn>]| LirBlock::new(0, insns.to_vec());
        let mut body = LirBody::new("long", 0, vec![block(&insns)], IndexMap::default(), IndexMap::default());
        crate::backend::postings::following(&body, |_| ());
        for round in 0..200 {
            for _ in 0..1 + next(6) {
                let at = next(insns.len());
                match next(4) {
                    0 => insns.insert(at, make(1000 + round, vec![20 + next(4) as u32], vec![next(12) as u32])),
                    1 if insns.len() > 60 => {
                        insns.remove(at);
                    }
                    2 => insns[at] = make(2000 + round, vec![next(9) as u32], vec![next(30) as u32]),
                    _ => {
                        let moved = insns.remove(at);
                        insns.insert(next(insns.len() + 1), moved);
                    }
                }
            }
            body = body.with_blocks(vec![block(&insns)]);
            crate::backend::postings::following(&body, |found| {
                assert!(
                    *found == crate::backend::postings::Postings::of(&body),
                    "round {round}: the patched postings differ from working them out"
                );
            });
        }
    }

    #[test]
    fn test_the_instructions_two_lists_share_are_found_by_pointer() {
        use crate::analysis::intervals::aligned;
        use crate::model::ir::Semantics;
        use crate::model::lir::Insn;
        let nop = |at: i64| {
            Arc::new(Insn::new(
                at,
                Some((at, at)),
                Some(Semantics { name: Some("nop".to_owned()), ..Semantics::new(Operation::Nothing) }),
                vec![],
                vec![],
            ))
        };
        let mut seed = 7u64;
        let mut next = |bound: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % bound
        };
        for round in 0..200 {
            let old: Vec<Arc<Insn>> = (0..20 + next(60)).map(|at| nop(at as i64)).collect();
            let mut new = old.clone();
            for _ in 0..next(12) {
                let at = next(new.len() + 1);
                match next(3) {
                    0 => new.insert(at, nop(1000 + at as i64)),
                    1 if !new.is_empty() => {
                        new.remove(at.min(new.len() - 1));
                    }
                    _ if !new.is_empty() => {
                        let moved = new.remove(at.min(new.len() - 1));
                        new.insert(next(new.len() + 1), moved);
                    }
                    _ => {}
                }
            }
            let runs = aligned(&old, &new).expect("alike").runs;
            let mut covered = 0;
            let mut after_old = 0;
            for (i, j, len) in &runs {
                assert!(*i >= after_old, "round {round}: runs out of order");
                for k in 0..*len {
                    assert!(Arc::ptr_eq(&old[i + k], &new[j + k]), "round {round}: a run holds different instructions");
                }
                (after_old, covered) = (i + len, covered + len);
            }
            let shared = old.iter().filter(|one| new.iter().any(|other| Arc::ptr_eq(one, other))).count();
            assert_eq!(covered, shared, "round {round}: an instruction both lists hold is in no run");
        }
    }

    #[test]
    fn test_a_spill_redoes_the_occurrences_of_the_blocks_it_changed_only() {
        let body = _three_blocks();
        crate::backend::postings::following(&body, |_| ());
        let (spilt, made) =
            spilled(&body, &set(&[1]), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        let before = crate::backend::postings::redone();
        crate::backend::postings::following(&spilt, |found| {
            assert!(
                *found == crate::backend::postings::Postings::of(&spilt),
                "the occurrences differ from working them out"
            )
        });
        assert!(
            crate::backend::postings::redone() - before <= 1,
            "{} blocks redone for a spill in one block",
            crate::backend::postings::redone() - before
        );
        // The values the rewrite made are found in the next one.
        let (again, _) =
            spilled(&spilt, &made, Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills again");
        crate::backend::postings::following(&again, |found| {
            assert!(*found == crate::backend::postings::Postings::of(&again))
        });
    }

    /// Each spill looked at every instruction of the body to rewrite those that
    /// name the values spilled: 8% of compiling d_alias (#559). A block
    /// that names none is left as it is.
    #[test]
    fn test_a_spill_looks_only_at_the_blocks_that_name_its_values() {
        let body = _three_blocks();
        let before = super::blocks_looked_at();
        let (spilt, _made) =
            spilled(&body, &set(&[1]), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
        assert!(
            super::blocks_looked_at() - before <= 1,
            "{} blocks looked at for a value in one",
            super::blocks_looked_at() - before
        );
        assert!(
            Arc::ptr_eq(&spilt.blocks[0].insns[0], &body.blocks[0].insns[0])
                && Arc::ptr_eq(&spilt.blocks[2].insns[1], &body.blocks[2].insns[1]),
            "the blocks that name nothing are as they were"
        );
    }

    /// The index fold may not move a second access through the same base.
    #[test]
    fn test_a_spilled_word_index_does_not_mutate_a_base_used_later() {
        let first = _read(0x102, 4, _far(0, Some(1)), &[5, 1]);
        let later = _read(0x104, 6, _far(2, None), &[5]);
        let result = _out(&_body(vec![_add(1, 2, 0x100), first, later]), &[1]);
        assert!(!result.iter().any(|one| _folded_add(one)), "{result:?}");
    }

    /// A base used later outside a memory operand was omitted from the death
    /// proof.
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

    /// UNWHITEFADE's frame counter, coalesced with its initial zero, lost its
    /// increment.
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
        let (done, _made) =
            spilled(&body, &set(&[1]), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
                .expect("spills");
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

    // -------------------------------------------
    // tests/test_rematerialized_definitions.py

    fn _remat_constant() -> Insn {
        insn(0, (0, 3), semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(64, 2)]), &[1], &[])
    }

    fn _push(
        at: i64,
        covers: (i64, i64),
        source: Loc,
        uses: &[u32],
    ) -> Insn {
        insn(at, covers, semantics(Operation::Push, "push", vec![], vec![source]), &[], uses)
    }

    fn _spilled_one(
        name: &str,
        insns: Vec<Insn>,
    ) -> LirBody {
        let body = LirBody::new(
            name,
            0,
            vec![LirBlock::new(0, insns.into_iter().map(Arc::new).collect())],
            IndexMap::default(),
            IndexMap::default(),
        );
        spilled(&body, &set(&[1]), Some(&mut Frame::new(0)), &crate::backend::classes::RegisterClasses::m16())
            .expect("spills")
            .0
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

    /// The anchor was a `nop` that select emitted: 220 NOPs across 146
    /// corpus objects, each where a definition owning bytes went away.
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
        assert!(owner.is_meta(), "{owner:?}");
        assert!(select::emit(owner.what.as_ref().unwrap(), 3, None, false, false, None).unwrap().code.is_empty());
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
