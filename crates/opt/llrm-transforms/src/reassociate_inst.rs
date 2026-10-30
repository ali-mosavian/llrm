//! Reassociate of one instruction: put it in a form that joins a tree, and if
//! it is the root of a tree, reassociate the tree.
//!
//! Translation of `OptimizeInst`, `canonicalizeOperands` and
//! `ReassociateExpression`
//! ([Reassociate.cpp:2292](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2292),
//! [:290](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L290),
//! [:2442](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L2442))
//! at d1106deb71cc.
//!
//! MIRROR: the order of the steps (shift to multiply, commute by rank, fold
//! negative floating constants, `or` to add, distribute, break up a subtract,
//! lower a negate); an interior node of a tree waits for its root so that a
//! tree is analyzed once.
//!
//! # How it works
//! The steps are a chain over one instruction; each step may replace the
//! instruction by the one it made. A root linearizes its tree, sorts the leaves
//! by rank (stable, highest first), optimizes the list, chooses the pair for
//! CSE and writes the list back.
//!
//! # Used by
//! `crate::reassociate` (`visit_block`).
//!
//! # Example
//! `(a + 4) + b` where `b` has the lower rank becomes `(b + a) + 4`.

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::reassociate::Reassociator;
use crate::reassociate_fp::float_value;
use crate::reassociate_pairs::GLOBAL_REASSOCIATE_LIMIT;
use crate::reassociate_tree::OverflowTracking;

