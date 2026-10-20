//! What a counted-loop rewrite places before the loop: its seeds, skip test and
//! exit values (LLVM's SCEVExpander, as IndVarSimplify and LoopRotate use it).
//!
//! # How it works
//! Everything is built from an `induction::CountedLoop` proof, so every pass
//! that replaces a loop's counter enters and leaves the loop the same way. A
//! `Seeds` names where the arithmetic goes (before `at`) and how wide it is.
//!
//! - `Seeds::computed` places `kind` of its arguments, or the constant where
//!   both are constants, so a rewrite whose count and start are known places
//!   nothing; `canonical::identities` folds the neutral terms of the rest.
//! - `Seeds::widened` zero-extends a trip count to an index's width.
//! - `Seeds::operand` and `constant` turn an affine term or a number into an
//!   operand of a given width.
//! - `skip_guard` builds the preheader compare that is true when the loop runs
//!   no trips; the caller branches on it to the loop's exit.
//! - `leaving` gives the new operands of the exit phis of a replaced counter:
//!   its exit value after a trip and, where a guard may skip the loop, its
//!   start after none.
//!
//! ```text
//!  preheader:                         built before `at`:
//!    br loop                  ==>       %trips = ...        Seeds::computed
//!                                       %skip  = icmp ...   skip_guard
//!                                       br %skip, exit, loop
//!  exit:                              exit:
//!    %r = phi [%i, latch]               %r = phi [%v, latch], [%i0, preheader]
//! ```
//!
//! A seed is placed as it is made, so a caller asks for one only once it will
//! commit to its rewrite.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %s, i16 %n) {
//! b0:
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ %s, %b0 ], [ %next, %b2 ]
//!   %c = icmp slt i16 %i, %n
//!   br i1 %c, label %b2, label %b3
//! ; ...
//! b3:
//!   %e = phi i16 [ %i, %b1 ]
//!   ret i16 %e
//! }
//! ```
//! After `skip_guard` and `leaving` (the lines the test asserts):
//! ```mir
//! b0:
//!   %0 = icmp sle i16 %n, %s
//!   %1 = add i16 %n, 0
//!   br i1 %0, label %b3, label %b1
//! ; ...
//! b3:
//!   %e = phi i16 [ %1, %b1 ], [ %s, %b0 ]
//! ```
//! The preheader tests for no trips and branches to the exit, and the exit phi
//! takes the exit value `%n` (from `add %n, 0`, left to
//! `canonical::identities`) or, with no trip, the start `%s`.
//!
//! From unit test
//! `a_symbolic_exit_value_and_skip_test_are_placed_before_the_loop` in
//! `counting_tests.rs`.
//!
//! # Used by
//! [`crate::loopidioms`], [`crate::window`] and
//! [`crate::lsr_pass`] build `Seeds` when they commit to replacing a loop's
//! counter, and `lftr` and others take `constant` for operands.
//! Not a pass: it runs only inside those.
//!
//! # See also
//! [`crate::expand`] for sums.

use llrm_analysis::cfg;
use llrm_analysis::consts::{ARITH, masked};
use llrm_analysis::induction::{self, AffineOperand, ControlReplacement, CountedLoop};
use llrm_analysis::num::Num as BigInt;
use llrm_mir::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};

/// Where a rewrite's preheader arithmetic goes, and how wide it is.
///
/// Arithmetic on constants is its constant, so a rewrite whose count and
/// start are known places nothing; `canonical::identities` folds the
/// neutral terms of the rest.
pub struct Seeds<'a> {
    pub context: &'a mut Context,
    pub function: &'a mut Function,
    /// Each seed goes before this instruction.
    pub at: InstId,
    pub width: u32,
}

