//! Where an expression is defined and whether an instruction is certain to
//! run when another does: `getDefiningScopeBound`,
//! `isGuaranteedToTransferExecutionTo` and `isSCEVExprNeverPoison`.

use std::collections::HashSet;

use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::valuetracking::guarantee;

use super::expr::{Kind, Scev, Use};
use super::{Cx, ScalarEvolution};

/// The `ScanLimit` default of `isGuaranteedToTransferExecutionToSuccessor`.
const SCAN_LIMIT: u32 = 32;

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7367
    /// `getNonTrivialDefiningScopeBound`.
    fn get_non_trivial_defining_scope_bound(
        &self,
        cx: &Cx,
        s: Scev,
    ) -> Option<InstId> {
        match self.node(s).kind {
            Kind::AddRec(_, header) => cx.function.block(header).instructions().first().copied(),
            Kind::Unknown(Operand::Value(id)) => match cx.function.value(id).def {
                ValueDef::Instruction(inst) => Some(inst),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7376
    /// `getDefiningScopeBound(Ops, Precise)`: an instruction that dominates
    /// every instruction the expressions of `ops` are defined at, found by a
    /// bounded search.
    pub fn get_defining_scope_bound(
        &self,
        cx: &Cx,
        ops: &[Use],
    ) -> (InstId, bool) {
        let mut precise = true;
        // Do a bounded search of the def relation of the requested SCEVs.
        let mut visited: HashSet<Scev> = HashSet::new();
        let mut worklist: Vec<Scev> = Vec::new();
        let mut push_op = |s: Scev, precise: &mut bool, worklist: &mut Vec<Scev>| {
            if !visited.insert(s) {
                return;
            }
            // Threshold of 30 here is arbitrary.
            if visited.len() > 30 {
                *precise = false;
                return;
            }
            worklist.push(s);
        };

        for s in ops {
            push_op(s.scev, &mut precise, &mut worklist);
        }

        let mut bound: Option<InstId> = None;
        while let Some(s) = worklist.pop() {
            if let Some(def) = self.get_non_trivial_defining_scope_bound(cx, s) {
                if bound.is_none_or(|bound| self.inst_dominates(cx, bound, def)) {
                    bound = Some(def);
                }
            } else {
                for op in self.operands_of(s) {
                    push_op(op.scev, &mut precise, &mut worklist);
                }
            }
        }
        let entry = cx.function.entry().expect("a function with a body");
        (bound.unwrap_or_else(|| cx.function.block(entry).instructions()[0]), precise)
    }

    /// `DT.dominates(const Instruction *, const Instruction *)`.
    fn inst_dominates(
        &self,
        cx: &Cx,
        a: InstId,
        b: InstId,
    ) -> bool {
        cx.tree.instruction_dominates(cx.function, a, b)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7407
    /// `ScalarEvolution::isGuaranteedToTransferExecutionTo`.
    pub fn is_guaranteed_to_transfer_execution_to(
        &self,
        cx: &Cx,
        a: InstId,
        b: InstId,
    ) -> bool {
        let q = cx.query(None);
        let function = cx.function;
        let (Some(a_block), Some(b_block)) = (function.parent(a), function.parent(b)) else { return false };
        let position = |block: BlockId, inst: InstId| -> usize {
            function
                .block(block)
                .instructions()
                .iter()
                .position(|&one| one == inst)
                .expect("an instruction of its block")
        };
        if a_block == b_block
            && position(a_block, a) <= position(b_block, b)
            && guarantee::is_guaranteed_to_transfer_execution_to_successor_between(
                &q,
                a_block,
                position(a_block, a),
                position(b_block, b),
                SCAN_LIMIT,
            )
        {
            return true;
        }

        if let Some(b_loop) = cx.loops.get_loop_for(b_block)
            && b_loop.get_header() == b_block
            && b_loop.get_loop_preheader(function) == Some(a_block)
            && guarantee::is_guaranteed_to_transfer_execution_to_successor_between(
                &q,
                a_block,
                position(a_block, a),
                function.block(a_block).instructions().len(),
                SCAN_LIMIT,
            )
            && guarantee::is_guaranteed_to_transfer_execution_to_successor_between(
                &q,
                b_block,
                0,
                position(b_block, b),
                SCAN_LIMIT,
            )
        {
            return true;
        }
        false
    }

    /// `isGuaranteedToTransferExecutionTo(getDefiningScopeBound(ops),
    /// &*L->getHeader()->begin())`, as `getAddExpr` asks it.
    pub(super) fn is_guaranteed_to_transfer_execution_to_loop_header(
        &mut self,
        cx: &Cx,
        ops: &[Use],
        l: BlockId,
    ) -> bool {
        let (def_i, _) = self.get_defining_scope_bound(cx, ops);
        let reach_i = cx.function.block(l).instructions()[0];
        self.is_guaranteed_to_transfer_execution_to(cx, def_i, reach_i)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7441
    /// `ScalarEvolution::isSCEVExprNeverPoison`: whether `i` is certain not to
    /// yield poison where the expressions of its operands are defined.
    pub fn is_scev_expr_never_poison(
        &mut self,
        cx: &Cx,
        i: InstId,
    ) -> bool {
        // Only proceed if we can prove that I does not yield poison.
        let q = cx.query(None);
        let result = cx.function.instruction(i).result.map(Operand::Value);
        let Some(result) = result else { return false };
        if !guarantee::program_undefined_if_poison(&q, result) {
            return false;
        }

        // At this point we know that if I is executed, then it does not wrap
        // according to at least one of NSW or NUW. If I is not executed, then
        // we do not know if the calculation that I represents would wrap.
        // Multiple instructions can map to the same SCEV. If we apply NSW or
        // NUW from I to the SCEV, we must guarantee no wrapping for that SCEV
        // also when it is derived from other instructions that map to the same
        // SCEV. We cannot make that guarantee for cases where I is not
        // executed. So we need to find a upper bound on the defining scope for
        // the SCEV, and prove that I is executed every time we enter that
        // scope. When the bounding scope is a loop (the common case), this is
        // equivalent to proving I executes on every iteration of that loop.
        let mut scev_ops: Vec<Use> = Vec::new();
        for op in cx.function.instruction(i).operands.clone() {
            // I could be an extractvalue from a call to an overflow intrinsic.
            // TODO: We can do better here in some cases.
            let scevable = match op {
                Operand::Value(id) => self.ty_of(cx, cx.function.value(id).ty).is_some(),
                Operand::Constant(id) => self.ty_of(cx, cx.context.get(id).ty).is_some(),
                Operand::Block(_) => false,
            };
            if scevable {
                scev_ops.push(Use::of(self.get_scev(cx, op)));
            }
        }
        let (def_i, _) = self.get_defining_scope_bound(cx, &scev_ops);
        self.is_guaranteed_to_transfer_execution_to(cx, def_i, i)
    }
}
