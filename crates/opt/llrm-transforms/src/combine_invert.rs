//! InstCombine's `getFreelyInverted` (`getFreelyInvertedImpl` of
//! InstructionCombining.cpp at d1106deb71cc): the `~V` of a value whose
//! inversion costs nothing, when every use of it is being rewritten to read
//! `~V`.
//!
//! MIRROR: the order of the cases and each condition, for the `not`, an
//! integer constant, a compare, `add`, `xor`, `sub`, `ashr`, `sext` and
//! `trunc`.
//!
//! OWED, in place as `// OWED:` comments: the select and min/max case (it
//! needs `shouldAvoidAbsorbingNotIntoSelect`), the phi case, and De Morgan's
//! laws. A value they would invert is not freely invertible here, so a fold
//! that asks finds less than the pin, never more.
//!
//! # How it works
//! [`Combiner::get_freely_inverted`] with `build` false only asks (it returns
//! some value when the answer is yes); with `build` it makes the `~V`
//! before `at`. `consumes` is set when the inversion eats an instruction (a
//! `not` that is dropped).
//!
//! # Used by
//! The equality folds of `combine_icmp_binop.rs` that turn an `and` of a freely
//! invertible operand into an `or` of its inverse.
//!
//! # Example
//! `~(x + 5)` is `-6 - x`:
//!
//! ```text
//! %x = add i32 %p, 5      ; ~%x = sub i32 -6, %p
//! ```

use llrm_mir::edit::Position;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::Combiner;
use crate::combine_shared::{has_one_use, query};

/// `MaxAnalysisRecursionDepth`.
const MAX_DEPTH: u32 = 6;

impl Combiner<'_> {
    /// [`canFreelyInvertAllUsersOf`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/InstCombine/InstCombiner.h#L267):
    /// every user of `v` but
    /// `ignored` can take the inverted value: a select condition (unless the
    /// select is a logical and/or, whose pattern a swap would hide), a branch
    /// or a `not`.
    pub(crate) fn can_freely_invert_all_users_of(
        &self,
        v: InstId,
        ignored: Option<InstId>,
    ) -> bool {
        let Some(result) = self.function.instruction(v).result else { return false };
        self.function
            .users(result)
            .iter()
            .all(
                |one| {
                    if Some(one.user) == ignored {
                        return true;
                    }
                    let user = self.function.instruction(one.user);
                    let Some(user_value) = user.result.map(Operand::Value) else {
                        return matches!(user.opcode, Opcode::Br) && user.operands.len() == 3;
                    };
                    let q = query(self, None);
                    match &user.opcode {
                        Opcode::Select => {
                            one.index == 0
                                && pattern::logical_and(&q, user_value).is_none()
                                && pattern::logical_or(&q, user_value).is_none()
                        }
                        Opcode::Binary(BinaryOp::Xor) => pattern::not(&q, user_value).is_some(),
                        _ => false,
                    }
                },
            )
    }

    /// [`freelyInvertAllUsersOf`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1455):
    /// the users of `v` but
    /// `ignored` take the inverted value: a select swaps its arms, a branch
    /// its successors, a `not` is replaced by `v`.
    pub(crate) fn freely_invert_all_users_of(
        &mut self,
        v: Operand,
        ignored: Option<InstId>,
    ) {
        let Operand::Value(value) = v else { return };
        let users: Vec<InstId> = self.function.users(value).iter().map(|one| one.user).collect();
        for user in users {
            if Some(user) == ignored {
                continue;
            }
            match self.function.instruction(user).opcode {
                Opcode::Select | Opcode::Br => {
                    let mut operands = self.function.instruction(user).operands.clone();
                    operands.swap(1, 2);
                    self.function.set_operands(user, operands);
                }
                _ => self.replace_inst_with(user, v),
            }
            self.add_to_worklist(user);
        }
    }

    /// [`canFreelyInvert`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5111).
    ///
    /// DEVIATE: a phi has no insertion point here (`getInsertionPointAfterDef`
    /// skips the phis), so it is not freely invertible. Exit: the first user
    /// that needs one.
    pub(crate) fn can_freely_invert(
        &mut self,
        op: Operand,
        ignored: InstId,
    ) -> bool {
        let Some(def) = self.definition_of(op) else { return false };
        if self.function.instruction(def).opcode == Opcode::Phi {
            return false;
        }
        let mut consumes = false;
        self.get_freely_inverted(ignored, op, true, false, &mut consumes).is_some()
            && self.can_freely_invert_all_users_of(def, Some(ignored))
    }

    /// [`freelyInvert`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5119):
    /// a `not` of `op` right after its definition takes every other use
    /// of `op`, and the users of that `not` are inverted.
    pub(crate) fn freely_invert(
        &mut self,
        op: Operand,
        ignored: InstId,
    ) -> Operand {
        let def = self.definition_of(op).expect("an instruction");
        let ty = self.type_of(op);
        let ones = self.all_ones(ty);
        let not = self.insert_after(def, Opcode::Binary(BinaryOp::Xor), ty, vec![op, ones]);
        let not_value = self.value_of(not);
        if let Operand::Value(value) = op {
            let uses: Vec<(InstId, u32)> = self.function.users(value).iter().map(|one| (one.user, one.index)).collect();
            for (user, index) in uses {
                if user != not {
                    self.function.set_operand(user, index as usize, not_value);
                    self.add_to_worklist(user);
                }
            }
        }
        self.freely_invert_all_users_of(not_value, Some(ignored));
        not_value
    }

    /// A new instruction right after the definition of `def`, queued
    /// (`Builder.SetInsertPoint(*I.getInsertionPointAfterDef())`).
    pub(crate) fn insert_after(
        &mut self,
        def: InstId,
        opcode: Opcode,
        ty: llrm_mir::TypeId,
        operands: Vec<Operand>,
    ) -> InstId {
        let new = self.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.function.insert(new, Position::After(def)).expect("a placed instruction");
        self.add_to_worklist(new);
        new
    }
}

