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
use crate::backend::allocate::{self, _whole};
use crate::backend::classes::RegisterClasses;
use crate::backend::regclass::{self, Classes};
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
    static MEMORY_PHIS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static SHARED_SLOTS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// `run` with no phi web sharing a slot on this thread: every argument is stored on its edge.
#[cfg(test)]
pub fn without_shared_slots<T>(run: impl FnOnce() -> T) -> T {
    let before = SHARED_SLOTS.with(|one| one.replace(false));
    let done = run();
    SHARED_SLOTS.with(|one| one.set(before));
    done
}

/// `run` with no phi taken into memory on this thread: every phi the registers cannot hold arrives in one and is stored.
#[cfg(test)]
pub fn without_memory_phis<T>(run: impl FnOnce() -> T) -> T {
    let before = MEMORY_PHIS.with(|one| one.replace(false));
    let done = run();
    MEMORY_PHIS.with(|one| one.set(before));
    done
}

/// What one run of the spiller was asked and settled, for whoever built its phase: whether a loop's entry may load what
/// the loop reads, and whether it changed the body and admitted a loop on a tie in the bytes it counts (the trips
/// decided, and the encoded code may not agree).
#[derive(Debug)]
pub struct Run {
    pub admission: bool,
    changed: std::cell::Cell<bool>,
    ties: std::cell::Cell<bool>,
}

impl Run {
    pub fn new(admission: bool) -> Rc<Self> {
        Rc::new(Self { admission, changed: std::cell::Cell::new(false), ties: std::cell::Cell::new(false) })
    }

    pub fn changed(&self) -> bool {
        self.changed.get()
    }

    pub fn ties(&self) -> bool {
        self.ties.get()
    }
}

impl Default for Run {
    fn default() -> Self {
        Self { admission: true, changed: std::cell::Cell::new(false), ties: std::cell::Cell::new(false) }
    }
}

pub struct SsaSpill {
    pub frame: Rc<RefCell<Frame>>,
    pub segments: Segments,
    pub classes: Rc<RegisterClasses>,
    pub prices: Prices,
    pub run: Rc<Run>,
}

/// How much a block or an edge counts: by how often it runs, or once where the price is bytes.
struct Weights<'a> {
    frequency: &'a Frequency,
    by_frequency: bool,
}

impl Weights<'_> {
    fn block(&self, at: i64) -> f64 {
        if self.by_frequency { self.frequency.block(at) } else { 1.0 }
    }

    fn edge(&self, from: i64, to: i64) -> f64 {
        if self.by_frequency { self.frequency.edge(from, to) } else { 1.0 }
    }
}

/// What a load from memory costs at the level being compiled, and whether it costs once per run or once per trip.
#[derive(Clone, Copy, Debug)]
pub struct Prices {
    pub load: f64,
    /// A jump: a branch's price on the machine, as the bridge block's last instruction takes it.
    pub jump: f64,
    /// By block frequency (clocks); not where the price is code bytes, which a loop's trips do not multiply.
    pub by_frequency: bool,
}

impl Prices {
    /// One clock a load and a jump, by block frequency: for a test that prices nothing in particular.
    pub fn clocks() -> Self {
        Self { load: 1.0, jump: 1.0, by_frequency: true }
    }

    /// The level `profile` compiles for: its machine's byte costs at -Os, its clocks otherwise.
    pub fn of(profile: &crate::backend::cpu::Profile) -> Self {
        let machine = profile.target();
        let costs = if profile.size { machine.size_costs() } else { machine.costs() };
        Self { load: costs.load as f64, jump: costs.branch as f64, by_frequency: !profile.size }
    }
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
        let made = changed(&body, &mut self.frame.borrow_mut(), &self.segments, &self.classes, self.prices, &self.run)?;
        if made.is_some() {
            self.run.changed.set(true);
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

/// A register file the spiller keeps within its size: the general registers, or the segment registers a selector value sits in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum File {
    General,
    Selector,
}

/// The registers one file's values may sit in: all of them, or one class's.
struct Machine<'a> {
    confined: &'a Classes,
    /// What the target requires of each instruction's operands.
    classes: &'a RegisterClasses,
    file: File,
    /// The data segment register, which an instruction that needs the data group takes from the selectors.
    data: Register,
    /// The file's registers (whole, as the allocator names them).
    general: BTreeSet<Register>,
    pools: BTreeSet<BTreeSet<Register>>,
    /// The registers with byte halves.
    bytes: BTreeSet<Register>,
}

impl<'a> Machine<'a> {
    fn of(confined: &'a Classes, file: File, segments: &Segments, classes: &'a RegisterClasses) -> Self {
        let general: BTreeSet<Register> = match file {
            File::General => classes.available.iter().map(|one| _whole(*one)).collect(),
            File::Selector => segments.selectors.iter().copied().collect(),
        };
        let pools: BTreeSet<BTreeSet<Register>> = confined
            .values()
            .map(|class| class.iter().map(|one| _whole(*one)).filter(|one| general.contains(one)).collect::<BTreeSet<Register>>())
            .filter(|class| !class.is_empty() && class.len() < general.len())
            .collect();
        Self { confined, file, classes, data: segments.data, general, pools, bytes: target::BYTE.iter().map(|one| _whole(*one)).collect() }
    }

    /// A value that lives in a register of this file at all: a selector only where the program names it so.
    fn registered(&self, value: u32) -> bool {
        match (self.file, self.confined.get(&value)) {
            (File::General, class) => class.is_none_or(|class| class.iter().any(|one| self.general.contains(&_whole(*one)))),
            (File::Selector, class) => class.is_some_and(|class| class.iter().any(|one| self.general.contains(&_whole(*one)))),
        }
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
        self.pools.iter().all(|class| {
            let counted = |value: &u32| {
                self.class(*value).is_some_and(|mine| mine.is_subset(class) && (acting.contains(value) || (mine.len() > 1 && mine.is_subset(&self.bytes))))
            };
            held.iter().filter(|value| counted(value)).count() <= class.len()
        })
    }
}

