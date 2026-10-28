//! Which register lanes are dead after each instruction of a block, from the
//! lanes dead at its exit.
//!
//! # How it works
//! One walk from the last instruction to the first. The lanes dead after an
//! instruction are those dead before the one that follows it: what that
//! instruction leaves unread and unwritten-before-read. `_unread_after` keeps
//! the high half of a register a call may clobber (`clobbers_high`) as read.
//!
//! # Example
//! `mov ax, 1; mov bx, ax` with `ax` dead at the exit: after the second
//! instruction `ax` and `bx` are dead; after the first only `bx` is.
//!
//! # Used by
//! `peephole`, `exactaddress`, `sharedstores`, `fixupleas`, `fixupbw`
//! and `parcopy`.

use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::liveness;
use crate::backend::peephole::{DeadAfter, Lanes, id};
use crate::model::lir::{Insn, LirBlock};

/// Per instruction, the register lanes dead once it has run.
pub fn _dead_after(
    regs: Regs,
    bits: u32,
    block: &LirBlock,
    dead: Lanes,
) -> DeadAfter {
    let by_position = _dead_after_by(regs, bits, &block.insns, dead);
    block.insns.iter().zip(by_position).map(|(one, dead)| (id(one), dead)).collect()
}

/// `_dead_after`, but a call's possible clobber of an upper half
/// (`clobbers_high`) is no write: the lanes no instruction after reads.
pub fn _unread_after(
    regs: Regs,
    bits: u32,
    block: &LirBlock,
    dead: Lanes,
) -> DeadAfter {
    let by_position = _dead_after_in(regs, bits, &block.insns, dead, true);
    block.insns.iter().zip(by_position).map(|(one, dead)| (id(one), dead)).collect()
}

/// `_dead_after` by position.
fn _dead_after_by(
    regs: Regs,
    bits: u32,
    insns: &[Arc<Insn>],
    dead: Lanes,
) -> Vec<Lanes> {
    _dead_after_in(regs, bits, insns, dead, false)
}

fn _dead_after_in(
    regs: Regs,
    bits: u32,
    insns: &[Arc<Insn>],
    dead: Lanes,
    keeping_high: bool,
) -> Vec<Lanes> {
    let mut dead = dead;
    let mut out = vec![Lanes::new(); insns.len()];
    for (position, one) in insns.iter().enumerate().rev() {
        out[position] = dead;
        dead = liveness::with_effect(regs, bits, one, |effect| {
            effect.map_or_else(Lanes::new, |effect| {
                if keeping_high { effect.keeping_high(regs, one).dead_before(&dead) } else { effect.dead_before(&dead) }
            })
        });
    }
    out
}
