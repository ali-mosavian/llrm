//! Port of `qbopt/backend/jumps.py`: block order, and the jumps it makes
//! redundant.
//!
//! Runs on the allocated body, after the last phase that adds or empties blocks:
//! edge splits and their undoing leave jumps to the next block, and the raise's
//! `if false goto` beside a `goto` leaves a `jcc` over a block that only jumps.
//! `placed` orders the blocks; `threaded` keeps that order and drops the jumps
//! and the blocks nothing reaches.

use std::collections::BTreeSet;
use crate::support::hash::HashSet;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::frequency::Frequency;
use llrm_analysis::branchprob;
use crate::backend::layout::_OPPOSITE;
use crate::backend::objbuild::{SHORT_JUMP, short_reaches};
use crate::backend::{cpu, machinedce, masm, select};
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::{Exception, LIRTransform};

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

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        self.placed(&body).map_err(|error| error.0)
    }

    fn transform_raising(&mut self, body: LirBody) -> Result<LirBody, Exception> {
        Ok(self.placed(&body)?)
    }
}

impl ControlFlow<'_> {
    /// `optimized`, its executed work on the `cost` channel.
    fn placed(&self, body: &LirBody) -> Result<LirBody, masm::Unprintable> {
        let placed = optimized(body, self.cpu.size)?;
        if crate::support::debug::enabled("blocks") {
            let frequency = crate::analysis::frequency::Frequency::of(&placed);
            for block in &placed.blocks {
                llrm_support::debug!("blocks", "{} {:#06x} freq {} insns {}", placed.name, block.at, frequency.block(block.at), block.insns.len());
            }
        }
        Ok(placed)
    }
}

/// Choose the cheapest common-tail fixed point without adding hot work.
pub fn optimized(body: &LirBody, size: bool) -> Result<LirBody, masm::Unprintable> {
    let mut candidate = _placed(&_hoisted(body), size)?;
    let baseline = threaded(&candidate);
    // Merging one physical tail may make the condition selecting between its
    // former copies dead; deleting that compare can in turn make predecessor
    // tails identical.  Settle those two machine facts before final threading
    // chooses fall-throughs.  Source-owned tails are refused by `_tail_key`.
    for _round in 0..std::cmp::max(1, candidate.blocks.len() + candidate.insns().len()) {
        let before = candidate.clone();
        candidate = machinedce::eliminated(merged(&candidate)?);
        // Python `candidate is before`: both passes return their input itself
        // exactly when they change nothing.
        if candidate == before {
            break;
        }
    }
    let placed = preferred(&baseline, &threaded(&candidate)).clone();
    Ok(inverted(&if size { placed } else { duplicated(&duplicated_tails(&placed), true) }))
}

