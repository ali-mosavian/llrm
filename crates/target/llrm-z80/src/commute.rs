//! A commutable two-address instruction ties its destination to the source
//! that stays live, so the one that dies there takes the tied place instead.
//!
//! # How it works
//! `add hl, rr` takes its first source in `hl`: the selector copies the value
//! there. When that value is read again later and the other source is made
//! just for this instruction and read by nothing else, the sources swap: the
//! dying value is the one copied into `hl`, the live one stays where it is.
//!
//! # Example
//! `x - 197 <u 22` with `x` used after it: `ld bc,#K; add hl,bc` needs a copy
//! of `x` beside `bc`, and with `de` taken that is a frame slot. Swapped:
//! `ld c,l; ld b,h; ld hl,#K; add hl,bc`, two pairs and no frame.
//!
//! Translated from `TwoAddressInstructionPass::tryInstructionCommute`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp)):
//! commute when the tied source is not killed and the other one is.
//!
//! Not when a call follows in the block: it takes every register, so the
//! live value is in the frame either way and the copy is wasted.
//!
//! Linear in the instructions per iteration of the liveness fixed point.
//!
//! DEVIATE Z80-COMMUTE: the shared `two_address` does not commute; the pin's
//! is run before it, here. owner: llrm-z80-c, exit: 2026-11-30, or when the
//! shared pass commutes.
//!
//! DEVIATE Z80-COMMUTE-MADE: the pin commutes when the tied source stays live
//! and the other dies (`tryInstructionCommute`, TwoAddressInstructionPass.cpp
//! L1312-1313) and has no test on how the dying source is made; its copies
//! are joined later by the register coalescer. This pass runs before
//! allocation, so it asks that an instruction makes the dying source (a copy
//! of a live value would move into `hl` where the unswapped form needs none:
//! phi_slot_unknown_arm +5 B / +3892 T without it). owner: llrm-z80-c, exit:
//! 2026-11-30, or when the allocator's coalescing sees the copy.

use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::{Kind, Operand, Reg};
use llrm_support::hash::{HashMap, HashSet};

use crate::mfunc::MFunction;

/// The virtual registers live out of each block, to a fixed point.
fn live_out(
    function: &MFunction,
    dead: &dyn Fn(&llrm_spike_mf::Inst) -> bool,
) -> Vec<HashSet<u32>> {
    let blocks = function.blocks.len();
    let (mut reads, mut writes) = (vec![HashSet::default(); blocks], vec![HashSet::default(); blocks]);
    for (at, block) in function.blocks.iter().enumerate() {
        for inst in block.insts.iter().filter(|inst| !dead(inst)) {
            for operand in &inst.operands {
                match operand {
                    Operand::Use(Reg::Virt(v)) if !writes[at].contains(v) => {
                        reads[at].insert(*v);
                    }
                    Operand::Def(Reg::Virt(v)) => {
                        writes[at].insert(*v);
                    }
                    _ => {}
                }
            }
        }
    }
    let succs = successors(function);
    let (mut live_in, mut live_out) = (reads.clone(), vec![HashSet::default(); blocks]);
    loop {
        let mut changed = false;
        for at in (0..blocks).rev() {
            let out: HashSet<u32> = succs[at].iter().flat_map(|next| live_in[*next].iter().copied()).collect();
            let into: HashSet<u32> =
                reads[at].iter().copied().chain(out.iter().copied().filter(|v| !writes[at].contains(v))).collect();
            changed |= out != live_out[at] || into != live_in[at];
            live_out[at] = out;
            live_in[at] = into;
        }
        if !changed {
            return live_out;
        }
    }
}

pub fn commute_to_the_dying_source(function: &mut MFunction) {
    // function-wide reads of each virtual register, and its definitions
    let mut reads: HashMap<u32, usize> = HashMap::default();
    let mut defs: HashMap<u32, usize> = HashMap::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            match operand {
                Operand::Use(Reg::Virt(v)) => *reads.entry(*v).or_default() += 1,
                Operand::Def(Reg::Virt(v)) => *defs.entry(*v).or_default() += 1,
                _ => {}
            }
        }
    }
    // a copy nothing reads is no read of its source
    let dead = |inst: &llrm_spike_mf::Inst| {
        matches!(
            &inst.operands[..],
            [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(_))] if inst.kind == Kind::Copy && !reads.contains_key(d)
        )
    };
    let live_out = live_out(function, &dead);
    let reads_in = |insts: &[llrm_spike_mf::Inst], v: u32| {
        insts
            .iter()
            .filter(|inst| !dead(inst))
            .flat_map(|inst| &inst.operands)
            .filter(|o| matches!(o, Operand::Use(Reg::Virt(w)) if *w == v))
            .count()
    };
    for (b, block) in function.blocks.iter_mut().enumerate() {
        for at in 0..block.insts.len() {
            let inst = &block.insts[at];
            if !inst.commutable {
                continue;
            }
            // `add hl, other`: the register it takes its first source in, and
            // the one virtual source
            let phys = inst
                .operands
                .iter()
                .find_map(
                    |o| match o {
                        Operand::Use(Reg::Phys(p)) => Some(*p),
                        _ => None,
                    },
                );
            let others: Vec<usize> =
                (0..inst.operands.len()).filter(|&n| matches!(inst.operands[n], Operand::Use(Reg::Virt(_)))).collect();
            let (Some(phys), [other]) = (phys, &others[..]) else { continue };
            let Operand::Use(Reg::Virt(dying)) = inst.operands[*other] else { continue };
            // the copy that put the first source in that register
            let Some(copy) = (0..at).rev().find(|&n| {
                matches!(
                    &block.insts[n].operands[..],
                    [Operand::Def(Reg::Phys(d)), Operand::Use(Reg::Virt(_))] if *d == phys
                )
            }) else {
                continue;
            };
            let Operand::Use(Reg::Virt(kept)) = block.insts[copy].operands[1] else { continue };
            if block.insts[copy].kind != Kind::Copy
                || kept == dying
                || function.classes[kept as usize] != function.classes[dying as usize]
            {
                continue;
            }
            // the other source dies here; the first is read again later in the
            // block or elsewhere (made in this block: a constant
            // hoisted out of a loop is read every trip)
            // by an instruction that can make it in `hl`: a copy of a live
            // value would move into `hl` where the unswapped form needs none
            let made_here = block.insts[..at]
                .iter()
                .find(|i| i.operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(w)) if *w == dying)))
                .is_some_and(|i| i.kind != Kind::Copy);
            let dies = made_here && reads.get(&dying) == Some(&1) && defs.get(&dying) == Some(&1);
            let after = reads_in(&block.insts[at + 1..], kept) > 0 || live_out[b].contains(&kept);
            // a call after it takes every register: the first source is in the
            // frame by then, and the copy is wasted
            let across_call = block.insts[at + 1..].iter().any(|i| i.call);
            if dies && after && !across_call {
                block.insts[copy].operands[1] = Operand::Use(Reg::Virt(dying));
                block.insts[at].operands[*other] = Operand::Use(Reg::Virt(kept));
            }
        }
    }
}
