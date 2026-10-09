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

use crate::analysis::frequency::Frequency;
use crate::analysis::intervals::{self as ranges, Indexes, Interval};
use crate::analysis::loops;
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::{self as frames, Frame, Refused};
use crate::backend::liveunion::{LiveUnion, Overlaps};
use crate::backend::target::{self, Segments};
use crate::backend::{constrain, datagroup, spiller, spillplacement, splitkit};
use crate::model::ir::{self, Addr, Held, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::hash::{IndexMap, IndexSet};
use crate::support::pyrepr::Repr;

/// Most queue visits an allocation may take: more means it does not converge, a
/// bug.
pub const BUDGET: usize = 200_000;

/// A value has no register, or no register it may take is free of values that
/// cannot be spilled: the body asks for more registers at one point than the
/// machine has, or names a register no value can be in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unplaced(pub String);

/// A value the allocator chose to spill, and nothing writes the spill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spilled(pub String);

impl fmt::Display for Unplaced {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for Spilled {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
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
    Refused(Refused),
    /// `ValueError`, or a call into a module not yet ported.
    Value(String),
}

impl fmt::Display for Error {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Unplaced(one) => one.fmt(formatter),
            Self::Spilled(one) => one.fmt(formatter),
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
pub fn _group_start(
    block: &LirBlock,
    index: usize,
) -> usize {
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
///
/// Dense: the values numbered by their order, each block's sets one row of bits
/// in one array, the fixed point a worklist over the rows. No set is built but
/// the answer.
pub fn live(body: &LirBody) -> (Live, Live) {
    let dense = live_rows(body);
    (dense.sets(&dense.into), dense.sets(&dense.out))
}

/// What is live at each block's entry and exit as the fixed point leaves it:
/// rows of bits, which a reader that only walks the values need not turn into
/// sets.
pub struct LiveRows {
    numbered: Vec<u32>,
    words: usize,
    position: IndexMap<i64, usize>,
    blocks: Vec<i64>,
    into: Vec<u64>,
    out: Vec<u64>,
}

impl LiveRows {
    /// How many values the rows number.
    pub fn numbered(&self) -> usize {
        self.numbered.len()
    }

    /// The values live at the entry of the block at `at`, in order.
    pub fn entering(
        &self,
        at: i64,
    ) -> impl Iterator<Item = u32> + '_ {
        self.values(&self.into, self.position[&at])
    }

    /// The values live at the exit of the block at `at`, in order.
    pub fn leaving(
        &self,
        at: i64,
    ) -> impl Iterator<Item = u32> + '_ {
        self.values(&self.out, self.position[&at])
    }

    fn holds(
        &self,
        rows: &[u64],
        block: i64,
        value: u32,
    ) -> bool {
        let (Some(&at), Ok(bit)) = (self.position.get(&block), self.numbered.binary_search(&value)) else {
            return false;
        };
        rows[at * self.words + bit / 64] >> (bit % 64) & 1 == 1
    }

    fn values<'a>(
        &'a self,
        rows: &'a [u64],
        block: usize,
    ) -> impl Iterator<Item = u32> + 'a {
        rows[block * self.words..(block + 1) * self.words].iter().enumerate().flat_map(move |(word, bits)| {
            (0..64).filter(move |bit| bits >> bit & 1 == 1).map(move |bit| self.numbered[word * 64 + bit])
        })
    }

    fn sets(
        &self,
        rows: &[u64],
    ) -> Live {
        self.blocks.iter().enumerate().map(|(at, block)| (*block, self.values(rows, at).collect())).collect()
    }
}

/// How many times this body's facts have walked every instruction for liveness
/// rows (`live_rows_by`), for a test that a web of a few values does not.
pub fn live_rows_walks(body: &LirBody) -> usize {
    body.facts.0.counted("live-rows-walks")
}

/// What is live at each block's entry and exit for a few values, found from
/// where they occur: a value is live into a block that reads it before writing
/// it and out of a block a successor has it live into, up the predecessors
/// until one writes it. The rows of `values` as `live_rows_by` finds them in a
/// body with no phis, at the cost of the occurrences and the blocks each is
/// live in, not of every instruction of the body.
pub struct WebRows {
    graph: Arc<crate::analysis::graph::Graph>,
    into: Vec<Vec<u32>>,
    out: Vec<Vec<u32>>,
    numbered: usize,
}

impl LiveAt for WebRows {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.graph.position.get(&block).is_some_and(|at| self.into[*at].binary_search(&value).is_ok())
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.graph.position.get(&block).is_some_and(|at| self.out[*at].binary_search(&value).is_ok())
    }
}

impl WebRows {
    pub fn numbered(&self) -> usize {
        self.numbered
    }

    pub fn entering(
        &self,
        at: i64,
    ) -> impl Iterator<Item = u32> + '_ {
        self.into[self.graph.position[&at]].iter().copied()
    }

    pub fn leaving(
        &self,
        at: i64,
    ) -> impl Iterator<Item = u32> + '_ {
        self.out[self.graph.position[&at]].iter().copied()
    }
}

/// `WebRows` of `values` (ascending); `places[value]` are the instructions that
/// name it, by block then position, once each, and whether each defines and
/// reads it.
pub fn live_rows_among(
    body: &LirBody,
    values: &[u32],
    places: &IndexMap<u32, Vec<ranges::Occurrence>>,
) -> WebRows {
    let count = body.blocks.len();
    let graph = crate::analysis::graph::Graph::of(body);
    let predecessors = &graph.parents;
    let (mut in_mark, mut out_mark, mut written_mark, mut run_mark) =
        (vec![0u32; count], vec![0u32; count], vec![0u32; count], vec![0u32; count]);
    // The first run of a block that names the value, as (defined, read): a read
    // decides, a write beside it too.
    let mut first_run: Vec<(usize, bool, bool)> = vec![(0, false, false); count];
    let mut into: Vec<Vec<u32>> = vec![Vec::new(); count];
    let mut out: Vec<Vec<u32>> = vec![Vec::new(); count];
    let mut turn = 0u32;
    let mut numbered = 0;
    for &value in values {
        let Some(found) = places.get(&value).filter(|found| !found.is_empty()) else { continue };
        numbered += 1;
        turn += 1;
        let mut reached: Vec<usize> = Vec::new();
        let mut work: Vec<usize> = Vec::new();
        for &((block_index, at), defined, used) in found {
            let end = ranges::_group_end(&body.blocks[block_index], at);
            if run_mark[block_index] != turn {
                run_mark[block_index] = turn;
                first_run[block_index] = (end, defined, used);
                reached.push(block_index);
            } else if first_run[block_index].0 == end {
                first_run[block_index].1 |= defined;
                first_run[block_index].2 |= used;
            }
            if defined {
                written_mark[block_index] = turn;
            }
        }
        for &block_index in &reached {
            if first_run[block_index].2 {
                in_mark[block_index] = turn;
                work.push(block_index);
            }
        }
        let mut live_out: Vec<usize> = Vec::new();
        while let Some(block_index) = work.pop() {
            for &before in &predecessors[block_index] {
                if out_mark[before] != turn {
                    out_mark[before] = turn;
                    live_out.push(before);
                    if written_mark[before] != turn && in_mark[before] != turn {
                        in_mark[before] = turn;
                        reached.push(before);
                        work.push(before);
                    }
                }
            }
        }
        for &block_index in &reached {
            if in_mark[block_index] == turn {
                into[block_index].push(value);
            }
        }
        for &block_index in &live_out {
            out[block_index].push(value);
        }
    }
    WebRows { graph, into, out, numbered }
}

/// Whether a value is live at the entry or exit of a block: what a caller that
/// asks of one value at a time reads, whether it has the sets of every block
/// (`Live`) or the rows.
pub trait LiveAt {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool;
    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool;
}

impl LiveAt for LiveRows {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.holds(&self.into, block, value)
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.holds(&self.out, block, value)
    }
}

impl LiveAt for (&Live, &Live) {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.0.get(&block).is_some_and(|live| live.contains(&value))
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.1.get(&block).is_some_and(|live| live.contains(&value))
    }
}

/// `live`, as rows.
pub fn live_rows(body: &LirBody) -> LiveRows {
    live_rows_by(body, |_| true)
}

/// `live_rows` of the values `keep` says only: each is live where it is as in
/// the whole, the others are not numbered, so a caller that asks of a few
/// values pays for rows of those.
pub fn live_rows_by(
    body: &LirBody,
    keep: impl Fn(u32) -> bool,
) -> LiveRows {
    body.facts.0.bump("live-rows-walks");
    // Every value the body names, numbered by order. Ids can be far apart, so
    // the number of a value is found by a table over the ids where they are
    // dense enough, else by search.
    let mut numbered: Vec<u32> = Vec::new();
    for block in &body.blocks {
        numbered.extend(
            block
                .phis
                .iter()
                .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)))
                .filter(|value| keep(*value)),
        );
        for one in &block.insns {
            numbered.extend(one.defines.iter().chain(&one.uses).copied().filter(|value| keep(*value)));
        }
    }
    numbered.sort_unstable();
    numbered.dedup();
    let largest = numbered.last().copied().unwrap_or(0) as usize;
    let table: Option<Vec<u32>> = (largest <= 8 * numbered.len() + 64).then(|| {
        let mut table = vec![u32::MAX; largest + 1];
        for (at, value) in numbered.iter().enumerate() {
            table[*value as usize] = at as u32;
        }
        table
    });
    let number = |value: u32| -> Option<usize> {
        if !keep(value) {
            return None;
        }
        Some(match &table {
            Some(table) => table[value as usize] as usize,
            None => numbered.binary_search(&value).expect("every value is numbered"),
        })
    };
    let words = numbered.len() / 64 + 1;
    let count = body.blocks.len();
    let set = |row: &mut [u64], value: u32| {
        if let Some(at) = number(value) {
            row[at / 64] |= 1 << (at % 64);
        }
    };
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    // Rows of `words` each, a block's at `block * words`.
    let mut defined = vec![0u64; count * words];
    let mut generated = vec![0u64; count * words];
    // A phi's argument is read at the end of the predecessor it comes from.
    let mut handed = vec![0u64; count * words];
    for block in &body.blocks {
        for phi in &block.phis {
            for (from, value) in &phi.incoming {
                if let Some(&at) = position.get(from) {
                    set(&mut handed[at * words..(at + 1) * words], *value);
                } else {
                    // A predecessor the body does not hold: its row is none, as
                    // its set was dropped.
                }
            }
        }
    }
    for (at, block) in body.blocks.iter().enumerate() {
        let row = at * words..(at + 1) * words;
        for phi in &block.phis {
            set(&mut defined[row.clone()], phi.result);
        }
        for one in &block.insns {
            for value in &one.defines {
                set(&mut defined[row.clone()], *value);
            }
        }
        let mut alive: Vec<u64> = handed[row.clone()].to_vec();
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let first = _group_start(block, index as usize);
            let group = &block.insns[first..=index as usize];
            for item in group {
                for value in &item.defines {
                    if let Some(at) = number(*value) {
                        alive[at / 64] &= !(1 << (at % 64));
                    }
                }
            }
            for item in group {
                for value in &item.uses {
                    set(&mut alive, *value);
                }
            }
            index = first as i64 - 1;
        }
        for phi in &block.phis {
            if let Some(at) = number(phi.result) {
                alive[at / 64] &= !(1 << (at % 64));
            }
        }
        generated[row].copy_from_slice(&alive);
    }
    let mut predecessors = vec![Vec::new(); count];
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (at, block) in body.blocks.iter().enumerate() {
        for successor in &block.succ {
            if let Some(&to) = position.get(successor) {
                predecessors[to].push(at);
                successors[at].push(to);
            }
        }
    }
    // The least fixed point of a backward problem: a worklist over the rows.
    let mut into = generated.clone();
    let mut out = vec![0u64; count * words];
    let mut pending: Vec<usize> = (0..count).collect();
    let mut queued = vec![true; count];
    let mut now = vec![0u64; words];
    while let Some(at) = pending.pop() {
        queued[at] = false;
        now.iter_mut().for_each(|word| *word = 0);
        for &from in &successors[at] {
            for (word, bits) in now.iter_mut().zip(&into[from * words..(from + 1) * words]) {
                *word |= bits;
            }
        }
        out[at * words..(at + 1) * words].copy_from_slice(&now);
        let mut changed = false;
        for word in 0..words {
            let entering = generated[at * words + word] | (now[word] & !defined[at * words + word]);
            if entering != into[at * words + word] {
                into[at * words + word] = entering;
                changed = true;
            }
        }
        if changed {
            for &pred in &predecessors[at] {
                if !queued[pred] {
                    queued[pred] = true;
                    pending.push(pred);
                }
            }
        }
    }
    // A phi's argument is read at the end of its predecessor, leaving it.
    for (at, row) in out.chunks_mut(words).enumerate() {
        for (word, bits) in row.iter_mut().zip(&handed[at * words..(at + 1) * words]) {
            *word |= bits;
        }
    }
    LiveRows { numbered, words, position, blocks: body.blocks.iter().map(|block| block.at).collect(), into, out }
}

