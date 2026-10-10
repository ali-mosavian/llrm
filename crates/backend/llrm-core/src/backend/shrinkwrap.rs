//! LLVM's ShrinkWrap: the registers a procedure keeps for its caller (SI and
//! DI), and its frame (the stack reserve and the frame register), set up where
//! they are first needed instead of at every entry, and taken back only on the
//! returns that path reaches.
//!
//! A procedure that leaves early (`if row == 7 { return 1 }`) before any of its
//! loop touches SI paid a push and a pop for it on every call. Each piece (a
//! register kept, the frame) is set up at the block that dominates every use of
//! it, as gcc's `shrink_wrap_separate` places each component, and taken back at
//! each return that block dominates; where a return can be reached both
//! through that block and around it, or the set-up would sit in a loop, the
//! piece is set up at the entry as before. One piece's entry use no longer
//! holds the others there: `paths(n, m)` named ECX at its entry, and paid the
//! frame and four more pushes for its `n == 0` answer.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::loops::{dominance, immediate_dominators, loops};
use crate::model::ir::Semantics;
use crate::model::ir::{self, Loc};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// What a procedure sets up and takes back: the frame, or a register it keeps
/// for its caller.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Piece {
    Frame,
    Saved(RegId),
}

/// Where a procedure sets its pieces up that are not set up at its entry:
/// at the top of each block of `opens`, in that order, and taken back before
/// each return of `closes`, in that order, before what the entry set up.
pub struct Wrap {
    pub opens: BTreeMap<i64, Vec<Piece>>,
    pub closes: BTreeMap<i64, Vec<Piece>>,
    pub wrapped: BTreeSet<Piece>,
    /// How far below the entry the pieces are set up: the blocks above each
    /// home, added.
    pub depth: usize,
}

/// The registers `one` names, and those the call it is disturbs by its
/// convention.
pub fn named(one: &Insn) -> BTreeSet<RegId> {
    let mut found: BTreeSet<RegId> =
        one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(ir::root)).collect();
    let Some(what) = &one.what else { return found };
    for place in what.dests.iter().chain(&what.sources) {
        match place {
            Loc::Reg(ir::Reg { register, .. }) => {
                found.insert(ir::root(*register));
            }
            Loc::Mem(ir::Mem { through, index_through, .. }) => {
                found.extend([*through, *index_through].map(ir::root));
            }
            _ => {}
        }
    }
    found
}

