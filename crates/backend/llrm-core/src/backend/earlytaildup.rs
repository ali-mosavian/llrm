//! Which blocks early tail duplication may copy, and into which predecessors:
//! `shouldTailDuplicate`, `isSimpleBB`, `canCompletelyDuplicateBB` and
//! `canTailDuplicate` of
//! [TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp)
//! with `PreRegAlloc` set and `LayoutMode` clear.
//!
//! # How it works
//! A block is a candidate when it cannot fall through, is no single-block loop,
//! holds at most two instructions counting its jump (one at `-Os`; twenty if it
//! ends in an indirect jump), and none of them is a call, a return or not
//! duplicable. A block with more than 16 predecessors and more than 16
//! successors is out when it or a successor has a phi. Past those, an indirect
//! jump or a block of one jump is taken as it is; any other block only if every
//! predecessor goes to it alone, by a jump with no condition
//! (`canCompletelyDuplicateBB`).
//!
//! ```text
//!   b1: ... jmp b3        b1: ... <b3's instructions> jmp b4      (b3 has two
//!   b2: ... jmp b3   ==>  b2: ... <b3's instructions> jmp b4       instructions
//!   b3: x; jmp b4         b3 is gone when nothing else reaches it)  or fewer)
//! ```
//!
//! `tail_duplicate` is `tailDuplicate` for a tail with no phi whose values
//! nothing outside it reads: each predecessor that goes to it alone loses its
//! jump to it and takes a copy of its instructions (values the tail defines are
//! new in each copy), the successors' phis read from the predecessor what they
//! read from the tail, the tail's edge odds move to the predecessor, and the
//! tail goes when nothing reaches it.
//!
//! DEVIATE ET1 (owner w-1, exit: step 3): a tail with a phi, or whose values
//! are read outside it, is left (`processPHI`, `updateSuccessorsPHIs`, the SSA
//! update). Nothing calls `tail_duplicate` yet.
//!
//! DEVIATE ET3 (no number): LIR blocks end in explicit jumps, so the pin's
//! predecessor that falls through into the tail is the one that jumps to it
//! from the block just before it; it is left for last and takes the copy only
//! when it is the tail's one predecessor left (the pin's merge into the
//! previous block), which has the same result.
//!
//! DEVIATE ET2 (no number): a block's instructions are not duplicable here if
//! they carry a group, a barrier or data (the placement's version adds a call's
//! record and a symbol, which only exist after allocation); the pin's
//! `isNotDuplicable` and `isConvergent` have no counterpart. LIR has no
//! `INLINEASM_BR`, EH pads or subregister phi operands, so those tests are not
//! made.
//!
//! # Example
//! The unit tests: a join of two arms with one `mov` is a candidate;
//! the same block with a call, or with two, is not.
//!
//! # Used by
//! `flow::machine_with`, first of the machine phases.

use std::sync::Arc;

use crate::model::ir::Operation;
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, HashSet};

mod update;

/// `TailDuplicateSize`.
const SIZE: usize = 2;
/// `TailDupIndirectBranchSize`.
const INDIRECT_SIZE: usize = 20;
/// `TailDupPredSize` and `TailDupSuccSize`.
const EDGES: usize = 16;

/// An instruction that is neither meta nor a no-op.
fn is_real(one: &crate::model::lir::Insn) -> bool {
    !one.is_meta()
        && !one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref() == Some("nop"))
}

/// The block's instructions that are neither meta nor a no-op.
fn real(block: &Block) -> impl DoubleEndedIterator<Item = &crate::model::lir::Insn> {
    block.insns.iter().map(|one| &**one).filter(|one| is_real(one))
}

fn operation(one: &crate::model::lir::Insn) -> Option<Operation> {
    one.what.as_ref().map(|what| what.op)
}

fn indirect(one: &crate::model::lir::Insn) -> bool {
    one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch) && what.indirect)
}

/// `canFallThrough`: the block's last instruction is not a jump with no
/// condition or a return.
fn can_fall_through(block: &Block) -> bool {
    match real(block).next_back() {
        Some(last) => !matches!(operation(last), Some(Operation::Return | Operation::Jump)),
        None => true,
    }
}

/// Distinct successors of `block`.
fn successors(block: &Block) -> Vec<i64> {
    crate::model::lir::distinct(&block.succ)
}

/// Where each block's edges come from, as places in `body.blocks`: made once
/// for a round of `duplicated`, where each candidate would otherwise scan the
/// body.
struct Preds(HashMap<i64, Vec<usize>>);

impl Preds {
    fn of(body: &LirBody) -> Self {
        let mut from: HashMap<i64, Vec<usize>> = HashMap::default();
        for (place, block) in body.blocks.iter().enumerate() {
            for to in crate::model::lir::distinct(&block.succ) {
                from.entry(to).or_default().push(place);
            }
        }
        Self(from)
    }

    /// The places of the blocks that go to `at`.
    fn places(
        &self,
        at: i64,
    ) -> &[usize] {
        self.0.get(&at).map_or(&[], Vec::as_slice)
    }
}

