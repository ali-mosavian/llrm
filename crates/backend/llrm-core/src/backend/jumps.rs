//! Orders the blocks of an allocated body and removes the jumps and blocks the
//! order makes redundant; merges and copies tails.
//!
//! # How it works
//! Blocks are first written with every fall-through as an explicit jump
//! (`_explicit`), which makes any order correct. `optimized` then runs these
//! steps on the body.
//!
//! 1. `_hoisted` moves an edge block's no-byte anchors up before the branch
//!    into it, so the edge block becomes a bare jump that threading can skip.
//! 2. `_placed` orders the blocks as LLVM's `MachineBlockPlacement` does
//!    (`placement`): chains joined along the likelier edges, loops innermost
//!    first with their top and exit chosen and rotated, trellises, and a short
//!    block copied into the blocks that jump to it where that saves taken
//!    branches. See `docs/architecture/jumps-placement-check.md`.
//! 3. `threaded` applies local changes until none applies (`_step`): a jump to
//!    a block that only jumps goes to its target (`_through`; a loop header
//!    with a measured trip count is kept), a jump to the next block is dropped,
//!    `jcc L; jmp M` where `L` is next becomes `j!cc M`, and a `jcc` over a
//!    block that only jumps is inverted. Unreachable blocks go.
//! 4. `tailmerge::merged` (LLVM's `BranchFolder::TailMergeBlocks`) gives the
//!    blocks that share the last instructions one copy of them, splitting a
//!    block where no one holds only the tail; `machinedce::eliminated` then
//!    removes what the merge made dead (a compare nothing selects on), and the
//!    body is threaded again. Both repeat until nothing merges.
//! 5. `inverted` flips each `jcc L; jmp M` the copying left where `L` is next;
//!    where neither way is next, it makes the likelier way the conditional
//!    jump, so the trip that goes on pays one taken branch, not a branch not
//!    taken and a jump (the loop that went round by `je exit; jmp top`).
//!
//! ```text
//!   before                       after (placed, a guard copy of the test)
//!   test: cmp; jge exit          cmp; jge exit
//!   body: ...; jmp test          body: ...; cmp; jl body
//!   exit: ...                    exit: ...
//! ```
//!
//! `duplicated_returns` is separate and runs later, from `assemble`, because
//! the frame teardown is emitted around `ret` and its size is known only to
//! `masm`. A block that only returns (no calls, nothing source-owned, within
//! `COPY_BB_INSNS` unconditional jumps of bytes, gcc's
//! `max-grow-copy-bb-insns`) is copied into each parent that is its only
//! predecessor edge, where the bytes saved are more than the extra copies
//! cost.
//!
//! # Example
//! The `Scheduler` stage ends with the loop entered by an explicit jump.
//! `ControlFlow` places the blocks and drops the jump to the block that
//! follows:
//!
//! ```lir
//! === Scheduler
//!     block 0x0000
//!       ; ...
//!       0x0003  AX := mov -7  ; inserted
//!       0x0003  jmp   ; inserted
//!     block 0x0004
//!       ; ...
//!       0x0008  AX := inc AX  ; inserted
//!       0x000a  jne   ; inserted
//! === ControlFlow
//!     block 0x0000
//!       ; ...
//!       0x0003  AX := mov -7  ; inserted
//!     block 0x0004
//!       ; ...
//!       0x0008  AX := inc AX  ; inserted
//!       0x000a  jne   ; inserted
//! ```
//!
//! From `ISEL_DUMP=1` of a nib program (a struct method with a `while` loop,
//! called from `main`) at `-m16 -O2`, stages `Scheduler` and `ControlFlow`.
//!
//! # Runs
//! `ControlFlow` is the last machine phase, because this physical order
//! decides which explicit edge is the fall-through. `jumps` shows as the
//! phase name. A body in source order (`source_order`) is only made explicit;
//! at -O0 (`Profile::optimized` off) every body is, and its redundant jumps are
//! dropped, as gcc's and clang's -O0 place no blocks.
//! `LLRM_DEBUG=blocks` prints each block's frequency and size afterwards.
//! `duplicated_returns` runs once per procedure in `assemble::assembled_by`
//! (`lir duplicated returns`).
//!
//! # See also
//! LLVM `BranchFolding` and `MachineBlockPlacement`; gcc `cleanup_cfg`,
//! `bb-reorder` and `copy_bb_p`.

mod placement;
mod tailmerge;

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::dataflow::{self, Direction};
use crate::analysis::frequency::Frequency;
use crate::analysis::loops as loopy;
use crate::backend::layout::_OPPOSITE;
use crate::backend::{cpu, machinedce, masm, select};
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{self, Block, Insn, LirBlock, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::hash::HashSet;
use crate::support::hash::{IndexMap, IndexSet};

/// Python's AttributeError on `None.op`: `_real` keeps semantics-less markers.
const NO_OP: &str = "'NoneType' object has no attribute 'op'";

/// Settle allocated tails and edges after every machine-shaping phase.
/// `size`: blocks placed for short code, every block counted once, rather
/// than by their estimated frequencies (-Os).
pub struct ControlFlow<'a> {
    pub cpu: &'a cpu::Profile,
}

impl LIRTransform for ControlFlow<'_> {
    fn class_name(&self) -> &'static str {
        "ControlFlow"
    }

    fn name(&self) -> &str {
        "jumps"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        self.placed(&body).map_err(|error| error.0)
    }

    fn transform_raising(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Exception> {
        Ok(self.placed(&body)?)
    }
}

impl ControlFlow<'_> {
    /// `optimized`, its executed work on the `cost` channel.
    fn placed(
        &self,
        body: &LirBody,
    ) -> Result<LirBody, masm::Unprintable> {
        // -O0 keeps the blocks in source order and drops the jumps that order
        // makes redundant: no tails merged, no block copied, no trace chosen.
        let placed =
            if self.cpu.optimized { optimized(body, self.cpu)? } else { threaded(&body.with_blocks(_explicit(body)?)) };
        if crate::support::debug::enabled("blocks") {
            let frequency = crate::analysis::frequency::Frequency::of(&placed);
            for block in &placed.blocks {
                llrm_support::debug!(
                    "blocks",
                    "{} {:#06x} freq {} insns {}",
                    placed.name,
                    block.at,
                    frequency.block(block.at),
                    block.insns.len()
                );
            }
        }
        Ok(placed)
    }
}

/// The blocks placed, their tails merged, copied and rotated, and the jumps
/// the order makes redundant dropped.
pub fn optimized(
    body: &LirBody,
    cpu: &cpu::Profile,
) -> Result<LirBody, masm::Unprintable> {
    let size = cpu.size;
    let mut placed = threaded(&_placed(&_hoisted(body), size)?);
    // Tail duplication before the merge, as LLVM places with it: after, the
    // merge would give back what the copy gained.
    // `BranchFolder::OptimizeFunction` after placement: the tails merged, then
    // the branches optimised (`threaded`), until neither changes anything. A
    // merge can leave a compare nothing selects on; LLVM leaves it (T6).
    for _round in 0..std::cmp::max(1, placed.blocks.len() + placed.insns().len()) {
        let explicit = placed.with_blocks(_explicit(&placed)?);
        let Some(merged) = tailmerge::merged(&explicit, size)? else {
            break;
        };
        placed = machinedce::eliminated(threaded(&merged));
    }
    let done = inverted(&placed);
    // Every block leaves by a way the printer can read.
    #[cfg(test)]
    for block in &done.blocks {
        masm::_falls_to(block, &done.name).expect("a block with no instruction choosing its way out");
    }
    Ok(done)
}

