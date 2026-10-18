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
//! 2. `_placed` orders the blocks. By estimated frequency (every block counts
//!    once at -Os): the likelier edge falls through, a diamond's likelier arm
//!    falls into its join, and a new trace starts at the busiest block left. A
//!    loop header that only decides whether to go round (`_tests`) is placed
//!    after its latch, so a pass round the loop takes one branch. Cold blocks
//!    go last.
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
//!
//! Then, unless sized for code size, `duplicated_tails` replaces a `jmp` to a
//! short tail (at most `TAIL_DUPLICATION` plain instructions besides its
//! jumps) by a copy of the tail, where the copy's own fall-through jump runs
//! less often than the jump removed, and a copy that would make a loop
//! irreducible is refused. A tail is not copied into a jump that follows a
//! branch whose other way is likelier than the tail (and is not the block
//! itself): the copy would leave code between that branch and what follows
//! it, and the likelier way would be taken whatever is laid next (a latch with
//! a short exit tail would pay a taken back edge every trip to save the exit's
//! one jump). `duplicated` then does the same for tails that return,
//! `rotated` rotates a loop so its latch falls into the header (gcc's
//! bb-reorder `rotate_loop`) when that takes fewer jumps by the target's jump
//! prices, and `inverted` flips each `jcc L; jmp M` the copying left where `L`
//! is next; where neither way is next, it makes the likelier way the
//! conditional jump, so the trip that goes on pays one taken branch, not a
//! branch not taken and a jump (the loop that went round by `je exit; jmp
//! top`).
//!
//! ```text
//!   before                       after (placed, threaded)
//!   test: cmp; jge exit          jmp test
//!   body: ...; jmp test          body: ...
//!   exit: ...                    test: cmp; jl body
//!                                exit: ...
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

mod tailmerge;

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_analysis::branchprob;

use crate::analysis::dataflow::{self, Direction};
use crate::analysis::frequency::Frequency;
use crate::analysis::loops::{self as loopy, Loop};
use crate::backend::layout::_OPPOSITE;
use crate::backend::objbuild::{SHORT_JUMP, short_reaches};
use crate::backend::{cpu, machinedce, masm, select};
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{self, Block, BlockOdds, Insn, LirBlock, LirBody};
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
    let price = Branches::of(cpu);
    let mut placed = threaded(&_placed(&_hoisted(body), size)?);
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
    Ok(inverted(&if size { placed } else { rotated(&duplicated(&duplicated_tails(&placed), true), &price)? }))
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

/// gcc's `max-grow-copy-bb-insns`: a block is copied while it is at most this
/// many unconditional jumps long.
const COPY_BB_INSNS: usize = 8;

/// gcc's `get_uncond_jump_length`: what the target prices one unconditional
/// jump at, in its long form.
pub(crate) fn uncond_jump_bytes(bits: u32) -> usize {
    let jump = Semantics { name: Some("jmp".into()), target: Some(2), ..Semantics::new(Operation::Jump) };
    select::priced_in(bits, &jump, 0, None, false, false, None).map_or(2, |made| made.code.len())
}

/// The bytes a block may be and still be copied (`copy_bb_p`): `COPY_BB_INSNS`
/// jumps of code.
fn copy_limit(bits: u32) -> usize {
    COPY_BB_INSNS * uncond_jump_bytes(bits)
}

/// A block that only returns: plain operations then a `ret`, no way on, within
/// `limit` bytes and none of it source-owned.
fn _return_tail(
    bits: u32,
    block: &LirBlock,
) -> bool {
    if !block.phis.is_empty()
        || !block.succ.is_empty()
        || block
            .insns
            .iter()
            .any(|one| one.call.is_some() || one.group.is_some() || one.symbol == Some(true) || !one.spread.is_empty())
    {
        return false;
    }
    let real = _real(block);
    let Some((last, rest)) = real.split_last() else { return false };
    // A return that covers source bytes is the program's own (a BASIC module's
    // end spells a runtime call at it): not copied.
    if last.what.as_ref().is_none_or(|what| what.op != Operation::Return)
        || !last.inserted()
        || rest.iter().any(|one| {
            one.call.is_some()
                || one.what.as_ref().is_none_or(|what| {
                    matches!(
                        what.op,
                        Operation::Barrier
                            | Operation::Branch
                            | Operation::Call
                            | Operation::Data
                            | Operation::Jump
                            | Operation::Return
                    )
                })
        })
    {
        return false;
    }
    real.iter()
        .try_fold(0usize, |bytes, one| {
            select::priced_in(bits, one.what.as_ref()?, 0, None, false, false, None).map(|made| bytes + made.code.len())
        })
        .is_some_and(|bytes| bytes <= copy_limit(bits))
}