/// `isSimpleBB`: one successor, a predecessor, and nothing but a jump.
pub fn is_simple(
    body: &LirBody,
    tail: &Block,
) -> bool {
    is_simple_in(&Preds::of(body), tail)
}

fn is_simple_in(
    preds: &Preds,
    tail: &Block,
) -> bool {
    if successors(tail).len() != 1 || preds.places(tail.at).is_empty() {
        return false;
    }
    // The first instruction the pin sees is a phi.
    if !tail.phis.is_empty() {
        return false;
    }
    match real(tail).next() {
        None => true,
        Some(first) => operation(first) == Some(Operation::Jump),
    }
}

/// `canTailDuplicate`: `pred` has `tail` as its only successor and reaches it
/// by a jump with no condition.
pub fn can_tail_duplicate(pred: &Block) -> bool {
    // A branch with a condition is a condition: analyzeBranch says so, and a
    // copy that removed the jump would leave the branch to a block that is
    // gone.
    successors(pred).len() <= 1
        && !real(pred).next_back().is_some_and(|one| indirect(one))
        && !real(pred).any(|one| operation(one) == Some(Operation::Branch))
}

/// `canCompletelyDuplicateBB`: every predecessor of `tail` can take a copy.
pub fn can_completely_duplicate(
    body: &LirBody,
    tail: &Block,
) -> bool {
    can_completely_duplicate_in(&Preds::of(body), body, tail)
}

fn can_completely_duplicate_in(
    preds: &Preds,
    body: &LirBody,
    tail: &Block,
) -> bool {
    preds.places(tail.at).iter().all(|pred| can_tail_duplicate(&body.blocks[*pred]))
}

/// The cheap half of `shouldTailDuplicate`, which needs no predecessor: the
/// block cannot fall through, is no loop of its own, holds few enough
/// instructions and none that is not copied before allocation.
fn candidate(
    tail: &Block,
    size: bool,
) -> bool {
    if can_fall_through(tail) || tail.succ.contains(&tail.at) {
        return false;
    }
    let most = if indirect_end(tail) {
        INDIRECT_SIZE
    } else if size {
        1
    } else {
        SIZE
    };
    let mut count = 0;
    for one in real(tail) {
        let op = operation(one);
        if one.group.is_some()
            || matches!(
                op,
                Some(Operation::Barrier | Operation::Data | Operation::Return | Operation::Call)
            )
        {
            return false;
        }
        count += 1;
        if count > most {
            return false;
        }
    }
    true
}

/// `shouldTailDuplicate`, before register allocation; `size` is `-Os`.
pub fn should_tail_duplicate(
    body: &LirBody,
    tail: &Block,
    simple: bool,
    size: bool,
) -> bool {
    should_tail_duplicate_in(&Preds::of(body), body, tail, simple, size)
}

fn should_tail_duplicate_in(
    preds: &Preds,
    body: &LirBody,
    tail: &Block,
    simple: bool,
    size: bool,
) -> bool {
    if !candidate(tail, size) {
        return false;
    }
    let indirect_end = indirect_end(tail);
    let after = successors(tail);
    if preds.places(tail.at).len() > EDGES && after.len() > EDGES {
        let phis = !tail.phis.is_empty()
            || after.iter().any(|to| body.blocks.iter().any(|block| block.at == *to && !block.phis.is_empty()));
        if phis {
            return false;
        }
    }
    if indirect_end || simple {
        return true;
    }
    can_completely_duplicate_in(preds, body, tail)
}

/// `duplicateSimpleBB`: a block of one jump is no code to copy: every
/// predecessor that can go to its successor directly does, whatever it ends in.
/// A predecessor that already goes to the successor, which has phis, is left
/// (`bothUsedInPHI`).
fn duplicate_simple(
    body: &LirBody,
    at: usize,
) -> Option<LirBody> {
    let tail = &body.blocks[at];
    let [to] = successors(tail)[..] else { return None };
    let target = body.blocks.iter().find(|block| block.at == to)?;
    let preds: Vec<usize> = (0..body.blocks.len())
        .filter(|one| *one != at && body.blocks[*one].succ.contains(&tail.at) && !indirect_end(&body.blocks[*one]))
        .filter(|one| target.phis.is_empty() || !body.blocks[*one].succ.contains(&to))
        .collect();
    if preds.is_empty() {
        return None;
    }
    let remains = body
        .blocks
        .iter()
        .enumerate()
        .any(|(one, block)| one != at && block.succ.contains(&tail.at) && !preds.contains(&one))
        || tail.at == body.entry;
    let mut blocks: Vec<Block> = Vec::new();
    for (place, old) in body.blocks.iter().enumerate() {
        if place == at && !remains {
            continue;
        }
        if preds.contains(&place) {
            let mut new = LirBlock::clone(old);
            new.insns = old
                .insns
                .iter()
                .map(|one| match one.what.as_ref() {
                    Some(what)
                        if matches!(what.op, Operation::Jump | Operation::Branch) && what.target == Some(tail.at) =>
                    {
                        let mut made = (**one).clone();
                        made.what = Some(crate::model::ir::Semantics { target: Some(to), ..what.clone() });
                        Arc::new(made)
                    }
                    _ => Arc::clone(one),
                })
                .collect::<Vec<_>>()
                .into();
            let mut succ: Vec<i64> = Vec::new();
            for next in old.succ.iter().map(|next| if *next == tail.at { to } else { *next }) {
                if !succ.contains(&next) {
                    succ.push(next);
                }
            }
            new.succ = succ.into();
            blocks.push(new.into());
        } else if old.at == to {
            let mut new = LirBlock::clone(old);
            new.phis = old
                .phis
                .iter()
                .map(|phi| {
                    let mut phi = phi.clone();
                    let from_tail: Vec<u32> =
                        phi.incoming.iter().filter(|(pred, _)| *pred == tail.at).map(|(_, value)| *value).collect();
                    for &pred in &preds {
                        for value in &from_tail {
                            phi.incoming.push((body.blocks[pred].at, *value));
                        }
                    }
                    if !remains {
                        phi.incoming.retain(|(pred, _)| *pred != tail.at);
                    }
                    phi
                })
                .collect::<Vec<_>>()
                .into();
            blocks.push(new.into());
        } else {
            blocks.push(old.clone());
        }
    }
    let mut made = body.with_blocks(blocks);
    for &pred in &preds {
        let from = body.blocks[pred].at;
        if let Some(odds) = made.odds.taken.shift_remove(&(from, tail.at)) {
            made.odds.taken.insert((from, to), odds);
        }
    }
    if !remains {
        made.odds.taken.retain(|(from, _), _| *from != tail.at);
    } else {
        made.frequencies = forgotten(&made, tail.at);
    }
    Some(made)
}

