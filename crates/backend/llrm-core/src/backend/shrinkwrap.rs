//! Sets up a procedure's frame and kept registers where they are first needed,
//! not at the entry, and takes them back only on the returns that path reaches.
//!
//! # How it works
//! A procedure has pieces to set up: the frame (the stack reserve and the frame
//! register) and each register it keeps for its caller. A piece is set up in
//! its home, the nearest block that dominates every use of it and is outside
//! any loop (a set-up in a loop would run each trip). It is taken back before
//! each return that home dominates. This is gcc's separate shrink-wrapping
//! (`try_shrink_wrapping_separate`), which places each component, where LLVM's
//! ShrinkWrap moves the whole prologue.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   entry["<b>entry</b>"]:::mid
//!   test["<b>test</b>"]:::mid
//!   early["<b>early return</b><br/><small>no push, no frame</small>"]:::mid
//!   home["<b>home</b><br/><small>push si#59; frame</small>"]:::src
//!   loop["<b>loop</b>"]:::mid
//!   ret["<b>pop si#59; ret</b>"]:::mid
//!   entry --> test
//!   test --> early
//!   test --> home
//!   home --> loop
//!   loop --> ret
//! ```
//!
//! *Frame and pushes only on the path that needs them*
//!
//! A piece is wrapped only if it is safe and pays:
//!
//! - Some return is not dominated by the home. Otherwise every path pays for
//!   the set-up and nothing is gained.
//! - A return the home reaches is also dominated by it. A return reachable both
//!   through the home and around it would pop what was never pushed.
//! - For the frame, and for every piece when the stack pointer addresses the
//!   frame cells, each block reachable from the home is dominated by it, so
//!   every block is entered at one stack depth.
//!
//! No register is set up above the frame, since the frame's cells sit just
//! below the arguments and a push before the reserve would cover them. The
//! frame therefore comes up to the dominator of its own uses and the homes of
//! the registers, or stays at the entry if a register is set up there. Pieces
//! open in the order of the dominator chain, the frame first, and close in
//! reverse.
//!
//! `tails_split` makes more pieces wait. A small block that ends in a return
//! and is reached by two or more paths (at most `MAX_GROW` = 8 instructions) is
//! copied for each way into it, when that raises `worth`, how much of the
//! procedure is set up late, to at least `WORTH` (three pieces). The early path
//! then returns through a copy that nothing was saved for.
//!
//! # Example
//! The pass places pieces while masm builds the procedure, after LIR is
//! final, so its result is in the assembly, not in a LIR dump. A function that
//! returns `k` at once when `n` is 0 and otherwise loops over calls to `g`:
//!
//! ```asm
//! f_ proc near
//! L0_0:
//!     or eax, eax
//!     je L0_13          ; early return: no push, no pop
//! L0_2:
//!     push ecx          ; set-up in the home block
//!     push ebx
//!     push esi
//!     ; ... loop with `call g_`
//! L0_14:
//!     mov eax, edx
//!     pop esi           ; taken back only on this return
//!     pop ebx
//!     pop ecx
//!     ret
//! L0_13:
//!     mov eax, edx
//!     ret
//! f_ endp
//! ```
//!
//! The three pushes sit in `L0_2`, which the early return does not reach, and
//! the early return leaves with a bare `ret`.
//!
//! From `-S` of a C function (`if (n == 0) return k;` then a loop calling `g`)
//! at -O2 -m32; the comments are added.
//!
//! # Runs
//! `wrapped` runs when masm builds a procedure's items, for a procedure that
//! saves a register or has a frame, whose blocks are all reachable from the
//! entry, and that returns. The frame is not moved in an interrupt handler, a
//! runtime entry frame, a procedure with a stack check or one with inline code.
//! Registers are not wrapped in an interrupt handler or a runtime entry frame.
//! `tails_split` runs before that in assembly, after `lir duplicated returns`,
//! not at -Os, and only for a procedure with at least three pieces. Neither has
//! a level switch beyond that.
//!
//! # See also
//! LLVM's
//! [`ShrinkWrap`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/ShrinkWrap.cpp)
//! and gcc's
//! [`shrink-wrap.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/shrink-wrap.cc).
//! The tail copy size is in `docs/reference/levels.md`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};