/// LLVM's `TailDupSize` at -O2: the instructions a tail may hold besides its
/// jumps.
const TAIL_DUPLICATION: usize = 2;

/// Each `jmp` to a short tail replaced by a copy of the tail, as LLVM's tail
/// duplication: the copy runs where the jump did, and the jump is gone. A
/// tail that falls through gets a `jmp` there in its copy, which runs only
/// as often as the copy falls through, so a copy is made only where that is
/// less often than the jump it removes. A copy claims none of the
/// original's bytes, which the tail keeps.
pub fn duplicated_tails(body: &LirBody) -> LirBody {
    duplicated(body, false)
}

/// `duplicated_tails`, or only the tails that return: the jump a copied tail
/// ended in may reach one. A tail is not copied into a jump after a branch
/// whose other way is likelier than the tail (and is not the block itself): the
/// copy would leave code between that branch and what follows it, and the
/// likelier way would be taken whatever is laid next (x_adler's `jne top; jmp
/// exit` latch).
fn duplicated(
    body: &LirBody,
    returns: bool,
) -> LirBody {
    let by_at: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let after: IndexMap<i64, i64> = body.blocks.windows(2).map(|pair| (pair[0].at, pair[1].at)).collect();
    let frequency = Frequency::of(body);
    let mut odds = body.odds.clone();
    let irreducible = |blocks: &[Block]| !loopy::irreducible(blocks, Some(body.entry)).is_empty();
    let reducible = !irreducible(&body.blocks);
    // Every copy that can be made, whether or not an earlier one is: what a
    // copy reads is the body as it came.
    let mut copies: Vec<(usize, LirBlock, i64, Vec<(i64, f64)>)> = Vec::new();
    for (index, parent) in body.blocks.iter().enumerate() {
        let made = (|| {
            let real = _real(parent);
            let Some(last) = real
                .last()
                .and_then(|one| one.what.as_ref())
                .filter(|what| what.op == Operation::Jump && !what.indirect)
            else {
                return None;
            };
            let Some(tail) =
                last.target.filter(|at| *at != parent.at && *at != body.entry).and_then(|at| by_at.get(&at))
            else {
                return None;
            };
            let Some((copied, falls)) = _duplicable(body.bits, tail, after.get(&tail.at).copied())
                .filter(|(_, falls)| !returns || falls.is_none())
                .filter(|_| !returns || _return_tail(body.bits, tail))
            else {
                return None;
            };
            // A conditional branch before this jump leaves by its way when
            // that way is the likelier, and it falls through to its block when
            // that block is laid next (`inverted`, `rotated` lay it so): the
            // jump is the cold way's. A copy of the tail in the jump's place
            // leaves code between the branch and what follows it, so the branch
            // is taken whatever is laid next: not where its way is the likelier
            // (a latch with a short exit tail paid a taken back edge every trip
            // to save the exit's one jump).
            if let (_, Some(branch)) = _real_tail(parent) {
                let way = branch.what.as_ref().filter(|what| what.op == Operation::Branch && !what.indirect);
                if let Some(way) = way.and_then(|what| what.target)
                    && way != tail.at
                    // A block cannot fall into itself: a loop of one block is a
                    // taken branch back whatever is laid next.
                    && way != parent.at
                    && frequency.edge(parent.at, way) > frequency.edge(parent.at, tail.at)
                {
                    return None;
                }
            }
            // Per run of the jump: the fall-through's jump runs as often as the
            // tail falls through.
            if let Some(next) = falls {
                let share =
                    |from: i64, to: i64| frequency.edge(from, to) / frequency.block(from).max(f64::MIN_POSITIVE);
                if share(tail.at, next) >= 1.0 {
                    return None;
                }
            }
            let jump = &parent.insns[parent
                .insns
                .iter()
                .rposition(|one| Arc::ptr_eq(one, real.last().expect("checked")))
                .expect("a real instruction")];
            // The jump's work goes; its source bytes stay, as `_reachable`
            // keeps them.
            let mut insns: Vec<Arc<Insn>> = parent
                .insns
                .iter()
                .map(|one| if Arc::ptr_eq(one, jump) { lir::bytes_only(one) } else { Arc::clone(one) })
                .collect();
            insns.extend(copied.iter().map(|one| _unowned(one, jump)));
            if let Some(next) = falls {
                let mut made = (**jump).clone();
                made.what = Some(Semantics { target: Some(next), ..last.clone() });
                insns.push(_unowned(&made, jump));
            }
            // The edge to the tail becomes the tail's edges, at its share.
            // The edge to the tail becomes the tail's edges, at their odds.
            let into: Vec<(i64, f64)> =
                tail.succ.iter().map(|next| (*next, body.odds.chance(tail.at, &tail.succ, *next))).collect();
            // The parent's own branches keep their targets.
            let mut succ: Vec<i64> = parent.succ.iter().copied().filter(|at| *at != tail.at).collect();
            succ.extend(tail.succ.iter().filter(|at| !succ.contains(at)).copied().collect::<Vec<_>>());
            Some((LirBlock { succ: succ.into(), ..parent.with_insns(insns).into_inner() }, tail.at, into))
        })();
        if let Some((made, tail, into)) = made {
            copies.push((index, made, tail, into));
        }
    }
    // A copy that makes a second entry into a loop is refused: an irreducible
    // body has no estimate of its loops. Checked once for all the copies, as
    // each is checked after the ones before it only when the whole is not
    // reducible.
    // One by one, each checked after the ones before it.
    let sequential = |odds: &mut BlockOdds, copies: Vec<(usize, LirBlock, i64, Vec<(i64, f64)>)>| {
        let mut blocks = body.blocks.clone();
        for (index, made, tail, into) in copies {
            let before = std::mem::replace(&mut blocks[index], made.into());
            if reducible && irreducible(&blocks) {
                blocks[index] = before;
            } else {
                odds.rerouted(body.blocks[index].at, &body.blocks[index].succ, tail, &into);
            }
        }
        blocks
    };
    let mut blocks: Vec<Block> = body.blocks.clone();
    for (index, made, _, _) in &copies {
        blocks[*index] = made.clone().into();
    }
    if reducible && irreducible(&blocks) {
        blocks = sequential(&mut odds, copies);
    } else {
        #[cfg(test)]
        {
            let mut checked = odds.clone();
            assert!(
                sequential(&mut checked, copies.clone()) == blocks
                    && checked == {
                        let mut all = odds.clone();
                        for (index, _, tail, into) in &copies {
                            all.rerouted(body.blocks[*index].at, &body.blocks[*index].succ, *tail, into);
                        }
                        all
                    },
                "the copies checked once are not the copies checked one by one"
            );
        }
        for (index, _, tail, into) in &copies {
            odds.rerouted(body.blocks[*index].at, &body.blocks[*index].succ, *tail, into);
        }
    }
    _reachable(&LirBody { odds, ..body.clone() }, blocks)
}

