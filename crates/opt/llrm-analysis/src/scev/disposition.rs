//! Loop and block dispositions: how an expression relates to a loop (invariant,
//! computable, uniform, variant) and to a block (does it dominate it).
//! A loop is named by its header.

use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Operand, ValueDef};

use super::expr::{Kind, Scev};
use super::{Cx, ScalarEvolution};

/// `ScalarEvolution::LoopDisposition`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopDisposition {
    /// The SCEV is loop-variant (changes in the loop in an unknowable way).
    Variant,
    /// The SCEV is loop-invariant.
    Invariant,
    /// The SCEV varies predictably with the loop.
    Computable,
    /// The SCEV varies in the same way for every iteration of an enclosing
    /// loop, at a given iteration.
    Uniform,
}

/// `ScalarEvolution::BlockDisposition`, in LLVM's order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BlockDisposition {
    DoesNotDominateBlock,
    DominatesBlock,
    ProperlyDominatesBlock,
}

/// The loop `header` heads.
pub(super) fn loop_of<'a>(
    cx: &'a Cx,
    header: BlockId,
) -> Loop<'a> {
    cx.loops.get_loop_for(header).filter(|one| one.get_header() == header).expect("a loop header")
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14466
    /// `ScalarEvolution::getLoopDisposition`; `None` is the function body.
    pub fn get_loop_disposition(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: Option<BlockId>,
    ) -> LoopDisposition {
        if let Some(&found) = self.loop_dispositions.get(&(s, l)) {
            return found;
        }
        // The provisional answer a recursive query sees.
        self.loop_dispositions.insert((s, l), LoopDisposition::Variant);
        let d = self.compute_loop_disposition(cx, s, l);
        self.loop_dispositions.insert((s, l), d);
        d
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14485
    /// `ScalarEvolution::computeLoopDisposition`.
    fn compute_loop_disposition(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: Option<BlockId>,
    ) -> LoopDisposition {
        match self.node(s).kind.clone() {
            Kind::Constant(_) => LoopDisposition::Invariant,
            Kind::AddRec(_, ar_loop) => {
                // If L is the addrec's loop, it's computable.
                if l == Some(ar_loop) {
                    return LoopDisposition::Computable;
                }

                // Add recurrences are never invariant in the function-body
                // (null loop).
                let Some(l) = l else { return LoopDisposition::Variant };

                // Everything that is not defined at loop entry is variant.
                if cx.tree.dominates(l, ar_loop) {
                    let outer = loop_of(cx, l);
                    if outer.contains_loop(Some(loop_of(cx, ar_loop)))
                        && self.operands(s).to_vec().iter().all(|op| self.is_loop_uniform(cx, op.scev, Some(l)))
                    {
                        return LoopDisposition::Uniform;
                    }
                    return LoopDisposition::Variant;
                }
                debug_assert!(
                    !loop_of(cx, l).contains_loop(Some(loop_of(cx, ar_loop))),
                    "Containing loop's header does not dominate the contained loop's header?"
                );

                // This recurrence is invariant w.r.t. L if AR's loop contains
                // L.
                if loop_of(cx, ar_loop).contains_loop(Some(loop_of(cx, l))) {
                    return LoopDisposition::Invariant;
                }

                // This recurrence is variant w.r.t. L if any of its operands
                // are variant.
                for op in self.operands(s).to_vec() {
                    if !self.is_loop_invariant(cx, op.scev, Some(l)) {
                        return LoopDisposition::Variant;
                    }
                }

                // Otherwise it's loop-invariant.
                LoopDisposition::Invariant
            }
            Kind::Truncate(_)
            | Kind::ZeroExtend(_)
            | Kind::SignExtend(_)
            | Kind::PtrToAddr(_)
            | Kind::Add(_)
            | Kind::Mul(_)
            | Kind::UDiv(..)
            | Kind::UMax(_)
            | Kind::SMax(_)
            | Kind::UMin(_)
            | Kind::SMin(_)
            | Kind::SequentialUMin(_) => {
                let mut has_varying = false;
                let mut has_uniform = false;
                for op in self.operands_of(s) {
                    let d = self.get_loop_disposition(cx, op.scev, l);
                    if d == LoopDisposition::Variant {
                        return LoopDisposition::Variant;
                    }
                    if d == LoopDisposition::Computable {
                        has_varying = true;
                    }
                    if d == LoopDisposition::Uniform {
                        has_uniform = true;
                    }
                }
                if has_varying {
                    if has_uniform { LoopDisposition::Variant } else { LoopDisposition::Computable }
                } else if has_uniform {
                    LoopDisposition::Uniform
                } else {
                    LoopDisposition::Invariant
                }
            }
            Kind::Unknown(value) => {
                // All non-instruction values are loop invariant. All
                // instructions are loop invariant if they are not contained in
                // the specified loop. Instructions are never considered
                // invariant in the function body (null loop) because they are
                // defined within the "loop".
                if let Operand::Value(id) = value
                    && let ValueDef::Instruction(inst) = cx.function.value(id).def
                {
                    return match l {
                        Some(l) if !loop_of(cx, l).contains_instruction(cx.function, inst) => {
                            LoopDisposition::Invariant
                        }
                        _ => LoopDisposition::Variant,
                    };
                }
                LoopDisposition::Invariant
            }
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14566
    /// `ScalarEvolution::isLoopUniform`.
    pub fn is_loop_uniform(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: Option<BlockId>,
    ) -> bool {
        matches!(
            self.get_loop_disposition(cx, s, l),
            LoopDisposition::Uniform | LoopDisposition::Invariant
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14571
    /// `ScalarEvolution::isLoopInvariant`.
    pub fn is_loop_invariant(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: Option<BlockId>,
    ) -> bool {
        self.get_loop_disposition(cx, s, l) == LoopDisposition::Invariant
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14575
    /// `ScalarEvolution::hasComputableLoopEvolution`.
    pub fn has_computable_loop_evolution(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: Option<BlockId>,
    ) -> bool {
        self.get_loop_disposition(cx, s, l) == LoopDisposition::Computable
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14580
    /// `ScalarEvolution::getBlockDisposition`.
    pub fn get_block_disposition(
        &mut self,
        cx: &Cx,
        s: Scev,
        bb: BlockId,
    ) -> BlockDisposition {
        if let Some(&found) = self.block_dispositions.get(&(s, bb)) {
            return found;
        }
        // The provisional answer a recursive query sees.
        self.block_dispositions.insert((s, bb), BlockDisposition::DoesNotDominateBlock);
        let d = self.compute_block_disposition(cx, s, bb);
        self.block_dispositions.insert((s, bb), d);
        d
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14599
    /// `ScalarEvolution::computeBlockDisposition`.
    fn compute_block_disposition(
        &mut self,
        cx: &Cx,
        s: Scev,
        bb: BlockId,
    ) -> BlockDisposition {
        match self.node(s).kind.clone() {
            Kind::Constant(_) => return BlockDisposition::ProperlyDominatesBlock,
            Kind::Unknown(value) => {
                if let Operand::Value(id) = value
                    && let ValueDef::Instruction(inst) = cx.function.value(id).def
                {
                    let Some(parent) = cx.function.parent(inst) else { return BlockDisposition::DoesNotDominateBlock };
                    if parent == bb {
                        return BlockDisposition::DominatesBlock;
                    }
                    if parent != bb && cx.tree.dominates(parent, bb) {
                        return BlockDisposition::ProperlyDominatesBlock;
                    }
                    return BlockDisposition::DoesNotDominateBlock;
                }
                return BlockDisposition::ProperlyDominatesBlock;
            }
            Kind::AddRec(_, header) => {
                // This uses a "dominates" query instead of "properly dominates"
                // query to test for proper dominance too, because the
                // instruction which produces the addrec's value is a PHI, and a
                // PHI effectively properly dominates its entire containing
                // block.
                if !cx.tree.dominates(header, bb) {
                    return BlockDisposition::DoesNotDominateBlock;
                }
            }
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
            _ => {}
        }
        let mut proper = true;
        for op in self.operands_of(s) {
            let d = self.get_block_disposition(cx, op.scev, bb);
            if d == BlockDisposition::DoesNotDominateBlock {
                return BlockDisposition::DoesNotDominateBlock;
            }
            if d == BlockDisposition::DominatesBlock {
                proper = false;
            }
        }
        if proper { BlockDisposition::ProperlyDominatesBlock } else { BlockDisposition::DominatesBlock }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14654
    /// `ScalarEvolution::dominates`.
    pub fn dominates_block(
        &mut self,
        cx: &Cx,
        s: Scev,
        bb: BlockId,
    ) -> bool {
        self.get_block_disposition(cx, s, bb) >= BlockDisposition::DominatesBlock
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14658
    /// `ScalarEvolution::properlyDominates`.
    pub fn properly_dominates(
        &mut self,
        cx: &Cx,
        s: Scev,
        bb: BlockId,
    ) -> bool {
        self.get_block_disposition(cx, s, bb) == BlockDisposition::ProperlyDominatesBlock
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2512
    /// `ScalarEvolution::isAvailableAtLoopEntry`.
    pub fn is_available_at_loop_entry(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
    ) -> bool {
        self.is_loop_invariant(cx, s, Some(l)) && self.properly_dominates(cx, s, l)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14662
    /// `ScalarEvolution::hasOperand`: whether `op` is anywhere in `s`.
    pub fn has_operand(
        &self,
        s: Scev,
        op: Scev,
    ) -> bool {
        self.expr_contains(s, &|expr| expr == op)
    }

    /// `SCEVExprContains`: whether any node of the tree of `root` satisfies
    /// `pred`, by `SCEVTraversal`'s order and cut-offs.
    pub fn expr_contains(
        &self,
        root: Scev,
        pred: &dyn Fn(Scev) -> bool,
    ) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut worklist = vec![root];
        visited.insert(root);
        // `follow` runs when a node is pushed: a node that satisfies the
        // predicate is found, and not followed.
        if pred(root) {
            return true;
        }
        while let Some(s) = worklist.pop() {
            if matches!(self.node(s).kind, Kind::Constant(_) | Kind::Unknown(_)) {
                continue;
            }
            for op in self.operands_of(s) {
                if visited.insert(op.scev) {
                    if pred(op.scev) {
                        return true;
                    }
                    worklist.push(op.scev);
                }
            }
        }
        false
    }
}
