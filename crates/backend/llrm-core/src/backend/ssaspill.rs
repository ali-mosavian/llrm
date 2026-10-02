//! Braun & Hack spilling on the SSA LIR ("Register Spilling and Live-Range
//! Splitting for SSA-Form Programs", CC 2009), run before `PhiElimination`.
//!
//! Per block, Belady's rule over the values a register holds (furthest next
//! use leaves, next uses seen across blocks with a loop-exit distance); a use
//! the set lacks is reloaded; where a successor expects a value in a register
//! that a predecessor does not end with, the edge reloads it. Whether a value
//! is kept into a loop, and where its store goes, follow block frequencies.
//! Reloads redefine the value, and `ssarepair` restores SSA.
//!
//! What a later phase will take (x87 compares' AX, constrained registers) is
//! stated on the instruction as `requires`, `delivers` and `clobbers`; this
//! reserves what the instruction states and nothing else.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::frequency::Frequency;
use crate::analysis::intervals as ranges;
use crate::backend::allocate::{self, Classes, _whole};
use crate::backend::frame::Frame;
use crate::backend::target::{self, Segments};
use crate::backend::{spiller, splitkit, ssarepair, twoaddr};
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

const FAR: i64 = 1 << 40;
/// Leaving a loop: what is used after it is used far from inside it.
const EXIT: i64 = 1 << 20;

thread_local! {
    static CHANGES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many bodies this phase has changed on this thread: what a caller reads to learn whether a run did anything.
pub fn changes() -> usize {
    CHANGES.with(std::cell::Cell::get)
}

pub struct SsaSpill {
    pub frame: Rc<RefCell<Frame>>,
    pub segments: Segments,
}

impl SsaSpill {
    pub const NAME: &'static str = "ssaspill";
}

impl LIRTransform for SsaSpill {
    fn class_name(&self) -> &'static str {
        "SsaSpill"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        // A body nothing was done to is returned as it came: a copy loses what later phases know of it.
        let made = changed(&body, &mut self.frame.borrow_mut(), &self.segments)?;
        if made.is_some() {
            CHANGES.with(|count| count.set(count.get() + 1));
        }
        Ok(made.unwrap_or(body))
    }
}

