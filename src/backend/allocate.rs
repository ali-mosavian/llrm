//! Port of `qbopt/backend/allocate.py`: registers for a lowered body, using
//! greedy allocation with eviction.
//!
//! LLVM's `RegAllocGreedy`: spilling is priced by `intervals::weights`, and
//! fixed intervals go first.

use std::cell::RefCell;
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, BinaryHeap};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::intervals::{self as ranges, Indexes, Interval};
use crate::analysis::loops;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::{self as frames, Frame, Refused};
use crate::backend::{coalesce, constrain, datagroup, spiller, spillplacement, splitkit, target};
use crate::model::ir::{self, Addr, Held, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::pyrepr::Repr;

/// Maximum queue visits before the remaining values are spilled.
pub const BUDGET: usize = 200_000;

/// How long a range may be and still count as a reload.
pub const RELOAD: i64 = 4 * ranges::PER_INSN;

/// A value reached emission with no register. Always a bug here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unplaced(pub String);

/// A value the allocator chose to spill, and nothing writes the spill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spilled(pub String);

impl fmt::Display for Unplaced {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for Spilled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unplaced {}
impl std::error::Error for Spilled {}

/// Every exception that may leave the register-allocation modules.
///
/// Python raises these through one another's frames; Rust says which one
/// it was so `except Unplaced` can catch exactly that one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Unplaced(Unplaced),
    Spilled(Spilled),
    Impossible(constrain::Impossible),
    Refused(Refused),
    /// `ValueError`, or a call into a module not yet ported.
    Value(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unplaced(one) => one.fmt(formatter),
            Self::Spilled(one) => one.fmt(formatter),
            Self::Impossible(one) => one.fmt(formatter),
            Self::Refused(one) => one.fmt(formatter),
            Self::Value(one) => formatter.write_str(one),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Python's class name and message, for a caller that catches by class.
    pub fn raised(&self) -> Exception {
        let (module, kind) = match self {
            Self::Unplaced(_) => ("qbopt.backend.allocate", "Unplaced"),
            Self::Spilled(_) => ("qbopt.backend.allocate", "Spilled"),
            Self::Impossible(_) => ("qbopt.backend.constrain", "Impossible"),
            Self::Refused(_) => ("qbopt.backend.frame", "Refused"),
            Self::Value(_) => ("builtins", "ValueError"),
        };
        Exception::defined_in(module, kind, self.to_string())
    }
}

impl From<Unplaced> for Error {
    fn from(one: Unplaced) -> Self {
        Self::Unplaced(one)
    }
}

impl From<Spilled> for Error {
    fn from(one: Spilled) -> Self {
        Self::Spilled(one)
    }
}

impl From<constrain::Impossible> for Error {
    fn from(one: constrain::Impossible) -> Self {
        Self::Impossible(one)
    }
}

impl From<Refused> for Error {
    fn from(one: Refused) -> Self {
        Self::Refused(one)
    }
}

/// Where each value lives, what it cost, and whether that is the best.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub r#where: IndexMap<u32, Register>,
    pub spilled: BTreeSet<u32>,
    pub cost: f64,
    pub optimal: bool,
    pub why: String,
}

/// The `group` run ending at `index`: where it starts.
pub fn _group_start(block: &LirBlock, index: usize) -> usize {
    let one = &block.insns[index];
    let mut first = index;
    if one.group.is_some() {
        while first > 0 && block.insns[first - 1].group == one.group {
            first -= 1;
        }
    }
    first
}

pub type Live = IndexMap<i64, BTreeSet<u32>>;

/// What is live at each block's entry and exit, to a fixed point.
pub fn live(body: &LirBody) -> (Live, Live) {
    let defines: IndexMap<i64, BTreeSet<u32>> = body
        .blocks
        .iter()
        .map(|block| {
            let mut found: BTreeSet<u32> = block.arrives().into_iter().collect();
            found.extend(block.insns.iter().flat_map(|one| one.defines.iter().copied()));
            (block.at, found)
        })
        .collect();
    let mut exposed: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> = BTreeSet::new();
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let first = _group_start(block, index as usize);
            let group = &block.insns[first..=index as usize];
            for item in group {
                for value in &item.defines {
                    alive.remove(value);
                }
            }
            for item in group {
                alive.extend(item.uses.iter().copied());
            }
            index = first as i64 - 1;
        }
        let arrives: BTreeSet<u32> = block.arrives().into_iter().collect();
        exposed.insert(block.at, alive.difference(&arrives).copied().collect());
    }

    // The least fixed point of a backward problem, found by a worklist over dense bit
    // sets: the round-robin over sorted sets it replaces reached the same sets.
    // Values numbered densely: ids can be far apart.
    let numbered: Vec<u32> = body
        .blocks
        .iter()
        .flat_map(|block| defines[&block.at].iter().chain(&exposed[&block.at]).copied())
        .collect::<BTreeSet<u32>>()
        .into_iter()
        .collect();
    let number: crate::support::hash::HashMap<u32, usize> =
        numbered.iter().enumerate().map(|(at, value)| (*value, at)).collect();
    let words = numbered.len() / 64 + 1;
    let bits = |values: &BTreeSet<u32>| {
        let mut set = vec![0u64; words];
        for value in values {
            let at = number[value];
            set[at / 64] |= 1 << (at % 64);
        }
        set
    };
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut predecessors = vec![Vec::new(); body.blocks.len()];
    for (at, block) in body.blocks.iter().enumerate() {
        for successor in &block.succ {
            if let Some(&to) = position.get(successor) {
                predecessors[to].push(at);
            }
        }
    }
    let kept: Vec<Vec<u64>> = body.blocks.iter().map(|block| bits(&defines[&block.at]).iter().map(|word| !word).collect()).collect();
    let generated: Vec<Vec<u64>> = body.blocks.iter().map(|block| bits(&exposed[&block.at])).collect();
    let mut into: Vec<Vec<u64>> = generated.clone();
    let mut out: Vec<Vec<u64>> = vec![vec![0u64; words]; body.blocks.len()];
    let mut pending: Vec<usize> = (0..body.blocks.len()).collect();
    let mut queued = vec![true; body.blocks.len()];
    while let Some(at) = pending.pop() {
        queued[at] = false;
        let mut now = vec![0u64; words];
        for successor in &body.blocks[at].succ {
            if let Some(&from) = position.get(successor) {
                for (word, bits) in now.iter_mut().zip(&into[from]) {
                    *word |= bits;
                }
            }
        }
        let entering: Vec<u64> = (0..words).map(|word| generated[at][word] | (now[word] & kept[at][word])).collect();
        out[at] = now;
        if entering != into[at] {
            into[at] = entering;
            for &pred in &predecessors[at] {
                if !queued[pred] {
                    queued[pred] = true;
                    pending.push(pred);
                }
            }
        }
    }
    let numbered = &numbered;
    let values = |set: &[u64]| -> BTreeSet<u32> {
        set.iter()
            .enumerate()
            .flat_map(|(word, bits)| (0..64).filter(move |bit| bits >> bit & 1 == 1).map(move |bit| numbered[word * 64 + bit]))
            .collect()
    };
    let live_in: Live = body.blocks.iter().zip(&into).map(|(block, set)| (block.at, values(set))).collect();
    let live_out: Live = body.blocks.iter().zip(&out).map(|(block, set)| (block.at, values(set))).collect();
    (live_in, live_out)
}

/// A call's results that nothing reads, said as clobbers instead.
pub fn narrowed(body: &LirBody, pinned: &IndexMap<u32, Register>) -> (LirBody, IndexMap<u32, Register>) {
    let mut read: BTreeSet<u32> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter().flat_map(|one| one.uses.iter().copied()))
        .collect();
    read.extend(body.blocks.iter().flat_map(LirBlock::arrives));
    let mut dropped: IndexMap<u32, Register> = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            let dead: Vec<u32> = one
                .defines
                .iter()
                .copied()
                .filter(|value| !read.contains(value) && pinned.get(value).is_some())
                .collect();
            if one.what.as_ref().is_none_or(|what| what.op != Operation::Call) || dead.is_empty() {
                insns.push(Arc::clone(one));
                continue;
            }
            let gone: IndexMap<u32, Register> = dead.iter().map(|value| (*value, pinned[value])).collect();
            dropped.extend(gone.iter().map(|(value, register)| (*value, *register)));
            let mut made = (**one).clone();
            made.defines = one.defines.iter().copied().filter(|value| !gone.contains_key(value)).collect();
            made.delivers = one
                .delivers
                .iter()
                .copied()
                .filter(|(held, _register)| !gone.contains_key(&held.value))
                .collect();
            made.clobbers.extend(gone.values().copied());
            insns.push(Arc::new(made));
        }
        blocks.push(block.with_insns(insns));
    }
    if dropped.is_empty() {
        return (body.clone(), pinned.clone());
    }
    (
        body.with_blocks(blocks),
        pinned
            .iter()
            .filter(|(value, _where)| !dropped.contains_key(*value))
            .map(|(value, register)| (*value, *register))
            .collect(),
    )
}

/// Which values are ever live at the same moment.
pub fn interference(body: &LirBody) -> IndexMap<u32, BTreeSet<u32>> {
    let (_into, out_of) = live(body);
    let mut graph: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();

    let meet = |graph: &mut IndexMap<u32, BTreeSet<u32>>, alive: &BTreeSet<u32>| {
        for one in alive {
            graph
                .entry(*one)
                .or_default()
                .extend(alive.iter().copied().filter(|other| other != one));
        }
    };

    for block in &body.blocks {
        for one in &block.insns {
            for value in one.defines.iter().chain(&one.uses) {
                graph.entry(*value).or_default();
            }
        }
        for value in block.arrives() {
            graph.entry(value).or_default();
        }
        let mut alive = out_of[&block.at].clone();
        meet(&mut graph, &alive);
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let first = _group_start(block, index as usize);
            let group = &block.insns[first..=index as usize];
            for item in group {
                for value in &item.defines {
                    alive.remove(value);
                }
            }
            for item in group {
                alive.extend(item.uses.iter().copied());
            }
            meet(&mut graph, &alive);
            index = first as i64 - 1;
        }
    }
    graph
}

/// How far a range has got, and therefore what may still be tried on it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Stage {
    Assign = 0,
    Split = 1,
    Spill = 2,
    Done = 3,
}

pub type Classes = IndexMap<u32, BTreeSet<Register>>;

fn _restrict(out: &mut Classes, value: u32, choices: &BTreeSet<Register>) {
    let now: BTreeSet<Register> = match out.get(&value) {
        Some(had) => had.intersection(choices).copied().collect(),
        None => choices.clone(),
    };
    out.insert(value, now);
}

/// The register class each value is confined to, where it is confined.
pub fn classes(body: &LirBody, prefer_indexes: &BTreeSet<u32>) -> Classes {
    let mut out: Classes = IndexMap::default();
    let mut selecting: BTreeSet<u32> = BTreeSet::new();
    let mut numeric: BTreeSet<u32> = BTreeSet::new();
    let mut word_pairs: Vec<(u32, u32)> = Vec::new();
    let bytes: BTreeSet<Register> = BTreeSet::from([Register::AX, Register::BX, Register::CX, Register::DX]);

    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if let Some(selector) = cell.selector {
                        selecting.insert(selector.value);
                    }
                    if let Some(base) = cell.base {
                        numeric.insert(base.value);
                    }
                }
                if let Loc::Held(held) = place {
                    if held.width != 2 || !_SEGMENT_OPERANDS.contains(&what.op) {
                        numeric.insert(held.value);
                    }
                }
                if let Loc::Mem(cell) = place {
                    if let (Some(base), None) = (cell.base, cell.index) {
                        if base.width == 2 {
                            let registers = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                                &*target::WORD_INDEXES
                            } else {
                                &*target::ADDRESSING
                            };
                            _restrict(&mut out, base.value, registers);
                        }
                    }
                    if let Some(index) = cell.index {
                        numeric.insert(index.value);
                        if index.width == 2 {
                            if cell.base.is_some_and(|base| base.width == 2) && cell.scale == 1 {
                                word_pairs.push((cell.base.expect("checked").value, index.value));
                            } else {
                                _restrict(&mut out, index.value, &target::WORD_INDEXES);
                                if let Some(base) = cell.base {
                                    _restrict(&mut out, base.value, &target::WORD_BASES);
                                }
                            }
                        }
                    }
                }
                if let Loc::Held(held) = place {
                    if held.width == 1 {
                        _restrict(&mut out, held.value, &bytes);
                    }
                }
            }
        }
    }
    let selectors: BTreeSet<Register> = target::SELECTORS.iter().copied().collect();
    for value in selecting.difference(&numeric) {
        _restrict(&mut out, *value, &selectors);
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            if target::far_load(what) {
                if let Loc::Held(held) = &what.dests[1] {
                    _restrict(&mut out, held.value, &selectors);
                }
            }
        }
    }
    _word_address_roles(&word_pairs, &mut out, body, prefer_indexes);
    out
}

/// Choose BX versus SI/DI for commutative `[word+word]` graphs.
fn _word_address_roles(
    pairs: &[(u32, u32)],
    confined: &mut Classes,
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
) {
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for (base, index) in pairs {
        adjacent.entry(*base).or_default().insert(*index);
        adjacent.entry(*index).or_default().insert(*base);
    }
    let mut unseen: BTreeSet<u32> = adjacent.keys().copied().collect();
    let numbered = ranges::indexed(body);
    let live = ranges::intervals(body, Some(&numbered));
    let masks = _masks(body, &numbered);
    let word_base = *target::WORD_BASES.iter().next().expect("one word base");

    let base_penalty = |values: &BTreeSet<u32>| -> i64 {
        values
            .iter()
            .filter_map(|value| live.get(value))
            .map(|interval| i64::from(_clobbered(interval, word_base, &masks, 2)))
            .sum()
    };

    let allowed = |confined: &Classes, values: &BTreeSet<u32>, choices: &BTreeSet<Register>| -> bool {
        values.iter().all(|value| match confined.get(value) {
            Some(had) => had.intersection(choices).next().is_some(),
            None => !choices.is_empty(),
        })
    };

    let restrict = |confined: &mut Classes, values: &BTreeSet<u32>, choices: &BTreeSet<Register>| {
        for value in values {
            _restrict(confined, *value, choices);
        }
    };

    while let Some(&seed) = unseen.iter().next() {
        let mut colors: IndexMap<u32, u8> = IndexMap::from_iter([(seed, 0)]);
        let mut work = vec![seed];
        let mut bipartite = true;
        while let Some(value) = work.pop() {
            for other in &adjacent[&value] {
                match colors.get(other) {
                    None => {
                        let color = 1 - colors[&value];
                        colors.insert(*other, color);
                        work.push(*other);
                    }
                    Some(color) if *color == colors[&value] => bipartite = false,
                    Some(_) => {}
                }
            }
        }
        let component: BTreeSet<u32> = colors.keys().copied().collect();
        for value in &component {
            unseen.remove(value);
        }
        let source_spelling = |confined: &mut Classes| {
            for (base, index) in pairs {
                if component.contains(base) {
                    restrict(confined, &BTreeSet::from([*base]), &target::WORD_BASES);
                    restrict(confined, &BTreeSet::from([*index]), &target::WORD_INDEXES);
                }
            }
        };
        if !bipartite {
            source_spelling(confined);
            continue;
        }
        let sides: (BTreeSet<u32>, BTreeSet<u32>) = (
            colors.iter().filter(|(_value, color)| **color == 0).map(|(value, _)| *value).collect(),
            colors.iter().filter(|(_value, color)| **color != 0).map(|(value, _)| *value).collect(),
        );
        let options: Vec<(&BTreeSet<u32>, &BTreeSet<u32>)> = [(&sides.0, &sides.1), (&sides.1, &sides.0)]
            .into_iter()
            .filter(|(left, right)| {
                allowed(confined, left, &target::WORD_BASES) && allowed(confined, right, &target::WORD_INDEXES)
            })
            .collect();
        if options.is_empty() {
            source_spelling(confined);
            continue;
        }
        let key = |option: &(&BTreeSet<u32>, &BTreeSet<u32>)| {
            (
                base_penalty(option.0),
                option.0.intersection(prefer_indexes).count(),
                option.0.len(),
                option.0.iter().copied().collect::<Vec<u32>>(),
            )
        };
        // `min` keeps the first of equal keys.
        let mut best = options[0];
        for option in &options[1..] {
            if key(option) < key(&best) {
                best = *option;
            }
        }
        let (bases, indexes) = (best.0.clone(), best.1.clone());
        restrict(confined, &bases, &target::WORD_BASES);
        restrict(confined, &indexes, &target::WORD_INDEXES);
    }
}

