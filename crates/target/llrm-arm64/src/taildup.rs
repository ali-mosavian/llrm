//! Copies a short block into the blocks that only jump to it, then lets a
//! copy into a register the block returns fold into the move that made it
//! (LLVM's post-RA `TailDuplicator` and the backward half of
//! `MachineCopyPropagation`).
//!
//! # How it works
//! A block of at most 2 instructions (1 at optsize) that cannot fall through
//! is copied over the `b` of every block whose only exit is that `b`. A block
//! left with no jumper is dropped later as unreachable.
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
//! returned (the pin reads kill flags). Exit: 11-12.

use std::collections::HashMap;

use crate::frame::Lowered;
use crate::layout::label_of;
use crate::mc::inst::{Gpr, Inst, Logic, Shift, Target, Width};
use crate::mc::visit::{Reg, RegRef, Role};

/// `TailDuplicator::shouldTailDuplicate` for `block`.
fn copyable(
    block: &Lowered,
    own: &str,
    size: usize,
) -> bool {
    let ends = matches!(
        block.insts.last(),
        Some(Inst::Branch { link: false, .. } | Inst::Ret { .. })
    );
    ends && block.insts.len() <= size
        && !block.insts.iter().any(|inst| matches!(inst, Inst::Cfi(_) | Inst::Label(_)))
        && !block.insts.iter().filter_map(label_of).any(|label| label == own)
}

/// Copies the short blocks into their sole-exit jumpers.
pub fn tail_duplicate(
    blocks: &mut [Lowered],
    optsize: bool,
) {
    let size = if optsize { 1 } else { 2 };
    let short: HashMap<String, (Vec<Inst>, Vec<Option<(u32, u32)>>)> = blocks
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, block)| copyable(block, &block.label, size))
        .map(|(_, block)| (block.label.clone(), (block.insts.clone(), block.locs.clone())))
        .collect();
    if short.is_empty() {
        return;
    }
    let at: HashMap<String, usize> = blocks.iter().enumerate().map(|(at, block)| (block.label.clone(), at)).collect();
    for (index, block) in blocks.iter_mut().enumerate() {
        // Only a block whose one exit is the jump takes a copy.
        let Some(Inst::Branch { link: false, target: Target::Label(to) }) = block.insts.last() else { continue };
        if block.insts.iter().filter_map(label_of).count() != 1 || *to == block.label {
            continue;
        }
        let Some((insts, locs)) = short.get(to) else { continue };
        // "Don't duplicate into a fall-through predecessor."
        if at[to] == index + 1 {
            continue;
        }
        block.insts.pop();
        block.locs.pop();
        block.insts.extend(insts.iter().cloned());
        block.locs.extend(locs.iter().copied());
    }
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
/// every jumper has its own copy).
pub fn drop_unreferenced(
    blocks: &mut Vec<Lowered>,
    tables: &[crate::jumptable::Table],
) {
    loop {
        let mut referenced: std::collections::HashSet<&str> = std::collections::HashSet::new();
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