/// A conditional branch taken to the block laid out next, followed by a jump: the opposite branch to the jump's target, and no jump
/// (LLVM's `analyzeBranch` / `reverseBranchCondition` in BranchFolding). `_step` does this while it places blocks; copying a tail
/// afterwards (`duplicated`) makes more of them.
fn inverted(body: &LirBody) -> LirBody {
    let mut blocks = body.blocks.clone();
    let mut changed = false;
    for index in 0..blocks.len() {
        let (last, before) = _real_tail(&blocks[index]);
        let (Some(last), Some(branch)) = (last, before) else { continue };
        let (last_what, branch_what) = (last.what.as_ref().expect(NO_OP), branch.what.as_ref().expect(NO_OP));
        let after = blocks[index + 1..].iter().find(|next| !next.phis.is_empty() || next.insns.iter().any(|one| !one.is_meta())).map(|next| next.at);
        let opposite = branch_what.name.as_deref().and_then(|name| _OPPOSITE.get(name));
        if last_what.op != Operation::Jump || branch_what.op != Operation::Branch || branch_what.indirect || branch_what.target.is_none() || branch_what.target != after || last_what.target.is_none() {
            continue;
        }
        let Some(opposite) = opposite else { continue };
        let flipped = Arc::new(Insn { what: Some(Semantics { name: Some((*opposite).to_owned()), target: last_what.target, ..branch_what.clone() }), ..(*branch).clone() });
        let insns = blocks[index].insns.iter().filter(|one| !Arc::ptr_eq(one, &last)).map(|one| if Arc::ptr_eq(one, &branch) { Arc::clone(&flipped) } else { Arc::clone(one) }).collect();
        blocks[index] = blocks[index].with_insns(insns);
        changed = true;
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// gcc's `max-grow-copy-bb-insns`: a block is copied while it is at most this many unconditional jumps long.
const COPY_BB_INSNS: usize = 8;

/// gcc's `get_uncond_jump_length`: what the target prices one unconditional jump at, in its long form.
pub(crate) fn uncond_jump_bytes(bits: u32) -> usize {
    let jump = Semantics { name: Some("jmp".into()), target: Some(2), ..Semantics::new(Operation::Jump) };
    select::priced_in(bits, &jump, 0, None, false, false, None).map_or(2, |made| made.code.len())
}

/// The bytes a block may be and still be copied (`copy_bb_p`): `COPY_BB_INSNS` jumps of code.
fn copy_limit(bits: u32) -> usize {
    COPY_BB_INSNS * uncond_jump_bytes(bits)
}

/// A block that only returns: plain operations then a `ret`, no way on, within `limit` bytes and none of it source-owned.
fn _return_tail(bits: u32, block: &LirBlock) -> bool {
    if !block.phis.is_empty() || !block.succ.is_empty() || block.insns.iter().any(|one| one.call.is_some() || one.group.is_some() || one.symbol == Some(true) || !one.spread.is_empty()) {
        return false;
    }
    let real = _real(block);
    let Some((last, rest)) = real.split_last() else { return false };
    // A return that covers source bytes is the program's own (a BASIC module's end spells a runtime call at it): not copied.
    if last.what.as_ref().is_none_or(|what| what.op != Operation::Return) || !last.inserted()
        || rest.iter().any(|one| one.call.is_some() || one.what.as_ref().is_none_or(|what| matches!(what.op, Operation::Barrier | Operation::Branch | Operation::Call | Operation::Data | Operation::Jump | Operation::Return)))
    {
        return false;
    }
    real.iter().try_fold(0usize, |bytes, one| select::priced_in(bits, one.what.as_ref()?, 0, None, false, false, None).map(|made| bytes + made.code.len())).is_some_and(|bytes| bytes <= copy_limit(bits))
}

/// LLVM's `TailDupSize` at -O2: the instructions a tail may hold besides its jumps.
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

/// `duplicated_tails`, or only the tails that return: the jump a copied tail ended in may reach one.
fn duplicated(body: &LirBody, returns: bool) -> LirBody {
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let after: IndexMap<i64, i64> = body.blocks.windows(2).map(|pair| (pair[0].at, pair[1].at)).collect();
    let frequency = Frequency::of(body);
    let mut odds = body.odds.clone();
    let irreducible = |blocks: &[LirBlock]| !loopy::irreducible(blocks, Some(body.entry)).is_empty();
    let reducible = !irreducible(&body.blocks);
    let mut blocks: Vec<LirBlock> = body.blocks.clone();
    for (index, parent) in body.blocks.iter().enumerate() {
        let mut tried = odds.clone();
        let made = (|| {
            let real = _real(parent);
            let Some(last) = real.last().and_then(|one| one.what.as_ref()).filter(|what| what.op == Operation::Jump && !what.indirect) else {
                return None;
            };
            let Some(tail) = last.target.filter(|at| *at != parent.at && *at != body.entry).and_then(|at| by_at.get(&at)) else {
                return None;
            };
            let Some((copied, falls)) = _duplicable(body.bits, tail, after.get(&tail.at).copied()).filter(|(_, falls)| !returns || falls.is_none()).filter(|_| !returns || _return_tail(body.bits, tail)) else {
                return None;
            };
            // Per run of the jump: the fall-through's jump runs as often as the tail falls through.
            if let Some(next) = falls {
                let share = |from: i64, to: i64| frequency.edge(from, to) / frequency.block(from).max(f64::MIN_POSITIVE);
                if share(tail.at, next) >= 1.0 {
                    return None;
                }
            }
            let jump = &parent.insns[parent.insns.iter().rposition(|one| Arc::ptr_eq(one, real.last().expect("checked"))).expect("a real instruction")];
            // The jump's work goes; its source bytes stay, as `_reachable` keeps them.
            let mut insns: Vec<Arc<Insn>> = parent.insns.iter().map(|one| if Arc::ptr_eq(one, jump) { lir::bytes_only(one) } else { Arc::clone(one) }).collect();
            insns.extend(copied.iter().map(|one| _unowned(one, jump)));
            if let Some(next) = falls {
                let mut made = (**jump).clone();
                made.what = Some(Semantics { target: Some(next), ..last.clone() });
                insns.push(_unowned(&made, jump));
            }
            // The edge to the tail becomes the tail's edges, at its share.
            // The edge to the tail becomes the tail's edges, at their odds.
            let into: Vec<(i64, f64)> = tail.succ.iter().map(|next| (*next, body.odds.chance(tail.at, &tail.succ, *next))).collect();
            tried.rerouted(parent.at, &parent.succ, tail.at, &into);
            // The parent's own branches keep their targets.
            let mut succ: Vec<i64> = parent.succ.iter().copied().filter(|at| *at != tail.at).collect();
            succ.extend(tail.succ.iter().filter(|at| !succ.contains(at)).copied().collect::<Vec<_>>());
            Some(LirBlock { succ, ..parent.with_insns(insns) })
        })();
        // A copy that makes a second entry into a loop is refused: an
        // irreducible body has no estimate of its loops.
        if let Some(made) = made {
            let before = std::mem::replace(&mut blocks[index], made);
            if reducible && irreducible(&blocks) {
                blocks[index] = before;
            } else {
                odds = tried;
            }
        }
    }
    _reachable(&LirBody { odds, ..body.clone() }, blocks)
}

/// A tail's printed instructions where it may be copied, and the block it
/// falls through to: at most `TAIL_DUPLICATION` besides the branches it ends
/// in, every one a plain operation.
fn _duplicable(bits: u32, tail: &LirBlock, next: Option<i64>) -> Option<(Vec<Arc<Insn>>, Option<i64>)> {
    if !tail.phis.is_empty() {
        return None;
    }
    // gcc's bb-reorder `copy_bb_p` for a block that only returns: copied up to `max-grow-copy-bb-insns` jumps of code.
    if _return_tail(bits, tail) {
        return Some((_real(tail), None));
    }
    let real = _real(tail);
    let ops: Vec<Semantics> = real.iter().map(|one| one.what.clone()).collect::<Option<_>>()?;
    let jumps = ops.iter().rev().take_while(|one| matches!(one.op, Operation::Jump | Operation::Branch) && !one.indirect).count();
    let plain = ops[..ops.len() - jumps]
        .iter()
        .all(|one| !matches!(one.op, Operation::Call | Operation::Return | Operation::Data | Operation::Barrier | Operation::Jump | Operation::Branch));
    if jumps == 0 || jumps > 2 || ops.len() - jumps > TAIL_DUPLICATION || !plain || real.iter().any(|one| one.group.is_some()) {
        return None;
    }
    let ends = ops.last().is_some_and(|one| one.op == Operation::Jump);
    let falls = if ends { None } else { Some(next.filter(|at| tail.succ.contains(at))?) };
    Some((real, falls))
}

/// `one` copied where `beside` was, claiming no bytes.
fn _unowned(one: &Insn, beside: &Insn) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let mut made = one.clone();
    made.at = beside.at;
    made.covers = Some((at, at));
    Arc::new(made)
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

/// `placed`, for short code where `size`, else by estimated frequency: the
/// likelier edge falls through, a diamond's likelier arm falls into its
/// join, an arm leaves its join only for a block before it that reaches the
/// join `LIKELY`, and a new trace starts at the busiest block left.
fn _placed(body: &LirBody, size: bool) -> Result<LirBody, masm::Unprintable> {
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
            block.insns.push(Arc::new(jump));
        }
        explicit.push(block);
    }
    if body.source_order {
        return Ok(body.with_blocks(explicit));
    }
    let by_at: IndexMap<i64, LirBlock> = explicit.iter().map(|block| (block.at, block.clone())).collect();
    let natural = loopy::loops(&explicit, Some(body.entry));
    let tests = _tests(&natural, body.entry, &by_at);
    // loops() is innermost first.  A block in nested loops follows the nearest
    // loop's trace before an exit from it; the outer trace resumes afterwards.
    let mut inside: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
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
    let mut order: Vec<LirBlock> = Vec::new();
    let mut done: HashSet<i64> = HashSet::default();
    let mut current: Option<i64> = Some(body.entry);
    let mut source: Option<i64> = None;
    let empty = BTreeSet::new();
    while order.len() < explicit.len() {
        if current.is_some_and(|at| by_at.get(&at).is_some_and(|block| block.cold))
            && explicit.iter().any(|one| !one.cold && !done.contains(&one.at))
        {
            current = None; // after the hot blocks
        }
        if current.is_none_or(|at| done.contains(&at) || !by_at.contains_key(&at)) {
            let waiting: Vec<&LirBlock> = explicit.iter().filter(|block| !done.contains(&block.at)).collect();
            // Next to the block last placed that reaches it: a trace started
            // in source order may land far past its branch.
            let placed_at: IndexMap<i64, usize> = order.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
            let nearest = waiting
                .iter()
                .filter(|block| !block.cold)
                .filter_map(|block| predecessors.get(&block.at)?.iter().filter_map(|from| placed_at.get(from)).max().map(|last| (*last, *block)))
                .max_by(|(one, first), (other, second)| match odds {
                    Some(busy) => busy.block(first.at).total_cmp(&busy.block(second.at)).then(one.cmp(other)),
                    None => one.cmp(other),
                })
                .map(|(_, block)| block);
            current = Some(nearest.or_else(|| waiting.iter().find(|block| !block.cold).copied()).unwrap_or(waiting[0]).at);
            source = None;
        }
        let at = current.expect("set above");
        if let Some((inner, latches)) = tests.get(&at) {
            if source.is_none_or(|source| !latches.contains(&source)) && !done.contains(inner) {
                // Reached from outside the loop: the body goes here, and the test after the latch
                // that jumps back to it, so a pass takes one branch.
                (current, source) = (Some(*inner), None);
                continue;
            }
        }
        // A join's other arm goes just before it, once everything reaching the
        // arm is placed: the arm falls into the join and the placed side jumps
        // over it. Left for later, it lands after the return, both of its jumps long.
        // By frequency, unless the block before it, whose fall-through the
        // arm takes, reaches the join as `LIKELY` as MachineBlockPlacement
        // asks: else the shorter layout stands.
        let before = order.last().filter(|last| last.succ.contains(&at));
        let arm = explicit.iter().find(|one| {
            one.at != at
                && !one.cold
                && !done.contains(&one.at)
                && one.succ == [at]
                && predecessors.get(&one.at).is_some_and(|from| from.iter().all(|from| done.contains(from)))
                && odds.is_none_or(|busy| before.is_none_or(|before| busy.edge(one.at, at) * branchprob::LIKELY / (1.0 - branchprob::LIKELY) >= busy.edge(before.at, at)))
        });
        if let Some(arm) = arm {
            (current, source) = (Some(arm.at), None);
            continue;
        }
        let block = &by_at[&at];
        order.push(block.clone());
        done.insert(at);
        (current, source) = (_onward(body.bits, block, &done, inside.get(&block.at).unwrap_or(&empty), Some(&by_at), odds, ties), Some(at));
    }
    Ok(body.with_blocks(order))
}

