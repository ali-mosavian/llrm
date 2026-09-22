//! Port of `qbopt/backend/jumps.py`: block order, and the jumps it makes
//! redundant.
//!
//! Runs on the allocated body, after the last phase that adds or empties blocks:
//! edge splits and their undoing leave jumps to the next block, and the raise's
//! `if false goto` beside a `goto` leaves a `jcc` over a block that only jumps.
//! `placed` orders the blocks; `threaded` keeps that order and drops the jumps
//! and the blocks nothing reaches.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use iced_x86::Register;
use indexmap::{IndexMap, IndexSet};

use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::intervals;
use crate::backend::layout::_OPPOSITE;
use crate::backend::{machinedce, masm, select};
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

/// Python's AttributeError on `None.op`: `_real` keeps semantics-less markers.
const NO_OP: &str = "'NoneType' object has no attribute 'op'";

/// Settle allocated tails and edges after every machine-shaping phase.
pub struct ControlFlow;

impl LIRTransform for ControlFlow {
    fn class_name(&self) -> &'static str {
        "ControlFlow"
    }

    fn name(&self) -> &str {
        "jumps"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        optimized(&body)
    }
}

/// Choose the cheapest common-tail fixed point without adding hot work.
pub fn optimized(body: &LirBody) -> Result<LirBody, String> {
    let mut candidate = placed(body)?;
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
    Ok(preferred(&baseline, &threaded(&candidate)).clone())
}

