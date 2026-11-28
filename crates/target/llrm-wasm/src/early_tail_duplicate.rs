//! `EarlyTailDuplicate`, the pass before the machine SSA optimizations
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TailDuplicator.cpp#L274)),
//! `TailDuplicator::tailDuplicateBlocks` with `PreRegAlloc` set: a small block
//! that cannot fall through is copied into each predecessor that branches to
//! it alone, and the blocks after it read the copies.
//!
//! A block of one `br` is dropped from the path of each predecessor, which
//! branches to its successor instead (`duplicateSimpleBB`): the IR's loop
//! preheader, kept apart by `CodeGenPrepare`, is such a block. Any other block
//! of at most `TailDupSize` instructions (two, one at -Os) is copied
//! whole when every predecessor can take it (`canCompletelyDuplicateBB`): a PHI
//! of the block is what it reads from that predecessor, a value it defines is
//! new in each copy, the PHIs of its successors read the copies
//! (`updateSuccessorsPHIs`), and a value read outside the block gets a PHI
//! where the copies meet (`MachineSSAUpdater`). An interpreter's loop latch
//! (`pc += 4` and the jump back, joined by three PHIs) is copied into every
//! handler this way; left shared, each handler pays a branch to it.
//!
//! DEVIATE 2: `canTailDuplicate` is `analyzeBranch` of the predecessor with no
//! condition; the pin's `LayoutMode`, indirect-branch
//! (`TailDupIndirectBranchSize` applies to a block ending in `br_table` only if
//! the instruction table marks it indirect) and profile paths are the post-RA
//! `tail_duplicate`'s. Exit: the shared tail duplicator over the neutral
//! machine function (S4); the LIR copy is `llrm-codegen`'s `earlytaildup`.
//!
//! # See also
//! `TailDuplicator::{tailDuplicateBlocks, tailDuplicateAndUpdate,
//! tailDuplicate, shouldTailDuplicate, canCompletelyDuplicateBB, processPHI,
//! duplicateInstruction, updateSuccessorsPHIs, isSimpleBB, duplicateSimpleBB}`,
//! pin d1106deb71cc.

use llrm_support::hash::{HashMap, HashSet};

use crate::branch_fold::{ending, set_branches};
use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand, PHI, VReg};

/// `isSimpleBB` and `shouldTailDuplicate` for a block that does not fall
/// through: it holds one unconditional branch.
fn is_simple(
    f: &Function,
    tail: usize,
    has_preds: bool,
) -> bool {
    has_preds
        && f.succs[tail].len() == 1
        && f.succs[tail][0] != tail
        && matches!(f.blocks[tail].as_slice(), [only] if only.opcode == op::BR)
}

/// `bothUsedInPHI`: `pred` and the tail share a successor that starts with a
/// PHI, which would need two entries for `pred`.
fn both_used_in_phi(
    f: &Function,
    pred: usize,
    tail: usize,
) -> bool {
    f.succs[pred].iter().any(|s| f.succs[tail].contains(s) && f.blocks[*s].first().is_some_and(|i| i.opcode == PHI))
}

/// `replaceSuccessor`, or `removeSuccessor` when `pred` already reaches `to`.
fn replace_successor(
    f: &mut Function,
    pred: usize,
    from: usize,
    to: usize,
) {
    let at = f.succs[pred].iter().position(|&s| s == from).expect("an edge to the tail");
    if !f.succs[pred].contains(&to) {
        f.succs[pred][at] = to;
        return;
    }
    f.succs[pred].remove(at);
    if let Some(probs) = f.edge_probs.get_mut(pred).filter(|p| p.len() > at) {
        probs.remove(at);
        let sum: u64 = probs.iter().map(|&p| u64::from(p)).sum();
        if sum != 0 {
            for p in probs.iter_mut() {
                *p = ((u64::from(*p) << 31) / sum) as u32;
            }
        }
    }
}

/// `updateSuccessorsPHIs`: each PHI of `succ` takes the tail's value from
/// every block that now jumps to `succ` directly.
fn update_phis(
    f: &mut Function,
    succ: usize,
    tail: usize,
    dead: bool,
    preds: &[usize],
) {
    for phi in f.blocks[succ].iter_mut().take_while(|inst| inst.opcode == PHI) {
        let at = (1..phi.operands.len())
            .step_by(2)
            .find(|&i| phi.operands[i + 1] == Operand::Block(tail as u32))
            .expect("a PHI entry for the tail");
        let value = phi.operands[at];
        if dead {
            phi.operands.drain(at..at + 2);
        }
        for &pred in preds {
            phi.operands.push(value);
            phi.operands.push(Operand::Block(pred as u32));
        }
    }
}