/// `body`'s table without the tail's entry. A tail that stays now has fewer
/// predecessors and runs as often as they send: a block the table does not hold
/// is worked out from them (as `tailmerge` does for a tail it keeps).
fn forgotten(
    body: &LirBody,
    tail: i64,
) -> Option<Arc<crate::model::lir::BlockFrequencies>> {
    let kept = body.frequencies.as_ref()?;
    let mut table = kept.0.clone();
    table.shift_remove(&tail);
    Some(Arc::new(crate::model::lir::BlockFrequencies(table)))
}

/// The block ends in an indirect jump.
fn indirect_end(block: &Block) -> bool {
    real(block).next_back().is_some_and(|last| indirect(last))
}

/// `body` with the block at `tail` copied into its predecessors, or None where
/// the pin would not. `size` is `-Os`.
pub fn tail_duplicate(
    body: &LirBody,
    tail: i64,
    size: bool,
) -> Option<LirBody> {
    tail_duplicate_in(&Preds::of(body), body, tail, size)
}

fn tail_duplicate_in(
    graph: &Preds,
    body: &LirBody,
    tail: i64,
    size: bool,
) -> Option<LirBody> {
    let at = body.blocks.iter().position(|block| block.at == tail)?;
    let block = &body.blocks[at];
    if !candidate(block, size) {
        return None;
    }
    let simple = is_simple_in(graph, block);
    if !should_tail_duplicate_in(graph, body, block, simple, size) {
        return None;
    }
    if simple {
        return duplicate_simple(body, at);
    }
    let preds: Vec<usize> = graph.places(tail).to_vec();
    // The block just before the tail in the layout that jumps to it is the one
    // the pin sees fall through.
    let falls = |pred: usize| {
        pred + 1 == at
            && real(&body.blocks[pred]).next_back().is_none_or(|last| {
                operation(last) == Some(Operation::Jump)
                    && last.what.as_ref().is_some_and(|what| what.target == Some(tail))
                    || !matches!(
                        operation(last),
                        Some(Operation::Jump | Operation::Branch | Operation::Return)
                    )
            })
    };
    let mut chosen: Vec<usize> = preds
        .iter()
        .copied()
        .filter(|pred| *pred != at && can_tail_duplicate(&body.blocks[*pred]) && !falls(*pred))
        .collect();
    let left: Vec<usize> = preds.iter().copied().filter(|pred| !chosen.contains(pred)).collect();
    if let [only] = left[..]
        && falls(only)
        && can_tail_duplicate(&body.blocks[only])
    {
        chosen.push(only);
    }
    if chosen.is_empty() {
        return None;
    }
    let remains = preds.iter().any(|pred| !chosen.contains(pred)) || tail == body.entry;
    Some(duplicate(body, at, &chosen, remains))
}

