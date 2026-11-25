//! Copies a short block into the blocks that only jump to it, then lets a
//! copy into a register the block returns fold into the move that made it
//! (LLVM's post-RA `TailDuplicator` and the backward half of
//! `MachineCopyPropagation`).
//!
//! # How it works
//! A block of at most 2 instructions (1 at optsize) that cannot fall through
//! is copied over the `b` of every block whose only exit is that `b`. A block
//! left with no jumper is dropped later as unreachable. A block that ends in a
//! computed goto may have up to 10 (`HasComputedGoto && !PreRegAlloc`): the
//! front end's one shared `goto *table[*pc++]` of an interpreter is copied
//! back into every handler, each with its own indirect branch.
//!
//! Then, in a block that returns, `mov d, s` with `s` a caller-saved register
//! that nothing reads afterwards, whose last write is an earlier plain
//! instruction, becomes that instruction writing `d`.
//!
//! ```text
//!  early:  mov x9, xzr         early:  mov x0, xzr
//!          b exit        ->            ret
//!  exit:   mov x0, x9
//!          ret
//! ```
//!
//! # Runs
//! After the frame is laid out, before block placement.
//!
//! # See also
//! [TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TailDuplicator.cpp#L562)
//! (`shouldTailDuplicate`, post-RA, not in layout mode) and
//! [MachineCopyPropagation.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineCopyPropagation.cpp)
//! (`BackwardCopyPropagateBlock`).
//!
//! DEVIATE: a block with a CFI directive or a jump-table label is not copied
//! (the pin copies DWARF CFI); the backward propagation only looks in blocks
//! that return, where the source is dead if it is caller-saved and not
//! returned (the pin reads kill flags). Owner w11, exit 2026-12-06.
//!
//! DEVIATE ARM64-TD-AFTER-PLACEMENT (owner w11, exit 2026-12-06): the pin
//! copies before block placement; this runs on the blocks placement ordered,
//! then propagates copies forward and backward (`MachineCopyPropagation` follows
//! `TailDuplicate` in `addMachineLateOptimization`).

use std::collections::{HashMap, HashSet};

use crate::frame::Lowered;
use crate::layout::label_of;
use crate::mc::inst::{Gpr, Inst, Logic, Shift, Target, Width};
use crate::mc::visit::{Reg, RegRef, Role};

/// What the pin allows a post-RA copy of a block that ends in a computed goto
/// (`TailDuplicator::shouldTailDuplicate`: `HasComputedGoto && !PreRegAlloc`).
const COMPUTED_GOTO_SIZE: usize = 10;

/// `TailDuplicator::shouldTailDuplicate` for `block`; `computed` is whether it
/// ends in a computed goto (`terminatorIsComputedGotoWithSuccessors`).
fn copyable(
    block: &Lowered,
    own: &str,
    size: usize,
    computed: bool,
) -> bool {
    let ends = matches!(
        block.insts.last(),
        Some(Inst::Branch { link: false, .. } | Inst::Ret { .. })
    )
        || computed && matches!(block.insts.last(), Some(Inst::BranchReg { link: false, .. }));
    let size = if computed { size.max(COMPUTED_GOTO_SIZE) } else { size };
    ends && block.insts.len() <= size
        && !block.insts.iter().any(|inst| matches!(inst, Inst::Cfi(_) | Inst::Label(_)))
        && !block.insts.iter().filter_map(label_of).any(|label| label == own)
}

/// The blocks that end in a computed goto: an `indirectbr` with successors,
/// all of them address-taken (`terminatorIsComputedGotoWithSuccessors`).
pub fn computed_gotos<'a>(
    indirect: &'a [(String, Vec<String>)],
    kept: &[String],
) -> HashSet<&'a str> {
    indirect
        .iter()
        .filter(|(_, targets)| !targets.is_empty() && targets.iter().all(|target| kept.contains(target)))
        .map(|(label, _)| label.as_str())
        .collect()
}