/// A conditional branch taken to the block laid out next, followed by a jump:
/// the opposite branch to the jump's target, and no jump (LLVM's
/// `analyzeBranch` / `reverseBranchCondition` in BranchFolding). `_step` does
/// this while it places blocks; copying a tail afterwards (`duplicated`) makes
/// more of them. Where neither way is laid out next, the likelier one becomes
/// the conditional jump, so the trip that goes on pays one taken branch.
fn inverted(body: &LirBody) -> LirBody {
    let mut blocks = body.blocks.clone();
    let mut changed = false;
    let mut odds: Option<std::sync::Arc<crate::analysis::frequency::Frequency>> = None;
    for index in 0..blocks.len() {
        let (last, before) = _real_tail(&blocks[index]);
        let (Some(last), Some(branch)) = (last, before) else { continue };
        let (last_what, branch_what) = (last.what.as_ref().expect(NO_OP), branch.what.as_ref().expect(NO_OP));
        let after = blocks[index + 1..]
            .iter()
            .find(|next| !next.phis.is_empty() || next.insns.iter().any(|one| !one.is_meta()))
            .map(|next| next.at);
        let opposite = branch_what.name.as_deref().and_then(|name| _OPPOSITE.get(name));
        if last_what.op != Operation::Jump
            || branch_what.op != Operation::Branch
            || branch_what.indirect
            || branch_what.target.is_none()
            || last_what.target.is_none()
            || blocks[index].succ.len() != 2
        {
            continue;
        }
        let Some(opposite) = opposite else { continue };
        if branch_what.target != after {
            // Neither way is the block laid out next: the likelier one is the
            // conditional jump, so that the trip that goes on pays one taken
            // branch, not a branch not taken and a jump (the loop that went
            // round by `je exit; jmp top`).
            let (taken, onward) = (branch_what.target.expect("a target"), last_what.target.expect("a target"));
            let busy = odds.get_or_insert_with(|| crate::analysis::frequency::Frequency::of(body));
            if after == last_what.target || busy.edge(blocks[index].at, onward) <= busy.edge(blocks[index].at, taken) {
                continue;
            }
            let flipped = Arc::new(Insn {
                what: Some(Semantics {
                    name: Some((*opposite).to_owned()),
                    target: Some(onward),
                    ..branch_what.clone()
                }),
                ..(*branch).clone()
            });
            let jump = Arc::new(Insn {
                what: Some(Semantics { target: Some(taken), ..last_what.clone() }),
                ..(*last).clone()
            });
            let insns = blocks[index]
                .insns
                .iter()
                .map(|one| {
                    if Arc::ptr_eq(one, &branch) {
                        Arc::clone(&flipped)
                    } else if Arc::ptr_eq(one, &last) {
                        Arc::clone(&jump)
                    } else {
                        Arc::clone(one)
                    }
                })
                .collect();
            blocks[index] = blocks[index].with_insns(insns);
            changed = true;
            continue;
        }
        let flipped = Arc::new(Insn {
            what: Some(Semantics {
                name: Some((*opposite).to_owned()),
                target: last_what.target,
                ..branch_what.clone()
            }),
            ..(*branch).clone()
        });
        let insns = blocks[index]
            .insns
            .iter()
            .filter(|one| !Arc::ptr_eq(one, &last))
            .map(|one| if Arc::ptr_eq(one, &branch) { Arc::clone(&flipped) } else { Arc::clone(one) })
            .collect();
        blocks[index] = blocks[index].with_insns(insns);
        changed = true;
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// `one` copied where `beside` was, claiming no bytes.
fn _unowned(
    one: &Insn,
    beside: &Insn,
) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = one.clone();
    made.at = beside.at;
    made.covers = Some((at, at));
    Arc::new(made)
}

/// `body`'s blocks with every fall-through written as a jump, which makes any
/// order correct.
fn _explicit(body: &LirBody) -> Result<Vec<Block>, masm::Unprintable> {
    let mut explicit = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        let fall = masm::_falls_to(&block, &body.name)?;
        if let Some(fall) = fall {
            let at = block.insns.last().map_or(block.at, |last| last.at);
            let jump = Insn::new(
                at,
                Some((at, at)),
                Some(Semantics { name: Some("jmp".to_owned()), target: Some(fall), ..Semantics::new(Operation::Jump) }),
                Vec::new(),
                Vec::new(),
            );
            block.insns = block.insns.iter().cloned().chain([Arc::new(jump)]).collect();
        }
        explicit.push(block);
    }
    Ok(explicit)
}

/// `order` less each direct `jmp` to the block laid out next. `threaded` does
/// this too, but reads a block with a second branch in it (a copied tail's) as
/// leaving for three: such a block keeps its jump.
fn _without_jumps_to_next(order: Vec<Block>) -> Vec<Block> {
    let next: Vec<Option<i64>> = order.iter().skip(1).map(|block| Some(block.at)).chain([None]).collect();
    order
        .into_iter()
        .zip(next)
        .map(|(block, next)| {
            let real = _real(&block);
            let branches =
                real.iter().filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Branch)).count();
            let jump = real
                .last()
                .filter(
                    |one| one.what.as_ref().is_some_and(|what| {
                        what.op == Operation::Jump && !what.indirect && next.is_some() && what.target == next
                    }),
                );
            match jump {
                Some(jump) if branches <= 1 => {
                    let insns = block
                        .insns
                        .iter()
                        .map(|one| if Arc::ptr_eq(one, jump) { lir::bytes_only(one) } else { Arc::clone(one) })
                        .collect();
                    block.with_insns(insns)
                }
                _ => block,
            }
        })
        .collect()
}

/// Each block placed after the jump that reaches it, where no block is already.
///
/// The raise lays a C loop out as it reads, test first, so even entered at
/// its body the latch jumped back to the test every pass. Every fall-through
/// is written as a jump first, which makes any order correct; `threaded` then
/// drops the jumps the order made redundant and turns the test into one
/// branch back to the body.
pub fn placed(body: &LirBody) -> Result<LirBody, masm::Unprintable> {
    _placed(body, false)
}

/// The blocks in the order `MachineBlockPlacement` lays them out (see
/// `placement`).
fn _placed(
    body: &LirBody,
    size: bool,
) -> Result<LirBody, masm::Unprintable> {
    let explicit = _explicit(body)?;
    if body.source_order {
        return Ok(body.with_blocks(explicit));
    }
    let natural = loopy::loops(&explicit, Some(body.entry));
    let busy = Frequency::over(body, &explicit);
    let (blocks, odds) = placement::order(&explicit, body.entry, &body.odds, &busy, &natural, size);
    Ok(_reachable(&LirBody { odds, ..body.clone() }, blocks))
}

