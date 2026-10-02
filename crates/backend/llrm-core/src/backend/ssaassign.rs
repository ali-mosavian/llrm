//! SSA assignment made real (Hack, ch. 4): the colours of a spilled SSA body
//! become the registers of its values, pinned for the allocator. A phi
//! becomes a parallel copy on each incoming edge between registers already
//! given; a value living through an instruction that takes its register moves
//! out of the way into a free one and back; an address whose registers cannot
//! form it reads a copy in one that can. Operands stay values: the allocator
//! writes the registers, two-address and constraint copies go through it as
//! always, and it places only the values those make.
//!
//! A body this cannot assign is left as it was, for Greedy; `fallbacks` counts those.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;

use crate::backend::allocate::{self, _whole};
use crate::backend::target::{self, Segments};
use crate::backend::{spiller, splitkit};
use crate::model::ir::{Held, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

thread_local! {
    static FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many bodies were left to Greedy because their colours could not be made registers.
pub fn fallbacks() -> usize {
    FALLBACKS.with(std::cell::Cell::get)
}

/// Count one more body left to Greedy.
pub fn fell_back() {
    FALLBACKS.with(|count| count.set(count.get() + 1));
}

thread_local! {
    static OVERRIDDEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many assigned bodies the allocator could not keep, and allocated afresh.
pub fn overrides() -> usize {
    OVERRIDDEN.with(std::cell::Cell::get)
}

/// Count one more assigned body the allocator could not keep.
pub fn overridden() {
    OVERRIDDEN.with(|count| count.set(count.get() + 1));
}

/// The group numbers this pass gives its copies: above any phi elimination makes.
const GROUPS: i64 = 1 << 40;

/// `body` out of SSA with each general value pinned where `colour` puts it (the
/// pins returned, the values this made included), or why not.
pub fn assigned(
    body: &LirBody,
    colour: &IndexMap<u32, Register>,
    general: &dyn Fn(u32) -> bool,
    segments: &Segments,
) -> Result<(LirBody, IndexMap<u32, Register>), String> {
    for one in body.insns() {
        for value in one.uses.iter().chain(&one.defines) {
            if general(*value) && !colour.contains_key(value) {
                return Err(format!("value#{value} has no colour"));
            }
        }
    }
    for block in &body.blocks {
        for phi in &block.phis {
            let coloured = colour.contains_key(&phi.result);
            if phi.incoming.iter().any(|(_, value)| colour.contains_key(value) != coloured) {
                return Err(format!("phi value#{} joins coloured and uncoloured values", phi.result));
            }
        }
    }
    let all: BTreeSet<u32> = body.insns().iter().flat_map(|one| one.defines.iter().chain(&one.uses).copied()).collect();
    let widths = spiller::_widest(body, &all);
    let width = |value: u32| widths.get(&value).copied().unwrap_or(2);
    let general_roots: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    let confined = allocate::classes(body, &BTreeSet::new(), segments);
    let class_of = |value: u32| -> Vec<Register> {
        let mut out: Vec<Register> = Vec::new();
        for register in target::order(confined.get(&value), segments) {
            let root = _whole(register);
            if general_roots.contains(&root) && !out.contains(&root) {
                out.push(root);
            }
        }
        out
    };
    let (_, live_out) = allocate::live(body);
    let mut pins = colour.clone();
    let mut next = spiller::_next_value(body);
    let mut group = GROUPS;
    let mut blocks: Vec<LirBlock> = Vec::new();
    for block in &body.blocks {
        let mut after: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); block.insns.len()];
        let mut live = live_out[&block.at].clone();
        for (position, one) in block.insns.iter().enumerate().rev() {
            after[position] = live.clone();
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for (position, one) in block.insns.iter().enumerate() {
            // No two values live at once share a register.
            let mut held: IndexMap<Register, u32> = IndexMap::default();
            for value in after[position].iter().chain(&one.defines) {
                if let Some(register) = colour.get(value) {
                    if let Some(other) = held.insert(*register, *value) {
                        if other != *value {
                            return Err(format!("{:#06x}: value#{other} and value#{value} are both in {register:?}", one.at));
                        }
                    }
                }
            }
            let Some(what) = &one.what else {
                insns.push(Arc::clone(one));
                continue;
            };
            // The registers the instruction itself takes.
            let mut taken: BTreeSet<Register> = target::requirements(what).values().map(|register| _whole(*register)).collect();
            taken.extend(one.requires.iter().chain(&one.delivers).map(|(_, register)| _whole(*register)));
            taken.extend(one.clobbers.iter().map(|register| _whole(*register)));
            if target::status_through_ax(what) {
                taken.insert(Register::EAX);
            }
            taken.retain(|register| general_roots.contains(register));
            let defined: BTreeSet<u32> = one.defines.iter().copied().collect();
            let mut busy: BTreeSet<Register> = taken.clone();
            busy.extend(after[position].iter().chain(&one.uses).chain(&one.defines).filter_map(|value| colour.get(value)).copied());
            let free = |busy: &BTreeSet<Register>, allowed: &[Register]| allowed.iter().copied().find(|register| !busy.contains(register));
            let mut before: Vec<Arc<Insn>> = Vec::new();
            let mut back: Vec<Arc<Insn>> = Vec::new();
            // What lives through and the instruction takes its register from: out of the way and back.
            for value in after[position].iter().copied().filter(|value| !defined.contains(value)) {
                let Some(home) = colour.get(&value).copied() else { continue };
                if !taken.contains(&home) {
                    continue;
                }
                let Some(spare) = free(&busy, &class_of(value)) else {
                    return Err(format!("{:#06x}: no register to keep value#{value} out of the instruction's way", one.at));
                };
                busy.insert(spare);
                let aside = next;
                next += 1;
                pins.insert(aside, spare);
                before.push(copied(one, aside, value, width(value)));
                back.push(copied(one, value, aside, width(value)));
            }
            // An address whose registers cannot form it reads a copy in one that can; with
            // none free, it trades places with a value that holds one, which is renamed
            // through the instruction and both are restored after it.
            let addressed_values: BTreeSet<u32> = what
                .dests
                .iter()
                .chain(&what.sources)
                .filter_map(|place| if let Loc::Mem(cell) = place { Some(cell) } else { None })
                .flat_map(|cell| [cell.base, cell.index])
                .flatten()
                .map(|held| held.value)
                .collect();
            let mut halves: IndexMap<(bool, usize, Role), u32> = IndexMap::default();
            let mut rename: IndexMap<u32, u32> = IndexMap::default();
            let mut trade: Vec<Arc<Insn>> = Vec::new();
            let mut restore: Vec<Arc<Insn>> = Vec::new();
            for (dest, side) in [(true, &what.dests), (false, &what.sources)] {
                for (index, place) in side.iter().enumerate() {
                    let Loc::Mem(cell) = place else { continue };
                    for (role, value, allowed) in roles(cell, colour) {
                        if let Some(copy) = rename.get(&value) {
                            if allowed.contains(&pins[copy]) {
                                halves.insert((dest, index, role), *copy);
                                continue;
                            }
                        }
                        if let Some(spare) = allowed.iter().copied().find(|register| !busy.contains(register)) {
                            busy.insert(spare);
                            let copy = next;
                            next += 1;
                            pins.insert(copy, spare);
                            before.push(copied(one, copy, value, 2));
                            halves.insert((dest, index, role), copy);
                            continue;
                        }
                        let home = colour[&value];
                        let holder = allowed.iter().copied().find_map(|register| {
                            let occupant = after[position].iter().chain(&one.uses).copied().find(|other| colour.get(other) == Some(&register))?;
                            let clear = occupant != value
                                && !addressed_values.contains(&occupant)
                                && !rename.contains_key(&occupant)
                                && !one.defines.iter().any(|made| colour.get(made).is_some_and(|got| *got == register || *got == home))
                                && !taken.contains(&register)
                                && !taken.contains(&home);
                            clear.then_some((register, occupant))
                        });
                        let Some((register, occupant)) = holder else {
                            return Err(format!("{:#06x}: no registers to form an address", one.at));
                        };
                        let (moved, displaced) = (next, next + 1);
                        next += 2;
                        pins.insert(moved, register);
                        pins.insert(displaced, home);
                        trade.push(copied(one, moved, value, width(value)));
                        trade.push(copied(one, displaced, occupant, width(occupant)));
                        rename.insert(value, moved);
                        rename.insert(occupant, displaced);
                        if after[position].contains(&value) {
                            restore.push(copied(one, value, moved, width(value)));
                        }
                        if after[position].contains(&occupant) {
                            restore.push(copied(one, occupant, displaced, width(occupant)));
                        }
                        halves.insert((dest, index, role), moved);
                    }
                }
            }
            let one = if halves.is_empty() && rename.is_empty() { Arc::clone(one) } else { addressed(one, &halves, &rename) };
            // What the instruction reads or writes in a register of its own choosing goes
            // there through a copy in the same parallel copy, and comes back after.
            let mut fixed: IndexMap<u32, u32> = IndexMap::default();
            if let Some(what) = &one.what {
                let mut reads: Vec<(u32, Register)> = one.requires.iter().map(|(held, register)| (held.value, _whole(*register))).collect();
                let mut writes: Vec<(u32, Register)> = one.delivers.iter().map(|(held, register)| (held.value, _whole(*register))).collect();
                for (place, register) in target::requirements(what) {
                    let side = if place.side == "dest" { &what.dests } else { &what.sources };
                    if let Some(Loc::Held(held)) = side.get(place.index) {
                        if place.side == "dest" {
                            writes.push((held.value, _whole(register)));
                        } else {
                            reads.push((held.value, _whole(register)));
                        }
                    }
                }
                // A tied instruction's first source is its result's register, not one of its own.
                let tied_first = crate::backend::twoaddr::ties(what).then(|| what.sources.first()).flatten().and_then(|first| if let Loc::Held(held) = first { Some(held.value) } else { None });
                for (value, register) in reads {
                    let Some(home) = pins.get(&value).copied() else { continue };
                    if home == register || fixed.contains_key(&value) || Some(value) == tied_first && writes.iter().all(|(_, into)| *into != register) {
                        continue;
                    }
                    let copy = next;
                    next += 1;
                    pins.insert(copy, register);
                    before.push(copied(&one, copy, value, width(value)));
                    fixed.insert(value, copy);
                }
                // A source the instruction reads where it takes another operand's register moves to a free one.
                let required: BTreeSet<u32> = fixed.keys().copied().collect();
                let mut spare_busy = busy.clone();
                spare_busy.extend(pins_of(&fixed, &pins));
                for value in one.uses.clone() {
                    let Some(home) = pins.get(&value).copied() else { continue };
                    if required.contains(&value) || !taken.contains(&home) || Some(value) == tied_first || after[position].contains(&value) {
                        continue;
                    }
                    if reads_in(&one, value, home) {
                        continue;
                    }
                    let Some(spare) = free(&spare_busy, &class_of(value)) else {
                        return Err(format!("{:#06x}: no register for value#{value} beside the instruction's own", one.at));
                    };
                    spare_busy.insert(spare);
                    let copy = next;
                    next += 1;
                    pins.insert(copy, spare);
                    before.push(copied(&one, copy, value, width(value)));
                    fixed.insert(value, copy);
                }
                for (value, register) in writes {
                    let Some(home) = colour.get(&value).copied() else { continue };
                    if home == register || fixed.contains_key(&value) {
                        continue;
                    }
                    let made = next;
                    next += 1;
                    pins.insert(made, register);
                    back.push(copied(&one, value, made, width(value)));
                    fixed.insert(value, made);
                }
            }
            let one = if fixed.is_empty() { one } else { spiller::_renamed(&one, &fixed) };
            if !trade.is_empty() {
                emit_group(&trade, &mut group, &mut before);
            }
            if !restore.is_empty() {
                let mut grouped = Vec::new();
                emit_group(&restore, &mut group, &mut grouped);
                back.splice(0..0, grouped);
            }
            emit_group(&before, &mut group, &mut insns);
            insns.push(one);
            emit_group(&back, &mut group, &mut insns);
        }
        let phis = block.phis.iter().filter(|phi| !colour.contains_key(&phi.result)).cloned().collect();
        blocks.push(LirBlock { phis, ..block.with_insns(insns) });
    }
    // Each coloured phi: a parallel copy on each incoming edge, defining its result.
    let mut on_edge: IndexMap<(i64, i64), Vec<(u32, u32)>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            if !colour.contains_key(&phi.result) {
                continue;
            }
            for (from, value) in &phi.incoming {
                on_edge.entry((*from, block.at)).or_default().push((phi.result, *value));
            }
        }
    }
    let mut next_at = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let mut bridges: Vec<LirBlock> = Vec::new();
    for ((from, to), pairs) in &on_edge {
        let at = blocks.iter().position(|block| block.at == *from).ok_or("an edge from no block")?;
        let source = blocks[at].clone();
        let beside = Arc::clone(source.insns.last().ok_or("a phi's predecessor is empty")?);
        let copies: Vec<Arc<Insn>> = pairs.iter().map(|(into, out_of)| copied(&beside, *into, *out_of, width(*into))).collect();
        let mut grouped: Vec<Arc<Insn>> = Vec::new();
        emit_group(&copies, &mut group, &mut grouped);
        if source.succ.len() == 1 {
            let tail = splitkit::_tail(&source);
            let mut insns = source.insns.clone();
            insns.splice(tail..tail, grouped);
            blocks[at] = source.with_insns(insns);
        } else {
            let bridge = next_at;
            next_at += 1;
            let mut jump = Insn::new(
                beside.at,
                Some((beside.at, beside.at)),
                Some(Semantics { name: Some("jmp".to_owned()), target: Some(*to), ..Semantics::new(Operation::Jump) }),
                Vec::new(),
                Vec::new(),
            );
            jump.op = beside.op.clone();
            grouped.push(Arc::new(jump));
            bridges.push(LirBlock { succ: vec![*to], ..LirBlock::new(bridge, grouped) });
            let retargeted: Vec<Arc<Insn>> = source
                .insns
                .iter()
                .map(|one| match &one.what {
                    Some(what) if what.target == Some(*to) && matches!(what.op, Operation::Jump | Operation::Branch) => {
                        let mut made = (**one).clone();
                        made.what = Some(Semantics { target: Some(bridge), ..what.clone() });
                        Arc::new(made)
                    }
                    _ => Arc::clone(one),
                })
                .collect();
            let succ = source.succ.iter().map(|one| if one == to { bridge } else { *one }).collect();
            blocks[at] = LirBlock { succ, ..source.with_insns(retargeted) };
            if let Some(target) = blocks.iter_mut().find(|block| block.at == *to) {
                for phi in &mut target.phis {
                    for (source, _) in &mut phi.incoming {
                        if source == from {
                            *source = bridge;
                        }
                    }
                }
            }
        }
    }
    blocks.extend(bridges);
    // What phis are left (values no general register holds) phi elimination lowers now: the body leaves SSA here.
    let mut done = crate::backend::phielim::eliminated(&body.with_blocks(blocks))?;
    // The coalescer keeps values pinned apart apart.
    done.pins.extend(pins.iter().map(|(value, register)| (*value, *register)));
    Ok((done, pins))
}