/// The registers an instruction states it takes: what it requires, delivers or clobbers.
fn stated(one: &Insn, general: &BTreeSet<Register>, classes: &RegisterClasses) -> BTreeSet<Register> {
    let mut out: BTreeSet<Register> = BTreeSet::new();
    if let Some(what) = &one.what {
        out.extend(classes.requirements(what).values().map(|register| _whole(*register)));
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

pub fn spilled(body: &LirBody, frame: &mut Frame, segments: &Segments, classes: &RegisterClasses, prices: Prices) -> Result<LirBody, String> {
    Ok(changed(body, frame, segments, classes, prices, &Run::default())?.unwrap_or_else(|| body.clone()))
}

/// `body` spilled, or None where there was nothing to spill and nothing to simplify.
fn changed(original: &LirBody, frame: &mut Frame, segments: &Segments, classes: &RegisterClasses, prices: Prices, run: &Run) -> Result<Option<LirBody>, String> {
    let simple = ssarepair::simplified(original);
    let body = simple.as_ref().unwrap_or(original);
    // The loops, found once: depths, headers and each loop's pressure all come from them.
    let loops = crate::analysis::loops::loops(&body.blocks, Some(body.entry));
    let flow = Flow::of(body, &loops);
    let confined = regclass::classes(body, &BTreeSet::new(), segments, classes);
    let skip = untouchable(body);
    let order = reverse_postorder(body);
    let place: IndexMap<i64, usize> = order.iter().enumerate().map(|(at, block)| (*block, at)).collect();
    let all: BTreeSet<u32> = body.insns().iter().flat_map(|one| one.defines.iter().copied()).collect();
    let remakes = remakable(body, &all);
    let frequency = Frequency::of(body);
    let headers: BTreeSet<i64> = loops.iter().map(|found| found.header).collect();
    let mut result = Simulated::default();
    let mut selectors: BTreeSet<u32> = BTreeSet::new();
    for file in [File::General, File::Selector] {
        let machine = Machine::of(&confined, file, segments, classes);
        if machine.general.is_empty() {
            continue;
        }
        // A selector is read from its cell or loaded into a register: never a constant it is made from.
        let kept: IndexMap<u32, Arc<Insn>> = match file {
            File::General => remakes.clone(),
            File::Selector => remakes.iter().filter(|(value, one)| remade_cell(one, **value).is_some()).map(|(value, one)| (*value, Arc::clone(one))).collect(),
        };
        if file == File::Selector {
            selectors.extend(kept.keys().copied().filter(|value| machine.registered(*value)));
        }
        let bridged: BTreeSet<(i64, i64)> = result.across.keys().copied().collect();
        let one = simulated_in(body, &flow, &machine, &skip, &kept, &order, &place, &frequency, &headers, &loops, prices, &bridged, run);
        result.merge(one);
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
        .filter(|value| !remakes.contains_key(value) && !result.shared.contains_key(value))
        .filter(|value| leaving.get(value).copied().unwrap_or(0.0) < home.get(value).map_or(f64::INFINITY, |at| frequency.block(*at)))
        .collect();
    let spilled = written(body, &result.edits, &result.across, &result.left, &result.stored, &result.memory, &result.shared, &result.moves, &at_leaves, &remakes, frame)?;
    let held: IndexMap<i64, BTreeSet<u32>> = result.edits.iter().map(|(at, edit)| (*at, edit.w_in.clone())).collect();
    Ok(Some(without_dead_remakes(ssarepair::repaired(&spilled, &result.stored, &held), &selectors)))
}

/// `body` without the definitions of remade selectors that nothing reads any more: each read made its own.
fn without_dead_remakes(body: LirBody, remade: &BTreeSet<u32>) -> LirBody {
    let mut gone: BTreeSet<*const Insn> = BTreeSet::new();
    let mut targets = remade.clone();
    loop {
        let read: BTreeSet<u32> = body
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .insns
                    .iter()
                    .filter(|one| !gone.contains(&Arc::as_ptr(one)))
                    .flat_map(|one| one.uses.iter().copied())
                    .chain(block.phis.iter().flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)))
            })
            .collect();
        let mut more = false;
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            let [value] = one.defines.as_slice() else { continue };
            let plain = one.what.as_ref().is_some_and(|what| what.op == Operation::Move && what.name.as_deref() == Some("mov")) && !one.rematerialized && one.uses.len() <= 1;
            if plain && targets.contains(value) && !read.contains(value) && gone.insert(Arc::as_ptr(one)) {
                // What it copied is read by it no more.
                targets.extend(one.uses.iter().copied());
                more = true;
            }
        }
        if !more {
            break;
        }
    }
    if gone.is_empty() {
        return body;
    }
    let blocks = body.blocks.iter().map(|block| block.with_insns(block.insns.iter().filter(|one| !gone.contains(&Arc::as_ptr(one))).cloned().collect())).collect();
    body.with_blocks(blocks)
}