/// Each block placed after the jump that reaches it, where no block is already.
///
/// The raise lays a C loop out as it reads, test first, so even entered at
/// its body the latch jumped back to the test every pass. Every fall-through
/// is written as a jump first, which makes any order correct; `threaded` then
/// drops the jumps the order made redundant and turns the test into one
/// branch back to the body.
pub fn placed(body: &LirBody) -> Result<LirBody, String> {
    let mut explicit = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        let fall = masm::_falls_to(&block, &body.name).map_err(|error| error.0)?;
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
    let by_at: IndexMap<i64, LirBlock> = explicit.iter().map(|block| (block.at, block.clone())).collect();
    let natural = loopy::loops(&intervals::_graph(&explicit), Some(body.entry));
    let tests = _tests(&natural, body.entry, &by_at);
    // loops() is innermost first.  A block in nested loops follows the nearest
    // loop's trace before an exit from it; the outer trace resumes afterwards.
    let mut inside: IndexMap<i64, BTreeSet<i64>> = IndexMap::new();
    for found in &natural {
        for at in &found.body {
            inside.entry(*at).or_insert_with(|| found.body.clone());
        }
    }
    let mut order: Vec<LirBlock> = Vec::new();
    let mut done: HashSet<i64> = HashSet::new();
    let mut current: Option<i64> = Some(body.entry);
    let mut source: Option<i64> = None;
    let empty = BTreeSet::new();
    while order.len() < explicit.len() {
        if current.is_none_or(|at| done.contains(&at) || !by_at.contains_key(&at)) {
            current = Some(
                explicit
                    .iter()
                    .map(|block| block.at)
                    .find(|at| !done.contains(at))
                    .expect("StopIteration"),
            );
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
        let block = &by_at[&at];
        order.push(block.clone());
        done.insert(at);
        (current, source) = (_onward(block, &done, inside.get(&block.at).unwrap_or(&empty), Some(&by_at)), Some(at));
    }
    Ok(LirBody { blocks: order, ..body.clone() })
}

/// The block to place next: where the final jump goes, or else where the branch before it goes.
///
/// The branch's target second, so that `jcc target; jmp placed` becomes one inverted branch.
pub fn _onward(
    block: &LirBlock,
    done: &HashSet<i64>,
    inside: &BTreeSet<i64>,
    by_at: Option<&IndexMap<i64, LirBlock>>,
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
    // Keep a loop chain together before following an exit.  The final jump is
    // still preferred when both edges stay in the loop, preserving the source
    // fall-through unless doing so would strand the rest of the loop.
    for target in &targets {
        if target.is_none_or(|at| !done.contains(&at)) && target.is_some_and(|at| inside.contains(&at)) {
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
pub(crate) fn _tests(
    natural: &[Loop],
    entry: i64,
    by_at: &IndexMap<i64, LirBlock>,
) -> IndexMap<i64, (i64, BTreeSet<i64>)> {
    let mut found = IndexMap::new();
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
    let (mut body, mut changed) = (_reachable(body, body.blocks.clone()), true);
    while changed {
        (body, changed) = _step(&body);
    }
    body
}

/// Python's `_tail_key` tuple.
pub type TailKey = (
    Vec<(Semantics, BTreeSet<Register>, BTreeSet<Register>, Vec<Register>, Vec<Register>, bool)>,
    Vec<i64>,
);

/// Merge physically identical allocated tails after fallthroughs are explicit.
pub fn merged(body: &LirBody) -> Result<LirBody, String> {
    // Removing a duplicate block is sound only when every incoming edge is an
    // instruction that can be retargeted. `placed` establishes exactly that
    // form. Decline a body presented at an earlier pipeline boundary.
    for block in &body.blocks {
        if masm::_falls_to(block, &body.name).map_err(|error| error.0)?.is_some() {
            return Ok(body.clone());
        }
    }
    let mut body = body.clone();
    loop {
        let mut groups: IndexMap<TailKey, Vec<&LirBlock>> = IndexMap::new();
        for block in &body.blocks {
            if let Some(key) = _tail_key(block) {
                groups.entry(key).or_default().push(block);
            }
        }
        let mut redirect: IndexMap<i64, i64> = IndexMap::new();
        for copies in groups.values() {
            if copies.len() < 2 {
                continue;
            }
            let canonical = copies.iter().find(|block| block.at == body.entry).unwrap_or(&copies[0]);
            redirect.extend(
                copies
                    .iter()
                    .filter(|block| !std::ptr::eq(**block, *canonical))
                    .map(|block| (block.at, canonical.at)),
            );
        }
        if redirect.is_empty() {
            return Ok(body);
        }
        body = _redirected(&body, &redirect);
    }
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
        let mut seen = HashSet::new();
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
        blocks.push(_fold_converged(LirBlock {
            insns,
            succ: successors.into_iter().collect(),
            ..block.clone()
        }));
    }
    let entry = target(body.entry);
    LirBody { entry, blocks, ..body.clone() }
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
pub fn _work(body: &LirBody) -> (i64, i64) {
    let depth = intervals::depths(body);
    let counts: IndexMap<i64, i64> = body.blocks.iter().map(|block| (block.at, _real(block).len() as i64)).collect();
    (counts.values().sum(), counts.iter().map(|(at, count)| count * 10_i64.pow(depth[at])).sum())
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
            let tail_size = _duplicable_return_size(tail, return_overhead);
            if tail.at == body.entry
                || tail_size.is_none()
                || parents.len() < 2
                || parents.iter().any(|parent| parent.succ != [tail.at])
            {
                continue;
            }
            let tail_size = tail_size.expect("checked above");
            let prepared: Vec<Option<(&LirBlock, i64)>> =
                parents.iter().map(|parent| _return_parent(parent, tail.at)).collect();
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

/// Selected bytes in a source-unowned terminal return block.
pub fn _duplicable_return_size(block: &LirBlock, return_overhead: i64) -> Option<i64> {
    let real = _real(block);
    if !block.phis.is_empty()
        || !block.succ.is_empty()
        || real.is_empty()
        || real[real.len() - 1].what.as_ref().is_none_or(|what| what.op != Operation::Return)
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
        .map(|one| select::emit(one.what.as_ref().expect("checked above"), 0, None, false, false, None))
        .collect();
    if emitted.iter().any(Option::is_none) {
        return None;
    }
    Some(return_overhead + emitted.iter().flatten().map(|one| one.code.len() as i64).sum::<i64>())
}

/// A dedicated edge to target and the shortest jump bytes it can save.
pub fn _return_parent(block: &LirBlock, target: i64) -> Option<(&LirBlock, i64)> {
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
        let emitted = select::emit(&Semantics { target: Some(2), ..what.clone() }, 0, None, true, false, None);
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
    LirBlock { insns: kept.chain(copies).collect(), succ: tail.succ.clone(), ..parent.clone() }
}

pub fn _step(body: &LirBody) -> (LirBody, bool) {
    let mut blocks = body.blocks.clone();
    let at: IndexMap<i64, usize> = blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let protected: BTreeSet<i64> = body.loop_trip_counts.iter().map(|(header, _count)| *header).collect();
    for index in 0..blocks.len() {
        let block = blocks[index].clone();
        let real = _real(&block);
        let after = blocks.get(index + 1).map(|next| next.at);
        let Some(last) = real.last() else {
            continue;
        };
        let last_what = last.what.as_ref().expect(NO_OP);
        if !matches!(last_what.op, Operation::Branch | Operation::Jump) {
            continue;
        }
        let target = _through(&blocks, &at, last_what.target, &protected);
        if target != last_what.target {
            blocks[index] = _retargeted(&block, last, target.expect("a passage names its target"));
            return (_reachable(body, blocks), true);
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
            return (_reachable(body, blocks), true);
        }
        if last_what.op == Operation::Jump
            && real.len() > 1
            && real[real.len() - 2].what.as_ref().expect(NO_OP).op == Operation::Branch
        {
            let branch = &real[real.len() - 2];
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
                return (_reachable(body, blocks), true);
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
                return (_reachable(body, blocks), true);
            }
        }
    }
    (body.clone(), false)
}

/// The instructions that print.
pub fn _real(block: &LirBlock) -> Vec<Arc<Insn>> {
    block
        .insns
        .iter()
        .filter(|one| match &one.what {
            None => true,
            Some(what) => what.op != Operation::Nothing || !matches!(what.name.as_deref().unwrap_or(""), "" | "nop"),
        })
        .cloned()
        .collect()
}

/// Where a block with no owned inert work goes.
///
/// An inert anchor emits no machine instruction, but it still owns decoded
/// bytes that fresh layout must account for.  Redirecting the incoming edge
/// makes the block unreachable and can strand that ownership.  Executable
/// jump passages retain the existing threading rule; source-owned jumps are
/// handled when a chosen fall-through removes the instruction itself.
pub fn _passage(block: &LirBlock) -> Option<i64> {
    if !block.phis.is_empty()
        || block.insns.iter().any(|one| {
            one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing)
                && (!one.inserted()
                    || !one.spread.is_empty()
                    // An identity copy may emit no instruction after allocation, but
                    // its anchor still establishes a virtual definition on this CFG
                    // edge. Threading around it would leave the successor's operand
                    // naming a value no surviving instruction defines.
                    || !one.defines.is_empty()
                    || !one.uses.is_empty())
        })
    {
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
    let (start, mut seen) = (target, HashSet::new());
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
    LirBlock {
        insns: block
            .insns
            .iter()
            .map(|one| if Arc::ptr_eq(one, last) { Arc::clone(&moved) } else { Arc::clone(one) })
            .collect(),
        succ: succ.into_iter().collect(),
        ..block.clone()
    }
}

pub fn _predecessors(blocks: &[LirBlock]) -> IndexMap<i64, BTreeSet<i64>> {
    let mut found: IndexMap<i64, BTreeSet<i64>> = IndexMap::new();
    for block in blocks {
        for one in &block.succ {
            found.entry(*one).or_default().insert(block.at);
        }
    }
    found
}

pub fn _reachable(body: &LirBody, blocks: Vec<LirBlock>) -> LirBody {
    let by_at: IndexMap<i64, &LirBlock> = blocks.iter().map(|block| (block.at, block)).collect();
    let (mut reached, mut work) = (HashSet::new(), vec![body.entry]);
    while let Some(one) = work.pop() {
        if reached.contains(&one) || !by_at.contains_key(&one) {
            continue;
        }
        reached.insert(one);
        work.extend(by_at[&one].succ.iter().copied());
    }
    // An optimizer may make a decoded region unreachable while leaving its
    // byte ownership on inert NOTHING anchors.  Those anchors emit no code,
    // but layout still needs them to prove that every source byte was
    // deliberately replaced.  Dropping the block made a fully unrolled BC
    // loop refuse fresh emission with an apparent hole in its source map.
    // Keep only genuinely inert orphan blocks; unreachable machine work still
    // disappears as before.
    let ownership: HashSet<i64> = blocks
        .iter()
        .filter(|block| {
            !reached.contains(&block.at)
                && !block.insns.is_empty()
                && block
                    .insns
                    .iter()
                    .all(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing))
                && block.insns.iter().any(|one| {
                    one.covers.is_some_and(|(start, end)| start < end) || !one.spread.is_empty()
                })
        })
        .map(|block| block.at)
        .collect();
    LirBody {
        blocks: blocks
            .iter()
            .filter(|block| reached.contains(&block.at) || ownership.contains(&block.at))
            .map(|block| {
                if ownership.contains(&block.at) { LirBlock { succ: Vec::new(), ..block.clone() } } else { block.clone() }
            })
            .collect(),
        ..body.clone()
    }
}

#[cfg(test)]
#[path = "jumps_tests.rs"]
mod tests;