/// The block to place next: where the final jump goes, or else where the branch before it goes.
///
/// The branch's target second, so that `jcc target; jmp placed` becomes one inverted branch.
pub fn _onward(
    bits: u32,
    block: &LirBlock,
    done: &HashSet<i64>,
    inside: &BTreeSet<i64>,
    by_at: Option<&IndexMap<i64, LirBlock>>,
    odds: Option<&Frequency>,
    ties: Option<&Frequency>,
) -> Option<i64> {
    let real: Vec<&Semantics> = block
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|what| what.op != Operation::Nothing)
        .collect();
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
        let join = |at: i64| by_at.and_then(|by_at| by_at.get(&at)).and_then(|arm| (arm.succ.len() == 1).then(|| arm.succ[0]));
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
            && [one, other].into_iter().all(|arm| _arm_bytes(bits, arm).is_some_and(|bytes| short_reaches(bytes + SHORT_JUMP)))
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
            target.is_some_and(|at| by_at.get(&at).is_some_and(|block| _passage(block).is_some_and(|to| done.contains(&to))))
        })
    };
    // Keep a loop chain together before following an exit.  The final jump is
    // still preferred when both edges stay in the loop, preserving the source
    // fall-through unless doing so would strand the rest of the loop.
    for target in &targets {
        if target.is_none_or(|at| !done.contains(&at)) && target.is_some_and(|at| inside.contains(&at)) && !spent(target) {
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

/// Each loop header that only decides whether to go round: its one successor inside, and its latches.
pub fn _tests(
    natural: &[Loop],
    entry: i64,
    by_at: &IndexMap<i64, LirBlock>,
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
        let inner: Vec<i64> = block
            .succ
            .iter()
            .copied()
            .filter(|at| found_loop.body.contains(at) && *at != found_loop.header)
            .collect();
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
    let mut at: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    while _step(&mut body, &mut at, &protected) {}
    body
}

/// An edge block's anchors moved up before the branch into it, where the other way reads nothing they define.
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
    let mut live: Option<IndexMap<i64, BTreeSet<u32>>> = None;
    for position in 0..blocks.len() {
        let block = &blocks[position];
        let (anchors, rest): (Vec<Arc<Insn>>, Vec<Arc<Insn>>) = block.insns.iter().cloned().partition(|one| _inert(one));
        let parents = predecessors.get(&block.at);
        if anchors.is_empty()
            || !block.phis.is_empty()
            || block.succ.len() != 1
            || rest.iter().any(|one| one.what.as_ref().is_none_or(|what| what.op != Operation::Jump))
            || parents.map_or(0, BTreeSet::len) != 1
        {
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
        let reads = live.get_or_insert_with(|| _live_values(&blocks));
        if anchors.iter().flat_map(|one| &one.defines).any(|value| reads.get(&other).is_some_and(|live| live.contains(value))) {
            continue;
        }
        let mut cut = before.insns.len();
        while cut > 0 && before.insns[cut - 1].what.as_ref().is_some_and(|what| matches!(what.op, Operation::Branch | Operation::Jump)) {
            cut -= 1;
        }
        let insns = before.insns[..cut].iter().chain(&anchors).chain(&before.insns[cut..]).cloned().collect();
        let (at, before) = (index[&parent], before.with_insns(insns));
        blocks[at] = before;
        blocks[position] = blocks[position].with_insns(rest);
        live = None;
    }
    body.with_blocks(blocks)
}

/// An inserted anchor: no bytes, no machine effect, only a virtual definition.
fn _inert(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing) && one.inserted() && one.spread.is_empty()
}

/// Per block, the virtual values live on entry.
fn _live_values(blocks: &[LirBlock]) -> IndexMap<i64, BTreeSet<u32>> {
    let by_at: IndexMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, block)).collect();
    let mut carried: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    for block in blocks {
        for phi in &block.phis {
            for (at, value) in &phi.incoming {
                carried.entry(*at).or_default().insert(*value);
            }
        }
    }
    let mut into: IndexMap<i64, BTreeSet<u32>> = by_at.keys().map(|at| (*at, BTreeSet::new())).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for (at, block) in &by_at {
            let mut live = carried.get(at).cloned().unwrap_or_default();
            for to in &block.succ {
                if let Some(after) = into.get(to) {
                    live.extend(after.iter().copied());
                }
            }
            for one in block.insns.iter().rev() {
                for value in one.defines.iter().copied().chain(one.delivers.iter().map(|(held, _)| held.value)) {
                    live.remove(&value);
                }
                live.extend(one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)));
            }
            for phi in &block.phis {
                live.remove(&phi.result);
            }
            if into[at] != live {
                into.insert(*at, live);
                changing = true;
            }
        }
    }
    into
}