/// The simulation of one register file's values over every block; for the selectors, the cheaper at this level's price of holding at a loop's entry what no predecessor ends with, or not.
#[allow(clippy::too_many_arguments)]
fn simulated_in(
    body: &LirBody,
    flow: &Flow,
    machine: &Machine<'_>,
    skip: &BTreeSet<u32>,
    remakes: &IndexMap<u32, Arc<Insn>>,
    order: &[i64],
    place: &IndexMap<i64, usize>,
    frequency: &Frequency,
    headers: &BTreeSet<i64>,
    loops: &[crate::analysis::loops::Loop],
    prices: Prices,
    bridged: &BTreeSet<(i64, i64)>,
    run: &Run,
) -> Simulated {
    let weights = Weights { frequency, by_frequency: prices.by_frequency };
    let attempt = |admit: &BTreeSet<i64>| simulated_with(body, flow, machine, skip, remakes, order, place, frequency, headers, loops, prices, &weights, bridged, admit);
    let nothing: BTreeSet<i64> = BTreeSet::new();
    let first = attempt(&nothing);
    if machine.file != File::Selector || !run.admission {
        return first;
    }
    // Whether a loop's entry loads what the loop reads: none, all, or each loop by itself; priced by the level's
    // measure (bytes at -Os), and by the trips where that measure ties.
    let none: BTreeSet<(i64, i64)> = body.critical_edges().into_iter().collect();
    let by_trips = Weights { frequency, by_frequency: true };
    let tied = std::cell::Cell::new(false);
    let cheaper = |tried: &Simulated, kept: &Simulated| {
        let (now, then) = (traffic(body.bits, tried, &weights, prices, &none, remakes), traffic(body.bits, kept, &weights, prices, &none, remakes));
        let by_clocks = now == then && traffic(body.bits, tried, &by_trips, prices, &none, remakes) < traffic(body.bits, kept, &by_trips, prices, &none, remakes);
        tied.set(tied.get() || (by_clocks && !prices.by_frequency));
        now < then || by_clocks
    };
    let mut kept = first;
    let everywhere = attempt(headers);
    if cheaper(&everywhere, &kept) {
        kept = everywhere;
    }
    if headers.len() > 1 {
        let mut admitted: BTreeSet<i64> = BTreeSet::new();
        let mut alone = attempt(&admitted);
        for header in headers {
            let mut trial = admitted.clone();
            trial.insert(*header);
            let tried = attempt(&trial);
            if cheaper(&tried, &alone) {
                admitted = trial;
                alone = tried;
            }
        }
        if cheaper(&alone, &kept) {
            kept = alone;
        }
    }
    if tied.get() {
        run.ties.set(true);
    }
    kept
}

/// `simulated_in` with the entry load of a loop settled: the values a loop header does not keep, to a fixed point.
#[allow(clippy::too_many_arguments)]
fn simulated_with(
    body: &LirBody,
    flow: &Flow,
    machine: &Machine<'_>,
    skip: &BTreeSet<u32>,
    remakes: &IndexMap<u32, Arc<Insn>>,
    order: &[i64],
    place: &IndexMap<i64, usize>,
    frequency: &Frequency,
    headers: &BTreeSet<i64>,
    loops: &[crate::analysis::loops::Loop],
    prices: Prices,
    weights: &Weights<'_>,
    bridged: &BTreeSet<(i64, i64)>,
    admit: &BTreeSet<i64>,
) -> Simulated {
    // A value a loop's back edge must reload each trip is not worth holding at its header.
    let mut dropped: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    // The most values live at once in each loop, which decides whether what it does not read can wait in registers.
    let room = loop_room(body, flow, machine, skip, loops);
    // The phis a block cannot take into registers live in memory: found by a first pass, then held out of the registers.
    // Each round leaves one register to the moves of a block with a memory phi, which can push another phi out.
    let mut memory: BTreeSet<u32> = BTreeSet::new();
    for _ in 0..if MEMORY_PHIS.with(std::cell::Cell::get) { 4 } else { 0 } {
        let probe = simulated(body, flow, machine, skip, remakes, order, &dropped, &room, frequency, headers, admit, &memory);
        let more = memory_phis(body, flow, remakes, &probe, machine, skip, weights);
        if more.is_subset(&memory) {
            break;
        }
        memory.extend(more);
    }
    let mut result = simulated(body, flow, machine, skip, remakes, order, &dropped, &room, frequency, headers, admit, &memory);
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
                    .map(|block| weights.edge(block.at, *to))
                    .sum();
                let drop = first_uses(flow, weights, &within.body, *to, *value);
                if std::env::var_os("DBG_KEEP").is_some() { eprintln!("KEEP {} header {:#x} v{} keep {:.2} drop {:.2}", body.name, to, value, keep, drop); }
                // Holding the value costs its register through the trip as well: keep it only when the back edge reloads it rarely.
                if 2.0 * keep >= drop {
                    more |= dropped.entry(*to).or_default().insert(*value);
                }
            }
        }
        if !more {
            break;
        }
        result = simulated(body, flow, machine, skip, remakes, order, &dropped, &room, frequency, headers, admit, &memory);
    }
    // Dropping what a loop evicts leaves the loop's registers short of use: a value is let back in
    // where the loop then moves less to and from memory.
    if machine.file == File::Selector && std::env::var_os("NO_LET").is_none() {
        // The jump a bridge on a loop's back edge takes runs every trip, unless another file's values already bring the bridge;
        // and what a loop that fits its registers does not hold, it need not bridge to hold.
        let critical: BTreeSet<(i64, i64)> = body.critical_edges().into_iter().filter(|edge| place[&edge.0] >= place[&edge.1] && !bridged.contains(edge) && room.get(&edge.1).is_some_and(|peak| *peak > machine.general.len())).collect();
        let mut best = traffic(body.bits, &result, weights, prices, &critical, remakes);
        let letting: Vec<(i64, u32)> = dropped.iter().flat_map(|(header, values)| values.iter().map(move |value| (*header, *value))).collect();
        for (header, value) in letting {
            let mut trial = dropped.clone();
            trial.get_mut(&header).map(|values| values.remove(&value));
            let tried = simulated(body, flow, machine, skip, remakes, order, &trial, &room, frequency, headers, admit, &memory);
            let moved = traffic(body.bits, &tried, weights, prices, &critical, remakes);
            if moved < best {
                best = moved;
                result = tried;
                dropped = trial;
            }
        }
    }
    result
}