/// The registers the copies in `fixed` were pinned to.
fn pins_of(fixed: &IndexMap<u32, u32>, pins: &IndexMap<u32, Register>) -> Vec<Register> {
    fixed.values().filter_map(|copy| pins.get(copy).copied()).collect()
}

/// Whether `one` reads `value` where the instruction requires that very register.
fn reads_in(one: &Insn, value: u32, register: Register) -> bool {
    one.requires.iter().any(|(held, wanted)| held.value == value && _whole(*wanted) == register)
        || one.what.as_ref().is_some_and(|what| {
            target::requirements(what).iter().any(|(place, wanted)| {
                place.side == "source" && _whole(*wanted) == register && matches!(what.sources.get(place.index), Some(Loc::Held(held)) if held.value == value)
            })
        })
}

/// `into := out_of`, a copy owning no bytes, beside `beside`.
fn copied(beside: &Insn, into: u32, out_of: u32, width: u32) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = Insn::new(
        beside.at,
        Some((at, at)),
        Some(Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Held(Held { value: into, width })],
            sources: vec![Loc::Held(Held { value: out_of, width })],
            ..Semantics::new(Operation::Move)
        }),
        vec![into],
        vec![out_of],
    );
    made.op = beside.op.clone();
    Arc::new(made)
}

/// `copies` as one parallel copy.
fn emit_group(copies: &[Arc<Insn>], group: &mut i64, out: &mut Vec<Arc<Insn>>) {
    if copies.is_empty() {
        return;
    }
    *group += 1;
    for one in copies {
        let mut made = (**one).clone();
        made.group = Some(*group);
        out.push(Arc::new(made));
    }
}