pub fn threaded(body: &LirBody) -> LirBody {
    let body = _hoisted(body);
    let mut body = _reachable(&body, body.blocks.clone());
    let protected: BTreeSet<i64> = body.loop_trip_counts.iter().map(|(header, _count)| *header).collect();
    let mut at = Places::of(&body);
    // One pass applies every change it meets, going on from the changed block
    // (LLVM's BranchFolding, gcc's `cleanup_cfg`); a change can open one in a
    // block before it, so passes repeat from the top until one finds none.
    let (mut from, mut visits) = (0, 0);
    loop {
        match _step(&mut body, &mut at, &protected, from, &mut visits) {
            Some(next) => from = next,
            None if from == 0 => break,
            None => from = 0,
        }
    }
    body.facts.0.bump_by("jump-visits", visits);
    // Threading can leave a branch whose two ways went to one block.
    _converged(&body)
}

/// An edge block's anchors moved up before the branch into it, where the other
/// way reads nothing they define.
///
/// A phi copy the allocation made an identity emits nothing, but its anchor
/// keeps a virtual definition on the edge, and `_passage` will not thread
/// around it: sum_three's loop ended `je exit; jmp top` for one. Before the
/// branch it defines the same value on both edges, harmless on the one that
/// never reads it; the edge block is then only a jump.
pub fn _hoisted(body: &LirBody) -> LirBody {
    let mut blocks = body.blocks.clone();
    let index: IndexMap<i64, usize> = blocks.iter().enumerate().map(|(position, block)| (block.at, position)).collect();
    let predecessors = _predecessors(&blocks);
    // Where each value is live on entry, from the values to their blocks: a
    // hoist moves the anchors' values alone, and the rest are not looked
    // at.
    let mut live: Option<(IndexMap<u32, BTreeSet<i64>>, Mentions)> = None;
    for position in 0..blocks.len() {
        let block = &blocks[position];
        let parents = predecessors.get(&block.at);
        // The cheap tests first: most blocks have no anchor or are not a lone
        // edge of their parent.
        if block.succ.len() != 1
            || parents.map_or(0, BTreeSet::len) != 1
            || !block.phis.is_empty()
            || !block.insns.iter().any(|one| _inert(one))
        {
            continue;
        }
        let (anchors, rest): (Vec<Arc<Insn>>, Vec<Arc<Insn>>) =
            block.insns.iter().cloned().partition(|one| _inert(one));
        if rest.iter().any(|one| one.what.as_ref().is_none_or(|what| what.op != Operation::Jump)) {
            continue;
        }
        let parent = *parents.and_then(|parents| parents.first()).expect("one predecessor");
        let before = &blocks[index[&parent]];
        if before.succ.len() != 2 || !before.succ.contains(&block.at) {
            continue;
        }
        let Some(&other) = before.succ.iter().find(|at| **at != block.at) else {
            continue;
        };
        let (reads, _) = live.get_or_insert_with(|| {
            let mut by_value: IndexMap<u32, BTreeSet<i64>> = IndexMap::default();
            for (at, values) in _live_values(&blocks) {
                for value in values {
                    by_value.entry(value).or_default().insert(at);
                }
            }
            (by_value, Mentions::of(&blocks))
        });
        if anchors
            .iter()
            .flat_map(|one| &one.defines)
            .any(|value| reads.get(value).is_some_and(|at| at.contains(&other)))
        {
            continue;
        }
        let mut cut = before.insns.len();
        while cut > 0
            && before.insns[cut - 1]
                .what
                .as_ref()
                .is_some_and(|what| matches!(what.op, Operation::Branch | Operation::Jump))
        {
            cut -= 1;
        }
        let insns = before.insns[..cut].iter().chain(&anchors).chain(&before.insns[cut..]).cloned().collect();
        let (at, before) = (index[&parent], before.with_insns(insns));
        blocks[at] = before;
        blocks[position] = blocks[position].with_insns(rest);
        // What is live of a value depends on that value's own definitions and
        // reads alone: only the values the moved anchors name changed
        // (a full dataflow of every value for each hoist was
        // most of this pass in a nest of copied loops).
        if let Some((held, mentions)) = live.as_mut() {
            let named: BTreeSet<u32> = anchors
                .iter()
                .flat_map(|one| {
                    one.defines
                        .iter()
                        .chain(&one.uses)
                        .copied()
                        .chain(one.delivers.iter().map(|(held, _)| held.value))
                        .chain(one.requires.iter().map(|(held, _)| held.value))
                })
                .collect();
            // The parent now mentions what the anchors name.
            for value in &named {
                mentions.add(*value, at);
            }
            for (value, entering) in mentions.live_in_of(&blocks, &named) {
                held.insert(value, entering);
            }
        }
        if llrm_support::env_flag!("LLRM_CHECK_JUMPS") {
            if let Some((held, _)) = &live {
                let fresh = _live_values(&blocks);
                assert!(
                    blocks.iter().all(|block| {
                        held.iter()
                            .filter(|(_, at)| at.contains(&block.at))
                            .map(|(value, _)| *value)
                            .collect::<BTreeSet<u32>>()
                            == fresh.get(&block.at).cloned().unwrap_or_default()
                    }),
                    "{}: the values live on entry to the blocks, patched past a hoist, are not those of a new solve",
                    body.name
                );
            }
        }
    }
    body.with_blocks(blocks)
}

/// An inserted anchor: no bytes and no machine effect, only a virtual
/// definition. What emits code or writes a register (the stackifier's `sahf`,
/// a pseudo that clobbers the flags) is an instruction.
fn _inert(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing)
        && one.inserted()
        && one.spread.is_empty()
        && !one.emits()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
}

/// The blocks that name each value, and the edges between blocks, which a hoist
/// does not change: what the liveness of a few values is found from.
struct Mentions {
    /// A superset of the positions of the blocks that name a value, as an
    /// instruction operand or a phi's.
    named: IndexMap<u32, Vec<usize>>,
    parents: IndexMap<i64, Vec<usize>>,
    /// The blocks whose end a phi reads each value at.
    carried: IndexMap<u32, BTreeSet<usize>>,
}

impl Mentions {
    fn of(blocks: &[Block]) -> Self {
        let position: IndexMap<i64, usize> = blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
        let mut parents: IndexMap<i64, Vec<usize>> = IndexMap::default();
        let mut named: IndexMap<u32, Vec<usize>> = IndexMap::default();
        let mut carried: IndexMap<u32, BTreeSet<usize>> = IndexMap::default();
        for (at, block) in blocks.iter().enumerate() {
            for to in &block.succ {
                parents.entry(*to).or_default().push(at);
            }
            for phi in &block.phis {
                named.entry(phi.result).or_default().push(at);
                for (from, value) in &phi.incoming {
                    if let Some(from) = position.get(from) {
                        carried.entry(*value).or_default().insert(*from);
                    }
                }
            }
            for one in &block.insns {
                let values = one
                    .defines
                    .iter()
                    .chain(&one.uses)
                    .copied()
                    .chain(one.delivers.iter().map(|(held, _)| held.value))
                    .chain(one.requires.iter().map(|(held, _)| held.value));
                for value in values {
                    let places = named.entry(value).or_default();
                    if places.last() != Some(&at) {
                        places.push(at);
                    }
                }
            }
        }
        Self { named, parents, carried }
    }

