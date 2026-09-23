//! Port of `qbopt/analysis/observers.py`: which cells nothing outside a body
//! can read.
//!
//! A store is dead when everything after it overwrites the cell before reading
//! it -- or when nothing after it can read the cell at all. `avail.dead_stores`
//! answers the first; this answers what the second needs: at a call or an exit,
//! which cells are still observable.
//!
//! A frame cell below bp is gone when the body returns, and nothing a call runs
//! can reach it unless its address was taken. A program variable is visible to
//! every other body that names it, to anything handed its address, and to a
//! later activation of the same body -- so only the main body, which nothing
//! calls, can own one, and only a variable no other code and no data names.
//!
//! Error and event handlers resume inside a body without a CFG edge, so a
//! module with either has nothing private.
//!

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};

use iced_x86::Register;
use crate::support::hash::IndexMap;

use super::alias::{self, PointsTo};
use super::frameescape;
use crate::abi::runtime;
use crate::model::memory::{Identity, MemoryKind, MemoryObject};
use crate::model::mir::{Arg, Kind, MemRef, MirBody, Symbol, Value};
use crate::abi::{events, handlers};
use crate::frontends::bc::blocks::Block;
use crate::frontends::bc::declen::Insn;
use crate::frontends::bc::extent;
use crate::objectfile::module::{Module, Space};
use crate::objectfile::omf;

// Past every displacement a 16-bit segment can hold.
pub const BEYOND: i64 = 1 << 17;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exposure {
    pub data: Option<i64>,
    pub main: Option<i64>,
    // Program-data ranges some code outside the owning body, some data, or a
    // taken address can reach.
    pub everywhere: Vec<(i64, i64)>,
    // The ranges each body's own direct operands name, by body seed.
    pub direct: IndexMap<i64, Vec<(i64, i64)>>,
}

/// A descriptor's effective identity, independent of relocation spelling.
fn _descriptor(symbol: &Symbol) -> (Space, i64, i64, u32) {
    (symbol.space, symbol.index, symbol.offset + symbol.addend, symbol.width)
}

/// Descriptor addresses carried by ordinary SSA values.
///
/// A dynamic array's descriptor is a frame/global object while its elements
/// live in a separate allocation. Pointer provenance deliberately describes
/// the address value itself, so the ownership edge from descriptor to current
/// allocation is tracked separately here.
fn _descriptor_values(body: &MirBody) -> IndexMap<Value, BTreeSet<(Space, i64, i64, u32)>> {
    let mut values: IndexMap<Value, BTreeSet<(Space, i64, i64, u32)>> = IndexMap::default();
    loop {
        let before = values.clone();
        for block in &body.blocks {
            for phi in &block.phis {
                let incoming: BTreeSet<(Space, i64, i64, u32)> = phi
                    .incoming
                    .values()
                    .flat_map(|value| values.get(value).into_iter().flatten().copied())
                    .collect();
                if !incoming.is_empty() {
                    values.insert(phi.result, incoming);
                }
            }
            for op in &block.ops {
                let [Arg::Held(result)] = &op.results[..] else {
                    continue;
                };
                let result = result.value;
                match (op.kind, &op.args[..]) {
                    (Kind::Address, [Arg::FrameAddress(address)]) => {
                        values.insert(result, BTreeSet::from([(Space::Frame, 0, address.offset, address.width)]));
                    }
                    (Kind::Copy | Kind::Address, [Arg::Symbol(symbol)]) => {
                        values.insert(result, BTreeSet::from([_descriptor(symbol)]));
                    }
                    (Kind::Copy, [Arg::Held(source)]) if values.contains_key(&source.value) => {
                        let copied = values[&source.value].clone();
                        values.insert(result, copied);
                    }
                    _ => {}
                }
            }
        }
        if values == before {
            return values;
        }
    }
}

