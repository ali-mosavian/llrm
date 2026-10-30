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

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::pattern;

use crate::combine::Combiner;
use crate::combine_shared::{has_one_use, query};

/// `MaxAnalysisRecursionDepth`.
const MAX_DEPTH: u32 = 6;

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

impl Combiner<'_> {
    /// `Builder.Insert*` with the insertion point `getInsertionPointAfterDef`
    /// of `def`: a new instruction after it (the first non-phi of the block
    /// after a phi), queued.
    pub(crate) fn insert_after(
        &mut self,
        def: InstId,
        opcode: Opcode,
        ty: llrm_mir::types::TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> InstId {
        let new = self.function.create_instruction(opcode, ty, operands, flags, None);
        let position = if self.function.instruction(def).opcode == Opcode::Phi {
            let block = self.function.parent(def).expect("placed");
            let first = self
                .function
                .iter_block(block)
                .find(|&inst| self.function.instruction(inst).opcode != Opcode::Phi)
                .expect("a terminator");
            llrm_mir::edit::Position::Before(first)
        } else {
            llrm_mir::edit::Position::After(def)
        };
        self.function.insert(new, position).expect("a placed instruction");
        self.worklist.add(new);
        new
    }

    /// `shouldAvoidAbsorbingNotIntoSelect` (InstCombiner.h:210): `a ? b :
    /// false` and `a ? true : b` are the canonical logical and/or, and
    /// swapping the arms to absorb a `not` would hide them.
    fn should_avoid_absorbing_not_into_select(
        &self,
        select: Operand,
    ) -> bool {
        let q = query(self, None);
        pattern::logical_and(&q, select).is_some() || pattern::logical_or(&q, select).is_some()
    }

    /// `canFreelyInvertAllUsersOf` (InstCombiner.h:267): every user of the i1
    /// `v` but `ignored` takes `!v` for free: a select that reads it as its
    /// condition, a conditional branch, a `not`.
    pub(crate) fn can_freely_invert_all_users_of(
        &self,
        v: Operand,
        ignored: Option<InstId>,
    ) -> bool {
        let Operand::Value(value) = v else { return false };
        self.function
            .users(value)
            .iter()
            .all(
                |one| {
                    if Some(one.user) == ignored {
                        return true;
                    }
                    let user = self.function.instruction(one.user);
                    match user.opcode {
                        // Only if the value is used as select cond.
                        Opcode::Select => {
                            let result = Operand::Value(user.result.expect("a value"));
                            one.index == 0 && !self.should_avoid_absorbing_not_into_select(result)
                        }
                        // Free to invert by swapping the successors.
                        Opcode::Br => user.operands.len() == 3,
                        // A `not` is dropped.
                        Opcode::Binary(BinaryOp::Xor) => {
                            pattern::not(&query(self, None), Operand::Value(user.result.expect("a value"))).is_some()
                        }
                        _ => false,
                    }
                },
            )
    }

    /// `freelyInvertAllUsersOf` (InstructionCombining.cpp:1455): adapt every
    /// user of `v` but `ignored` as if `v` were changed to `!v`. Only after
    /// [`Self::can_freely_invert_all_users_of`] said yes.
    ///
    /// DEVIATE: the swapped select's and branch's `!prof` is not swapped.
    pub(crate) fn freely_invert_all_users_of(
        &mut self,
        v: Operand,
        ignored: Option<InstId>,
    ) {
        let Operand::Value(value) = v else { unreachable!("an instruction") };
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
                Opcode::Binary(BinaryOp::Xor) => {
                    self.replace_inst_with(user, v);
                }
                _ => unreachable!("out of sync with can_freely_invert_all_users_of"),
            }
            self.add_to_worklist(user);
        }
    }
}
