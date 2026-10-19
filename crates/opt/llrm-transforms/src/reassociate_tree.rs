//! Reassociate's expression trees: collect the leaves of a tree of one
//! associative operation with their weights, and write a new leaf list back
//! into the tree's own nodes.
//!
//! Translation of `LinearizeExprTree` and `RewriteExprTree`
//! ([Reassociate.cpp:436](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L436),
//! [:626](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L626)),
//! `OverflowTracking` (Local.h:581) and `isFMulAddCandidate` (:189) at
//! d1106deb71cc.
//!
//! MIRROR: a node of the tree has one use; a value met again with all its
//! uses inside the tree is morphed (a `neg` in a multiply tree becomes a
//! multiply by -1), not a leaf. Nodes the optimizers leave unused are reused
//! for the rewritten tree before a new one is made, and the flags that no
//! longer hold are cleared from the first changed node up to the root.
//!
//! # How it works
//! `linearize_expr_tree` is one pass over the tree: O(size).
//! `rewrite_expr_tree` walks the new leaf list once, O(leaves).
//!
//! # Used by
//! `crate::reassociate` (`ReassociateExpression`) and
//! `crate::reassociate_optimize` (`RemoveFactorFromExpression`).
//!
//! # Example
//! `((a + b) + c) + a` linearizes to the leaves `a` (weight 2), `b`, `c`.

use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::valuetracking::known::is_known_non_negative;
use llrm_mir::valuetracking::nonzero::is_known_non_zero;

use crate::reassociate::{OperandMap, Reassociator, ValueEntry};

/// `OverflowTracking`: what the flags of every node of a tree allow the
/// rewritten tree to claim.
#[derive(Clone, Copy)]
pub(crate) struct OverflowTracking {
    pub has_nuw: bool,
    pub has_nsw: bool,
    pub is_disjoint: bool,
    pub all_known_non_negative: bool,
    pub all_known_non_zero: bool,
}

impl Default for OverflowTracking {
    fn default() -> Self {
        Self { has_nuw: true, has_nsw: true, is_disjoint: true, all_known_non_negative: true, all_known_non_zero: true }
    }
}

impl OverflowTracking {
    /// `mergeFlags`: `op` with `flags` is a node of the tree.
    fn merge_flags(
        &mut self,
        op: BinaryOp,
        flags: Flags,
    ) {
        if matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
        ) {
            self.has_nuw &= flags.contains(Flags::NUW);
            self.has_nsw &= flags.contains(Flags::NSW);
        }
        if op == BinaryOp::Or {
            self.is_disjoint &= flags.contains(Flags::DISJOINT);
        }
    }

    /// `applyFlags`: the flags `op` may have, after the poison-generating ones
    /// are dropped.
    pub fn flags_for(
        &self,
        op: BinaryOp,
    ) -> Flags {
        let mut flags = Flags::default();
        if op == BinaryOp::Add || (op == BinaryOp::Mul && self.all_known_non_zero) {
            if self.has_nuw {
                flags.insert(Flags::NUW);
            }
            if self.has_nsw && (self.all_known_non_negative || self.has_nuw) {
                flags.insert(Flags::NSW);
            }
        }
        if op == BinaryOp::Or && self.is_disjoint {
            flags.insert(Flags::DISJOINT);
        }
        flags
    }
}

