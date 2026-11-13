//! `PHIElimination` for wasm MIR without `LiveVariables`, as the pin runs it at
//! `-O0`: each `PHI` becomes a copy of a fresh register into its destination at
//! the top of the block, and a copy of each incoming value into the fresh
//! register before the first terminator of its predecessor.
//!
//! DEVIATE W13: no critical edge splitting and no reuse of the incoming
//! register across the PHIs of a switch's duplicate edges (`LoweredPHIs`,
//! `AllEdgesCritical`). Reason: both are guarded by `LiveVariables` or
//! `LiveIntervals`, which the `-O0` pipeline does not compute. Exit: with a
//! register allocator (undated: wasm colours registers after the stackifier).
//!
//! # See also
//! `PHIElimination.cpp`, pin d1106deb71cc: `LowerPHINode`,
//! `findPHICopyInsertPoint`.

use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, IMPLICIT_DEF, Inst, Operand, PHI};

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
    // `SrcUndef`: a PHI operand that only an `IMPLICIT_DEF` defines gets no
    // copy; the incoming register is itself an `IMPLICIT_DEF`, which the
    // coalescer erases where a join overlaps it.
    let undef_sources: std::collections::HashSet<u32> = f
        .blocks
        .iter()
        .flatten()
        .filter(|inst| inst.opcode == IMPLICIT_DEF)
        .filter_map(|inst| match inst.operands.first() {
            Some(Operand::Def(r)) => Some(*r),
            _ => None,
        })
        .collect();
    let mut undef_used = false;
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
                let made = if undef_sources.contains(&src) {
                    undef_used = true;
                    Inst::new(IMPLICIT_DEF, &[Operand::Def(incoming)])
                } else {
                    Inst::new(ty.copy(), &[Operand::Def(incoming), Operand::Use(src)])
                };
                before_terminators[pred as usize].push(made);
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
    if undef_used {
        // The `IMPLICIT_DEF`s nothing reads any more.
        let read: std::collections::HashSet<u32> = f
            .blocks
            .iter()
            .flatten()
            .flat_map(|inst| inst.operands.iter())
            .filter_map(|operand| match operand {
                Operand::Use(r) => Some(*r),
                _ => None,
            })
            .collect();
        for block in &mut f.blocks {
            block.retain(|inst| {
                inst.opcode != IMPLICIT_DEF
                    || !matches!(
                        inst.operands.first(),
                        Some(Operand::Def(r)) if undef_sources.contains(r) && !read.contains(r)
                    )
            });
        }
    }
    changed
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
    let (live_in, _) = crate::liveness::liveness(f);
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
                if split && can_split_critical_edge(f, pred) {
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

/// `MachineBasicBlock::canSplitCriticalEdge`: the terminators of `pred` have
/// to be ones `analyzeBranch` understands, so a block ending in `BR_TABLE`
/// keeps its edges whole and the PHI copy goes before the jump. The pin's
/// jump-table path (`findJumpTableIndex`) does not apply: wasm's `BR_TABLE`
/// names its blocks, not a jump table (pin `-debug-only=phi-node-elimination`:
/// "Failed to split critical edge").
pub(crate) fn can_split_critical_edge(
    f: &Function,
    pred: usize,
) -> bool {
    // Both exits to one block is a degenerate input the pin leaves alone.
    crate::tail_merge::analyze_branch(&f.blocks[pred])
        .is_some_and(|exits| exits.taken.is_none() || exits.taken != exits.other)
}

/// A block of one `BR` between `pred` and `block`, laid out after `pred`.
pub(crate) fn split_edge(
    f: &mut Function,
    pred: usize,
    block: usize,
) {
    let new = pred + 1;
    // `pred` may fall into the block laid out after it: the new block now
    // stands there, so the fall-through needs its own jump (the block moves
    // up by one).
    let falls_into =
        f.blocks[pred].last().is_some_and(|last| !last.is_barrier()) && pred + 1 < f.blocks.len() && pred + 1 != block;
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
    if falls_into {
        f.blocks[pred as usize]
            .push(Inst::new(crate::dagisel::generated::ids::opcode::BR, &[Operand::Block(new_id + 1)]));
    }
    f.blocks.insert(new, vec![Inst::new(crate::dagisel::generated::ids::opcode::BR, &[Operand::Block(target)])]);
    f.succs.insert(new, vec![block]);
    if !f.edge_probs.is_empty() {
        f.edge_probs.insert(new, vec![1 << 31]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::{ValType, is_copy};

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

    /// bb0 ends in `BR_IF bb2` and falls into bb1: splitting the edge to bb2
    /// puts the new block where bb1 was, so bb0 needs its own `BR` to bb1. It
    /// had none and fell into the new block: every input took the split edge
    /// (wasm switch_peeled_table -Os returned its argument for all inputs).
    #[test]
    fn a_split_after_a_falling_through_branch_keeps_the_fall_through() {
        let mut f = function();
        f.blocks[0].pop();
        assert!(split_phi_edges(&mut f));
        assert_eq!(f.blocks[0].last(), Some(&Inst::new(op::BR, &[Operand::Block(2)])), "{:?}", f.blocks[0]);
        assert_eq!(f.blocks[0][2].operands[0], Operand::Block(1));
    }

    /// bb0 ends in a `BR_TABLE`, which `analyzeBranch` cannot read, so the pin
    /// refuses to split its edge: the PHI copy goes in bb0. We split anyway
    /// and left an empty block on the switch's first case (wasm
    /// div_constants -O1 +7 B per function).
    #[test]
    fn an_edge_out_of_a_br_table_is_not_split() {
        let mut f = function();
        let last = f.blocks[0].len() - 2;
        f.blocks[0].truncate(last);
        f.blocks[0].push(Inst::new(op::BR_TABLE_I32, &[Operand::Use(1), Operand::Block(2), Operand::Block(1)]));
        assert!(!split_phi_edges(&mut f));
        assert_eq!(f.blocks.len(), 3);
    }

    /// A branch-free function has no edge to split.
    #[test]
    fn a_function_with_no_critical_edge_is_left_alone() {
        let mut f = function();
        f.succs[0] = vec![1];
        f.blocks[0].remove(2);
        assert!(!split_phi_edges(&mut f));
    }
    /// `%p = phi [poison, bb0], [%x, bb1]`: bb0's copy was a `COPY` of a shared
    /// `IMPLICIT_DEF` register, which the coalescer must treat as a real value;
    /// the prolog of a runtime unroll left 14 of them in a loop's exit edge
    /// (wasm x87_arguments +176 B over the pin). The pin writes an
    /// `IMPLICIT_DEF` of the incoming register (`SrcUndef`).
    #[test]
    fn a_poison_operand_is_an_implicit_def_of_the_incoming_register_not_a_copy() {
        let mut f = Function::default();
        let (undef, x, p) = (0, 1, 2);
        for _ in 0..3 {
            f.new_vreg(ValType::I32);
        }
        let def = Operand::Def;
        let (reg, block) = (Operand::Use, |b| Operand::Block(b));
        f.blocks = vec![
            vec![Inst::new(IMPLICIT_DEF, &[def(undef)]), Inst::new(op::BR_IF, &[block(2), reg(undef)])],
            vec![Inst::new(op::CONST_I32, &[def(x), Operand::Imm(7)]), Inst::new(op::BR, &[block(2)])],
            vec![Inst::new(PHI, &[def(p), reg(undef), block(0), reg(x), block(1)]), Inst::new(op::RETURN, &[reg(p)])],
        ];
        f.succs = vec![vec![2, 1], vec![2], vec![]];
        assert!(phi_elimination(&mut f));
        let copies = f.blocks.iter().flatten().filter(|inst| is_copy(inst.opcode)).count();
        let implicit = f.blocks[0].iter().filter(|inst| inst.opcode == IMPLICIT_DEF).count();
        // bb0 keeps its one read of `%undef` (the branch); the PHI's operand is
        // a def.
        assert_eq!((copies, implicit), (2, 2), "{:?}", f.blocks);
    }
}
