//! What a counted-loop rewrite places before the loop: its seeds, skip test
//! and exit values. Adapted from llrm-core's `optimize/counting.rs`, the
//! port of `qbopt/optimize/counting.py`; LLVM's counterpart is
//! SCEVExpander, as IndVarSimplify and LoopRotate use it.
//!
//! Built only from `induction::CountedLoop`, so every pass that replaces a
//! loop's counter enters and leaves the loop the same way.
//!
//! A seed is placed as it is made, before `Seeds::at`: the old ones were
//! ops held for the caller to splice once its proof was complete, so a
//! caller asks only once it will commit. `fresh` and `held` have no
//! counterpart: a value is its instruction's result, and a phi takes a
//! constant as it takes a value. `skip_guard` gives the compare; the
//! caller branches on it, as the old caller placed the branch it returned.

use llrm_analysis::cfg;
use llrm_analysis::consts::{ARITH, masked};
use llrm_analysis::induction::{self, AffineOperand, ControlReplacement, CountedLoop};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::Context;
use num_bigint::BigInt;

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
    pub fn computed(&mut self, kind: BinaryOp, args: Vec<AffineOperand>) -> AffineOperand {
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

    /// `term` as an operand.
    pub fn operand(&mut self, term: &AffineOperand) -> Operand {
        match term {
            AffineOperand::Value(value, _) => Operand::Value(*value),
            AffineOperand::Const(known) => constant(self.context, &known.n, known.width),
        }
    }
}

/// `n` as a constant `width` bits wide.
pub fn constant(context: &mut Context, n: &BigInt, width: u32) -> Operand {
    let ty = context.types.int(width);
    let bits = u128::try_from(masked(n, width)).expect("a masked number fits its width");
    Operand::Constant(context.int(ty, bits as i128))
}

/// The preheader compare that is true where a counted loop runs no trips;
/// a branch on it to `proof.exit` skips the loop.
pub fn skip_guard(seeds: &mut Seeds, proof: &CountedLoop) -> Option<ValueId> {
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
pub fn leaving(seeds: &mut Seeds, replacement: &ControlReplacement<'_>, guarded: bool) -> Vec<(InstId, Vec<Operand>)> {
    let proof = replacement.counted;
    if replacement.exits.is_empty() {
        return Vec::new();
    }
    let exit = induction::exit_value(proof, &mut |kind, args| seeds.computed(kind, args)).expect("a pre-tested proof");
    let value = seeds.operand(&exit);
    let preheader = Operand::Block(cfg::block(proof.preheader.expect("control_replacement proved a preheader")));
    let start = seeds.function.instruction(proof.phi).operands.chunks(2).find(|pair| pair[1] == preheader).expect("a preheader edge")[0];
    replacement
        .exits
        .iter()
        .map(|&phi| {
            let arms = seeds.function.instruction(phi).operands.chunks(2).filter(|pair| !guarded || pair[1] != preheader);
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
