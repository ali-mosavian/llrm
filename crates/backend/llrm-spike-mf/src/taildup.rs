//! Tail duplication after allocation: a small block that ends in a conditional
//! branch is copied into each predecessor that only jumps to it, inside a loop;
//! a block that only returns is copied into each predecessor that jumps to it.
//!
//! ```text
//!  before                          after
//!  a: ..; bra c                    a: ..; cmp; ble h   (falls to the exit)
//!  b: ..                           b: ..
//!  c: cmp; ble h                   c: cmp; ble h
//!  x: exit                         x: exit
//! ```
//!
//! The copy runs the compare where the `bra` was: a loop trip that took `a`
//! no longer pays a taken branch to reach it. [`crate::placement`] then lays
//! the copies out and fixes their terminators.
//!
//! DEVIATE: the pin does this inside `MachineBlockPlacement`, as chains are
//! built, with frequencies (`isProfitableToTailDup`,
//! `canTailDuplicateUnplacedPreds`, `precomputeTriangleChains`) and
//! `TailDuplicator::shouldTailDuplicate` / `canTailDuplicate` for the copy.
//! Here it is a pass before placement: the block is copied when it has at most
//! `max_size` instructions (`TailDupSize`) and two ways on, the predecessor has
//! no other way, and both lie in one loop (a trip through the copy saves the
//! taken branch); no triangle chains, no profile. Reason: bsearch at 68040 -O2
//! runs 13 clk behind gcc, which copies the loop's latch compare into both
//! arms. Owner m68k. Exit: 2026-11-28, when the placement host takes over
//! (`llrm-codegen`'s port, `placement/taildup.rs`).
//!
//! # See also
//! `TailDuplicator::shouldTailDuplicate`, `canTailDuplicate`, `tailDuplicate`.

use crate::cfg::Cfg;
use crate::fold::analyze;
use crate::mfunc::{Inst, MFunction, Operand};
use crate::sink::Jump;

/// Copies the small conditional blocks of `function` into the predecessors
/// that jump to them; true when anything was copied.
pub fn tail_duplicate<C, J: Jump>(
    function: &mut MFunction<C>,
    max_size: usize,
) -> bool {
    let mut changed = false;
    let mut tail = 1;
    while tail < function.blocks.len() {
        if duplicate_into_predecessors::<C, J>(function, tail, max_size) {
            changed = true;
            // nothing reaches the original once every way in was a copy
            if !reached(function, tail) {
                function.blocks.remove(tail);
                continue;
            }
        }
        tail += 1;
    }
    changed
}

/// Copies the small return blocks of `function` into the predecessors that go
/// only there; true when anything was copied. A separate pass from
/// [`tail_duplicate`]: a copy grows the code by the epilogue it expands to,
/// which only the caller can price.
pub fn duplicate_returns<C>(
    function: &mut MFunction<C>,
    max_size: usize,
) -> bool {
    let mut changed = false;
    let mut tail = 1;
    while tail < function.blocks.len() {
        if duplicate_return::<C>(function, tail, max_size) {
            changed = true;
            if !reached(function, tail) {
                function.blocks.remove(tail);
                continue;
            }
        }
        tail += 1;
    }
    changed
}

/// Whether a branch names block `at` or the block before it falls into it.
fn reached<C>(
    function: &MFunction<C>,
    at: usize,
) -> bool {
    let id = function.blocks[at].id;
    let named = function
        .blocks
        .iter()
        .enumerate()
        .any(
            |(one, block)| one != at
                && block
                    .insts
                    .iter()
                    .any(|inst| inst.operands.iter().any(|op| matches!(op, Operand::Block(to) if *to == id))),
        );
    let falls = at > 0 && !function.blocks[at - 1].insts.last().is_some_and(|inst| inst.barrier);
    named || falls
}

/// A block that only returns is copied into each predecessor that ends in a
/// jump to it (`shouldTailDuplicate`: no successors, at most `TailDupSize`
/// instructions, not before allocation), and the jump goes. Two or more ways
/// in, else the copy is the block.
fn duplicate_return<C>(
    function: &mut MFunction<C>,
    tail: usize,
    max_size: usize,
) -> bool {
    let id = function.blocks[tail].id;
    let body = &function.blocks[tail].insts;
    let returns = body
        .last()
        .is_some_and(
            |inst| inst.terminator && inst.barrier && !inst.operands.iter().any(|op| matches!(op, Operand::Block(_))),
        );
    if !returns || body.len() > max_size || body.iter().any(|inst| inst.call || inst.side_effects) {
        return false;
    }
    // a predecessor goes only here: by a jump, or by falling into the block
    let jumps: Vec<(usize, bool)> = (0..function.blocks.len())
        .filter(|&pred| pred != tail)
        .filter_map(|pred| {
            let from = analyze(function, pred)?;
            let ends_in_jump = function.blocks[pred].insts.last().is_some_and(|inst| inst.terminator && inst.barrier);
            if from.cond.is_none() && from.taken == Some(id) && ends_in_jump {
                Some((pred, true))
            } else if pred + 1 == tail && from.cond.is_none() && from.taken.is_none() && !ends_in_jump {
                Some((pred, false))
            } else {
                None
            }
        })
        .collect();
    if jumps.len() < 2 {
        return false;
    }
    let copy: Vec<Inst> = function.blocks[tail].insts.clone();
    for (pred, jump) in jumps {
        let block = &mut function.blocks[pred].insts;
        if jump {
            block.pop();
        }
        block.extend(copy.iter().cloned());
    }
    true
}

fn duplicate_into_predecessors<C, J: Jump>(
    function: &mut MFunction<C>,
    tail: usize,
    max_size: usize,
) -> bool {
    let Some(branches) = analyze(function, tail) else { return false };
    // a block that goes one way is never copied
    if branches.cond.is_none() {
        return false;
    }
    let id = function.blocks[tail].id;
    let body = &function.blocks[tail].insts;
    if body.iter().any(|inst| inst.call || inst.side_effects)
        || branches.taken == Some(id)
        || branches.other == Some(id)
    {
        return false;
    }
    let falls_to = (branches.other.is_none() && tail + 1 < function.blocks.len()).then(|| function.blocks[tail + 1].id);
    if branches.other.is_none() && falls_to.is_none() {
        return false;
    }
    // the jump to the block next in layout is no instruction
    let size = body.len();
    if size > max_size {
        return false;
    }
    let cfg = Cfg::new(function);
    let Some(cycle) = cfg.cycle(tail) else { return false };
    let mut copied = false;
    for pred in 0..function.blocks.len() {
        if pred == tail || cfg.cycle(pred) != Some(cycle) {
            continue;
        }
        let Some(from) = analyze(function, pred) else { continue };
        if from.cond.is_some() || from.other.is_some() || from.taken != Some(id) {
            continue;
        }
        let ends_in_jump = function.blocks[pred].insts.last().is_some_and(|inst| inst.terminator && inst.barrier);
        if !ends_in_jump {
            continue;
        }
        let mut insts: Vec<Inst> = function.blocks[tail].insts.clone();
        if let Some(next) = falls_to {
            insts.push(J::jump(next));
        }
        let block = &mut function.blocks[pred].insts;
        block.pop();
        block.extend(insts);
        copied = true;
    }
    copied
}