/// Whether anything changed. The caller removes the blocks left without a
/// predecessor. `for_size`: -Os or -Oz (`shouldOptimizeForSize`).
pub fn early_tail_duplicate(
    f: &mut Function,
    for_size: bool,
) -> bool {
    let mut preds = predecessors(f);
    let mut changed = false;
    for tail in 1..f.blocks.len() {
        if !is_simple(f, tail, !preds[tail].is_empty()) {
            if should_tail_duplicate(f, &preds, tail, for_size) && tail_duplicate_and_update(f, &preds, tail) {
                changed = true;
                preds = predecessors(f);
            }
            continue;
        }
        let target = f.succs[tail][0];
        let mut duplicated = Vec::new();
        for pred in preds[tail].clone() {
            if f.succs[pred].iter().any(|&s| f.is_eh_pad(s)) || both_used_in_phi(f, pred, tail) {
                continue;
            }
            let Some(end) = ending(f, pred) else { continue };
            let next = (pred + 1 < f.blocks.len()).then_some(pred + 1);
            let (mut taken, mut other, mut cond) = (end.taken, end.other, end.cond);
            if cond.is_none() {
                other = taken;
            }
            taken = taken.or(next);
            other = other.or(next);
            let redirect = |b: Option<usize>| if b == Some(tail) { Some(target) } else { b };
            (taken, other) = (redirect(taken), redirect(other));
            if taken == other {
                cond = None;
                other = None;
            }
            if other == next {
                other = None;
            }
            if taken == next && other.is_none() {
                taken = None;
            }
            set_branches(f, pred, taken, other, cond.as_ref());
            replace_successor(f, pred, tail, target);
            preds[tail].retain(|&p| p != pred);
            if !preds[target].contains(&pred) {
                preds[target].push(pred);
            }
            duplicated.push(pred);
        }
        if duplicated.is_empty() {
            continue;
        }
        let dead = preds[tail].is_empty();
        update_phis(f, target, tail, dead, &duplicated);
        if dead {
            f.blocks[tail].clear();
            f.succs[tail].clear();
            if let Some(probs) = f.edge_probs.get_mut(tail) {
                probs.clear();
            }
            preds[target].retain(|&p| p != tail);
        }
        changed = true;
    }
    changed
}

/// The unique predecessors of each block, in the order of `succs`.
fn predecessors(f: &Function) -> Vec<Vec<usize>> {
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); f.blocks.len()];
    for (from, to) in f.succs.iter().enumerate() {
        for &to in to {
            if !preds[to].contains(&from) {
                preds[to].push(from);
            }
        }
    }
    preds
}

fn has(
    inst: &Inst,
    bit: u64,
) -> bool {
    desc(inst.opcode).is_some_and(|d| d.has(bit))
}

/// `MachineBasicBlock::canFallThrough` (`getFallThrough` with
/// `JumpToFallThrough`): the layout successor is a successor and the block
/// does not branch past it; an explicit branch to it counts as reaching it.
fn can_fall_through(
    f: &Function,
    block: usize,
) -> bool {
    let next = block + 1;
    if next >= f.blocks.len() || !f.succs[block].contains(&next) {
        return false;
    }
    let Some(end) = ending(f, block) else {
        return f.blocks[block].last().is_none_or(|last| !has(last, flag::BARRIER));
    };
    if end.taken.is_none() {
        return true;
    }
    if end.taken == Some(next) || end.other == Some(next) {
        return true;
    }
    end.cond.is_some() && end.other.is_none()
}

/// `TailDuplicateSize`, `TailDupIndirectBranchSize`, `TailDupPredSize` and
/// `TailDupSuccSize`.
const TAIL_DUP_SIZE: usize = 2;
const TAIL_DUP_INDIRECT_SIZE: usize = 20;
const TAIL_DUP_PRED_SIZE: usize = 16;
const TAIL_DUP_SUCC_SIZE: usize = 16;

