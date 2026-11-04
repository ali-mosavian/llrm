//! `BranchFolder::TailMergeBlocks` for wasm: blocks that end in the same
//! instructions share one copy of them. The algorithm is the shared one
//! (`llrm_spike_mf::tail_merge_blocks`); this file is wasm's side of it, the
//! `analyzeBranch`, `removeBranch` and `insertBranch` of
//! `WebAssemblyInstrInfo` over [`Function`].
//!
//! The pin leaves tail merging on for wasm: `WebAssemblyTargetMachine` does
//! not call `setRequiresStructuredCFG`, "it disables optimizations that we're
//! ok with, and want, such as critical edge splitting and tail merging"
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyTargetMachine.cpp#L198)).
//!
//! A block is named by its index in [`Function::blocks`]; a split appends the
//! new block there and the layout, a list of those indices, puts it next to
//! the block it came from. The blocks are renumbered into layout order at the
//! end (`apply_order`).
//!
//! DEVIATE 1: not in a function with a personality (try scopes name their
//! blocks), and `shouldOptimizeForSize` is the optimization level (`-Os`,
//! `-Oz`), not the function's attribute, as in `tail_duplicate`. Instructions
//! are the same only when their memory operand is too. Exit: the shared branch
//! folder over the neutral machine function (S4), no later than 2026-11-20.
//!
//! # See also
//! `BranchFolder::{TailMergeBlocks, TryTailMergeBlocks, ProfitableToMerge}`,
//! `WebAssemblyInstrInfo::{analyzeBranch, removeBranch, insertBranch}`, pin
//! d1106deb71cc.

use std::cell::RefCell;

use llrm_spike_mf::tailmerge::{Exits, TailMergeFunction, tail_merge_blocks};

use crate::branch_fold::reversed;
use crate::dagisel::generated::ids::opcode as op;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Inst, Operand};

fn has(
    inst: &Inst,
    bit: u64,
) -> bool {
    desc(inst.opcode).is_some_and(|d| d.has(bit))
}

fn is_cond(inst: &Inst) -> bool {
    matches!(inst.opcode, op::BR_IF | op::BR_UNLESS)
}

struct Tails<'a> {
    f: &'a mut Function,
    /// The blocks in layout order.
    layout: Vec<usize>,
    for_size: bool,
    /// The predecessors of each block, rebuilt when the edges changed.
    preds: RefCell<Option<Vec<Vec<usize>>>>,
}

impl Tails<'_> {
    fn position(
        &self,
        block: usize,
    ) -> usize {
        self.layout.iter().position(|&one| one == block).expect("a block in the layout")
    }

    fn next(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.layout.get(self.position(block) + 1).copied()
    }

    /// The edges of `block` as its code leaves them: the targets of its
    /// terminators in order, then the fall-through.
    fn reset_succs(
        &mut self,
        block: usize,
    ) {
        let mut succs: Vec<usize> = Vec::new();
        let targets =
            self.f.blocks[block].iter().filter(|inst| inst.is_terminator()).filter_map(
                |inst| match inst.operands[..] {
                    [Operand::Block(to), ..] => Some(to as usize),
                    _ => None,
                },
            );
        for to in targets.chain(self.next(block).filter(|_| self.can_fall_through(block))) {
            if !succs.contains(&to) {
                succs.push(to);
            }
        }
        self.f.succs[block] = succs;
        *self.preds.borrow_mut() = None;
    }
}