/// What a simulation moves to and from memory, by block frequency: reloads, operands read in place, and edge reloads.
fn traffic(bits: u32, result: &Simulated, weights: &Weights<'_>, prices: Prices, critical: &BTreeSet<(i64, i64)>, remakes: &IndexMap<u32, Arc<Insn>>) -> f64 {
    let price = |values: &mut dyn Iterator<Item = &u32>| -> f64 { values.map(|value| reload_price(bits, *value, remakes, prices)).sum() };
    let blocks: f64 = result
        .edits
        .iter()
        .map(|(at, edit)| {
            let reads = price(&mut edit.before.values().flatten().chain(&edit.at_end).chain(edit.folded.values().flatten()));
            weights.block(*at) * reads
        })
        .sum();
    // A bridged edge takes its jump as well.
    let edges: f64 = result.across.iter().map(|((from, to), values)| weights.edge(*from, *to) * (price(&mut values.iter()) + if critical.contains(&(*from, *to)) { prices.jump } else { 0.0 })).sum();
    // Each value kept in a slot is stored once where it is made.
    let stores: f64 = result.stored.len() as f64 * prices.load;
    // A value handed to a phi's slot on an edge is a load and a store there.
    let moves: f64 = result.moves.iter().map(|((from, to), pairs)| weights.edge(*from, *to) * 2.0 * prices.load * pairs.len() as f64).sum();
    blocks + edges + stores + moves
}

/// What reading `value` from memory costs: a load by `prices`; where the code is what counts (-Os), the bytes the
/// load that makes it again encodes to, in a selector register and with its address registers as they will be.
fn reload_price(bits: u32, value: u32, remakes: &IndexMap<u32, Arc<Insn>>, prices: Prices) -> f64 {
    if prices.by_frequency {
        return prices.load;
    }
    let Some(what) = remakes.get(&value).and_then(|one| one.what.as_ref()) else { return prices.load };
    let mut held: crate::backend::select::HeldMap = IndexMap::default();
    held.insert(value, Register::ES);
    for place in what.sources.iter().chain(&what.dests) {
        for used in crate::model::ir::values(place) {
            held.entry(used.value).or_insert(Register::BX);
        }
    }
    crate::backend::select::priced_in(bits, what, 0, None, false, false, Some(&held)).map_or(prices.load, |code| code.code.len() as f64)
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
    let copies = spiller::_copies(body, values);
    for value in spiller::_stable_loads_through(body, values, &copies).keys() {
        if let Some([only]) = defining.get(value).map(Vec::as_slice) {
            out.insert(*value, Arc::clone(only));
        }
    }
    // A plain copy of a value made again is made again the same way: its own, as wide as it is.
    for (value, source) in copies {
        let Some(made) = out.get(&source) else { continue };
        let width = made.what.as_ref().and_then(|what| match what.dests.as_slice() {
            [Loc::Held(held)] => Some(held.width),
            _ => None,
        });
        let Some(width) = width.filter(|_| !out.contains_key(&value)) else { continue };
        let mut copy = (**made).clone();
        copy.defines = vec![value];
        if let Some(what) = &mut copy.what {
            what.dests = vec![Loc::Held(crate::model::ir::Held { value, width })];
        }
        out.insert(value, Arc::new(copy));
    }
    out
}

/// The cell a remade `value` is read from, where its one instruction is a plain load: a user that
/// takes a memory operand reads it there, with no register made for it.
fn remade_cell(one: &Insn, value: u32) -> Option<crate::model::ir::Mem> {
    let what = one.what.as_ref()?;
    match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, Some("mov"), [Loc::Held(dest)], [Loc::Mem(cell)]) if dest.value == value && dest.width == cell.width && one.uses.is_empty() && cell.addr.is_some() => {
            Some(cell.clone())
        }
        _ => None,
    }
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

#[derive(Default)]
struct Simulated {
    edits: IndexMap<i64, Edits>,
    across: IndexMap<(i64, i64), Vec<u32>>,
    left: IndexMap<(i64, i64), Vec<u32>>,
    stored: BTreeSet<u32>,
    /// Phi results that live in their slot: what each in-edge hands them is stored there, `(argument, result)`.
    memory: BTreeSet<u32>,
    /// An argument that shares its phi result's slot: stored where it is defined, and no code on the edge.
    shared: IndexMap<u32, u32>,
    moves: IndexMap<(i64, i64), Vec<(u32, u32)>>,
}

impl Simulated {
    /// This with the simulation of another file's values, which are other values.
    fn merge(&mut self, other: Simulated) {
        for (at, edit) in other.edits {
            self.edits.entry(at).or_default().merge(edit);
        }
        for (edge, values) in other.across {
            self.across.entry(edge).or_default().extend(values);
        }
        for (edge, values) in other.left {
            self.left.entry(edge).or_default().extend(values);
        }
        self.stored.extend(other.stored);
        self.memory.extend(other.memory);
        self.shared.extend(other.shared);
        for (edge, values) in other.moves {
            self.moves.entry(edge).or_default().extend(values);
        }
    }
}