/// `tailDuplicate` and `tailDuplicateAndUpdate`: the tail's instructions into
/// each block of `chosen`, then the phis of its successors and the SSA form.
fn duplicate(
    body: &LirBody,
    at: usize,
    chosen: &[usize],
    remains: bool,
) -> LirBody {
    let block = &body.blocks[at];
    let tail = block.at;
    let widths = super::phielim::_widths(body);
    // Everything the tail defines, and what of it another block reads.
    let defined: HashSet<u32> = block
        .phis
        .iter()
        .map(|phi| phi.result)
        .chain(block.insns.iter().flat_map(|one| one.defines.iter().copied()))
        .collect();
    let mut outside: Vec<u32> = body
        .blocks
        .iter()
        .flat_map(|other| {
            let insns = other
                .insns
                .iter()
                .filter(move |_| other.at != tail)
                .flat_map(|one| one.uses.iter().chain(&one.undefs).copied().collect::<Vec<_>>());
            let phis = other.phis.iter().flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value));
            insns.chain(phis).collect::<Vec<_>>()
        })
        .filter(|value| defined.contains(value))
        .collect();
    outside.sort_unstable();
    outside.dedup();
    let mut next = body.next_value();
    // What the tail's values are in each copy, and the block that holds it.
    let mut avail: HashMap<u32, HashMap<i64, u32>> = outside.iter().map(|value| (*value, HashMap::default())).collect();
    let mut made: HashMap<usize, Vec<Arc<Insn>>> = HashMap::default();
    for &pred in chosen {
        let host = &body.blocks[pred];
        let mut map: HashMap<u32, u32> = HashMap::default();
        for phi in block.phis.iter() {
            if let Some((_, value)) = phi.incoming.iter().find(|(from, _)| *from == host.at) {
                map.insert(phi.result, *value);
            }
        }
        for one in block.insns.iter() {
            for value in &one.defines {
                map.entry(*value).or_insert_with(|| {
                    next += 1;
                    next - 1
                });
            }
        }
        let mut insns: Vec<Arc<Insn>> = host.insns.to_vec();
        // The jump to the tail goes; a block that falls into it has none.
        let beside = match insns.iter().rposition(|one| {
            operation(one) == Some(Operation::Jump) && one.what.as_ref().is_some_and(|what| what.target == Some(tail))
        }) {
            Some(jump) => insns.remove(jump),
            None => Arc::clone(insns.last().or(block.insns.first()).expect("a block with an instruction")),
        };
        let copies: Vec<Arc<Insn>> = block
            .insns
            .iter()
            .map(|one| {
                let copy = one.renamed(&|value| map.get(&value).copied().unwrap_or(value));
                super::jumps::_unowned(&copy, &beside)
            })
            .collect();
        // The copies of what a phi gave, for the successors that read it, go
        // before the terminators the tail ends in (`appendCopies`).
        let ends = copies
            .iter()
            .rposition(|one| !matches!(operation(one), Some(Operation::Jump | Operation::Branch)) && !one.is_meta())
            .map_or(0, |last| last + 1);
        let mut kept = copies;
        let terminators = kept.split_off(ends);
        insns.extend(kept);
        for phi in block.phis.iter().filter(|phi| outside.contains(&phi.result)) {
            let Some(&from) = map.get(&phi.result) else { continue };
            let width = widths.get(&phi.result).copied().unwrap_or(4);
            let value = next;
            next += 1;
            let what = super::phielim::move_of(
                crate::model::ir::Held { value, width },
                crate::model::ir::Held { value: from, width },
            );
            let mut copy = Insn::new(beside.at, Some((beside.at, beside.at)), Some(what), vec![value], vec![from]);
            copy.call = beside.call.clone();
            insns.push(Arc::new(copy));
            avail.get_mut(&phi.result).expect("listed").insert(host.at, value);
        }
        insns.extend(terminators);
        for value in &outside {
            if !avail[value].contains_key(&host.at) {
                avail.get_mut(value).expect("listed").insert(host.at, map[value]);
            }
        }
        made.insert(pred, insns);
    }
    let mut blocks: Vec<LirBlock> = Vec::new();
    for (place, old) in body.blocks.iter().enumerate() {
        if place == at && !remains {
            continue;
        }
        let mut new = LirBlock::clone(old);
        if let Some(insns) = made.remove(&place) {
            new.insns = insns.into();
            new.succ = block.succ.clone();
        }
        // A tail that stays no longer follows the blocks it was copied into.
        if place == at {
            let gone: Vec<i64> = chosen.iter().map(|pred| body.blocks[*pred].at).collect();
            for phi in new.phis.iter_mut() {
                phi.incoming.retain(|(from, _)| !gone.contains(from));
            }
        }
        // A successor of the tail reads from each copy what it read from the
        // tail (`updateSuccessorsPHIs`).
        if block.succ.contains(&old.at) {
            new.phis = old
                .phis
                .iter()
                .map(|phi| {
                    let mut phi = phi.clone();
                    let from_tail: Vec<u32> =
                        phi.incoming.iter().filter(|(from, _)| *from == tail).map(|(_, value)| *value).collect();
                    for &pred in chosen {
                        let from = body.blocks[pred].at;
                        for value in &from_tail {
                            let there =
                                avail.get(value).and_then(|places| places.get(&from)).copied().unwrap_or(*value);
                            phi.incoming.push((from, there));
                        }
                    }
                    if !remains {
                        phi.incoming.retain(|(from, _)| *from != tail);
                    }
                    phi
                })
                .collect::<Vec<_>>()
                .into();
        }
        blocks.push(new);
    }
    // The SSA form: a value the tail defined and others read has a definition
    // in each copy.
    for value in &outside {
        let mut places = avail[value].clone();
        if remains {
            places.insert(tail, *value);
        }
        let keep: Vec<i64> = if remains { vec![tail] } else { Vec::new() };
        update::Updater::rewrite(&mut blocks, *value, places, &keep, &mut next);
    }
    let mut made_body = body.with_blocks(blocks.into_iter().map(Block::from).collect());
    for &pred in chosen {
        let from = body.blocks[pred].at;
        for to in block.succ.iter() {
            if let Some(odds) = body.odds.taken.get(&(tail, *to)).copied() {
                made_body.odds.taken.insert((from, *to), odds);
            }
        }
        made_body.odds.taken.shift_remove(&(from, tail));
    }
    if !remains {
        made_body.odds.taken.retain(|(from, _), _| *from != tail);
    } else {
        made_body.frequencies = forgotten(&made_body, tail);
    }
    made_body
}