/// A tail's printed instructions where it may be copied, and the block it
/// falls through to: at most `TAIL_DUPLICATION` besides the branches it ends
/// in, every one a plain operation.
fn _duplicable(
    bits: u32,
    tail: &LirBlock,
    next: Option<i64>,
) -> Option<(Vec<Arc<Insn>>, Option<i64>)> {
    if !tail.phis.is_empty() {
        return None;
    }
    // gcc's bb-reorder `copy_bb_p` for a block that only returns: copied up to
    // `max-grow-copy-bb-insns` jumps of code.
    if _return_tail(bits, tail) {
        return Some((_real(tail), None));
    }
    let real = _real(tail);
    let ops: Vec<Semantics> = real.iter().map(|one| one.what.clone()).collect::<Option<_>>()?;
    let jumps = ops
        .iter()
        .rev()
        .take_while(|one| matches!(one.op, Operation::Jump | Operation::Branch) && !one.indirect)
        .count();
    let plain = ops[..ops.len() - jumps]
        .iter()
        .all(
            |one| !matches!(
                one.op,
                Operation::Call
                    | Operation::Return
                    | Operation::Data
                    | Operation::Barrier
                    | Operation::Jump
                    | Operation::Branch
            ),
        );
    if jumps == 0
        || jumps > 2
        || ops.len() - jumps > TAIL_DUPLICATION
        || !plain
        || real.iter().any(|one| one.group.is_some())
    {
        return None;
    }
    let ends = ops.last().is_some_and(|one| one.op == Operation::Jump);
    let falls = if ends { None } else { Some(next.filter(|at| tail.succ.contains(at))?) };
    Some((real, falls))
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

/// What the target charges for the ways a block can leave, in clocks.
pub struct Branches {
    /// A `jmp`.
    jump: f64,
    /// A `jcc` that is taken.
    taken: f64,
    /// A `jcc` that falls through.
    not_taken: f64,
}

impl Branches {
    /// The prices on `cpu`; a CPU that states none leaves its loops as they
    /// are.
    fn of(cpu: &cpu::Profile) -> Self {
        let clocks = |form: &str| cpu.cost(form).map_or(f64::NAN, |one| one as f64);
        Self { jump: clocks("jmp_short"), taken: clocks("jcc"), not_taken: clocks("jcc_not_taken") }
    }
}

/// The longest run of blocks tried at each turn.
const ROTATION_BLOCKS: usize = 24;

/// A loop laid out as one run of blocks, started at another of its blocks when
/// that takes fewer jumps, as gcc's bb-reorder `rotate_loop`: the latch then
/// falls into the header and the loop is entered by a jump to it. A pass round
/// a loop whose body is a diamond took two jumps, one into an arm or over it
/// and one back to the header; it takes one. Counted by the edges that are not
/// to the next block, each as often as the estimate runs it.
fn rotated(
    body: &LirBody,
    price: &Branches,
) -> Result<LirBody, masm::Unprintable> {
    if body.source_order || [price.jump, price.taken, price.not_taken].iter().any(|one| one.is_nan()) {
        return Ok(body.clone());
    }
    let mut order = body.blocks.clone();
    let mut position: IndexMap<i64, usize> = order.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    // The runs a branch goes back over: from its target to the farthest block
    // that goes back to it.
    let mut latest: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for (index, block) in order.iter().enumerate() {
        for to in &block.succ {
            if let Some(&header) =
                position.get(to).filter(|&&header| header <= index && index - header < ROTATION_BLOCKS)
            {
                let last = latest.entry(header).or_insert(index);
                *last = (*last).max(index);
            }
        }
    }
    if latest.is_empty() {
        return Ok(body.clone());
    }
    let mut entered: Option<IndexMap<i64, Vec<i64>>> = None;
    // Of the body as it is laid out: the copies of tails made its entries.
    let mut estimate: Option<Arc<Frequency>> = None;
    let mut changed = false;
    let runs: Vec<(usize, usize)> = latest.iter().map(|(&header, &last)| (header, last + 1)).collect();
    for &(start, end) in &runs {
        // A run round another is left as the inner one places it: rotating one
        // round another only moves its test. Nor one entered anywhere
        // but at its first block.
        // A run without a diamond in it has nothing to gain.
        if latest.range(start + 1..end).next().is_some()
            || order[start..end].iter().filter(|block| block.succ.len() > 1).count() < 2
            || order[start..end].iter().any(|block| block.at == body.entry)
        {
            continue;
        }
        let from = entered.get_or_insert_with(|| {
            let mut from: IndexMap<i64, Vec<i64>> = IndexMap::default();
            for block in &order {
                for to in &block.succ {
                    from.entry(*to).or_default().push(block.at);
                }
            }
            from
        });
        let outside = |at: &i64| position.get(at).is_none_or(|&index| index < start || index >= end);
        // The one block the run is entered at from outside, where that is not
        // its first: the run is turned to start there, so the way in falls
        // into it. A run entered at two blocks is left as it is.
        let entered_at: Vec<usize> = (0..end - start)
            .filter(|&at| from.get(&order[start + at].at).is_some_and(|from| from.iter().any(outside)))
            .collect();
        let only = match entered_at[..] {
            [] | [0] => None,
            [at] => Some(at),
            _ => continue,
        };
        let frequency = estimate.get_or_insert_with(|| Frequency::of(body));
        // The jumps a run of blocks takes, `after` being what follows it and
        // `before` what leads into it.
        // A block with a branch in its middle leaves by its last jump alone:
        // the others are no fall-through whatever block comes next.
        let falls_into = |block: &LirBlock, to: i64| -> bool {
            let real = _real(block);
            let branches =
                real.iter().filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Branch)).count();
            branches <= 1
                || real
                    .last()
                    .and_then(|one| one.what.as_ref())
                    .is_some_and(|what| what.op == Operation::Jump && what.target == Some(to))
        };
        let clocks = |run: &[Block], before: Option<&Block>, after: Option<i64>| -> f64 {
            let sequence: Vec<&Block> = before.into_iter().chain(run.iter()).collect();
            let mut total = 0.0;
            for (index, block) in sequence.iter().enumerate() {
                let next = sequence.get(index + 1).map(|one| one.at).or(after);
                let succ: Vec<i64> = block.succ.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
                let went = |to: i64| frequency.edge(block.at, to);
                let simple = _real(block)
                    .iter()
                    .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Branch))
                    .count()
                    <= 1;
                total += match succ[..] {
                    [] => 0.0,
                    [only] if Some(only) == next => 0.0,
                    [only] => went(only) * price.jump,
                    // One way is the `jcc`; the other falls through when it is
                    // next, else follows a `jmp` of its own, the colder.
                    [one, other] if simple => {
                        let by = |taken: i64, kept: i64| {
                            went(taken) * price.taken
                                + went(kept) * (price.not_taken + if Some(kept) == next { 0.0 } else { price.jump })
                        };
                        match next {
                            Some(next) if next == one => by(other, one),
                            Some(next) if next == other => by(one, other),
                            _ => by(one, other).min(by(other, one)),
                        }
                    }
                    _ => succ
                        .iter()
                        .map(|&to| if Some(to) == next && falls_into(block, to) { 0.0 } else { went(to) * price.taken })
                        .sum(),
                };
            }
            total
        };
        let before = start.checked_sub(1).map(|index| &order[index]);
        let after = order.get(end).map(|block| block.at);
        let run = &order[start..end];
        let current = clocks(run, before, after);
        let best = (1..run.len())
            .filter(|at| only.is_none_or(|one| *at == one))
            .map(|at| {
                let turned: Vec<Block> = run[at..].iter().chain(&run[..at]).cloned().collect();
                (clocks(&turned, before, after), at)
            })
            .min_by(|one, other| one.0.total_cmp(&other.0));
        // The estimate does not conserve flow: what a block is said to receive
        // is not what it is said to send. A gain smaller than that is the
        // estimate's own error, not a gain.
        let noise: f64 = run
            .iter()
            .map(|block| {
                let received: f64 =
                    from.get(&block.at).map_or(0.0, |from| from.iter().map(|&at| frequency.edge(at, block.at)).sum());
                let sent: f64 = block
                    .succ
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(|to| frequency.edge(block.at, to))
                    .sum();
                (received - sent).abs() * price.taken
            })
            .sum();
        if let Some((_, at)) = best.filter(|(cost, _)| current - *cost > noise) {
            let turned: Vec<Block> = run[at..].iter().chain(&run[..at]).cloned().collect();
            order.splice(start..end, turned);
            for (index, block) in order.iter().enumerate().take(end).skip(start) {
                position.insert(block.at, index);
            }
            changed = true;
        }
    }
    if !changed {
        return Ok(body.clone());
    }
    // A block that leaves for several with no instruction choosing (a switch
    // table) is the placer's to refuse; here it is left.
    let Ok(explicit) = _explicit(body) else { return Ok(body.clone()) };
    let by_at: IndexMap<i64, Block> = explicit.into_iter().map(|block| (block.at, block)).collect();
    let turned = order.iter().map(|block| by_at[&block.at].clone()).collect();
    Ok(body.with_blocks(_without_jumps_to_next(turned)))
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

