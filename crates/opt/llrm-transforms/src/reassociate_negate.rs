//! Reassociate's subtractions and negations: forms that let a subtract or a
//! negate join an add or multiply tree.
//!
//! Translation of `NegateValue`, `ShouldBreakUpSubtract`, `BreakUpSubtract`
//! and `LowerNegateToMultiply`
//! ([Reassociate.cpp:343](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L343),
//! [:820](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L820),
//! [:1081](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1081),
//! [:1111](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1111))
//! and the negative floating-point constant canonicalization
//! (:2165-:2288) at d1106deb71cc.
//!
//! MIRROR: a negate is pushed through a one-use add into the leaves, an
//! existing negate of a leaf is reused (moved to just after the leaf), a new
//! one is made otherwise; a subtract becomes an add of a negate when that
//! joins an add or subtract tree.
//!
//! DEVIATE 1: `NegateValue` is an explicit stack, not recursion: a chain of
//! one-use adds is as deep as the function is long. The order in which
//! negates are placed and operands set is upstream's.
//!
//! DEVIATE 2: a leaf that is an `invoke` result keeps no reusable negate (the
//! insertion point after its definition is in another block).
//!
//! DEVIATE 4 (of `reassociate.rs`): MIR has no constant `sub`, so
//! `ConstantExpr::getNeg` of a constant expression (`ptrtoint (ptr @g to i32)`)
//! is a `sub 0, C` instruction before the instruction negated, not a constant
//! operand. It is used by the add that replaced the subtract (bench row
//! x_strlen: the pin prints the constant `sub (i32 0, ptrtoint ...)` there).
//! Exit: a constant `sub` in MIR.
//!
//! Complexity (not a DEVIATE): `NegateValue` recurses into both operands of
//! every one-use add under the value it negates
//! ([:840-:862](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L840)),
//! and `ShouldBreakUpSubtract` breaks up a subtract whose operand is such an
//! add
//! ([:1093-:1099](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1093)),
//! so in a chain of n right-nested subtracts (`a0 - (a1 - (a2 - ...))`) each
//! break-up negates the tree the last one made: O(n^2), upstream's. A
//! left-nested chain takes O(n) (`reassociate_tests.rs` times it).
//!
//! # Used by
//! `crate::reassociate_tree` (`lower_negate_to_multiply`) and
//! `crate::reassociate` (`OptimizeInst`).
//!
//! # Example
//! `a - (b + c)` becomes `a + (-b + -c)`; `-(x * y)` becomes `x * y * -1`.

use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::reassociate::Reassociator;
use crate::reassociate_fp::{float_constant, float_value};

/// One step of the iterative `NegateValue`.
enum Step {
    /// Negate this value, leaving the result on the stack.
    Visit(Operand),
    /// Both operands of the add are negated: set them and move the add.
    Finish(InstId),
}