impl Edits {
    fn merge(&mut self, other: Edits) {
        for (position, values) in other.before {
            self.before.entry(position).or_default().extend(values);
        }
        self.at_end.extend(other.at_end);
        for (position, values) in other.folded {
            self.folded.entry(position).or_default().extend(values);
        }
        for (position, values) in other.leaves {
            self.leaves.entry(position).or_default().extend(values);
        }
        self.leaves_at_end.extend(other.leaves_at_end);
        self.leaves_at_top.extend(other.leaves_at_top);
        self.w_in.extend(other.w_in);
        self.w_out.extend(other.w_out);
    }
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
    admit: &BTreeSet<i64>,
    memory: &BTreeSet<u32>,
) -> Simulated {
    let k = machine.general.len();
    let wanted = |value: u32| machine.registered(value) && !skip.contains(&value);
    let shared = shared_slots(body, flow, remakes, &wanted, memory);
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
            .filter(|value| wanted(*value) && !memory.contains(value) && !dropped.get(at).is_some_and(|set| set.contains(value)))
            .map(|value| {
                let seen = ends.iter().filter(|set| set.contains(&value)).count();
                let tier = if ends.is_empty() || seen == ends.len() { 0 } else if seen > 0 { 1 } else { 2 };
                let near = flow.uses[at].get(&value).and_then(|list| list.first()).map_or(FAR, |first| *first as i64).min(flow.from_top[at].get(&value).copied().unwrap_or(FAR));
                (tier, near, value)
            })
            .collect();
        candidates.sort_unstable();
        let mut held: BTreeSet<u32> = BTreeSet::new();
        // A memory phi's arguments go through a register on the way in: the block leaves one.
        let top = k;
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
            if (!far || spare > 0) && (*tier < 2 || (header && admit.contains(at)) || block.arrives().contains(value)) {
                let mut next = held.clone();
                next.insert(*value);
                if machine.fits(&next, &BTreeSet::new(), top) {
                    held = next;
                    if far && spare != usize::MAX {
                        spare -= 1;
                    }
                }
            }
        }
        done.w_in = held.clone();
        done.leaves_at_top = block.arrives().into_iter().filter(|value| wanted(*value) && !memory.contains(value) && !held.contains(value)).collect();
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
                    if remakes.get(&value).is_none_or(|made| remade_cell(made, value).is_some()) && done.folded.get(&position).is_none_or(Vec::is_empty) && folds(one, value) {
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
            let mut taken = stated(one, &machine.general, machine.classes);
            if machine.file == File::Selector && machine.general.contains(&machine.data) && target::needs_data_group(one) {
                taken.insert(machine.data);
            }
            // A register a value of this instruction sits in is that value's, not one more.
            let mut covered: BTreeSet<Register> = one.requires.iter().chain(&one.delivers).map(|(_, register)| _whole(*register)).collect();
            if let Some(what) = &one.what {
                for (place, register) in machine.classes.requirements(what) {
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
            .filter(|phi| !memory.contains(&phi.result))
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
            // An argument stored to a memory phi's slot that waits in memory is loaded into a register for the store.
            let stored_through: BTreeSet<u32> = block
                .succ
                .iter()
                .filter_map(|to| by_at.get(to))
                .flat_map(|next| next.phis.iter())
                .filter(|phi| memory.contains(&phi.result))
                .flat_map(|phi| phi.incoming.iter().filter(|(from, value)| from == at && *value != phi.result).map(|(_, value)| *value))
                .filter(|value| !shared.contains_key(value))
                .collect();
            while !machine.fits(&held, &handed, if stored_through.iter().any(|value| !held.contains(value)) { k.saturating_sub(1) } else { k }) {
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
    let mut moves: IndexMap<(i64, i64), Vec<(u32, u32)>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            for phi in by_at[to].phis.iter().filter(|phi| memory.contains(&phi.result)) {
                for (from, value) in phi.incoming.iter().filter(|(from, _)| *from == block.at) {
                    let _ = from;
                    if shared.get(value) == Some(&phi.result) {
                        stored.insert(phi.result);
                        stored.insert(*value);
                        continue;
                    }
                    if *value != phi.result && !moves.get(&(block.at, *to)).is_some_and(|list| list.contains(&(*value, phi.result))) {
                        moves.entry((block.at, *to)).or_default().push((*value, phi.result));
                        stored.insert(phi.result);
                        if !edits.get(&block.at).is_some_and(|here| here.w_out.contains(value)) {
                            stored.insert(*value);
                        }
                    }
                }
            }
        }
    }
    Simulated { edits, across, left, stored, memory: memory.clone(), shared, moves }
}

/// The phis of `first` whose results the block does not take into registers, that can be stored through instead:
/// result and arguments in this file, none of the arguments another such phi's result (the stores would need an order),
/// and the result read somewhere.
fn memory_phis(body: &LirBody, flow: &Flow, remakes: &IndexMap<u32, Arc<Insn>>, first: &Simulated, machine: &Machine<'_>, skip: &BTreeSet<u32>, weights: &Weights<'_>) -> BTreeSet<u32> {
    let wanted = |value: u32| machine.registered(value) && !skip.contains(&value);
    let read: BTreeSet<u32> = body.insns().iter().flat_map(|one| one.uses.iter().copied()).chain(body.blocks.iter().flat_map(|block| block.phis.iter().flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)))).collect();
    let mut reads: IndexMap<u32, f64> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for value in &one.uses {
                *reads.entry(*value).or_default() += weights.block(block.at);
            }
        }
    }
    let home: IndexMap<u32, i64> = flow.defines.iter().flat_map(|(at, made)| made.keys().map(move |value| (*value, *at))).collect();
    let mut chosen: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        let Some(edit) = first.edits.get(&block.at) else { continue };
        // What the block's parallel copy cannot hold in registers must be in memory; the price picks which.
        let arriving: BTreeSet<u32> = edit.w_in.iter().copied().chain(block.arrives().into_iter().filter(|value| wanted(*value))).collect();
        let mut crowd = arriving.len().saturating_sub(machine.general.len());
        let mut eligible: Vec<(f64, u32)> = block
            .phis
            .iter()
            .filter(|phi| wanted(phi.result) && read.contains(&phi.result) && phi.incoming.iter().all(|(_, value)| wanted(*value)))
            .map(|phi| {
                // A load at each read, a store on each in-edge, or at the definition of an argument that shares the slot.
                let one = BTreeSet::from([phi.result]);
                let shared = shared_slots(body, flow, remakes, &wanted, &one);
                let stores: f64 = phi
                    .incoming
                    .iter()
                    .filter(|(_, value)| *value != phi.result)
                    .map(|(from, value)| if shared.contains_key(value) { home.get(value).map_or(0.0, |at| weights.block(*at)) } else { weights.edge(*from, block.at) })
                    .sum();
                (reads.get(&phi.result).copied().unwrap_or(0.0) + stores, phi.result)
            })
            .collect();
        eligible.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        eligible.truncate(crowd);
        // The register form spills each at the top and makes room for it at the copy by evicting what the block
        // reads last, as the choice of what it holds at its top does: a store and the eviction's reloads.
        let top = weights.block(block.at);
        let mut held: Vec<(i64, u32)> = edit
            .w_in
            .iter()
            .copied()
            .filter(|value| !block.arrives().contains(value))
            .map(|value| (flow.uses[&block.at].get(&value).and_then(|list| list.first()).map_or(FAR, |first| *first as i64).min(flow.from_top[&block.at].get(&value).copied().unwrap_or(FAR)), value))
            .collect();
        held.sort_unstable_by(|a, b| b.cmp(a));
        // Past what it can evict the register form has no room: those are memory phis, whatever they cost.
        let forced = eligible.len().saturating_sub(held.len());
        chosen.extend(eligible.iter().take(forced).map(|(_, result)| *result));
        let rest = &eligible[forced..];
        let evicting: f64 = held.iter().take(rest.len()).map(|(near, _)| if *near < EXIT { top } else { 0.0 }).sum();
        let memory_cost: f64 = rest.iter().map(|(cost, result)| cost - reads.get(result).copied().unwrap_or(0.0)).sum();
        if memory_cost < rest.len() as f64 * top + evicting {
            chosen.extend(rest.iter().map(|(_, result)| *result));
        }
        let _ = &mut crowd;
    }
    let phis: IndexMap<u32, &crate::model::lir::Phi> = body.blocks.iter().flat_map(|block| block.phis.iter()).map(|phi| (phi.result, phi)).collect();
    loop {
        let blocked: Vec<u32> = chosen.iter().copied().filter(|result| phis[result].incoming.iter().any(|(_, value)| *value != *result && chosen.contains(value))).collect();
        if blocked.is_empty() {
            return chosen;
        }
        for result in blocked {
            chosen.remove(&result);
        }
    }
}