impl Combiner<'_> {
    /// `getFreelyInverted(V, WillInvertAllUses, Builder, DoesConsume)`: the
    /// inverted value (made before `at` when `build`), if `v` is free to
    /// invert.
    pub(crate) fn get_freely_inverted(
        &mut self,
        at: InstId,
        v: Operand,
        will_invert_all_uses: bool,
        build: bool,
        consumes: &mut bool,
    ) -> Option<Operand> {
        *consumes = false;
        self.freely_inverted(at, v, will_invert_all_uses, build, consumes, 0)
    }

    fn freely_inverted(
        &mut self,
        at: InstId,
        v: Operand,
        will_invert_all_uses: bool,
        build: bool,
        consumes: &mut bool,
        depth: u32,
    ) -> Option<Operand> {
        // ~(~(X)) -> X.
        if let Some(a) = pattern::not(&query(self, Some(at)), v) {
            *consumes = true;
            return Some(a);
        }
        // Constants can be considered to be not'ed values.
        if let Some((bits, _)) = query(self, None).int_constant(v) {
            return Some(self.int(self.type_of(v), !bits));
        }
        if depth >= MAX_DEPTH {
            return None;
        }
        // The rest of the cases require that we invert all uses so don't
        // bother doing the analysis if we know we can't use the result.
        if !will_invert_all_uses {
            return None;
        }
        let (_, instruction) = query(self, None).instruction_of(v)?;
        let (opcode, operands) = (instruction.opcode.clone(), instruction.operands.clone());
        let depth = depth + 1;
        let inner = |c: &mut Self, x: Operand, consumes: &mut bool| {
            c.freely_inverted(at, x, has_one_use(c, x), build, consumes, depth)
        };
        match opcode {
            // Compares can be inverted if all of their uses are being
            // modified to use the ~V.
            Opcode::ICmp(pred) => {
                Some(if build { self.create_icmp(at, pred.inverse(), operands[0], operands[1]) } else { v })
            }
            // `-1 - (A + B)` is `(-1 - B) - A`.
            Opcode::Binary(BinaryOp::Add) => {
                let (a, b) = (operands[0], operands[1]);
                if let Some(not_b) = inner(self, b, consumes) {
                    return Some(if build { self.create_sub(at, not_b, a) } else { v });
                }
                let not_a = inner(self, a, consumes)?;
                Some(if build { self.create_sub(at, not_a, b) } else { v })
            }
            // `~(A ^ ~B)` is `A ^ B`.
            Opcode::Binary(BinaryOp::Xor) => {
                let (a, b) = (operands[0], operands[1]);
                if let Some(not_b) = inner(self, b, consumes) {
                    return Some(if build {
                        self.create_binop(at, BinaryOp::Xor, a, not_b, Flags::default())
                    } else {
                        v
                    });
                }
                let not_a = inner(self, a, consumes)?;
                Some(if build { self.create_binop(at, BinaryOp::Xor, not_a, b, Flags::default()) } else { v })
            }
            // `-1 - (A - B)` is `(-1 - A) + B`.
            Opcode::Binary(BinaryOp::Sub) => {
                let not_a = inner(self, operands[0], consumes)?;
                Some(if build { self.create_add(at, not_a, operands[1]) } else { v })
            }
            // `~((~A) s>> B)` is `A s>> B`.
            Opcode::Binary(BinaryOp::AShr) => {
                let not_a = inner(self, operands[0], consumes)?;
                Some(if build {
                    self.create_binop(at, BinaryOp::AShr, not_a, operands[1], Flags::default())
                } else {
                    v
                })
            }
            // OWED: the select and min/max case, and the phi case.
            Opcode::Cast(op @ (CastOp::SExt | CastOp::Trunc)) => {
                let not_a = inner(self, operands[0], consumes)?;
                let ty = self.type_of(v);
                Some(if build { self.create_cast(at, op, not_a, ty, Flags::default()) } else { v })
            }
            // OWED: De Morgan's laws.
            _ => None,
        }
    }
}