impl Reassociator<'_> {
    /// `ConstantExpr::getNeg` / `ConstantFoldUnaryOpOperand(FNeg)` of a
    /// constant.
    fn negate_constant(
        &self,
        v: Operand,
    ) -> Option<Operand> {
        let ty = self.type_of(v);
        if self.is_float(ty) {
            return float_value(self.context, v).map(|x| float_constant(self.context, ty, -x));
        }
        let (bits, _) = self.int_constant(v)?;
        Some(Operand::Constant(self.context.int(ty, (bits as i128).wrapping_neg())))
    }

    /// `CreateNeg(S1, Name, InsertBefore, FlagsOp)`: `sub 0, v` or `fneg v`
    /// with the fast-math flags of `flags_op`.
    pub fn create_neg(
        &mut self,
        v: Operand,
        at: Position,
        flags_op: InstId,
    ) -> InstId {
        let ty = self.type_of(v);
        if self.is_float(ty) {
            let fast = self.flags(flags_op).intersect(Flags::FAST);
            self.insert_at(at, Opcode::FNeg, ty, vec![v], fast)
        } else {
            let zero = Operand::Constant(self.context.int(ty, 0));
            self.insert_at(at, Opcode::Binary(BinaryOp::Sub), ty, vec![zero, v], Flags::default())
        }
    }

    /// `CreateAdd(S1, S2, Name, InsertBefore, FlagsOp)`.
    pub fn create_add(
        &mut self,
        a: Operand,
        b: Operand,
        at: InstId,
        flags_op: InstId,
    ) -> InstId {
        let ty = self.type_of(a);
        if self.is_float(ty) {
            let fast = self.flags(flags_op).intersect(Flags::FAST);
            self.insert_before(at, Opcode::Binary(BinaryOp::FAdd), ty, vec![a, b], fast)
        } else {
            self.insert_before(at, Opcode::Binary(BinaryOp::Add), ty, vec![a, b], Flags::default())
        }
    }

    /// `CreateMul(S1, S2, Name, InsertBefore, FlagsOp)`.
    pub fn create_mul(
        &mut self,
        a: Operand,
        b: Operand,
        at: InstId,
        flags_op: InstId,
    ) -> InstId {
        let ty = self.type_of(a);
        if self.is_float(ty) {
            let fast = self.flags(flags_op).intersect(Flags::FAST);
            self.insert_before(at, Opcode::Binary(BinaryOp::FMul), ty, vec![a, b], fast)
        } else {
            self.insert_before(at, Opcode::Binary(BinaryOp::Mul), ty, vec![a, b], Flags::default())
        }
    }

    /// `LowerNegateToMultiply`: `0 - x` is `x * -1`.
    pub fn lower_negate_to_multiply(
        &mut self,
        neg: InstId,
    ) -> InstId {
        let index = usize::from(self.binop(neg).is_some());
        let ty = self.function.instruction(neg).ty;
        let negative_one = if self.is_float(ty) {
            float_constant(self.context, ty, -1.0)
        } else {
            Operand::Constant(self.context.int(ty, -1))
        };
        let x = self.operand(neg, index);
        let mul = self.create_mul(x, negative_one, neg, neg);
        let zero = self.zero_of(ty);
        self.function.set_operand(neg, index, zero);
        let (from, to) = (
            self.function.instruction(neg).result.expect("a value"),
            self.function.instruction(mul).result.expect("a value"),
        );
        self.function.take_name(from, to);
        self.replace_all_uses(neg, Operand::Value(to));
        mul
    }

    /// `NegateValue(V, BI, ToRedo)`: the negation of `v`, placed before `bi`.
    pub fn negate_value(
        &mut self,
        v: Operand,
        bi: InstId,
    ) -> Operand {
        let mut steps = vec![Step::Visit(v)];
        let mut negated: Vec<Operand> = Vec::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Visit(v) => {
                    if let Operand::Constant(c) = v {
                        negated.push(match self.negate_constant(v) {
                            Some(folded) => folded,
                            // `-undef` is undef, `-poison` poison.
                            None if self.is_undef_or_poison(c) => v,
                            None => self.negate_leaf(v, bi),
                        });
                    } else if let Some(add) = self.reassociable(v, &[BinaryOp::Add, BinaryOp::FAdd]) {
                        steps.push(Step::Finish(add));
                        steps.push(Step::Visit(self.operand(add, 1)));
                        steps.push(Step::Visit(self.operand(add, 0)));
                    } else {
                        negated.push(self.negate_leaf(v, bi));
                    }
                }
                Step::Finish(add) => {
                    // Push the negates through the add.
                    let right = negated.pop().expect("the right operand's negation");
                    let left = negated.pop().expect("the left operand's negation");
                    self.function.set_operand(add, 0, left);
                    self.function.set_operand(add, 1, right);
                    if self.binop(add) == Some(BinaryOp::Add) {
                        let flags = self.flags(add).without(Flags::NUW).without(Flags::NSW);
                        self.function.set_flags(add, flags);
                    }
                    // The negates just made do not dominate the old add; moving
                    // it assures they dominate the instruction made next.
                    self.function.move_to(add, Position::Before(bi)).expect("placed instructions");
                    self.redo.insert(add);
                    negated.push(self.result(add));
                }
            }
        }
        negated.pop().expect("the negation")
    }

    /// The rest of `NegateValue`, for a `v` that is not an add to push
    /// through: an existing negate of it, moved to just after its definition,
    /// else a new one before `bi`.
    fn negate_leaf(
        &mut self,
        v: Operand,
        bi: InstId,
    ) -> Operand {
        // Scan the use list of `v` for a negate of it.
        let found = self.negate_of(v);
        if let Some(the_neg) = found {
            let at = self.insertion_point_after_def(v).expect("checked");
            self.function.move_to(the_neg, at).expect("placed instructions");
            if self.binop(the_neg) == Some(BinaryOp::Sub) {
                let flags = self.flags(the_neg).without(Flags::NUW).without(Flags::NSW);
                self.function.set_flags(the_neg, flags);
            } else {
                // `andIRFlags(BI)`.
                let kept = self.flags(the_neg).intersect(self.flags(bi)).intersect(Flags::FAST);
                let flags = self.flags(the_neg).without(Flags::FAST).union(kept);
                self.function.set_flags(the_neg, flags);
            }
            self.redo.insert(the_neg);
            return self.result(the_neg);
        }
        // Insert a `neg` that subtracts the value from zero.
        let new_neg = self.create_neg(v, Position::Before(bi), bi);
        self.note_constant_negate(new_neg);
        self.redo.insert(new_neg);
        self.result(new_neg)
    }

    /// Record `neg` as a user of the constant it negates.
    pub fn note_constant_negate(
        &mut self,
        neg: InstId,
    ) {
        let result = self.result(neg);
        let negated =
            pattern::neg(&self.query(), result).map(|(_, x, _)| x).or_else(|| pattern::fneg(&self.query(), result));
        if let Some(Operand::Constant(constant)) = negated {
            self.constant_negates.get_or_insert_with(constant.index(), Vec::new).push(neg);
        }
    }

    /// The first user of `v` that negates it and can be moved after the
    /// definition of `v`.
    fn negate_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let query = self.query();
        let negates = |user: InstId| {
            let result = self.function.instruction(user).result.map(Operand::Value);
            result.is_some_and(|r| pattern::neg(&query, r).is_some() || pattern::fneg(&query, r).is_some())
                && self.insertion_point_after_def(v).is_some()
        };
        match v {
            Operand::Value(value) => self.function.users(value).iter().map(|one| one.user).find(|&user| negates(user)),
            // A constant has no use list here: the negates of it are kept in
            // `constant_negates`, the newest first as upstream's list is.
            Operand::Constant(constant) => self
                .constant_negates
                .get(&constant.index())?
                .iter()
                .rev()
                .copied()
                .find(|&user| self.function.parent(user).is_some() && negates(user)),
            Operand::Block(_) => None,
        }
    }

    /// `getInsertionPointAfterDef` for the definition of `v`, or the first
    /// non-phi of the entry block for an argument; none where there is no
    /// place after the definition in its block (an `invoke`).
    fn insertion_point_after_def(
        &self,
        v: Operand,
    ) -> Option<Position> {
        let Operand::Value(value) = v else {
            // Not an instruction: the first non-phi of the entry block.
            let first = self
                .function
                .iter_block(self.function.entry()?)
                .find(|&inst| self.function.instruction(inst).opcode != Opcode::Phi)?;
            return Some(Position::Before(first));
        };
        match self.function.value(value).def {
            ValueDef::Argument(_) => {
                let entry = self.function.entry()?;
                let first = self
                    .function
                    .iter_block(entry)
                    .find(|&inst| self.function.instruction(inst).opcode != Opcode::Phi)?;
                Some(Position::Before(first))
            }
            ValueDef::Instruction(inst) => {
                let block = self.function.parent(inst)?;
                match self.function.instruction(inst).opcode {
                    Opcode::Invoke(_) => None,
                    Opcode::Phi => {
                        let first = self
                            .function
                            .iter_block(block)
                            .find(|&inst| self.function.instruction(inst).opcode != Opcode::Phi)?;
                        Some(Position::Before(first))
                    }
                    _ => Some(Position::After(inst)),
                }
            }
        }
    }

    /// `ShouldBreakUpSubtract`: `sub` is not a negation and joins an add or
    /// subtract tree.
    pub fn should_break_up_subtract(
        &self,
        sub: InstId,
    ) -> bool {
        let value = self.result(sub);
        if self.match_neg(value).is_some() || self.match_fneg(value).is_some() {
            return false;
        }
        // Don't break up `X - undef`.
        if matches!(self.operand(sub, 1), Operand::Constant(c) if self.is_undef(c)) {
            return false;
        }
        let tree = [BinaryOp::Add, BinaryOp::FAdd, BinaryOp::Sub, BinaryOp::FSub];
        let joins = |v: Operand| self.reassociable(v, &tree).is_some();
        if joins(self.operand(sub, 0)) || joins(self.operand(sub, 1)) {
            return true;
        }
        self.only_user_value(value).is_some_and(joins)
    }

    /// `BreakUpSubtract`: `x - y` is `x + (0 - y)`.
    pub fn break_up_subtract(
        &mut self,
        sub: InstId,
    ) -> InstId {
        let negative = self.negate_value(self.operand(sub, 1), sub);
        let new = self.create_add(self.operand(sub, 0), negative, sub, sub);
        let ty = self.function.instruction(sub).ty;
        let zero = self.zero_of(ty);
        // Drop the uses of the operands.
        self.function.set_operand(sub, 0, zero);
        self.function.set_operand(sub, 1, zero);
        let (from, to) = (
            self.function.instruction(sub).result.expect("a value"),
            self.function.instruction(new).result.expect("a value"),
        );
        self.function.take_name(from, to);
        self.replace_all_uses(sub, Operand::Value(to));
        new
    }

    fn is_undef(
        &self,
        c: llrm_mir::context::ConstantId,
    ) -> bool {
        matches!(
            self.context.get(c).kind,
            llrm_mir::context::ConstantKind::Undef
        )
    }

    fn is_undef_or_poison(
        &self,
        c: llrm_mir::context::ConstantId,
    ) -> bool {
        matches!(
            self.context.get(c).kind,
            llrm_mir::context::ConstantKind::Undef | llrm_mir::context::ConstantKind::Poison
        )
    }
}