/// Copies the short blocks into their sole-exit jumpers; returns each
/// (jumper, copied block) pair, as the copy keeps the original's edge odds.
/// `indirect` is each computed goto's block and its targets.
pub fn tail_duplicate(
    blocks: &mut [Lowered],
    optsize: bool,
    computed: &HashSet<&str>,
    indirect: &[(String, Vec<String>)],
) -> Vec<(String, String)> {
    let size = if optsize { 1 } else { 2 };
    // A block that takes a computed goto's copy ends in one itself: the
    // pin asks `terminatorIsComputedGotoWithSuccessors` of every block, each
    // time round (`tailDuplicateBlocks` repeats until it changes nothing).
    let mut computed: HashSet<String> = computed.iter().map(|label| (*label).to_owned()).collect();
    let mut copies = Vec::new();
    // Each round removes a jump, so the rounds end; the bound is for a cycle
    // of tails copied into one another.
    for _ in 0..blocks.len() {
        let short: HashMap<String, (Vec<Inst>, Vec<Option<(u32, u32)>>)> = blocks
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, block)| {
                let goto = computed.contains(block.label.as_str());
                // "Don't try to tail-duplicate single-block loops": a dispatch that
                // can reach itself stays.
                let loops = indirect.iter().any(|(label, targets)| *label == block.label && targets.contains(label));
                copyable(block, &block.label, size, goto) && !loops
            })
            .map(|(_, block)| (block.label.clone(), (block.insts.clone(), block.locs.clone())))
            .collect();
        if short.is_empty() {
            break;
        }
        let at: HashMap<String, usize> =
            blocks.iter().enumerate().map(|(at, block)| (block.label.clone(), at)).collect();
        let mut changed = false;
        for (index, block) in blocks.iter_mut().enumerate() {
            // Only a block whose one exit is the jump takes a copy.
            let Some(Inst::Branch { link: false, target: Target::Label(to) }) = block.insts.last() else { continue };
            if block.insts.iter().filter_map(label_of).count() != 1 || *to == block.label {
                continue;
            }
            let to = to.clone();
            let Some((insts, locs)) = short.get(&to) else { continue };
            // "Don't duplicate into a fall-through predecessor."
            if at[&to] == index + 1 {
                continue;
            }
            block.insts.pop();
            block.locs.pop();
            block.insts.extend(insts.iter().cloned());
            block.locs.extend(locs.iter().copied());
            if computed.contains(&to) {
                computed.insert(block.label.clone());
            }
            copies.push((block.label.clone(), to));
            changed = true;
        }
        if !changed {
            break;
        }
    }
    copies
}

/// `mov rd, rm` as `orr rd, xzr, rm`: the copy's destination, source and
/// width.
fn copy_of(inst: &Inst) -> Option<(Gpr, Gpr, Width)> {
    match inst {
        Inst::LogicShifted { op: Logic::Orr, invert: false, width, rd, rn, rm, shift: Shift::Lsl, amount: 0 }
            if *rn == Gpr::R31 && rd.0 < 31 =>
        {
            Some((*rd, *rm, *width))
        }
        _ => None,
    }
}

/// `inst` with its one written register renamed to `to`, when it writes
/// exactly `from` and reads nothing of it.
fn renamed(
    inst: &Inst,
    from: Gpr,
    to: Gpr,
) -> Option<Inst> {
    let mut out = inst.clone();
    let (mut defs, mut reads_from) = (0, false);
    out.visit(&mut |register, role| {
        if let RegRef::Gpr(gpr) = register {
            if role == Role::Def && *gpr == from {
                defs += 1;
            } else if role.reads() && *gpr == from {
                reads_from = true;
            }
        }
    });
    if defs != 1 || reads_from || matches!(
        inst,
        Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. }
    ) {
        return None;
    }
    out.visit(&mut |register, role| {
        if let RegRef::Gpr(gpr) = register
            && role == Role::Def
            && *gpr == from
        {
            *gpr = to;
        }
    });
    Some(out)
}