/// What `shouldTailDuplicate` reads of a block, so that the decision is the
/// pin's lines and no machine type.
struct TailFacts {
    can_fall_through: bool,
    self_loop: bool,
    for_size: bool,
    /// The block ends in an indirect branch.
    indirect: bool,
    /// Instructions that are neither PHIs nor meta, the terminators among them.
    instructions: usize,
    phis: usize,
    /// No instruction is a call, a return or not duplicable.
    duplicable: bool,
    preds: usize,
    succs: usize,
    /// A successor starts with a PHI.
    successor_phi: bool,
    /// `canCompletelyDuplicateBB`.
    all_preds_take_it: bool,
}

/// `shouldTailDuplicate` for a block that is not simple, `PreRegAlloc`, no
/// `LayoutMode`.
fn should_duplicate(t: &TailFacts) -> bool {
    if t.can_fall_through || t.self_loop || !t.duplicable {
        return false;
    }
    let mut max = if t.for_size { 1 } else { TAIL_DUP_SIZE };
    if t.indirect {
        max = TAIL_DUP_INDIRECT_SIZE;
    }
    if t.instructions > max {
        return false;
    }
    if t.preds > TAIL_DUP_PRED_SIZE && t.succs > TAIL_DUP_SUCC_SIZE && (t.phis != 0 || t.successor_phi) {
        return false;
    }
    t.indirect || t.all_preds_take_it
}

/// `canTailDuplicate`: the predecessor goes to the tail by one branch and by
/// no other edge, and has no condition.
fn can_tail_duplicate(
    f: &Function,
    pred: usize,
) -> bool {
    f.succs[pred].len() == 1 && ending(f, pred).is_some_and(|end| end.cond.is_none())
}

/// `canCompletelyDuplicateBB`: every predecessor can take a copy.
fn can_completely_duplicate(
    f: &Function,
    preds: &[Vec<usize>],
    tail: usize,
) -> bool {
    preds[tail].iter().all(|&pred| can_tail_duplicate(f, pred))
}

/// The facts of `tail` that `shouldTailDuplicate` reads.
fn should_tail_duplicate(
    f: &Function,
    preds: &[Vec<usize>],
    tail: usize,
    for_size: bool,
) -> bool {
    if preds[tail].is_empty() {
        return false;
    }
    let insts = &f.blocks[tail];
    let counted = insts.iter().filter(|inst| inst.opcode != PHI);
    should_duplicate(&TailFacts {
        can_fall_through: can_fall_through(f, tail),
        self_loop: f.succs[tail].contains(&tail),
        for_size,
        indirect: insts.last().is_some_and(|last| has(last, flag::INDIRECT_BRANCH)),
        instructions: counted.clone().filter(|inst| !has(inst, flag::META)).count(),
        phis: insts.len() - counted.clone().count(),
        duplicable: counted
            .clone()
            .all(|inst| !has(inst, flag::NOT_DUPLICABLE) && !has(inst, flag::RETURN) && !has(inst, flag::CALL)),
        preds: preds[tail].len(),
        succs: f.succs[tail].len(),
        successor_phi: f.succs[tail].iter().any(|&s| f.blocks[s].first().is_some_and(|i| i.opcode == PHI)),
        all_preds_take_it: can_completely_duplicate(f, preds, tail),
    })
}

/// The value a PHI reads from `pred`.
fn incoming(
    phi: &Inst,
    pred: usize,
) -> Option<(usize, VReg)> {
    (1..phi.operands.len())
        .step_by(2)
        .find_map(
            |at| match (phi.operands[at], phi.operands[at + 1]) {
                (Operand::Use(reg), Operand::Block(block)) if block as usize == pred => Some((at, reg)),
                _ => None,
            },
        )
}

/// A register defined in the tail and its value in each copy
/// (`SSAUpdateVals`), in the order the copies were made.
#[derive(Default)]
struct Updates {
    order: Vec<VReg>,
    values: HashMap<VReg, Vec<(usize, VReg)>>,
}

impl Updates {
    /// `addSSAUpdateEntry`.
    fn add(
        &mut self,
        original: VReg,
        copy: VReg,
        block: usize,
    ) {
        if !self.values.contains_key(&original) {
            self.order.push(original);
        }
        self.values.entry(original).or_default().push((block, copy));
    }
}