impl TailMergeFunction for Tails<'_> {
    type Block = usize;
    type Inst = Inst;
    type Cond = Inst;

    fn blocks(&self) -> Vec<usize> {
        self.layout.clone()
    }

    fn successors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        for &to in &self.f.succs[block] {
            if !out.contains(&to) {
                out.push(to);
            }
        }
        out
    }

    fn predecessors(
        &self,
        block: usize,
    ) -> Vec<usize> {
        let mut cache = self.preds.borrow_mut();
        let preds = cache.get_or_insert_with(|| {
            let mut preds = vec![Vec::new(); self.f.blocks.len()];
            for &from in &self.layout {
                for to in self.successors(from) {
                    preds[to].push(from);
                }
            }
            preds
        });
        preds[block].clone()
    }

    fn insts(
        &self,
        block: usize,
    ) -> &[Inst] {
        &self.f.blocks[block]
    }

    fn same_inst(
        &self,
        one: &Inst,
        other: &Inst,
    ) -> bool {
        one.opcode == other.opcode && one.operands == other.operands && one.mem == other.mem
    }

    fn hash_inst(
        &self,
        inst: &Inst,
    ) -> u32 {
        let mut hash = 2166136261u32;
        let mut mix = |word: u64| {
            for byte in word.to_le_bytes() {
                hash = (hash ^ u32::from(byte)).wrapping_mul(16777619);
            }
        };
        mix(u64::from(inst.opcode));
        for operand in &inst.operands {
            match *operand {
                Operand::Def(reg) => mix(1 << 56 | u64::from(reg)),
                Operand::Use(reg) => mix(2 << 56 | u64::from(reg)),
                Operand::Imm(value) => mix(3 << 56 ^ value as u64),
                Operand::Sym(index) => mix(4 << 56 | u64::from(index)),
                Operand::Block(block) => mix(5 << 56 | u64::from(block)),
                Operand::Frame(index) => mix(6 << 56 ^ index as u64),
                Operand::F32(bits) => mix(7 << 56 | u64::from(bits)),
                Operand::F64(bits) => mix(8 << 56 ^ bits),
            }
        }
        hash
    }

    fn is_terminator(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.is_terminator()
    }

    fn is_barrier(
        &self,
        inst: &Inst,
    ) -> bool {
        inst.is_barrier()
    }

    fn is_call(
        &self,
        inst: &Inst,
    ) -> bool {
        has(inst, flag::CALL)
    }

    fn may_load_or_store(
        &self,
        inst: &Inst,
    ) -> bool {
        has(inst, flag::MAY_LOAD) || has(inst, flag::MAY_STORE)
    }

    fn is_return_or_indirect(
        &self,
        inst: &Inst,
    ) -> bool {
        has(inst, flag::RETURN) || has(inst, flag::INDIRECT_BRANCH)
    }

    fn can_fall_through(
        &self,
        block: usize,
    ) -> bool {
        self.next(block).is_some() && self.f.blocks[block].last().is_none_or(|last| !last.is_barrier())
    }

    fn optimize_for_size(
        &self,
        _block: usize,
    ) -> bool {
        self.for_size
    }

    /// `WebAssemblyInstrInfo::analyzeBranch`: `BR_IF`, `BR_UNLESS` and `BR`;
    /// any other terminator is not understood.
    fn analyze_branch(
        &self,
        block: usize,
    ) -> Option<Exits<usize, Inst>> {
        let mut exits = Exits { taken: None, other: None, cond: None };
        let target = |inst: &Inst| match inst.operands[..] {
            [Operand::Block(to), ..] => Some(to as usize),
            _ => None,
        };
        for inst in self.f.blocks[block].iter().skip_while(|inst| !inst.is_terminator()) {
            if !inst.is_terminator() {
                return None;
            }
            match inst.opcode {
                op::BR_IF | op::BR_UNLESS => {
                    if exits.cond.is_some() {
                        return None;
                    }
                    exits.taken = target(inst);
                    exits.cond = Some(inst.clone());
                }
                op::BR if exits.cond.is_none() => exits.taken = target(inst),
                op::BR => exits.other = target(inst),
                _ => return None,
            }
            if inst.is_barrier() {
                break;
            }
        }
        Some(exits)
    }

    fn reverse_branch_condition(
        &self,
        cond: &Inst,
    ) -> Option<Inst> {
        Some(reversed(cond))
    }

    /// `removeBranch`: every terminator at the end goes; the block then falls
    /// into the next one.
    fn remove_branch(
        &mut self,
        block: usize,
    ) {
        while self.f.blocks[block].last().is_some_and(Inst::is_terminator) {
            self.f.blocks[block].pop();
        }
        self.reset_succs(block);
    }

    /// `insertBranch`: `cond` to `taken` then a jump to `other`, or `cond`
    /// alone falling into the next block, or a jump to `taken`.
    fn insert_branch(
        &mut self,
        block: usize,
        taken: usize,
        other: Option<usize>,
        cond: Option<&Inst>,
    ) {
        let jump = |to: usize| Inst::new(op::BR, &[Operand::Block(to as u32)]);
        match cond {
            Some(cond) => {
                debug_assert!(is_cond(cond));
                let mut cond = cond.clone();
                cond.operands[0] = Operand::Block(taken as u32);
                self.f.blocks[block].push(cond);
                if let Some(other) = other {
                    self.f.blocks[block].push(jump(other));
                }
            }
            None => self.f.blocks[block].push(jump(taken)),
        }
        self.reset_succs(block);
    }

    /// `SplitMBBAt`: the tail is a new block, laid out next, with the old
    /// successors; the old block falls into it.
    fn split_block_at(
        &mut self,
        block: usize,
        at: usize,
    ) -> Option<usize> {
        let new = self.f.blocks.len();
        let tail = self.f.blocks[block].split_off(at);
        self.f.blocks.push(tail);
        let succs = std::mem::take(&mut self.f.succs[block]);
        self.f.succs.push(succs);
        let at = self.position(block) + 1;
        self.layout.insert(at, new);
        self.reset_succs(block);
        Some(new)
    }

    /// `ReplaceTailWithBranchTo`: the block ends before `at` and goes to
    /// `dest`, by falling into it or by a jump.
    fn replace_tail_with_branch_to(
        &mut self,
        block: usize,
        at: usize,
        dest: usize,
    ) {
        self.f.blocks[block].truncate(at);
        if self.next(block) != Some(dest) {
            self.f.blocks[block].push(Inst::new(op::BR, &[Operand::Block(dest as u32)]));
        }
        self.reset_succs(block);
    }
}