/// `live` as it was written over sorted sets, which the tests hold the dense
/// one to.
#[cfg(test)]
pub fn live_reference(body: &LirBody) -> (Live, Live) {
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
    // A phi's argument is read at the end of the predecessor it comes from.
    let mut handed: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            for (from, value) in &phi.incoming {
                handed.entry(*from).or_default().insert(*value);
            }
        }
    }
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> = handed.get(&block.at).cloned().unwrap_or_default();
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

    // The least fixed point of a backward problem, found by a worklist over
    // dense bit sets: the round-robin over sorted sets it replaces reached
    // the same sets. Values numbered densely: ids can be far apart.
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
    let kept: Vec<Vec<u64>> =
        body.blocks.iter().map(|block| bits(&defines[&block.at]).iter().map(|word| !word).collect()).collect();
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
            .flat_map(|(word, bits)| {
                (0..64).filter(move |bit| bits >> bit & 1 == 1).map(move |bit| numbered[word * 64 + bit])
            })
            .collect()
    };
    let live_in: Live = body.blocks.iter().zip(&into).map(|(block, set)| (block.at, values(set))).collect();
    let live_out: Live = body
        .blocks
        .iter()
        .zip(&out)
        .map(|(block, set)| {
            let mut leaving = values(set);
            leaving.extend(handed.get(&block.at).into_iter().flatten().copied());
            (block.at, leaving)
        })
        .collect();
    (live_in, live_out)
}

/// A call's results that nothing reads, said as clobbers instead.
pub fn narrowed(
    body: &LirBody,
    pinned: &IndexMap<u32, Register>,
) -> (LirBody, IndexMap<u32, Register>) {
    let mut read: BTreeSet<u32> =
        body.blocks.iter().flat_map(|block| block.insns.iter().flat_map(|one| one.uses.iter().copied())).collect();
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
            made.delivers =
                one.delivers.iter().copied().filter(|(held, _register)| !gone.contains_key(&held.value)).collect();
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

/// How far a range has got, and therefore what may still be tried on it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Stage {
    Assign = 0,
    Split = 1,
    Spill = 2,
    Done = 3,
}