/// `ForwardCopyPropagateBlock` over the laid-out blocks, after tail duplication
/// (the pin's `MachineCopyPropagation` runs again in
/// `addMachineLateOptimization`, after `TailDuplicate`): a read of a register a
/// live 64-bit copy filled reads the copy's source, a copy of a register back
/// onto the one it was copied from, or onto itself, goes. A write to either
/// register, a call, a return or an indirect jump ends the copy; so does a
/// label inside the block (another path joins there).
pub fn propagate_forward(blocks: &mut [Lowered]) {
    for block in blocks.iter_mut() {
        let mut copies: Vec<(Gpr, Gpr)> = Vec::new();
        let mut at = 0;
        while at < block.insts.len() {
            let plain = !matches!(
                block.insts[at],
                Inst::Branch { .. } | Inst::BranchReg { .. } | Inst::Ret { .. } | Inst::Cfi(_) | Inst::Label(_)
            );
            if plain && !copies.is_empty() {
                let mut inst = block.insts[at].clone();
                inst.visit(&mut |register, role| {
                    let RegRef::Gpr(used) = register else { return };
                    if role != Role::Use || used.0 == 31 {
                        return;
                    }
                    if let Some(&(_, source)) = copies.iter().find(|&&(dst, _)| dst == *used) {
                        *used = source;
                    }
                });
                block.insts[at] = inst;
            }
            if let Some((dst, src, Width::X)) = copy_of(&block.insts[at])
                && (dst == src || copies.iter().any(|&(d, s)| d == src && s == dst))
            {
                block.insts.remove(at);
                block.locs.remove(at);
                continue;
            }
            match &block.insts[at] {
                Inst::Label(_)
                | Inst::Branch { link: true, .. }
                | Inst::BranchReg { .. }
                | Inst::Ret { .. } => copies.clear(),
                inst => {
                    let (defs, _) = inst.defs_and_uses();
                    copies.retain(|&(d, s)| !defs.contains(&Reg::Gpr(d)) && !defs.contains(&Reg::Gpr(s)));
                    if let Some((dst, src, Width::X)) = copy_of(inst)
                        && src != Gpr::R31
                        && dst != src
                    {
                        copies.push((dst, src));
                    }
                }
            }
            at += 1;
        }
    }
}

/// The backward propagation in the blocks that return. `exits` are the
/// registers the function returns.
pub fn propagate_backward(
    blocks: &mut [Lowered],
    exits: &[Reg],
) {
    for block in blocks.iter_mut() {
        if !matches!(block.insts.last(), Some(Inst::Ret { .. })) {
            continue;
        }
        let mut at = 0;
        while at < block.insts.len() {
            let Some((dst, src, width)) = copy_of(&block.insts[at]) else {
                at += 1;
                continue;
            };
            let mine = |inst: &Inst| {
                let (defs, uses) = inst.defs_and_uses();
                (defs, uses)
            };
            // The source must be a caller-saved temporary that nothing after
            // the copy reads and the function does not return.
            let later_reads = block.insts[at + 1..].iter().any(|inst| mine(inst).1.contains(&Reg::Gpr(src)));
            let dead = src != Gpr::R31
                && src.0 <= 17
                && !exits.contains(&Reg::Gpr(src))
                && src != dst
                && !later_reads
                && !block.insts[at + 1..].iter().any(|inst| matches!(inst, Inst::Branch { link: true, .. }));
            if !dead {
                at += 1;
                continue;
            }
            // The last write of the source, with the destination untouched
            // and the source unread between.
            let mut found = None;
            for before in (0..at).rev() {
                let (defs, uses) = mine(&block.insts[before]);
                let touches_dst = defs.contains(&Reg::Gpr(dst)) || uses.contains(&Reg::Gpr(dst));
                if defs.contains(&Reg::Gpr(src)) {
                    if !touches_dst && let Some(new) = renamed(&block.insts[before], src, dst) {
                        // A 32-bit copy of a 64-bit write would drop the high
                        // half.
                        let wide = matches!(
                            block.insts[before],
                            Inst::LogicShifted { width: Width::W, .. }
                        )
                            || matches!(block.insts[before], Inst::MoveWide { .. });
                        if width == Width::X || wide || !is_64(&block.insts[before]) {
                            found = Some((before, new));
                        }
                    }
                    break;
                }
                if touches_dst
                    || uses.contains(&Reg::Gpr(src))
                    || matches!(block.insts[before], Inst::Branch { link: true, .. })
                {
                    break;
                }
            }
            match found {
                Some((before, new)) => {
                    block.insts[before] = new;
                    block.insts.remove(at);
                    block.locs.remove(at);
                }
                None => at += 1,
            }
        }
    }
}

