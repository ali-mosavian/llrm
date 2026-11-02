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

use std::collections::HashSet;

use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand, PHI, VReg};

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

/// `LiveVariables`' liveness per block: `(live-in, live-out)`. A PHI's use is
/// in its predecessor, so a register only a PHI reads is not live out of that
/// predecessor, and is live into it when defined before.
fn liveness(f: &Function) -> (Vec<HashSet<VReg>>, Vec<HashSet<VReg>>) {
    let n = f.blocks.len();
    let (mut used, mut defined) = (vec![HashSet::new(); n], vec![HashSet::new(); n]);
    for (b, block) in f.blocks.iter().enumerate() {
        for inst in block {
            if inst.opcode != PHI {
                for operand in &inst.operands {
                    if let Operand::Use(r) = operand
                        && !defined[b].contains(r)
                    {
                        used[b].insert(*r);
                    }
                }
            }
            for operand in &inst.operands {
                if let Operand::Def(r) = operand {
                    defined[b].insert(*r);
                }
            }
        }
    }
    // A PHI's use is at the end of its predecessor.
    for block in &f.blocks {
        for phi in block.iter().take_while(|inst| inst.opcode == PHI) {
            for pair in phi.operands[1..].chunks(2) {
                if let [Operand::Use(r), Operand::Block(pred)] = *pair
                    && !defined[pred as usize].contains(&r)
                {
                    used[pred as usize].insert(r);
                }
            }
        }
    }
    let (mut live_in, mut live_out) = (used.clone(), vec![HashSet::new(); n]);
    loop {
        let mut changed = false;
        for b in (0..n).rev() {
            let out: HashSet<VReg> = f.succs[b].iter().flat_map(|&s| live_in[s].iter().copied()).collect();
            let inn: HashSet<VReg> =
                used[b].iter().copied().chain(out.iter().filter(|r| !defined[b].contains(r)).copied()).collect();
            if out != live_out[b] || inn != live_in[b] {
                live_out[b] = out;
                live_in[b] = inn;
                changed = true;
            }
        }
        if !changed {
            return (live_in, live_out);
        }
    }
}

/// `PHIEliminationImpl::SplitPHIEdges`, run where `LiveVariables` exists
/// (every level but `-O0`): a critical edge into a PHI whose incoming
/// register stays live in the predecessor is split, so the copy the PHI
/// needs sits on the edge and the coalescer can join it. Back edges of a loop
/// are kept whole, to leave the loop free of out-of-line blocks.
///
/// DEVIATE: the edges are chosen on the function as it is before any split,
/// and split afterwards; the pin decides one block at a time on the function
/// so far. Reason: a split adds an empty block, which changes no register's
/// liveness, so the choices are the same. Exit: none.
///
/// Whether anything changed.
pub fn split_phi_edges(f: &mut Function) -> bool {
    let (_, loops) = crate::cfg::Cfg::of(f).analyses();
    let (live_in, _) = liveness(f);
    let id = |b: usize| llrm_mir::module::BlockId(b as u32);
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for block in 0..f.blocks.len() {
        if f.is_eh_pad(block) {
            continue;
        }
        for phi in f.blocks[block].iter().take_while(|inst| inst.opcode == PHI) {
            for pair in phi.operands[1..].chunks(2) {
                let [Operand::Use(reg), Operand::Block(pred)] = *pair else { continue };
                let pred = pred as usize;
                let mut distinct = f.succs[pred].clone();
                distinct.sort_unstable();
                distinct.dedup();
                if distinct.len() == 1 || pred == block || edges.contains(&(pred, block)) {
                    continue;
                }
                let (cur, pre) = (loops.get_loop_for(id(block)), loops.get_loop_for(id(pred)));
                if cur.is_some_and(|cur| cur.get_header() == id(block) && pre == Some(cur)) {
                    continue;
                }
                // Live past the PHIs in the predecessor: some successor reads
                // it.
                let mut split = f.succs[pred].iter().any(|&s| live_in[s].contains(&reg));
                if !split {
                    continue;
                }
                // Not live into the block means live into another successor,
                // and the split avoids the interference.
                split = !live_in[block].contains(&reg);
                if !split && cur != pre {
                    // An edge entering a loop from outside is kept.
                    split = pre.is_some_and(|pre| !pre.contains_loop(cur));
                }
                if split {
                    edges.push((pred, block));
                }
            }
        }
    }
    // From the last predecessor back, so the blocks still to split keep their
    // numbers; a target after a split block moves up by one.
    edges.sort_unstable_by(|a, b| b.cmp(a));
    for at in 0..edges.len() {
        let (pred, block) = edges[at];
        split_edge(f, pred, block);
        for later in &mut edges[at + 1..] {
            if later.1 > pred {
                later.1 += 1;
            }
        }
    }
    !edges.is_empty()
}