use crate::analysis::loops::{dominance, immediate_dominators, loops};
use crate::model::ir::{self, Loc, Mem};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};

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
pub fn named(
    regs: Regs,
    one: &Insn,
) -> BTreeSet<RegId> {
    let mut found: BTreeSet<RegId> =
        one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(|register| regs.root(register))).collect();
    let Some(what) = &one.what else { return found };
    for place in what.dests.iter().chain(&what.sources) {
        match place {
            Loc::Reg(ir::Reg { register, .. }) => {
                found.insert(regs.root(*register));
            }
            Loc::Mem(ir::Mem { through, index_through, .. }) => {
                found.extend([*through, *index_through].map(|register| regs.root(register)));
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
        frame.iter().flat_map(|(pointer, stack)| [body.regs().root(*pointer), body.regs().root(*stack)]).collect();
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
            let names = named(body.regs(), one);
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
        // Where a return is reached around the home, the set-up goes up to the
        // nearest block above it that every such return is below: LLVM's
        // ShrinkWrap moves the save point up the dominators until it reaches
        // the restores' dominator. A guard that skips the loop a register is
        // used in joins the returns after it, and kept that register at the
        // entry.
        let strict = stack_addressed || piece == Piece::Frame;
        let mut candidate = Some(home);
        while let Some(at) = candidate {
            if returns.iter().all(|&ret| from_entry.dominates(at, ret)) {
                break;
            }
            if closed_at(at, strict).is_some() {
                homes.insert(piece, at);
                break;
            }
            candidate = idom.get(&at).copied().flatten().filter(|&above| above != body.entry);
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
            // The copy lets a path reload nothing it stored on the way: what
            // the early path stored for the tail to read is gone before the
            // pieces it needs are counted.
            let made = returns_cleaned(&made);
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

/// `body` with, on each path that returns through a block of its own, the
/// reloads a store on that path fills forwarded and the allocator's stores no
/// later instruction of the path reads dropped: a frame dies at its return, as
/// gcc's and LLVM's dead store elimination of a local at the end of a function.
fn returns_cleaned(body: &LirBody) -> LirBody {
    let body = crate::backend::spillforward::forwarded(body);
    let parents = crate::backend::jumps::_predecessors(&body.blocks);
    let mut cleaned: BTreeMap<i64, Vec<Arc<Insn>>> = BTreeMap::new();
    for block in &body.blocks {
        let returns = block
            .insns
            .last()
            .is_some_and(|last| last.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return));
        let Some(&only) =
            parents.get(&block.at).filter(|ways| ways.len() == 1 && returns).and_then(|ways| ways.first())
        else {
            continue;
        };
        let Some(way) = body.blocks.iter().find(|one| one.at == only) else { continue };
        if way.succ != [block.at] {
            continue;
        }
        let mut read: Vec<Mem> = Vec::new();
        let mut dead: BTreeSet<usize> = BTreeSet::new();
        for one in block.insns.iter().chain(way.insns.iter()).rev() {
            let Some(what) = &one.what else { continue };
            if one.spill_store
                && let [Loc::Mem(cell)] = what.dests.as_slice()
                && crate::backend::peephole::_frame_cell(cell)
                && !read.iter().any(|seen| crate::backend::peephole::_overlapping(seen, cell))
            {
                dead.insert(crate::backend::peephole::id(one));
                continue;
            }
            for place in what.sources.iter().chain(what.dests.iter().filter(|_| what.op != ir::Operation::Move)) {
                if let Loc::Mem(cell) = place {
                    read.push(cell.clone());
                }
            }
        }
        if !dead.is_empty() {
            cleaned.insert(
                way.at,
                way.insns
                    .iter()
                    .map(|one| {
                        if dead.contains(&crate::backend::peephole::id(one)) {
                            crate::model::lir::anchor(Arc::clone(one))
                        } else {
                            Arc::clone(one)
                        }
                    })
                    .collect(),
            );
        }
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| match cleaned.remove(&block.at) {
            Some(insns) => block.with_insns(insns),
            None => block.clone(),
        })
        .collect();
    body.with_blocks(blocks)
}

/// `body` with `tail` copied for each of `ways`, the original gone.
fn tail_copied(
    body: &LirBody,
    tail: &LirBlock,
    ways: &BTreeSet<i64>,
) -> Option<LirBody> {
    let mut next = body.blocks.iter().map(|block| block.at).max()? + 1;
    let mut odds = body.odds.clone();
    let mut blocks: Vec<Block> = Vec::new();
    let mut made: Vec<Block> = Vec::new();
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
                one.what.as_ref().is_some_and(|what| what.remapped(|to| if to == tail.at { -1 } else { to }).is_some())
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
            match one.what.as_ref().and_then(|what| what.remapped(|to| if to == tail.at { copy } else { to })) {
                Some(what) => {
                    let mut moved = (**one).clone();
                    moved.what = Some(what);
                    insns.push(Arc::new(moved));
                }
                None => insns.push(Arc::clone(one)),
            }
        }
        let succ = block.succ.iter().map(|&to| if to == tail.at { copy } else { to }).collect();
        blocks.push(LirBlock { succ, ..block.with_insns(insns).into_inner() }.into());
        let body_copy: Vec<Arc<Insn>> = tail.insns.iter().map(|one| Arc::new((**one).clone())).collect();
        let own = LirBlock { succ: tail.succ.clone(), ..LirBlock::new(copy, body_copy) };
        // A block that fell into the tail falls into its copy; the others jump
        // to theirs, after the rest.
        if named == 0 { blocks.push(own.into()) } else { made.push(own.into()) }
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
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<Block>) -> LirBody {
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
        let wrap =
            wrapped(&body(crate::model::lir::shared(blocks)), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false)
                .expect("a wrap");
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
        assert!(
            wrapped(&body(crate::model::lir::shared(blocks)), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false)
                .is_none()
        );
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
        let before = body(crate::model::lir::shared(blocks));
        assert_eq!(worth(&before), 0);
        let after = tails_split(&before, &worth);
        assert_eq!(after.blocks.len(), 4, "the return, once for each way into it");
        assert!(worth(&after) >= WORTH);
    }
}