/// `isDefLiveOut` for every register the tail defines: an instruction
/// outside `block` reads it.
fn live_out_defs(
    f: &Function,
    block: usize,
) -> HashSet<VReg> {
    let defined: HashSet<VReg> = f.blocks[block].iter().flat_map(Inst::defs).collect();
    f.blocks
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != block)
        .flat_map(|(_, insts)| insts.iter())
        .flat_map(|inst| inst.operands.iter())
        .filter_map(
            |operand| if let Operand::Use(reg) = operand { defined.contains(reg).then_some(*reg) } else { None },
        )
        .collect()
}

/// `processPHI` and `duplicateInstruction` over the tail's instructions into
/// `pred`: the PHIs become the value `pred` gives them, every other def a
/// register of its own. `remove`: the PHI loses its entry for `pred`.
struct Copier<'a> {
    tail: usize,
    used_by_phi: &'a HashSet<VReg>,
    live_out: &'a HashSet<VReg>,
    updates: &'a mut Updates,
    copies: &'a mut Vec<(VReg, VReg, usize)>,
}

impl Copier<'_> {
    /// `tailDuplicate`'s loop body: the copy of the tail's instructions in
    /// `pred`; the tail's PHIs lose their entry for `pred`.
    fn copy_into(
        &mut self,
        f: &mut Function,
        pred: usize,
    ) {
        let mut map: HashMap<VReg, VReg> = HashMap::default();
        let mut infos: Vec<(VReg, VReg)> = Vec::new();
        let insts = f.blocks[self.tail].clone();
        let mut kept = Vec::new();
        for inst in insts {
            if inst.opcode == PHI {
                let phi = self.process_phi(f, inst, pred, &mut map, &mut infos, true);
                kept.extend(phi);
                continue;
            }
            kept.push(inst.clone());
            let clone = self.duplicate(f, inst, pred, &mut map);
            let at = f.blocks[pred].len();
            f.blocks[pred].insert(at, clone);
        }
        f.blocks[self.tail] = kept;
        self.append_copies(f, pred, &infos);
    }

    /// `processPHI`: the PHI's def is what `pred` gives it; a COPY of that into
    /// a register of its own is the value live out of the block.
    fn process_phi(
        &mut self,
        f: &mut Function,
        mut phi: Inst,
        pred: usize,
        map: &mut HashMap<VReg, VReg>,
        infos: &mut Vec<(VReg, VReg)>,
        remove: bool,
    ) -> Option<Inst> {
        let Some(Operand::Def(def)) = phi.operands.first().copied() else { return Some(phi) };
        let (at, source) = incoming(&phi, pred).expect("a PHI entry for the predecessor");
        map.insert(def, source);
        let new_def = f.new_vreg(f.vreg_type(def));
        infos.push((new_def, source));
        if !remove {
            phi.operands[at] = Operand::Use(new_def);
            return Some(phi);
        }
        if self.live_out.contains(&def) || self.used_by_phi.contains(&def) {
            self.updates.add(def, new_def, pred);
        }
        phi.operands.drain(at..at + 2);
        (phi.operands.len() > 1).then_some(phi)
    }

    /// `duplicateInstruction`: a def is a new register, a use of a PHI's def or
    /// of an earlier def reads what replaced it.
    fn duplicate(
        &mut self,
        f: &mut Function,
        inst: Inst,
        pred: usize,
        map: &mut HashMap<VReg, VReg>,
    ) -> Inst {
        let mut copy = inst;
        for operand in &mut copy.operands {
            match *operand {
                Operand::Def(reg) => {
                    let new = f.new_vreg(f.vreg_type(reg));
                    *operand = Operand::Def(new);
                    map.insert(reg, new);
                    if self.live_out.contains(&reg) || self.used_by_phi.contains(&reg) {
                        self.updates.add(reg, new, pred);
                    }
                }
                Operand::Use(reg) => {
                    if let Some(&mapped) = map.get(&reg) {
                        *operand = Operand::Use(mapped);
                    }
                }
                _ => {}
            }
        }
        copy
    }

    /// `appendCopies`: the COPYs go before the block's terminators.
    fn append_copies(
        &mut self,
        f: &mut Function,
        block: usize,
        infos: &[(VReg, VReg)],
    ) {
        let first_terminator = f.blocks[block].iter().position(Inst::is_terminator).unwrap_or(f.blocks[block].len());
        for (at, &(new, source)) in infos.iter().enumerate() {
            let copy = f.vreg_type(new).copy();
            f.blocks[block].insert(first_terminator + at, Inst::new(copy, &[Operand::Def(new), Operand::Use(source)]));
            self.copies.push((new, source, block));
        }
    }
}