/// Owned allocations exposed by passing their descriptors to callees.
///
/// ARG operations describe the physical stack while CALL arguments are not
/// yet recovered for every BASIC procedure. Track that stack within each
/// block. A known cleanup consumes only its suffix, preserving an early
/// outer argument across a nested call; an unknown call conservatively owns
/// every pending argument.
///
/// Allocation/reallocation is the one non-publication case: its descriptor
/// argument precedes the generation the call creates. Erasure destroys the
/// current generation rather than exposing it to later program code.
fn _descriptor_publications(
    body: &MirBody,
    pointers: &PointsTo,
    calls: &IndexMap<i64, String>,
) -> BTreeSet<MemoryObject> {
    let values = _descriptor_values(body);
    let mut owned: IndexMap<(Space, i64, i64, u32), BTreeSet<MemoryObject>> = IndexMap::default();
    let mut provenances: Vec<_> = pointers.values.values().cloned().collect();
    for block in &body.blocks {
        for op in &block.ops {
            for r#ref in op.loads.iter().chain(&op.stores) {
                if let Some(provenance) = pointers.reference(r#ref) {
                    provenances.push(provenance);
                }
            }
        }
    }
    for provenance in &provenances {
        for one in &provenance.slices {
            if one.object.kind != MemoryKind::Allocation {
                continue;
            }
            if let Some(Identity::Tuple(identity)) = &one.object.identity {
                if let Some(Identity::Symbol(symbol)) = identity.first() {
                    owned.entry(_descriptor(symbol)).or_default().insert(one.object.clone());
                }
            }
        }
    }
    if owned.is_empty() {
        return BTreeSet::new();
    }

    let descriptors = |arg: &Arg| -> BTreeSet<(Space, i64, i64, u32)> {
        match arg {
            Arg::Held(held) => values.get(&held.value).cloned().unwrap_or_default(),
            Arg::FrameAddress(address) => BTreeSet::from([(Space::Frame, 0, address.offset, address.width)]),
            Arg::Symbol(symbol) => BTreeSet::from([_descriptor(symbol)]),
            _ => BTreeSet::new(),
        }
    };

    let mut published: BTreeSet<MemoryObject> = BTreeSet::new();
    for block in &body.blocks {
        let mut pending: Vec<(i64, BTreeSet<(Space, i64, i64, u32)>)> = Vec::new();
        for op in &block.ops {
            if op.kind == Kind::Arg && op.args.len() == 1 {
                let argument = &op.args[0];
                let width = match argument {
                    Arg::Held(one) => one.width,
                    Arg::Const(one) => one.width,
                    Arg::Symbol(one) => one.width,
                    Arg::FrameAddress(one) => one.width,
                    Arg::FrameSelector(one) => one.width,
                    Arg::Cell(_) | Arg::Opaque(_) => 0,
                };
                pending.push((i64::from(width), descriptors(argument)));
                continue;
            }
            if op.kind != Kind::Call {
                continue;
            }

            let name = calls.get(&op.at);
            let cleanup = match &op.array {
                Some(array) => Some(4 * array.bounds.len() as i64 + 6),
                None => {
                    let rule = runtime::contract(name.map(String::as_str));
                    rule.cleanup.map(|cleanup| cleanup + rule.caller_cleanup)
                }
            };
            let consumed;
            match cleanup {
                None => {
                    consumed = std::mem::take(&mut pending);
                }
                Some(cleanup) => {
                    let (mut total, mut first) = (0, pending.len());
                    while first > 0 && total < cleanup {
                        first -= 1;
                        total += pending[first].0;
                    }
                    if total != cleanup {
                        consumed = std::mem::take(&mut pending);
                    } else {
                        consumed = pending.split_off(first);
                    }
                }
            }

            let direct: Vec<_> = op.args.iter().map(descriptors).collect();
            let lifecycle = op.array.is_some() || name.is_some_and(|name| name == "B$ERAS");
            if !lifecycle {
                for found in consumed.iter().map(|one| &one.1).chain(&direct) {
                    for descriptor in found {
                        published.extend(owned.get(descriptor).into_iter().flatten().cloned());
                    }
                }
            }
        }

        // An unmatched physical argument is malformed or leaves this block by
        // an edge the stack reconstruction cannot account for. Losing DSE is
        // safer than declaring its reachable allocation invisible.
        for (_width, found) in &pending {
            for descriptor in found {
                published.extend(owned.get(descriptor).into_iter().flatten().cloned());
            }
        }
    }
    published
}

thread_local! {
    /// Python's `_known`, keyed by the module's identity.
    static KNOWN: RefCell<Vec<(Weak<Module>, Option<Exposure>)>> = const { RefCell::new(Vec::new()) };
}

/// The module's program-data visibility, or None where it cannot be told.
pub fn exposure(found: &Rc<Module>, blocks: &[Block]) -> Result<Option<Exposure>, String> {
    let held = KNOWN.with(|known| {
        known
            .borrow()
            .iter()
            .find(|(module, _)| module.upgrade().is_some_and(|module| Rc::ptr_eq(&module, found)))
            .map(|(_, got)| got.clone())
    });
    if let Some(held) = held {
        return Ok(held);
    }
    let got = _exposure(found, blocks)?;
    KNOWN.with(|known| {
        let mut known = known.borrow_mut();
        known.retain(|(module, _)| module.strong_count() > 0);
        known.push((Rc::downgrade(found), got.clone()));
    });
    Ok(got)
}