/// `TailDuplicator::tailDuplicateBlocks` until it changes nothing: every block
/// but the entry, in order, tried as a tail.
pub fn duplicated(
    mut body: LirBody,
    size: bool,
) -> LirBody {
    loop {
        let mut changed = false;
        let mut place = 1;
        let mut graph = Preds::of(&body);
        while place < body.blocks.len() {
            let at = body.blocks[place].at;
            // Most blocks fail the cheap tests: no scan of the body for them.
            if candidate(&body.blocks[place], size)
                && let Some(made) = tail_duplicate_in(&graph, &body, at, size)
            {
                body = made;
                graph = Preds::of(&body);
                changed = true;
                // The tail may be gone: what stands at `place` is the next
                // block.
                if body.blocks.get(place).is_none_or(|block| block.at != at) {
                    continue;
                }
            }
            place += 1;
        }
        if !changed {
            return body;
        }
    }
}

/// The phase: early tail duplication, directly after selection.
pub struct EarlyTailDuplicate {
    pub size: bool,
}

impl crate::model::passes::LIRTransform for EarlyTailDuplicate {
    fn class_name(&self) -> &'static str {
        "EarlyTailDuplicate"
    }

    fn name(&self) -> &str {
        "earlytaildup"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(duplicated(body, self.size))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Imm, Loc, Semantics};
    use crate::model::lir::{Insn, LirBlock};
    use crate::support::hash::IndexMap;

    fn work(value: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(crate::model::ir::Held { value, width: 4 })],
            sources: vec![Loc::Imm(Imm { value: 1, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![value], vec![]))
    }

    fn of(
        name: &str,
        op: Operation,
        target: Option<i64>,
        indirect: bool,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), target, indirect, ..Semantics::new(op) };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]))
    }

    fn jump(to: i64) -> Arc<Insn> {
        of("jmp", Operation::Jump, Some(to), false)
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: &[i64],
    ) -> LirBlock {
        LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new("t", 0, blocks.into_iter().map(Block::from).collect(), IndexMap::default(), IndexMap::default())
    }

    /// Two arms jump to a join of the given instructions, which jumps on to 9.
    fn join(insns: Vec<Arc<Insn>>) -> LirBody {
        let mut tail = insns;
        tail.push(jump(9));
        body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, tail, &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ])
    }

    fn at(
        body: &LirBody,
        block: i64,
    ) -> &Block {
        body.blocks.iter().find(|one| one.at == block).unwrap()
    }

    fn should(
        body: &LirBody,
        block: i64,
        size: bool,
    ) -> bool {
        let tail = at(body, block);
        should_tail_duplicate(body, tail, is_simple(body, tail), size)
    }

    /// A join of two arms is a candidate with one instruction and its jump
    /// (two, as the pin counts); one more is not.
    #[test]
    fn test_a_join_of_an_instruction_and_a_jump_is_copied_and_one_more_is_not() {
        assert!(should(&join(vec![work(3)]), 3, false));
        assert!(!should(&join(vec![work(3), work(4)]), 3, false));
    }

    /// At `-Os` the jump alone is the most: the copy saves one jump against it.
    #[test]
    fn test_at_os_a_jump_alone_is_the_most() {
        assert!(should(&join(vec![]), 3, true));
        assert!(!should(&join(vec![work(3)]), 3, true));
    }

    /// A call is never copied before allocation: it is a barrier to it, and a
    /// copy of one adds spills. (A return too: it expands after.)
    #[test]
    fn test_a_call_is_not_copied_before_allocation() {
        let call = of("call", Operation::Call, Some(7), false);
        assert!(!should(&join(vec![call]), 3, false));
        assert!(should(&join(vec![work(3)]), 3, false), "the same block of a move is");
    }

    /// A block of one jump (`isSimpleBB`) is copied whatever its predecessors
    /// do; any other waits for `canCompletelyDuplicateBB`.
    #[test]
    fn test_a_predecessor_with_a_condition_stops_a_block_that_is_not_simple() {
        let shape = |pred_exits: &[i64]| {
            body(vec![
                block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
                block(1, vec![work(1), jump(3)], pred_exits),
                block(2, vec![work(2), jump(3)], &[3]),
                block(3, vec![work(3), jump(9)], &[9]),
                block(9, vec![of("ret", Operation::Return, None, false)], &[]),
            ])
        };
        assert!(should(&shape(&[3]), 3, false));
        assert!(!should(&shape(&[3, 9]), 3, false), "predecessor 1 has two successors");
        let simple = body(vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(3)], &[3, 2]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ]);
        assert!(should(&simple, 3, false), "a block of one jump goes into a predecessor that branches");
    }

    /// A block that falls out of its end, or loops on itself, is left.
    #[test]
    fn test_a_self_loop_and_a_fall_through_are_not_copied() {
        let looped = body(vec![
            block(0, vec![jump(3)], &[3]),
            block(3, vec![work(3), of("jne", Operation::Branch, Some(3), false), jump(9)], &[3, 9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ]);
        assert!(!should(&looped, 3, false));
        let falls = body(vec![block(0, vec![jump(3)], &[3]), block(3, vec![work(3)], &[9]), block(9, vec![], &[])]);
        assert!(!should(&falls, 3, false));
    }

    /// An indirect jump is copied up to twenty instructions, which undoes the
    /// merging of dispatch blocks (an interpreter's switch).
    #[test]
    fn test_a_block_ending_in_an_indirect_jump_may_hold_twenty_instructions() {
        let dispatch = |count: usize| {
            let mut insns: Vec<Arc<Insn>> = (0..count as u32).map(work).collect();
            insns.push(of("jmp", Operation::Jump, None, true));
            body(vec![
                block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
                block(1, vec![work(100), jump(3)], &[3]),
                block(2, vec![work(101), jump(3)], &[3]),
                block(3, insns, &[1, 2]),
            ])
        };
        assert!(should(&dispatch(19), 3, false));
        assert!(!should(&dispatch(21), 3, false));
    }

    fn insn_names(block: &Block) -> Vec<String> {
        block.insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    /// The join of two arms goes into both and is gone: each arm ends with the
    /// join's instruction and its jump, the join's value is new in each copy.
    #[test]
    fn test_a_join_is_copied_into_each_arm_and_removed() {
        let shape = join(vec![work(3)]);
        let done = tail_duplicate(&shape, 3, false).expect("duplicated");
        assert!(done.blocks.iter().all(|block| block.at != 3), "the join is gone");
        for arm in [1, 2] {
            let block = at(&done, arm);
            assert_eq!(insn_names(block), ["mov", "mov", "jmp"], "arm {arm}");
            assert_eq!(block.succ.iter().copied().collect::<Vec<_>>(), [9]);
        }
        let values: Vec<u32> =
            [1, 2].iter().flat_map(|arm| at(&done, *arm).insns.iter().flat_map(|one| one.defines.clone())).collect();
        assert_eq!(values.len(), 4);
        assert!(values[1] != 3 && values[3] != 3 && values[1] != values[3], "new values per copy: {values:?}");
    }

    /// A block of one jump (`duplicateSimpleBB`) is no copy: each predecessor
    /// goes to its successor, a conditional one by its branch.
    #[test]
    fn test_a_block_of_one_jump_is_bypassed_by_every_predecessor() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(3), false), jump(2)], &[3, 2]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        let mut shape = body(blocks);
        shape.odds.taken.insert((0, 3), 777);
        let done = tail_duplicate(&shape, 3, false).expect("bypassed");
        assert!(done.blocks.iter().all(|block| block.at != 3), "the jump block is gone");
        assert_eq!(at(&done, 0).succ.iter().copied().collect::<Vec<_>>(), [9, 2]);
        assert_eq!(at(&done, 2).succ.iter().copied().collect::<Vec<_>>(), [9]);
        let branch = at(&done, 0).insns.iter().find_map(|one| one.what.as_ref().filter(|w| w.op == Operation::Branch));
        assert_eq!(branch.and_then(|w| w.target), Some(9));
        assert_eq!(done.odds.taken.get(&(0, 9)), Some(&777));
    }

    /// A predecessor that cannot take a copy (two successors) stops a block
    /// that is not one jump: `canCompletelyDuplicateBB`.
    #[test]
    fn test_one_predecessor_that_cannot_take_a_copy_keeps_the_join_whole() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(1, vec![work(1), of("jne", Operation::Branch, Some(3), false), jump(9)], &[3, 9]),
            block(3, vec![work(3), jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        assert!(tail_duplicate(&body(blocks), 3, false).is_none());
    }

    /// The join stayed for the one predecessor (an indirect jump) that could
    /// not take a copy but kept its table entry, the runs of every
    /// predecessor: the verifier refused it ("block runs 0.75 times, its
    /// predecessors 0.25"), and Nib's `@N$BRES` did not compile. The table
    /// forgets it, as `tailmerge` does.
    #[test]
    fn test_a_join_that_stays_is_forgotten_by_the_frequency_table() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(7)], &[7, 2]),
            block(7, vec![of("jne", Operation::Branch, Some(1), false), jump(8)], &[8, 1]),
            block(1, vec![work(1), of("jmp", Operation::Jump, None, true)], &[3, 9]),
            block(8, vec![work(8), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        let mut before = body(blocks);
        before.frequencies = Some(crate::analysis::frequency::Frequency::over(&before, &before.blocks).table());
        let after = tail_duplicate(&before, 3, false).expect("the arm that can take it bypasses the join");
        assert!(at(&after, 3).succ.contains(&9) && at(&after, 1).succ.contains(&3), "premise: the join stays");
        assert_eq!(crate::analysis::frequency::Frequency::violations(&after), Vec::<String>::new());
    }

    /// The successor's phi reads from each arm what it read from the join.
    #[test]
    fn test_a_successors_phi_reads_from_each_copy_what_it_read_from_the_join() {
        use crate::model::lir::Phi;
        let phi = Phi { result: 50, incoming: vec![(3, 7)] };
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            LirBlock { phis: vec![phi].into(), ..block(9, vec![of("ret", Operation::Return, None, false)], &[]) },
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("duplicated");
        let mut from: Vec<(i64, u32)> = at(&done, 9).phis[0].incoming.clone();
        from.sort();
        assert_eq!(from, [(1, 7), (2, 7)], "the join is no predecessor any more");
    }

    fn reading(value: u32) -> Arc<Insn> {
        let what = Semantics {
            name: Some("add".into()),
            dests: vec![Loc::Held(crate::model::ir::Held { value: 90, width: 4 })],
            sources: vec![Loc::Held(crate::model::ir::Held { value, width: 4 })],
            ..Semantics::new(Operation::Binary)
        };
        Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![90], vec![value]))
    }

    /// A value the join defines and a successor reads is defined in each copy
    /// and the read takes a phi of them: the join's one value had two
    /// definitions.
    #[test]
    fn test_a_value_the_join_defines_gets_a_definition_in_each_copy_and_a_phi_where_they_meet() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            block(9, vec![reading(3), of("ret", Operation::Return, None, false)], &[]),
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("duplicated");
        let exit = at(&done, 9);
        assert_eq!(exit.phis.len(), 1, "one phi over the two copies: {exit:#?}");
        let phi = &exit.phis[0];
        let from: Vec<i64> = phi.incoming.iter().map(|(from, _)| *from).collect();
        assert_eq!(from, [1, 2]);
        assert!(phi.incoming.iter().all(|(_, value)| *value != 3), "neither is the join's own value");
        assert_eq!(exit.insns[0].uses, [phi.result], "the read takes the phi");
        let problems: Vec<String> =
            crate::backend::verify::verify(&done, true).into_iter().filter(|line| line.contains("alue")).collect();
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// A phi of the join becomes, in each copy, the value its predecessor gave
    /// it; a successor that reads it takes a phi of those, by a copy made
    /// in each.
    #[test]
    fn test_a_phi_of_the_join_is_what_each_predecessor_gave_it() {
        use crate::model::lir::Phi;
        let phi = Phi { result: 40, incoming: vec![(1, 11), (2, 12)] };
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(11), jump(3)], &[3]),
            block(2, vec![work(12), jump(3)], &[3]),
            LirBlock { phis: vec![phi].into(), ..block(3, vec![reading(40), jump(9)], &[9]) },
            block(9, vec![reading(40), of("ret", Operation::Return, None, false)], &[]),
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("duplicated");
        for (arm, given) in [(1, 11), (2, 12)] {
            let host = at(&done, arm);
            let used: Vec<u32> = host.insns.iter().flat_map(|one| one.uses.clone()).collect();
            assert!(used.contains(&given), "arm {arm}'s copy reads what it gave the phi: {used:?}");
            assert!(!used.contains(&40), "the phi's own value is not read in a copy");
        }
        let exit = at(&done, 9);
        assert_eq!(exit.phis.len(), 1, "{exit:#?}");
        assert_eq!(exit.insns[0].uses, [exit.phis[0].result]);
        let problems: Vec<String> =
            crate::backend::verify::verify(&done, true).into_iter().filter(|line| line.contains("alue")).collect();
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// A tail that stays (a predecessor that cannot take a copy) keeps its own
    /// definition: a read after both takes a phi of it and the copy.
    #[test]
    fn test_a_tail_that_stays_is_one_of_the_definitions_a_phi_joins() {
        let mut end = vec![work(3)];
        end.push(of("jmp", Operation::Jump, None, true));
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), of("jne", Operation::Branch, Some(3), false), jump(4)], &[3, 4]),
            block(3, end, &[5]),
            block(4, vec![of("ret", Operation::Return, None, false)], &[]),
            block(5, vec![reading(3), of("ret", Operation::Return, None, false)], &[]),
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("arm 1 takes a copy");
        assert!(done.blocks.iter().any(|block| block.at == 3), "arm 2 still reaches the tail");
        let exit = at(&done, 5);
        assert_eq!(exit.phis.len(), 1, "{exit:#?}");
        let mut from: Vec<(i64, bool)> =
            exit.phis[0].incoming.iter().map(|(from, value)| (*from, *value == 3)).collect();
        from.sort();
        assert_eq!(from, [(1, false), (3, true)], "the copy's value from arm 1, the tail's own from the tail");
    }

    /// A tail that stays kept phi operands from the arms it was copied into,
    /// which no longer branch to it: PHI elimination copied them anyway, and
    /// x_funcptr kept a home per case for the sum (66 instructions, the pin
    /// 49). The tail's phis read only the arms that still reach it.
    #[test]
    fn test_a_tail_that_stays_reads_no_phi_operand_from_an_arm_it_was_copied_into() {
        use crate::model::lir::Phi;
        let phi = Phi { result: 40, incoming: vec![(1, 11), (2, 12)] };
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(11), jump(3)], &[3]),
            block(2, vec![work(12), of("jne", Operation::Branch, Some(3), false), jump(4)], &[3, 4]),
            LirBlock {
                phis: vec![phi].into(),
                ..block(3, vec![reading(40), of("jmp", Operation::Jump, None, true)], &[5])
            },
            block(4, vec![of("ret", Operation::Return, None, false)], &[]),
            block(5, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("arm 1 takes a copy");
        assert!(!at(&done, 1).succ.contains(&3) && at(&done, 2).succ.contains(&3), "premise: arm 2 alone reaches it");
        assert_eq!(at(&done, 3).phis[0].incoming, [(2, 12)]);
    }

    /// The join's edge odds are the copy's.
    #[test]
    fn test_the_joins_edge_odds_move_to_each_copy() {
        let mut shape = join(vec![work(3)]);
        shape.odds.taken.insert((3, 9), 1234);
        let done = tail_duplicate(&shape, 3, false).expect("duplicated");
        assert_eq!(done.odds.taken.get(&(1, 9)), Some(&1234));
        assert_eq!(done.odds.taken.get(&(2, 9)), Some(&1234));
        assert!(!done.odds.taken.keys().any(|(from, _)| *from == 3));
    }

    /// A block of a phi and a jump is not "one jump" (the pin's first
    /// instruction is the phi): bypassing it left the successor's phi reading a
    /// value nothing defined ("value#20 is read but never defined", horner at
    /// m32). It is copied like any join, and the phi's value stays defined.
    #[test]
    fn test_a_block_of_a_phi_and_a_jump_is_not_a_simple_block() {
        use crate::model::lir::Phi;
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            LirBlock {
                phis: vec![Phi { result: 40, incoming: vec![(1, 1), (2, 2)] }].into(),
                ..block(3, vec![jump(9)], &[9])
            },
            LirBlock {
                phis: vec![Phi { result: 50, incoming: vec![(3, 40)] }].into(),
                ..block(9, vec![of("ret", Operation::Return, None, false)], &[])
            },
        ];
        let shape = body(blocks);
        assert!(!is_simple(&shape, at(&shape, 3)));
        let done = tail_duplicate(&shape, 3, false).expect("copied like a join");
        let problems = crate::backend::verify::verify(&done, true);
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// A predecessor that reaches the join by falling into it has no jump to
    /// remove: the pass panicked on it ("a jump to the tail", c/logical_value
    /// at -O0) and, taking some other jump of the block for it, left reads
    /// of values nothing defined (nib print_float).
    #[test]
    fn test_a_predecessor_that_falls_into_the_join_takes_it_with_no_jump_to_remove() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2)], &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("duplicated");
        assert!(done.blocks.iter().all(|block| block.at != 3), "the join is gone");
        assert_eq!(insn_names(at(&done, 2)), ["mov", "mov", "jmp"], "the fall-through took the copy");
        assert_eq!(insn_names(at(&done, 1)), ["mov", "mov", "jmp"]);
        let problems = crate::backend::verify::verify(&done, true);
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// A read in a loop of a value the join defines: the block read in has one
    /// predecessor, the header two (the copies and the latch). Reading a block
    /// before its header, the walk met the header's phi as "no definition" and
    /// the phi took the join's own value, which no longer exists ("value read
    /// but never defined", nib `floats.print_float`).
    #[test]
    fn test_a_value_the_join_defines_and_a_loop_reads_is_a_phi_over_the_copies_and_itself() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(2), false), jump(1)], &[1, 2]),
            block(1, vec![work(1), jump(3)], &[3]),
            block(2, vec![work(2), jump(3)], &[3]),
            block(3, vec![work(3), jump(4)], &[4]),
            block(4, vec![of("jne", Operation::Branch, Some(5), false), jump(6)], &[5, 6]),
            block(5, vec![reading(3), jump(4)], &[4]),
            block(6, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        let done = tail_duplicate(&body(blocks), 3, false).expect("duplicated");
        let header = at(&done, 4);
        assert_eq!(header.phis.len(), 1, "{header:#?}");
        let phi = &header.phis[0];
        assert!(phi.incoming.iter().all(|(_, value)| *value != 3), "{phi:?}");
        assert_eq!(at(&done, 5).insns[0].uses, [phi.result], "the loop reads the phi");
        let problems = crate::backend::verify::verify(&done, true);
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// A predecessor that reaches the join by a branch as well as a jump (both
    /// edges to it) has a condition: it takes no copy, whose removal of the
    /// jump left the branch jumping to a block that is gone ("a jump to
    /// L2_93, which is nowhere", c/logical_value at -O0).
    #[test]
    fn test_a_predecessor_that_branches_to_the_join_takes_no_copy() {
        let blocks = vec![
            block(0, vec![of("jne", Operation::Branch, Some(3), false), jump(3)], &[3]),
            block(3, vec![work(3), jump(9)], &[9]),
            block(9, vec![of("ret", Operation::Return, None, false)], &[]),
        ];
        assert!(tail_duplicate(&body(blocks), 3, false).is_none());
    }
}