    fn add(
        &mut self,
        value: u32,
        at: usize,
    ) {
        self.named.entry(value).or_default().push(at);
    }

    /// Where each of `values` is live on entry to a block: `_live_values` for
    /// those alone. A value is live where a path from the start of the
    /// block reaches a read of it that no definition comes before; the
    /// block's own instructions decide, backwards from its end, by the earliest
    /// mention.
    fn live_in_of(
        &self,
        blocks: &[Block],
        values: &BTreeSet<u32>,
    ) -> IndexMap<u32, BTreeSet<i64>> {
        #[derive(Clone, Copy, PartialEq)]
        enum Transfer {
            Gen,
            Kill,
        }
        let mut found: IndexMap<u32, BTreeSet<i64>> = IndexMap::default();
        for &value in values {
            // Only the blocks that name the value are not passed through.
            let mut how: IndexMap<usize, Transfer> = IndexMap::default();
            for &at in self.named.get(&value).into_iter().flatten() {
                let block = &blocks[at];
                let mut mark = None;
                for one in block.insns.iter().rev() {
                    if one.defines.contains(&value) || one.delivers.iter().any(|(held, _)| held.value == value) {
                        mark = Some(Transfer::Kill);
                    }
                    if one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value) {
                        mark = Some(Transfer::Gen);
                    }
                }
                if block.phis.iter().any(|phi| phi.result == value) {
                    mark = Some(Transfer::Kill);
                }
                if let Some(mark) = mark {
                    how.insert(at, mark);
                }
            }
            let ends = self.carried.get(&value);
            let mut entering: BTreeSet<usize> = BTreeSet::new();
            let mut work: Vec<usize> = Vec::new();
            for &at in how.keys().chain(ends.into_iter().flatten()) {
                let live_out = ends.is_some_and(|set| set.contains(&at));
                if (how.get(&at) == Some(&Transfer::Gen) || (!how.contains_key(&at) && live_out)) && entering.insert(at)
                {
                    work.push(at);
                }
            }
            while let Some(at) = work.pop() {
                for &parent in self.parents.get(&blocks[at].at).into_iter().flatten() {
                    if !how.contains_key(&parent) && entering.insert(parent) {
                        work.push(parent);
                    }
                }
            }
            found.insert(value, entering.into_iter().map(|at| blocks[at].at).collect());
        }
        found
    }
}

/// Per block, the virtual values live on entry.
fn _live_values(blocks: &[Block]) -> IndexMap<i64, BTreeSet<u32>> {
    let by_at: IndexMap<i64, &Block> = blocks.iter().map(|block| (block.at, block)).collect();
    let mut carried: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    for block in blocks {
        for phi in &block.phis {
            for (at, value) in &phi.incoming {
                carried.entry(*at).or_default().insert(*value);
            }
        }
    }
    let nodes: Vec<&Block> = blocks.iter().collect();
    dataflow::solve(
        &nodes,
        Direction::Backward,
        |_| BTreeSet::new(),
        |at, into: &IndexMap<i64, BTreeSet<u32>>| {
            let mut live = carried.get(&at).cloned().unwrap_or_default();
            for to in &by_at[&at].succ {
                if let Some(after) = into.get(to) {
                    live.extend(after.iter().copied());
                }
            }
            live
        },
        |at, after| {
            let block = by_at[&at];
            let mut live = after.clone();
            for one in block.insns.iter().rev() {
                for value in one.defines.iter().copied().chain(one.delivers.iter().map(|(held, _)| held.value)) {
                    live.remove(&value);
                }
                live.extend(one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)));
            }
            for phi in &block.phis {
                live.remove(&phi.result);
            }
            live
        },
    )
    .output
}

/// The value `one` is renamed to, through every rename after it.
fn _root(
    rename: &IndexMap<u32, u32>,
    mut one: u32,
) -> u32 {
    while let Some(next) = rename.get(&one) {
        one = *next;
    }
    one
}

/// Redirect every explicit edge, remove duplicate blocks, and fold diamonds.
pub fn _redirected(
    body: &LirBody,
    redirect: &IndexMap<i64, i64>,
) -> LirBody {
    let target = |mut at: i64| -> i64 {
        let mut seen = HashSet::default();
        while redirect.contains_key(&at) && !seen.contains(&at) {
            seen.insert(at);
            at = redirect[&at];
        }
        at
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        if redirect.contains_key(&block.at) {
            continue;
        }
        let mut insns = Vec::new();
        for one in &block.insns {
            let mut one = Arc::clone(one);
            if let Some(what) = &one.what {
                if matches!(what.op, Operation::Branch | Operation::Jump) {
                    if let Some(old) = what.target {
                        let what = Semantics { target: Some(target(old)), ..what.clone() };
                        one = Arc::new(Insn { what: Some(what), ..(*one).clone() });
                    } else if !what.table.is_empty() {
                        let table = what.table.iter().map(|at| target(*at)).collect();
                        one = Arc::new(Insn { what: Some(Semantics { table, ..what.clone() }), ..(*one).clone() });
                    }
                }
            }
            insns.push(one);
        }
        let successors: IndexSet<i64> = block.succ.iter().map(|at| target(*at)).collect();
        blocks.push(
            _fold_converged(LirBlock {
                succ: successors.into_iter().collect(),
                ..block.with_insns(insns).into_inner()
            })
            .into(),
        );
    }
    let entry = target(body.entry);
    LirBody { entry, ..body.with_blocks(blocks) }
}