/// The arguments of the memory phis in `memory` that share their result's slot: stored where they are defined, which
/// leaves the phi no code on the edge. A value shares only where it is dead whenever another member of the web is
/// defined; any other argument is stored on its edge.
fn shared_slots(body: &LirBody, flow: &Flow, remakes: &IndexMap<u32, Arc<Insn>>, wanted: &dyn Fn(u32) -> bool, memory: &BTreeSet<u32>) -> IndexMap<u32, u32> {
    let mut shared: IndexMap<u32, u32> = IndexMap::default();
    if memory.is_empty() || !SHARED_SLOTS.with(std::cell::Cell::get) {
        return shared;
    }
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let phi_block: IndexMap<u32, i64> = body.blocks.iter().flat_map(|block| block.phis.iter().map(move |phi| (phi.result, block.at))).collect();
    // Whether `x` is live where `y` is defined.
    let live_at_def = |x: u32, y: u32| -> bool {
        if let Some(at) = phi_block.get(&y) {
            return flow.live_in[at].contains(&x) || by_at[at].phis.iter().any(|phi| phi.result == x);
        }
        flow.defines.iter().find_map(|(at, made)| made.get(&y).map(|position| (*at, *position))).is_some_and(|(at, position)| flow.live_after(at, position, x))
    };
    let apart = |x: u32, y: u32| !live_at_def(x, y) && !live_at_def(y, x);
    let mut webs: IndexMap<u32, Vec<u32>> = IndexMap::default();
    for block in &body.blocks {
        for phi in block.phis.iter().filter(|phi| memory.contains(&phi.result)) {
            let web = webs.entry(phi.result).or_insert_with(|| vec![phi.result]);
            for (_, value) in &phi.incoming {
                let alone = !shared.contains_key(value) && *value != phi.result && !remakes.contains_key(value) && wanted(*value) && !memory.contains(value);
                if alone && web.iter().all(|member| apart(*member, *value)) {
                    shared.insert(*value, phi.result);
                    web.push(*value);
                }
            }
        }
    }
    shared
}