use crate::backend::classes::RegisterClasses;
use crate::backend::regclass::_SEGMENT_OPERANDS;
use crate::backend::regclass::{Classes, classes};

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
pub fn explicit_selectors(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
    segments: &Segments,
    registers: &RegisterClasses,
) -> LirBody {
    let confined = classes(body, &BTreeSet::new(), segments, registers);
    let selectors: BTreeSet<Register> = segments.selectors.iter().copied().collect();
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
                            || !target::SEGMENTS.contains(pinned.get(&selector.value).unwrap_or(&Register::ES))
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
                return Loc::Mem(Mem {
                    selector: None,
                    addr: Some(Addr { segment: Register::ES, ..addr }),
                    ..cell.clone()
                });
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
            let requires: IndexSet<(Held, Register)> =
                one.requires.iter().copied().chain(named.iter().map(|held| (*held, Register::ES))).collect();
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
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Queued {}

impl PartialOrd for Queued {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Queued {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
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
fn _wanted(
    hints: &IndexMap<u32, Vec<u32>>,
    fixed: &IndexMap<u32, Register>,
) -> IndexMap<u32, Register> {
    let mut wanted: IndexMap<u32, Register> =
        fixed.iter().map(|(value, register)| (*value, _whole(*register))).collect();
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

/// The registers wanted by the values that want one and have none yet, where
/// they are live: found by looking at every such value until enough is asked
/// since the intervals last moved, then by a tree over where they are live.
#[derive(Default)]
struct Claims {
    tree: Option<Overlaps>,
    asked: u32,
}

impl Claims {
    /// The registers `wanted` for the values live where `mine` is, other than
    /// `value` and those placed.
    fn by(
        &mut self,
        wanted: &IndexMap<u32, Register>,
        live: &IndexMap<u32, Interval>,
        value: u32,
        mine: &Interval,
        placed: &IndexMap<u32, Register>,
    ) -> BTreeSet<Register> {
        if self.tree.is_none() {
            self.asked += 1;
            if self.asked < 16 || wanted.len() < 64 {
                return wanted
                    .iter()
                    .filter(|(other, _)| {
                        **other != value
                            && !placed.contains_key(*other)
                            && live.get(*other).is_some_and(|theirs| theirs.overlaps(mine))
                    })
                    .map(|(_, register)| *register)
                    .collect();
            }
            self.tree = Some(Overlaps::new(
                wanted
                    .keys()
                    .filter_map(|value| live.get(value).map(|found| (*value, found)))
                    .flat_map(|(value, found)| {
                        found.segments.iter().map(move |segment| (segment.start, segment.end, value))
                    })
                    .collect(),
            ));
        }
        let mut claimed = BTreeSet::new();
        for segment in &mine.segments {
            self.tree
                .as_ref()
                .expect("built")
                .meeting(
                    segment.start,
                    segment.end,
                    &mut |other| {
                        if other != value && !placed.contains_key(&other) {
                            claimed.insert(wanted[&other]);
                        }
                    },
                );
        }
        claimed
    }
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
    segments: &Segments,
    classes: &RegisterClasses,
) -> Result<Assignment, Error> {
    Ok(_allocated(body, pinned, unspillable, protected, preferred, cpu, segments, classes, None)?.0)
}

/// `allocate`, splitting and spilling as it goes, as LLVM's greedy allocator
/// does: a split is carved and a spill rewritten into the body the moment it
/// is chosen, and the pieces and reloads join the queue. The body returned
/// holds every value in a register; the set is what went to the stack.
pub fn rewritten(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
    unspillable: Option<&BTreeSet<u32>>,
    protected: Option<&BTreeSet<u32>>,
    cpu: ProfileOrName<'_>,
    segments: &Segments,
    classes: &RegisterClasses,
    frame: &mut Frame,
    splitting: bool,
) -> Result<(Assignment, LirBody, BTreeSet<u32>), Error> {
    _allocated(body, pinned, unspillable, protected, None, cpu, segments, classes, Some((frame, splitting)))
}

/// What the allocator knows of a body, recomputed whenever it rewrites it.
struct Facts {
    index: Indexes,
    live: IndexMap<u32, Interval>,
    masks: Masks,
    widths: IndexMap<u32, u32>,
    confined: Classes,
    hints: IndexMap<u32, Vec<u32>>,
}

impl Facts {
    fn of(
        body: &LirBody,
        profile: &Profile,
        segments: &Segments,
        registers: &RegisterClasses,
        unspillable: &BTreeSet<u32>,
        protected: &BTreeSet<u32>,
        busy: &Frequency,
    ) -> Self {
        let _span = llrm_support::debug::span("regalloc facts");
        let index = llrm_support::debug::timed("facts slots", || ranges::indexed(body));
        let base = llrm_support::debug::timed("facts intervals", || ranges::intervals_over(body, Some(&index), busy));
        let base = llrm_support::debug::timed("facts sibling prices", || _sibling_priced(body, base, busy));
        let mut live = llrm_support::debug::timed("facts fold prices", || _fold_priced(body, base, profile, busy));
        // A spiller product lives for one use: a spill gains nothing.
        for one in unspillable {
            if let Some(interval) = live.get_mut(one) {
                interval.weight = INF;
            }
        }
        for one in protected {
            if let Some(interval) = live.get_mut(one) {
                interval.weight = INF;
            }
        }
        let masks = llrm_support::debug::timed("facts masks", || _masks(body, &index, segments));
        let widths = llrm_support::debug::timed("facts widths", || _widest(body));
        let confined = llrm_support::debug::timed("facts classes", || {
            let given = crate::backend::regclass::Found { live: &live, masks: &masks };
            let found = crate::backend::regclass::classes_given(body, protected, segments, registers, &given);
            if llrm_support::env_set("LLRM_CHECK_CLASSES") {
                assert!(
                    found.iter().eq(classes(body, protected, segments, registers).iter()),
                    "{}: classes from the given intervals differ from working them out",
                    body.name
                );
            }
            found
        });
        let hints = llrm_support::debug::timed("facts hints", || _copy_hints(body));
        Self { index, live, masks, widths, confined, hints }
    }
}

/// The values whose register a rewrite made them share with another, lose
/// to a point that destroys it, or leave their class: the later of each pair.
fn _overlapping(
    union: &LiveUnion,
    r#where: &IndexMap<u32, Register>,
    facts: &Facts,
) -> BTreeSet<u32> {
    let found = _overlapping_by_start(union, r#where, facts);
    if llrm_support::env_set("LLRM_CHECK_OVERLAPPING") {
        assert!(
            found == _overlapping_reference(union, r#where, facts),
            "values sharing a register found by start differ from the pairwise look"
        );
    }
    found
}

fn _overlapping_by_start(
    union: &LiveUnion,
    r#where: &IndexMap<u32, Register>,
    facts: &Facts,
) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for (_, held) in union.registers() {
        // The segments of the values kept so far, by start: kept values do not
        // overlap one another, so a segment meets a kept one only if
        // the last that starts before its end reaches past its start.
        let mut kept: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
        for value in &held {
            let Some(mine) = facts.live.get(value) else { continue };
            let width = facts.widths.get(value).copied().unwrap_or(4);
            let outside = facts
                .confined
                .get(value)
                .is_some_and(|class| !class.iter().any(|one| _whole(*one) == _whole(r#where[value])));
            let meets = |kept: &std::collections::BTreeMap<i64, i64>| {
                mine.segments
                    .iter()
                    .any(|seg| kept.range(..seg.end).next_back().is_some_and(|(_, end)| *end > seg.start))
            };
            if outside || meets(&kept) || _clobbered(mine, r#where[value], &facts.masks, width) {
                out.insert(*value);
            } else {
                kept.extend(mine.segments.iter().map(|seg| (seg.start, seg.end)));
            }
        }
    }
    out
}

/// What `_overlapping` was: each value against every one kept before it.
fn _overlapping_reference(
    union: &LiveUnion,
    r#where: &IndexMap<u32, Register>,
    facts: &Facts,
) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for (_, held) in union.registers() {
        let mut kept: Vec<u32> = Vec::new();
        for value in &held {
            let Some(mine) = facts.live.get(value) else { continue };
            let width = facts.widths.get(value).copied().unwrap_or(4);
            let outside = facts
                .confined
                .get(value)
                .is_some_and(|class| !class.iter().any(|one| _whole(*one) == _whole(r#where[value])));
            if outside
                || kept.iter().any(|other| facts.live[other].overlaps(mine))
                || _clobbered(mine, r#where[value], &facts.masks, width)
            {
                out.insert(*value);
            } else {
                kept.push(*value);
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn _allocated(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, Register>>,
    unspillable: Option<&BTreeSet<u32>>,
    protected: Option<&BTreeSet<u32>>,
    preferred: Option<&IndexMap<u32, Register>>,
    cpu: ProfileOrName<'_>,
    segments: &Segments,
    classes: &RegisterClasses,
    rewrite: Option<(&mut Frame, bool)>,
) -> Result<(Assignment, LirBody, BTreeSet<u32>), Error> {
    let splitting = rewrite.as_ref().is_some_and(|(_, splitting)| *splitting);
    let mut rewrite: Option<&mut Frame> = rewrite.map(|(frame, _)| frame);
    let profile = targets::profile(cpu).map_err(Error::Value)?;
    let mut body = body.clone();
    let data_free = !datagroup::names_data_segment(&body, segments);
    let empty = BTreeSet::new();
    let protected = protected.unwrap_or(&empty);
    let mut unspillable: BTreeSet<u32> = unspillable.cloned().unwrap_or_default();
    // The values merging updates made: spilled in their turn they are not
    // merged again, which would make the value spilled again, without end.
    let mut plain: BTreeSet<u32> = BTreeSet::new();
    let mut facts = Facts::of(&body, profile, segments, classes, &unspillable, protected, &Frequency::of(&body));
    if let Some(why) = unallocatable(&facts, pinned.unwrap_or(&IndexMap::default()), &unspillable, segments, classes) {
        return Err(Unplaced(why).into());
    }
    // Every value this allocation has known: a new one is numbered above them.
    let mut floor = splitkit::_next_value(&body);
    let mut fixed: IndexMap<u32, Register> = pinned.cloned().unwrap_or_default();
    // Values a split made or left behind, never split again: LLVM's `RS_Split2`
    // and `RS_Spill`.
    let mut pieces: BTreeSet<u32> = BTreeSet::new();
    let mut splits_made = 0_usize;
    let mut placing: Option<(spillplacement::Bundles, LiveRows)> = None;
    let no_preference = IndexMap::default();
    let preferred = preferred.unwrap_or(&no_preference);

    let mut union = LiveUnion::new();
    let mut r#where: IndexMap<u32, Register> = IndexMap::default();
    let mut stage: IndexMap<u32, Stage> = IndexMap::default();
    let mut spilled: BTreeSet<u32> = BTreeSet::new();
    let mut cost = 0.0;
    let mut cascades: IndexMap<u32, i64> = IndexMap::default();
    let mut newest = 1;

    let wide = classes.available.len();
    let queued = |value: u32,
                  live: &IndexMap<u32, Interval>,
                  stage: &IndexMap<u32, Stage>,
                  fixed: &IndexMap<u32, Register>,
                  confined: &Classes| {
        // A value that only some registers can hold goes first, as LLVM's
        // register class priority puts it: the wide ones fit around it.
        Reverse(Queued(
            !fixed.contains_key(&value),
            -_queue_priority(
                live.get(&value),
                stage.get(&value).copied().unwrap_or(Stage::Assign),
                confined.get(&value).map(BTreeSet::len),
                wide,
            ),
            value,
        ))
    };

    let mut queue: BinaryHeap<Reverse<Queued>> =
        _values(&body).into_iter().map(|one| queued(one, &facts.live, &stage, &fixed, &facts.confined)).collect();
    // How many entries each value has in the queue.
    let mut waiting: IndexMap<u32, usize> = IndexMap::default();
    for Reverse(Queued(_, _, one)) in &queue {
        *waiting.entry(*one).or_insert(0) += 1;
    }
    let mut wanted = _wanted(&facts.hints, &fixed);
    let mut claims = Claims::default();
    let mut fenced: BTreeSet<u32> = fixed.keys().copied().chain(protected.iter().copied()).collect();
    let mut seen = 0;
    while !queue.is_empty() && seen < BUDGET {
        seen += 1;
        let Reverse(Queued(_flexible, _prio, value)) = queue.pop().expect("queue is not empty");
        *waiting.entry(value).or_insert(1) -= 1;
        if r#where.contains_key(&value) || spilled.contains(&value) {
            continue;
        }
        let at = *stage.entry(value).or_insert(Stage::Assign);
        let Some(mine) = facts.live.get(&value).cloned() else {
            continue;
        };
        let mut order: Vec<Register> = match fixed.get(&value) {
            None => target::order(facts.confined.get(&value), segments, classes),
            Some(register) => vec![*register],
        };
        if !data_free {
            order.retain(|one| _whole(*one) != segments.data);
        }
        if protected.contains(&value) && _reserves_word_base(&body, value, &facts.confined, classes) {
            let word: BTreeSet<Register> = classes.word_bases.iter().map(|one| _whole(*one)).collect();
            order = order
                .iter()
                .copied()
                .filter(|one| !word.contains(&_whole(*one)))
                .chain(order.iter().copied().filter(|one| word.contains(&_whole(*one))))
                .collect();
        }
        if !fixed.contains_key(&value) {
            let mut votes: IndexMap<Register, i64> = IndexMap::default();
            for other in facts.hints.get(&value).into_iter().flatten() {
                if let Some(register) = fixed.get(other).or_else(|| r#where.get(other)) {
                    *votes.entry(_whole(*register)).or_insert(0) += 1;
                }
            }
            if votes.is_empty() {
                if let Some(register) = wanted.get(&value) {
                    *votes.entry(*register).or_insert(0) += 1;
                }
            }
            let claimed = claims.by(&wanted, &facts.live, value, &mine, &r#where);
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
        order.sort_by_key(|one| _whole(*one) == segments.data);
        let width = facts.widths.get(&value).copied().unwrap_or(4);
        if let Some(got) = _free(&mine, &order, &union, &facts.live, &facts.masks, width) {
            r#where.insert(value, got);
            union.add(_whole(got), value, &facts.live);
            stage.insert(value, Stage::Done);
            continue;
        }

        // An unspillable range has no fallback, so it may evict at any stage.
        if at == Stage::Assign || mine.weight == INF {
            let movable = |other: u32, register: Register| -> bool {
                if fixed.contains_key(&other) {
                    return false;
                }
                let elsewhere: Vec<Register> = target::order(facts.confined.get(&other), segments, classes)
                    .into_iter()
                    .filter(|one| _whole(*one) != _whole(register))
                    .filter(|one| data_free || _whole(*one) != segments.data)
                    .collect();
                _free(
                    &facts.live[&other],
                    &elsewhere,
                    &union,
                    &facts.live,
                    &facts.masks,
                    facts.widths.get(&other).copied().unwrap_or(4),
                )
                .is_some()
            };
            let evicted = llrm_support::debug::timed("regalloc evict", || {
                _evict(
                    &mine,
                    &order,
                    &union,
                    &facts.live,
                    &facts.masks,
                    &movable,
                    &fenced,
                    width,
                    Some(cascades.get(&value).copied().unwrap_or(newest)),
                    Some(&cascades),
                )
            });
            if let Some((got, victims)) = evicted {
                if !cascades.contains_key(&value) {
                    cascades.insert(value, newest);
                    newest += 1;
                }
                for one in victims {
                    union.remove(_whole(got), one, &facts.live);
                    r#where.shift_remove(&one);
                    cascades.insert(one, cascades[&value]);
                    stage.insert(one, Stage::Assign);
                    queue.push(queued(one, &facts.live, &stage, &fixed, &facts.confined));
                    *waiting.entry(one).or_insert(0) += 1;
                }
                r#where.insert(value, got);
                union.add(_whole(got), value, &facts.live);
                stage.insert(value, Stage::Done);
                continue;
            }
            if at == Stage::Assign {
                stage.insert(value, Stage::Split);
                queue.push(queued(value, &facts.live, &stage, &fixed, &facts.confined));
                *waiting.entry(value).or_insert(0) += 1;
                continue;
            }
        }

        let bound = fixed.contains_key(&value) || mine.weight == INF;
        // `trySplit`, carved at once: the pieces and the rest compete again.
        let mut rewritten: Option<Vec<u32>> = None;
        if splitting && at == Stage::Split && !pieces.contains(&value) && !bound {
            let _split = llrm_support::debug::span("regalloc split");
            let (bundles, live_sets) = placing.get_or_insert_with(|| {
                llrm_support::debug::timed("split placing", || (spillplacement::bundles(&body), live_rows(&body)))
            });
            let spread =
                llrm_support::debug::timed("split spread", || splitkit::live_blocks(&body, value, &*live_sets));
            let occupied = llrm_support::debug::timed("split occupied", || splitkit::Occupied {
                segments: union
                    .registers()
                    .map(|(register, held)| {
                        (
                            register,
                            held.iter()
                                .filter(|other| **other != value)
                                .flat_map(|other| facts.live[other].segments.clone())
                                .collect(),
                        )
                    })
                    .collect(),
                masks: &facts.masks,
            });
            let sets: &dyn LiveAt = &*live_sets;
            // A range in one block splits locally; any other by region, and
            // failing that block by block. Only a piece that pays is carved.
            let regions: Vec<splitkit::Region> = match llrm_support::debug::timed("split local", || {
                splitkit::local(&body, value, &facts.index, sets, &order, &occupied, width)
            }) {
                Some(found) => vec![found],
                None => {
                    let placed = llrm_support::debug::timed("split placed", || {
                        splitkit::placed(&body, value, &facts.index, sets, bundles, &order, &occupied, width)
                    });
                    if placed.is_empty() {
                        llrm_support::debug::timed("split per block", || splitkit::per_block(&body, value, sets))
                    } else {
                        placed
                    }
                }
            };
            let regions: Vec<splitkit::Region> = llrm_support::debug::timed("split pays", || {
                regions.into_iter().filter(|region| splitkit::pays(&body, value, region, sets)).collect()
            });
            for region in &regions {
                llrm_support::debug!("split", "{}: split {value} at {:?}", body.name, region.spans);
            }
            let mut cut = body.clone();
            let mut moves: Vec<splitkit::Moved> = Vec::new();
            let mut made: Vec<u32> = Vec::new();
            let _carving = llrm_support::debug::span("split carving");
            for region in regions {
                let fresh = splitkit::_next_value(&cut).max(floor);
                floor = fresh + 1;
                let moved = moves.iter().fold(region, |region, moved| region.moved(moved));
                if let Some((next, shifted)) = splitkit::carved_moving(&cut, value, fresh, width, &moved) {
                    cut = next;
                    moves.push(shifted);
                    made.push(fresh);
                }
            }
            if !made.is_empty() {
                // As LLVM's RS_Split2: a piece splits again only while its
                // live blocks strictly shrink, so splitting ends.
                drop(_carving);
                // Only the pieces made are asked of.
                let after = llrm_support::debug::timed("split after liveness", || {
                    crate::analysis::occurrences::live_among(&cut, &made.iter().copied().collect())
                });
                pieces.extend(made.iter().copied().filter(|one| splitkit::live_blocks(&cut, *one, &*after) >= spread));
                pieces.insert(value);
                body = cut;
                for one in &made {
                    stage.insert(*one, Stage::Assign);
                }
                // The rest may only take a free register or spill.
                stage.insert(value, Stage::Spill);
                made.push(value);
                splits_made += 1;
                rewritten = Some(made);
            }
        }
        if rewritten.is_none() {
            // Last chance: move what holds a register rather than spill.
            let choices = |other: u32| -> Vec<Register> {
                let mut order = match fixed.get(&other) {
                    None => target::order(facts.confined.get(&other), segments, classes),
                    Some(register) => vec![*register],
                };
                order.retain(|one| data_free || _whole(*one) != segments.data);
                order.sort_by_key(|one| _whole(*one) == segments.data);
                order
            };
            let wide = |other: u32| facts.widths.get(&other).copied().unwrap_or(4);
            let mut coloring = Coloring {
                union: &mut union,
                r#where: &mut r#where,
                live: &facts.live,
                masks: &facts.masks,
                order: &choices,
                width: &wide,
                fenced: &fenced,
                budget: Coloring::BUDGET,
                stack: Vec::new(),
            };
            if llrm_support::debug::timed("regalloc recolor", || coloring.recolor(value, 0, &mut BTreeSet::new())) {
                stage.insert(value, Stage::Done);
                continue;
            }
            // A value that cannot be spilled takes a register by force.
            if fixed.contains_key(&value) || unspillable.contains(&value) {
                let hard = |other: u32| fixed.contains_key(&other) || unspillable.contains(&other);
                let Some((got, victims)) = _forced(&mine, &order, &union, &facts.live, &facts.masks, &hard, width)
                else {
                    for register in &order {
                        let holders: Vec<(u32, bool)> = union
                            .meeting(&_whole(*register), &mine, &facts.live)
                            .into_iter()
                            .map(|other| (other, hard(other)))
                            .collect();
                        llrm_support::debug!(
                            "regalloc",
                            "{}: value#{value} {:?} {}: clobbered {}, held by {holders:?}",
                            body.name,
                            mine.segments,
                            register.repr(),
                            _clobbered(&mine, *register, &facts.masks, width)
                        );
                    }
                    // `unallocatable` refused every body that could land here.
                    unreachable!(
                        "{}: value#{value} cannot be spilled and no register it may take is free of values that cannot be",
                        body.name
                    );
                };
                LAST_RESORTS.with(|count| count.set(count.get() + 1));
                llrm_support::debug!(
                    "regalloc",
                    "{}: last resort for value#{value}: {} evicts {victims:?}",
                    body.name,
                    got.repr()
                );
                for one in victims {
                    union.remove(_whole(got), one, &facts.live);
                    r#where.shift_remove(&one);
                    stage.insert(one, Stage::Spill);
                    queue.push(queued(one, &facts.live, &stage, &fixed, &facts.confined));
                    *waiting.entry(one).or_insert(0) += 1;
                }
                r#where.insert(value, got);
                union.add(_whole(got), value, &facts.live);
                stage.insert(value, Stage::Done);
                continue;
            }
            cost += mine.weight;
            stage.insert(value, Stage::Done);
            match rewrite.as_deref_mut() {
                None => {
                    spilled.insert(value);
                    continue;
                }
                // InlineSpiller: the value and the siblings worth sharing its
                // slot go to the stack now; their reloads join the queue.
                Some(frame) => {
                    let _spill = llrm_support::debug::span("regalloc spill");
                    // A sibling may already hold a register: it gives it up,
                    // since sharing the slot makes the copies between them
                    // free.
                    let mut chosen = BTreeSet::from([value]);
                    let settled: BTreeSet<u32> =
                        fixed.keys().chain(protected.iter()).chain(unspillable.iter()).copied().collect();
                    chosen.extend(llrm_support::debug::timed("spill siblings", || {
                        spiller::siblings(&body, &chosen, Some(frame), &settled)
                    })?);
                    llrm_support::debug!("spill", "{}: spill {value} with {:?}", body.name, chosen);
                    for one in &chosen {
                        if let Some(register) = r#where.shift_remove(one) {
                            union.remove(_whole(register), *one, &facts.live);
                        }
                    }
                    let (spilt, mut made, merged) =
                        spiller::spilled_apart(&body, &chosen, frame, floor, classes, &plain)?;
                    plain.extend(merged.iter().copied());
                    body = spilt;
                    // A value the spiller keeps (a load from its own home
                    // cell) is now as short as a reload, and is placed as one.
                    let kept: BTreeSet<u32> = _values(&body).into_iter().filter(|one| chosen.contains(one)).collect();
                    for one in &chosen {
                        if kept.contains(one) {
                            stage.insert(*one, Stage::Assign);
                        } else {
                            spilled.insert(*one);
                            stage.insert(*one, Stage::Done);
                        }
                    }
                    made.extend(kept);
                    unspillable.extend(made.iter().copied());
                    rewritten = Some(made.into_iter().chain(merged).collect());
                }
            }
        }
        // The body changed: every fact about it is recomputed, and whatever
        // the change left sharing a register competes again.
        let Some(made) = rewritten else { continue };
        floor = floor.max(splitkit::_next_value(&body));
        facts = Facts::of(
            &body,
            profile,
            segments,
            classes,
            &unspillable,
            protected,
            &llrm_support::debug::timed("regalloc frequency", || Frequency::of(&body)),
        );
        // What is done of the rewrite besides its facts: the pins, the placed
        // values it disturbs, the queue.
        let _after = llrm_support::debug::span("regalloc after rewrite");
        placing = None;
        llrm_support::debug::timed("after required", || {
            for (one, register) in constrain::required(&body, classes) {
                fixed.entry(one).or_insert(register);
            }
        });
        wanted = llrm_support::debug::timed("after wanted", || _wanted(&facts.hints, &fixed));
        claims = Claims::default();
        union.refresh();
        fenced = fixed.keys().copied().chain(protected.iter().copied()).collect();
        let gone: Vec<u32> = r#where.keys().copied().filter(|one| !facts.live.contains_key(one)).collect();
        let clashing = llrm_support::debug::timed("after overlapping", || _overlapping(&union, &r#where, &facts));
        for one in gone.iter().chain(&clashing) {
            if let Some(register) = r#where.shift_remove(one) {
                union.remove(_whole(register), *one, &facts.live);
            }
        }
        // What the rewrite made or moved competes again; the rest still waits.
        let changed: BTreeSet<u32> = made.iter().chain(&gone).chain(&clashing).copied().collect();
        llrm_support::debug::timed("after queue", || {
            for one in facts.live.keys().copied().filter(|one| !r#where.contains_key(one) && !spilled.contains(one)) {
                if changed.contains(&one) || waiting.get(&one).copied().unwrap_or(0) == 0 {
                    queue.push(queued(one, &facts.live, &stage, &fixed, &facts.confined));
                    *waiting.entry(one).or_insert(0) += 1;
                }
            }
        });
    }
    if seen >= BUDGET {
        llrm_support::debug!("regalloc", "{}: out of budget with {} queued", body.name, queue.len());
    }
    let settled = Settled {
        live: &facts.live,
        masks: &facts.masks,
        widths: &facts.widths,
        hints: &facts.hints,
        confined: &facts.confined,
        data_free,
        segments,
        classes,
    };
    _recolored_hints(&mut r#where, &mut union, &settled, |value| {
        fixed.contains_key(&value) || protected.contains(&value) || preferred.contains_key(&value)
    });
    LAST_STATS
        .with(|stats| stats.set((splits_made, pieces.len(), pieces.intersection(&spilled).count(), spilled.len())));
    Ok((
        Assignment {
            r#where,
            spilled: if rewrite.is_some() { BTreeSet::new() } else { spilled.clone() },
            cost,
            optimal: false,
            why: "greedy with eviction".to_owned(),
        },
        body,
        spilled,
    ))
}

thread_local! {
    /// How many splits the base allocation of the last body that had one made.
    static BASE_SPLITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many splits the base allocation of the last body this thread allocated
/// made, for a test.
pub fn base_splits() -> usize {
    BASE_SPLITS.with(std::cell::Cell::get)
}

thread_local! {
    /// The last allocation's (splits made, pieces, pieces spilled, values
    /// spilled), for the trial log.
    static LAST_STATS: std::cell::Cell<(usize, usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0, 0)) };
}

/// What recoloring reads of a finished assignment.
struct Settled<'a> {
    live: &'a IndexMap<u32, Interval>,
    masks: &'a Masks,
    widths: &'a IndexMap<u32, u32>,
    hints: &'a IndexMap<u32, Vec<u32>>,
    confined: &'a Classes,
    data_free: bool,
    segments: &'a Segments,
    classes: &'a RegisterClasses,
}

/// LLVM's `tryHintsRecoloring`. Greedy order decides which end of a copy
/// is placed first, and that end picks blindly when its partner has no
/// register yet: sum_three's pointer went to DX and its latch kept
/// `mov di,dx`. Afterwards, a value moves to a copy partner's register when
/// that is free over its whole interval and joins more of its copies than
/// the register it has; each joined copy is a deleted move. Every move
/// joins more pairs than it splits, so this ends.
fn _recolored_hints(
    r#where: &mut IndexMap<u32, Register>,
    union: &mut LiveUnion,
    settled: &Settled<'_>,
    pinned: impl Fn(u32) -> bool,
) {
    let joined = |value: u32, register: Register, r#where: &IndexMap<u32, Register>| {
        settled.hints[&value]
            .iter()
            .filter(|other| r#where.get(*other).is_some_and(|theirs| _whole(*theirs) == register))
            .count()
    };
    let mut moved = true;
    while moved {
        moved = false;
        let values: Vec<u32> =
            r#where.keys().copied().filter(|value| settled.hints.contains_key(value) && !pinned(*value)).collect();
        for value in values {
            let (now, Some(mine)) = (_whole(r#where[&value]), settled.live.get(&value)) else {
                continue;
            };
            let width = settled.widths.get(&value).copied().unwrap_or(4);
            let mut best: Option<(usize, Register)> = Some((joined(value, now, r#where), now));
            for other in &settled.hints[&value] {
                let Some(theirs) = r#where.get(other).map(|one| _whole(*one)) else {
                    continue;
                };
                let gained = joined(value, theirs, r#where);
                if best.is_some_and(|(most, _)| gained <= most)
                    || (!settled.data_free && theirs == settled.segments.data)
                {
                    continue;
                }
                let order: Vec<Register> =
                    target::order(settled.confined.get(&value), settled.segments, settled.classes)
                        .into_iter()
                        .filter(|one| _whole(*one) == theirs)
                        .collect();
                if let Some(register) = _free(mine, &order, union, settled.live, settled.masks, width) {
                    best = Some((gained, register));
                }
            }
            if let Some((_, register)) = best.filter(|(_, register)| _whole(*register) != now) {
                union.remove(now, value, settled.live);
                union.add(_whole(register), value, settled.live);
                r#where.insert(value, register);
                moved = true;
            }
        }
    }
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

/// Where a range sits in the queue: first by how few registers may hold it
/// (LLVM's register class `AllocationPriority`,
/// which `RegClassPriorityTrumpsGlobalness` puts before anything else), then
/// `_priority`. The wide values fit around the narrow ones.
fn _queue_priority(
    one: Option<&Interval>,
    at: Stage,
    class: Option<usize>,
    wide: usize,
) -> f64 {
    let narrow = class.map_or(0, |registers| wide.saturating_sub(registers));
    _priority(one, at) + narrow as f64 * 1e9
}

/// Where this range sits in the queue. Larger first, as LLVM does.
fn _priority(
    one: Option<&Interval>,
    at: Stage,
) -> f64 {
    match one {
        None => 0.0,
        Some(one) => one.size() as f64 + if at != Stage::Assign { 1e6 } else { 0.0 },
    }
}

/// Whether an address-class value should leave a 16-bit word base free.
fn _reserves_word_base(
    body: &LirBody,
    value: u32,
    confined: &Classes,
    classes: &RegisterClasses,
) -> bool {
    let Some(choices) = confined.get(&value) else {
        return false;
    };
    if choices.is_empty() || !choices.is_subset(&classes.addressing) {
        return false;
    }
    body.blocks
        .iter()
        .flat_map(|block| &block.insns)
        .any(
            |one| one.what
                .as_ref()
                .is_some_and(
                    |what| what.dests.iter().chain(&what.sources).any(|place| match place {
                        Loc::Mem(cell) => match (cell.base, cell.index) {
                            (Some(base), Some(index)) => base.value != value && index.value != value && cell.scale == 1,
                            _ => false,
                        },
                        _ => false,
                    }),
                ),
        )
}

/// How wide each value is anywhere it is read or written.
pub fn _widest(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            let named = one
                .what
                .iter()
                .flat_map(|what| what.dests.iter().chain(&what.sources))
                .flat_map(ir::values)
                .chain(one.requires.iter().chain(&one.delivers).map(|(place, _register)| *place));
            for place in named {
                let widest = out.entry(place.value).or_insert(0);
                *widest = (*widest).max(place.width);
            }
            for (value, width) in &one.widths {
                let widest = out.entry(*value).or_insert(0);
                *widest = (*widest).max(*width);
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

/// The points that destroy registers, and, for each register, where, in order:
/// whether a value is live across one is asked of every value and every
/// register, over and over, and answered from the slots by bisection, not by a
/// look at every point.
#[derive(Default)]
pub struct Masks {
    list: Vec<Mask>,
    reaching: std::cell::OnceCell<crate::support::hash::HashMap<Register, Reaching>>,
}

/// Where one register is destroyed, sorted: before the point's own reads
/// (`read`), during it (`during`), or only its high half (`high`).
#[derive(Default)]
struct Reaching {
    read: Vec<i64>,
    during: Vec<i64>,
    high: Vec<i64>,
}

impl Masks {
    pub fn new(list: Vec<Mask>) -> Self {
        Self { list, reaching: std::cell::OnceCell::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Mask> {
        self.list.iter()
    }

    fn reaching(
        &self,
        register: Register,
    ) -> Option<&Reaching> {
        self.reaching
            .get_or_init(|| {
                let mut found: crate::support::hash::HashMap<Register, Reaching> =
                    crate::support::hash::HashMap::default();
                for mask in &self.list {
                    for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
                        let one = found.entry(*register).or_default();
                        if mask.before.contains(register) {
                            one.read.push(mask.slot);
                        } else if mask.during.contains(register) {
                            one.during.push(mask.slot);
                        } else {
                            one.high.push(mask.slot);
                        }
                    }
                }
                for one in found.values_mut() {
                    one.read.sort_unstable();
                    one.during.sort_unstable();
                    one.high.sort_unstable();
                }
                found
            })
            .get(&register)
    }
}

/// Every point a register is destroyed without being named, and which. The
/// data segment register is reloaded ahead of a point that needs the data
/// group, so it holds none of that point's operands either.
pub fn _masks(
    body: &LirBody,
    index: &Indexes,
    segments: &Segments,
) -> Masks {
    let mut out: Vec<Mask> = Vec::new();
    for block in &body.blocks {
        for one in &block.insns {
            let during: BTreeSet<Register> = one.clobbers.iter().map(|register| _whole(*register)).collect();
            let high: BTreeSet<Register> = one.clobbers_high.iter().map(|register| _whole(*register)).collect();
            let before: BTreeSet<Register> =
                target::needs_data_group(one).then_some(segments.data).into_iter().collect();
            if !during.is_empty() || !high.is_empty() || !before.is_empty() {
                out.push(Mask { slot: index.at[&ranges::key(one)], during, high, before });
            }
        }
    }
    Masks::new(out)
}

/// The 32-bit register this one is part of.
pub fn _whole(register: Register) -> Register {
    ir::root(register)
}

/// Whether this range is live across a point that destroys the register, or
/// into one that destroys it before reading.
pub fn _clobbered(
    one: &Interval,
    register: Register,
    masks: &Masks,
    width: u32,
) -> bool {
    let answer = _clobbered_from(one, register, masks, width);
    if check_clobbered() {
        assert_eq!(
            answer,
            _clobbered_reference(one, register, masks, width),
            "LLRM_CHECK_CLOBBERED: {register:?} over {:?}",
            one.segments
        );
    }
    answer
}

fn check_clobbered() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_CLOBBERED"))
}

/// `_clobbered`, from where each register is destroyed: a segment meets a point
/// where the point is after its start and, by `reaches`, no later than its end.
fn _clobbered_from(
    one: &Interval,
    register: Register,
    masks: &Masks,
    width: u32,
) -> bool {
    let Some(places) = masks.reaching(_whole(register)) else { return false };
    // The first slot after `start` is the best candidate: any later one reaches
    // no further.
    let after = |slots: &[i64], start: i64| slots.get(slots.partition_point(|slot| *slot <= start)).copied();
    one.segments
        .iter()
        .any(
            |seg| after(&places.read, seg.start).is_some_and(|slot| seg.end >= ranges::def_point(slot))
                || after(&places.during, seg.start).is_some_and(|slot| seg.end > ranges::def_point(slot))
                || (width > 2 && after(&places.high, seg.start).is_some_and(|slot| seg.end > ranges::def_point(slot))),
        )
}

/// What `_clobbered` was: a look at every point.
pub fn _clobbered_reference(
    one: &Interval,
    register: Register,
    masks: &Masks,
    width: u32,
) -> bool {
    let mine = _whole(register);
    for mask in masks.iter() {
        let slot = mask.slot;
        let read = mask.before.contains(&mine);
        if !read && !mask.during.contains(&mine) && (!mask.high.contains(&mine) || width <= 2) {
            continue;
        }
        // A use keeps its value alive to `slot + DEF`.
        let reaches = |end: i64| if read { end >= ranges::def_point(slot) } else { end > ranges::def_point(slot) };
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
    union: &LiveUnion,
    live: &IndexMap<u32, Interval>,
    masks: &Masks,
    width: u32,
) -> Option<Register> {
    for register in order {
        if _clobbered(one, *register, masks, width) {
            continue;
        }
        if !union.busy(&_whole(*register), one, live) {
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
    union: &LiveUnion,
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
        let victims: Vec<u32> = union.meeting(&_whole(*register), one, live);
        if victims.is_empty() {
            continue;
        }
        if victims.iter().any(|other| protected.contains(other)) {
            continue;
        }
        // LLVM's urgent eviction: an unspillable range may break the cascade
        // of a spillable one, or it would have nowhere to go.
        if let Some(cascade) = cascade {
            if victims.iter().any(|other| {
                cascades.and_then(|found| found.get(other)).copied().unwrap_or(0) >= cascade
                    && !(one.weight == INF && live[other].weight < INF)
            }) {
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

/// The last resort for a value that cannot be spilled: the register of its
/// class whose holders it overlaps cost least to evict, and those holders.
/// A holder that cannot be spilled either (`hard`) rules its register out; a
/// protected holder may be evicted, and a trial that loses it is dropped;
/// when every register is ruled out, one point of the body asks for more
/// registers than the machine has.
fn _forced(
    one: &Interval,
    order: &[Register],
    union: &LiveUnion,
    live: &IndexMap<u32, Interval>,
    masks: &Masks,
    hard: &dyn Fn(u32) -> bool,
    width: u32,
) -> Option<(Register, Vec<u32>)> {
    let mut best: Option<(f64, Register, Vec<u32>)> = None;
    for register in order {
        if _clobbered(one, *register, masks, width) {
            continue;
        }
        let victims: Vec<u32> = union.meeting(&_whole(*register), one, live);
        if victims.iter().any(|other| hard(*other)) {
            continue;
        }
        let bill: f64 = victims.iter().map(|other| live[other].weight).sum();
        if best.as_ref().is_none_or(|found| bill < found.0) {
            best = Some((bill, *register, victims));
        }
    }
    best.map(|(_bill, register, victims)| (register, victims))
}

thread_local! {
    static TRIALS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many allocations this thread has made to try another shape of a body
/// than the one it was given.
pub fn trials() -> usize {
    TRIALS.with(std::cell::Cell::get)
}

thread_local! {
    static LAST_RESORTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread's allocations had to evict by force: a value
/// that cannot be spilled found no register by any other means.
pub fn last_resorts() -> usize {
    LAST_RESORTS.with(std::cell::Cell::get)
}

/// What last-chance recoloring may change: LLVM's `LiveRegMatrix` and
/// `VirtRegMap`.
pub struct Coloring<'a> {
    pub union: &'a mut LiveUnion,
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
    /// Registers tried per recoloring session. A search that fails tries every
    /// register at every level, so it grows with the class (x_ll_arith: 7
    /// registers instead of 6 took 8 of 25 sessions to the old 2000, 3x the
    /// time). Of 19.6k sessions on QCport and the 66 programs, 178 succeed
    /// and 176 of those within 256 tries.
    pub const BUDGET: usize = 256;

    fn take(
        &mut self,
        value: u32,
        register: Register,
    ) {
        self.r#where.insert(value, register);
        self.union.add(_whole(register), value, self.live);
    }

    fn release(
        &mut self,
        value: u32,
    ) {
        if let Some(register) = self.r#where.shift_remove(&value) {
            self.union.remove(_whole(register), value, self.live);
        }
    }

    /// `RAGreedy::tryLastChanceRecoloring`: a register for `value` whose
    /// holders all move elsewhere, recursively. `recolored` are the values
    /// placed in this session, never moved again. On success `value` is
    /// placed; on failure nothing has changed.
    pub fn recolor(
        &mut self,
        value: u32,
        depth: usize,
        recolored: &mut BTreeSet<u32>,
    ) -> bool {
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
            let mut holders: Vec<u32> = self.union.meeting(&_whole(register), mine, self.live);
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
            let all = holders
                .iter()
                .all(
                    |other| match _free(
                        &self.live[other],
                        &(self.order)(*other),
                        self.union,
                        self.live,
                        self.masks,
                        (self.width)(*other),
                    ) {
                        Some(found) => {
                            self.take(*other, found);
                            recolored.insert(*other);
                            true
                        }
                        None => self.recolor(*other, depth + 1, recolored),
                    },
                );
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

/// Why no allocation of `body` can exist, if so: a value no register may hold,
/// two values required in one register where both are live, or more values
/// that cannot be spilled live at one point than there are registers. Checked
/// once, before allocation, which then has no refusal of its own.
fn unallocatable(
    facts: &Facts,
    fixed: &IndexMap<u32, Register>,
    unspillable: &BTreeSet<u32>,
    segments: &Segments,
    classes: &RegisterClasses,
) -> Option<String> {
    if let Some((value, _)) =
        facts.confined.iter().find(|(_, class)| target::order(Some(*class), segments, classes).is_empty())
    {
        return Some(format!("value#{value} may be in no register"));
    }
    let mut by_register: IndexMap<Register, Vec<u32>> = IndexMap::default();
    for (value, register) in fixed {
        by_register.entry(_whole(*register)).or_default().push(*value);
    }
    for (register, values) in &by_register {
        for (at, first) in values.iter().enumerate() {
            for second in &values[at + 1..] {
                if let (Some(one), Some(other)) = (facts.live.get(first), facts.live.get(second)) {
                    if one.overlaps(other) {
                        return Some(format!(
                            "value#{first} and value#{second} are both required in {} and both live",
                            register.repr()
                        ));
                    }
                }
            }
        }
    }
    // The busiest point among the values that cannot be spilled.
    let mut edges: Vec<(i64, i64)> = Vec::new();
    let general: BTreeSet<Register> = classes.available.iter().map(|one| _whole(*one)).collect();
    let in_general = |value: &u32| match (fixed.get(value), facts.confined.get(value)) {
        (Some(register), _) => general.contains(&_whole(*register)),
        (None, Some(class)) => class.iter().any(|one| general.contains(&_whole(*one))),
        (None, None) => true,
    };
    let hard: BTreeSet<u32> = fixed.keys().chain(unspillable).copied().collect();
    for value in hard.iter().filter(|value| in_general(value)) {
        for segment in facts.live.get(value).map_or(&[][..], |one| &one.segments[..]) {
            edges.extend([(segment.start, 1), (segment.end, -1)]);
        }
    }
    edges.sort_unstable();
    let (mut now, mut busiest) = (0, 0);
    for (_, change) in edges {
        now += change;
        busiest = busiest.max(now);
    }
    if busiest > classes.available.len() as i64 {
        return Some(format!(
            "{busiest} values that cannot be spilled are live at one point, and the machine has {} registers",
            classes.available.len()
        ));
    }
    None
}

/// Assign, then rewrite. LLVM's two halves, in one phase.
pub struct RegAlloc {
    pub pinned: IndexMap<u32, Register>,
    /// One frame, shared with the phases around this one, as Python shares it.
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
    pub segments: Segments,
    pub classes: Rc<RegisterClasses>,
}

impl RegAlloc {
    pub const NAME: &'static str = "regalloc";

    pub fn new(
        pinned: Option<&IndexMap<u32, Register>>,
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: ProfileOrName<'_>,
        segments: &Segments,
        classes: &Rc<RegisterClasses>,
    ) -> Result<Self, String> {
        Ok(Self {
            pinned: pinned.cloned().unwrap_or_default(),
            frame,
            cpu: targets::profile(cpu)?.clone(),
            segments: segments.clone(),
            classes: Rc::clone(classes),
        })
    }

    /// Assign; where that spills, make the spill real and assign again.
    pub fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Error> {
        let prepare = llrm_support::debug::span("regalloc prepare");
        if self.frame.is_none() {
            self.frame = Some(Rc::new(RefCell::new(frames::of(&body, None, "", None)?)));
        }
        let cpu = self.cpu.clone();
        // Only a body that never names the data segment register itself may
        // find it holding one of its values.
        let segments = self.segments.clone();
        let data_free = !datagroup::names_data_segment(&body, &segments);
        if crate::support::debug::enabled("regclass") {
            let skip = crate::backend::ssaspill::untouchable(&body);
            let found = crate::backend::regclass::violations(&body, &segments, &self.classes, &skip);
            let crowded =
                found.iter().filter(|one| matches!(one.why, crate::backend::regclass::Why::Crowded { .. })).count();
            llrm_support::debug!(
                "regclass",
                "{}: {} points do not fit entering RegAlloc ({} crowded, {} unmatched)",
                body.name,
                found.len(),
                crowded,
                found.len() - crowded
            );
        }
        let floor = self.pinned.keys().copied().max().map_or(0, |one| one + 1);
        let mut body =
            constrain::distinct_classes(&constrain::distinct_roles(&body, floor), floor, &segments, &self.classes);
        body = explicit_selectors(&body, Some(&self.pinned), &segments, &self.classes);
        let (narrowed_body, narrower) = narrowed(&body, &self.pinned);
        body = narrowed_body;
        self.pinned = narrower;
        let confined = classes(&body, &BTreeSet::new(), &segments, &self.classes);
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
        let selectors: BTreeSet<Register> = segments.selectors.iter().copied().collect();
        self.pinned.retain(|value, register| {
            !(target::SEGMENTS.contains(register) && confined.get(value) == Some(&selectors))
        });
        let (constrained_body, fixed) = constrain::constrained(&body, Some(&self.pinned), &self.classes);
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
        let reloads: BTreeSet<u32> = fixed.keys().copied().collect();
        let abandoned: BTreeSet<usize> = body
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .filter(|one| _unread_move(one))
            .map(ranges::key)
            .collect();
        body = spiller::_remove_abandoned(&body, &abandoned);
        self.pinned = prefer.clone();
        self.pinned.extend(constrain::required(&body, &self.classes));
        let frame = Rc::clone(self.frame.as_ref().expect("made above"));
        // A value read once, from a cell that holds, by an instruction that
        // takes the cell: never held in a register. Once per body,
        // before the candidates: every trial starts from the body with those
        // reads placed.
        let mut reloads = reloads;
        {
            let wanted: BTreeSet<u32> = _values(&body)
                .into_iter()
                .filter(|value| !reloads.contains(value) && !self.pinned.contains_key(value))
                .collect();
            let read_at_use = spiller::_folded_reads(&body, &wanted);
            if !read_at_use.is_empty() {
                let floor = splitkit::_next_value(&body);
                let (moved, made) =
                    spiller::spilled_from(&body, &read_at_use, Some(&mut frame.borrow_mut()), floor, &self.classes)?;
                body = moved;
                reloads.extend(made);
            }
        }
        let start = frame.borrow().saved();
        drop(prepare);
        // One allocation per candidate body, each splitting and spilling as
        // it goes; the one whose output costs least is kept.
        let run = |name: &'static str,
                   candidate: &LirBody,
                   unspillable: &BTreeSet<u32>,
                   protected: &BTreeSet<u32>,
                   splitting: bool|
         -> Result<Outcome, Error> {
            let _run = llrm_support::debug::span(name);
            let mut pins = prefer.clone();
            pins.extend(constrain::required(candidate, &self.classes));
            let before = last_resorts();
            let (got, out, spilled) = rewritten(
                candidate,
                Some(&pins),
                Some(unspillable),
                Some(protected),
                (&cpu).into(),
                &segments,
                &self.classes,
                &mut frame.borrow_mut(),
                splitting,
            )?;
            let cost = llrm_support::debug::timed("regalloc cost", || _emitted(&out));
            Ok(Outcome { cost, got, out, spilled, slots: frame.borrow().saved(), forced: last_resorts() - before })
        };
        let mut best = run("regalloc base", &body, &reloads, &BTreeSet::new(), true)?;
        llrm_support::debug!(
            "regalloc",
            "{}: {} insns, {} spilled, cost {}, {} forced",
            body.name,
            best.out.insns().len(),
            best.spilled.len(),
            best.cost,
            best.forced
        );
        let spilled = best.spilled.clone();
        let base_stats = LAST_STATS.with(std::cell::Cell::get);
        BASE_SPLITS.with(|splits| splits.set(base_stats.0));
        if !spilled.is_empty() && cpu.search {
            // Other shapes of the same body, which the base allocation's spills
            // suggest: each is kept only if its output is cheaper.
            let building = llrm_support::debug::span("regalloc candidates");
            let mut candidates: Vec<(Shape, LirBody, BTreeSet<u32>, BTreeSet<u32>, BTreeSet<u32>)> = Vec::new();
            let (separated, opened) = constrain::addressed(&body, &spilled);
            if !opened.is_empty() {
                candidates.push((Shape::Addressed, separated, reloads.clone(), BTreeSet::new(), opened));
            }
            let (unfolded, opened) = spiller::unfolded_indexes(&body, &spilled);
            if !opened.is_empty() {
                candidates.push((Shape::Unfolded, unfolded, reloads.clone(), BTreeSet::new(), BTreeSet::new()));
            }
            let (scoped, keep) = splitkit::loop_bases(&body, &spilled);
            if !keep.is_empty() {
                let folded = _scoped_foldable_indexes(&scoped, &keep);
                let (opened_body, opened) = spiller::unfolded_indexes(&scoped, &folded);
                if !opened.is_empty() {
                    candidates.push((Shape::ScopedOpened, opened_body, reloads.clone(), keep.clone(), keep.clone()));
                }
                candidates.push((Shape::Scoped, scoped, reloads.clone(), keep.clone(), keep.clone()));
            }
            for candidate in _retainable_bases(&body, &spilled) {
                let keep = BTreeSet::from([candidate]);
                candidates.push((Shape::Retainable, body.clone(), reloads.clone(), keep.clone(), keep));
            }
            // Splitting is priced one value at a time, against registers its
            // pieces may later lose; the whole output without it is the check.
            let whole = candidates.len();
            candidates.push((Shape::Whole, body.clone(), reloads.clone(), BTreeSet::new(), BTreeSet::new()));
            drop(building);
            // Unless the search is exhaustive (-Omax): the first shape of
            // `Shape::PICKED` the spills admit, and the
            // body without splitting, which no spill suggests. Across 4045
            // allocations of QCport, the bench and the 66 programs
            // this is the exhaustive search's output on every bench row, +0.04%
            // on QCport's bytes, at 2 allocations instead of up to
            // 12.
            let picked = if cpu.exhaustive {
                None
            } else {
                Shape::PICKED.iter().find_map(|want| candidates.iter().position(|one| one.0 == *want))
            };
            for (at, (shape, candidate, unspillable, protected, kept)) in candidates.into_iter().enumerate() {
                for splitting in [true, false] {
                    if !cpu.exhaustive
                        && !(shape == Shape::Whole && !splitting)
                        && !(Some(at) == picked && splitting != (shape == Shape::Whole))
                    {
                        continue;
                    }
                    // The base allocation split nothing, so allocating without
                    // splitting is that allocation again.
                    if shape == Shape::Whole && !splitting && base_stats.0 == 0 {
                        continue;
                    }
                    // The base run was this body with splitting.
                    if at == whole && splitting {
                        continue;
                    }
                    frame.borrow_mut().restore(&start);
                    TRIALS.with(|count| count.set(count.get() + 1));
                    let trial = match run("regalloc trial", &candidate, &unspillable, &protected, splitting) {
                        Ok(trial) => trial,
                        Err(other) => return Err(other),
                    };
                    llrm_support::debug!(
                        "regalloc",
                        "  trial {shape:?} (splitting {splitting}): {} spilled, cost {}, {} forced",
                        trial.spilled.len(),
                        trial.cost,
                        trial.forced
                    );
                    if kept.is_disjoint(&trial.spilled) && trial.cost < best.cost {
                        llrm_support::debug!("regalloc", "  kept the trial");
                        best = trial;
                    }
                }
            }
        }
        frame.borrow_mut().restore(&best.slots);
        let _apply = llrm_support::debug::span("regalloc apply");
        applied(&best.out, &best.got, &self.classes).map(|placed| datagroup::restored(&placed, data_free, &segments))
    }
}

/// The other shapes of a body the allocator may try after the first allocation,
/// by what they change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// Address computations of spilled values made apart from their users.
    Addressed,
    /// Folded indexes of spilled values opened.
    Unfolded,
    /// Loop-local copies of the base values a loop's spills lost, protected
    /// from spilling.
    Scoped,
    /// `Scoped`, with the indexes it can fold opened.
    ScopedOpened,
    /// One spilled invariant base value protected from spilling.
    Retainable,
    /// The body as it was, for the no-splitting allocation.
    Whole,
}

impl Shape {
    /// The order a shape is picked in: the one that won most often, by mean and
    /// worst case, over every order tried on 4045 allocations.
    const PICKED: [Shape; 4] = [Shape::Retainable, Shape::Scoped, Shape::Addressed, Shape::Unfolded];
}

/// One finished allocation: its output, what that costs, and the frame it left.
struct Outcome {
    cost: f64,
    got: Assignment,
    out: LirBody,
    spilled: BTreeSet<u32>,
    slots: (IndexMap<frames::SlotKey, i64>, IndexMap<i64, i64>),
    /// How many values it took a register for by force.
    forced: usize,
}

/// What a body costs to run: its instructions and memory operands, each
/// weighted by how often its block runs. Alternatives differ only in what the
/// allocator added, so this is the cost of that.
fn _emitted(body: &LirBody) -> f64 {
    // The count the run makes: an anchor that prints nothing is not an
    // instruction.
    if let Some(work) = crate::backend::executed::work(body) {
        return work;
    }
    let busy = Frequency::of(body);
    body.blocks
        .iter()
        .map(|block| {
            let printed = block.insns.iter().filter(|one| crate::backend::masm::prints(one));
            let memory: usize = printed
                .clone()
                .filter_map(|one| one.what.as_ref())
                .map(|what| what.dests.iter().chain(&what.sources).filter(|place| matches!(place, Loc::Mem(_))).count())
                .sum();
            busy.block(block.at) * (printed.count() + memory) as f64
        })
        .sum()
}

impl LIRTransform for RegAlloc {
    fn class_name(&self) -> &'static str {
        "RegAlloc"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        RegAlloc::transform(self, body).map_err(|error| error.to_string())
    }

    fn transform_raising(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Exception> {
        RegAlloc::transform(self, body).map_err(|error| error.raised())
    }
}

/// Python `max(first, second)`: the first unless the second is larger.
fn _max(
    first: f64,
    second: f64,
) -> f64 {
    if second > first { second } else { first }
}

/// Python `min(first, second)`: the first unless the second is smaller.
fn _min(
    first: f64,
    second: f64,
) -> f64 {
    if second < first { second } else { first }
}

/// Intervals whose copies to a value they could share a slot with cost nothing.
fn _sibling_priced(
    body: &LirBody,
    live: IndexMap<u32, Interval>,
    busy: &Frequency,
) -> IndexMap<u32, Interval> {
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
        let in_place = one
            .what
            .as_ref()
            .is_some_and(
                |what| matches!(what.op, Operation::Binary | Operation::Unary)
                    && one.requires.is_empty()
                    && one.delivers.is_empty()
                    && !what.dests.iter().chain(&what.sources).any(|x| matches!(x, Loc::Mem(_))),
            );
        impure.extend(
            one.defines
                .iter()
                .chain(&one.uses)
                .copied()
                .filter(|value| !(in_place && one.defines.contains(value) && one.uses.contains(value))),
        );
    }
    let mut free: IndexMap<u32, f64> = IndexMap::default();
    for (at, (into, out_of)) in moves {
        if impure.contains(&into)
            || impure.contains(&out_of)
            || live.get(&into).zip(live.get(&out_of)).is_some_and(|(one, other)| one.overlaps(other))
        {
            continue;
        }
        let each = busy.block(at);
        for value in [into, out_of] {
            *free.entry(value).or_insert(0.0) += each;
        }
    }
    let mut live = live;
    for (value, found) in &free {
        if let Some(one) = live.get_mut(value) {
            one.weight = _max(0.0, one.weight - found / one.spill_size() as f64);
        }
    }
    live
}

/// Frame traffic inside loops by cause, weighted by loop depth: the spiller's
/// reloads, stores and rematerializations, and the frame operands of x87 and
/// other instructions. The allocator can reach only the first three.
pub fn traffic_by_cause(body: &LirBody) -> std::collections::BTreeMap<&'static str, f64> {
    let deep = ranges::depths(body);
    let busy = Frequency::of(body);
    let mut out = std::collections::BTreeMap::new();
    for block in &body.blocks {
        let depth = deep.get(&block.at).copied().unwrap_or(0);
        if depth == 0 {
            continue;
        }
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            let frame = what
                .dests
                .iter()
                .chain(&what.sources)
                .any(
                    |place| matches!(
                        place,
                        Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                    ),
                );
            let float = what.op.is_x87();
            let cause = match () {
                _ if one.spill_reload => "reload",
                _ if one.spill_store => "store",
                _ if one.rematerialized => "remat",
                _ if frame && float => "x87-frame",
                _ if frame => "int-frame",
                _ => continue,
            };
            *out.entry(cause).or_insert(0.0) += busy.block(block.at);
        }
    }
    out
}

/// Spilled invariant frame loads that repeatedly form addresses.
fn _retainable_bases(
    body: &LirBody,
    spilled: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    if spilled.is_empty() {
        return BTreeSet::new();
    }
    let stable: BTreeSet<u32> = spiller::_stable_loads(body, spilled).keys().copied().collect();
    if stable.is_empty() {
        return BTreeSet::new();
    }
    let busy = Frequency::of(body);
    let mut references: IndexMap<u32, f64> = IndexMap::default();
    for block in &body.blocks {
        let weight = busy.block(block.at);
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
                *references.entry(*value).or_insert(0.0) += weight;
            }
        }
    }
    let repeated: BTreeSet<u32> =
        references.iter().filter(|(_value, weight)| **weight > 1.0).map(|(value, _)| *value).collect();
    stable.intersection(&repeated).copied().collect()
}

/// Dying word indexes in a natural loop that actually uses `bases`.
fn _scoped_foldable_indexes(
    body: &LirBody,
    bases: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    if bases.is_empty() {
        return BTreeSet::new();
    }
    let blocks: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut indexes: BTreeSet<u32> = BTreeSet::new();
    for found in loops::loops(&body.blocks, Some(body.entry)) {
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
        indexes
            .extend(cells.iter().filter(|cell| cell.scale == 1).filter_map(|cell| cell.index.map(|index| index.value)));
    }
    spiller::foldable_indexes(body, &indexes)
}

/// How much of a spilled read disappears when it becomes a memory operand, for
/// each form an instruction can fold in: a profile's prices, looked up once for
/// a rebuild and not once per instruction.
struct FoldDiscounts {
    alu: f64,
    imul32: f64,
}

impl FoldDiscounts {
    fn of(profile: &Profile) -> Self {
        Self { alu: Self::form(profile, "alu_rr", "alu_rm"), imul32: Self::form(profile, "imul_r32", "imul_m32") }
    }

    fn form(
        profile: &Profile,
        register: &str,
        memory: &str,
    ) -> f64 {
        if ![register, memory, "mov_rm"].iter().all(|form| profile.prices(form)) {
            return 0.0;
        }
        let load = profile.cost("mov_rm").expect("priced above");
        if load <= 0 {
            return 0.0;
        }
        let remainder =
            0.max(profile.cost(memory).expect("priced above") - profile.cost(register).expect("priced above"));
        _max(0.0, _min(1.0, 1.0 - remainder as f64 / load as f64))
    }

    /// What `one` folds, by its form: nothing for one that has none.
    fn of_insn(
        &self,
        one: &Insn,
    ) -> f64 {
        let Some(what) = &one.what else {
            return 0.0;
        };
        match (what.op, what.name.as_deref(), what.sources.as_slice()) {
            (Operation::Binary | Operation::Compare, name, _) => {
                if matches!(name, Some("add" | "sub" | "and" | "or" | "xor" | "cmp")) { self.alu } else { 0.0 }
            }
            (Operation::Multiply, Some("imul"), [Loc::Held(first), Loc::Held(second)])
                if first.width == 4 && second.width == 4 =>
            {
                self.imul32
            }
            _ => 0.0,
        }
    }
}

/// The old per-instruction form of `FoldDiscounts`, which `LLRM_CHECK_FOLDS=1`
/// holds it to.
pub(crate) fn _fold_discount(
    one: &Insn,
    profile: &Profile,
) -> f64 {
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
pub(crate) fn _fold_priced(
    body: &LirBody,
    live: IndexMap<u32, Interval>,
    profile: &Profile,
    busy: &Frequency,
) -> IndexMap<u32, Interval> {
    let discounts = FoldDiscounts::of(profile);
    let check = llrm_support::env_set("LLRM_CHECK_FOLDS");
    let mut free: IndexMap<u32, f64> = IndexMap::default();
    for block in &body.blocks {
        let each = busy.block(block.at);
        for one in &block.insns {
            let discount = discounts.of_insn(one);
            if check {
                assert!(
                    discount == _fold_discount(one, profile),
                    "{}: a fold discount differs from the per-instruction lookup",
                    body.name
                );
            }
            if discount == 0.0 {
                continue;
            }
            // Only the second source of the pair can fold, once for each time
            // the instruction reads it.
            let Some((_, right)) = spiller::folded_pair(one, true) else { continue };
            for value in &one.uses {
                if *value == right.value {
                    *free.entry(*value).or_insert(0.0) += each * discount;
                }
            }
        }
    }
    let mut live = live;
    for (value, found) in &free {
        if let Some(one) = live.get_mut(value).filter(|one| one.weight != INF) {
            one.weight = _max(0.0, one.weight - found / one.spill_size() as f64);
        }
    }
    live
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
pub fn applied(
    body: &LirBody,
    got: &Assignment,
    classes: &RegisterClasses,
) -> Result<LirBody, Error> {
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
                Some(|one: &Arc<Insn>| match _placed_for_rewrite(one, held, classes) {
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
    Ok(LirBody {
        origin: body.origin.clone(),
        pins: body.pins.clone(),
        ordered: body.ordered,
        ..body.with_blocks(blocks)
    })
}

/// Delete unused allocator copies before physical identity loses their use
/// graph.
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
            body.blocks
                .iter()
                .flat_map(|block| &block.insns)
                .flat_map(|one| one.requires.iter().map(|(held, _)| held.value)),
        );
        used.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.phis)
                .flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)),
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
        body = body.with_blocks(
            body.blocks
                .iter()
                .map(|block| {
                    block.with_insns(
                        block.insns.iter().filter(|one| !dead.contains(&ranges::key(one))).cloned().collect(),
                    )
                })
                .collect(),
        );
    }
}

/// Retain byte ownership without requiring an encodable register self-copy.
fn _identity_anchor(one: Arc<Insn>) -> Arc<Insn> {
    if one.group.is_some() || !_pointless(&one) {
        return one;
    }
    let mut made = (*one).clone();
    made.what = Some(lir::inert());
    Arc::new(made)
}

/// An identity that is not still owned by the parallel-copy scheduler.
fn _discardable_identity(one: &Insn) -> bool {
    one.group.is_none() && _pointless(one)
}

/// Place one instruction without discarding an inserted definition.
fn _placed_for_rewrite(
    one: &Arc<Insn>,
    held: &IndexMap<u32, Register>,
    classes: &RegisterClasses,
) -> Result<Arc<Insn>, Unplaced> {
    let placed = _placed(one, held, classes)?;
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
    matches!(
        (&what.dests[0], &what.sources[0]),
        (Loc::Reg(into), Loc::Reg(out_of)) if into.register == out_of.register
    )
}

fn _placed(
    one: &Arc<Insn>,
    held: &IndexMap<u32, Register>,
    classes: &RegisterClasses,
) -> Result<Arc<Insn>, Unplaced> {
    let Some(what) = &one.what else {
        return Ok(Arc::clone(one));
    };
    let dests = what.dests.iter().map(|x| _settled(x, held, classes)).collect::<Result<Vec<_>, _>>()?;
    let sources = what.sources.iter().map(|x| _settled(x, held, classes)).collect::<Result<Vec<_>, _>>()?;
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
fn _settled(
    place: &Loc,
    held: &IndexMap<u32, Register>,
    classes: &RegisterClasses,
) -> Result<Loc, Unplaced> {
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
                && classes.word_indexes.contains(&base_register)
                && classes.word_bases.contains(&index_register)
            {
                std::mem::swap(&mut base_register, &mut index_register);
            }
            return Ok(Loc::Mem(Mem { through: base_register, index_through: index_register, ..cell.clone() }));
        }
        if let Some(base) = cell.base {
            let Some(register) = held.get(&base.value) else {
                return Ok(place.clone());
            };
            let placed = target::named(*register, i64::from(base.width).max(2));
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

    use super::*;
    use crate::analysis::intervals::Segment;
    use crate::backend::regalloc_input::{before_regalloc, through};
    use crate::backend::{parcopy, select, verify};
    use crate::model::ir::Imm;
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

    fn reg(
        register: Register,
        width: u32,
    ) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn block(
        at: i64,
        insns: Vec<Insn>,
    ) -> LirBlock {
        LirBlock::new(at, insns.into_iter().map(Arc::new).collect())
    }

    fn body_of(
        name: &str,
        entry: i64,
        insns: Vec<Insn>,
    ) -> LirBody {
        LirBody::new(name, entry, vec![block(entry, insns)], IndexMap::default(), IndexMap::default())
    }

    /// lru bas `BENCHLRU&` at -Os: an argument loaded in the entry block and
    /// pushed after two loops sat in ax the whole way (`mov ax,[bp+6]` ...
    /// `push ax`), where re-reading its cell at the push is the same memory
    /// operand and one instruction fewer; the registers it held took the
    /// constants' (+1 instruction, +3 B).
    #[test]
    fn test_a_load_read_once_by_a_push_is_read_at_the_push() {
        let cell = Mem { offset: 0, disp_width: 1, ..Mem::new(Some(Addr::new(Space::Frame, 6)), 2) };
        let at = |at: i64, what: Semantics, defines: Vec<u32>, uses: Vec<u32>| {
            Insn::new(at, Some((at, at + 1)), Some(what), defines, uses)
        };
        let load = at(0, semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(cell)]), vec![1], vec![]);
        let enter = at(1, semantics(Operation::Jump, "jmp", vec![], vec![]), vec![], vec![]);
        // A counted loop between the load and the push: the push's block runs
        // as often as the load's.
        let count = at(
            10,
            semantics(Operation::Binary, "add", vec![held(2, 2)], vec![held(2, 2), imm(1, 2)]),
            vec![2],
            vec![2],
        );
        let test = at(11, semantics(Operation::Compare, "cmp", vec![], vec![held(2, 2), imm(3, 2)]), vec![], vec![2]);
        let back = at(12, semantics(Operation::Branch, "jne", vec![], vec![]), vec![], vec![]);
        let push = at(20, semantics(Operation::Push, "push", vec![], vec![held(1, 2)]), vec![], vec![1]);
        let ret = at(21, semantics(Operation::Return, "retf", vec![], vec![]), vec![], vec![]);
        let zero = at(2, semantics(Operation::Move, "mov", vec![held(2, 2)], vec![imm(0, 2)]), vec![2], vec![]);
        let (count_again, test_again, back_again) = (
            at(
                15,
                semantics(Operation::Binary, "add", vec![held(2, 2)], vec![held(2, 2), imm(1, 2)]),
                vec![2],
                vec![2],
            ),
            at(16, semantics(Operation::Compare, "cmp", vec![], vec![held(2, 2), imm(7, 2)]), vec![], vec![2]),
            at(17, semantics(Operation::Branch, "jne", vec![], vec![]), vec![], vec![]),
        );
        let (mut first, mut looping, mut again, mut last) = (
            block(0, vec![load, zero, enter]),
            block(10, vec![count, test, back]),
            block(15, vec![count_again, test_again, back_again]),
            block(20, vec![push, ret]),
        );
        first.succ = vec![10];
        looping.succ = vec![10, 15];
        again.succ = vec![15, 20];
        last.succ = vec![];
        let mut body =
            LirBody::new("t", 0, vec![first, looping, again, last], IndexMap::default(), IndexMap::default());
        body.loop_trip_counts = vec![(10, 3), (15, 7)];
        let got = _through_regalloc(body, &[]);
        let loads_in = |at: i64| {
            got.blocks.iter().find(|one| one.at == at).expect("a block").insns.iter().any(|one| {
                one.what
                    .as_ref()
                    .is_some_and(|what| matches!(what.sources.as_slice(), [Loc::Mem(_)]) && what.op == Operation::Move)
            })
        };
        assert!(!loads_in(0), "the load stayed in the entry block: {:?}", got.blocks[0].insns);
        assert!(loads_in(20), "the load is not at the push");
    }

    /// The first version of reading loads at their use ran once per allocation,
    /// and an allocation that spills runs the base and each candidate
    /// shape: deedlines compiled 2% slower. The scan is a fact of the body:
    /// once.
    #[test]
    fn test_loads_read_at_their_use_are_found_once_per_body_however_many_candidates_run() {
        // Volatile loads: not made again, so ten of them live at once spill.
        let mut insns: Vec<Insn> = (1..=10_u32)
            .map(|value| {
                let cell = Mem::new(Some(Addr::new(Space::Segment, i64::from(value) * 2)), 2);
                Insn {
                    volatile: true,
                    ..Insn::new(
                        i64::from(value) * 4,
                        Some((0, 1)),
                        Some(semantics(Operation::Move, "mov", vec![held(value, 2)], vec![Loc::Mem(cell)])),
                        vec![value],
                        vec![],
                    )
                }
            })
            .collect();
        for value in 2..=10_u32 {
            let what = semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), held(value, 2)]);
            insns.push(Insn::new(100 + i64::from(value), Some((100, 101)), Some(what), vec![1], vec![1, value]));
        }
        let before = spiller::folded_read_scans();
        let got = _through_regalloc(_one_block(insns), &[]);
        assert!(
            got.insns().iter().any(|one| one.spill_reload || one.spill_store),
            "premise: the body spills, so candidates run"
        );
        assert_eq!(spiller::folded_read_scans() - before, 1);
    }

    fn _one_block(insns: Vec<Insn>) -> LirBody {
        body_of("one", 0, insns)
    }

    fn _mov(
        into: u32,
        value: i64,
        at: i64,
    ) -> Insn {
        Insn::new(
            at,
            Some((at, at + 2)),
            Some(semantics(Operation::Move, "mov", vec![held(into, 2)], vec![imm(value, 2)])),
            vec![into],
            vec![],
        )
    }

    fn _shl(
        result: u32,
        count: u32,
        at: i64,
    ) -> Insn {
        let what = semantics(Operation::Binary, "shl", vec![held(result, 2)], vec![held(result, 2), held(count, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![result], vec![result, count])
    }

    fn _named(
        body: &LirBody,
        name: &str,
    ) -> Vec<Semantics> {
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
        let mut phase = RegAlloc::new(
            None,
            Some(Rc::clone(&frame)),
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("a cpu");
        RegAlloc::transform(&mut phase, body).expect("allocates");
        assert!(!frame.borrow().slots.is_empty());
    }

    fn _through_regalloc(
        body: LirBody,
        pinned: &[(u32, Register)],
    ) -> LirBody {
        let pinned = pins(pinned);
        let frame = frames::of(&body, None, "", None).expect("a frame");
        let mut phase = RegAlloc::new(
            Some(&pinned),
            Some(Rc::new(RefCell::new(frame))),
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("a cpu");
        RegAlloc::transform(&mut phase, body).expect("allocates")
    }

    fn pins(pairs: &[(u32, Register)]) -> IndexMap<u32, Register> {
        pairs.iter().copied().collect()
    }

    fn values(items: &[u32]) -> BTreeSet<u32> {
        items.iter().copied().collect()
    }

    fn allocated(
        body: &LirBody,
        pinned: Option<&IndexMap<u32, Register>>,
    ) -> Result<Assignment, Error> {
        allocate(
            body,
            pinned,
            None,
            None,
            None,
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
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
        let what = semantics(Operation::Multiply, "imul", vec![held(1, 2), held(2, 2)], vec![held(1, 2), held(3, 2)]);
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
        let one = allocate(
            &_one_block(vec![_mov(1, 1, 0x100)]),
            None,
            None,
            None,
            Some(&preferred),
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
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
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
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
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");
        assert_eq!(got.r#where[&1], Register::EDX, "{:?}", got.r#where);
    }

    #[test]
    fn test_a_copy_placed_before_its_partner_moves_to_join_it() {
        // The longer source was placed first, blind to its copy; the copy's
        // destination then found that register held by a pinned value and
        // the copy stayed a mov.
        let add = |at: i64, value: u32| {
            Insn::new(
                at,
                Some((at, at + 2)),
                Some(semantics(Operation::Binary, "add", vec![held(value, 2)], vec![held(value, 2), imm(1, 2)])),
                vec![value],
                vec![value],
            )
        };
        let mut insns = vec![_mov(1, 1, 0x100)];
        insns.extend((0..4).map(|at| add(0x102 + 2 * at, 1)));
        insns.push(Insn::new(
            0x10a,
            Some((0x10a, 0x10c)),
            Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
            vec![2],
            vec![1],
        ));
        insns.push(_mov(3, 3, 0x10c));
        insns.push(Insn::new(
            0x10e,
            Some((0x10e, 0x110)),
            Some(semantics(Operation::Binary, "add", vec![held(2, 2)], vec![held(2, 2), held(3, 2)])),
            vec![2],
            vec![2, 3],
        ));
        let blind = allocate(
            &_one_block(insns.clone()),
            None,
            None,
            None,
            None,
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");
        let got = allocate(
            &_one_block(insns),
            Some(&pins(&[(3, blind.r#where[&1])])),
            None,
            None,
            None,
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");
        assert_eq!(_whole(got.r#where[&1]), _whole(got.r#where[&2]), "{:?}", got.r#where);
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

        let on_386 = allocate(
            &body,
            None,
            None,
            None,
            None,
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");
        let on_core = allocate(
            &body,
            None,
            None,
            None,
            None,
            "Core".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");

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
            let got = _settled(
                &Loc::Mem(was.clone()),
                &pins(&[(21, register)]),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("placed");
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
        let mut call = Insn::new(
            0xF0,
            Some((0xF0, 0xF3)),
            Some(semantics(Operation::Call, "call", vec![], vec![])),
            vec![21],
            vec![],
        );
        call.delivers = vec![(Held { value: 21, width: 2 }, Register::AX)];
        let mut body = _one_block(vec![call, _based_cell()]);
        body.pins = pins(&[(21, Register::EAX)]);
        let placed = _through_regalloc(body, &[(21, Register::EAX)]);
        let load = placed
            .insns()
            .into_iter()
            .find(|one| one.at == 0x100 && matches!(
                one.what.as_ref().map(|what| &what.sources[0]),
                Some(Loc::Mem(_))
            ));
        let through = cell_of(&load.expect("the load")).through;
        assert!(crate::backend::classes::RegisterClasses::m16().addressing.contains(&through), "{through:?}");
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
        let got = applied(
            &_one_block(vec![load]),
            &assignment(pins(&[(17, Register::BX)])),
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("applies");
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
        assert!(message.contains("both required in"), "{message}");
    }

    /// A base or index on m16 may be BX, SI, DI or BP; a value of any class
    /// fits in the rest. The queue took the longest range first whatever
    /// its class, so a short base found its three registers taken by wide
    /// values that could have been elsewhere: bench/quicksort +18%
    /// instructions against the search over shapes, which tried the
    /// base first (#944).
    #[test]
    fn test_a_value_few_registers_may_hold_goes_before_a_longer_one_any_may() {
        let span = |end: i64| Interval::new(1, vec![Segment { start: 0, end }]);
        let (short, long) = (span(4), span(400));
        assert!(
            _queue_priority(Some(&short), Stage::Assign, Some(4), 6)
                > _queue_priority(Some(&long), Stage::Assign, Some(6), 6)
        );
        assert!(
            _queue_priority(Some(&short), Stage::Assign, None, 6)
                < _queue_priority(Some(&long), Stage::Assign, None, 6),
            "no class: longest first"
        );
        assert!(
            _queue_priority(Some(&long), Stage::Assign, Some(4), 6)
                > _queue_priority(Some(&short), Stage::Assign, Some(4), 6),
            "one class: longest first"
        );
        assert!(
            _queue_priority(Some(&short), Stage::Assign, Some(1), 6)
                > _queue_priority(Some(&short), Stage::Assign, Some(4), 6),
            "the fewer registers, the sooner"
        );
    }

    #[test]
    fn test_a_hard_register_assignment_is_not_an_eviction_victim() {
        let kept = Interval { weight: 0.1, ..Interval::new(1, vec![Segment { start: 0, end: 4 }]) };
        let incoming = Interval { weight: 10.0, ..Interval::new(2, vec![Segment { start: 0, end: 4 }]) };
        let live: IndexMap<u32, Interval> = IndexMap::from_iter([(1, kept), (2, incoming.clone())]);
        let union = LiveUnion::of([(_whole(Register::DI), vec![1])], &live);
        let got = _evict(
            &incoming,
            &[Register::DI],
            &union,
            &live,
            &Masks::default(),
            &|_, _| false,
            &values(&[1]),
            4,
            None,
            None,
        );
        assert!(got.is_none(), "{got:?}");
    }

    /// Eviction only weighs the holders' spill costs, so a value whose register
    /// was held by something that could move, if a third value moved first,
    /// spilled (deedlines: 3600 weighted frame operands).
    #[test]
    fn test_a_register_whose_holder_moves_through_a_chain_is_taken_not_spilled() {
        let span = |value: u32, end: i64| {
            (value, Interval { weight: 1.0, ..Interval::new(value, vec![Segment { start: 0, end }]) })
        };
        let live: IndexMap<u32, Interval> = IndexMap::from_iter([span(1, 10), span(2, 4), span(3, 10)]);
        let choices = |value: u32| match value {
            1 => vec![Register::AX, Register::BX],
            2 => vec![Register::BX, Register::CX],
            _ => vec![Register::AX],
        };
        let mut union = LiveUnion::of([(_whole(Register::AX), vec![1]), (_whole(Register::BX), vec![2])], &live);
        let mut placed: IndexMap<u32, Register> = IndexMap::from_iter([(1, Register::AX), (2, Register::BX)]);
        let mut coloring = Coloring {
            union: &mut union,
            r#where: &mut placed,
            live: &live,
            masks: &Masks::default(),
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

    /// A failing recolor tries every register at every level: with a class one
    /// register larger it took 2000 tries, three times the compile time of
    /// x_ll_arith, for a search that nearly never succeeds (3 of 366 sessions
    /// on the 66 programs).
    #[test]
    fn test_a_recolor_that_fails_over_a_full_class_stays_within_its_budget() {
        let registers =
            [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI, Register::EBP];
        let live: IndexMap<u32, Interval> = (0..=registers.len() as u32)
            .map(|value| (value, Interval { weight: 1.0, ..Interval::new(value, vec![Segment { start: 0, end: 10 }]) }))
            .collect();
        let choices = |_: u32| registers.to_vec();
        let mut union = LiveUnion::of(
            registers.iter().enumerate().map(|(at, register)| (_whole(*register), vec![at as u32 + 1])),
            &live,
        );
        let mut placed: IndexMap<u32, Register> =
            registers.iter().enumerate().map(|(at, register)| (at as u32 + 1, *register)).collect();
        let mut coloring = Coloring {
            union: &mut union,
            r#where: &mut placed,
            live: &live,
            masks: &Masks::default(),
            order: &choices,
            width: &|_| 4,
            fenced: &BTreeSet::new(),
            budget: Coloring::BUDGET,
            stack: Vec::new(),
        };
        assert!(!coloring.recolor(0, 0, &mut BTreeSet::new()));
        assert!(Coloring::BUDGET - coloring.budget <= 256, "{} tries", Coloring::BUDGET - coloring.budget);
    }

    /// A rewrite can leave a value's class narrower than the register it
    /// holds: kept, a pointer in AX reached the encoder as `mov [di+si]`.
    #[test]
    fn test_an_assignment_outside_its_rewritten_class_is_evicted() {
        let cell = Mem { base: Some(Held { value: 1, width: 2 }), ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2) };
        let body = _one_block(vec![_mov(1, 5, 0), _load(4, 2, cell, vec![1])]);
        let profile = targets::profile(ProfileOrName::from("386")).expect("a profile");
        let facts = Facts::of(
            &body,
            profile,
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            &Frequency::of(&body),
        );
        let union = LiveUnion::of([(_whole(Register::AX), vec![1])], &facts.live);
        let placed = IndexMap::from_iter([(1, Register::AX)]);
        assert_eq!(_overlapping(&union, &placed, &facts), BTreeSet::from([1]));
    }

    /// A phi's argument is read at the end of the predecessor it names: a value
    /// only a phi reads was live nowhere, and a next-use analysis saw it dead.
    #[test]
    fn test_a_phi_argument_is_live_out_of_its_predecessor() {
        let define = Insn::new(
            0,
            Some((0, 2)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(5, 2)])),
            vec![1],
            vec![],
        );
        let mut first = block(0, vec![define]);
        first.succ = vec![0x10];
        let mut second = block(0x10, vec![]);
        second.phis = vec![crate::model::lir::Phi { result: 2, incoming: vec![(0, 1)] }];
        let body = LirBody::new("phi", 0, vec![first, second], IndexMap::default(), IndexMap::default());
        let (live_in, live_out) = live(&body);
        assert!(live_out[&0].contains(&1), "the argument leaves its predecessor: {live_out:?}");
        assert!(!live_in[&0x10].contains(&1) && !live_in[&0x10].contains(&2));
    }

    /// A failed recoloring must leave every holder where it was.
    #[test]
    fn test_a_recoloring_that_fails_restores_every_holder() {
        let span = |value: u32, end: i64| {
            (value, Interval { weight: 1.0, ..Interval::new(value, vec![Segment { start: 0, end }]) })
        };
        let live: IndexMap<u32, Interval> = IndexMap::from_iter([span(1, 10), span(2, 4), span(3, 10)]);
        let choices = |value: u32| match value {
            1 => vec![Register::AX, Register::BX],
            2 => vec![Register::BX],
            _ => vec![Register::AX],
        };
        let mut union = LiveUnion::of([(_whole(Register::AX), vec![1]), (_whole(Register::BX), vec![2])], &live);
        let mut placed: IndexMap<u32, Register> = IndexMap::from_iter([(1, Register::AX), (2, Register::BX)]);
        let mut coloring = Coloring {
            union: &mut union,
            r#where: &mut placed,
            live: &live,
            masks: &Masks::default(),
            order: &choices,
            width: &|_| 2,
            fenced: &BTreeSet::new(),
            budget: Coloring::BUDGET,
            stack: Vec::new(),
        };
        assert!(!coloring.recolor(3, 0, &mut BTreeSet::new()));
        assert_eq!(placed, IndexMap::from_iter([(1, Register::AX), (2, Register::BX)]));
        assert_eq!(union.holders(&_whole(Register::AX)), vec![1]);
        assert_eq!(union.holders(&_whole(Register::BX)), vec![2]);
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
        let pinned: IndexMap<u32, Register> =
            (1..7).zip(crate::backend::classes::RegisterClasses::m16().available.clone()).collect();
        let got = allocate(
            &_one_block(insns),
            Some(&pinned),
            Some(&values(&[7])),
            None,
            None,
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let message = unplaced(got);
        assert!(message.contains("7 values that cannot be spilled are live at one point"), "{message}");
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
        let used = Insn::new(
            3,
            Some((3, 4)),
            Some(semantics(Operation::Push, "push", vec![], vec![held(2, 2)])),
            vec![],
            vec![2],
        );
        LirBody { pins: pins(&[(2, Register::EDI)]), ..body_of("call-copy", 0, vec![define, copy, used]) }
    }

    /// Excess call copies grew SCREEN and contributed to E1M1's BASIC error 14.
    #[test]
    fn test_call_input_is_computed_in_its_available_required_register() {
        let body = call_copy();
        let assignment = allocated(&body, Some(&body.pins)).expect("allocates");
        let result = applied(&body, &assignment, &crate::backend::classes::RegisterClasses::m16()).expect("applies");
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
        // A loop sum two copies from the return's AX lost AX to a counter
        // placed first; the exit then moved it.
        let mov = |at: i64, dest: u32, source: Loc| {
            let uses = if let Loc::Held(one) = &source { vec![one.value] } else { vec![] };
            Insn::new(
                at,
                Some((at, at + 1)),
                Some(semantics(Operation::Move, "mov", vec![held(dest, 2)], vec![source])),
                vec![dest],
                uses,
            )
        };
        let push = |at: i64, value: u32| {
            Insn::new(
                at,
                Some((at, at + 1)),
                Some(semantics(Operation::Push, "push", vec![], vec![held(value, 2)])),
                vec![],
                vec![value],
            )
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
        let mut call = Insn::new(
            1,
            Some((1, 6)),
            Some(semantics(Operation::Call, "call", vec![], vec![one.clone()])),
            vec![1],
            vec![1],
        );
        call.clobbers = BTreeSet::from([Register::ESI]);
        let after =
            Insn::new(6, Some((6, 7)), Some(semantics(Operation::Push, "push", vec![], vec![one])), vec![], vec![1]);
        let body = body_of("call", 0, vec![before.clone(), call.clone(), after.clone()]);
        let si = pins(&[(1, Register::SI)]);
        let assigned = allocate(
            &body,
            Some(&si),
            Some(&values(&[1])),
            None,
            None,
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");
        assert!(assigned.spilled.is_empty() && assigned.r#where[&1] == Register::SI);

        let surviving = body_of("call", 0, vec![before, Insn { defines: vec![], ..call }, after]);
        assert_eq!(allocated(&surviving, Some(&si)).expect("allocates").spilled, values(&[1]));
    }

    // --------------------------------------------- tests/test_address_roles.py

    fn _instruction(
        at: i64,
        what: Semantics,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Insn {
        Insn::new(at, Some((at, at)), Some(what), defines, uses)
    }

    fn _frame_load(
        at: i64,
        value: u32,
        displacement: i64,
    ) -> Insn {
        let cell = Mem { through: Register::BP, ..Mem::new(Some(Addr::new(Space::Frame, displacement)), 2) };
        _instruction(
            at,
            semantics(Operation::Move, "mov", vec![held(value, 2)], vec![Loc::Mem(cell)]),
            vec![value],
            vec![],
        )
    }

    fn _load(
        at: i64,
        into: u32,
        cell: Mem,
        uses: Vec<u32>,
    ) -> Insn {
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
        let placed = applied(&body, &assignment, &crate::backend::classes::RegisterClasses::m16()).expect("applies");
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

        let found =
            classes(&body, &BTreeSet::new(), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());

        assert_eq!(found[&1], crate::backend::classes::RegisterClasses::m16().word_indexes);
        assert_eq!(found[&2], crate::backend::classes::RegisterClasses::m16().word_bases);
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

        let confined =
            classes(&body, &values(&[1]), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());

        assert_eq!(confined[&1], crate::backend::classes::RegisterClasses::m16().word_indexes);
        assert_eq!(confined[&2], crate::backend::classes::RegisterClasses::m16().word_bases);
    }

    /// The allocator ranked its candidate bodies counting every instruction,
    /// anchors that print nothing among them: a body with more anchors cost
    /// more than one with more code, and the run, which counts what prints,
    /// said the reverse.
    #[test]
    fn test_anchors_that_print_nothing_do_not_cost_the_candidate() {
        let anchor = |at| _instruction(at, semantics(Operation::Nothing, "", vec![], vec![]), vec![], vec![]);
        let body = body_of(
            "anchored",
            0,
            vec![
                anchor(1),
                _mov(1, 1, 2),
                anchor(3),
                anchor(4),
                Insn { what: Some(semantics(Operation::Return, "ret", vec![], vec![])), ..anchor(5) },
            ],
        );
        assert_eq!(_emitted(&body), 2.0);
    }

    #[test]
    fn test_32_bit_secondary_base_is_not_confined_to_16_bit_address_registers() {
        let cell = Mem { base: Some(Held { value: 1, width: 4 }), ..Mem::new(Some(Addr::new(Space::Far, 0)), 2) };
        let body = body_of("secondary-base-class", 0, vec![_load(2, 2, cell, vec![1])]);

        assert!(
            !classes(&body, &BTreeSet::new(), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16())
                .contains_key(&1)
        );
    }

    // ------------------------------------------------------ tests/test_lir.py

    // Python's cell has the string address "[bx]", which Rust cannot hold.
    fn _celled(
        base: Option<Held>,
        through: Register,
    ) -> LirBody {
        let at = 0x100;
        let cell = Mem { through, base, ..Mem::new(None, 2) };
        let what = semantics(Operation::Move, "mov", vec![held(30, 2)], vec![Loc::Mem(cell)]);
        let uses = base.map(|one| vec![one.value]).unwrap_or_default();
        _one_block(vec![Insn::new(at, Some((at, at + 2)), Some(what), vec![30], uses)])
    }

    #[test]
    fn test_an_address_value_takes_the_class_a_base_register_must_be_in() {
        for through in [Register::BX, Register::SI] {
            let got = classes(
                &_celled(Some(Held { value: 21, width: 2 }), through),
                &BTreeSet::new(),
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16(),
            );
            assert_eq!(
                got.get(&21),
                Some(&crate::backend::classes::RegisterClasses::m16().addressing),
                "through={through:?}: {:?}",
                got.get(&21)
            );
        }
    }

    #[test]
    fn test_an_unbased_cell_confines_no_value() {
        assert!(
            !classes(
                &_celled(None, Register::BX),
                &BTreeSet::new(),
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .contains_key(&21)
        );
    }

    // ------------------------------------- tests/test_dead_call_deliveries.py

    #[test]
    fn test_dead_call_result_does_not_create_an_undefined_copy() {
        let mut call =
            Insn::new(0, Some((0, 3)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![1], vec![]);
        call.delivers = vec![(Held { value: 1, width: 2 }, Register::AX)];
        let (narrow, pinned) = narrowed(&body_of("dead result", 0, vec![call]), &pins(&[(1, Register::EAX)]));
        let (lowered, _fixed) =
            constrain::constrained(&narrow, Some(&pinned), &crate::backend::classes::RegisterClasses::m16());
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

            let placed = applied(
                &body,
                &assignment(pins(&[(1, Register::AX), (2, Register::AX)])),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("applies");

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
        let consumed = Insn::new(
            2,
            Some((2, 2)),
            Some(semantics(Operation::Push, "push", vec![], vec![held(2, 2)])),
            vec![],
            vec![2],
        );
        let body = LirBody { inputs: values(&[1]), ..body_of("parallel-identity", 1, vec![identity, consumed]) };
        let assignment = assignment(pins(&[(1, Register::AX), (2, Register::AX)]));

        let placed = applied(&body, &assignment, &crate::backend::classes::RegisterClasses::m16()).expect("applies");
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
            classes(&body, &BTreeSet::new(), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16())[&1],
            BTreeSet::from([Register::AX, Register::BX, Register::CX, Register::DX])
        );
    }
    /// Ten arrays walked by pointer: the loop preheader's parallel copy keeps
    /// a rematerialized `lea` live across the whole group, longer than a
    /// reload's range, so it was spillable; spilling it made the same `lea`
    /// again, a new value every pass, and compilation did not finish (#104).
    #[test]
    fn test_a_reload_stretched_by_a_parallel_copy_is_not_spilled_again() {
        let (body, phases) = before_regalloc("walks10.ll", "_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum", "386");
        let longest =
            body.blocks.iter().map(|one| one.insns.iter().filter(|insn| insn.group.is_some()).count()).max().unwrap();
        assert!(longest >= 5, "premise: a parallel copy of {longest} insns stretches what is live across it");

        let before = splitkit::_next_value(&body);
        let after = through(body, phases).expect("allocates");
        let made = splitkit::_next_value(&after) - before;
        assert!(made < 1000, "{made} values made for ten walks");
    }
}