const _SEGMENT_OPERANDS: [Operation; 3] = [Operation::Move, Operation::Push, Operation::Pop];

/// A plain move into one value; whether anything reads it is the caller's.
pub fn _unread_move(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    what.op == Operation::Move
        && what.dests.len() == 1
        && what.sources.len() == 1
        && matches!(what.dests[0], Loc::Held(_))
        && matches!(what.sources[0], Loc::Imm(_) | Loc::Held(_))
        && matches!(&what.dests[0], Loc::Held(dest) if one.defines == [dest.value])
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.group.is_none()
        && one.symbol != Some(true)
}

/// Each far cell whose selector is also read as a number, or pinned to a
/// general register, reached through ES.
pub fn explicit_selectors(body: &LirBody, pinned: Option<&IndexMap<u32, Register>>) -> LirBody {
    let confined = classes(body, &BTreeSet::new());
    let selectors: BTreeSet<Register> = target::SELECTORS.iter().copied().collect();
    let empty = IndexMap::default();
    let pinned = pinned.unwrap_or(&empty);
    let mut conflicted: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if let Some(selector) = cell.selector {
                        if confined.get(&selector.value) != Some(&selectors)
                            || !target::SEGMENTS
                                .contains(pinned.get(&selector.value).unwrap_or(&Register::ES))
                        {
                            conflicted.insert(selector.value);
                        }
                    }
                }
            }
        }
    }
    if conflicted.is_empty() {
        return body.clone();
    }

    let through_es = |place: &Loc| -> Loc {
        if let Loc::Mem(cell) = place {
            if cell.selector.is_some_and(|selector| conflicted.contains(&selector.value)) {
                let addr = cell.addr.expect("a far cell has an address");
                return Loc::Mem(Mem { selector: None, addr: Some(Addr { segment: Register::ES, ..addr }), ..cell.clone() });
            }
        }
        place.clone()
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            let named: Vec<Held> = match &one.what {
                None => Vec::new(),
                Some(what) => what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .filter_map(|place| match place {
                        Loc::Mem(cell) => cell.selector.filter(|selector| conflicted.contains(&selector.value)),
                        _ => None,
                    })
                    .collect(),
            };
            if named.is_empty() {
                insns.push(Arc::clone(one));
                continue;
            }
            let what = one.what.as_ref().expect("named is empty without semantics");
            let what = Semantics {
                dests: what.dests.iter().map(&through_es).collect(),
                sources: what.sources.iter().map(&through_es).collect(),
                ..what.clone()
            };
            let requires: IndexSet<(Held, Register)> = one
                .requires
                .iter()
                .copied()
                .chain(named.iter().map(|held| (*held, Register::ES)))
                .collect();
            let uses: IndexSet<u32> = one.uses.iter().copied().chain(named.iter().map(|held| held.value)).collect();
            let mut made = (**one).clone();
            made.what = Some(what);
            made.requires = requires.into_iter().collect();
            made.uses = uses.into_iter().collect();
            insns.push(Arc::new(made));
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

fn _copy_hints(body: &LirBody) -> IndexMap<u32, Vec<u32>> {
    let mut hints: IndexMap<u32, Vec<u32>> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            if let Some(what) = &one.what {
                if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                    if let ([Loc::Held(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) {
                        if dest.width == source.width && dest.value != source.value {
                            hints.entry(dest.value).or_default().push(source.value);
                            hints.entry(source.value).or_default().push(dest.value);
                        }
                    }
                }
            }
        }
    }
    hints
}

/// A heap entry: `(value not in fixed, -priority, value)`.
#[derive(Clone, Copy, Debug)]
struct Queued(bool, f64, u32);

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Queued {}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> Ordering {
        // Python compares floats with `<`, so -0.0 and 0.0 tie.
        self.0
            .cmp(&other.0)
            .then(self.1.partial_cmp(&other.1).expect("priorities are never NaN"))
            .then(self.2.cmp(&other.2))
    }
}

const INF: f64 = f64::INFINITY;

/// The fixed register each value reaches through the fewest copies.
///
/// A loop's sum placed before the return's copy to AX is otherwise seated
/// where nothing asked, and whatever took AX leaves a move on the exit.
fn _wanted(hints: &IndexMap<u32, Vec<u32>>, fixed: &IndexMap<u32, Register>) -> IndexMap<u32, Register> {
    let mut wanted: IndexMap<u32, Register> = fixed.iter().map(|(value, register)| (*value, _whole(*register))).collect();
    let mut frontier: Vec<u32> = wanted.keys().copied().collect();
    while !frontier.is_empty() {
        let mut reached = Vec::new();
        for value in frontier {
            for other in hints.get(&value).into_iter().flatten() {
                if !wanted.contains_key(other) {
                    wanted.insert(*other, wanted[&value]);
                    reached.push(*other);
                }
            }
        }
        frontier = reached;
    }
    wanted.into_iter().filter(|(value, _)| !fixed.contains_key(value)).collect()
}

/// A register for every value, by LLVM's `RegAllocGreedy`.
#[allow(clippy::too_many_arguments)]
pub fn allocate(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
    unspillable: Option<&BTreeSet<u32>>,
    protected: Option<&BTreeSet<u32>>,
    preferred: Option<&IndexMap<u32, Register>>,
    cpu: ProfileOrName<'_>,
) -> Result<Assignment, Error> {
    Ok(_allocated(body, pinned, unspillable, protected, preferred, cpu, None)?.0)
}

/// A value's range divided in allocation: the piece `fresh` holds `region`.
#[derive(Clone, Debug)]
pub struct Split {
    pub value: u32,
    pub fresh: u32,
    pub region: splitkit::Region,
    /// Whether the piece's references outweigh its copies. One that does
    /// not pays only by letting the rest of the value into a register.
    pub pays: bool,
}

/// `allocate`, splitting a value that can neither be placed nor evict around
/// the region split placement chooses, as `RAGreedy::tryRegionSplit` does;
/// the pieces then compete again. The splits are virtual: carve them into
/// the body before using the assignment.
/// `settled` are the values an earlier split made, or left behind: as
/// LLVM's `RS_Split2` and `RS_Spill`, they are never split again.
pub fn planned(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
    unspillable: Option<&BTreeSet<u32>>,
    settled: &BTreeSet<u32>,
    cpu: ProfileOrName<'_>,
) -> Result<(Assignment, Vec<Split>), Error> {
    _allocated(body, pinned, unspillable, None, None, cpu, Some(settled))
}