/// Whether `one` reads or writes the frame or the stack pointer: a frame cell
/// or argument, a push or pop, or an instruction the printer cannot read.
fn touches_frame(one: &Insn) -> bool {
    let Some(what) = &one.what else { return false };
    if matches!(
        what.op,
        ir::Operation::Push | ir::Operation::Pop | ir::Operation::Leave | ir::Operation::Escape
    )
        || (what.op == ir::Operation::Nothing
            && what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")))
    {
        return true;
    }
    what.dests.iter().chain(&what.sources).any(Loc::in_frame)
}

/// The wrap of `body` for the registers `kept`, and for the frame where `frame`
/// names the frame register and the stack pointer, if one is worth having.
/// No register is set up above the frame, whichever register addresses its
/// cells: they sit directly below the arguments, and a push before the
/// reserve would put the cells over it. With `stack_addressed` the stack
/// pointer addresses them at the depth each block is entered at, and every
/// block must be entered at one, whatever a piece sets up.
pub fn wrapped(
    body: &LirBody,
    kept: &BTreeSet<RegId>,
    frame: Option<(RegId, RegId)>,
    stack_addressed: bool,
) -> Option<Wrap> {
    if body.noreturn || (kept.is_empty() && frame.is_none()) {
        return None;
    }
    let entry = Some(body.entry);
    let from_entry = dominance(&body.blocks, entry);
    // Every block must be reached from the entry: a block only the runtime
    // enters has no dominator.
    if body.blocks.iter().any(|block| !from_entry.reachable(block.at)) {
        return None;
    }
    let idom = immediate_dominators(&body.blocks, entry);
    let cycles = loops(&body.blocks, entry);
    let above = |block: i64| {
        let mut chain = vec![block];
        while let Some(&Some(next)) = idom.get(chain.last().expect("one")) {
            chain.push(next);
        }
        chain
    };
    let frame_registers: BTreeSet<RegId> =
        frame.iter().flat_map(|(pointer, stack)| [ir::root(*pointer), ir::root(*stack)]).collect();
    // The nearest block above every use that is not in a loop: a set-up there
    // would run each trip. None where that is the entry.
    let home_of = |uses: &[i64]| -> Option<i64> {
        let mut home = *uses.first()?;
        // The home only rises, so the climb is the depth of the first use in
        // all, not a chain of its ancestors for each use.
        for &other in &uses[1..] {
            while !from_entry.dominates(home, other) {
                home = idom[&home].expect("the entry dominates every block");
            }
        }
        while home != body.entry && cycles.iter().any(|one| one.body.contains(&home)) {
            home = idom[&home].expect("a block below the entry has a dominator");
        }
        (home != body.entry).then_some(home)
    };
    // Whether a piece set up at `home` is entered at one depth everywhere and
    // taken back on every return it reaches, and those returns.
    let closed_at = |home: i64, strict: bool| -> Option<BTreeSet<i64>> {
        let from_home = dominance(&body.blocks, Some(home));
        // A block it reaches that something else also reaches would be entered
        // at two depths.
        if strict
            && body.blocks.iter().any(|block| from_home.reachable(block.at) && !from_entry.dominates(home, block.at))
        {
            return None;
        }
        let mut returns = BTreeSet::new();
        for block in &body.blocks {
            let returning =
                block.insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return));
            if !returning || !from_home.reachable(block.at) {
                continue;
            }
            // Reached around the home as well: a restore there would pop what
            // was never pushed.
            if !from_entry.dominates(home, block.at) {
                return None;
            }
            returns.insert(block.at);
        }
        Some(returns)
    };
    let mut pieces: Vec<Piece> = Vec::new();
    if frame.is_some() {
        pieces.push(Piece::Frame);
    }
    pieces.extend(kept.iter().map(|&register| Piece::Saved(register)));
    // The blocks that use each piece, found in one pass over the instructions.
    let mut uses: BTreeMap<Piece, Vec<i64>> = pieces.iter().map(|&piece| (piece, Vec::new())).collect();
    for block in &body.blocks {
        let mut found: BTreeSet<Piece> = BTreeSet::new();
        for one in block.insns.iter() {
            let names = named(one);
            if frame.is_some()
                && (touches_frame(one) || names.iter().any(|register| frame_registers.contains(register)))
            {
                found.insert(Piece::Frame);
            }
            found.extend(
                names.iter().filter(|register| kept.contains(register)).map(|&register| Piece::Saved(register)),
            );
        }
        for piece in found {
            uses.get_mut(&piece).expect("a piece").push(block.at);
        }
    }
    let returns: Vec<i64> = body
        .blocks
        .iter()
        .filter(|block| {
            block.insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return))
        })
        .map(|block| block.at)
        .collect();
    let mut homes: BTreeMap<Piece, i64> = BTreeMap::new();
    for &piece in &pieces {
        let Some(home) = home_of(&uses[&piece]) else { continue };
        // Set up late only where some return skips it: else every path pays
        // for it, as at the entry, and the pushes and pops are copied for
        // nothing.
        if returns.iter().all(|&at| from_entry.dominates(home, at)) {
            continue;
        }
        if closed_at(home, stack_addressed || piece == Piece::Frame).is_some() {
            homes.insert(piece, home);
        }
    }
    // The frame register addresses the cells and the arguments from the frame's
    // set-up on: no register is set up above it, so the frame comes up to the
    // block above the homes of the registers (the entry, when one is set up
    // there or not wrapped, where it stays).
    if homes.contains_key(&Piece::Frame) {
        let mut blocks = uses[&Piece::Frame].clone();
        let mut entry_use = false;
        for piece in &pieces {
            if *piece == Piece::Frame || uses[piece].is_empty() {
                continue;
            }
            match homes.get(piece) {
                Some(&home) => blocks.push(home),
                None => entry_use = true,
            }
        }
        let lifted = if entry_use { None } else { home_of(&blocks) };
        match lifted {
            Some(home)
                if returns.iter().any(|&at| !from_entry.dominates(home, at)) && closed_at(home, true).is_some() =>
            {
                homes.insert(Piece::Frame, home);
            }
            _ => {
                homes.remove(&Piece::Frame);
            }
        }
    }
    if homes.is_empty() {
        return None;
    }
    // Opened in the order of the dominator chain, the frame before a register
    // in one block.
    let depth = |block: i64| above(block).len();
    let mut order: Vec<(usize, Piece)> = homes.iter().map(|(&piece, &home)| (depth(home), piece)).collect();
    order.sort();
    let mut opens: BTreeMap<i64, Vec<Piece>> = BTreeMap::new();
    for (&piece, &home) in &homes {
        opens.entry(home).or_default().push(piece);
    }
    let mut closes: BTreeMap<i64, Vec<Piece>> = BTreeMap::new();
    for (&piece, &home) in &homes {
        let strict = stack_addressed || piece == Piece::Frame;
        for block in closed_at(home, strict).expect("checked above") {
            closes.entry(block).or_default().push(piece);
        }
    }
    let rank = |piece: &Piece| order.iter().position(|(_, one)| one == piece).expect("a wrapped piece");
    for pieces in closes.values_mut() {
        pieces.sort_by_key(|piece| std::cmp::Reverse(rank(piece)));
    }
    for pieces in opens.values_mut() {
        pieces.sort_by_key(rank);
    }
    let depth = homes.values().map(|&home| above(home).len() - 1).sum();
    Some(Wrap { opens, closes, wrapped: homes.keys().copied().collect(), depth })
}