/// `tailDuplicateAndUpdate` and `tailDuplicate`.
fn tail_duplicate_and_update(
    f: &mut Function,
    preds: &[Vec<usize>],
    tail: usize,
) -> bool {
    let successors = f.succs[tail].clone();
    let used_by_phi: HashSet<VReg> = f.blocks[tail]
        .iter()
        .take_while(|inst| inst.opcode == PHI)
        .flat_map(|phi| {
            phi.operands[1..].chunks(2).filter_map(|pair| if let Operand::Use(r) = pair[0] { Some(r) } else { None })
        })
        .collect();
    let live_out = live_out_defs(f, tail);
    let mut updates = Updates::default();
    let mut copies: Vec<(VReg, VReg, usize)> = Vec::new();
    let mut done: Vec<usize> = Vec::new();
    for &pred in &preds[tail] {
        if !can_tail_duplicate(f, pred) || (pred + 1 == tail && can_fall_through(f, pred)) {
            continue;
        }
        set_branches(f, pred, None, None, None);
        Copier { tail, used_by_phi: &used_by_phi, live_out: &live_out, updates: &mut updates, copies: &mut copies }
            .copy_into(f, pred);
        f.succs[pred] = f.succs[tail].clone();
        f.edge_probs.resize_with(f.blocks.len(), Vec::new);
        f.edge_probs[pred] = f.edge_probs[tail].clone();
        done.push(pred);
    }
    // The tail duplicated into all its predecessors but the one that falls
    // into it: that one takes the tail's contents (the pin's merge into the
    // previous block).
    let mut remaining: Vec<usize> = predecessors(f)[tail].clone();
    if let Some(prev) = tail.checked_sub(1) {
        let merge = f.succs[prev] == [tail]
            && remaining == [prev]
            && ending(f, prev)
                .is_some_and(|end| end.cond.is_none() && end.other.is_none() && end.taken.is_none_or(|t| t == tail));
        if merge {
            set_branches(f, prev, None, None, None);
            if !f.blocks[prev].iter().any(Inst::is_terminator) {
                Copier {
                    tail,
                    used_by_phi: &used_by_phi,
                    live_out: &live_out,
                    updates: &mut updates,
                    copies: &mut copies,
                }
                .copy_into(f, prev);
                f.succs[prev] = f.succs[tail].clone();
                f.edge_probs.resize_with(f.blocks.len(), Vec::new);
                f.edge_probs[prev] = f.edge_probs[tail].clone();
                done.push(prev);
                remaining.clear();
            }
        }
    }
    if done.is_empty() {
        return false;
    }
    // A predecessor that kept the tail still has to read the tail's PHIs from
    // a register of its own: the copies of the "nasty case".
    let kept: Vec<usize> =
        preds[tail].iter().copied().filter(|p| !done.contains(p) && f.succs[*p].len() == 1).collect();
    for pred in kept {
        let mut map = HashMap::default();
        let mut infos = Vec::new();
        let mut copier =
            Copier { tail, used_by_phi: &used_by_phi, live_out: &live_out, updates: &mut updates, copies: &mut copies };
        let phis: Vec<usize> = (0..f.blocks[tail].len()).take_while(|&at| f.blocks[tail][at].opcode == PHI).collect();
        for at in phis {
            let phi = f.blocks[tail][at].clone();
            if let Some(changed) = copier.process_phi(f, phi, pred, &mut map, &mut infos, false) {
                f.blocks[tail][at] = changed;
            }
        }
        copier.append_copies(f, pred, &infos);
    }
    let dead = predecessors(f)[tail].is_empty();
    update_successor_phis(f, tail, dead, &done, &successors, &updates);
    if dead {
        f.blocks[tail].clear();
        f.succs[tail].clear();
        if let Some(probs) = f.edge_probs.get_mut(tail) {
            probs.clear();
        }
    }
    update_ssa(f, tail, dead, &updates);
    propagate_copies(f, &copies);
    true
}