#[allow(clippy::too_many_arguments)]
fn _allocated(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
    unspillable: Option<&BTreeSet<u32>>,
    protected: Option<&BTreeSet<u32>>,
    preferred: Option<&IndexMap<u32, Register>>,
    cpu: ProfileOrName<'_>,
    splitting: Option<&BTreeSet<u32>>,
) -> Result<(Assignment, Vec<Split>), Error> {
    let profile = targets::profile(cpu).map_err(Error::Value)?;
    let index = ranges::indexed(body);
    let mut live = _fold_priced(body, _sibling_priced(body, ranges::intervals(body, Some(&index))), profile);
    let masks = _masks(body, &index);
    let data_free = !datagroup::names_data_segment(body);
    let mut widths = _widest(body);
    let mut splits: Vec<Split> = Vec::new();
    let mut pieces: BTreeSet<u32> = splitting.cloned().unwrap_or_default();
    let mut fresh = splitkit::_next_value(body);
    let mut placing: Option<(spillplacement::Bundles, (Live, Live))> = None;
    for one in unspillable.into_iter().flatten() {
        if let Some(interval) = live.get_mut(one) {
            if interval.size() <= RELOAD {
                interval.weight = INF;
            }
        }
    }
    let empty = BTreeSet::new();
    let protected = protected.unwrap_or(&empty);
    for one in protected {
        if let Some(interval) = live.get_mut(one) {
            interval.weight = INF;
        }
    }
    let mut confined = classes(body, protected);
    let fixed: IndexMap<u32, Register> = pinned.cloned().unwrap_or_default();
    let hints = _copy_hints(body);
    let wanted = _wanted(&hints, &fixed);
    let mut claims: IndexMap<Register, Vec<u32>> = IndexMap::default();
    for (one, register) in &wanted {
        claims.entry(*register).or_default().push(*one);
    }
    let no_preference = IndexMap::default();
    let preferred = preferred.unwrap_or(&no_preference);

    let mut union: IndexMap<Register, Vec<u32>> = IndexMap::default();
    let mut r#where: IndexMap<u32, Register> = IndexMap::default();
    let mut stage: IndexMap<u32, Stage> = IndexMap::default();
    let mut spilled: BTreeSet<u32> = BTreeSet::new();
    let mut cost = 0.0;
    let mut cascades: IndexMap<u32, i64> = IndexMap::default();
    let mut newest = 1;

    let queued = |value: u32, live: &IndexMap<u32, Interval>, stage: &IndexMap<u32, Stage>| {
        Reverse(Queued(
            !fixed.contains_key(&value),
            -_priority(live.get(&value), stage.get(&value).copied().unwrap_or(Stage::Assign)),
            value,
        ))
    };

    let mut queue: BinaryHeap<Reverse<Queued>> = _values(body).into_iter().map(|one| queued(one, &live, &stage)).collect();
    let mut seen = 0;
    let fenced: BTreeSet<u32> = fixed.keys().copied().chain(protected.iter().copied()).collect();
    while !queue.is_empty() && seen < BUDGET {
        seen += 1;
        let Reverse(Queued(_flexible, _prio, value)) = queue.pop().expect("queue is not empty");
        if r#where.contains_key(&value) || spilled.contains(&value) {
            continue;
        }
        let at = *stage.entry(value).or_insert(Stage::Assign);
        let Some(mine) = live.get(&value).cloned() else {
            continue;
        };
        let mut order: Vec<Register> = match fixed.get(&value) {
            None => target::order(confined.get(&value)),
            Some(register) => vec![*register],
        };
        if !data_free {
            order.retain(|one| _whole(*one) != *target::DATA_SEGMENT);
        }
        if protected.contains(&value) && _reserves_word_base(body, value, &confined) {
            let word: BTreeSet<Register> = target::WORD_BASES.iter().map(|one| _whole(*one)).collect();
            order = order
                .iter()
                .copied()
                .filter(|one| !word.contains(&_whole(*one)))
                .chain(order.iter().copied().filter(|one| word.contains(&_whole(*one))))
                .collect();
        }
        if !fixed.contains_key(&value) {
            let mut votes: IndexMap<Register, i64> = IndexMap::default();
            for other in hints.get(&value).into_iter().flatten() {
                if let Some(register) = fixed.get(other).or_else(|| r#where.get(other)) {
                    *votes.entry(_whole(*register)).or_insert(0) += 1;
                }
            }
            if votes.is_empty() {
                if let Some(register) = wanted.get(&value) {
                    *votes.entry(*register).or_insert(0) += 1;
                }
            }
            let claimed: BTreeSet<Register> = claims
                .iter()
                .filter(|(_, others)| {
                    others.iter().any(|other| {
                        *other != value
                            && !r#where.contains_key(other)
                            && live.get(other).is_some_and(|theirs| theirs.overlaps(&mine))
                    })
                })
                .map(|(register, _)| *register)
                .collect();
            order.sort_by_key(|register| {
                (-votes.get(&_whole(*register)).copied().unwrap_or(0), claimed.contains(&_whole(*register)))
            });
            if let Some(choice) = preferred.get(&value) {
                let wanted = _whole(*choice);
                order = order
                    .iter()
                    .copied()
                    .filter(|one| _whole(*one) == wanted)
                    .chain(order.iter().copied().filter(|one| _whole(*one) != wanted))
                    .collect();
            }
        }

        // The data segment register only once the selectors run out: holding
        // a value there costs a restore and a prefix on every data access.
        order.sort_by_key(|one| _whole(*one) == *target::DATA_SEGMENT);
        let width = widths.get(&value).copied().unwrap_or(4);
        if let Some(got) = _free(&mine, &order, &union, &live, &masks, width) {
            r#where.insert(value, got);
            union.entry(_whole(got)).or_default().push(value);
            stage.insert(value, Stage::Done);
            continue;
        }

        if at == Stage::Assign {
            let movable = |other: u32, register: Register| -> bool {
                if fixed.contains_key(&other) {
                    return false;
                }
                let elsewhere: Vec<Register> = target::order(confined.get(&other))
                    .into_iter()
                    .filter(|one| _whole(*one) != _whole(register))
                    .filter(|one| data_free || _whole(*one) != *target::DATA_SEGMENT)
                    .collect();
                _free(&live[&other], &elsewhere, &union, &live, &masks, widths.get(&other).copied().unwrap_or(4))
                    .is_some()
            };
            let evicted = _evict(
                &mine,
                &order,
                &union,
                &live,
                &masks,
                &movable,
                &fenced,
                width,
                Some(cascades.get(&value).copied().unwrap_or(newest)),
                Some(&cascades),
            );
            if let Some((got, victims)) = evicted {
                if !cascades.contains_key(&value) {
                    cascades.insert(value, newest);
                    newest += 1;
                }
                for one in victims {
                    let holders = union.get_mut(&_whole(got)).expect("a victim is in the register it was taken from");
                    let position = holders.iter().position(|other| *other == one).expect("a victim is listed");
                    holders.remove(position);
                    r#where.shift_remove(&one);
                    cascades.insert(one, cascades[&value]);
                    stage.insert(one, Stage::Assign);
                    queue.push(queued(one, &live, &stage));
                }
                r#where.insert(value, got);
                union.entry(_whole(got)).or_default().push(value);
                stage.insert(value, Stage::Done);
                continue;
            }
            stage.insert(value, Stage::Split);
            queue.push(queued(value, &live, &stage));
            continue;
        }

        let bound = fixed.contains_key(&value) || mine.weight == INF;
        if splitting.is_some() && at == Stage::Split && !pieces.contains(&value) && !bound {
            let (bundles, live_sets) = placing.get_or_insert_with(|| (spillplacement::bundles(body), self::live(body)));
            let occupied = splitkit::Occupied {
                segments: union
                    .iter()
                    .map(|(register, held)| {
                        (*register, held.iter().filter(|other| **other != value).flat_map(|other| live[other].segments.clone()).collect())
                    })
                    .collect(),
                masks: &masks,
            };
            let sets = (&live_sets.0, &live_sets.1);
            // `trySplit`: a range in one block splits locally; any other by
            // region, and failing that block by block.
            let region: Vec<splitkit::Region> = match splitkit::local(body, value, &index, sets, &order, &occupied, width) {
                Some(found) => vec![found],
                None => {
                    let placed = splitkit::placed(body, value, &index, sets, bundles, &order, &occupied, width);
                    if placed.is_empty() { splitkit::per_block(body, value, sets) } else { placed }
                }
            };
            let mut rest = mine.clone();
            let mut divided = false;
            // Each piece's class, and the rest's, from the references it
            // keeps: LLVM's `recomputeRegClass`. The rest of a value only an
            // address confined may take any register.
            let mut scratch = body.clone();
            let mut moves: Vec<splitkit::Moved> = Vec::new();
            let mut made: Vec<u32> = Vec::new();
            for region in region {
                let (piece, remaining) = splitkit::divided(body, &index, &rest, &region, fresh);
                if piece.segments.is_empty() || remaining.segments.is_empty() {
                    continue;
                }
                let moved = moves.iter().fold(region.clone(), |region, moved| region.moved(moved));
                let Some((cut, shifted)) = splitkit::carved_moving(&scratch, value, fresh, width, &moved) else {
                    continue;
                };
                scratch = cut;
                moves.push(shifted);
                rest = remaining;
                live.insert(fresh, piece);
                widths.insert(fresh, width);
                stage.insert(fresh, Stage::Assign);
                pieces.insert(fresh);
                let pays = splitkit::pays(body, value, &region, sets);
                splits.push(Split { value, fresh, region, pays });
                made.push(fresh);
                fresh += 1;
                divided = true;
            }
            if divided {
                let recomputed = classes(&scratch, protected);
                for one in made.iter().copied().chain([value]) {
                    match recomputed.get(&one) {
                        Some(class) => confined.insert(one, class.clone()),
                        None => confined.shift_remove(&one),
                    };
                }
                for one in made {
                    queue.push(queued(one, &live, &stage));
                }
            }
            if divided {
                // The rest may only take a free register or spill; the pieces compete again.
                live.insert(value, rest);
                stage.insert(value, Stage::Spill);
                pieces.insert(value);
                queue.push(queued(value, &live, &stage));
                continue;
            }
        }
        {
            // Last chance: move what holds a register rather than spill.
            let choices = |other: u32| -> Vec<Register> {
                let mut order = match fixed.get(&other) {
                    None => target::order(confined.get(&other)),
                    Some(register) => vec![*register],
                };
                order.retain(|one| data_free || _whole(*one) != *target::DATA_SEGMENT);
                order.sort_by_key(|one| _whole(*one) == *target::DATA_SEGMENT);
                order
            };
            let wide = |other: u32| widths.get(&other).copied().unwrap_or(4);
            let mut coloring = Coloring {
                union: &mut union,
                r#where: &mut r#where,
                live: &live,
                masks: &masks,
                order: &choices,
                width: &wide,
                fenced: &fenced,
                budget: Coloring::BUDGET,
                stack: Vec::new(),
            };
            if coloring.recolor(value, 0, &mut BTreeSet::new()) {
                stage.insert(value, Stage::Done);
                continue;
            }
        }
        if let Some(register) = fixed.get(&value) {
            return Err(Unplaced(format!("value#{value} cannot be placed in fixed {}", register.repr())).into());
        }
        if mine.weight == INF {
            return Err(Unplaced(format!("value#{value} cannot be spilled and no register is free for it")).into());
        }
        spilled.insert(value);
        cost += mine.weight;
        stage.insert(value, Stage::Done);
    }

    Ok((Assignment { r#where, spilled, cost, optimal: false, why: "greedy with eviction".to_owned() }, splits))
}

/// Allocate one evaluated retention plan, or discard that plan.
fn _assigned_plan(
    body: &LirBody,
    pinned: &IndexMap<u32, Register>,
    reloads: &BTreeSet<u32>,
    retained: &BTreeSet<u32>,
    cpu: &Profile,
) -> Result<(Assignment, BTreeSet<u32>), Error> {
    if !retained.is_empty() {
        match allocate(body, Some(pinned), Some(reloads), Some(retained), None, cpu.into()) {
            Ok(got) => return Ok((got, retained.clone())),
            Err(Error::Unplaced(_)) => {}
            Err(other) => return Err(other),
        }
    }
    Ok((allocate(body, Some(pinned), Some(reloads), None, None, cpu.into())?, BTreeSet::new()))
}

/// Every value that wants a register, dead definitions included.
fn _values(body: &LirBody) -> Vec<u32> {
    let mut out: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        out.extend(block.arrives());
        for one in &block.insns {
            out.extend(one.defines.iter().copied());
            out.extend(one.uses.iter().copied());
        }
    }
    out.into_iter().collect()
}

/// Where this range sits in the queue. Larger first, as LLVM does.
fn _priority(one: Option<&Interval>, at: Stage) -> f64 {
    match one {
        None => 0.0,
        Some(one) => one.size() as f64 + if at != Stage::Assign { 1e6 } else { 0.0 },
    }
}

/// Whether an address-class value should leave a 16-bit word base free.
fn _reserves_word_base(body: &LirBody, value: u32, confined: &Classes) -> bool {
    let Some(choices) = confined.get(&value) else {
        return false;
    };
    if choices.is_empty() || !choices.is_subset(&target::ADDRESSING) {
        return false;
    }
    body.blocks.iter().flat_map(|block| &block.insns).any(|one| {
        one.what.as_ref().is_some_and(|what| {
            what.dests.iter().chain(&what.sources).any(|place| match place {
                Loc::Mem(cell) => match (cell.base, cell.index) {
                    (Some(base), Some(index)) => base.value != value && index.value != value && cell.scale == 1,
                    _ => false,
                },
                _ => false,
            })
        })
    })
}

/// How wide each value is anywhere it is read or written.
pub fn _widest(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            let mut held: Vec<Held> = match &one.what {
                Some(what) => what.dests.iter().chain(&what.sources).flat_map(ir::values).collect(),
                None => Vec::new(),
            };
            held.extend(one.requires.iter().chain(&one.delivers).map(|(place, _register)| *place));
            for place in held {
                let had = out.get(&place.value).copied().unwrap_or(0);
                out.insert(place.value, had.max(place.width));
            }
            for (value, width) in &one.widths {
                let had = out.get(value).copied().unwrap_or(0);
                out.insert(*value, had.max(*width));
            }
        }
    }
    out
}

/// A point that destroys registers without naming them: `during` the
/// instruction, their high halves only, or `before` it reads its operands.
pub struct Mask {
    pub slot: i64,
    pub during: BTreeSet<Register>,
    pub high: BTreeSet<Register>,
    pub before: BTreeSet<Register>,
}

pub type Masks = Vec<Mask>;

/// Every point a register is destroyed without being named, and which. The
/// data segment register is reloaded ahead of a point that needs the data
/// group, so it holds none of that point's operands either.
pub fn _masks(body: &LirBody, index: &Indexes) -> Masks {
    let mut out = Vec::new();
    for block in &body.blocks {
        for one in &block.insns {
            let during: BTreeSet<Register> = one.clobbers.iter().map(|register| _whole(*register)).collect();
            let high: BTreeSet<Register> = one.clobbers_high.iter().map(|register| _whole(*register)).collect();
            let before: BTreeSet<Register> =
                target::needs_data_group(one).then_some(*target::DATA_SEGMENT).into_iter().collect();
            if !during.is_empty() || !high.is_empty() || !before.is_empty() {
                out.push(Mask { slot: index.at[&ranges::key(one)], during, high, before });
            }
        }
    }
    out
}

/// The 32-bit register this one is part of.
pub fn _whole(register: Register) -> Register {
    ir::root(register)
}

/// Whether this range is live across a point that destroys the register, or
/// into one that destroys it before reading.
pub fn _clobbered(one: &Interval, register: Register, masks: &Masks, width: u32) -> bool {
    let mine = _whole(register);
    for mask in masks {
        let slot = mask.slot;
        let read = mask.before.contains(&mine);
        if !read && !mask.during.contains(&mine) && (!mask.high.contains(&mine) || width <= 2) {
            continue;
        }
        // A use keeps its value alive to `slot + DEF`.
        let reaches = |end: i64| if read { end >= slot + ranges::DEF } else { end > slot + ranges::DEF };
        if one.segments.iter().any(|seg| seg.start < slot && reaches(seg.end)) {
            return true;
        }
    }
    false
}

/// A register nothing live at the same time is using, and no call kills.
fn _free(
    one: &Interval,
    order: &[Register],
    union: &IndexMap<Register, Vec<u32>>,
    live: &IndexMap<u32, Interval>,
    masks: &Masks,
    width: u32,
) -> Option<Register> {
    for register in order {
        if _clobbered(one, *register, masks, width) {
            continue;
        }
        let busy = union
            .get(&_whole(*register))
            .into_iter()
            .flatten()
            .filter_map(|other| live.get(other))
            .any(|other| other.overlaps(one));
        if !busy {
            return Some(*register);
        }
    }
    None
}

/// The cheapest register to take, and what has to move out of it.
#[allow(clippy::too_many_arguments)]
fn _evict(
    one: &Interval,
    order: &[Register],
    union: &IndexMap<Register, Vec<u32>>,
    live: &IndexMap<u32, Interval>,
    masks: &Masks,
    movable: &dyn Fn(u32, Register) -> bool,
    protected: &BTreeSet<u32>,
    width: u32,
    cascade: Option<i64>,
    cascades: Option<&IndexMap<u32, i64>>,
) -> Option<(Register, Vec<u32>)> {
    let mut best: Option<(f64, Register, Vec<u32>)> = None;
    for register in order {
        if _clobbered(one, *register, masks, width) {
            continue;
        }
        let victims: Vec<u32> = union
            .get(&_whole(*register))
            .into_iter()
            .flatten()
            .copied()
            .filter(|other| live.get(other).is_some_and(|found| found.overlaps(one)))
            .collect();
        if victims.is_empty() {
            continue;
        }
        if victims.iter().any(|other| protected.contains(other)) {
            continue;
        }
        if let Some(cascade) = cascade {
            if victims
                .iter()
                .any(|other| cascades.and_then(|found| found.get(other)).copied().unwrap_or(0) >= cascade)
            {
                continue;
            }
        }
        let mut bill = 0.0;
        for other in &victims {
            bill += if movable(*other, *register) { 0.0 } else { live[other].weight };
        }
        if bill >= one.weight {
            continue;
        }
        if best.as_ref().is_none_or(|found| bill < found.0) {
            best = Some((bill, *register, victims));
        }
    }
    best.map(|(_bill, register, victims)| (register, victims))
}

/// What last-chance recoloring may change: LLVM's `LiveRegMatrix` and `VirtRegMap`.
pub struct Coloring<'a> {
    pub union: &'a mut IndexMap<Register, Vec<u32>>,
    pub r#where: &'a mut IndexMap<u32, Register>,
    pub live: &'a IndexMap<u32, Interval>,
    pub masks: &'a Masks,
    /// Each value's registers, in the order it tries them.
    pub order: &'a dyn Fn(u32) -> Vec<Register>,
    pub width: &'a dyn Fn(u32) -> u32,
    /// Values whose register is not the allocator's to change.
    pub fenced: &'a BTreeSet<u32>,
    /// Recoloring attempts left, bounding the search as a whole.
    pub budget: usize,
    /// Each moved value and the register it held: LLVM's `RecolorStack`.
    pub stack: Vec<(u32, Register)>,
}