/// Python's `_tail_key` tuple.
pub type TailKey = (
    Vec<(Semantics, BTreeSet<Register>, BTreeSet<Register>, Vec<Register>, Vec<Register>, bool)>,
    Vec<i64>,
);

/// Merge physically identical allocated tails after fallthroughs are explicit.
pub fn merged(body: &LirBody) -> Result<LirBody, masm::Unprintable> {
    // Removing a duplicate block is sound only when every incoming edge is an
    // instruction that can be retargeted. `placed` establishes exactly that
    // form. Decline a body presented at an earlier pipeline boundary.
    for block in &body.blocks {
        if masm::_falls_to(block, &body.name)?.is_some() {
            return Ok(body.clone());
        }
    }
    let mut body = body.clone();
    loop {
        let mut groups: IndexMap<TailKey, Vec<&LirBlock>> = IndexMap::default();
        for block in &body.blocks {
            if let Some(key) = _tail_key(block) {
                groups.entry(key).or_default().push(block);
            }
        }
        let mut redirect: IndexMap<i64, i64> = IndexMap::default();
        // A copy's values are the kept block's, the same registers by name.
        let mut rename: IndexMap<u32, u32> = IndexMap::default();
        for copies in groups.values() {
            if copies.len() < 2 {
                continue;
            }
            let canonical = copies.iter().find(|block| block.at == body.entry).unwrap_or(&copies[0]);
            for copy in copies.iter().filter(|block| !std::ptr::eq(**block, *canonical)) {
                redirect.insert(copy.at, canonical.at);
                // An anchor left by an identity copy defines what it reads.
                let identities = copy.insns.iter().filter(|one| _inert(one)).flat_map(|one| one.defines.iter().zip(&one.uses));
                let mut pairs: Vec<(u32, u32)> = identities.map(|(from, to)| (*from, *to)).collect();
                for (one, kept) in _real(copy).iter().zip(_real(canonical)) {
                    pairs.extend(one.defines.iter().zip(&kept.defines).chain(one.uses.iter().zip(&kept.uses)).map(|(from, to)| (*from, *to)));
                }
                for (from, to) in pairs.into_iter().filter(|(from, to)| from != to) {
                    let (from, to) = (_root(&rename, from), _root(&rename, to));
                    if from != to {
                        rename.insert(from, to);
                    }
                }
            }
        }
        if redirect.is_empty() {
            return Ok(body);
        }
        if !rename.is_empty() {
            let rename: IndexMap<u32, u32> = rename.keys().map(|one| (*one, _root(&rename, *one))).collect();
            body = body.with_blocks(
                body.blocks
                    .iter()
                    .map(|block| block.with_insns(block.insns.iter().map(|one| crate::backend::spiller::_renamed(one, &rename)).collect()))
                    .collect(),
            );
        }
        body = _redirected(&body, &redirect);
    }
}