fn _exposure(found: &Module, blocks: &[Block]) -> Result<Option<Exposure>, String> {
    if !events::handler_entries(found).is_empty() || !handlers::error_entries(found).is_empty() {
        return Ok(None);
    }
    let Ok(partition) = extent::partition(found) else {
        return Ok(None);
    };
    if !partition.complete() {
        return Ok(None);
    }
    let mut data = found.program_data;
    if data.is_some_and(|one| omf::combines(&found.records).get(&one) == Some(&omf::COMBINE_COMMON)) {
        data = None;
    }
    let segments = omf::segments(&found.records);
    let debug: BTreeSet<i64> = segments
        .iter()
        .enumerate()
        .filter(|(_, one)| one.as_ref().is_some_and(|one| one.0.starts_with("$$")))
        .map(|(index, _)| index as i64)
        .collect();
    let fixups: Vec<omf::Fixup> =
        omf::fixups(&found.records).into_iter().filter(|one| !one.seg.is_some_and(|seg| debug.contains(&seg))).collect();
    let publics: Vec<(i64, i64)> =
        omf::public_definitions(&found.records).map_err(|error| error.to_string())?.into_values().collect();
    let mut named: BTreeSet<i64> = fixups
        .iter()
        .filter(|one| one.target == "segment" && Some(one.index) == data)
        .map(|one| one.disp)
        .collect();
    named.extend(publics.iter().filter(|(seg, _)| Some(*seg) == data).map(|(_, offset)| *offset));
    let named: Vec<i64> = named.into_iter().collect();
    let reach = |disp: i64| -> (i64, i64) {
        let after = named.partition_point(|&one| one <= disp);
        (disp, if after < named.len() { named[after] } else { BEYOND })
    };

    let insns: IndexMap<usize, &Insn> = blocks.iter().flat_map(|block| &block.insns).map(|insn| (insn.at, insn)).collect();
    let mut by_field: IndexMap<usize, &Insn> = IndexMap::default();
    for insn in insns.values() {
        for field in [insn.disp_at, insn.imm_at].into_iter().flatten() {
            by_field.insert(field, insn);
        }
    }
    let mut everywhere: Vec<(i64, i64)> =
        publics.iter().filter(|(seg, _)| Some(*seg) == data).map(|(_, offset)| reach(*offset)).collect();
    let mut direct: IndexMap<i64, Vec<(i64, i64)>> = IndexMap::default();
    for one in &fixups {
        if one.target != "segment" || Some(one.index) != data {
            continue;
        }
        let insn = if one.seg == Some(found.seg) { by_field.get(&(one.offset as usize)).copied() } else { None };
        let operand = insn.is_some_and(|insn| Some(one.offset as usize) == insn.disp_at);
        let owner = match insn {
            Some(insn)
                if operand && insn.memory_base() == Register::None && insn.memory_index() == Register::None =>
            {
                partition
                    .bodies
                    .iter()
                    .find(|body| body.ranges.iter().any(|&(lo, hi)| lo <= insn.at && insn.at < hi))
                    .map(|body| body.seed as i64)
            }
            _ => None,
        };
        if let Some(owner) = owner {
            direct.entry(owner).or_default().push(reach(one.disp));
        } else if insn.is_some() && !operand {
            everywhere.push((one.disp, BEYOND));
        } else {
            everywhere.push(reach(one.disp));
        }
    }
    let main = partition
        .bodies
        .iter()
        .find(|body| {
            body.kind == extent::BodyKind::Main
                && !found.publics.contains(&(body.seed as i64))
                && !found.targets.contains(&(body.seed as i64))
        })
        .map(|body| body.seed as i64);
    Ok(Some(Exposure { data, main, everywhere, direct }))
}