impl Coloring<'_> {
    /// LLVM's `lcr-max-depth` and `lcr-max-interf`.
    pub const DEPTH: usize = 5;
    pub const INTERFERENCES: usize = 8;
    /// Registers tried per recoloring session.
    pub const BUDGET: usize = 2000;

    fn take(&mut self, value: u32, register: Register) {
        self.r#where.insert(value, register);
        self.union.entry(_whole(register)).or_default().push(value);
    }

    fn release(&mut self, value: u32) {
        if let Some(register) = self.r#where.shift_remove(&value) {
            self.union.get_mut(&_whole(register)).expect("a placed value is in its register").retain(|other| *other != value);
        }
    }

    /// `RAGreedy::tryLastChanceRecoloring`: a register for `value` whose
    /// holders all move elsewhere, recursively. `recolored` are the values
    /// placed in this session, never moved again. On success `value` is
    /// placed; on failure nothing has changed.
    pub fn recolor(&mut self, value: u32, depth: usize, recolored: &mut BTreeSet<u32>) -> bool {
        if depth >= Self::DEPTH {
            return false;
        }
        let mine = &self.live[&value];
        let width = (self.width)(value);
        recolored.insert(value);
        for register in (self.order)(value) {
            if self.budget == 0 {
                break;
            }
            self.budget -= 1;
            if _clobbered(mine, register, self.masks, width) {
                continue;
            }
            let mut holders: Vec<u32> = self
                .union
                .get(&_whole(register))
                .into_iter()
                .flatten()
                .copied()
                .filter(|other| self.live.get(other).is_some_and(|found| found.overlaps(mine)))
                .collect();
            if holders.len() >= Self::INTERFERENCES
                || holders.iter().any(|other| self.fenced.contains(other) || recolored.contains(other))
            {
                continue;
            }
            // Largest first, as the allocator's queue orders them.
            holders.sort_by_key(|other| Reverse(self.live[other].size()));
            let entry = self.stack.len();
            let session = recolored.clone();
            for other in &holders {
                self.stack.push((*other, self.r#where[other]));
                self.release(*other);
            }
            self.take(value, register);
            let all = holders.iter().all(|other| {
                match _free(&self.live[other], &(self.order)(*other), self.union, self.live, self.masks, (self.width)(*other)) {
                    Some(found) => {
                        self.take(*other, found);
                        recolored.insert(*other);
                        true
                    }
                    None => self.recolor(*other, depth + 1, recolored),
                }
            });
            if all {
                return true;
            }
            // Undo every move this attempt made, deeper ones included.
            self.release(value);
            let moved: Vec<(u32, Register)> = self.stack.drain(entry..).collect();
            for (other, _) in &moved {
                self.release(*other);
            }
            for (other, register) in moved {
                self.take(other, register);
            }
            *recolored = session;
            recolored.insert(value);
        }
        recolored.remove(&value);
        false
    }
}

/// Assign, then rewrite. LLVM's two halves, in one phase.
pub struct RegAlloc {
    pub pinned: IndexMap<u32, Register>,
    /// One frame, shared with the phases around this one, as Python shares it.
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
}

impl RegAlloc {
    pub const NAME: &'static str = "regalloc";

    /// How many times a body may be spilled and re-allocated.
    pub const ROUNDS: usize = 12;

    /// How many times region splitting may carve and plan again in one round.
    pub const SPLIT_PASSES: usize = 6;

    pub fn new(
        pinned: Option<&IndexMap<u32, Register>>,
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: ProfileOrName<'_>,
    ) -> Result<Self, String> {
        Ok(Self { pinned: pinned.cloned().unwrap_or_default(), frame, cpu: targets::profile(cpu)?.clone() })
    }

    /// Allocate with region splits, carve them, and allocate the carved body
    /// preferring the planned registers: kept when the traffic it spills and
    /// the copies it adds, both weighted by loop depth, are below `got`'s.
    fn _region_split(&self, body: &LirBody, reloads: &BTreeSet<u32>, got: &Assignment, settled: &mut BTreeSet<u32>) -> Result<Option<(LirBody, Assignment)>, Error> {
        let mut cut = body.clone();
        let mut plan = None;
        let before = settled.clone();
        // Each carve lets the pieces evict; what they evict splits in the next pass.
        for _ in 0..Self::SPLIT_PASSES {
            let (found, splits) = match planned(&cut, Some(&self.pinned), Some(reloads), settled, (&self.cpu).into()) {
                Ok(found) => found,
                Err(Error::Unplaced(_)) => break,
                Err(other) => return Err(other),
            };
            let widths = _widest(&cut);
            let mut next = cut.clone();
            // A piece that found no register would only add copies between two spills.
            // Each carve moves the positions the later regions were planned at.
            let mut moves: Vec<splitkit::Moved> = Vec::new();
            let kept = |split: &&Split| {
                found.r#where.contains_key(&split.fresh) && (split.pays || found.r#where.contains_key(&split.value))
            };
            for split in splits.iter().filter(kept) {
                let region = moves.iter().fold(split.region.clone(), |region, moved| region.moved(moved));
                if let Some((carved, moved)) = splitkit::carved_moving(&next, split.value, split.fresh, widths.get(&split.value).copied().unwrap_or(4), &region) {
                    next = carved;
                    moves.push(moved);
                    settled.extend([split.value, split.fresh]);
                }
            }
            plan = Some(found);
            if next == cut {
                break;
            }
            cut = next;
        }
        let Some(plan) = plan.filter(|_| cut != *body) else {
            *settled = before;
            return Ok(None);
        };
        let after = match allocate(&cut, Some(&self.pinned), Some(reloads), None, Some(&plan.r#where), (&self.cpu).into()) {
            Ok(after) => after,
            Err(Error::Unplaced(_)) => {
                *settled = before;
                return Ok(None);
            }
            Err(other) => return Err(other),
        };
        let (was, now) = (_traffic(body, &got.spilled), _traffic(&cut, &after.spilled) + _added(body, &cut));
        crate::debug!("regalloc", "  region split: traffic {was} -> {now} (spilled {} added {})", _traffic(&cut, &after.spilled), _added(body, &cut));
        if now >= was {
            *settled = before;
            return Ok(None);
        }
        Ok(Some((cut, after)))
    }

    /// Assign; where that spills, make the spill real and assign again.
    pub fn transform(&mut self, body: LirBody) -> Result<LirBody, Error> {
        if self.frame.is_none() {
            self.frame = Some(Rc::new(RefCell::new(frames::of(&body, None, "", None)?)));
        }
        let cpu = self.cpu.clone();
        // Only a body that never names the data segment register itself may
        // find it holding one of its values.
        let data_free = !datagroup::names_data_segment(&body);
        let mut body = explicit_selectors(&body, Some(&self.pinned));
        let (narrowed_body, narrower) = narrowed(&body, &self.pinned);
        body = narrowed_body;
        self.pinned = narrower;
        let confined = classes(&body, &BTreeSet::new());
        let incompatible: BTreeSet<u32> = self
            .pinned
            .iter()
            .filter(|(value, register)| {
                confined.get(*value).is_some_and(|choices| {
                    !choices.iter().map(|choice| _whole(*choice)).any(|choice| choice == _whole(**register))
                })
            })
            .map(|(value, _register)| *value)
            .collect();
        if !incompatible.is_empty() {
            let delivered: BTreeSet<u32> = body
                .blocks
                .iter()
                .flat_map(|block| &block.insns)
                .flat_map(|one| one.delivers.iter().map(|(held, _register)| held.value))
                .collect();
            let missing: BTreeSet<u32> = incompatible.difference(&delivered).copied().collect();
            if let Some(value) = missing.first() {
                return Err(Unplaced(format!(
                    "value#{value} is pinned outside its register class and has no defining occurrence to split"
                ))
                .into());
            }
            self.pinned.retain(|value, _register| !incompatible.contains(value));
        }
        let selectors: BTreeSet<Register> = target::SELECTORS.iter().copied().collect();
        self.pinned.retain(|value, register| {
            !(target::SEGMENTS.contains(register) && confined.get(value) == Some(&selectors))
        });
        let (constrained_body, fixed) = constrain::constrained(&body, Some(&self.pinned))?;
        body = constrained_body;
        let clash: BTreeSet<u32> = fixed
            .keys()
            .copied()
            .filter(|one| self.pinned.get(one).is_some_and(|register| *register != fixed[one]))
            .collect();
        if let Some(first) = clash.first() {
            return Err(Unplaced(format!("value#{first} is pinned and required in different registers")).into());
        }
        let mut prefer = self.pinned.clone();
        prefer.extend(fixed.iter().map(|(value, register)| (*value, *register)));
        let mut reloads: BTreeSet<u32> = fixed.keys().copied().collect();
        let mut retained: BTreeSet<u32> = BTreeSet::new();

        // Values a region split made or left behind, never split again: LLVM's stages.
        let mut settled: BTreeSet<u32> = BTreeSet::new();
        for round in 0..Self::ROUNDS {
            let abandoned: BTreeSet<usize> = body
                .blocks
                .iter()
                .flat_map(|block| &block.insns)
                .filter(|one| _unread_move(one))
                .map(ranges::key)
                .collect();
            body = spiller::_remove_abandoned(&body, &abandoned);
            self.pinned = prefer.clone();
            self.pinned.extend(constrain::required(&body)?);
            let (answer, now) = _assigned_plan(&body, &self.pinned, &reloads, &retained, &cpu)?;
            let mut got = answer;
            retained = now;
            crate::debug!(
                "regalloc",
                "{}: round {}: {} insns, {} spilled",
                body.name,
                round + 1,
                body.blocks.iter().map(|block| block.insns.len()).sum::<usize>(),
                got.spilled.len()
            );
            let pinned = self.pinned.clone();
            let trial_of = |candidate: &LirBody,
                            unspillable: &BTreeSet<u32>,
                            protected: &BTreeSet<u32>|
             -> Result<Option<Assignment>, Error> {
                match allocate(candidate, Some(&pinned), Some(unspillable), Some(protected), None, (&cpu).into()) {
                    Ok(trial) => {
                        crate::debug!("regalloc", "  trial allocation: {} spilled", trial.spilled.len());
                        Ok(Some(trial))
                    }
                    Err(Error::Unplaced(_)) => Ok(None),
                    Err(other) => Err(other),
                }
            };
            if retained.is_empty() && !got.spilled.is_empty() {
                if let Some((cut, after)) = self._region_split(&body, &reloads, &got, &mut settled)? {
                    body = cut;
                    got = after;
                }
            }
            if !got.spilled.is_empty() {
                let (separated, opened) = constrain::addressed(&body, &got.spilled);
                if !opened.is_empty() {
                    let trial = trial_of(&separated, &reloads, &retained)?;
                    if let Some(trial) = trial {
                        if opened.is_disjoint(&trial.spilled)
                            && _traffic(&separated, &trial.spilled) < _traffic(&body, &got.spilled)
                        {
                            body = separated;
                            got = trial;
                        }
                    }
                }
            }
            if !got.spilled.is_empty() {
                let (unfolded, opened) = spiller::unfolded_indexes(&body, &got.spilled);
                if !opened.is_empty() {
                    let trial = trial_of(&unfolded, &reloads, &retained)?;
                    if let Some(trial) = trial {
                        if _traffic(&unfolded, &trial.spilled) < _traffic(&body, &got.spilled) {
                            body = unfolded;
                            got = trial;
                        }
                    }
                }
            }
            if retained.is_empty() && !got.spilled.is_empty() {
                let (scoped, keep) = splitkit::loop_bases(&body, &got.spilled);
                if !keep.is_empty() {
                    let folded = _scoped_foldable_indexes(&scoped, &keep);
                    let (mut opened_body, opened) = spiller::unfolded_indexes(&scoped, &folded);
                    if !opened.is_empty() {
                        let mut opened_reloads = reloads.clone();
                        let before = self.frame.as_ref().map(|frame| frame.borrow().saved());
                        let mut trial = trial_of(&opened_body, &reloads, &keep)?;
                        if let Some(found) = &trial {
                            let rematerializable = spiller::rematerializable(&opened_body, &found.spilled);
                            if !rematerializable.is_empty() {
                                let (remade, made) =
                                    spiller::spilled(&opened_body, &rematerializable, self.frame.as_ref().map(|one| one.borrow_mut()).as_deref_mut())?;
                                if remade != opened_body {
                                    opened_body = remade;
                                    opened_reloads.extend(made);
                                    trial = trial_of(&opened_body, &opened_reloads, &keep)?;
                                }
                            }
                        }
                        if let Some(trial) = trial.filter(|trial| {
                            keep.is_disjoint(&trial.spilled)
                                && _traffic(&opened_body, &trial.spilled) + _added(&scoped, &opened_body)
                                    < _traffic(&body, &got.spilled)
                        }) {
                            body = opened_body;
                            retained = keep.clone();
                            got = trial;
                            reloads = opened_reloads;
                        } else if let (Some(frame), Some(before)) = (&self.frame, &before) {
                            frame.borrow_mut().restore(before);
                        }
                    }
                    if retained.is_empty() {
                        // If the unfolded indexes still do not fit, commit
                        // them to slots and consume those slots directly in
                        // the identical dying-base adds.
                        let before = self.frame.as_ref().map(|frame| frame.borrow().saved());
                        let (prepared, made) = if folded.is_empty() {
                            (scoped.clone(), BTreeSet::new())
                        } else {
                            spiller::spilled(&scoped, &folded, self.frame.as_ref().map(|one| one.borrow_mut()).as_deref_mut())?
                        };
                        let with: BTreeSet<u32> = reloads.union(&made).copied().collect();
                        let trial = trial_of(&prepared, &with, &keep)?;
                        // The candidate may spill ordinary cold values into
                        // normal slots. Admit it only when the whole
                        // pre-folded trial lowers weighted traffic --
                        // including its own folded slots, whose values
                        // `prepared` no longer names.
                        let slot_traffic = match &self.frame {
                            Some(frame) => _slot_traffic(&prepared, &frame.borrow(), &folded),
                            None => 0.0,
                        };
                        if let Some(trial) = trial.filter(|trial| {
                            keep.is_disjoint(&trial.spilled)
                                && _traffic(&prepared, &trial.spilled) + slot_traffic < _traffic(&body, &got.spilled)
                        }) {
                            body = prepared;
                            retained = keep.clone();
                            got = trial;
                            reloads = with;
                        } else if let (Some(frame), Some(before)) = (&self.frame, &before) {
                            frame.borrow_mut().restore(before);
                        }
                    }
                }
                if retained.is_empty() {
                    let candidates = _retainable_bases(&body, &got.spilled);
                    let mut ordered: Vec<(f64, u32)> = candidates
                        .iter()
                        .map(|value| (-_traffic(&body, &BTreeSet::from([*value])), *value))
                        .collect();
                    ordered.sort_by(|one, other| {
                        one.0.partial_cmp(&other.0).expect("traffic is never NaN").then(one.1.cmp(&other.1))
                    });
                    for (_traffic_key, candidate) in ordered {
                        let mut keep = retained.clone();
                        keep.insert(candidate);
                        let Some(trial) = trial_of(&body, &reloads, &keep)? else {
                            continue;
                        };
                        let mut recoverable = spiller::rematerializable(&body, &trial.spilled);
                        recoverable.extend(spiller::foldable_indexes(&body, &trial.spilled));
                        if keep.is_disjoint(&trial.spilled)
                            && trial.spilled.is_subset(&recoverable)
                            && _traffic(&body, &trial.spilled) < _traffic(&body, &got.spilled)
                        {
                            retained = keep;
                            got = trial;
                        }
                    }
                }
            }
            if got.spilled.is_empty() {
                return applied(&body, &got).map(|placed| datagroup::restored(&placed, data_free));
            }
            if !retained.is_empty() {
                let (spilt, made) = spiller::spilled(&body, &got.spilled, self.frame.as_ref().map(|one| one.borrow_mut()).as_deref_mut())?;
                body = spilt;
                reloads.extend(made);
                continue;
            }
            let rematerializable = spiller::rematerializable(&body, &got.spilled);
            if !rematerializable.is_empty() {
                let (remade, made) = spiller::spilled(&body, &rematerializable, self.frame.as_ref().map(|one| one.borrow_mut()).as_deref_mut())?;
                if remade != body {
                    body = remade;
                    reloads.extend(made);
                    continue;
                }
            }
            let fixed: BTreeSet<u32> = self
                .pinned
                .keys()
                .copied()
                .chain(reloads.iter().copied())
                .chain(retained.iter().copied())
                .collect();
            let mut chosen = got.spilled.clone();
            chosen.extend(spiller::siblings(&body, &got.spilled, self.frame.as_ref().map(|one| one.borrow_mut()).as_deref_mut(), &fixed)?);
            let (spilt, made) = spiller::spilled(&body, &chosen, self.frame.as_ref().map(|one| one.borrow_mut()).as_deref_mut())?;
            body = spilt;
            reloads.extend(made);
        }
        self.pinned = prefer.clone();
        self.pinned.extend(constrain::required(&body)?);
        let (answer, _retained) = _assigned_plan(&body, &self.pinned, &reloads, &retained, &cpu)?;
        applied(&body, &answer).map(|placed| datagroup::restored(&placed, data_free))
    }
}

impl LIRTransform for RegAlloc {
    fn class_name(&self) -> &'static str {
        "RegAlloc"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        RegAlloc::transform(self, body).map_err(|error| error.to_string())
    }

    fn transform_raising(&mut self, body: LirBody) -> Result<LirBody, Exception> {
        RegAlloc::transform(self, body).map_err(|error| error.raised())
    }
}

/// Python `max(first, second)`: the first unless the second is larger.
fn _max(first: f64, second: f64) -> f64 {
    if second > first { second } else { first }
}

/// Python `min(first, second)`: the first unless the second is smaller.
fn _min(first: f64, second: f64) -> f64 {
    if second < first { second } else { first }
}

/// Intervals whose copies to a value they could share a slot with cost nothing.
fn _sibling_priced(body: &LirBody, live: IndexMap<u32, Interval>) -> IndexMap<u32, Interval> {
    let moves: Vec<(i64, (u32, u32))> = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .insns
                .iter()
                .filter_map(|one| spiller::_plain_move(one).filter(|pair| pair.0 != pair.1))
                .map(move |pair| (block.at, pair))
        })
        .collect();
    if moves.is_empty() {
        return live;
    }
    let mut impure: BTreeSet<u32> = BTreeSet::new();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if spiller::_plain_move(one).is_some() {
            continue;
        }
        let in_place = one.what.as_ref().is_some_and(|what| {
            matches!(what.op, Operation::Binary | Operation::Unary)
                && one.requires.is_empty()
                && one.delivers.is_empty()
                && !what.dests.iter().chain(&what.sources).any(|x| matches!(x, Loc::Mem(_)))
        });
        impure.extend(
            one.defines
                .iter()
                .chain(&one.uses)
                .copied()
                .filter(|value| !(in_place && one.defines.contains(value) && one.uses.contains(value))),
        );
    }
    let near = coalesce::_interference(body);
    let deep = ranges::depths(body);
    let mut free: IndexMap<u32, f64> = IndexMap::default();
    for (at, (into, out_of)) in moves {
        if impure.contains(&into)
            || impure.contains(&out_of)
            || near.get(&into).is_some_and(|found| found.contains(&out_of))
        {
            continue;
        }
        let each = ranges::level(deep.get(&at).copied().unwrap_or(0));
        for value in [into, out_of] {
            *free.entry(value).or_insert(0.0) += each;
        }
    }
    live.into_iter()
        .map(|(value, one)| match free.get(&value) {
            Some(found) => {
                let weight = _max(0.0, one.weight - found / (one.size() + ranges::GRACE) as f64);
                (value, Interval { weight, ..one })
            }
            None => (value, one),
        })
        .collect()
}