/// The value `one` is renamed to, through every rename after it.
fn _root(rename: &IndexMap<u32, u32>, mut one: u32) -> u32 {
    while let Some(next) = rename.get(&one) {
        one = *next;
    }
    one
}

/// The complete physical form of a fresh, mergeable block.
pub fn _tail_key(block: &LirBlock) -> Option<TailKey> {
    if !block.phis.is_empty() {
        return None;
    }
    let mut shaped = Vec::new();
    for one in _real(block) {
        let Some(what) = &one.what else {
            return None;
        };
        if !one.inserted()
            || matches!(what.op, Operation::Barrier | Operation::Call | Operation::Data)
            || one.symbol == Some(true)
            || one.group.is_some()
        {
            return None;
        }
        shaped.push((
            what.clone(),
            one.clobbers.clone(),
            one.clobbers_high.clone(),
            one.requires.iter().map(|(_value, register)| *register).collect(),
            one.delivers.iter().map(|(_value, register)| *register).collect(),
            one.frame_adjust,
        ));
    }
    if shaped.is_empty() {
        return None;
    }
    let successors: BTreeSet<i64> = block.succ.iter().copied().collect();
    Some((shaped, successors.into_iter().collect()))
}

/// Redirect every explicit edge, remove duplicate blocks, and fold diamonds.
pub fn _redirected(body: &LirBody, redirect: &IndexMap<i64, i64>) -> LirBody {
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
                    }
                }
            }
            insns.push(one);
        }
        let successors: IndexSet<i64> = block.succ.iter().map(|at| target(*at)).collect();
        blocks.push(_fold_converged(LirBlock { succ: successors.into_iter().collect(), ..block.with_insns(insns) }));
    }
    let entry = target(body.entry);
    LirBody { entry, ..body.with_blocks(blocks) }
}