/// Every conditional whose two CFG edges became one is an unconditional edge
/// (`OptimizeBlock` removing a branch whose two ways are one block).
fn _converged(body: &LirBody) -> LirBody {
    let mut changed = false;
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            if block.succ.is_empty() || block.succ.iter().any(|to| *to != block.succ[0]) {
                return block.clone();
            }
            let folded = _fold_converged((**block).clone());
            if folded.insns.same_insns(&block.insns) && folded.succ.len() == block.succ.len() {
                block.clone()
            } else {
                changed = true;
                folded.into()
            }
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// A conditional whose two CFG edges became one is an unconditional edge.
pub fn _fold_converged(block: LirBlock) -> LirBlock {
    // The edges are a set: `jae L; jmp L` has the one successor, listed twice
    // or once.
    if block.succ.is_empty() || block.succ.iter().any(|to| *to != block.succ[0]) {
        return block;
    }
    let block = if block.succ.len() == 1 { block } else { LirBlock { succ: vec![block.succ[0]].into(), ..block } };
    let destination = block.succ[0];
    let real = _real(&block);
    let last = real.last();
    if let Some(last) = last {
        if last.what.as_ref().is_some_and(|what| what.op == Operation::Jump && what.target == Some(destination))
            && real.len() > 1
        {
            let branch = &real[real.len() - 2];
            if branch.what.as_ref().is_some_and(|what| what.op == Operation::Branch && what.target == Some(destination))
            {
                let insns = block
                    .insns
                    .iter()
                    .map(|one| if Arc::ptr_eq(one, branch) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                    .collect();
                return LirBlock { insns, ..block };
            }
        }
        if last.what.as_ref().is_some_and(|what| what.op == Operation::Branch) {
            let jump = Arc::new(Insn {
                what: Some(Semantics {
                    name: Some("jmp".to_owned()),
                    target: Some(destination),
                    ..Semantics::new(Operation::Jump)
                }),
                uses: Vec::new(),
                ..(**last).clone()
            });
            let insns = block
                .insns
                .iter()
                .map(|one| if Arc::ptr_eq(one, last) { Arc::clone(&jump) } else { Arc::clone(one) })
                .collect();
            return LirBlock { insns, ..block };
        }
    }
    block
}

/// Duplicate a terminal tail when doing so costs no bytes and removes a jump.
///
/// Frame teardown is emitted around RETURN rather than represented in LIR, so
/// its exact selected size is supplied by the emitter.  Source-owned tails are
/// deliberately excluded: duplicating them would duplicate provenance,
/// fixups, or line anchors rather than merely choosing a machine layout.
pub fn duplicated_returns(
    body: LirBody,
    return_overhead: &dyn Fn() -> i64,
) -> LirBody {
    let mut body = body;
    loop {
        let blocks = body.blocks.clone();
        let by_at: IndexMap<i64, &Block> = blocks.iter().map(|block| (block.at, block)).collect();
        let predecessors = _predecessors(&blocks);
        let mut changed = false;
        for tail in &blocks {
            // Parent order reaches only sums and a map keyed by `at`.
            let parents: Vec<&Block> =
                predecessors.get(&tail.at).into_iter().flatten().filter_map(|at| by_at.get(at).copied()).collect();
            if tail.at == body.entry || parents.len() < 2 || parents.iter().any(|parent| parent.succ != [tail.at]) {
                continue;
            }
            let Some(tail_bytes) = _duplicable_return_size(body.bits, tail) else { continue };
            let prepared: Vec<Option<(&LirBlock, i64)>> =
                parents.iter().map(|parent| _return_parent(body.bits, parent, tail.at)).collect();
            if prepared.iter().any(Option::is_none) {
                continue;
            }
            let copies: Vec<(&LirBlock, i64)> = prepared.into_iter().flatten().collect();
            let saved: i64 = copies.iter().map(|(_parent, jump_size)| jump_size).sum();
            if saved == 0 {
                continue;
            }
            let tail_size = return_overhead() + tail_bytes;
            if parents.len() as i64 * tail_size > tail_size + saved {
                continue;
            }

            let replacement: IndexMap<i64, Block> =
                copies.iter().map(|(parent, _jump_size)| (parent.at, _with_return(parent, tail))).collect();
            body = LirBody {
                blocks: blocks
                    .iter()
                    .filter(|block| block.at != tail.at)
                    .map(|block| replacement.get(&block.at).unwrap_or(block).clone())
                    .collect(),
                ..body
            };
            changed = true;
            break;
        }
        if !changed {
            return body;
        }
    }
}

/// An arm's bytes but its final jump, as selected; none where an
/// instruction has no encoding here.
fn _arm_bytes(
    bits: u32,
    block: &LirBlock,
) -> Option<i64> {
    let real = _real(block);
    let body = real.split_last().map_or(&real[..], |(last, rest)| {
        if last.what.as_ref().is_some_and(|what| what.op == Operation::Jump) { rest } else { &real[..] }
    });
    body.iter()
        .map(|one| {
            select::priced_in(bits, one.what.as_ref()?, 0, None, false, false, None).map(|made| made.code.len() as i64)
        })
        .sum()
}

/// Selected bytes in a source-unowned terminal return block, without the frame
/// teardown the emitter adds.
pub fn _duplicable_return_size(
    bits: u32,
    block: &LirBlock,
) -> Option<i64> {
    let real = _real(block);
    if !block.phis.is_empty()
        || !block.succ.is_empty()
        || real.is_empty()
        || real[real.len() - 1].what.as_ref().is_none_or(|what| what.op != Operation::Return)
        // A return that pops 64 KB or more is several instructions
        // (`masm::moved_return`), not one `ret`.
        || real[real.len() - 1].what.as_ref().is_some_and(|what| matches!(
            what.sources.first(),
            Some(crate::model::ir::Loc::Imm(popped)) if popped.value > llrm_x86::calling::RET_POPS_MOST
        ))
        || block.insns.iter().any(|one| {
            !one.inserted()
                || one.what.is_none()
                || one.symbol == Some(true)
                || one.group.is_some()
                || !one.spread.is_empty()
                || one.what.as_ref().is_some_and(|what| {
                    matches!(
                        what.op,
                        Operation::Barrier | Operation::Branch | Operation::Call | Operation::Data | Operation::Jump
                    )
                })
        })
    {
        return None;
    }
    let emitted: Vec<Option<select::Emitted>> = real
        .iter()
        .map(|one| select::priced_in(bits, one.what.as_ref().expect("checked above"), 0, None, false, false, None))
        .collect();
    if emitted.iter().any(Option::is_none) {
        return None;
    }
    Some(emitted.iter().flatten().map(|one| one.code.len() as i64).sum::<i64>())
}

/// A dedicated edge to target and the shortest jump bytes it can save.
pub fn _return_parent(
    bits: u32,
    block: &LirBlock,
    target: i64,
) -> Option<(&LirBlock, i64)> {
    let real = _real(block);
    let last = real.last()?;
    let what = last.what.as_ref()?;
    if what.op == Operation::Jump {
        if what.target != Some(target)
            || !last.inserted()
            || last.symbol == Some(true)
            || last.group.is_some()
            || !last.spread.is_empty()
        {
            return None;
        }
        let emitted =
            select::priced_in(bits, &Semantics { target: Some(2), ..what.clone() }, 0, None, true, false, None);
        return emitted.map(|emitted| (block, emitted.code.len() as i64));
    }
    if matches!(
        what.op,
        Operation::Branch | Operation::Call | Operation::Data | Operation::Return
    ) {
        return None;
    }
    Some((block, 0))
}

/// Replace parent's dedicated edge with a fresh copy of tail.
pub fn _with_return(
    parent: &LirBlock,
    tail: &LirBlock,
) -> Block {
    let real = _real(parent);
    let jump = real.last().filter(|last| last.what.as_ref().is_some_and(|what| what.op == Operation::Jump));
    let anchor = match jump {
        Some(jump) => jump.at,
        None => parent.insns.last().map_or(parent.at, |last| last.at),
    };
    let kept = parent.insns.iter().filter(|one| jump.is_none_or(|jump| !Arc::ptr_eq(one, jump))).cloned();
    let copies = tail
        .insns
        .iter()
        .map(
            |one| Arc::new(Insn { at: anchor, covers: Some((anchor, anchor)), spread: Vec::new(), ..(**one).clone() }),
        );
    LirBlock { succ: tail.succ.clone(), ..parent.with_insns(kept.chain(copies).collect()).into_inner() }.into()
}

/// One change to `body`, the first that applies, in place: its blocks are
/// neither copied nor, but for the edited one and those the change strands,
/// rebuilt. Whether it changed anything.
///
/// `at` is each block's position, kept while the blocks stay as they are: a
/// change that only rewrites an instruction strands nothing, so the body is not
/// searched for what it reaches again.
pub fn _step(
    body: &mut LirBody,
    at: &mut Places,
    protected: &BTreeSet<i64>,
    from: usize,
    visits: &mut usize,
) -> Option<usize> {
    let mut blocks = std::mem::take(&mut body.blocks);
    for index in from..blocks.len() {
        *visits += 1;
        // The block's last two real instructions, which is all the rules read
        // of the rest: nothing is copied for a block no rule applies
        // to.
        let (last, before) = _real_tail(&blocks[index]);
        let Some(last) = last else {
            continue;
        };
        let last_what = last.what.as_ref().expect(NO_OP);
        if !matches!(last_what.op, Operation::Branch | Operation::Jump) {
            continue;
        }
        let block = blocks[index].clone();
        let here = block.at;
        let last = &last;
        // Control falls through blocks of only meta instructions, as layout
        // places them.
        let after = blocks[index + 1..]
            .iter()
            .find(|next| !next.phis.is_empty() || next.insns.iter().any(|one| !one.is_meta()))
            .map(|next| next.at);
        if !last_what.table.is_empty() {
            // A table jump's index to a block that only jumps goes to where it
            // jumps. None of the rules below read a jump with no target.
            let table: Vec<i64> = last_what
                .table
                .iter()
                .map(|&entry| _through(&blocks, at, Some(entry), protected).unwrap_or(entry))
                .collect();
            if table == last_what.table {
                continue;
            }
            let changed: Vec<(i64, i64)> = last_what
                .table
                .iter()
                .zip(&table)
                .filter(|(old, new)| old != new)
                .map(|(&old, &new)| (old, new))
                .collect();
            for &(old, onward) in &changed {
                body.odds.rerouted(block.at, &block.succ, old, &[(onward, 1.0)]);
            }
            let moved = Arc::new(Insn {
                what: Some(Semantics { table: table.clone(), ..last_what.clone() }),
                ..(**last).clone()
            });
            let succ: IndexSet<i64> = table.iter().copied().collect();
            let insns =
                block.insns.iter().map(|one| if Arc::ptr_eq(one, last) { Arc::clone(&moved) } else { Arc::clone(one) });
            let moved_succ: Vec<i64> = succ.into_iter().collect();
            at.moved(&block.succ, &moved_succ);
            blocks[index] =
                LirBlock { succ: moved_succ.into(), ..block.with_insns(insns.collect()).into_inner() }.into();
            body.blocks = blocks;
            let freed: Vec<i64> = changed.iter().map(|&(old, _)| old).collect();
            _stranded(body, at, &freed);
            return Some(at.get(&here).unwrap_or(0));
        }
        let target = _through(&blocks, at, last_what.target, protected);
        if target != last_what.target {
            let onward = target.expect("a passage names its target");
            if let Some(old) = last_what.target {
                body.odds.rerouted(block.at, &block.succ, old, &[(onward, 1.0)]);
            }
            let retargeted = _retargeted(&block, last, onward);
            at.moved(&block.succ, &retargeted.succ);
            blocks[index] = retargeted.into();
            body.blocks = blocks;
            // Every block with edges is reached (the body was pruned after each
            // change), so the only block this can strand is the one
            // the branch left, and only when nothing else enters it.
            match last_what.target {
                Some(old) => _stranded(body, at, &[old]),
                None => _stranded_whole(body, at),
            }
            return Some(at.get(&here).unwrap_or(0));
        }
        // A branch before the final jump goes through a block that only jumps
        // as well (BranchFolding's `OptimizeBlock`, the true branch of a
        // `Bcc T; B F` pair).
        if let Some(branch) = before.as_ref().filter(|one| {
            one.what
                .as_ref()
                .is_some_and(|what| what.op == Operation::Branch && !what.indirect && what.target.is_some())
        }) {
            let old = branch.what.as_ref().and_then(|what| what.target).expect("checked");
            let onward = _through(&blocks, at, Some(old), protected).expect("a passage names its target");
            if onward != old {
                body.odds.rerouted(block.at, &block.succ, old, &[(onward, 1.0)]);
                let retargeted = _retargeted(&block, branch, onward);
                at.moved(&block.succ, &retargeted.succ);
                blocks[index] = retargeted.into();
                body.blocks = blocks;
                _stranded(body, at, &[old]);
                return Some(at.get(&here).unwrap_or(0));
            }
        }
        // A block with a branch before the last two keeps its jump: without it,
        // three ways out and nothing choosing among them.
        let choosable = block.succ.len() <= 2;
        if last_what.op == Operation::Jump && target == after && choosable {
            // A fall-through needs no machine jump.  A decoded jump may still
            // own source bytes, though; retain those as an inert anchor so
            // fresh layout can account for the replaced range.  A frontend-
            // inserted jump owns no bytes and disappears outright.
            let kept = block
                .insns
                .iter()
                .filter(|one| !Arc::ptr_eq(one, last) || !one.inserted())
                .map(|one| {
                    if Arc::ptr_eq(one, last) && !one.inserted() {
                        lir::anchor(Arc::clone(one))
                    } else {
                        Arc::clone(one)
                    }
                })
                .collect();
            blocks[index] = LirBlock { insns: kept, ..block.into_inner() }.into();
            body.blocks = blocks;
            return Some(at.get(&here).unwrap_or(0));
        }
        if last_what.op == Operation::Jump
            && choosable
            && before.as_ref().is_some_and(|one| one.what.as_ref().expect(NO_OP).op == Operation::Branch)
        {
            let branch = before.as_ref().expect("checked");
            let branch_what = branch.what.as_ref().expect(NO_OP);
            let opposite = branch_what.name.as_deref().and_then(|name| _OPPOSITE.get(name));
            if branch_what.target == after && opposite.is_some() {
                let inverted = Arc::new(Insn {
                    what: Some(Semantics {
                        name: opposite.map(|name| (*name).to_owned()),
                        target,
                        ..branch_what.clone()
                    }),
                    ..(**branch).clone()
                });
                let insns = block
                    .insns
                    .iter()
                    .filter(|one| !Arc::ptr_eq(one, last))
                    .map(|one| if Arc::ptr_eq(one, branch) { Arc::clone(&inverted) } else { Arc::clone(one) })
                    .collect();
                blocks[index] = LirBlock { insns, ..block.into_inner() }.into();
                body.blocks = blocks;
                return Some(at.get(&here).unwrap_or(0));
            }
        }
        let opposite = last_what.name.as_deref().and_then(|name| _OPPOSITE.get(name));
        if last_what.op == Operation::Branch && after.is_some() && opposite.is_some() {
            // Falling into a block that only jumps, with the branch taken to
            // the block past it.
            let over = blocks[index + 1].clone();
            let beyond = blocks.get(index + 2).map(|past| past.at);
            let onward = _passage(&over);
            if onward.is_some()
                && !protected.contains(&over.at)
                && !_real(&over).is_empty()
                && last_what.target == beyond
                && _predecessors(&blocks).get(&over.at) == Some(&BTreeSet::from([block.at]))
            {
                let inverted = Arc::new(Insn {
                    what: Some(Semantics {
                        name: opposite.map(|name| (*name).to_owned()),
                        target: onward,
                        ..last_what.clone()
                    }),
                    ..(**last).clone()
                });
                let insns = block
                    .insns
                    .iter()
                    .map(|one| if Arc::ptr_eq(one, last) { Arc::clone(&inverted) } else { Arc::clone(one) })
                    .collect();
                let succ = vec![onward.expect("checked above"), beyond.expect("a branch names its target")];
                at.moved(&block.succ, &succ);
                at.moved(&over.succ, &[]);
                let freed = vec![over.at];
                blocks[index] = LirBlock { insns, succ: succ.into(), ..block.into_inner() }.into();
                blocks[index + 1] = LirBlock { succ: Vec::new().into(), ..over.into_inner() }.into();
                body.blocks = blocks;
                _stranded(body, at, &freed);
                return Some(at.get(&here).unwrap_or(0));
            }
        }
    }
    body.blocks = blocks;
    None
}

/// The last two of `_real(block)`, the last first, without making the rest.
fn _real_tail(block: &LirBlock) -> (Option<Arc<Insn>>, Option<Arc<Insn>>) {
    let nop = |one: &Insn| {
        one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"))
    };
    let mut real = block.insns.iter().rev().filter(|one| !one.is_meta() && !nop(one));
    (real.next().cloned(), real.next().cloned())
}

/// The instructions that print.
/// The block's work: neither meta instructions nor source `nop`s.
pub fn _real(block: &LirBlock) -> Vec<Arc<Insn>> {
    let nop = |one: &Insn| {
        one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"))
    };
    block.insns.iter().filter(|one| !one.is_meta() && !nop(one)).cloned().collect()
}

/// Where a block with no work goes.
///
/// Meta instructions are not work: threading past them strands no bytes,
/// since an unreachable block keeps what it owns. An identity copy's anchor
/// is: it defines a value on this edge, and threading around it would leave
/// the successor's operand naming a value no surviving instruction defines.
pub fn _passage(block: &LirBlock) -> Option<i64> {
    if !block.phis.is_empty() {
        return None;
    }
    let real = _real(block);
    if real.is_empty() && block.succ.len() == 1 {
        return Some(block.succ[0]);
    }
    if real.len() == 1 && real[0].what.as_ref().is_some_and(|what| what.op == Operation::Jump) {
        return real[0].what.as_ref().and_then(|what| what.target);
    }
    None
}

/// The first block past pure passages, retaining measured loop anchors.
///
/// An exact loop header may emit no bytes after scalar optimization.  It is
/// still a program fact: redirecting a nested backedge through it can merge
/// two natural loops in the final CFG and invalidate the trip-count metadata
/// used by measurement.  Keeping the zero-length block changes no encoding.
pub fn _through(
    blocks: &[Block],
    at: &Places,
    target: Option<i64>,
    protected: &BTreeSet<i64>,
) -> Option<i64> {
    let (start, mut seen) = (target, HashSet::default());
    let mut target = target;
    while let Some(current) = target {
        if protected.contains(&current) || !at.contains(&current) {
            break;
        }
        let Some(onward) = _passage(&blocks[at.get(&current).expect("known above")]) else {
            break;
        };
        if seen.contains(&current) {
            return start;
        }
        seen.insert(current);
        target = Some(onward);
    }
    target
}

pub fn _retargeted(
    block: &LirBlock,
    last: &Arc<Insn>,
    target: i64,
) -> LirBlock {
    let what = last.what.as_ref().expect(NO_OP);
    let old = what.target;
    let moved = Arc::new(Insn { what: Some(Semantics { target: Some(target), ..what.clone() }), ..(**last).clone() });
    let succ: IndexSet<i64> = block.succ.iter().map(|one| if Some(*one) == old { target } else { *one }).collect();
    LirBlock {
        succ: succ.into_iter().collect(),
        ..block
            .with_insns(
                block
                    .insns
                    .iter()
                    .map(|one| if Arc::ptr_eq(one, last) { Arc::clone(&moved) } else { Arc::clone(one) })
                    .collect(),
            )
            .into_inner()
    }
}

pub fn _predecessors(blocks: &[Block]) -> IndexMap<i64, BTreeSet<i64>> {
    let mut found: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    for block in blocks {
        for one in &block.succ {
            found.entry(*one).or_default().insert(block.at);
        }
    }
    found
}

pub fn _reachable(
    body: &LirBody,
    blocks: Vec<Block>,
) -> LirBody {
    let mut made = body.with_blocks(Vec::new());
    made.blocks = blocks;
    _prune(&mut made);
    made
}

/// `_prune`, and the positions in `at` made again where it removed a block,
/// from the whole body: what the counts cannot settle.
fn _stranded_whole(
    body: &mut LirBody,
    at: &mut Places,
) {
    #[cfg(test)]
    WALKS.with(|one| one.set(one.get() + 1));
    // Followed over the positions the caller keeps, and the blocks rebuilt only
    // if one that is reached by nothing is not already the bytes it owned:
    // the body is pruned after each change, so that is rarely more than the
    // block a branch just left.
    let blocks = &body.blocks;
    let mut reached = vec![false; blocks.len()];
    let mut work: Vec<usize> = at.get(&body.entry).into_iter().collect();
    while let Some(index) = work.pop() {
        if std::mem::replace(&mut reached[index], true) {
            continue;
        }
        work.extend(blocks[index].succ.iter().filter_map(|to| at.get(to)).filter(|&next| !reached[next]));
    }
    let settled = |block: &LirBlock| {
        block.succ.is_empty()
            && block.phis.is_empty()
            && !block.insns.is_empty()
            && block.insns.iter().all(|one| Arc::ptr_eq(&lir::bytes_only(one), one) && !one.owned().is_empty())
    };
    let stranded: Vec<usize> = (0..blocks.len()).filter(|&index| !reached[index] && !settled(&blocks[index])).collect();
    if stranded.is_empty() {
        return;
    }
    // What `_prune` makes of them, in place: the blocks that keep bytes stay
    // where they are, and only a block with none to keep takes the
    // positions after it with it.
    let mut keep = vec![true; body.blocks.len()];
    for index in stranded {
        let block = &body.blocks[index];
        let kept: Vec<Arc<Insn>> =
            block.insns.iter().filter(|one| !one.owned().is_empty()).map(lir::bytes_only).collect();
        if kept.is_empty() {
            keep[index] = false;
        } else {
            body.blocks[index] =
                LirBlock { succ: Vec::new().into(), phis: Vec::new().into(), ..block.with_insns(kept).into_inner() }
                    .into();
        }
    }
    if keep.contains(&false) {
        let mut places = keep.into_iter();
        body.blocks.retain(|_| places.next().unwrap_or(true));
    }
    *at = Places::of(body);
}

// Walks from the entry made to find the blocks a change strands.
#[cfg(test)]
thread_local! {
    pub(crate) static WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The blocks on a cycle: those of a strongly connected component of more than
/// one block, or with an edge to themselves.
fn _cyclic(body: &LirBody) -> HashSet<i64> {
    let mut cyclic = HashSet::default();
    let blocks: IndexMap<i64, &crate::model::lir::Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    for members in llrm_support::graph::components(&body.blocks) {
        if members.len() > 1 || blocks[&members[0]].succ.contains(&members[0]) {
            cyclic.extend(members);
        }
    }
    cyclic
}

/// The blocks that `freed`, blocks a change took an edge from, strand: the
/// positions in `at` made again where one is removed.
///
/// What `_prune` makes of them, in place: the blocks that keep bytes stay where
/// they are, and only a block with none to keep takes the positions after it
/// with it.
fn _stranded(
    body: &mut LirBody,
    at: &mut Places,
    freed: &[i64],
) {
    let settled = |block: &LirBlock| {
        block.succ.is_empty()
            && block.phis.is_empty()
            && !block.insns.is_empty()
            && block.insns.iter().all(|one| Arc::ptr_eq(&lir::bytes_only(one), one) && !one.owned().is_empty())
    };
    // Only a block nothing enters is stranded; one that still has an edge in
    // is left to the walk that `_prune` makes in the end.
    let mut work: Vec<i64> =
        freed.iter().copied().filter(|one| *one != body.entry && at.entering_of(*one) == Some(0)).collect();
    let mut gone: Vec<usize> = Vec::new();
    let mut stripped: HashSet<i64> = HashSet::default();
    while let Some(block_at) = work.pop() {
        if !stripped.insert(block_at) {
            continue;
        }
        let Some(index) = at.get(&block_at) else { continue };
        let block = &body.blocks[index];
        if settled(block) {
            continue;
        }
        let succ = block.succ.clone();
        let kept: Vec<Arc<Insn>> =
            block.insns.iter().filter(|one| !one.owned().is_empty()).map(lir::bytes_only).collect();
        if kept.is_empty() {
            gone.push(index);
        } else {
            body.blocks[index] =
                LirBlock { succ: Vec::new().into(), phis: Vec::new().into(), ..block.with_insns(kept).into_inner() }
                    .into();
        }
        for to in succ.iter().copied() {
            let Some(count) = at.drop_edge(to) else { continue };
            if count > 0 {
                // Cut off from the entry all the same if the blocks that
                // still enter it are a cycle's.
                if at.on_cycle(body, to) {
                    return _stranded_whole(body, at);
                }
            } else if to != body.entry {
                work.push(to);
            }
        }
    }
    if !gone.is_empty() {
        let mut keep = vec![true; body.blocks.len()];
        for index in gone {
            keep[index] = false;
        }
        for (index, block) in body.blocks.iter().enumerate() {
            if !keep[index] {
                at.remove(block.at);
            }
        }
        let mut places = keep.into_iter();
        body.blocks.retain(|_| places.next().unwrap_or(true));
    }
}

/// Where each block is, kept as blocks are removed: a block's place is the one
/// it began at less the blocks removed before it, from a Fenwick tree, so a
/// removal does not number the blocks after it again.
///
/// It also keeps how many edges enter each block across the changes `_step`
/// makes. Every block that has edges is reached (the body is pruned after each
/// change), so the blocks a change strands are the ones whose count it takes to
/// zero, and those they enter in turn: no walk from the entry. A block on a
/// cycle can keep its count and be cut off, so a count that reaches one is
/// settled by the walk.
pub struct Places {
    began: IndexMap<i64, usize>,
    removed: Vec<u32>,
    gone: Vec<bool>,
    taken: usize,
    entering: Vec<u32>,
    /// Found when a count first needs it, on the body as it is then: the
    /// cycles only shrink.
    cyclic: Option<HashSet<i64>>,
}

impl Places {
    fn of(body: &LirBody) -> Self {
        let count = body.blocks.len();
        let began: IndexMap<i64, usize> =
            body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
        let mut entering = vec![0; count];
        for block in &body.blocks {
            for to in &block.succ {
                if let Some(&index) = began.get(to) {
                    entering[index] += 1;
                }
            }
        }
        Self { began, removed: vec![0; count + 1], gone: vec![false; count], taken: 0, entering, cyclic: None }
    }

    /// How many edges enter `at`.
    fn entering_of(
        &self,
        at: i64,
    ) -> Option<u32> {
        self.began.get(&at).map(|&index| self.entering[index])
    }

    /// An edge into `to` is gone; how many are left.
    fn drop_edge(
        &mut self,
        to: i64,
    ) -> Option<u32> {
        let index = *self.began.get(&to)?;
        self.entering[index] -= 1;
        Some(self.entering[index])
    }

    fn on_cycle(
        &mut self,
        body: &LirBody,
        at: i64,
    ) -> bool {
        self.cyclic.get_or_insert_with(|| _cyclic(body)).contains(&at)
    }

    /// A block's edges `old` become `new`.
    fn moved(
        &mut self,
        old: &[i64],
        new: &[i64],
    ) {
        for to in old {
            if let Some(&index) = self.began.get(to) {
                self.entering[index] -= 1;
            }
        }
        for to in new {
            if let Some(&index) = self.began.get(to) {
                self.entering[index] += 1;
            }
        }
    }

    pub fn get(
        &self,
        at: &i64,
    ) -> Option<usize> {
        let began = *self.began.get(at)?;
        if self.taken == 0 {
            return Some(began);
        }
        if self.gone[began] {
            return None;
        }
        // The blocks removed before it.
        let (mut before, mut node) = (0_u32, began);
        while node > 0 {
            before += self.removed[node];
            node &= node - 1;
        }
        Some(began - before as usize)
    }

    pub fn contains(
        &self,
        at: &i64,
    ) -> bool {
        self.get(at).is_some()
    }

    fn remove(
        &mut self,
        at: i64,
    ) {
        let began = self.began[&at];
        self.gone[began] = true;
        self.taken += 1;
        let mut node = began + 1;
        while node < self.removed.len() {
            self.removed[node] += 1;
            node += node & node.wrapping_neg();
        }
    }
}

/// `body` with the blocks its entry does not reach stripped to their source
/// bytes, in place: a reached block is moved, not copied.
pub fn _prune(body: &mut LirBody) {
    let blocks = std::mem::take(&mut body.blocks);
    let reached: HashSet<i64> = {
        let by_at: IndexMap<i64, &Block> = blocks.iter().map(|block| (block.at, block)).collect();
        let (mut reached, mut work) = (HashSet::default(), vec![body.entry]);
        while let Some(one) = work.pop() {
            if reached.contains(&one) || !by_at.contains_key(&one) {
                continue;
            }
            reached.insert(one);
            work.extend(by_at[&one].succ.iter().copied());
        }
        reached
    };
    // An unreachable block's work goes and its source bytes stay, as
    // byte-only markers: layout must account for every byte. Dropped whole,
    // a fully unrolled BC loop left a hole in its source map, and a threaded
    // `jmp` passage lost its three bytes.
    body.blocks = blocks
        .into_iter()
        .filter_map(|block| {
            if reached.contains(&block.at) {
                return Some(block);
            }
            let kept: Vec<Arc<Insn>> =
                block.insns.iter().filter(|one| !one.owned().is_empty()).map(lir::bytes_only).collect();
            (!kept.is_empty()).then(|| {
                LirBlock { succ: Vec::new().into(), phis: Vec::new().into(), ..block.with_insns(kept).into_inner() }
                    .into()
            })
        })
        .collect();
}

#[cfg(test)]
#[path = "jumps_tests.rs"]
mod tests;