/// Whether `inst` writes a 64-bit result.
fn is_64(inst: &Inst) -> bool {
    match inst {
        Inst::LogicShifted { width, .. } | Inst::LogicImm { width, .. } | Inst::AddSubImm { width, .. } => {
            *width == Width::X
        }
        Inst::AddSubShifted { width, .. } => *width == Width::X,
        _ => true,
    }
}

/// Drops the blocks nothing branches to or falls into (the short block, once
/// every jumper has its own copy). A block of `kept` has its address taken: an
/// `indirectbr` reaches it, whatever its labels' users say.
pub fn drop_unreferenced(
    blocks: &mut Vec<Lowered>,
    tables: &[crate::jumptable::Table],
    kept: &[String],
) {
    loop {
        let mut referenced: std::collections::HashSet<&str> = kept.iter().map(String::as_str).collect();
        for (at, block) in blocks.iter().enumerate() {
            referenced.extend(block.insts.iter().filter_map(label_of));
            referenced.extend(tables.iter().flat_map(|table| table.entries.iter().map(String::as_str)));
            let ends = matches!(
                block.insts.last(),
                Some(
                    Inst::Branch { link: false, .. }
                        | Inst::BranchReg { link: false, .. }
                        | Inst::Ret { .. }
                        | Inst::Brk { .. }
                )
            );
            if !ends && let Some(next) = blocks.get(at + 1) {
                referenced.insert(next.label.as_str());
            }
        }
        let dead: Vec<usize> =
            (1..blocks.len()).filter(|&at| !referenced.contains(blocks[at].label.as_str())).collect();
        if dead.is_empty() {
            return;
        }
        for at in dead.into_iter().rev() {
            blocks.remove(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::inst::AddSub;

    fn mov(
        dst: u32,
        src: u32,
    ) -> Inst {
        Inst::LogicShifted {
            op: Logic::Orr,
            invert: false,
            width: Width::X,
            rd: Gpr(dst),
            rn: Gpr(31),
            rm: Gpr(src),
            shift: Shift::Lsl,
            amount: 0,
        }
    }

    fn add(
        rd: u32,
        rn: u32,
    ) -> Inst {
        Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: Gpr(rd),
            rn: Gpr(rn),
            imm12: 1,
            shift12: false,
        }
    }

    fn after(insts: Vec<Inst>) -> Vec<Inst> {
        let locs = vec![None; insts.len()];
        let mut blocks = vec![Lowered { label: "f".into(), insts, locs }];
        propagate_forward(&mut blocks);
        blocks.remove(0).insts
    }

    /// A dispatch copied into a handler kept `mov x20, x25 ... mov x25, x20`: the
    /// carried value moved to a second register and straight back (arm64 QB
    /// `exec_loop` -Os: one wasted instruction per dispatched op).
    #[test]
    fn a_copy_back_onto_the_register_it_was_copied_from_goes() {
        let ret = Inst::Ret { rn: Gpr(30) };
        assert_eq!(after(vec![mov(20, 25), add(9, 23), mov(25, 20), ret.clone()]), vec![mov(20, 25), add(9, 23), ret]);
    }

    /// The dispatch copy of QB's arm64 `exec_loop`: the two carried-value
    /// copies, the table lookup and the `br`. The bounce back is the only
    /// instruction clang does not have (5 instructions, as its copy).
    #[test]
    fn a_dispatch_copy_keeps_its_carried_copies_and_loses_the_bounce_back() {
        let branch = Inst::BranchReg { link: false, rn: Gpr(9) };
        let copy = vec![mov(20, 25), mov(23, 9), add(9, 9), add(9, 26), mov(25, 20), branch.clone()];
        assert_eq!(after(copy), vec![mov(20, 25), mov(23, 9), add(9, 9), add(9, 26), branch]);
    }

    #[test]
    fn a_read_of_a_copys_destination_reads_its_source_and_a_write_ends_the_copy() {
        assert_eq!(after(vec![mov(20, 25), add(0, 20)]), vec![mov(20, 25), add(0, 25)]);
        assert_eq!(after(vec![mov(20, 25), add(25, 23), add(0, 20)]), vec![mov(20, 25), add(25, 23), add(0, 20)]);
    }
}