/// `updateSuccessorsPHIs`: each PHI of a successor reads, from every copy of
/// the tail, what it read from the tail.
fn update_successor_phis(
    f: &mut Function,
    tail: usize,
    dead: bool,
    done: &[usize],
    successors: &[usize],
    updates: &Updates,
) {
    for &succ in successors {
        let edges: Vec<usize> = f.succs.iter().enumerate().filter(|(_, s)| s.contains(&succ)).map(|(b, _)| b).collect();
        let count = f.blocks[succ].iter().take_while(|inst| inst.opcode == PHI).count();
        for at in 0..count {
            let phi = &mut f.blocks[succ][at];
            let Some((entry, reg)) = incoming(phi, tail) else { continue };
            let mut reuse = dead.then_some(entry);
            if dead {
                // Duplicate entries for the tail go.
                let mut i = phi.operands.len() - 2;
                while i > entry {
                    if phi.operands[i + 1] == Operand::Block(tail as u32) {
                        phi.operands.drain(i..i + 2);
                    }
                    i -= 2;
                }
            }
            let mut add = |phi: &mut Inst, reg: VReg, block: usize| match reuse.take() {
                Some(slot) => {
                    phi.operands[slot] = Operand::Use(reg);
                    phi.operands[slot + 1] = Operand::Block(block as u32);
                }
                None => {
                    phi.operands.push(Operand::Use(reg));
                    phi.operands.push(Operand::Block(block as u32));
                }
            };
            match updates.values.get(&reg) {
                Some(values) => {
                    for &(block, copy) in values {
                        if edges.contains(&block) {
                            add(phi, copy, block);
                        }
                    }
                }
                None => {
                    for &block in done {
                        add(phi, reg, block);
                    }
                }
            }
            if let Some(slot) = reuse {
                phi.operands.drain(slot..slot + 2);
            }
        }
    }
}

/// `MachineSSAUpdater`: every use of a register the tail defined, outside the
/// tail, reads the value of the copy (or the tail) that reaches it; a PHI
/// where different ones meet.
fn update_ssa(
    f: &mut Function,
    tail: usize,
    tail_alive: bool,
    updates: &Updates,
) {
    for &reg in &updates.order {
        let mut ends: HashMap<usize, VReg> = updates.values[&reg].iter().copied().collect();
        if tail_alive {
            ends.insert(tail, reg);
        }
        rewrite_uses(f, reg, tail, tail_alive, &ends);
    }
}