/// The memory references spilling these values costs, weighted by loop depth.
pub fn _traffic(body: &LirBody, spilled: &BTreeSet<u32>) -> f64 {
    let deep = ranges::depths(body);
    let mut total = 0.0;
    for block in &body.blocks {
        let each = ranges::level(deep.get(&block.at).copied().unwrap_or(0));
        for one in &block.insns {
            for value in one.defines.iter().chain(&one.uses) {
                if spilled.contains(value) {
                    total += each;
                }
            }
        }
    }
    total
}

/// Frame traffic inside loops by cause, weighted by loop depth: the spiller's
/// reloads, stores and rematerializations, and the frame operands of x87 and
/// other instructions. The allocator can reach only the first three.
pub fn traffic_by_cause(body: &LirBody) -> std::collections::BTreeMap<&'static str, f64> {
    let deep = ranges::depths(body);
    let mut out = std::collections::BTreeMap::new();
    for block in &body.blocks {
        let depth = deep.get(&block.at).copied().unwrap_or(0);
        if depth == 0 {
            continue;
        }
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            let frame = what.dests.iter().chain(&what.sources).any(|place| matches!(place, Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)));
            let float = matches!(what.op, Operation::FloatLoad | Operation::FloatStore | Operation::FloatArith | Operation::FloatArithPop | Operation::FloatUnary);
            let cause = match () {
                _ if one.spill_reload => "reload",
                _ if one.spill_store => "store",
                _ if one.rematerialized => "remat",
                _ if frame && float => "x87-frame",
                _ if frame => "int-frame",
                _ => continue,
            };
            *out.entry(cause).or_insert(0.0) += ranges::level(depth);
        }
    }
    out
}

/// The instructions a plan inserted, weighted by loop depth.
///
/// Unpriced, sum_three's unfolded `add di,bx` looked free and the loop grew
/// an instruction.
fn _added(before: &LirBody, after: &LirBody) -> f64 {
    let deep = ranges::depths(after);
    let was: IndexMap<i64, usize> = before.blocks.iter().map(|block| (block.at, block.insns.len())).collect();
    after
        .blocks
        .iter()
        .map(|block| {
            ranges::level(deep.get(&block.at).copied().unwrap_or(0))
                * block.insns.len().saturating_sub(was.get(&block.at).copied().unwrap_or(0)) as f64
        })
        .sum()
}

/// The memory references to these values' frame slots, weighted by loop depth.
fn _slot_traffic(body: &LirBody, frame: &Frame, values: &BTreeSet<u32>) -> f64 {
    let homes: BTreeSet<i64> =
        values.iter().filter_map(|value| frame.slots.get(&frames::SlotKey::from(*value)).copied()).collect();
    if homes.is_empty() {
        return 0.0;
    }
    let deep = ranges::depths(body);
    let mut total = 0.0;
    for block in &body.blocks {
        let each = ranges::level(deep.get(&block.at).copied().unwrap_or(0));
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if cell.addr.is_some_and(|addr| addr.space == Space::Frame && homes.contains(&addr.disp))
                        && cell.base.is_none()
                    {
                        total += each;
                    }
                }
            }
        }
    }
    total
}

/// Spilled invariant frame loads that repeatedly form addresses.
fn _retainable_bases(body: &LirBody, spilled: &BTreeSet<u32>) -> BTreeSet<u32> {
    if spilled.is_empty() {
        return BTreeSet::new();
    }
    let stable: BTreeSet<u32> = spiller::_stable_loads(body, spilled).keys().copied().collect();
    if stable.is_empty() {
        return BTreeSet::new();
    }
    let deep = ranges::depths(body);
    let mut references: IndexMap<u32, i64> = IndexMap::default();
    for block in &body.blocks {
        let weight = ranges::PER_LEVEL.pow(deep.get(&block.at).copied().unwrap_or(0));
        for one in &block.insns {
            let bases: BTreeSet<u32> = match &one.what {
                Some(what) => what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .filter_map(|place| match place {
                        Loc::Mem(cell) => cell.base.map(|base| base.value),
                        _ => None,
                    })
                    .collect(),
                None => BTreeSet::new(),
            };
            for value in stable.intersection(&bases) {
                *references.entry(*value).or_insert(0) += weight;
            }
        }
    }
    let repeated: BTreeSet<u32> =
        references.iter().filter(|(_value, weight)| **weight > 1).map(|(value, _)| *value).collect();
    stable.intersection(&repeated).copied().collect()
}

/// Dying word indexes in a natural loop that actually uses `bases`.
fn _scoped_foldable_indexes(body: &LirBody, bases: &BTreeSet<u32>) -> BTreeSet<u32> {
    if bases.is_empty() {
        return BTreeSet::new();
    }
    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut indexes: BTreeSet<u32> = BTreeSet::new();
    for found in loops::loops(&ranges::_graph(&body.blocks), Some(body.entry)) {
        let cells: Vec<&Mem> = found
            .body
            .iter()
            .map(|at| blocks[at])
            .flat_map(|block| &block.insns)
            .filter_map(|one| one.what.as_ref())
            .flat_map(|what| what.dests.iter().chain(&what.sources))
            .filter_map(|place| match place {
                Loc::Mem(cell) => Some(cell),
                _ => None,
            })
            .collect();
        if !cells.iter().any(|cell| cell.base.is_some_and(|base| bases.contains(&base.value))) {
            continue;
        }
        indexes.extend(cells.iter().filter(|cell| cell.scale == 1).filter_map(|cell| cell.index.map(|index| index.value)));
    }
    spiller::foldable_indexes(body, &indexes)
}

/// How much of a spilled read disappears when it becomes a memory operand.
fn _fold_discount(one: &Insn, profile: &Profile) -> f64 {
    let Some(what) = &one.what else {
        return 0.0;
    };
    let (register, memory) = match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Binary | Operation::Compare, name, _) => {
            if !matches!(name, Some("add" | "sub" | "and" | "or" | "xor" | "cmp")) {
                return 0.0;
            }
            ("alu_rr", "alu_rm")
        }
        (Operation::Multiply, Some("imul"), [Loc::Held(first), Loc::Held(second)])
            if first.width == 4 && second.width == 4 =>
        {
            ("imul_r32", "imul_m32")
        }
        _ => return 0.0,
    };
    if ![register, memory, "mov_rm"].iter().all(|form| profile.prices(form)) {
        return 0.0;
    }
    let load = profile.cost("mov_rm").expect("priced above");
    if load <= 0 {
        return 0.0;
    }
    let remainder = 0.max(profile.cost(memory).expect("priced above") - profile.cost(register).expect("priced above"));
    _max(0.0, _min(1.0, 1.0 - remainder as f64 / load as f64))
}

/// Discount reads by the target-specific saving from folding them.
fn _fold_priced(body: &LirBody, live: IndexMap<u32, Interval>, profile: &Profile) -> IndexMap<u32, Interval> {
    let deep = ranges::depths(body);
    let mut free: IndexMap<u32, f64> = IndexMap::default();
    for block in &body.blocks {
        let each = ranges::level(deep.get(&block.at).copied().unwrap_or(0));
        for one in &block.insns {
            let discount = _fold_discount(one, profile);
            if discount == 0.0 {
                continue;
            }
            for value in &one.uses {
                if spiller::folded_source(one, &BTreeSet::from([*value])).is_some() {
                    *free.entry(*value).or_insert(0.0) += each * discount;
                }
            }
        }
    }
    live.into_iter()
        .map(|(value, one)| match free.get(&value) {
            Some(found) if one.weight != INF => {
                let weight = _max(0.0, one.weight - found / (one.size() + ranges::GRACE) as f64);
                (value, Interval { weight, ..one })
            }
            _ => (value, one),
        })
        .collect()
}

/// Python `format(value, "g")`.
fn _g(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let scientific = format!("{value:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific notation");
    let exponent: i32 = exponent.parse().expect("an exponent");
    let trimmed = |text: String| -> String {
        if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
    };
    if !(-4..6).contains(&exponent) {
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{}e{sign}{:02}", trimmed(mantissa.to_owned()), exponent.abs());
    }
    trimmed(format!("{value:.*}", (5 - exponent) as usize))
}

/// `body` with every operand naming a value replaced by its register.
pub fn applied(body: &LirBody, got: &Assignment) -> Result<LirBody, Error> {
    if !got.spilled.is_empty() {
        let worst: Vec<String> = got.spilled.iter().take(4).map(|one| format!("value#{one}")).collect();
        return Err(Spilled(format!(
            "{} values want a stack slot ({}) at a cost of {} and spilling them did not settle. Measured on \
             bools-q-evt: the same twelve every round, thirty-six instructions added each time. Their ranges \
             cross calls that clobber every register, so no register can hold them and the reload cannot either",
            got.spilled.len(),
            worst.join(", "),
            _g(got.cost),
        ))
        .into());
    }
    let body = _dead_insertions(body);
    let held = &got.r#where;
    let failed: RefCell<Option<Unplaced>> = RefCell::new(None);
    let blocks = body
        .blocks
        .iter()
        .map(|block| LirBlock {
            at: block.at,
            insns: lir::without(
                &block.insns,
                |one| _discardable_identity(one),
                Some(|one: &Arc<Insn>| match _placed_for_rewrite(one, held) {
                    Ok(placed) => placed,
                    Err(error) => {
                        failed.borrow_mut().get_or_insert(error);
                        Arc::clone(one)
                    }
                }),
            )
            .into_iter()
            .map(_identity_anchor)
            .collect(),
            succ: block.succ.clone(),
            phis: block.phis.clone(),
            cold: block.cold,
        })
        .collect::<Vec<_>>();
    if let Some(error) = failed.into_inner() {
        return Err(error.into());
    }
    Ok(LirBody { origin: body.origin.clone(), pins: body.pins.clone(), ordered: body.ordered, ..body.with_blocks(blocks) })
}

/// Delete unused allocator copies before physical identity loses their use graph.
fn _dead_insertions(body: &LirBody) -> LirBody {
    if body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .any(|one| one.what.as_ref().is_none_or(|what| what.op == Operation::Barrier))
    {
        return body.clone();
    }
    let mut body = body.clone();
    loop {
        let mut used: BTreeSet<u32> =
            body.blocks.iter().flat_map(|block| &block.insns).flat_map(|one| one.uses.iter().copied()).collect();
        used.extend(
            body.blocks.iter().flat_map(|block| &block.insns).flat_map(|one| one.requires.iter().map(|(held, _)| held.value)),
        );
        used.extend(
            body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)),
        );
        let mut dead: BTreeSet<usize> = BTreeSet::new();
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            if one.covers.is_none_or(|covers| covers.0 != covers.1)
                || !one.spread.is_empty()
                || !one.clobbers.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || one.symbol == Some(true)
            {
                continue;
            }
            let Some(what) = &one.what else {
                continue;
            };
            if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                if let ([Loc::Held(dest)], [source]) = (what.dests.as_slice(), what.sources.as_slice()) {
                    let value = dest.value;
                    if one.defines == [value]
                        && !used.contains(&value)
                        && (matches!(source, Loc::Held(_) | Loc::Imm(_))
                            || matches!(source, Loc::Mem(_)) && one.spill_reload)
                    {
                        dead.insert(ranges::key(one));
                    }
                }
            }
        }
        if dead.is_empty() {
            return body;
        }
        body = body.with_blocks(body
                .blocks
                .iter()
                .map(|block| block.with_insns(block.insns.iter().filter(|one| !dead.contains(&ranges::key(one))).cloned().collect()))
                .collect());
    }
}

/// Retain byte ownership without requiring an encodable register self-copy.
fn _identity_anchor(one: Arc<Insn>) -> Arc<Insn> {
    if one.group.is_some() || !_pointless(&one) {
        return one;
    }
    let mut made = (*one).clone();
    made.what = Some(Semantics { name: Some("nop".to_owned()), ..Semantics::new(Operation::Nothing) });
    Arc::new(made)
}

/// An identity that is not still owned by the parallel-copy scheduler.
fn _discardable_identity(one: &Insn) -> bool {
    one.group.is_none() && _pointless(one)
}

/// Place one instruction without discarding an inserted definition.
fn _placed_for_rewrite(one: &Arc<Insn>, held: &IndexMap<u32, Register>) -> Result<Arc<Insn>, Unplaced> {
    let placed = _placed(one, held)?;
    if placed.group.is_none() && _pointless(&placed) {
        return Ok(lir::anchor(placed));
    }
    Ok(placed)
}