/// Whether any tail was shared.
pub fn tail_merge(
    f: &mut Function,
    for_size: bool,
) -> bool {
    if f.personality || f.blocks.len() < 3 {
        return false;
    }
    let layout = (0..f.blocks.len()).collect();
    let mut tails = Tails { f, layout, for_size, preds: RefCell::new(None) };
    let changed = tail_merge_blocks(&mut tails);
    let order = std::mem::take(&mut tails.layout);
    if changed && order.iter().enumerate().any(|(at, &block)| at != block) {
        f.apply_order(&order);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::ValType;

    fn consts(
        f: &Function,
        range: std::ops::Range<i64>,
    ) -> usize {
        f.blocks
            .iter()
            .flatten()
            .filter(|i| i.opcode == op::CONST_I32 && matches!(i.operands[1], Operand::Imm(v) if range.contains(&v)))
            .count()
    }

    /// Every edge of `succs` is a terminator's target or the fall-through.
    fn succs_match_code(f: &Function) {
        for (at, block) in f.blocks.iter().enumerate() {
            let mut want: Vec<usize> = block
                .iter()
                .filter(|i| i.is_terminator())
                .filter_map(|i| match i.operands.first() {
                    Some(Operand::Block(to)) => Some(*to as usize),
                    _ => None,
                })
                .collect();
            if block.last().is_none_or(|last| !last.is_barrier()) && at + 1 < f.blocks.len() {
                want.push(at + 1);
            }
            want.sort_unstable();
            want.dedup();
            let mut have = f.succs[at].clone();
            have.sort_unstable();
            have.dedup();
            assert_eq!(have, want, "successors of bb{at}");
        }
    }

    /// bb0 branches to bb1 or bb2, which end in the same three constants and
    /// a return after a constant of their own.
    fn two_arms() -> Function {
        let mut f = Function::default();
        let c = f.new_vreg(ValType::I32);
        let x = f.new_vreg(ValType::I32);
        let [a, b, d] = [(); 3].map(|()| f.new_vreg(ValType::I32));
        let imm = |reg, value| Inst::new(op::CONST_I32, &[Operand::Def(reg), Operand::Imm(value)]);
        let tail = |ret: bool| {
            let mut tail = vec![imm(a, 10), imm(b, 11), imm(d, 12)];
            if ret {
                tail.push(Inst::new(op::RETURN, &[Operand::Use(d)]));
            }
            tail
        };
        let arm = |value| [vec![imm(x, value)], tail(true)].concat();
        f.blocks = vec![
            vec![
                imm(c, 0),
                Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(c)]),
                Inst::new(op::BR, &[Operand::Block(2)]),
            ],
            arm(1),
            arm(2),
        ];
        f.succs = vec![vec![1, 2], vec![], vec![]];
        f
    }

    /// Two returns that ended in the same constants each kept their own copy:
    /// the pin's branch folder shares them, at the cost of one jump.
    #[test]
    fn blocks_that_end_in_the_same_instructions_share_them() {
        let mut f = two_arms();
        assert_eq!(consts(&f, 10..13), 6);
        assert!(tail_merge(&mut f, false));
        assert_eq!(consts(&f, 10..13), 3);
        succs_match_code(&f);
    }

    /// A tail that is a whole block of one predecessor and the end of another
    /// splits the longer one: bb1 is `x; a; b; d; return`, bb2 is `a; b; d;
    /// return`, and bb1 ends up jumping into bb2.
    #[test]
    fn a_block_that_is_the_tail_of_another_is_jumped_to() {
        let mut f = two_arms();
        f.blocks[2].remove(0);
        assert!(tail_merge(&mut f, false));
        assert_eq!(consts(&f, 10..13), 3);
        succs_match_code(&f);
    }
}