/// A conditional whose two CFG edges became one is an unconditional edge.
pub fn _fold_converged(block: LirBlock) -> LirBlock {
    if block.succ.len() != 1 {
        return block;
    }
    let destination = block.succ[0];
    let real = _real(&block);
    let last = real.last();
    if let Some(last) = last {
        if last
            .what
            .as_ref()
            .is_some_and(|what| what.op == Operation::Jump && what.target == Some(destination))
            && real.len() > 1
        {
            let branch = &real[real.len() - 2];
            if branch
                .what
                .as_ref()
                .is_some_and(|what| what.op == Operation::Branch && what.target == Some(destination))
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

/// Static and profile-free dynamic machine-instruction counts.
pub fn _work(body: &LirBody) -> (i64, f64) {
    let busy = Frequency::of(body);
    let counts: IndexMap<i64, i64> = body.blocks.iter().map(|block| (block.at, _real(block).len() as i64)).collect();
    (counts.values().sum(), counts.iter().map(|(at, count)| *count as f64 * busy.block(*at)).sum())
}

/// Take tail sharing only when size falls without adding executed work.
pub fn preferred<'a>(before: &'a LirBody, after: &'a LirBody) -> &'a LirBody {
    let (before_static, before_dynamic) = _work(before);
    let (after_static, after_dynamic) = _work(after);
    if after_static < before_static && after_dynamic <= before_dynamic { after } else { before }
}

/// Duplicate a terminal tail when doing so costs no bytes and removes a jump.
///
/// Frame teardown is emitted around RETURN rather than represented in LIR, so
/// its exact selected size is supplied by the emitter.  Source-owned tails are
/// deliberately excluded: duplicating them would duplicate provenance,
/// fixups, or line anchors rather than merely choosing a machine layout.
pub fn duplicated_returns(body: LirBody, return_overhead: i64) -> LirBody {
    let mut body = body;
    loop {
        let blocks = body.blocks.clone();
        let by_at: IndexMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, block)).collect();
        let predecessors = _predecessors(&blocks);
        let mut changed = false;
        for tail in &blocks {
            // Parent order reaches only sums and a map keyed by `at`.
            let parents: Vec<&LirBlock> = predecessors
                .get(&tail.at)
                .into_iter()
                .flatten()
                .filter_map(|at| by_at.get(at).copied())
                .collect();
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

            let replacement: IndexMap<i64, LirBlock> =
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
fn _arm_bytes(bits: u32, block: &LirBlock) -> Option<i64> {
    let real = _real(block);
    let body = real.split_last().map_or(&real[..], |(last, rest)| if last.what.as_ref().is_some_and(|what| what.op == Operation::Jump) { rest } else { &real[..] });
    body.iter().map(|one| select::priced_in(bits, one.what.as_ref()?, 0, None, false, false, None).map(|made| made.code.len() as i64)).sum()
}

/// Selected bytes in a source-unowned terminal return block.
pub fn _duplicable_return_size(bits: u32, block: &LirBlock, return_overhead: i64) -> Option<i64> {
    let real = _real(block);
    if !block.phis.is_empty()
        || !block.succ.is_empty()
        || real.is_empty()
        || real[real.len() - 1].what.as_ref().is_none_or(|what| what.op != Operation::Return)
        // A return that pops 64 KB or more is several instructions (`masm::moved_return`), not one `ret`.
        || real[real.len() - 1].what.as_ref().is_some_and(|what| matches!(what.sources.first(), Some(crate::model::ir::Loc::Imm(popped)) if popped.value > llrm_x86::calling::RET_POPS_MOST))
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
pub fn _return_parent(bits: u32, block: &LirBlock, target: i64) -> Option<(&LirBlock, i64)> {
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
        let emitted = select::priced_in(bits, &Semantics { target: Some(2), ..what.clone() }, 0, None, true, false, None);
        return emitted.map(|emitted| (block, emitted.code.len() as i64));
    }
    if matches!(what.op, Operation::Branch | Operation::Call | Operation::Data | Operation::Return) {
        return None;
    }
    Some((block, 0))
}

/// Replace parent's dedicated edge with a fresh copy of tail.
pub fn _with_return(parent: &LirBlock, tail: &LirBlock) -> LirBlock {
    let real = _real(parent);
    let jump = real
        .last()
        .filter(|last| last.what.as_ref().is_some_and(|what| what.op == Operation::Jump));
    let anchor = match jump {
        Some(jump) => jump.at,
        None => parent.insns.last().map_or(parent.at, |last| last.at),
    };
    let kept = parent.insns.iter().filter(|one| jump.is_none_or(|jump| !Arc::ptr_eq(one, jump))).cloned();
    let copies = tail
        .insns
        .iter()
        .map(|one| Arc::new(Insn { at: anchor, covers: Some((anchor, anchor)), spread: Vec::new(), ..(**one).clone() }));
    LirBlock { succ: tail.succ.clone(), ..parent.with_insns(kept.chain(copies).collect()) }
}

/// One change to `body`, the first that applies, in place: its blocks are neither copied nor, but for the
/// edited one and those the change strands, rebuilt. Whether it changed anything.
///
/// `at` is each block's position, kept while the blocks stay as they are: a change that only rewrites an
/// instruction strands nothing, so the body is not searched for what it reaches again.
pub fn _step(body: &mut LirBody, at: &mut IndexMap<i64, usize>, protected: &BTreeSet<i64>) -> bool {
    let mut blocks = std::mem::take(&mut body.blocks);
    for index in 0..blocks.len() {
        // The block's last two real instructions, which is all the rules read of the rest: nothing is
        // copied for a block no rule applies to.
        let (last, before) = _real_tail(&blocks[index]);
        let Some(last) = last else {
            continue;
        };
        let last_what = last.what.as_ref().expect(NO_OP);
        if !matches!(last_what.op, Operation::Branch | Operation::Jump) {
            continue;
        }
        let block = blocks[index].clone();
        let last = &last;
        // Control falls through blocks of only meta instructions, as layout
        // places them.
        let after = blocks[index + 1..]
            .iter()
            .find(|next| !next.phis.is_empty() || next.insns.iter().any(|one| !one.is_meta()))
            .map(|next| next.at);
        let target = _through(&blocks, at, last_what.target, protected);
        if target != last_what.target {
            let onward = target.expect("a passage names its target");
            if let Some(old) = last_what.target {
                body.odds.rerouted(block.at, &block.succ, old, &[(onward, 1.0)]);
            }
            blocks[index] = _retargeted(&block, last, onward);
            body.blocks = blocks;
            _stranded(body, at);
            return true;
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
                    if Arc::ptr_eq(one, last) && !one.inserted() { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) }
                })
                .collect();
            blocks[index] = LirBlock { insns: kept, ..block };
            body.blocks = blocks;
            return true;
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
                blocks[index] = LirBlock { insns, ..block };
                body.blocks = blocks;
                return true;
            }
        }
        let opposite = last_what.name.as_deref().and_then(|name| _OPPOSITE.get(name));
        if last_what.op == Operation::Branch && after.is_some() && opposite.is_some() {
            // Falling into a block that only jumps, with the branch taken to the block past it.
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
                blocks[index] = LirBlock { insns, succ, ..block };
                blocks[index + 1] = LirBlock { succ: Vec::new(), ..over };
                body.blocks = blocks;
                _stranded(body, at);
                return true;
            }
        }
    }
    body.blocks = blocks;
    false
}