impl Reassociator<'_> {
    /// `canonicalizeOperands`: a constant to the right, else the lower rank to
    /// the right.
    fn canonicalize_operands(
        &mut self,
        inst: InstId,
    ) {
        let (lhs, rhs) = (self.operand(inst, 0), self.operand(inst, 1));
        if lhs == rhs || self.is_constant(rhs) {
            return;
        }
        if self.is_constant(lhs) || self.rank(rhs) < self.rank(lhs) {
            self.swap_operands(inst);
            self.changed = true;
        }
    }

    /// `OptimizeInst`.
    pub fn optimize_inst(
        &mut self,
        mut i: InstId,
    ) {
        // Only the operations understood: unary and binary operators.
        if !matches!(
            self.function.instruction(i).opcode,
            Opcode::Binary(_) | Opcode::FNeg
        ) {
            return;
        }

        if self.binop(i) == Some(BinaryOp::Shl)
            && self.int_constant(self.operand(i, 1)).is_some()
            && (self.reassociable(self.operand(i, 0), &[BinaryOp::Mul]).is_some()
                || self.only_user_value(self.result(i)).is_some_and(|user| {
                    self.reassociable(user, &[BinaryOp::Mul]).is_some()
                        || self.reassociable(user, &[BinaryOp::Add]).is_some()
                }))
        {
            // A shift of a reassociable multiply, or used by a reassociable
            // multiply or add: a multiply by a constant.
            let new = self.convert_shift_to_mul(i);
            self.redo.insert(i);
            self.changed = true;
            i = new;
        }

        // Commute to canonicalize the order of the operands: more CSE, and
        // simpler transformations.
        if self.is_commutative(i) {
            self.canonicalize_operands(i);
        }

        // Negative constants out of expressions.
        i = self.canonicalize_neg_fp_constants(i);

        // A floating operation without the flags that allow it is left alone.
        if self.is_fp_math(i) && !self.has_fp_associative_flags(i) {
            return;
        }

        // Not boolean expressions: the order of evaluation of short-circuited
        // comparisons that SimplifyCFG folded is kept.
        if self.context.types.int_bits(self.function.instruction(i).ty) == Some(1) {
            return;
        }

        // An `or` of operands with no common bit is an add.
        if self.binop(i) == Some(BinaryOp::Or) && self.should_convert_or_with_no_common_bits_to_add(i) {
            let new = self.convert_or_with_no_common_bits_to_add(i);
            self.redo.insert(i);
            self.changed = true;
            i = new;
        }

        if self.binop(i) == Some(BinaryOp::Mul) && self.should_break_up_distribution(i) {
            let user = self.only_user(self.result(i)).expect("one user");
            let new = self.break_up_distribute(i);
            self.redo.insert(i);
            self.redo.insert(user);
            self.changed = true;
            i = new;
        }

        // A subtract not in negate form joining an add tree is `X + -Y`.
        if self.binop(i) == Some(BinaryOp::Sub) {
            if self.should_break_up_subtract(i) {
                let new = self.break_up_subtract(i);
                self.redo.insert(i);
                self.changed = true;
                i = new;
            } else if self.match_neg(self.result(i)).is_some() {
                // A negation of a multiply tree that is not an inner node of
                // one.
                i = self.lower_negate_of_multiply_tree(i, BinaryOp::Mul, 1).unwrap_or(i);
            }
        } else if self.function.instruction(i).opcode == Opcode::FNeg || self.binop(i) == Some(BinaryOp::FSub) {
            if self.should_break_up_subtract(i) {
                let new = self.break_up_subtract(i);
                self.redo.insert(i);
                self.changed = true;
                i = new;
            } else if self.match_fneg(self.result(i)).is_some() {
                let index = usize::from(self.binop(i).is_some());
                i = self.lower_negate_of_multiply_tree(i, BinaryOp::FMul, index).unwrap_or(i);
            }
        }

        // An associative operator is processed at the root of its tree.
        if !self.is_associative(i) {
            return;
        }
        let value = self.result(i);
        let opcode = self.function.instruction(i).opcode.clone();
        // An interior node waits for the root, so as not to analyze a tree
        // more than once. When redoing there is no guarantee that the root is
        // visited, so it is queued.
        if self.has_one_use(value) {
            let user = self.only_user(value).expect("one user");
            if self.function.instruction(user).opcode == opcode {
                if user != i && self.function.parent(user) == self.function.parent(i) {
                    self.redo.insert(user);
                }
                return;
            }
            // An add tree used by a subtract waits for the subtract.
            let user_op = self.binop(user);
            if (self.binop(i) == Some(BinaryOp::Add) && user_op == Some(BinaryOp::Sub))
                || (self.binop(i) == Some(BinaryOp::FAdd) && user_op == Some(BinaryOp::FSub))
            {
                return;
            }
        }
        self.reassociate_expression(i);
    }

    /// The negate `neg` of a one-use multiply tree that is not itself in one
    /// is `x * -1`: the new multiply, if made.
    fn lower_negate_of_multiply_tree(
        &mut self,
        neg: InstId,
        mul: BinaryOp,
        operand_index: usize,
    ) -> Option<InstId> {
        let operand = self.operand(neg, operand_index);
        let value = self.result(neg);
        if self.reassociable(operand, &[mul]).is_none()
            || self.only_user_value(value).is_some_and(|user| self.reassociable(user, &[mul]).is_some())
        {
            return None;
        }
        let new = self.lower_negate_to_multiply(neg);
        // The negate was simplified: its users may reassociate further.
        let users: Vec<InstId> = match self.result(new) {
            Operand::Value(v) => self.function.users(v).iter().map(|one| one.user).collect(),
            _ => Vec::new(),
        };
        for user in users {
            if self.binop(user).is_some() {
                self.redo.insert(user);
            }
        }
        self.redo.insert(neg);
        self.changed = true;
        Some(new)
    }

    /// `ReassociateExpression`: linearize the tree rooted at `i`, sort and
    /// optimize its leaves, and write them back.
    fn reassociate_expression(
        &mut self,
        i: InstId,
    ) {
        let mut flags = OverflowTracking::default();
        let (tree, changed) = self.linearize_expr_tree(i, &mut flags);
        self.changed |= changed;
        let mut ops = Vec::new();
        for (leaf, weight) in tree {
            let entry = self.entry(leaf);
            for _ in 0..weight {
                ops.push(entry);
            }
        }

        // Highest rank first; stable, so equal ranks keep their order.
        ops.sort_by(|a, b| b.rank.cmp(&a.rank));

        // Optimize the sorted list as a whole.
        if let Some(v) = self.optimize_expression(i, &mut ops) {
            if v == self.result(i) {
                // A self-referential expression in unreachable code.
                return;
            }
            // The tree is not a tree any more: eliminate it.
            self.replace_all_uses(i, v);
            self.redo.insert(i);
            return;
        }

        // Sink immediates as deeply as possible, except for a multiply tree
        // used only by an add with -1 last: keep the negation outside, to fold
        // into the add: `(-X)*Y + Z` is `Z - X*Y`.
        let value = self.result(i);
        if self.has_one_use(value) {
            let user_op = self.only_user(value).and_then(|user| self.binop(user));
            let last = ops.last().map(|entry| entry.op);
            let minus_one = |this: &Self, v: Option<Operand>| match v {
                Some(v) if this.is_float(this.type_of(v)) => float_value(this.context, v) == Some(-1.0),
                Some(v) => this.int_constant(v).is_some_and(|(bits, width)| bits == llrm_mir::context::mask(width)),
                None => false,
            };
            let hoist = (self.binop(i) == Some(BinaryOp::Mul)
                && user_op == Some(BinaryOp::Add)
                && minus_one(self, last))
                || (self.binop(i) == Some(BinaryOp::FMul) && user_op == Some(BinaryOp::FAdd) && minus_one(self, last));
            if hoist {
                let last = ops.pop().expect("a leaf");
                ops.insert(0, last);
            }
        }

        if ops.len() == 1 {
            if ops[0].op == value {
                // A self-referential expression in unreachable code.
                return;
            }
            self.replace_all_uses(i, ops[0].op);
            self.redo.insert(i);
            return;
        }

        if ops.len() > 2 && ops.len() <= GLOBAL_REASSOCIATE_LIMIT {
            self.move_most_popular_pair_to_back(i, &mut ops);
        }

        // Splat the sorted, optimized leaves back into the tree, dropping the
        // nodes that are not needed.
        self.rewrite_expr_tree(i, &ops, flags);
    }
}