/// The most instructions in a tail copied to give early paths one of their own:
/// gcc's `try_shrink_wrapping` copies a block of at most
/// `max-grow-copy-bb-insns` (8, params.opt:529) unconditional jumps' length
/// (shrink-wrap.cc:781-782, `can_dup_for_shrink_wrapping`); a jump is one
/// instruction here.
const MAX_GROW: usize = 8;
/// The least `worth` (a thousand a piece) that pays for the copies: three
/// pieces, six instructions off the early path.
const WORTH: usize = 3000;
/// The pieces a procedure must have for a split to be worth asking after.
pub const PIECES: usize = 3;

/// `body` with the small blocks that end in a return copied for each way into
/// them, while `worth` (how much of the procedure is set up late) rises: a path
/// that returns early joined the others at the return, so the return was
/// reachable around whatever they set up, and nothing could be set up late
/// (gcc's `try_shrink_wrapping` duplicates the tail the same way). Each copy
/// follows the last block, and the paths that fell into the tail now jump.
pub fn tails_split(
    body: &LirBody,
    worth: &dyn Fn(&LirBody) -> usize,
) -> LirBody {
    let mut best = body.clone();
    let mut score: Option<usize> = None;
    'again: loop {
        let parents = crate::backend::jumps::_predecessors(&best.blocks);
        for tail in &best.blocks {
            let ways = parents.get(&tail.at).map_or(0, BTreeSet::len);
            let returns = tail
                .insns
                .last()
                .is_some_and(|last| last.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return));
            if tail.at == best.entry
                || ways < 2
                || !returns
                || !tail.phis.is_empty()
                || tail.insns.len() > MAX_GROW
                || tail.insns.iter().any(|one| one.arrival())
            {
                continue;
            }
            let before = *score.get_or_insert_with(|| worth(&best));
            let Some(made) = tail_copied(&best, tail, &parents[&tail.at]) else { continue };
            let now = worth(&made);
            // Only where the early path skips several pieces: each is a push
            // and a pop saved, and the copy costs bytes on every
            // call.
            if now > before && now >= WORTH {
                best = made;
                score = Some(now);
                continue 'again;
            }
        }
        return best;
    }
}