// Entries of the search for the next trace that `_placed` looked at.
#[cfg(test)]
thread_local! {
    pub(crate) static EXAMINED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn examined(count: usize) {
    #[cfg(test)]
    EXAMINED.with(|one| one.set(one.get() + count));
    #[cfg(not(test))]
    let _ = count;
}

/// A waiting block for `_placed` to start a trace at, ranked as it picks: the
/// likeliest to run, then the one whose placed predecessor is latest, then the
/// last in source order.
struct Nearest {
    busy: f64,
    reached: usize,
    place: usize,
    at: i64,
}

impl PartialEq for Nearest {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Nearest {}

impl PartialOrd for Nearest {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Nearest {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.busy.total_cmp(&other.busy).then(self.reached.cmp(&other.reached)).then(self.place.cmp(&other.place))
    }
}

/// `placed`, for short code where `size`, else by estimated frequency: the
/// likelier edge falls through, a diamond's likelier arm falls into its
/// join, an arm leaves its join only for a block before it that reaches the
/// join `LIKELY`, and a new trace starts at the busiest block left.
fn _placed(
    body: &LirBody,
    size: bool,
) -> Result<LirBody, masm::Unprintable> {
    let explicit = _explicit(body)?;
    if body.source_order {
        return Ok(body.with_blocks(explicit));
    }
    let by_at: IndexMap<i64, Block> = explicit.iter().map(|block| (block.at, block.clone())).collect();
    let natural = loopy::loops(&explicit, Some(body.entry));
    let tests = _tests(&natural, body.entry, &by_at);
    // loops() is innermost first.  A block in nested loops follows the nearest
    // loop's trace before an exit from it; the outer trace resumes afterwards.
    // Each loop's body is the forest's view, shared by its blocks: a copy for
    // each block was the square of a nest's depth.
    let mut inside: IndexMap<i64, loopy::Body> = IndexMap::default();
    for found in &natural {
        for at in &found.body {
            inside.entry(*at).or_insert_with(|| found.body.clone());
        }
    }
    let mut predecessors: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &explicit {
        for to in &block.succ {
            predecessors.entry(*to).or_default().push(block.at);
        }
    }
    let busy = Frequency::over(body, &explicit);
    // For size, frequency only orders what is the same size either way.
    let (odds, ties) = if size { (None, Some(&busy)) } else { (Some(&busy), None) };
    let mut order: Vec<Block> = Vec::new();
    let mut done: HashSet<i64> = HashSet::default();
    let mut current: Option<i64> = Some(body.entry);
    let mut source: Option<i64> = None;
    let empty = loopy::Body::from_blocks([]);
    // What the search for the next trace needs, kept as blocks are placed and
    // not worked out again over every block waiting: where each is in source
    // order, the first waiting block (any, and not cold), and, for a waiting
    // block that is not cold, the place of the block placed last that reaches
    // it, in a heap that is cleaned as it is read.
    let position: IndexMap<i64, usize> = explicit.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let (mut first_waiting, mut first_hot) = (0_usize, 0_usize);
    let mut last_reach: IndexMap<i64, usize> = IndexMap::default();
    let mut nearest_heap: std::collections::BinaryHeap<Nearest> = std::collections::BinaryHeap::new();
    while order.len() < explicit.len() {
        while first_hot < explicit.len() && (explicit[first_hot].cold || done.contains(&explicit[first_hot].at)) {
            examined(1);
            first_hot += 1;
        }
        if current.is_some_and(|at| by_at.get(&at).is_some_and(|block| block.cold)) && first_hot < explicit.len() {
            current = None; // after the hot blocks
        }
        if current.is_none_or(|at| done.contains(&at) || !by_at.contains_key(&at)) {
            // Next to the block last placed that reaches it: a trace started
            // in source order may land far past its branch.
            while let Some(top) = nearest_heap.peek() {
                examined(1);
                if done.contains(&top.at) || last_reach.get(&top.at) != Some(&top.reached) {
                    nearest_heap.pop();
                } else {
                    break;
                }
            }
            while first_waiting < explicit.len() && done.contains(&explicit[first_waiting].at) {
                examined(1);
                first_waiting += 1;
            }
            let nearest = nearest_heap.peek().map(|top| top.at);
            current = Some(
                nearest
                    .or_else(|| (first_hot < explicit.len()).then(|| explicit[first_hot].at))
                    .unwrap_or(explicit[first_waiting].at),
            );
            source = None;
        }
        let at = current.expect("set above");
        if let Some((inner, latches)) = tests.get(&at) {
            if source.is_none_or(|source| !latches.contains(&source)) && !done.contains(inner) {
                // Reached from outside the loop: the body goes here, and the
                // test after the latch that jumps back to it,
                // so a pass takes one branch.
                (current, source) = (Some(*inner), None);
                continue;
            }
        }
        // A join's other arm goes just before it, once everything reaching the
        // arm is placed: the arm falls into the join and the placed side jumps
        // over it. Left for later, it lands after the return, both of its jumps
        // long. By frequency, unless the block before it, whose
        // fall-through the arm takes, reaches the join as `LIKELY` as
        // MachineBlockPlacement asks: else the shorter layout stands.
        let before = order.last().filter(|last| last.succ.contains(&at));
        let is_arm = |one: &&Block| {
            one.at != at
                && !one.cold
                && !done.contains(&one.at)
                && one.succ == [at]
                && predecessors.get(&one.at).is_some_and(|from| from.iter().all(|from| done.contains(from)))
                && odds.is_none_or(|busy| {
                    before.is_none_or(|before| {
                        busy.edge(one.at, at) * branchprob::LIKELY / (1.0 - branchprob::LIKELY)
                            >= busy.edge(before.at, at)
                    })
                })
        };
        // Only a block that jumps to it can be, in the order blocks come.
        let arm = predecessors.get(&at).into_iter().flatten().filter_map(|from| by_at.get(from)).find(is_arm);
        #[cfg(test)]
        assert_eq!(
            arm.map(|one| one.at),
            explicit.iter().find(is_arm).map(|one| one.at),
            "the arm found among the predecessors is not the first of all the blocks"
        );
        if let Some(arm) = arm {
            (current, source) = (Some(arm.at), None);
            continue;
        }
        let block = &by_at[&at];
        order.push(block.clone().into());
        done.insert(at);
        for to in &block.succ {
            let Some(&place) = position.get(to) else { continue };
            if done.contains(to) || explicit[place].cold {
                continue;
            }
            last_reach.insert(*to, order.len() - 1);
            nearest_heap.push(Nearest {
                busy: odds.map_or(0.0, |busy| busy.block(*to)),
                reached: order.len() - 1,
                place,
                at: *to,
            });
        }
        (current, source) = (
            _onward(body.bits, block, &done, inside.get(&block.at).map_or(&empty, |one| one), Some(&by_at), odds, ties),
            Some(at),
        );
    }
    Ok(body.with_blocks(order))
}

/// The block to place next: where the final jump goes, or else where the branch
/// before it goes.
///
/// The branch's target second, so that `jcc target; jmp placed` becomes one
/// inverted branch.
pub fn _onward(
    bits: u32,
    block: &LirBlock,
    done: &HashSet<i64>,
    inside: &loopy::Body,
    by_at: Option<&IndexMap<i64, Block>>,
    odds: Option<&Frequency>,
    ties: Option<&Frequency>,
) -> Option<i64> {
    let real: Vec<&Semantics> =
        block.insns.iter().filter_map(|one| one.what.as_ref()).filter(|what| what.op != Operation::Nothing).collect();
    if real.last().is_none_or(|last| last.op != Operation::Jump) {
        return None;
    }
    let mut targets = vec![real[real.len() - 1].target];
    if real.len() > 1 && real[real.len() - 2].op == Operation::Branch {
        targets.push(real[real.len() - 2].target);
        // A conditional arm that rejoins the other edge is the complete
        // straight-line trace: placing it first removes both the explicit
        // false-edge jump and the arm's jump to the join.  Following the
        // source fall-through first instead strands the arm after its join,
        // as the QB frontend's two clamp assignments demonstrated.
        let (jump_target, branch_target) = (targets[0], targets[1]);
        let arm = by_at.and_then(|by_at| branch_target.and_then(|at| by_at.get(&at)));
        let passage = match (by_at, jump_target) {
            (Some(by_at), Some(at)) if by_at.contains_key(&at) => _passage(&by_at[&at]),
            _ => None,
        };
        let join = if passage.is_none() { jump_target } else { passage };
        if arm.is_some_and(|arm| arm.succ.len() == 1 && Some(arm.succ[0]) == join) {
            targets = vec![branch_target, jump_target];
        }
    }
    // By frequency, the likelier edge first. In a diamond short of `LIKELY`
    // the arm rule keeps both arms before the join, so there the likelier
    // arm goes second and falls into the join instead of jumping over the
    // other; `LIKELY` and over, it falls through and the other leaves.
    if let (Some(busy), [Some(first), Some(second)]) = (odds, targets.as_slice()) {
        let (one, other) = (busy.edge(block.at, *first), busy.edge(block.at, *second));
        let join =
            |at: i64| by_at.and_then(|by_at| by_at.get(&at)).and_then(|arm| (arm.succ.len() == 1).then(|| arm.succ[0]));
        let diamond = join(*first).is_some() && join(*first) == join(*second);
        let likely = |hot: f64, cold: f64| hot > 0.0 && hot >= branchprob::LIKELY * (hot + cold);
        let swap = if diamond { likely(other, one) || (one > other && !likely(one, other)) } else { other > one };
        if swap {
            targets.reverse();
        }
    }
    // For size, a diamond whose arms keep short jumps in either order is the
    // same size either way: its likelier arm goes second, into the join.
    if let (Some(busy), Some(by_at), [Some(first), Some(second)]) = (ties, by_at, targets.as_slice()) {
        let arm = |at: &i64| by_at.get(at).filter(|arm| arm.succ.len() == 1);
        if let (Some(one), Some(other)) = (arm(first), arm(second))
            && one.succ == other.succ
            && [one, other]
                .into_iter()
                .all(|arm| _arm_bytes(bits, arm).is_some_and(|bytes| short_reaches(bytes + SHORT_JUMP)))
            && busy.edge(block.at, *first) > busy.edge(block.at, *second)
        {
            targets.reverse();
        }
    }
    // A cold successor goes last; see `LirBlock::cold`.
    if let Some(by_at) = by_at {
        targets.sort_by_key(|target| target.is_some_and(|at| by_at.get(&at).is_some_and(|block| block.cold)));
    }
    // Only a jump to a placed block: threading removes it, and placing it
    // next would strand the other edge behind a jump of its own.
    let spent = |target: &Option<i64>| {
        by_at.is_some_and(|by_at| {
            target.is_some_and(|at| {
                by_at.get(&at).is_some_and(|block| _passage(block).is_some_and(|to| done.contains(&to)))
            })
        })
    };
    // Keep a loop chain together before following an exit.  The final jump is
    // still preferred when both edges stay in the loop, preserving the source
    // fall-through unless doing so would strand the rest of the loop.
    for target in &targets {
        if target.is_none_or(|at| !done.contains(&at))
            && target.is_some_and(|at| inside.contains(&at))
            && !spent(target)
        {
            return *target;
        }
    }
    for target in &targets {
        if target.is_none_or(|at| !done.contains(&at)) && !spent(target) {
            return *target;
        }
    }
    for target in &targets {
        if target.is_none_or(|at| !done.contains(&at)) {
            return *target;
        }
    }
    None
}

/// Each loop header that only decides whether to go round: its one successor
/// inside, and its latches.
pub fn _tests(
    natural: &[Loop],
    entry: i64,
    by_at: &IndexMap<i64, Block>,
) -> IndexMap<i64, (i64, BTreeSet<i64>)> {
    let mut found = IndexMap::default();
    for found_loop in natural {
        let block = &by_at[&found_loop.header];
        let real: Vec<&Semantics> = block
            .insns
            .iter()
            .filter_map(|one| one.what.as_ref())
            .filter(|what| what.op != Operation::Nothing)
            .collect();
        let inner: Vec<i64> =
            block.succ.iter().copied().filter(|at| found_loop.body.contains(at) && *at != found_loop.header).collect();
        if found_loop.header != entry
            && real.len() >= 2
            && real[real.len() - 2].op == Operation::Branch
            && block.succ.iter().collect::<HashSet<_>>().len() == 2
            && inner.len() == 1
        {
            found.insert(found_loop.header, (inner[0], found_loop.latches.clone()));
        }
    }
    found
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

/// An inserted anchor: no bytes, no machine effect, only a virtual definition.
fn _inert(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing) && one.inserted() && one.spread.is_empty()
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
    return_overhead: i64,
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
            let tail_size = _duplicable_return_size(body.bits, tail, return_overhead);
            if tail.at == body.entry
                || tail_size.is_none()
                || parents.len() < 2
                || parents.iter().any(|parent| parent.succ != [tail.at])
            {
                continue;
            }
            let tail_size = tail_size.expect("checked above");
            let prepared: Vec<Option<(&LirBlock, i64)>> =
                parents.iter().map(|parent| _return_parent(body.bits, parent, tail.at)).collect();
            if prepared.iter().any(Option::is_none) {
                continue;
            }
            let copies: Vec<(&LirBlock, i64)> = prepared.into_iter().flatten().collect();
            let saved: i64 = copies.iter().map(|(_parent, jump_size)| jump_size).sum();
            if saved == 0 || parents.len() as i64 * tail_size > tail_size + saved {
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

/// Selected bytes in a source-unowned terminal return block.
pub fn _duplicable_return_size(
    bits: u32,
    block: &LirBlock,
    return_overhead: i64,
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
    Some(return_overhead + emitted.iter().flatten().map(|one| one.code.len() as i64).sum::<i64>())
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
        if last_what.op == Operation::Jump && target == after {
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