/// What reloading `value` at its first register uses costs, within one trip of the loop
/// `within` entered at `header`: the frequency of each block on that frontier.
fn first_uses(flow: &Flow, weights: &Weights<'_>, within: &BTreeSet<i64>, header: i64, value: u32) -> f64 {
    let mut cost = 0.0;
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    let mut todo: Vec<i64> = vec![header];
    while let Some(at) = todo.pop() {
        if !seen.insert(at) {
            continue;
        }
        let length = flow.length[&at];
        if flow.uses[&at].get(&value).is_some_and(|list| list.iter().any(|position| *position < length)) {
            cost += weights.block(at);
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

#[derive(Clone, Copy)]
enum Code {
    Store(u32),
    Reload(u32),
    Move(u32, u32),
}

/// What one edge carries: stores of values leaving the register set on it,
/// then reloads of values the successor expects in one.
#[derive(Default)]
struct Crossing {
    stores: Vec<u32>,
    reloads: Vec<u32>,
    /// An argument stored to the slot of the phi result it feeds.
    moves: Vec<(u32, u32)>,
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
    memory: &BTreeSet<u32>,
    shared: &IndexMap<u32, u32>,
    moves: &IndexMap<(i64, i64), Vec<(u32, u32)>>,
    at_leaves: &BTreeSet<u32>,
    remakes: &IndexMap<u32, Arc<Insn>>,
    frame: &mut Frame,
) -> Result<LirBody, String> {
    let widths = widths_through_phis(body, stored);
    let mut cells: IndexMap<u32, crate::model::ir::Mem> = IndexMap::default();
    for value in stored.iter().filter(|value| !remakes.contains_key(*value) && !shared.contains_key(*value)) {
        let width = widths.get(value).copied().unwrap_or(2);
        // Its own keys: a later phase that spills a value with the same number (after phi elimination and coalescing renamed things) must not be handed this slot.
        cells.insert(*value, frame.cell(("ssaspill", i64::from(*value)), width).map_err(|error| error.to_string())?);
    }
    // An argument that shares a phi result's slot: the same cell.
    for (value, result) in shared {
        if let Some(cell) = cells.get(result).cloned() {
            cells.insert(*value, cell);
        }
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
    for (edge, values) in moves {
        crossing.entry(*edge).or_default().moves.extend(values.iter().copied());
    }
    let critical = body.critical_edges();
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
    let mut at_end: IndexMap<i64, Vec<(i64, Code)>> = IndexMap::default();
    let mut at_top: IndexMap<i64, Vec<(i64, Code)>> = IndexMap::default();
    // An argument leaves for the slot of the phi result it feeds, from a register or by way of one.
    let encode = |beside: &Insn, from: i64, one: Code| -> Vec<Arc<Insn>> {
        match one {
            Code::Store(value) => vec![store(beside, value)],
            Code::Reload(value) => vec![reload(beside, value)],
            Code::Move(value, result) => {
                let mut made = Vec::new();
                if !edits[&from].w_out.contains(&value) {
                    made.push(reload(beside, value));
                }
                made.push(spiller::_store(beside, value, &cells[&result]));
                made
            }
        }
    };
    for ((from, to), carried) in &crossing {
        if carried.stores.is_empty() && carried.reloads.is_empty() && carried.moves.is_empty() {
            continue;
        }
        let source = by_at[from];
        let listed: Vec<(i64, Code)> =
            carried.stores.iter().map(|value| Code::Store(*value)).chain(carried.reloads.iter().map(|value| Code::Reload(*value))).chain(carried.moves.iter().map(|(value, result)| Code::Move(*value, *result))).map(|one| (*from, one)).collect();
        if source.succ.len() == 1 {
            at_end.entry(*from).or_default().extend(listed);
        } else if !critical.contains(&(*from, *to)) {
            at_top.entry(*to).or_default().extend(listed);
        } else {
            let at = next_at;
            next_at += 1;
            let beside = anchor(source);
            let beside = &*beside;
            let mut insns: Vec<Arc<Insn>> = listed.iter().flat_map(|(source, one)| encode(beside, *source, *one)).collect();
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
            for phi in block.phis.iter().filter(|phi| !memory.contains(&phi.result)) {
                let leaves_here = edit.leaves_at_top.contains(&phi.result);
                if in_memory(&phi.result) && (!at_leaves.contains(&phi.result) || leaves_here) {
                    insns.push(store(&first, phi.result));
                }
            }
            for (source, one) in at_top.get(&block.at).into_iter().flatten() {
                insns.extend(encode(&first, *source, *one));
            }
        }
        let leave = |position: usize| edit.leaves.get(&position).into_iter().flatten().copied().filter(|value| in_memory(value) && at_leaves.contains(value));
        let ending = |beside: &Insn, insns: &mut Vec<Arc<Insn>>| {
            for value in edit.leaves_at_end.iter().copied().filter(|value| in_memory(value) && at_leaves.contains(value)) {
                insns.push(store(beside, value));
            }
            for (source, one) in at_end.get(&block.at).into_iter().flatten() {
                insns.extend(encode(beside, *source, *one));
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
                let cell = match remakes.get(value) {
                    Some(made) => remade_cell(made, *value).expect("a remade load"),
                    None => cells[value].clone(),
                };
                one = folded_into(&one, *value, &cell).expect("a fold the simulation chose");
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
        let mut made = LirBlock { succ, ..block.with_insns(insns) };
        made.phis.retain(|phi| !memory.contains(&phi.result));
        blocks.push(made);
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

    /// A copy of a load was made again as that load, from the source's stability alone: the cell it read was written
    /// before the copy's last use, and the fuzz of the shared predicate stored through a wrong address.
    #[test]
    fn test_a_copy_of_a_load_is_not_remade_after_its_cell_changes() {
        use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
        let cell = Mem { addr: Some(Addr::new(Space::Segment, 0)), ..Mem::new(None, 2) };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let what = |op, name: &str, dests, sources| Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) });
        let one = |at: i64, what, defines: Vec<u32>, uses: Vec<u32>| Arc::new(Insn::new(at, Some((at, at)), what, defines, uses));
        let insns = vec![
            one(0, what(Operation::Move, "mov", vec![held(1)], vec![Loc::Mem(cell.clone())]), vec![1], vec![]),
            one(1, what(Operation::Move, "mov", vec![held(2)], vec![held(1)]), vec![2], vec![1]),
            one(2, what(Operation::Binary, "add", vec![held(3)], vec![held(1), held(1)]), vec![3], vec![1]),
            one(3, what(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![Loc::Imm(Imm { value: 9, width: 2, address: None })]), vec![], vec![]),
            one(4, what(Operation::Binary, "add", vec![held(4)], vec![held(2), held(2)]), vec![4], vec![2]),
        ];
        let body = LirBody::new("t", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());
        let made = remakable(&body, &BTreeSet::from([1, 2]));
        // The load is made again for its copies too, so it holds only to the last of their uses.
        assert!(!made.contains_key(&2), "the copy is read after the cell was written");
        assert!(!made.contains_key(&1), "the load is made again for a copy read after the write");
    }

    /// A load the optimizer proved no write in the loop changes was held in a stack slot all the same: a store through a
    /// far pointer has no address in LIR, so the cell never held (particle bas: +0.6% instructions, +30 B).
    fn spared_body(spared: bool, store_address: Option<crate::model::ir::Addr>) -> LirBody {
        use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
        let cell = Mem { addr: Some(Addr::new(Space::Segment, 2)), ..Mem::new(None, 2) };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let what = |op, name: &str, dests, sources| Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) });
        let one = |at: i64, what, defines: Vec<u32>, uses: Vec<u32>| Arc::new(Insn::new(at, Some((at, at)), what, defines, uses));
        let into = Mem { addr: store_address, ..Mem::new(None, 2) };
        let insns = vec![
            one(0, what(Operation::Move, "mov", vec![held(1)], vec![Loc::Mem(cell)]), vec![1], vec![]),
            one(1, what(Operation::Move, "mov", vec![Loc::Mem(into)], vec![Loc::Imm(Imm { value: 9, width: 2, address: None })]), vec![], vec![]),
            one(2, what(Operation::Binary, "add", vec![held(2)], vec![held(1), held(1)]), vec![2], vec![1]),
        ];
        let mut body = LirBody::new("t", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());
        if spared {
            body.spares = Arc::new(BTreeSet::from([(0, 1)]));
        }
        body
    }

    #[test]
    fn test_a_load_the_optimizer_proved_apart_from_a_write_holds_across_it() {
        assert!(remakable(&spared_body(true, None), &BTreeSet::from([1])).contains_key(&1), "the unknown address is the pair's to answer");
    }

    #[test]
    fn test_a_far_write_proved_apart_from_a_load_does_not_end_it() {
        use crate::model::ir::{Addr, Space};
        assert!(remakable(&spared_body(true, Some(Addr::new(Space::Far, 100))), &BTreeSet::from([1])).contains_key(&1));
        assert!(!remakable(&spared_body(false, Some(Addr::new(Space::Far, 100))), &BTreeSet::from([1])).contains_key(&1));
    }

    #[test]
    fn test_a_write_not_proved_apart_still_ends_the_load() {
        assert!(!remakable(&spared_body(false, None), &BTreeSet::from([1])).contains_key(&1));
    }

    #[test]
    fn test_a_proved_pair_does_not_excuse_a_write_to_the_cell_itself() {
        use crate::model::ir::{Addr, Space};
        assert!(!remakable(&spared_body(true, Some(Addr::new(Space::Segment, 2))), &BTreeSet::from([1])).contains_key(&1));
    }

    /// Before #516 nothing in `remakable` or `_stable_loads_through` looked at `volatile` (the #507 path included): a load
    /// of a device cell, read twice, was made again at its second use, a second read the program never asked for. No
    /// bench or QCport object differs with the guard removed, so no blessed row since #507 depended on it.
    #[test]
    fn test_a_volatile_load_is_never_made_again() {
        use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
        let cell = Mem { addr: Some(Addr::new(Space::Segment, 0)), ..Mem::new(None, 2) };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let what = |op, name: &str, dests, sources| Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) });
        let load = Insn { volatile: true, ..Insn::new(0, Some((0, 0)), what(Operation::Move, "mov", vec![held(1)], vec![Loc::Mem(cell)]), vec![1], vec![]) };
        let insns = vec![
            Arc::new(load),
            Arc::new(Insn::new(1, Some((1, 1)), what(Operation::Binary, "add", vec![held(2)], vec![held(1), held(1)]), vec![2], vec![1])),
            Arc::new(Insn::new(2, Some((2, 2)), what(Operation::Binary, "add", vec![held(3)], vec![held(1), held(2)]), vec![3], vec![1, 2])),
        ];
        let body = LirBody::new("t", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());
        assert!(!remakable(&body, &BTreeSet::from([1])).contains_key(&1));
        assert!(spiller::_stable_loads(&body, &BTreeSet::from([1])).is_empty());
    }

    /// Two values that may only sit in BX (each is the base of an address
    /// somewhere) were counted as a byte pair although only one acts: deedlines
    /// COPPER evicted down to three held values in a six-register machine, and
    /// its palette loop reloaded two addresses every iteration.
    #[test]
    fn test_two_base_only_values_fit_while_one_acts() {
        let bx = BTreeSet::from([Register::EBX]);
        let confined: Classes = [(1, bx.clone()), (2, bx), (3, BTreeSet::from([Register::ESI, Register::EDI]))].into_iter().collect();
        let classes = crate::backend::classes::RegisterClasses::m16();
        let machine = Machine::of(&confined, File::General, &target::BUILT_IN, &classes);
        let held: BTreeSet<u32> = [1, 2, 3].into_iter().collect();
        assert!(machine.fits(&held, &BTreeSet::from([1]), 6), "one acting BX value leaves the other waiting");
        assert!(!machine.fits(&held, &BTreeSet::from([1, 2]), 6), "premise: two acting BX values cannot both sit in BX");
    }
}