/// The widest each value of `values` is read or written, a phi's result and
/// arguments counting as one value: a value only phis name has no width of its own.
fn widths_through_phis(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, u32> {
    let mut web: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            let members: BTreeSet<u32> = std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)).collect();
            for one in &members {
                web.entry(*one).or_default().extend(members.iter().copied());
            }
        }
    }
    // Close each web.
    loop {
        let mut grew = false;
        let keys: Vec<u32> = web.keys().copied().collect();
        for key in keys {
            let reach: BTreeSet<u32> = web[&key].iter().flat_map(|one| web.get(one).cloned().unwrap_or_default()).collect();
            if !reach.is_subset(&web[&key]) {
                web.get_mut(&key).expect("a member").extend(reach);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let every: BTreeSet<u32> = values.iter().flat_map(|one| web.get(one).cloned().unwrap_or_else(|| BTreeSet::from([*one]))).collect();
    let known = spiller::_widest(body, &every);
    values
        .iter()
        .map(|one| {
            let members = web.get(one).cloned().unwrap_or_else(|| BTreeSet::from([*one]));
            (*one, members.iter().filter_map(|member| known.get(member)).copied().max().unwrap_or(2))
        })
        .collect()
}

/// What each block does with each value: where it reads it, in order.
struct Flow {
    /// The positions a block reads a value at; the block's length stands for its end,
    /// where the phis of its successors read their arguments.
    uses: IndexMap<i64, IndexMap<u32, Vec<usize>>>,
    /// Where a block defines a value (a phi result before position 0).
    defines: IndexMap<i64, IndexMap<u32, usize>>,
    live_in: allocate::Live,
    live_out: allocate::Live,
    depth: IndexMap<i64, u32>,
    from_top: IndexMap<i64, IndexMap<u32, i64>>,
    length: IndexMap<i64, usize>,
    succ: IndexMap<i64, Vec<i64>>,
}

impl Flow {
    fn of(body: &LirBody, loops: &[crate::analysis::loops::Loop]) -> Self {
        let (live_in, live_out) = allocate::live(body);
        let mut uses: IndexMap<i64, IndexMap<u32, Vec<usize>>> = IndexMap::default();
        let mut defines: IndexMap<i64, IndexMap<u32, usize>> = IndexMap::default();
        let mut length: IndexMap<i64, usize> = IndexMap::default();
        for block in &body.blocks {
            let (mut read, mut made): (IndexMap<u32, Vec<usize>>, IndexMap<u32, usize>) = Default::default();
            for (at, one) in block.insns.iter().enumerate() {
                for value in &one.uses {
                    let list = read.entry(*value).or_default();
                    if list.last() != Some(&at) {
                        list.push(at);
                    }
                }
                for value in &one.defines {
                    made.insert(*value, at);
                }
            }
            for phi in &block.phis {
                made.insert(phi.result, 0);
            }
            uses.insert(block.at, read);
            defines.insert(block.at, made);
            length.insert(block.at, block.insns.len());
        }
        for block in &body.blocks {
            for phi in &block.phis {
                for (from, value) in &phi.incoming {
                    let end = length[from];
                    let list = uses.get_mut(from).expect("a predecessor").entry(*value).or_default();
                    if list.last() != Some(&end) {
                        list.push(end);
                    }
                }
            }
        }
        let mut flow = Self {
            uses,
            defines,
            live_in,
            live_out,
            depth: ranges::depths_in(body, loops),
            from_top: body.blocks.iter().map(|block| (block.at, IndexMap::default())).collect(),
            length,
            succ: body.blocks.iter().map(|block| (block.at, block.succ.clone())).collect(),
        };
        flow.distances(body);
        flow
    }

    fn leaving(&self, from: i64, to: i64) -> i64 {
        if self.depth[&to] < self.depth[&from] { EXIT } else { 0 }
    }

    fn beyond(&self, block: i64, value: u32) -> i64 {
        self.succ[&block]
            .iter()
            .filter_map(|to| self.from_top.get(to).and_then(|found| found.get(&value)).map(|got| got + self.leaving(block, *to)))
            .min()
            .unwrap_or(FAR)
    }

    /// Distance from every block's entry to each live-in value's next use, to a fixed point.
    fn distances(&mut self, body: &LirBody) {
        for _ in 0..64 {
            let mut changed = false;
            for block in body.blocks.iter().rev() {
                let entering: BTreeSet<u32> = self.live_in[&block.at].iter().copied().chain(block.arrives()).collect();
                for value in entering {
                    let first = self.uses[&block.at].get(&value).and_then(|list| list.first().copied());
                    let here = match first {
                        Some(at) => at as i64,
                        None => self.beyond(block.at, value).saturating_add(self.length[&block.at] as i64).min(FAR),
                    };
                    if self.from_top[&block.at].get(&value) != Some(&here) {
                        self.from_top.get_mut(&block.at).expect("a block").insert(value, here);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Whether `value` is read after position `at` of `block`, or lives out of it. Death is
    /// decided here and never from `next_use`: its `FAR` is also what an unconverged distance reads.
    fn live_after(&self, block: i64, at: usize, value: u32) -> bool {
        self.uses[&block].get(&value).is_some_and(|list| list.iter().any(|next| *next > at)) || self.live_out[&block].contains(&value)
    }

    /// How far after position `at` of `block` the value is read next.
    fn next_use(&self, block: i64, at: usize, value: u32) -> i64 {
        if let Some(list) = self.uses[&block].get(&value) {
            if let Some(next) = list.iter().find(|next| **next > at) {
                return (*next - at) as i64;
            }
        }
        if !self.live_out[&block].contains(&value) {
            return FAR;
        }
        ((self.length[&block] - at) as i64).saturating_add(self.beyond(block, value)).min(FAR)
    }
}

/// The registers a block's values may sit in: all of them, or one class's.
struct Machine<'a> {
    confined: &'a Classes,
    general: BTreeSet<Register>,
    classes: BTreeSet<BTreeSet<Register>>,
    /// The registers with byte halves.
    bytes: BTreeSet<Register>,
}

impl<'a> Machine<'a> {
    fn of(confined: &'a Classes) -> Self {
        let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
        let classes = confined
            .values()
            .map(|class| class.iter().map(|one| _whole(*one)).filter(|one| general.contains(one)).collect::<BTreeSet<Register>>())
            .filter(|class| !class.is_empty() && class.len() < general.len())
            .collect();
        Self { confined, general, classes, bytes: target::BYTE.iter().map(|one| _whole(*one)).collect() }
    }

    /// A value that lives in a general register at all.
    fn registered(&self, value: u32) -> bool {
        self.confined.get(&value).is_none_or(|class| class.iter().any(|one| self.general.contains(&_whole(*one))))
    }

    fn class(&self, value: u32) -> Option<BTreeSet<Register>> {
        self.confined.get(&value).map(|class| class.iter().map(|one| _whole(*one)).filter(|one| self.general.contains(one)).collect())
    }

    /// Whether `held` fits `room` registers, and its values satisfy Hall's condition
    /// over the classes: a byte value always (no other register has a byte half), a
    /// value confined to one register (an address base) only for the `acting` values,
    /// which an instruction needs in it just now (a value waiting in another register
    /// costs a copy to act, not a place).
    fn fits(&self, held: &BTreeSet<u32>, acting: &BTreeSet<u32>, room: usize) -> bool {
        if held.len() > room {
            return false;
        }
        self.classes.iter().all(|class| {
            let counted = |value: &u32| {
                self.class(*value).is_some_and(|mine| mine.is_subset(class) && (acting.contains(value) || (mine.len() > 1 && mine.is_subset(&self.bytes))))
            };
            held.iter().filter(|value| counted(value)).count() <= class.len()
        })
    }
}

/// The registers an instruction states it takes: what it requires, delivers or clobbers.
fn stated(one: &Insn, general: &BTreeSet<Register>) -> BTreeSet<Register> {
    let mut out: BTreeSet<Register> = BTreeSet::new();
    if let Some(what) = &one.what {
        out.extend(target::requirements(what).values().map(|register| _whole(*register)));
    }
    out.extend(one.requires.iter().chain(&one.delivers).map(|(_, register)| _whole(*register)));
    out.extend(one.clobbers.iter().map(|register| _whole(*register)));
    if one.what.as_ref().is_some_and(target::status_through_ax) {
        out.insert(Register::EAX);
    }
    out.retain(|register| general.contains(register));
    out
}

/// Values the spiller leaves alone: x87 values, pinned ones, and the body's inputs.
pub(crate) fn untouchable(body: &LirBody) -> BTreeSet<u32> {
    let mut out = floating(body);
    out.extend(body.pins.keys().copied().chain(body.inputs.iter().copied()));
    out
}

/// Values no general register holds: x87 values and wider ones, and every phi web they join.
pub(crate) fn floating(body: &LirBody) -> BTreeSet<u32> {
    let mut out: BTreeSet<u32> = BTreeSet::new();
    for one in body.insns() {
        if one.what.as_ref().is_some_and(|what| target::_on_the_stack(what) || what.op.is_x87()) {
            out.extend(one.uses.iter().chain(&one.defines).copied());
        }
        // A value no general register holds: a float, whatever reads it.
        for place in one.what.iter().flat_map(|what| what.dests.iter().chain(&what.sources)) {
            for held in crate::model::ir::values(place) {
                if held.width > 4 {
                    out.insert(held.value);
                }
            }
        }
    }
    // A phi joins its result and arguments: the web is as wide as its widest member.
    loop {
        let mut grew = false;
        for block in &body.blocks {
            for phi in &block.phis {
                let web: Vec<u32> = std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)).collect();
                if web.iter().any(|value| out.contains(value)) {
                    for value in web {
                        grew |= out.insert(value);
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    out
}

/// `one` reading `value` from `cell` as its second source (`op reg, [slot]`), a commutative
/// operation turning its operands over first, as Greedy folds a spilled source; None where it cannot.
/// The operand is as wide as the instruction reads it, which may be less than the slot.
pub(crate) fn folded_into(one: &Insn, value: u32, cell: &crate::model::ir::Mem) -> Option<Arc<Insn>> {
    let what = one.what.as_ref()?;
    let values = BTreeSet::from([value]);
    let commutes = what.sources.len() == 2
        && matches!((what.op, what.name.as_deref()), (Operation::Binary, Some("add" | "and" | "or" | "xor")) | (Operation::Multiply, Some("imul")));
    let turned = commutes && matches!(&what.sources[0], Loc::Held(left) if left.value == value) && matches!(&what.sources[1], Loc::Held(right) if right.value != value);
    let mut sources = what.sources.clone();
    if turned {
        sources.swap(0, 1);
    }
    let candidate = if turned {
        let mut made = one.clone();
        made.what = Some(Semantics { sources: sources.clone(), ..what.clone() });
        Arc::new(made)
    } else {
        Arc::new(one.clone())
    };
    if what.op == Operation::Move {
        return None;
    }
    let right = spiller::folded_source_in(&candidate, &values, false)?;
    sources[1] = Loc::Mem(crate::model::ir::Mem { width: right.width, ..cell.clone() });
    let mut made = (*candidate).clone();
    made.what = Some(Semantics { sources, ..what.clone() });
    made.uses = one.uses.iter().copied().filter(|each| *each != value).collect();
    Some(Arc::new(made))
}

/// Whether `one` can read `value` from its slot.
fn folds(one: &Insn, value: u32) -> bool {
    folded_into(one, value, &crate::model::ir::Mem::new(None, 2)).is_some()
}

/// What the simulation of one block decided.
#[derive(Default)]
struct Edits {
    /// Values reloaded before the instruction at each position.
    before: IndexMap<usize, Vec<u32>>,
    /// Values reloaded before the block's terminators.
    at_end: Vec<u32>,
    /// Values an instruction reads from their slot, at each position.
    folded: IndexMap<usize, Vec<u32>>,
    /// Values that leave the register set while live, before the instruction at each position.
    leaves: IndexMap<usize, Vec<u32>>,
    /// Values that leave it before the block's terminators.
    leaves_at_end: Vec<u32>,
    /// Phi results the block does not take into registers.
    leaves_at_top: Vec<u32>,
    w_in: BTreeSet<u32>,
    w_out: BTreeSet<u32>,
}

pub fn spilled(body: &LirBody, frame: &mut Frame, segments: &Segments) -> Result<LirBody, String> {
    Ok(changed(body, frame, segments)?.unwrap_or_else(|| body.clone()))
}

/// `body` spilled, or None where there was nothing to spill and nothing to simplify.
fn changed(original: &LirBody, frame: &mut Frame, segments: &Segments) -> Result<Option<LirBody>, String> {
    let simple = ssarepair::simplified(original);
    let body = simple.as_ref().unwrap_or(original);
    // The loops, found once: depths, headers and each loop's pressure all come from them.
    let loops = crate::analysis::loops::loops(&ranges::_graph(&body.blocks), Some(body.entry));
    let flow = Flow::of(body, &loops);
    let confined = allocate::classes(body, &BTreeSet::new(), segments);
    let machine = Machine::of(&confined);
    let skip = untouchable(body);
    let order = reverse_postorder(body);
    let place: IndexMap<i64, usize> = order.iter().enumerate().map(|(at, block)| (*block, at)).collect();
    // A value a loop's back edge must reload each trip is not worth holding at its header.
    let mut dropped: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    // The most values live at once in each loop, which decides whether what it does not read can wait in registers.
    let room = loop_room(body, &flow, &machine, &skip, &loops);
    let all: BTreeSet<u32> = body.insns().iter().flat_map(|one| one.defines.iter().copied()).collect();
    let remakes = remakable(body, &all);
    let frequency = Frequency::of(body);
    let headers: BTreeSet<i64> = loops.iter().map(|found| found.header).collect();
    let mut result = simulated(body, &flow, &machine, &skip, &remakes, &order, &dropped, &room, &frequency, &headers);
    // A loop header keeps a value its back edge must reload only while those
    // reloads run at most half as often as reloads at its first uses inside one trip.
    for _ in 0..4 {
        let mut more = false;
        for ((from, to), values) in &result.across {
            if place[from] < place[to] {
                continue;
            }
            let Some(within) = loops.iter().find(|one| one.header == *to) else { continue };
            for value in values {
                let keep: f64 = body
                    .blocks
                    .iter()
                    .filter(|block| block.succ.contains(to) && !result.edits[&block.at].w_out.contains(value))
                    .map(|block| frequency.edge(block.at, *to))
                    .sum();
                let drop = first_uses(&flow, &frequency, &within.body, *to, *value);
                // Holding the value costs its register through the trip as well: keep it only when the back edge reloads it rarely.
                if 2.0 * keep >= drop {
                    more |= dropped.entry(*to).or_default().insert(*value);
                }
            }
        }
        if !more {
            break;
        }
        result = simulated(body, &flow, &machine, &skip, &remakes, &order, &dropped, &room, &frequency, &headers);
    }
    if result.stored.is_empty() {
        return Ok(simple);
    }
    // A value is stored once after its definition, or where it leaves the registers, whichever runs less.
    let mut home: IndexMap<u32, i64> = IndexMap::default();
    for (at, made) in &flow.defines {
        for value in made.keys() {
            home.insert(*value, *at);
        }
    }
    let mut leaving: IndexMap<u32, f64> = IndexMap::default();
    for (at, edit) in &result.edits {
        let here = frequency.block(*at);
        for value in edit.leaves.values().flatten().chain(&edit.leaves_at_end).chain(&edit.leaves_at_top) {
            *leaving.entry(*value).or_default() += here;
        }
    }
    for ((from, to), values) in &result.left {
        for value in values {
            *leaving.entry(*value).or_default() += frequency.edge(*from, *to);
        }
    }
    let at_leaves: BTreeSet<u32> = result
        .stored
        .iter()
        .copied()
        .filter(|value| !remakes.contains_key(value))
        .filter(|value| leaving.get(value).copied().unwrap_or(0.0) < home.get(value).map_or(f64::INFINITY, |at| frequency.block(*at)))
        .collect();
    let spilled = written(body, &result.edits, &result.across, &result.left, &result.stored, &at_leaves, &remakes, frame)?;
    let held: IndexMap<i64, BTreeSet<u32>> = result.edits.iter().map(|(at, edit)| (*at, edit.w_in.clone())).collect();
    Ok(Some(ssarepair::repaired(&spilled, &result.stored, &held)))
}

/// The values of `values` that are made again rather than stored and loaded:
/// what reads nothing (a constant, an address), and what a cell nothing
/// changes holds. Each maps to the one instruction that makes it.
fn remakable(body: &LirBody, values: &BTreeSet<u32>) -> IndexMap<u32, Arc<Insn>> {
    let mut defining: IndexMap<u32, Vec<Arc<Insn>>> = IndexMap::default();
    for one in body.insns() {
        for value in &one.defines {
            if values.contains(value) {
                defining.entry(*value).or_default().push(one.clone());
            }
        }
    }
    let mut out: IndexMap<u32, Arc<Insn>> = IndexMap::default();
    let pure: BTreeSet<u32> =
        spiller::_constants(body, values).keys().chain(spiller::_addresses(body, values).keys()).copied().collect();
    for value in &pure {
        let Some(one) = defining.get(value).and_then(|found| spiller::_one_definition(found)) else { continue };
        let alone = one.defines == [*value]
            && one.uses.is_empty()
            && one.requires.is_empty()
            && one.delivers.is_empty()
            && one.clobbers.is_empty()
            && one.group.is_none();
        if alone {
            out.insert(*value, Arc::clone(one));
        }
    }
    for value in spiller::_stable_loads(body, values).keys() {
        if let Some([only]) = defining.get(value).map(Vec::as_slice) {
            out.insert(*value, Arc::clone(only));
        }
    }
    out
}

/// `one` where it is read again: made once more beside `beside`, owning no bytes.
fn remade(one: &Insn, beside: &Insn) -> Arc<Insn> {
    let mut made = one.clone();
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    made.at = beside.at;
    made.covers = Some((at, at));
    made.rematerialized = true;
    Arc::new(made)
}

struct Simulated {
    edits: IndexMap<i64, Edits>,
    across: IndexMap<(i64, i64), Vec<u32>>,
    left: IndexMap<(i64, i64), Vec<u32>>,
    stored: BTreeSet<u32>,
}

fn simulated(
    body: &LirBody,
    flow: &Flow,
    machine: &Machine<'_>,
    skip: &BTreeSet<u32>,
    remakes: &IndexMap<u32, Arc<Insn>>,
    order: &[i64],
    dropped: &IndexMap<i64, BTreeSet<u32>>,
    room: &IndexMap<i64, usize>,
    frequency: &Frequency,
    headers: &BTreeSet<i64>,
) -> Simulated {
    let k = machine.general.len();
    let wanted = |value: u32| machine.registered(value) && !skip.contains(&value);
    let mut edits: IndexMap<i64, Edits> = IndexMap::default();
    let mut stored: BTreeSet<u32> = BTreeSet::new();
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            preds.entry(*to).or_default().push(block.at);
        }
    }
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut made_in: IndexMap<u32, i64> = IndexMap::default();
    for (at, made) in &flow.defines {
        for value in made.keys() {
            made_in.insert(*value, *at);
        }
    }
    for at in order {
        let block = by_at[at];
        // What is stored where it is defined, as often as this block runs, costs a store each time it leaves: evicted last.
        let hot = |value: &u32| made_in.get(value).is_some_and(|home| frequency.block(*home) >= 0.5 * frequency.block(*at));
        let mut done = Edits::default();
        let ends: Vec<&BTreeSet<u32>> = preds.get(at).into_iter().flatten().filter_map(|from| edits.get(from)).map(|one| &one.w_out).collect();
        let mut candidates: Vec<(usize, i64, u32)> = flow.live_in[at]
            .iter()
            .copied()
            .chain(block.arrives())
            .filter(|value| wanted(*value) && !dropped.get(at).is_some_and(|set| set.contains(value)))
            .map(|value| {
                let seen = ends.iter().filter(|set| set.contains(&value)).count();
                let tier = if ends.is_empty() || seen == ends.len() { 0 } else if seen > 0 { 1 } else { 2 };
                let near = flow.uses[at].get(&value).and_then(|list| list.first()).map_or(FAR, |first| *first as i64).min(flow.from_top[at].get(&value).copied().unwrap_or(FAR));
                (tier, near, value)
            })
            .collect();
        candidates.sort_unstable();
        let mut held: BTreeSet<u32> = BTreeSet::new();
        // Entering a loop, what is read only after it does not wait in a register.
        let header = headers.contains(at);
        // A loop that fits its registers with them keeps what it does not read; one that does not, makes room.
        let through = candidates.iter().filter(|(_, near, _)| *near >= EXIT).count();
        let mut spare = match (header, room.get(at)) {
            (true, Some(peak)) if *peak > k => k.saturating_sub(peak - through.min(*peak)),
            _ => usize::MAX,
        };
        for (tier, near, value) in &candidates {
            // What no predecessor ends with is reloaded where it is read, never on the edge.
            let far = header && *near >= EXIT;
            if (!far || spare > 0) && (*tier < 2 || block.arrives().contains(value)) {
                let mut next = held.clone();
                next.insert(*value);
                if machine.fits(&next, &BTreeSet::new(), k) {
                    held = next;
                    if far && spare != usize::MAX {
                        spare -= 1;
                    }
                }
            }
        }
        done.w_in = held.clone();
        done.leaves_at_top = block.arrives().into_iter().filter(|value| wanted(*value) && !held.contains(value)).collect();
        for (position, one) in block.insns.iter().enumerate() {
            let used: BTreeSet<u32> = one.uses.iter().copied().filter(|value| wanted(*value)).collect();
            let made: BTreeSet<u32> = one.defines.iter().copied().filter(|value| wanted(*value)).collect();
            let acting: BTreeSet<u32> = used.union(&made).copied().collect();
            let mut leaving: Vec<u32> = Vec::new();
            let evict = |held: &mut BTreeSet<u32>, leaving: &mut Vec<u32>, keep: &BTreeSet<u32>, room: usize| {
                while !machine.fits(held, &acting, room) {
                    let victim = held
                        .iter()
                        .filter(|value| !keep.contains(*value))
                        .max_by_key(|value| (!hot(value), flow.next_use(*at, position, **value), **value))
                        .copied();
                    let Some(victim) = victim else { break };
                    held.remove(&victim);
                    if flow.live_after(*at, position, victim) {
                        leaving.push(victim);
                    }
                }
            };
            let mut used = used;
            for value in used.clone() {
                if !held.contains(&value) {
                    stored.insert(value);
                    if !remakes.contains_key(&value) && done.folded.get(&position).is_none_or(Vec::is_empty) && folds(one, value) {
                        used.remove(&value);
                        done.folded.entry(position).or_default().push(value);
                    } else {
                        done.before.entry(position).or_default().push(value);
                        held.insert(value);
                    }
                }
            }
            evict(&mut held, &mut leaving, &used, k);
            // What the instruction states it takes leaves less for what lives through it.
            let taken = stated(one, &machine.general);
            // A register a value of this instruction sits in is that value's, not one more.
            let mut covered: BTreeSet<Register> = one.requires.iter().chain(&one.delivers).map(|(_, register)| _whole(*register)).collect();
            if let Some(what) = &one.what {
                for (place, register) in target::requirements(what) {
                    let side = if place.side == "dest" { &what.dests } else { &what.sources };
                    if matches!(side.get(place.index), Some(Loc::Held(_))) {
                        covered.insert(_whole(register));
                    }
                }
            }
            let outside = taken.iter().filter(|register| !covered.contains(*register)).count();
            if outside > 0 {
                let through: BTreeSet<u32> = held.iter().copied().filter(|value| !used.contains(value) && !made.contains(value)).collect();
                let mut across = held.clone();
                let keep: BTreeSet<u32> = used.union(&made).copied().collect();
                while across.len() > k - outside.min(k) {
                    let victim = through.iter().filter(|value| across.contains(*value) && !keep.contains(*value)).max_by_key(|value| (!hot(value), flow.next_use(*at, position, **value), **value)).copied();
                    let Some(victim) = victim else { break };
                    across.remove(&victim);
                    held.remove(&victim);
                    leaving.push(victim);
                }
            }
            // The first source of a tied instruction gives its register to the result.
            let tied = one.what.as_ref().is_some_and(twoaddr::ties);
            let first = if tied { one.what.as_ref().and_then(|what| what.sources.first()).and_then(|source| if let Loc::Held(first) = source { Some(first.value) } else { None }) } else { None };
            let dying = |value: &u32| !flow.live_after(*at, position, *value);
            let gone: Vec<u32> = used.iter().copied().filter(|value| dying(value) && (!tied || Some(*value) == first)).collect();
            for value in gone {
                held.remove(&value);
            }
            let keep: BTreeSet<u32> = used.iter().copied().filter(|value| held.contains(value)).collect();
            for value in &made {
                if !dying(value) || flow.live_out[at].contains(value) {
                    held.insert(*value);
                }
            }
            evict(&mut held, &mut leaving, &made.union(&keep).copied().collect(), k);
            held.retain(|value| flow.live_after(*at, position, *value));
            // A value made here leaves after the instruction, not before it.
            let (after, before): (Vec<u32>, Vec<u32>) = leaving.into_iter().partition(|value| made.contains(value));
            if !before.is_empty() {
                done.leaves.entry(position).or_default().extend(before);
            }
            if !after.is_empty() {
                done.leaves.entry(position + 1).or_default().extend(after);
            }
        }
        // The arguments of its successors' phis are read at the end.
        let handed: BTreeSet<u32> = block
            .succ
            .iter()
            .filter_map(|to| by_at.get(to))
            .flat_map(|next| next.phis.iter())
            .flat_map(|phi| phi.incoming.iter().filter(|(from, _)| from == at).map(|(_, value)| *value))
            .filter(|value| wanted(*value))
            .collect();
        for value in &handed {
            if !held.contains(value) {
                done.at_end.push(*value);
                stored.insert(*value);
                held.insert(*value);
            }
        }
        {
            let end = block.insns.len();
            while !machine.fits(&held, &handed, k) {
                let victim = held.iter().filter(|value| !handed.contains(*value)).max_by_key(|value| (!hot(value), flow.next_use(*at, end, **value), **value)).copied();
                let Some(victim) = victim else { break };
                held.remove(&victim);
                if flow.live_out[at].contains(&victim) {
                    done.leaves_at_end.push(victim);
                }
            }
        }
        held.retain(|value| flow.live_out[at].contains(value) || handed.contains(value));
        done.w_out = held;
        edits.insert(*at, done);
    }
    // Where a successor expects a register the predecessor does not end with, the edge reloads.
    let mut across: IndexMap<(i64, i64), Vec<u32>> = IndexMap::default();
    // And where it ends with one the successor does not take, the value leaves on the edge.
    let mut left: IndexMap<(i64, i64), Vec<u32>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            let (Some(next), Some(here)) = (edits.get(to), edits.get(&block.at)) else { continue };
            let arrives: BTreeSet<u32> = by_at[to].arrives().into_iter().collect();
            for value in &next.w_in {
                if !arrives.contains(value) && flow.live_in[to].contains(value) && !here.w_out.contains(value) {
                    across.entry((block.at, *to)).or_default().push(*value);
                    stored.insert(*value);
                }
            }
            for value in &here.w_out {
                if !arrives.contains(value) && flow.live_in[to].contains(value) && !next.w_in.contains(value) {
                    left.entry((block.at, *to)).or_default().push(*value);
                }
            }
        }
    }
    Simulated { edits, across, left, stored }
}

/// What reloading `value` at its first register uses costs, within one trip of the loop
/// `within` entered at `header`: the frequency of each block on that frontier.
fn first_uses(flow: &Flow, frequency: &Frequency, within: &BTreeSet<i64>, header: i64, value: u32) -> f64 {
    let mut cost = 0.0;
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    let mut todo: Vec<i64> = vec![header];
    while let Some(at) = todo.pop() {
        if !seen.insert(at) {
            continue;
        }
        let length = flow.length[&at];
        if flow.uses[&at].get(&value).is_some_and(|list| list.iter().any(|position| *position < length)) {
            cost += frequency.block(at);
            continue;
        }
        for to in &flow.succ[&at] {
            if *to != header && within.contains(to) && flow.live_in[to].contains(&value) {
                todo.push(*to);
            }
        }
    }
    cost
}

/// For each loop header, the most values live at once inside the loop.
fn loop_room(body: &LirBody, flow: &Flow, machine: &Machine<'_>, skip: &BTreeSet<u32>, loops: &[crate::analysis::loops::Loop]) -> IndexMap<i64, usize> {
    let wanted = |value: &u32| machine.registered(*value) && !skip.contains(value);
    let mut most: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        let mut live: BTreeSet<u32> = flow.live_out[&block.at].iter().copied().filter(|value| wanted(value)).collect();
        let mut peak = live.len();
        for one in block.insns.iter().rev() {
            peak = peak.max(live.len());
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied().filter(|value| wanted(value)));
            peak = peak.max(live.len());
        }
        most.insert(block.at, peak);
    }
    loops
        .iter()
        .map(|found| {
            let busiest = found.body.iter().filter_map(|at| most.get(at)).copied().max().unwrap_or(0);
            llrm_support::debug!("ssaspill", "{}: loop at {:#x} peaks at {busiest}", body.name, found.header);
            (found.header, busiest)
        })
        .collect()
}

fn reverse_postorder(body: &LirBody) -> Vec<i64> {
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    let mut post: Vec<i64> = Vec::new();
    let mut todo: Vec<(i64, usize)> = vec![(body.entry, 0)];
    seen.insert(body.entry);
    while let Some((at, next)) = todo.pop() {
        match by_at[&at].succ.get(next) {
            Some(to) => {
                todo.push((at, next + 1));
                if by_at.contains_key(to) && seen.insert(*to) {
                    todo.push((*to, 0));
                }
            }
            None => post.push(at),
        }
    }
    post.reverse();
    let mut rest: Vec<i64> = body.blocks.iter().map(|block| block.at).filter(|at| !seen.contains(at)).collect();
    post.append(&mut rest);
    post
}

/// What one edge carries: stores of values leaving the register set on it,
/// then reloads of values the successor expects in one.
#[derive(Default)]
struct Crossing {
    stores: Vec<u32>,
    reloads: Vec<u32>,
}

/// `body` with the edits made: reloads; each value reloaded anywhere stored
/// once after its definition, or, for the values in `at_leaves`, wherever it
/// leaves the register set while live; and the edge code.
#[allow(clippy::too_many_arguments)]
fn written(
    body: &LirBody,
    edits: &IndexMap<i64, Edits>,
    across: &IndexMap<(i64, i64), Vec<u32>>,
    left: &IndexMap<(i64, i64), Vec<u32>>,
    stored: &BTreeSet<u32>,
    at_leaves: &BTreeSet<u32>,
    remakes: &IndexMap<u32, Arc<Insn>>,
    frame: &mut Frame,
) -> Result<LirBody, String> {
    let widths = widths_through_phis(body, stored);
    let mut cells: IndexMap<u32, crate::model::ir::Mem> = IndexMap::default();
    for value in stored.iter().filter(|value| !remakes.contains_key(*value)) {
        let width = widths.get(value).copied().unwrap_or(2);
        // Its own keys: a later phase that spills a value with the same number (after phi elimination and coalescing renamed things) must not be handed this slot.
        cells.insert(*value, frame.cell(("ssaspill", i64::from(*value)), width).map_err(|error| error.to_string())?);
    }
    let in_memory = |value: &u32| stored.contains(value) && !remakes.contains_key(value);
    let reload = |beside: &Insn, value: u32| match remakes.get(&value) {
        Some(one) => remade(one, beside),
        None => spiller::_reload(beside, value, &cells[&value]),
    };
    let store = |beside: &Insn, value: u32| spiller::_store(beside, value, &cells[&value]);
    // The code on each edge.
    let mut crossing: IndexMap<(i64, i64), Crossing> = IndexMap::default();
    for (edge, values) in across {
        crossing.entry(*edge).or_default().reloads.extend(values.iter().copied());
    }
    for (edge, values) in left {
        crossing.entry(*edge).or_default().stores.extend(values.iter().copied().filter(|value| in_memory(value) && at_leaves.contains(value)));
    }
    let mut preds: IndexMap<i64, usize> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            *preds.entry(*to).or_default() += 1;
        }
    }
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    // What code put in a block sits beside: its last instruction, or the block's own address when it has none.
    let anchor = |block: &LirBlock| -> Arc<Insn> {
        block.insns.last().cloned().unwrap_or_else(|| Arc::new(Insn::new(block.at, Some((block.at, block.at)), None, Vec::new(), Vec::new())))
    };
    let mut odds = body.odds.clone();
    let mut next_at = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let mut bridges: Vec<LirBlock> = Vec::new();
    let mut retarget: IndexMap<(i64, i64), i64> = IndexMap::default();
    // Edge code at the end of a predecessor with one successor, at the start of a successor with one predecessor, else in a block on the edge.
    let mut at_end: IndexMap<i64, Vec<(bool, u32)>> = IndexMap::default();
    let mut at_top: IndexMap<i64, Vec<(bool, u32)>> = IndexMap::default();
    for ((from, to), code) in &crossing {
        if code.stores.is_empty() && code.reloads.is_empty() {
            continue;
        }
        let source = by_at[from];
        let listed: Vec<(bool, u32)> = code.stores.iter().map(|value| (true, *value)).chain(code.reloads.iter().map(|value| (false, *value))).collect();
        if source.succ.len() == 1 {
            at_end.entry(*from).or_default().extend(listed);
        } else if preds.get(to) == Some(&1) {
            at_top.entry(*to).or_default().extend(listed);
        } else {
            let at = next_at;
            next_at += 1;
            let beside = anchor(source);
            let beside = &*beside;
            let mut insns: Vec<Arc<Insn>> = listed.iter().map(|(stores, value)| if *stores { store(beside, *value) } else { reload(beside, *value) }).collect();
            let mut jump = Insn::new(
                beside.at,
                Some((beside.at, beside.at)),
                Some(Semantics { name: Some("jmp".to_owned()), target: Some(*to), ..Semantics::new(Operation::Jump) }),
                Vec::new(),
                Vec::new(),
            );
            jump.call = beside.call.clone();
            insns.push(Arc::new(jump));
            bridges.push(LirBlock { succ: vec![*to], ..LirBlock::new(at, insns) });
            odds.rerouted(*from, &source.succ, *to, &[(at, 1.0)]);
            retarget.insert((*from, *to), at);
        }
    }
    let mut blocks: Vec<LirBlock> = Vec::new();
    for block in &body.blocks {
        let edit = &edits[&block.at];
        let tail = splitkit::_tail(block);
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        // A phi's result is written when control enters; its store comes first.
        {
            let first = block.insns.first().cloned().unwrap_or_else(|| anchor(block));
            for phi in &block.phis {
                let leaves_here = edit.leaves_at_top.contains(&phi.result);
                if in_memory(&phi.result) && (!at_leaves.contains(&phi.result) || leaves_here) {
                    insns.push(store(&first, phi.result));
                }
            }
            for (stores, value) in at_top.get(&block.at).into_iter().flatten() {
                insns.push(if *stores { store(&first, *value) } else { reload(&first, *value) });
            }
        }
        let leave = |position: usize| edit.leaves.get(&position).into_iter().flatten().copied().filter(|value| in_memory(value) && at_leaves.contains(value));
        let ending = |beside: &Insn, insns: &mut Vec<Arc<Insn>>| {
            for value in edit.leaves_at_end.iter().copied().filter(|value| in_memory(value) && at_leaves.contains(value)) {
                insns.push(store(beside, value));
            }
            for (stores, value) in at_end.get(&block.at).into_iter().flatten() {
                insns.push(if *stores { store(beside, *value) } else { reload(beside, *value) });
            }
            for value in &edit.at_end {
                insns.push(reload(beside, *value));
            }
        };
        for (position, one) in block.insns.iter().enumerate() {
            for value in leave(position) {
                insns.push(store(one, value));
            }
            if position == tail {
                ending(one, &mut insns);
            }
            for value in edit.before.get(&position).into_iter().flatten() {
                insns.push(reload(one, *value));
            }
            let mut one = Arc::clone(one);
            for value in edit.folded.get(&position).into_iter().flatten() {
                one = folded_into(&one, *value, &cells[value]).expect("a fold the simulation chose");
            }
            if one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch)) {
                if let Some(what) = &one.what {
                    if let Some(to) = what.target.and_then(|to| retarget.get(&(block.at, to))) {
                        let mut made = (*one).clone();
                        made.what = Some(Semantics { target: Some(*to), ..what.clone() });
                        one = Arc::new(made);
                    }
                }
            }
            insns.push(Arc::clone(&one));
            for value in &one.defines {
                if in_memory(value) && !at_leaves.contains(value) && !block.phis.iter().any(|phi| phi.result == *value) {
                    insns.push(store(&one, *value));
                }
            }
        }
        if tail == block.insns.len() {
            let last = anchor(block);
            for value in leave(block.insns.len()) {
                insns.push(store(&last, value));
            }
            ending(&last, &mut insns);
        }
        let succ = block.succ.iter().map(|to| retarget.get(&(block.at, *to)).copied().unwrap_or(*to)).collect();
        blocks.push(LirBlock { succ, ..block.with_insns(insns) });
    }
    for ((from, to), at) in &retarget {
        if let Some(target) = blocks.iter_mut().find(|block| block.at == *to) {
            for phi in &mut target.phis {
                for (source, _) in &mut phi.incoming {
                    if source == from {
                        *source = *at;
                    }
                }
            }
        }
    }
    blocks.extend(bridges);
    let mut out = body.with_blocks(blocks);
    out.odds = odds;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two values that may only sit in BX (each is the base of an address
    /// somewhere) were counted as a byte pair although only one acts: deedlines
    /// COPPER evicted down to three held values in a six-register machine, and
    /// its palette loop reloaded two addresses every iteration.
    #[test]
    fn test_two_base_only_values_fit_while_one_acts() {
        let bx = BTreeSet::from([Register::EBX]);
        let confined: Classes = [(1, bx.clone()), (2, bx), (3, BTreeSet::from([Register::ESI, Register::EDI]))].into_iter().collect();
        let machine = Machine::of(&confined);
        let held: BTreeSet<u32> = [1, 2, 3].into_iter().collect();
        assert!(machine.fits(&held, &BTreeSet::from([1]), 6), "one acting BX value leaves the other waiting");
        assert!(!machine.fits(&held, &BTreeSet::from([1, 2]), 6), "premise: two acting BX values cannot both sit in BX");
    }
}