/// `body` with `tail` copied for each of `ways`, the original gone.
fn tail_copied(
    body: &LirBody,
    tail: &LirBlock,
    ways: &BTreeSet<i64>,
) -> Option<LirBody> {
    let mut next = body.blocks.iter().map(|block| block.at).max()? + 1;
    let mut odds = body.odds.clone();
    let mut blocks: Vec<LirBlock> = Vec::new();
    let mut made: Vec<LirBlock> = Vec::new();
    for block in &body.blocks {
        if block.at == tail.at {
            continue;
        }
        if !ways.contains(&block.at) {
            blocks.push(block.clone());
            continue;
        }
        // Only the edges a branch or a jump names, or the fall-through into the
        // tail, are redirected; more than one edge to it is left alone.
        let named = block
            .insns
            .iter()
            .filter(|one| {
                one.what
                    .as_ref()
                    .is_some_and(
                        |what| matches!(what.op, ir::Operation::Branch | ir::Operation::Jump)
                            && !what.indirect
                            && what.target == Some(tail.at),
                    )
            })
            .count();
        if named > 1 || block.succ.iter().filter(|&&to| to == tail.at).count() != 1 {
            return None;
        }
        let copy = next;
        next += 1;
        odds.rerouted(block.at, &block.succ, tail.at, &[(copy, 1.0)]);
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in block.insns.iter() {
            match one.what.as_ref() {
                Some(what)
                    if matches!(what.op, ir::Operation::Branch | ir::Operation::Jump)
                        && !what.indirect
                        && what.target == Some(tail.at) =>
                {
                    let mut moved = (**one).clone();
                    moved.what = Some(Semantics { target: Some(copy), ..what.clone() });
                    insns.push(Arc::new(moved));
                }
                _ => insns.push(Arc::clone(one)),
            }
        }
        let succ = block.succ.iter().map(|&to| if to == tail.at { copy } else { to }).collect();
        blocks.push(LirBlock { succ, ..block.with_insns(insns) });
        let body_copy: Vec<Arc<Insn>> = tail.insns.iter().map(|one| Arc::new((**one).clone())).collect();
        let own = LirBlock { succ: tail.succ.clone(), ..LirBlock::new(copy, body_copy) };
        // A block that fell into the tail falls into its copy; the others jump
        // to theirs, after the rest.
        if named == 0 { blocks.push(own) } else { made.push(own) }
    }
    blocks.extend(made);
    let mut out = body.with_blocks(blocks);
    out.odds = odds;
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Imm, Reg, Semantics};
    use crate::support::hash::IndexMap;

    fn insn(
        at: i64,
        what: Semantics,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![]))
    }

    /// Writes `register`.
    fn touch(
        at: i64,
        register: RegId,
    ) -> Arc<Insn> {
        let one = Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Reg(Reg { register, width: 4 })],
            sources: vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
            ..Semantics::new(ir::Operation::Move)
        };
        insn(at, one)
    }

    fn branch(
        at: i64,
        target: i64,
    ) -> Arc<Insn> {
        insn(
            at,
            Semantics { name: Some("je".to_owned()), target: Some(target), ..Semantics::new(ir::Operation::Branch) },
        )
    }

    fn ret(at: i64) -> Arc<Insn> {
        insn(at, Semantics { name: Some("ret".to_owned()), ..Semantics::new(ir::Operation::Return) })
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new("wrap", blocks[0].at, blocks, IndexMap::default(), IndexMap::default())
    }

    /// `paths(0, m)` named a register at its entry and paid the frame and the
    /// pushes of every other for its answer: a register used at the entry no
    /// longer holds the others there.
    #[test]
    fn test_a_register_used_below_the_early_exit_is_saved_below_it_though_the_entry_uses_another() {
        let blocks = vec![
            block(0, vec![touch(0, RegId::EBX), branch(2, 1)], vec![1, 2]),
            block(1, vec![ret(10)], vec![]),
            block(2, vec![touch(20, RegId::ESI), ret(22)], vec![]),
        ];
        let wrap = wrapped(&body(blocks), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false).expect("a wrap");
        assert_eq!(wrap.wrapped, BTreeSet::from([Piece::Saved(RegId::ESI)]));
        assert_eq!(wrap.opens, BTreeMap::from([(2, vec![Piece::Saved(RegId::ESI)])]));
        assert_eq!(wrap.closes, BTreeMap::from([(2, vec![Piece::Saved(RegId::ESI)])]));
    }

    /// A return that the early exit and the late path share is reached around
    /// the save: nothing is saved late.
    #[test]
    fn test_a_return_the_early_exit_shares_with_the_late_path_keeps_the_save_at_the_entry() {
        let blocks = vec![
            block(0, vec![touch(0, RegId::EBX), branch(2, 3)], vec![3, 2]),
            block(2, vec![touch(20, RegId::ESI)], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        assert!(wrapped(&body(blocks), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false).is_none());
    }

    /// The shared return copied for each way into it, the early exit has a
    /// return of its own that nothing was saved for.
    #[test]
    fn test_a_small_shared_return_is_copied_when_that_lets_three_pieces_wait() {
        let blocks = vec![
            block(0, vec![touch(0, RegId::EAX), branch(2, 3)], vec![3, 2]),
            block(2, vec![touch(20, RegId::EBX), touch(22, RegId::ESI), touch(24, RegId::EDI)], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        let kept = BTreeSet::from([RegId::EBX, RegId::ESI, RegId::EDI]);
        let worth = |candidate: &LirBody| {
            wrapped(candidate, &kept, None, false).map_or(0, |wrap| wrap.wrapped.len() * 1000 + wrap.depth)
        };
        let before = body(blocks);
        assert_eq!(worth(&before), 0);
        let after = tails_split(&before, &worth);
        assert_eq!(after.blocks.len(), 4, "the return, once for each way into it");
        assert!(worth(&after) >= WORTH);
    }
}