/// `one` reading the copies its addresses' halves were given (`halves`, by side,
/// operand and half), and reading and writing `rename`'s values in place of others.
fn addressed(one: &Insn, halves: &IndexMap<(bool, usize, Role), u32>, rename: &IndexMap<u32, u32>) -> Arc<Insn> {
    let what = one.what.as_ref().expect("an address has semantics");
    let swap = |held: Held| Held { value: rename.get(&held.value).copied().unwrap_or(held.value), ..held };
    let place = |dest: bool, index: usize, place: &Loc| -> Loc {
        match place {
            Loc::Mem(cell) => {
                let half = |role: Role, held: Option<Held>| held.map(|held| match halves.get(&(dest, index, role)) {
                    Some(copy) => Held { value: *copy, ..held },
                    None => swap(held),
                });
                Loc::Mem(Mem { base: half(Role::Base, cell.base), index: half(Role::Index, cell.index), selector: cell.selector.map(swap), ..cell.clone() })
            }
            Loc::Held(held) => Loc::Held(swap(*held)),
            other => other.clone(),
        }
    };
    let mut made = one.clone();
    made.what = Some(Semantics {
        dests: what.dests.iter().enumerate().map(|(index, one)| place(true, index, one)).collect(),
        sources: what.sources.iter().enumerate().map(|(index, one)| place(false, index, one)).collect(),
        ..what.clone()
    });
    // A value an operand named and no longer names was read through a copy.
    let named = |what: &Semantics| -> BTreeSet<u32> { what.dests.iter().chain(&what.sources).flat_map(crate::model::ir::values).map(|held| held.value).collect() };
    let (was, now) = (named(what), named(made.what.as_ref().expect("set above")));
    let mut uses: Vec<u32> = Vec::new();
    for value in one.uses.iter().map(|value| rename.get(value).copied().unwrap_or(*value)).chain(halves.values().copied()) {
        let replaced = was.contains(&value) && !now.contains(&value);
        if !replaced && !uses.contains(&value) {
            uses.push(value);
        }
    }
    made.uses = uses;
    made.requires = one.requires.iter().map(|(held, register)| (swap(*held), *register)).collect();
    Arc::new(made)
}