/// Whether this instruction moves a register into itself.
fn _pointless(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    if what.op != Operation::Move {
        return false;
    }
    if what.dests.len() != 1 || what.sources.len() != 1 {
        return false;
    }
    matches!((&what.dests[0], &what.sources[0]), (Loc::Reg(into), Loc::Reg(out_of)) if into.register == out_of.register)
}

fn _placed(one: &Arc<Insn>, held: &IndexMap<u32, Register>) -> Result<Arc<Insn>, Unplaced> {
    let Some(what) = &one.what else {
        return Ok(Arc::clone(one));
    };
    let dests = what.dests.iter().map(|x| _settled(x, held)).collect::<Result<Vec<_>, _>>()?;
    let sources = what.sources.iter().map(|x| _settled(x, held)).collect::<Result<Vec<_>, _>>()?;
    let mut name = what.name.clone();
    if target::far_load(what) {
        if let Loc::Reg(selector) = &dests[1] {
            if let Some(spelt) = target::FAR_LOADS.get(&selector.register) {
                name = Some((*spelt).to_owned());
            }
        }
    }
    let mut made = (**one).clone();
    made.what = Some(Semantics { name, dests, sources, ..what.clone() });
    Ok(Arc::new(made))
}

/// One operand with its value resolved to the register holding it.
fn _settled(place: &Loc, held: &IndexMap<u32, Register>) -> Result<Loc, Unplaced> {
    let mut place = place.clone();
    if let Loc::Mem(cell) = &place {
        if let Some(selector) = cell.selector {
            let Some(register) = held.get(&selector.value) else {
                return Err(Unplaced(format!("selector value#{} has no register", selector.value)));
            };
            let addr = cell.addr.expect("a selected cell has an address");
            place = Loc::Mem(Mem { addr: Some(Addr { segment: *register, ..addr }), ..cell.clone() });
        }
    }
    if let Loc::Mem(cell) = &place {
        if let Some(index) = cell.index {
            let base = cell.base.and_then(|base| held.get(&base.value).copied());
            let index_register = held.get(&index.value).copied();
            let Some(index_register) = index_register.filter(|_| cell.base.is_none() || base.is_some()) else {
                return Err(Unplaced(format!("scaled cell {} has no register for its base or index", cell.repr())));
            };
            let mut base_register = match (base, cell.base) {
                (Some(base), Some(held_base)) => target::named(base, i64::from(held_base.width)),
                _ => cell.through,
            };
            let mut index_register = target::named(index_register, i64::from(index.width));
            if cell.base.is_some_and(|held_base| held_base.width == 2 && index.width == 2)
                && cell.scale == 1
                && target::WORD_INDEXES.contains(&base_register)
                && target::WORD_BASES.contains(&index_register)
            {
                std::mem::swap(&mut base_register, &mut index_register);
            }
            return Ok(Loc::Mem(Mem { through: base_register, index_through: index_register, ..cell.clone() }));
        }
        if let Some(base) = cell.base {
            let Some(register) = held.get(&base.value) else {
                return Ok(place.clone());
            };
            let placed = target::named(*register, 2);
            if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                return Ok(Loc::Mem(Mem { through: Register::BP, index_through: placed, ..cell.clone() }));
            }
            return Ok(Loc::Mem(Mem { through: placed, ..cell.clone() }));
        }
    }
    let Loc::Held(value) = &place else {
        return Ok(place);
    };
    let Some(register) = held.get(&value.value) else {
        return Err(Unplaced(format!("value#{} at width {} has no register", value.value, value.width)));
    };
    Ok(Loc::Reg(Reg { register: target::named(*register, i64::from(value.width)), width: value.width }))
}

#[cfg(test)]
mod tests {
    //! Ports of `tests/test_allocation.py`, `tests/test_allocation_hints.py`,
    //! `tests/test_interval_redefinitions.py`, and the allocator tests of
    //! `test_address_roles`, `test_lir`, `test_dead_call_deliveries`,
    //! `test_lir_verify`, `test_lower_arguments` and `test_pointer_memory`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::*;
    use crate::analysis::intervals::Segment;
    use crate::backend::{cpu, parcopy, select, verify};
    use crate::model::ir::Imm;

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(value: i64, width: u32) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn reg(register: Register, width: u32) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn block(at: i64, insns: Vec<Insn>) -> LirBlock {
        LirBlock::new(at, insns.into_iter().map(Arc::new).collect())
    }

    fn body_of(name: &str, entry: i64, insns: Vec<Insn>) -> LirBody {
        LirBody::new(name, entry, vec![block(entry, insns)], IndexMap::default(), IndexMap::default())
    }

    fn _one_block(insns: Vec<Insn>) -> LirBody {
        body_of("one", 0, insns)
    }

    fn _mov(into: u32, value: i64, at: i64) -> Insn {
        Insn::new(
            at,
            Some((at, at + 2)),
            Some(semantics(Operation::Move, "mov", vec![held(into, 2)], vec![imm(value, 2)])),
            vec![into],
            vec![],
        )
    }

    fn _shl(result: u32, count: u32, at: i64) -> Insn {
        let what =
            semantics(Operation::Binary, "shl", vec![held(result, 2)], vec![held(result, 2), held(count, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![result], vec![result, count])
    }

    fn _named(body: &LirBody, name: &str) -> Vec<Semantics> {
        body.insns()
            .iter()
            .filter_map(|one| one.what.clone())
            .filter(|what| what.name.as_deref() == Some(name) && !matches!(what.sources[0], Loc::Imm(_)))
            .collect()
    }

    /// Python's phases share one frame object; an owned copy lost every
    /// spill slot RegAlloc made, so the prologue reserved too little.
    #[test]
    fn test_spill_slots_land_in_the_shared_frame() {
        let mut insns = vec![];
        let mut at = 0x100;
        for value in 1..=8u32 {
            insns.push(_mov(value, i64::from(value), at));
            at += 2;
        }
        for value in 1..=8u32 {
            let what = semantics(Operation::Binary, "add", vec![held(value, 2)], vec![held(value, 2), held(value, 2)]);
            insns.push(Insn::new(at, Some((at, at + 2)), Some(what), vec![value], vec![value]));
            at += 2;
        }
        for value in 1..=8u32 {
            let what = semantics(Operation::Push, "push", vec![], vec![held(value, 2)]);
            insns.push(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![value]));
            at += 2;
        }
        let body = _one_block(insns);
        let frame = Rc::new(RefCell::new(frames::of(&body, None, "", None).expect("a frame")));
        let mut phase =
            RegAlloc::new(None, Some(Rc::clone(&frame)), ProfileOrName::Name("386")).expect("a cpu");
        RegAlloc::transform(&mut phase, body).expect("allocates");
        assert!(!frame.borrow().slots.is_empty());
    }

    fn _through_regalloc(body: LirBody, pinned: &[(u32, Register)]) -> LirBody {
        let pinned = pins(pinned);
        let frame = frames::of(&body, None, "", None).expect("a frame");
        let mut phase = RegAlloc::new(Some(&pinned), Some(Rc::new(RefCell::new(frame))), ProfileOrName::Name("386")).expect("a cpu");
        RegAlloc::transform(&mut phase, body).expect("allocates")
    }

    fn pins(pairs: &[(u32, Register)]) -> IndexMap<u32, Register> {
        pairs.iter().copied().collect()
    }

    fn values(items: &[u32]) -> BTreeSet<u32> {
        items.iter().copied().collect()
    }

    fn allocated(body: &LirBody, pinned: Option<&IndexMap<u32, Register>>) -> Result<Assignment, Error> {
        allocate(body, pinned, None, None, None, ProfileOrName::Name("386"))
    }

    fn registers(places: &[Loc]) -> Vec<Register> {
        places.iter().map(|x| if let Loc::Reg(one) = x { one.register } else { Register::None }).collect()
    }

    fn assignment(r#where: IndexMap<u32, Register>) -> Assignment {
        Assignment { r#where, spilled: BTreeSet::new(), cost: 0.0, optimal: true, why: String::new() }
    }

    fn cell_of(one: &Insn) -> Mem {
        match &one.what.as_ref().expect("semantics").sources[0] {
            Loc::Mem(cell) => cell.clone(),
            other => panic!("not a cell: {other:?}"),
        }
    }

    fn unplaced(got: Result<Assignment, Error>) -> String {
        match got {
            Err(Error::Unplaced(Unplaced(message))) => message,
            other => panic!("not Unplaced: {other:?}"),
        }
    }

    // ------------------------------------------------ tests/test_allocation.py

    #[test]
    fn test_a_widening_multiply_puts_its_halves_in_ax_and_dx() {
        let what =
            semantics(Operation::Multiply, "imul", vec![held(1, 2), held(2, 2)], vec![held(1, 2), held(3, 2)]);
        let imul = Insn::new(0x100, Some((0x100, 0x102)), Some(what), vec![1, 2], vec![1, 3]);
        let got = _through_regalloc(_one_block(vec![_mov(1, 3, 0xFC), _mov(3, 5, 0xFE), imul]), &[]);
        let made = _named(&got, "imul");
        assert_eq!(made.len(), 1);
        let made = &made[0];
        assert_eq!(ir::root(registers(&made.dests)[0]), Register::EAX, "{:?}", made.dests);
        assert_eq!(ir::root(registers(&made.dests)[1]), Register::EDX, "{:?}", made.dests);
        assert_eq!(made.sources[0], made.dests[0], "the tie was broken");
    }

    #[test]
    fn test_a_half_register_and_its_whole_are_the_same_register() {
        let mut insns: Vec<Insn> =
            [3, 4, 5].iter().zip(0..).map(|(one, index)| _mov(*one, i64::from(*one), 0xF0 + 2 * index)).collect();
        insns.push(Insn::new(
            0x100,
            Some((0x100, 0x104)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 4)], vec![imm(7, 4)])),
            vec![1],
            vec![],
        ));
        insns.push(_mov(2, 3, 0x104));
        insns.push(Insn::new(
            0x106,
            Some((0x106, 0x10A)),
            Some(semantics(Operation::Binary, "add", vec![held(1, 4)], vec![held(1, 4), held(2, 2)])),
            vec![1],
            vec![1, 2],
        ));
        insns.push(Insn::new(
            0x10A,
            Some((0x10A, 0x10C)),
            Some(semantics(Operation::Binary, "add", vec![held(3, 2)], vec![held(3, 2), held(4, 2)])),
            vec![3],
            vec![3, 4, 5],
        ));
        let got = allocated(&_one_block(insns), Some(&pins(&[(2, Register::DX)]))).expect("allocates");
        let (long, pinned) = (got.r#where.get(&1), got.r#where.get(&2));
        assert!(long.is_some() || pinned.is_some(), "{:?}", got.r#where);
        if let (Some(long), Some(pinned)) = (long, pinned) {
            assert_ne!(
                ir::root(*long),
                ir::root(*pinned),
                "the long was given {long:?} and the pinned value {pinned:?}, which are one register"
            );
        }
    }

    #[test]
    fn test_a_soft_preference_changes_free_register_order_without_becoming_a_pin() {
        let preferred = pins(&[(1, Register::EDX)]);
        let one = allocate(&_one_block(vec![_mov(1, 1, 0x100)]), None, None, None, Some(&preferred), "386".into())
            .expect("allocates");
        assert_eq!(one.r#where[&1], Register::EDX);
        let overlap = Insn::new(
            0x104,
            Some((0x104, 0x106)),
            Some(semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), held(2, 2)])),
            vec![1],
            vec![1, 2],
        );
        let kept = allocate(
            &_one_block(vec![_mov(2, 2, 0x100), _mov(1, 1, 0x102), overlap]),
            Some(&pins(&[(2, Register::EDX)])),
            None,
            None,
            Some(&preferred),
            "386".into(),
        )
        .expect("allocates");
        assert_ne!(kept.r#where[&1], Register::EDX);
    }