/// A test for the cells no call and no exit of `body` can observe.
///
/// Without a module there is no program data to know the reach of, but a
/// body the raise calls sealed still owns its frame: nothing resumes inside
/// it, so the frame rule needs nothing the module would have said.
#[allow(clippy::type_complexity)]
pub fn private(
    body: &Rc<MirBody>,
    found: Option<&Rc<crate::objectfile::module::Module>>,
    blocks: Option<&Rc<Vec<crate::frontends::bc::blocks::Block>>>,
) -> Result<Option<Box<dyn Fn(&MemRef) -> bool>>, String> {
    let exposed = match (found, blocks) {
        (Some(found), Some(blocks)) => match exposure(found, blocks)? {
            Some(exposed) => exposed,
            None => return Ok(None),
        },
        _ => {
            if !body.sealed {
                return Ok(None);
            }
            Exposure { data: None, main: None, everywhere: Vec::new(), direct: IndexMap::default() }
        }
    };
    let calls: IndexMap<i64, String> = found.map(|found| found.calls.clone()).unwrap_or_default();
    let escapes = frameescape::analysed(body);
    let pointers = alias::pointers(body)?;
    let mut published: BTreeSet<MemoryObject> = pointers.escaped.clone();
    published.extend(_descriptor_publications(body, &pointers, &calls));
    let read_allocations: BTreeSet<Symbol> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| &op.loads)
        .filter_map(|r#ref| r#ref.allocation)
        .collect();
    let mut unknown_frame_publication = false;
    let publishing = [Kind::Arg, Kind::Call, Kind::Return, Kind::Escape, Kind::Opaque];
    for block in &body.blocks {
        for op in &block.ops {
            if !publishing.contains(&op.kind) && !op.barrier() && op.exits.is_empty() {
                continue;
            }
            // Interprocedural summaries spell a callee's reachable memory as
            // explicit call loads/stores.  Those canonical objects are
            // observable even when ABI physicalization has split the pointer
            // value that originally led to them.
            if op.kind == Kind::Call {
                for r#ref in &op.loads {
                    if let Some(provenance) = pointers.reference(r#ref) {
                        published.extend(provenance.slices.into_iter().map(|one| one.object));
                    }
                }
            }
            let mut values: BTreeSet<Value> = op.uses.iter().chain(&op.exits).copied().collect();
            values.extend(op.args.iter().filter_map(|arg| match arg {
                Arg::Held(held) => Some(held.value),
                _ => None,
            }));
            for value in &values {
                if let Some(provenance) = pointers.values.get(value) {
                    published.extend(provenance.slices.iter().map(|one| one.object.clone()));
                }
            }
            // A direct address operand has no SSA value through which to
            // match a frontend-defined object identity.  Conservatively
            // expose every canonical frame object rather than guessing from
            // coincident offsets.
            unknown_frame_publication |= op.args.iter().any(|arg| matches!(arg, Arg::FrameAddress(_)));
        }
    }
    let frame = escapes.reach.is_some() && escapes.opaque_addresses.is_empty();
    let reach = escapes.reach.unwrap_or_default();
    let statics = exposed.data.is_some() && Some(body.entry) == exposed.main;
    let mut seen = exposed.everywhere.clone();
    seen.extend(
        exposed.direct.iter().filter(|(seed, _)| **seed != body.entry).flat_map(|(_, ranges)| ranges.iter().copied()),
    );

    let unobserved = move |r#ref: &MemRef| -> bool {
        let provenance = pointers.reference(r#ref);
        if let Some(provenance) = provenance.filter(|provenance| !provenance.slices.is_empty()) {
            let canonical_allocation = provenance.slices.iter().all(|one| {
                one.object.kind == MemoryKind::Allocation
                    && one.object.extent.is_some_and(|extent| {
                        0 <= one.low && one.low < one.high && one.high + one.width - 1 <= extent
                    })
                    && match &one.object.identity {
                        Some(Identity::Tuple(identity)) => match identity.first() {
                            Some(Identity::Symbol(symbol)) => !read_allocations.contains(symbol),
                            _ => true,
                        },
                        _ => true,
                    }
            });
            let canonical_frame = !unknown_frame_publication
                && provenance.slices.iter().all(|one| {
                    one.object.kind == MemoryKind::Frame
                        && one.object.extent.is_some_and(|extent| {
                            0 <= one.low && one.low < one.high && one.high + one.width - 1 <= extent
                        })
                });
            if canonical_frame || canonical_allocation {
                // Canonical object identity is a complete answer in both
                // directions.  A current owning allocation is private for the
                // same reason as a current frame object once no load in this
                // body can still observe it: only publishing a pointer can
                // then let code outside the body inspect its contents.
                // Passing the *descriptor's* frame address to its allocator or
                // deallocator does not publish the separate allocation object.
                // Falling through to the raw BP-offset rule after finding a
                // published object made that same object private again and let
                // DSE erase stores before a BYREF call.
                return provenance.slices.iter().all(|one| !published.contains(&one.object));
            }
        }
        let Some(addr) = r#ref.addr else {
            return false;
        };
        if addr.base != Register::None || r#ref.base.is_some() || r#ref.segment.is_some() {
            return false;
        }
        let (lo, hi) = (addr.disp, addr.disp + i64::from(r#ref.width));
        if addr.space == Space::Frame {
            return frame && hi <= 0 && !reach.iter().any(|(start, end)| *start < hi && lo < *end);
        }
        if addr.space == Space::Segment && Some(addr.index) == exposed.data {
            return statics && !seen.iter().any(|(start, end)| *start < hi && lo < *end);
        }
        false
    };

    Ok(Some(Box::new(unobserved)))
}

#[cfg(test)]
#[path = "observers_tests.rs"]
mod tests;