/// A block of one `BR` between `pred` and `block`, laid out after `pred`.
fn split_edge(
    f: &mut Function,
    pred: usize,
    block: usize,
) {
    let new = pred + 1;
    let shift = |b: &mut u32| {
        if *b as usize >= new {
            *b += 1;
        }
    };
    for operand in f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
        if let Operand::Block(b) = operand {
            shift(b);
        }
    }
    for to in f.succs.iter_mut().flatten() {
        if *to >= new {
            *to += 1;
        }
    }
    let (old, block) = (block, if block >= new { block + 1 } else { block });
    let pred = pred as u32;
    let (target, new_id) = (block as u32, new as u32);
    for inst in f.blocks[pred as usize].iter_mut().filter(|inst| inst.opcode != PHI) {
        if is_terminator(inst) {
            for operand in &mut inst.operands {
                if *operand == Operand::Block(target) {
                    *operand = Operand::Block(new_id);
                }
            }
        }
    }
    for inst in f.blocks[old].iter_mut().take_while(|inst| inst.opcode == PHI) {
        for pair in inst.operands[1..].chunks_mut(2) {
            if pair[1] == Operand::Block(pred) {
                pair[1] = Operand::Block(new_id);
            }
        }
    }
    for to in &mut f.succs[pred as usize] {
        if *to == block {
            *to = new;
        }
    }
    f.blocks.insert(new, vec![Inst::new(crate::dagisel::generated::ids::opcode::BR, &[Operand::Block(target)])]);
    f.succs.insert(new, vec![block]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::ValType;

    /// bb0 -> {bb2, bb1}, bb1 -> bb2, with `%a` read in bb1 and by the PHI of
    /// bb2: the edge bb0 -> bb2 is critical and `%a` stays live past the PHI.
    fn function() -> Function {
        let mut f = Function::default();
        let (a, c, b, p) = (0, 1, 2, 3);
        for _ in 0..4 {
            f.new_vreg(ValType::I32);
        }
        let def = Operand::Def;
        let (reg, block) = (Operand::Use, |b| Operand::Block(b));
        f.blocks = vec![
            vec![
                Inst::new(op::CONST_I32, &[def(a), Operand::Imm(1)]),
                Inst::new(op::CONST_I32, &[def(c), Operand::Imm(0)]),
                Inst::new(op::BR_IF, &[block(2), reg(c)]),
                Inst::new(op::BR, &[block(1)]),
            ],
            vec![Inst::new(op::ADD_I32, &[def(b), reg(a), reg(a)]), Inst::new(op::BR, &[block(2)])],
            vec![Inst::new(PHI, &[def(p), reg(a), block(0), reg(b), block(1)]), Inst::new(op::RETURN, &[reg(p)])],
        ];
        f.succs = vec![vec![2, 1], vec![2], vec![]];
        f
    }

    /// The copy for the PHI went before the branch in bb0, so the path through
    /// bb1 paid a copy of `%a` it never reads.
    #[test]
    fn a_critical_edge_whose_register_stays_live_is_split() {
        let mut f = function();
        assert!(split_phi_edges(&mut f));
        assert_eq!(f.blocks.len(), 4);
        // The new block follows bb0, holds a branch to the PHI's block, and
        // is where the PHI now reads `%a` from.
        assert_eq!(f.blocks[1], vec![Inst::new(op::BR, &[Operand::Block(3)])]);
        assert_eq!(f.succs[0], vec![1, 2]);
        assert!(f.blocks[3][0].operands.contains(&Operand::Block(1)), "{:?}", f.blocks[3][0]);
        assert_eq!(f.blocks[0][2].operands[0], Operand::Block(1));
    }

    /// A branch-free function has no edge to split.
    #[test]
    fn a_function_with_no_critical_edge_is_left_alone() {
        let mut f = function();
        f.succs[0] = vec![1];
        f.blocks[0].remove(2);
        assert!(!split_phi_edges(&mut f));
    }
}