    #[test]
    fn test_an_explicit_soft_preference_beats_an_ordinary_copy_hint() {
        let source = _mov(2, 2, 0x100);
        let copy = Insn::new(
            0x102,
            Some((0x102, 0x104)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![held(2, 2)])),
            vec![1],
            vec![2],
        );
        let used = Insn::new(
            0x104,
            Some((0x104, 0x106)),
            Some(semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), imm(1, 2)])),
            vec![1],
            vec![1],
        );
        let got = allocate(
            &_one_block(vec![source, copy, used]),
            Some(&pins(&[(2, Register::EAX)])),
            None,
            None,
            Some(&pins(&[(1, Register::EDX)])),
            "386".into(),
        )
        .expect("allocates");
        assert_eq!(got.r#where[&1], Register::EDX, "{:?}", got.r#where);
    }

    #[test]
    fn test_folded_spill_cost_depends_on_the_selected_cpu() {
        let mut insns: Vec<Insn> =
            (1..8).map(|value| _mov(value, i64::from(value), 0x100 + 2 * (i64::from(value) - 1))).collect();
        insns.push(Insn::new(
            0x110,
            Some((0x110, 0x112)),
            Some(semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), held(7, 2)])),
            vec![1],
            vec![1, 7],
        ));
        let mut last = Insn::new(0x112, Some((0x112, 0x114)), None, vec![], (1..7).collect());
        last.widths = (1..7).map(|value| (value, 2)).collect();
        insns.push(last);
        let body = _one_block(insns);

        let on_386 = allocate(&body, None, None, None, None, "386".into()).expect("allocates");
        let on_core = allocate(&body, None, None, None, None, "Core".into()).expect("allocates");

        assert!(!on_386.spilled.contains(&7), "{on_386:?}");
        assert_eq!(on_core.spilled, values(&[7]), "{on_core:?}");
    }

    #[test]
    fn test_a_fixed_source_that_is_not_the_multiply_pair_is_honoured() {
        let got = _through_regalloc(_one_block(vec![_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104)]), &[]);
        let made = _named(&got, "shl");
        assert_eq!(made.len(), 1);
        assert_eq!(ir::root(registers(&made[0].sources)[1]), Register::ECX, "{:?}", made[0].sources);
    }

    #[test]
    fn test_a_value_required_in_two_registers_gets_one_fresh_value_per_site() {
        let cwd = Insn::new(
            0x106,
            Some((0x106, 0x107)),
            Some(semantics(Operation::Extend, "cwd", vec![held(8, 2)], vec![held(7, 2)])),
            vec![8],
            vec![7],
        );
        let got =
            _through_regalloc(_one_block(vec![_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104), cwd]), &[]);
        let shift = _named(&got, "shl");
        let extend = _named(&got, "cwd");
        assert_eq!((shift.len(), extend.len()), (1, 1));
        assert_eq!(ir::root(registers(&shift[0].sources)[1]), Register::ECX, "{:?}", shift[0].sources);
        assert_eq!(ir::root(registers(&extend[0].sources)[0]), Register::EAX, "{:?}", extend[0].sources);
    }

    #[test]
    fn test_an_origin_pin_does_not_override_what_the_instruction_requires() {
        let body = _one_block(vec![_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104)]);
        let got = _through_regalloc(body, &[(7, Register::EBX)]);
        let made = _named(&got, "shl");
        assert_eq!(made.len(), 1);
        assert_eq!(ir::root(registers(&made[0].sources)[1]), Register::ECX, "{:?}", made[0].sources);
    }

    fn _based_cell() -> Insn {
        let r#where = Addr { base: Register::SI, ..Addr::new(Space::Segment, 0x10) };
        let cell = Mem { disp_width: 2, base: Some(Held { value: 21, width: 2 }), ..Mem::new(Some(r#where), 2) };
        let what = semantics(Operation::Move, "mov", vec![held(30, 2)], vec![Loc::Mem(cell)]);
        Insn::new(0x100, Some((0x100, 0x104)), Some(what), vec![30], vec![21])
    }

    #[test]
    fn test_a_placed_cell_reaches_memory_by_the_register_its_value_got() {
        for register in [Register::EBX, Register::ESI] {
            let was = cell_of(&_based_cell());
            let got = _settled(&Loc::Mem(was.clone()), &pins(&[(21, register)])).expect("placed");
            let Loc::Mem(got) = got else { panic!("not a cell: {got:?}") };
            assert_eq!(got.through, target::named(register, 2), "{register:?}: {:?}", got.through);
            assert_eq!(got.base, Some(Held { value: 21, width: 2 }), "the cell stopped naming its value");
            assert_eq!(got.addr, was.addr, "addr");
            assert_eq!(got.width, was.width, "width");
            assert_eq!(got.offset, was.offset, "offset");
            assert_eq!(got.disp_width, was.disp_width, "disp_width");
        }
    }

    /// nbodys placed `fld [si]`'s base in AX, where the call left it: the
    /// allocator released the whole-range AX pin, constrain found it again in
    /// `body.pins`, and `[ax]`, which has no 16-bit encoding, was emitted.
    #[test]
    fn test_a_call_result_used_as_a_base_leaves_the_register_it_was_delivered_in() {
        let mut call =
            Insn::new(0xF0, Some((0xF0, 0xF3)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![21], vec![]);
        call.delivers = vec![(Held { value: 21, width: 2 }, Register::AX)];
        let mut body = _one_block(vec![call, _based_cell()]);
        body.pins = pins(&[(21, Register::EAX)]);
        let placed = _through_regalloc(body, &[(21, Register::EAX)]);
        let load = placed.insns().into_iter().find(|one| {
            one.at == 0x100 && matches!(one.what.as_ref().map(|what| &what.sources[0]), Some(Loc::Mem(_)))
        });
        let through = cell_of(&load.expect("the load")).through;
        assert!(target::ADDRESSING.contains(&through), "{through:?}");
    }

    #[test]
    fn test_a_based_cell_keeps_the_register_the_allocation_gave_its_base() {
        let r#where = Addr { base: Register::SI, ..Addr::new(Space::Literal, 0x2) };
        let cell =
            Mem { offset: 2, disp_width: 1, base: Some(Held { value: 17, width: 2 }), ..Mem::new(Some(r#where), 2) };
        let load = Insn::new(
            0xA2,
            Some((0xA2, 0xA4)),
            Some(semantics(Operation::Move, "mov", vec![reg(Register::AX, 2)], vec![Loc::Mem(cell)])),
            vec![],
            vec![17],
        );
        let got = applied(&_one_block(vec![load]), &assignment(pins(&[(17, Register::BX)]))).expect("applies");
        let read = cell_of(&got.blocks[0].insns[0]);
        assert_eq!(read.through, Register::BX, "the cell is still reached through {:?}", read.through);
        assert_eq!(read.base, Some(Held { value: 17, width: 2 }), "the cell stopped saying which value reached it");
        assert_eq!((read.addr, read.width, read.offset, read.disp_width), (Some(r#where), 2, 2, 1));
    }

    #[test]
    fn test_a_fixed_call_argument_reaches_its_register_through_the_whole_phase() {
        let made = Insn::new(
            0,
            Some((0, 3)),
            Some(semantics(
                Operation::Move,
                "mov",
                vec![held(2, 2)],
                vec![Loc::Mem(Mem::new(Some(Addr::new(Space::Segment, 0x10)), 2))],
            )),
            vec![2],
            vec![],
        );
        let mut call =
            Insn::new(3, Some((3, 8)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![], vec![2]);
        call.requires = vec![(Held { value: 2, width: 2 }, Register::CX)];
        let got = _through_regalloc(_one_block(vec![made, call]), &[(2, Register::SI)]);
        let insns = &got.blocks[0].insns;
        assert_eq!(insns.len(), 3, "the pre-call copy is missing: {} instructions", insns.len());
        let copy = insns[1].what.as_ref().expect("semantics");
        assert_eq!(copy.dests[0], reg(Register::CX, 2), "the fresh value went to {:?}", copy.dests[0]);
        assert_eq!(copy.sources[0], reg(Register::SI, 2), "the argument moved off si: {:?}", copy.sources[0]);
        assert_eq!(
            insns[0].what.as_ref().expect("semantics").dests[0],
            reg(Register::SI, 2),
            "the original was pinned away from si"
        );
    }

    #[test]
    fn test_a_call_result_nothing_reads_becomes_a_clobber() {
        let mut call = Insn::new(
            0x10,
            Some((0x10, 0x15)),
            Some(semantics(Operation::Call, "call", vec![], vec![])),
            vec![11, 12],
            vec![],
        );
        call.clobbers = BTreeSet::from([Register::ESI]);
        let read = Insn::new(
            0x15,
            Some((0x15, 0x17)),
            Some(semantics(Operation::Move, "mov", vec![held(20, 2)], vec![held(12, 2)])),
            vec![20],
            vec![12],
        );
        let (got, pins) = narrowed(&_one_block(vec![call, read]), &pins(&[(11, Register::EAX), (12, Register::ECX)]));
        let after = &got.blocks[0].insns[0];
        assert_eq!(after.defines, vec![12], "the dead result survived: {:?}", after.defines);
        assert!(!pins.contains_key(&11), "its pin survived: {pins:?}");
        assert_eq!(pins.get(&12), Some(&Register::ECX), "the read result lost its pin");
        assert!(
            after.clobbers.contains(&Register::EAX),
            "the register it destroyed was forgotten: {:?}",
            after.clobbers
        );
        assert!(after.clobbers.contains(&Register::ESI), "an existing clobber was dropped");
    }

    #[test]
    fn test_conflicting_hard_register_assignments_are_unplaceable_not_spills() {
        let body = _one_block(vec![_mov(1, 1, 0), _mov(2, 2, 2), _shl(1, 2, 4)]);
        let message = unplaced(allocated(&body, Some(&pins(&[(1, Register::DX), (2, Register::DX)]))));
        assert!(message.contains("value#2 cannot be placed"), "{message}");
    }

    #[test]
    fn test_a_hard_register_assignment_is_not_an_eviction_victim() {
        let kept = Interval { weight: 0.1, ..Interval::new(1, vec![Segment { start: 0, end: 4 }]) };
        let incoming = Interval { weight: 10.0, ..Interval::new(2, vec![Segment { start: 0, end: 4 }]) };
        let union: IndexMap<Register, Vec<u32>> = IndexMap::from_iter([(_whole(Register::DI), vec![1])]);
        let live: IndexMap<u32, Interval> = IndexMap::from_iter([(1, kept), (2, incoming.clone())]);
        let got =
            _evict(&incoming, &[Register::DI], &union, &live, &Vec::new(), &|_, _| false, &values(&[1]), 4, None, None);
        assert!(got.is_none(), "{got:?}");
    }

    /// Eviction only weighs the holders' spill costs, so a value whose register
    /// was held by something that could move, if a third value moved first,
    /// spilled (deedlines: 3600 weighted frame operands).
    #[test]
    fn test_a_register_whose_holder_moves_through_a_chain_is_taken_not_spilled() {
        let span = |value: u32, end: i64| (value, Interval { weight: 1.0, ..Interval::new(value, vec![Segment { start: 0, end }]) });
        let live: IndexMap<u32, Interval> = IndexMap::from_iter([span(1, 10), span(2, 4), span(3, 10)]);
        let choices = |value: u32| match value {
            1 => vec![Register::AX, Register::BX],
            2 => vec![Register::BX, Register::CX],
            _ => vec![Register::AX],
        };
        let mut union: IndexMap<Register, Vec<u32>> =
            IndexMap::from_iter([(_whole(Register::AX), vec![1]), (_whole(Register::BX), vec![2])]);
        let mut placed: IndexMap<u32, Register> = IndexMap::from_iter([(1, Register::AX), (2, Register::BX)]);
        let mut coloring = Coloring {
            union: &mut union,
            r#where: &mut placed,
            live: &live,
            masks: &Vec::new(),
            order: &choices,
            width: &|_| 2,
            fenced: &BTreeSet::new(),
            budget: Coloring::BUDGET,
            stack: Vec::new(),
        };
        assert!(coloring.recolor(3, 0, &mut BTreeSet::new()));
        assert_eq!(placed.get(&3), Some(&Register::AX));
        assert_eq!(placed.get(&1), Some(&Register::BX));
        assert_eq!(placed.get(&2), Some(&Register::CX));
    }

    /// A failed recoloring must leave every holder where it was.
    #[test]
    fn test_a_recoloring_that_fails_restores_every_holder() {
        let span = |value: u32, end: i64| (value, Interval { weight: 1.0, ..Interval::new(value, vec![Segment { start: 0, end }]) });
        let live: IndexMap<u32, Interval> = IndexMap::from_iter([span(1, 10), span(2, 4), span(3, 10)]);
        let choices = |value: u32| match value {
            1 => vec![Register::AX, Register::BX],
            2 => vec![Register::BX],
            _ => vec![Register::AX],
        };
        let mut union: IndexMap<Register, Vec<u32>> =
            IndexMap::from_iter([(_whole(Register::AX), vec![1]), (_whole(Register::BX), vec![2])]);
        let mut placed: IndexMap<u32, Register> = IndexMap::from_iter([(1, Register::AX), (2, Register::BX)]);
        let mut coloring = Coloring {
            union: &mut union,
            r#where: &mut placed,
            live: &live,
            masks: &Vec::new(),
            order: &choices,
            width: &|_| 2,
            fenced: &BTreeSet::new(),
            budget: Coloring::BUDGET,
            stack: Vec::new(),
        };
        assert!(!coloring.recolor(3, 0, &mut BTreeSet::new()));
        assert_eq!(placed, IndexMap::from_iter([(1, Register::AX), (2, Register::BX)]));
        assert_eq!(union[&_whole(Register::AX)], vec![1]);
        assert_eq!(union[&_whole(Register::BX)], vec![2]);
    }

    #[test]
    fn test_an_unspillable_range_without_a_register_is_unplaceable() {
        let mut insns: Vec<Insn> =
            (1..8).map(|value| _mov(value, i64::from(value), 2 * (i64::from(value) - 1))).collect();
        insns.push(Insn::new(
            14,
            Some((14, 16)),
            Some(semantics(Operation::Nothing, "", vec![], vec![])),
            vec![],
            (1..8).collect(),
        ));
        let pinned: IndexMap<u32, Register> = (1..7).zip(target::AVAILABLE).collect();
        let got = allocate(&_one_block(insns), Some(&pinned), Some(&values(&[7])), None, None, "386".into());
        let message = unplaced(got);
        assert!(message.contains("value#7 cannot be spilled"), "{message}");
    }

    // ----------------------------------------- tests/test_allocation_hints.py

    fn call_copy() -> LirBody {
        let define = Insn::new(
            0,
            Some((0, 3)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(42, 2)])),
            vec![1],
            vec![],
        );
        let copy = Insn::new(
            3,
            Some((3, 3)),
            Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
            vec![2],
            vec![1],
        );
        let used =
            Insn::new(3, Some((3, 4)), Some(semantics(Operation::Push, "push", vec![], vec![held(2, 2)])), vec![], vec![2]);
        LirBody { pins: pins(&[(2, Register::EDI)]), ..body_of("call-copy", 0, vec![define, copy, used]) }
    }

    /// Excess call copies grew SCREEN and contributed to E1M1's BASIC error 14.
    #[test]
    fn test_call_input_is_computed_in_its_available_required_register() {
        let body = call_copy();
        let assignment = allocated(&body, Some(&body.pins)).expect("allocates");
        let result = applied(&body, &assignment).expect("applies");
        assert!(assignment.spilled.is_empty());
        let mut emitted: Vec<u8> = Vec::new();
        for one in result.insns() {
            let what = one.what.as_ref().expect("semantics");
            let encoded = select::emit(what, 0, None, false, false, None).expect("encodes");
            emitted.extend(encoded.code);
        }
        assert_eq!(emitted, [0xBF, 0x2A, 0x00, 0x57]);
    }

    #[test]
    fn test_fixed_result_can_remain_in_its_return_register() {
        let body = call_copy();
        let got = allocated(&body, Some(&pins(&[(1, Register::EDI)]))).expect("allocates");
        assert!(got.spilled.is_empty());
        assert_eq!(got.r#where[&1], Register::EDI);
        assert_eq!(got.r#where[&2], Register::EDI);
    }

    #[test]
    fn test_copy_hint_cannot_override_a_register_requirement() {
        for barrier in ["overlap", "clobber", "class", "fixed"] {
            let mut body = call_copy();
            let insns: Vec<Insn> = body.insns().iter().map(|one| (**one).clone()).collect();
            let (define, copy, used) = (insns[0].clone(), insns[1].clone(), insns[2].clone());
            let mut pinned = body.pins.clone();
            match barrier {
                "overlap" => {
                    let what = Semantics { sources: vec![held(1, 2)], ..used.what.clone().expect("semantics") };
                    let extra = Insn { at: 4, covers: Some((4, 5)), what: Some(what), uses: vec![1], ..used.clone() };
                    body.blocks = vec![block(0, vec![define, copy, used, extra])];
                }
                "clobber" => {
                    let mut extra = Insn::new(2, Some((2, 2)), None, vec![], vec![]);
                    extra.clobbers = BTreeSet::from([Register::EDI]);
                    body.blocks = vec![block(0, vec![define, extra, copy, used])];
                }
                "class" => {
                    let byte_use = Insn::new(
                        2,
                        Some((2, 2)),
                        Some(semantics(Operation::Compare, "cmp", vec![], vec![held(1, 1), imm(0, 1)])),
                        vec![],
                        vec![1],
                    );
                    body.blocks = vec![block(0, vec![define, byte_use, copy, used])];
                }
                _ => {
                    pinned.insert(1, Register::ECX);
                }
            }
            let got = allocated(&body, Some(&pinned)).expect("allocates");
            assert!(got.spilled.is_empty(), "{barrier}");
            assert_ne!(got.r#where[&1], Register::EDI, "{barrier}");
            assert_eq!(got.r#where[&2], Register::EDI, "{barrier}");
        }
    }

    #[test]
    fn test_a_value_copied_on_to_a_fixed_register_is_seated_there_first() {
        // A loop sum two copies from the return's AX lost AX to a counter placed first; the exit then moved it.
        let mov = |at: i64, dest: u32, source: Loc| {
            let uses = if let Loc::Held(one) = &source { vec![one.value] } else { vec![] };
            Insn::new(at, Some((at, at + 1)), Some(semantics(Operation::Move, "mov", vec![held(dest, 2)], vec![source])), vec![dest], uses)
        };
        let push = |at: i64, value: u32| {
            Insn::new(at, Some((at, at + 1)), Some(semantics(Operation::Push, "push", vec![], vec![held(value, 2)])), vec![], vec![value])
        };
        let (counter, total, copied, returned) = (1, 2, 3, 4);
        let insns = vec![
            mov(0, counter, imm(9, 2)),
            mov(1, total, imm(0, 2)),
            push(2, total),
            push(3, counter),
            mov(4, copied, held(total, 2)),
            mov(5, returned, held(copied, 2)),
            push(6, returned),
        ];
        let pins = IndexMap::from_iter([(returned, Register::EAX)]);
        let body = LirBody::new("chain", 0, vec![block(0, insns)], IndexMap::default(), pins);
        let got = allocated(&body, Some(&body.pins)).expect("allocates");
        assert_eq!(got.r#where[&total], Register::EAX);
        assert_eq!(got.r#where[&copied], Register::EAX);
    }

    // ------------------------------------ tests/test_interval_redefinitions.py

    #[test]
    #[ignore = "fails in Python too: the surviving case raises Unplaced, not a spill"]
    fn test_call_redefinition_is_not_a_value_surviving_the_clobber() {
        let one = Loc::Held(Held { value: 1, width: 2 });
        let before = Insn::new(
            0,
            Some((0, 1)),
            Some(semantics(Operation::Move, "mov", vec![one.clone()], vec![imm(7, 2)])),
            vec![1],
            vec![],
        );
        let mut call =
            Insn::new(1, Some((1, 6)), Some(semantics(Operation::Call, "call", vec![], vec![one.clone()])), vec![1], vec![1]);
        call.clobbers = BTreeSet::from([Register::ESI]);
        let after =
            Insn::new(6, Some((6, 7)), Some(semantics(Operation::Push, "push", vec![], vec![one])), vec![], vec![1]);
        let body = body_of("call", 0, vec![before.clone(), call.clone(), after.clone()]);
        let si = pins(&[(1, Register::SI)]);
        let assigned = allocate(&body, Some(&si), Some(&values(&[1])), None, None, "386".into()).expect("allocates");
        assert!(assigned.spilled.is_empty() && assigned.r#where[&1] == Register::SI);

        let surviving = body_of("call", 0, vec![before, Insn { defines: vec![], ..call }, after]);
        assert_eq!(allocated(&surviving, Some(&si)).expect("allocates").spilled, values(&[1]));
    }

    // --------------------------------------------- tests/test_address_roles.py

    fn _instruction(at: i64, what: Semantics, defines: Vec<u32>, uses: Vec<u32>) -> Insn {
        Insn::new(at, Some((at, at)), Some(what), defines, uses)
    }

    fn _frame_load(at: i64, value: u32, displacement: i64) -> Insn {
        let cell = Mem { through: Register::BP, ..Mem::new(Some(Addr::new(Space::Frame, displacement)), 2) };
        _instruction(at, semantics(Operation::Move, "mov", vec![held(value, 2)], vec![Loc::Mem(cell)]), vec![value], vec![])
    }

    fn _load(at: i64, into: u32, cell: Mem, uses: Vec<u32>) -> Insn {
        _instruction(at, semantics(Operation::Move, "mov", vec![held(into, 2)], vec![Loc::Mem(cell)]), vec![into], uses)
    }

    /// QCport's far_put could not allocate two live pointer bases.
    #[test]
    fn test_shared_word_index_takes_bx_instead_of_spilling_a_base() {
        let index =
            _instruction(3, semantics(Operation::Move, "mov", vec![held(3, 2)], vec![imm(1, 2)]), vec![3], vec![]);
        let source = Mem {
            base: Some(Held { value: 1, width: 2 }),
            index: Some(Held { value: 3, width: 2 }),
            scale: 1,
            ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2)
        };
        let destination = Mem {
            base: Some(Held { value: 2, width: 2 }),
            index: Some(Held { value: 3, width: 2 }),
            scale: 1,
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), 2)
        };
        let read = _load(4, 4, source, vec![1, 3]);
        let write = _instruction(
            5,
            semantics(Operation::Move, "mov", vec![Loc::Mem(destination)], vec![held(4, 2)]),
            vec![],
            vec![2, 3, 4],
        );
        let body = body_of("shared-index", 0, vec![_frame_load(1, 1, 4), _frame_load(2, 2, 6), index, read, write]);

        let assignment = allocated(&body, None).expect("allocates");
        assert!(assignment.spilled.is_empty(), "{assignment:?}");
        let placed = applied(&body, &assignment).expect("applies");
        let accesses: Vec<Semantics> =
            placed.insns().iter().filter(|one| [4, 5].contains(&one.at)).filter_map(|one| one.what.clone()).collect();
        assert!(accesses.iter().all(|what| select::emit(what, 0, None, false, false, None).is_some()), "{accesses:?}");
    }

    #[test]
    fn test_word_address_role_keeps_a_call_crossing_base_out_of_bx() {
        let mut call = Insn::new(
            2,
            Some((2, 2)),
            Some(Semantics { name: Some("call".to_owned()), ..Semantics::new(Operation::Call) }),
            vec![],
            vec![],
        );
        call.clobbers = BTreeSet::from([Register::EAX, Register::EBX, Register::ECX, Register::EDX]);
        call.clobbers_high = BTreeSet::from([Register::ESI, Register::EDI]);
        let index =
            _instruction(3, semantics(Operation::Move, "mov", vec![held(2, 2)], vec![imm(1, 2)]), vec![2], vec![]);
        let cell = Mem {
            base: Some(Held { value: 1, width: 2 }),
            index: Some(Held { value: 2, width: 2 }),
            scale: 1,
            ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2)
        };
        let read = _load(4, 3, cell, vec![1, 2]);
        let body = body_of("call-crossing-base", 0, vec![_frame_load(1, 1, 4), call, index, read]);

        let found = classes(&body, &BTreeSet::new());

        assert_eq!(found[&1], *target::WORD_INDEXES);
        assert_eq!(found[&2], *target::WORD_BASES);
    }

    #[test]
    fn test_unallocatable_retention_plan_falls_back_to_ordinary_spilling() {
        let mut insns: Vec<Insn> =
            [1u32, 2, 3].iter().zip(1..).map(|(value, at)| _frame_load(at, *value, -2 * i64::from(*value))).collect();
        for value in [1u32, 2, 3] {
            let cell = Mem {
                index: Some(Held { value, width: 2 }),
                ..Mem::new(Some(Addr::new(Space::Literal, i64::from(value))), 2)
            };
            insns.push(_load(3 + i64::from(value), 10 + value, cell, vec![value]));
        }
        let body = body_of("retention-fallback", 0, insns);

        let (result, retained) =
            _assigned_plan(&body, &IndexMap::default(), &values(&[2]), &values(&[1, 3]), cpu::profile("386").expect("386"))
                .expect("allocates");

        assert_eq!(retained, BTreeSet::new());
        assert!(!result.spilled.contains(&2));
    }

    #[test]
    fn test_repeated_acyclic_stable_address_base_is_a_retention_candidate() {
        let mut insns = vec![_frame_load(1, 1, 6)];
        for at in [2u32, 3] {
            let cell = Mem {
                base: Some(Held { value: 1, width: 2 }),
                ..Mem::new(Some(Addr::new(Space::Far, i64::from(at) * 2)), 2)
            };
            insns.push(_load(i64::from(at), at, cell, vec![1]));
        }
        let body = body_of("acyclic-stable-base", 0, insns);

        assert_eq!(_retainable_bases(&body, &values(&[1])), values(&[1]));
    }

    #[test]
    fn test_retained_owner_influences_commutative_address_roles_before_allocation() {
        let cell = Mem {
            base: Some(Held { value: 1, width: 2 }),
            index: Some(Held { value: 2, width: 2 }),
            ..Mem::new(Some(Addr::new(Space::Far, 0)), 2)
        };
        let read = _load(3, 3, cell, vec![1, 2]);
        let body = body_of("retained-address-role", 0, vec![_frame_load(1, 1, 6), _frame_load(2, 2, 8), read]);

        let confined = classes(&body, &values(&[1]));

        assert_eq!(confined[&1], *target::WORD_INDEXES);
        assert_eq!(confined[&2], *target::WORD_BASES);
    }

    #[test]
    fn test_32_bit_secondary_base_is_not_confined_to_16_bit_address_registers() {
        let cell = Mem { base: Some(Held { value: 1, width: 4 }), ..Mem::new(Some(Addr::new(Space::Far, 0)), 2) };
        let body = body_of("secondary-base-class", 0, vec![_load(2, 2, cell, vec![1])]);

        assert!(!classes(&body, &BTreeSet::new()).contains_key(&1));
    }

    // ------------------------------------------------------ tests/test_lir.py

    // Python's cell has the string address "[bx]", which Rust cannot hold.
    fn _celled(base: Option<Held>, through: Register) -> LirBody {
        let at = 0x100;
        let cell = Mem { through, base, ..Mem::new(None, 2) };
        let what = semantics(Operation::Move, "mov", vec![held(30, 2)], vec![Loc::Mem(cell)]);
        let uses = base.map(|one| vec![one.value]).unwrap_or_default();
        _one_block(vec![Insn::new(at, Some((at, at + 2)), Some(what), vec![30], uses)])
    }

    #[test]
    fn test_an_address_value_takes_the_class_a_base_register_must_be_in() {
        for through in [Register::BX, Register::SI] {
            let got = classes(&_celled(Some(Held { value: 21, width: 2 }), through), &BTreeSet::new());
            assert_eq!(got.get(&21), Some(&*target::ADDRESSING), "through={through:?}: {:?}", got.get(&21));
        }
    }

    #[test]
    fn test_an_unbased_cell_confines_no_value() {
        assert!(!classes(&_celled(None, Register::BX), &BTreeSet::new()).contains_key(&21));
    }

    // ------------------------------------- tests/test_dead_call_deliveries.py

    #[test]
    fn test_dead_call_result_does_not_create_an_undefined_copy() {
        let mut call =
            Insn::new(0, Some((0, 3)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![1], vec![]);
        call.delivers = vec![(Held { value: 1, width: 2 }, Register::AX)];
        let (narrow, pinned) = narrowed(&body_of("dead result", 0, vec![call]), &pins(&[(1, Register::EAX)]));
        let (lowered, _fixed) = constrain::constrained(&narrow, Some(&pinned)).expect("constrains");
        let insns = lowered.insns();
        assert_eq!(insns.len(), 1);
        assert!(insns[0].defines.is_empty());
        assert!(insns[0].delivers.is_empty());
        assert!(insns[0].clobbers.contains(&Register::EAX));
    }

    // --------------------------------------------- tests/test_lir_verify.py

    #[test]
    fn test_register_allocation_keeps_the_definition_of_an_elided_identity() {
        for covers in [None, Some((1, 1)), Some((1, 3))] {
            let mut identity = Insn::new(
                1,
                covers,
                Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
                vec![2],
                vec![1],
            );
            identity.symbol = Some(covers == Some((1, 3)));
            let mut returning =
                Insn::new(2, None, Some(semantics(Operation::Return, "", vec![], vec![])), vec![], vec![2]);
            returning.requires = vec![(Held { value: 2, width: 2 }, Register::AX)];
            let body = LirBody { inputs: values(&[1]), ..body_of("fixed-identity", 1, vec![identity, returning]) };

            let placed = applied(&body, &assignment(pins(&[(1, Register::AX), (2, Register::AX)]))).expect("applies");

            let wrong = verify::verify(&placed, false);
            assert!(wrong.is_empty(), "{covers:?}: {wrong:?}");
            let first = &placed.insns()[0];
            assert_eq!(first.what.as_ref().expect("semantics").op, Operation::Nothing, "{covers:?}");
            assert_ne!(first.symbol, Some(true), "{covers:?}");
        }
    }

    /// C sieve read value#126 after its identity phi copy disappeared.
    #[test]
    fn test_parallel_copy_identity_keeps_its_virtual_definition() {
        let identity = Insn {
            group: Some(7),
            ..Insn::new(
                1,
                Some((1, 1)),
                Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
                vec![2],
                vec![1],
            )
        };
        let consumed =
            Insn::new(2, Some((2, 2)), Some(semantics(Operation::Push, "push", vec![], vec![held(2, 2)])), vec![], vec![2]);
        let body = LirBody { inputs: values(&[1]), ..body_of("parallel-identity", 1, vec![identity, consumed]) };
        let assignment = assignment(pins(&[(1, Register::AX), (2, Register::AX)]));

        let placed = applied(&body, &assignment).expect("applies");
        let scheduled = parcopy::scheduled(&placed).expect("schedules");

        assert!(verify::verify(&placed, false).is_empty(), "{:?}", verify::verify(&placed, false));
        assert!(verify::verify(&scheduled, false).is_empty(), "{:?}", verify::verify(&scheduled, false));
    }

    // ------------------------------------------ tests/test_lower_arguments.py

    #[test]
    fn test_dead_inserted_copy_chain_does_not_erase_observable_values() {
        for keep in ["none", "read", "original", "source_load", "opaque"] {
            let slot = Frame::new(0).cell(1u32, 2).expect("a slot");
            let mut load = Insn::new(
                0,
                Some((0, 0)),
                Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(slot)])),
                vec![1],
                vec![],
            );
            load.spill_reload = keep != "source_load";
            let copy = Insn::new(
                1,
                Some(if keep == "original" { (1, 2) } else { (1, 1) }),
                Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
                vec![2],
                vec![1],
            );
            let mut insns = vec![load.clone(), copy];
            if keep == "read" || keep == "opaque" {
                insns.push(Insn::new(2, Some((2, 3)), None, vec![], if keep == "read" { vec![2] } else { vec![] }));
            }
            let result = _dead_insertions(&body_of("dead", 0, insns.clone()));
            let expected: Vec<Insn> = match keep {
                "none" => vec![],
                "source_load" => vec![load],
                _ => insns,
            };
            let got: Vec<Insn> = result.insns().iter().map(|one| (**one).clone()).collect();
            assert_eq!(got, expected, "{keep}");
        }
    }

    // ------------------------------------------ tests/test_pointer_memory.py

    #[test]
    fn test_generated_byte_value_cannot_be_allocated_to_edi() {
        let what = semantics(Operation::Move, "mov", vec![held(1, 1)], vec![imm(12, 1)]);
        let body = body_of("byte", 0, vec![Insn::new(0, Some((0, 0)), Some(what), vec![1], vec![])]);
        assert_eq!(
            classes(&body, &BTreeSet::new())[&1],
            BTreeSet::from([Register::AX, Register::BX, Register::CX, Register::DX])
        );
    }
}
