//! `PHIElimination` for wasm MIR without `LiveVariables`, as the pin runs it at
//! `-O0`: each `PHI` becomes a copy of a fresh register into its destination at
//! the top of the block, and a copy of each incoming value into the fresh
//! register before the first terminator of its predecessor.
//!
//! DEVIATE W13: no critical edge splitting and no reuse of the incoming
//! register across the PHIs of a switch's duplicate edges (`LoweredPHIs`,
//! `AllEdgesCritical`). Reason: both are guarded by `LiveVariables` or
//! `LiveIntervals`, which the `-O0` pipeline does not compute. Exit: with a
//! register allocator.
//!
//! # See also
//! `PHIElimination.cpp`, pin d1106deb71cc: `LowerPHINode`,
//! `findPHICopyInsertPoint`.

use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand, PHI};

/// `isTerminator`.
fn is_terminator(inst: &Inst) -> bool {
    desc(inst.opcode).is_some_and(|desc| desc.has(flag::TERMINATOR))
}

/// Whether anything changed.
pub fn phi_elimination(f: &mut Function) -> bool {
    // Copies into the incoming register, per predecessor, in the order the PHIs
    // were lowered.
    let mut before_terminators: Vec<Vec<Inst>> = vec![Vec::new(); f.blocks.len()];
    let mut changed = false;
    for b in 0..f.blocks.len() {
        let phis = f.blocks[b].iter().take_while(|inst| inst.opcode == PHI).count();
        if phis == 0 {
            continue;
        }
        changed = true;
        let mut copies = Vec::with_capacity(phis);
        let lowered: Vec<Inst> = f.blocks[b][..phis].to_vec();
        for inst in &lowered {
            let Operand::Def(dest) = inst.operands[0] else { unreachable!("a PHI defines its result first") };
            let ty = f.vreg_type(dest);
            let incoming = f.new_vreg(ty);
            copies.push(Inst::new(ty.copy(), &[Operand::Def(dest), Operand::Use(incoming)]));
            let mut done: Vec<u32> = Vec::new();
            for pair in inst.operands[1..].chunks(2).rev() {
                let [Operand::Use(src), Operand::Block(pred)] = *pair else {
                    unreachable!("PHI pairs a use and a block")
                };
                if done.contains(&pred) {
                    continue;
                }
                done.push(pred);
                before_terminators[pred as usize]
                    .push(Inst::new(ty.copy(), &[Operand::Def(incoming), Operand::Use(src)]));
            }
        }
        // Each PHI's copy goes in before the first non-PHI, ahead of the copies
        // already placed.
        copies.reverse();
        f.blocks[b].splice(..phis, copies);
    }
    for (b, copies) in before_terminators.into_iter().enumerate() {
        if copies.is_empty() {
            continue;
        }
        let end = f.blocks[b].iter().rposition(|inst| !is_terminator(inst)).map_or(0, |last| last + 1);
        f.blocks[b].splice(end..end, copies);
    }
    changed
}