impl Reassociator<'_> {
    /// `isFMulAddCandidate`: a one-use `fadd` with a one-use contractible
    /// `fmul` operand, kept whole so the pair can become one `fma`.
    pub fn is_fmul_add_candidate(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let fadd = self.reassociable(v, &[BinaryOp::FAdd])?;
        if !self.flags(fadd).contains(Flags::CONTRACT) {
            return None;
        }
        let contractible_fmul = |operand: Operand| {
            self.inst_of(operand).filter(|&inst| {
                self.binop(inst) == Some(BinaryOp::FMul)
                    && self.flags(inst).contains(Flags::CONTRACT)
                    && self.has_one_use(operand)
            })
        };
        let (mul, other) = match contractible_fmul(self.operand(fadd, 0)) {
            Some(mul) => (mul, self.operand(fadd, 1)),
            None => (contractible_fmul(self.operand(fadd, 1))?, self.operand(fadd, 0)),
        };
        // Constants, nested additions and other contractible multiplies stay
        // visible to the enclosing expression.
        let other_is_fmul = self
            .inst_of(other)
            .is_some_and(|inst| self.binop(inst) == Some(BinaryOp::FMul) && self.flags(inst).contains(Flags::CONTRACT));
        (!self.is_constant(other) && self.reassociable(other, &[BinaryOp::FAdd]).is_none() && !other_is_fmul)
            .then_some(mul)
    }

    /// `LinearizeExprTree`: the leaves of the tree rooted at `root` with their
    /// weights, and whether the function changed. A `true` obliges the caller
    /// to write a tree back (see the header).
    pub fn linearize_expr_tree(
        &mut self,
        root: InstId,
        flags: &mut OverflowTracking,
    ) -> (Vec<(Operand, u64)>, bool) {
        let opcode = self.binop(root).expect("a binary operator");
        let mut worklist: Vec<(InstId, u64)> = vec![(root, 1)];
        let mut changed = false;
        let mut leaves: OperandMap<u64> = OperandMap::new();
        let mut leaf_order: Vec<Operand> = Vec::new();

        while let Some((inst, mut weight)) = worklist.pop() {
            self.tick();
            flags.merge_flags(self.binop(inst).expect("a binary operator"), self.flags(inst));

            for index in 0..self.function.instruction(inst).operands.len() {
                self.tick();
                let op = self.operand(inst, index);

                // A one-use operator of the right kind: its operands join.
                if let Some(node) = self.reassociable(op, &[opcode])
                    && (opcode != BinaryOp::FAdd || self.is_fmul_add_candidate(op).is_none())
                {
                    worklist.push((node, weight));
                    continue;
                }

                // Appears to be a leaf. Is it already in the set?
                if let Some(seen) = leaves.get_mut(op) {
                    *seen += weight;
                    // Uses the expression does not account for: not safe to
                    // modify.
                    if !self.has_one_use(op) {
                        continue;
                    }
                    weight = *seen;
                    leaves.remove(op);
                } else if !self.has_one_use(op) {
                    leaf_order.push(op);
                    leaves.insert(op, weight);
                    continue;
                }

                // Used only inside the expression: try to morph it into the
                // right kind. In a multiply tree a negation is a multiply by
                // -1.
                if let Some(neg) = self.inst_of(op)
                    && ((opcode == BinaryOp::Mul && self.match_neg(op).is_some())
                        || (opcode == BinaryOp::FMul && self.match_fneg(op).is_some()))
                {
                    let mul = self.lower_negate_to_multiply(neg);
                    worklist.push((mul, weight));
                    let result = self.result(mul);
                    let users: Vec<InstId> = match result {
                        Operand::Value(value) => self.function.users(value).iter().map(|one| one.user).collect(),
                        _ => Vec::new(),
                    };
                    for user in users {
                        if self.binop(user).is_some() {
                            self.redo.insert(user);
                        }
                    }
                    self.redo.insert(neg);
                    changed = true;
                    continue;
                }

                leaf_order.push(op);
                leaves.insert(op, weight);
            }
        }

        // The leaves, repeated by weight, are the linearized form.
        let mut ops: Vec<(Operand, u64)> = Vec::new();
        for v in leaf_order {
            let Some(slot) = leaves.get_mut(v) else { continue };
            let weight = std::mem::take(slot);
            ops.push((v, weight));
            self.track_leaf(opcode, v, flags);
        }

        // For nilpotent operations or addition there may be no operands, for
        // example because the expression was `X xor X`: the weight reduced to
        // 0 and the value was skipped.
        if ops.is_empty() {
            let ty = self.function.instruction(root).ty;
            ops.push((self.bin_op_identity(opcode, ty), 1));
        }
        (ops, changed)
    }

    /// What a leaf `v` of an `opcode` tree takes from the flags the tree may
    /// claim.
    fn track_leaf(
        &self,
        opcode: BinaryOp,
        v: Operand,
        flags: &mut OverflowTracking,
    ) {
        let query = self.query();
        if opcode == BinaryOp::Add && flags.all_known_non_negative && flags.has_nsw {
            flags.all_known_non_negative &= is_known_non_negative(&query, v, 0);
        } else if opcode == BinaryOp::Mul {
            // To keep NUW every input must be non-zero, to keep NSW strictly
            // positive.
            if flags.all_known_non_zero && (flags.has_nuw || (flags.has_nsw && flags.all_known_non_negative)) {
                flags.all_known_non_zero &= is_known_non_zero(&query, v, 0);
                if flags.has_nsw && flags.all_known_non_negative {
                    flags.all_known_non_negative &= is_known_non_negative(&query, v, 0);
                }
            }
        }
    }

    /// `RewriteExprTree`: write `ops` (two or more) into the tree rooted at
    /// `root`, in order, from the leaf-most node to the root.
    pub fn rewrite_expr_tree(
        &mut self,
        root: InstId,
        ops: &[ValueEntry],
        flags: OverflowTracking,
    ) {
        assert!(ops.len() > 1, "single values should be used directly");
        let opcode = self.binop(root).expect("a binary operator");
        let mut nodes_to_rewrite: Vec<InstId> = Vec::new();
        let mut op = root;

        // The operands being written are the leaves of the new tree and must
        // not be reused as inner nodes by mistake.
        let mut not_rewritable = llrm_mir::dense::IdSet::<ValueId>::new();
        for entry in ops {
            if let Operand::Value(value) = entry.op {
                not_rewritable.insert(value);
            }
        }
        let rewritable = |this: &Self, v: Operand| {
            this.reassociable(v, &[opcode]).filter(|&node| {
                !matches!(
                    this.result(node),
                    Operand::Value(value) if not_rewritable.contains(&value)
                )
            })
        };

        // Non-null when the new tree differs from the old in more than operand
        // order, so flags must be cleared from `start` up to `end` inclusive.
        let mut expression_changed_start: Option<InstId> = None;
        let mut expression_changed_end: Option<InstId> = None;
        let mut i = 0;
        loop {
            self.tick();
            // The last operation (the earliest in the IR) takes both operands
            // from `ops`.
            if i + 2 == ops.len() {
                let (new_lhs, new_rhs) = (ops[i].op, ops[i + 1].op);
                let (old_lhs, old_rhs) = (self.operand(op, 0), self.operand(op, 1));
                if new_lhs == old_lhs && new_rhs == old_rhs {
                    break;
                }
                if new_lhs == old_rhs && new_rhs == old_lhs {
                    self.swap_operands(op);
                    self.changed = true;
                    break;
                }
                if new_lhs != old_lhs {
                    if let Some(node) = rewritable(self, old_lhs) {
                        nodes_to_rewrite.push(node);
                    }
                    self.function.set_operand(op, 0, new_lhs);
                }
                if new_rhs != old_rhs {
                    if let Some(node) = rewritable(self, old_rhs) {
                        nodes_to_rewrite.push(node);
                    }
                    self.function.set_operand(op, 1, new_rhs);
                }
                expression_changed_start = Some(op);
                expression_changed_end.get_or_insert(op);
                self.changed = true;
                break;
            }

            // Not the last operation: the left-hand side is a sub-expression,
            // the right-hand side the current element.
            let new_rhs = ops[i].op;
            if new_rhs != self.operand(op, 1) {
                if new_rhs == self.operand(op, 0) {
                    // Already the left operand: swapping may sort out both.
                    self.swap_operands(op);
                } else {
                    if let Some(node) = rewritable(self, self.operand(op, 1)) {
                        nodes_to_rewrite.push(node);
                    }
                    self.function.set_operand(op, 1, new_rhs);
                    expression_changed_start = Some(op);
                    expression_changed_end.get_or_insert(op);
                }
                self.changed = true;
            }

            // The left-hand side: a node of the original tree takes the rest.
            if let Some(node) = rewritable(self, self.operand(op, 0)) {
                op = node;
                i += 1;
                continue;
            }

            // Otherwise a spare node of the original tree, or a new one: the
            // optimizers may make a tree with more nodes than the original.
            let new_op = match nodes_to_rewrite.pop() {
                Some(spare) => spare,
                None => {
                    let ty = self.function.instruction(root).ty;
                    let poison = self.poison_of(ty);
                    let fast = self.flags(root).intersect(Flags::FAST);
                    self.insert_before(root, Opcode::Binary(opcode), ty, vec![poison, poison], fast)
                }
            };
            let new_value = self.result(new_op);
            self.function.set_operand(op, 0, new_value);
            expression_changed_start = Some(op);
            expression_changed_end.get_or_insert(op);
            self.changed = true;
            op = new_op;
            i += 1;
        }

        // A tree that changed in more than order loses its optional flags from
        // the changed node to the root, and its nodes are packed just before
        // the root so that the tree is dominated by every leaf.
        if let Some(mut node) = expression_changed_start {
            let mut clear_flags = true;
            loop {
                if clear_flags {
                    self.apply_flags(root, node, &flags);
                }
                if Some(node) == expression_changed_end {
                    clear_flags = false;
                }
                if node == root {
                    break;
                }
                self.function.move_to(node, Position::Before(root)).expect("placed instructions");
                let result = self.result(node);
                node = self.only_user(result).expect("a node of the tree has one use");
            }
        }

        // Throw away any left over nodes from the original expression.
        for node in nodes_to_rewrite {
            self.redo.insert(node);
        }
    }

    /// The flags of `node`, a node of the rewritten tree rooted at `root`: a
    /// floating tree takes the root's fast-math flags, an integer one what the
    /// tree can prove.
    fn apply_flags(
        &mut self,
        root: InstId,
        node: InstId,
        flags: &OverflowTracking,
    ) {
        let op = self.binop(node).expect("a binary operator");
        let new = if self.is_fp_math(root) {
            self.flags(node).without(Flags::FAST).union(self.flags(root).intersect(Flags::FAST))
        } else {
            self.function.drop_poison_generating_annotations(node);
            self.flags(node).without(Flags::POISON_GENERATING).union(flags.flags_for(op))
        };
        self.function.set_flags(node, new);
    }

    /// `swapOperands` of a commutative binary operator.
    pub fn swap_operands(
        &mut self,
        inst: InstId,
    ) {
        let (a, b) = (self.operand(inst, 0), self.operand(inst, 1));
        self.function.set_operand(inst, 0, b);
        self.function.set_operand(inst, 1, a);
    }
}