impl Seeds<'_> {
    /// `kind` of `args`, the constant it is where both are.
    pub fn computed(
        &mut self,
        kind: BinaryOp,
        args: Vec<AffineOperand>,
    ) -> AffineOperand {
        let arith = ARITH.iter().find(|(one, _)| *one == kind).map(|(_, arith)| *arith);
        if let (Some(arith), [AffineOperand::Const(left), AffineOperand::Const(right)]) = (arith, args.as_slice()) {
            return AffineOperand::constant(arith(&left.n, &right.n), self.width);
        }
        let ty = self.context.types.int(self.width);
        let operands = args.iter().map(|one| self.operand(one)).collect();
        let inst = self.function.create_instruction(Opcode::Binary(kind), ty, operands, Flags::default(), None);
        self.function.insert(inst, Position::Before(self.at)).expect("`at` is placed");
        AffineOperand::Value(self.function.instruction(inst).result.expect("an integer result"), self.width)
    }

    /// `term`, unsigned, `to` bits wide where it is narrower: a trip count in
    /// an index's width.
    pub fn widened(
        &mut self,
        term: &AffineOperand,
        to: u32,
    ) -> AffineOperand {
        match term {
            AffineOperand::Const(known) => AffineOperand::constant(known.n.clone(), to),
            AffineOperand::Value(_, from) if *from >= to => term.clone(),
            AffineOperand::Value(value, _) => {
                let ty = self.context.types.int(to);
                let inst = self
                    .function
                    .create_instruction(
                        Opcode::Cast(CastOp::ZExt),
                        ty,
                        vec![Operand::Value(*value)],
                        Flags::default(),
                        None,
                    );
                self.function.insert(inst, Position::Before(self.at)).expect("`at` is placed");
                AffineOperand::Value(self.function.instruction(inst).result.expect("an integer result"), to)
            }
        }
    }

    /// `term` as an operand.
    pub fn operand(
        &mut self,
        term: &AffineOperand,
    ) -> Operand {
        match term {
            AffineOperand::Value(value, _) => Operand::Value(*value),
            AffineOperand::Const(known) => constant(self.context, &known.n, known.width),
        }
    }
}

/// `n` as a constant `width` bits wide.
pub fn constant(
    context: &Context,
    n: &BigInt,
    width: u32,
) -> Operand {
    let ty = context.types.int(width);
    let bits = u128::try_from(masked(n, width)).expect("a masked number fits its width");
    Operand::Constant(context.int(ty, bits as i128))
}

/// The preheader compare that is true where a counted loop runs no trips;
/// a branch on it to `proof.exit` skips the loop.
pub fn skip_guard(
    seeds: &mut Seeds,
    proof: &CountedLoop,
) -> Option<ValueId> {
    let ((left, right), test) = induction::skipped(proof)?;
    let operands = vec![seeds.operand(&left), seeds.operand(&right)];
    let ty = seeds.context.types.int(1);
    let compare = seeds.function.create_instruction(Opcode::ICmp(test), ty, operands, Flags::default(), None);
    seeds.function.insert(compare, Position::Before(seeds.at)).expect("`at` is placed");
    seeds.function.instruction(compare).result
}

/// Exit phis of a replaced counter, reading its exit value after a trip and,
/// where a guard may skip the loop, its start after none: each phi's new
/// operands.
pub fn leaving(
    seeds: &mut Seeds,
    replacement: &ControlReplacement<'_>,
    guarded: bool,
) -> Vec<(InstId, Vec<Operand>)> {
    let proof = replacement.counted;
    if replacement.exits.is_empty() {
        return Vec::new();
    }
    let exit = induction::exit_value(proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    let value = seeds.operand(&exit);
    let preheader = Operand::Block(cfg::block(proof.preheader.expect("control_replacement proved a preheader")));
    let start = seeds
        .function
        .instruction(proof.phi)
        .operands
        .chunks(2)
        .find(|pair| pair[1] == preheader)
        .expect("a preheader edge")[0];
    replacement
        .exits
        .iter()
        .map(|&phi| {
            let arms =
                seeds.function.instruction(phi).operands.chunks(2).filter(|pair| !guarded || pair[1] != preheader);
            let mut operands = arms.flat_map(|pair| [value, pair[1]]).collect::<Vec<_>>();
            if guarded {
                operands.extend([start, preheader]);
            }
            (phi, operands)
        })
        .collect()
}

#[cfg(test)]
#[path = "counting_tests.rs"]
mod tests;