/// Which half of an address a value forms.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Role {
    Base,
    Index,
}

/// The halves of `cell` whose word value sits in a register that cannot form
/// it, each with the registers that can.
fn roles(cell: &Mem, colour: &IndexMap<u32, Register>) -> Vec<(Role, u32, BTreeSet<Register>)> {
    let bx: BTreeSet<Register> = target::WORD_BASES.iter().map(|one| _whole(*one)).collect();
    let indexes: BTreeSet<Register> = target::WORD_INDEXES.iter().map(|one| _whole(*one)).collect();
    let bases: BTreeSet<Register> = target::ADDRESSING.iter().map(|one| _whole(*one)).filter(|one| *one != Register::EBP).collect();
    let word = |held: &Option<Held>| held.filter(|held| held.width == 2 && colour.contains_key(&held.value));
    let mut out = Vec::new();
    match (word(&cell.base), word(&cell.index)) {
        (Some(base), None) => {
            let allowed = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) { indexes } else { bases };
            if !allowed.contains(&colour[&base.value]) {
                out.push((Role::Base, base.value, allowed));
            }
        }
        (Some(base), Some(index)) if cell.scale == 1 => {
            let (b, i) = (colour[&base.value], colour[&index.value]);
            let fits = base.value != index.value && ((bx.contains(&b) && indexes.contains(&i)) || (indexes.contains(&b) && bx.contains(&i)));
            if !fits {
                if bx.contains(&b) {
                    out.push((Role::Index, index.value, indexes));
                } else if indexes.contains(&b) {
                    out.push((Role::Index, index.value, bx));
                } else if base.value != index.value && bx.contains(&i) {
                    out.push((Role::Base, base.value, indexes));
                } else if base.value != index.value && indexes.contains(&i) {
                    out.push((Role::Base, base.value, bx));
                } else {
                    out.push((Role::Base, base.value, bx));
                    out.push((Role::Index, index.value, indexes));
                }
            }
        }
        (base, Some(index)) => {
            if !indexes.contains(&colour[&index.value]) {
                out.push((Role::Index, index.value, indexes));
            }
            if let Some(base) = base {
                if !bx.contains(&colour[&base.value]) {
                    out.push((Role::Base, base.value, bx));
                }
            }
        }
        (None, None) => {}
    }
    out
}