/// The last two of `_real(block)`, the last first, without making the rest.
fn _real_tail(block: &LirBlock) -> (Option<Arc<Insn>>, Option<Arc<Insn>>) {
    let nop = |one: &Insn| one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"));
    let mut real = block.insns.iter().rev().filter(|one| !one.is_meta() && !nop(one));
    (real.next().cloned(), real.next().cloned())
}

/// The instructions that print.
/// The block's work: neither meta instructions nor source `nop`s.
pub fn _real(block: &LirBlock) -> Vec<Arc<Insn>> {
    let nop = |one: &Insn| one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"));
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
    blocks: &[LirBlock],
    at: &IndexMap<i64, usize>,
    target: Option<i64>,
    protected: &BTreeSet<i64>,
) -> Option<i64> {
    let (start, mut seen) = (target, HashSet::default());
    let mut target = target;
    while let Some(current) = target {
        if protected.contains(&current) || !at.contains_key(&current) {
            break;
        }
        let Some(onward) = _passage(&blocks[at[&current]]) else {
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

pub fn _retargeted(block: &LirBlock, last: &Arc<Insn>, target: i64) -> LirBlock {
    let what = last.what.as_ref().expect(NO_OP);
    let old = what.target;
    let moved = Arc::new(Insn { what: Some(Semantics { target: Some(target), ..what.clone() }), ..(**last).clone() });
    let succ: IndexSet<i64> =
        block.succ.iter().map(|one| if Some(*one) == old { target } else { *one }).collect();
    LirBlock { succ: succ.into_iter().collect(), ..block.with_insns(block
            .insns
            .iter()
            .map(|one| if Arc::ptr_eq(one, last) { Arc::clone(&moved) } else { Arc::clone(one) })
            .collect()) }
}

pub fn _predecessors(blocks: &[LirBlock]) -> IndexMap<i64, BTreeSet<i64>> {
    let mut found: IndexMap<i64, BTreeSet<i64>> = IndexMap::default();
    for block in blocks {
        for one in &block.succ {
            found.entry(*one).or_default().insert(block.at);
        }
    }
    found
}

pub fn _reachable(body: &LirBody, blocks: Vec<LirBlock>) -> LirBody {
    let mut made = body.with_blocks(Vec::new());
    made.blocks = blocks;
    _prune(&mut made);
    made
}

/// `_prune`, and the positions in `at` made again where it removed a block.
fn _stranded(body: &mut LirBody, at: &mut IndexMap<i64, usize>) {
    let before = body.blocks.len();
    _prune(body);
    if body.blocks.len() != before {
        *at = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    }
}

/// `body` with the blocks its entry does not reach stripped to their source bytes, in place: a reached
/// block is moved, not copied.
pub fn _prune(body: &mut LirBody) {
    let blocks = std::mem::take(&mut body.blocks);
    let reached: HashSet<i64> = {
        let by_at: IndexMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, block)).collect();
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
            let kept: Vec<Arc<Insn>> = block.insns.iter().filter(|one| !one.owned().is_empty()).map(lir::bytes_only).collect();
            (!kept.is_empty()).then(|| LirBlock { succ: Vec::new(), phis: Vec::new(), ..block.with_insns(kept) })
        })
        .collect();
}

#[cfg(test)]
#[path = "jumps_tests.rs"]
mod tests;