fn rewrite_uses(
    f: &mut Function,
    reg: VReg,
    tail: usize,
    tail_alive: bool,
    ends: &HashMap<usize, VReg>,
) {
    let preds = predecessors(f);
    let ty = f.vreg_type(reg);
    let blocks = f.blocks.len();
    // The value entering each block, by a fixed point over the blocks: the one
    // value every way in agrees on, a PHI of its own where two differ. A
    // block takes a PHI once and keeps it.
    let mut entering: Vec<Option<VReg>> = vec![None; blocks];
    let mut phi_of: HashMap<usize, VReg> = HashMap::default();
    loop {
        let mut changed = false;
        for block in 0..blocks {
            if phi_of.contains_key(&block) {
                continue;
            }
            let mut values: Vec<VReg> = Vec::new();
            for &pred in &preds[block] {
                if let Some(value) = ends.get(&pred).copied().or(entering[pred]) {
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
            }
            let now = match values.len() {
                0 => None,
                1 => Some(values[0]),
                _ => Some(*phi_of.entry(block).or_insert_with(|| f.new_vreg(ty))),
            };
            if now != entering[block] {
                entering[block] = now;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let end_value = |block: usize| ends.get(&block).copied().or(entering[block]);
    for block in 0..blocks {
        for at in 0..f.blocks[block].len() {
            let is_phi = f.blocks[block][at].opcode == PHI;
            if block == tail && tail_alive && !is_phi {
                continue;
            }
            for i in 0..f.blocks[block][at].operands.len() {
                if f.blocks[block][at].operands[i] != Operand::Use(reg) {
                    continue;
                }
                let value = if is_phi {
                    let Operand::Block(from) = f.blocks[block][at].operands[i + 1] else { continue };
                    end_value(from as usize)
                } else {
                    entering[block]
                };
                if let Some(value) = value.filter(|value| *value != reg) {
                    f.blocks[block][at].operands[i] = Operand::Use(value);
                }
            }
        }
    }
    for (&block, &phi) in &phi_of {
        let mut operands = vec![Operand::Def(phi)];
        for &pred in &preds[block] {
            operands.push(Operand::Use(end_value(pred).unwrap_or(reg)));
            operands.push(Operand::Block(pred as u32));
        }
        f.blocks[block].insert(0, Inst::new(PHI, &operands));
    }
}

/// The last step of `tailDuplicateAndUpdate`: a COPY whose source has no other
/// use is dropped, its users reading the source.
fn propagate_copies(
    f: &mut Function,
    copies: &[(VReg, VReg, usize)],
) {
    for &(dst, src, block) in copies {
        let uses = f.blocks.iter().flatten().filter(|inst| inst.operands.contains(&Operand::Use(src))).count();
        if uses != 1 {
            continue;
        }
        let Some(at) = f.blocks[block].iter().position(|inst| inst.operands == [Operand::Def(dst), Operand::Use(src)])
        else {
            continue;
        };
        f.blocks[block].remove(at);
        for inst in f.blocks.iter_mut().flatten() {
            for operand in &mut inst.operands {
                if *operand == Operand::Use(dst) {
                    *operand = Operand::Use(src);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{Inst, ValType};

    /// bb0 branches to bb1 or bb3; bb1 is a lone `br bb2`; bb2 has a PHI
    /// reading bb1 and bb3.
    fn function() -> (Function, u32) {
        let mut f = Function::default();
        let (a, b, c) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        f.blocks = vec![
            vec![Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(a)]), Inst::new(op::BR, &[Operand::Block(3)])],
            vec![Inst::new(op::BR, &[Operand::Block(2)])],
            vec![Inst::new(
                PHI,
                &[Operand::Def(c), Operand::Use(a), Operand::Block(1), Operand::Use(b), Operand::Block(3)],
            )],
            vec![Inst::new(op::BR, &[Operand::Block(2)])],
        ];
        f.succs = vec![vec![1, 3], vec![2], vec![], vec![2]];
        (f, a)
    }

    /// A loop preheader that only branches stayed a block of its own, and
    /// the PHI after it kept reading it.
    #[test]
    fn a_predecessor_branches_past_a_block_of_one_branch() {
        let (mut f, a) = function();
        assert!(early_tail_duplicate(&mut f, false));
        assert_eq!(f.succs[0], [2, 3]);
        assert_eq!(f.blocks[0][0].operands, [Operand::Block(2), Operand::Use(a)]);
        assert!(f.blocks[1].is_empty() && f.succs[1].is_empty());
        let pairs: Vec<_> = f.blocks[2][0].operands[1..].chunks(2).map(|p| p[1]).collect();
        assert_eq!(pairs, [Operand::Block(3), Operand::Block(0)]);
    }

    /// The interpreter's loop latch: three handlers branch to one block that
    /// reads a PHI of their values, adds to it, and jumps to the dispatch,
    /// whose PHI reads the sum. Left shared, each handler pays a branch to the
    /// latch (`computed_goto` O2 +3 instructions on wasm32).
    fn latch(preds: usize) -> Function {
        let mut f = Function::default();
        let ty = ValType::I32;
        let (four, sel) = (f.new_vreg(ty), f.new_vreg(ty));
        let values: Vec<_> = (0..preds).map(|_| f.new_vreg(ty)).collect();
        let (merged, next, dispatch) = (f.new_vreg(ty), f.new_vreg(ty), f.new_vreg(ty));
        let latch = preds + 1;
        // A block between the latch and its successor: a latch that branches to
        // the next block "falls through" and stays (`canFallThrough`).
        let dispatch_block = latch + 2;
        // 0: entry, `br_table` to the handlers; 1..=preds: handlers; then the
        // latch and the block that reads its sum.
        let mut table = vec![Operand::Use(sel)];
        table.extend((1..=preds).map(|b| Operand::Block(b as u32)));
        f.blocks.push(vec![
            Inst::new(op::CONST_I32, &[Operand::Def(four), Operand::Imm(4)]),
            Inst::new(op::BR_TABLE_I32, &table),
        ]);
        f.succs.push((1..=preds).collect());
        for (at, value) in values.iter().enumerate() {
            f.blocks.push(vec![
                Inst::new(op::CONST_I32, &[Operand::Def(*value), Operand::Imm(at as i64)]),
                Inst::new(op::BR, &[Operand::Block(latch as u32)]),
            ]);
            f.succs.push(vec![latch]);
        }
        let mut phi = vec![Operand::Def(merged)];
        for (at, value) in values.iter().enumerate() {
            phi.push(Operand::Use(*value));
            phi.push(Operand::Block(at as u32 + 1));
        }
        f.blocks.push(vec![
            Inst::new(PHI, &phi),
            Inst::new(op::ADD_I32, &[Operand::Def(next), Operand::Use(merged), Operand::Use(four)]),
            Inst::new(op::BR, &[Operand::Block(dispatch_block as u32)]),
        ]);
        f.succs.push(vec![dispatch_block]);
        f.blocks.push(vec![Inst::new(op::RETURN, &[Operand::Use(four)])]);
        f.succs.push(vec![]);
        f.blocks.push(vec![
            Inst::new(PHI, &[Operand::Def(dispatch), Operand::Use(next), Operand::Block(latch as u32)]),
            Inst::new(op::RETURN, &[Operand::Use(dispatch)]),
        ]);
        f.succs.push(vec![]);
        f
    }

    #[test]
    fn a_latch_of_three_phis_is_copied_into_every_handler() {
        for preds in 2..=4 {
            let mut f = latch(preds);
            assert!(early_tail_duplicate(&mut f, false), "{preds} handlers");
            let latch = preds + 1;
            assert!(f.blocks[latch].is_empty() && f.succs[latch].is_empty(), "the shared latch stayed");
            for handler in 1..=preds {
                assert_eq!(f.succs[handler], [latch + 2]);
                assert!(
                    f.blocks[handler].iter().any(|inst| inst.opcode == op::ADD_I32),
                    "handler {handler} has no copy of the add"
                );
            }
            let phi = &f.blocks[latch + 2][0];
            assert_eq!(phi.operands.len(), 1 + 2 * preds, "the PHI after the latch reads each copy");
        }
    }

    /// At -Os the budget is one instruction: the add and the branch are two.
    #[test]
    fn a_latch_of_two_instructions_stays_shared_at_os() {
        let mut f = latch(3);
        assert!(!early_tail_duplicate(&mut f, true));
        assert_eq!(f.blocks[4].len(), 3);
    }

    /// A loop header of two instructions (`br_if` body; `br` exit) whose `br`
    /// goes to the next block can fall through, whatever its explicit branch:
    /// the pin leaves it (`canFallThrough` counts a branch to the layout
    /// successor). Copying it into the entry made `recdeep` -O1 44 B and
    /// 11,989 instructions worse than the pin.
    #[test]
    fn a_loop_header_that_branches_to_the_next_block_is_not_copied() {
        let mut f = Function::default();
        let ty = ValType::I32;
        let (start, n, next, counter) = (f.new_vreg(ty), f.new_vreg(ty), f.new_vreg(ty), f.new_vreg(ty));
        f.blocks = vec![
            vec![Inst::new(op::CONST_I32, &[Operand::Def(start), Operand::Imm(5)])],
            vec![
                Inst::new(
                    PHI,
                    &[Operand::Def(n), Operand::Use(start), Operand::Block(0), Operand::Use(next), Operand::Block(3)],
                ),
                Inst::new(op::BR_IF, &[Operand::Block(3), Operand::Use(n)]),
                Inst::new(op::BR, &[Operand::Block(2)]),
            ],
            vec![Inst::new(op::RETURN, &[Operand::Use(n)])],
            vec![
                Inst::new(op::CONST_I32, &[Operand::Def(counter), Operand::Imm(-1)]),
                Inst::new(op::ADD_I32, &[Operand::Def(next), Operand::Use(n), Operand::Use(counter)]),
                Inst::new(op::BR, &[Operand::Block(1)]),
            ],
        ];
        f.succs = vec![vec![1], vec![3, 2], vec![], vec![1]];
        let before = f.blocks.clone();
        assert!(!early_tail_duplicate(&mut f, false));
        assert_eq!(f.blocks, before);
    }
}
