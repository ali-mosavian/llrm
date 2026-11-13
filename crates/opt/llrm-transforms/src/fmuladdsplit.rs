//! Splits an `llvm.fmuladd` whose product a loop does not change, so that
//! LICM hoists the multiply.
//!
//! # How it works
//! clang contracts `x + a * b` into `llvm.fmuladd(a, b, x)`. The intrinsic is
//! one instruction to LICM, which cannot hoist half of it: a product of values
//! the loop does not write is made on every trip, until the DAG splits the call
//! into `fmul` and `fadd` (the target having no fused multiply-add). The
//! target says (`Tti::splits_invariant_fmuladd`) that no fused form will come,
//! and a call in a loop whose two factors are defined outside it and whose
//! addend is not becomes `fmul` and `fadd`, each with the call's flags, in
//! its place.
//!
//! ```text
//! loop:  %p = call fmuladd(%r, 6.7e7, %x)        loop:  %m = fmul %r, 6.7e7
//!                                          =>           %p = fadd %m, %x
//!                                                  (LICM: %m leaves the loop)
//! ```
//!
//! DEVIATE FMULADD-SPLIT (no pin pass: clang's `llvm.fmuladd` stays one
//! instruction until `SelectionDAGBuilder::visitIntrinsicCall` splits it, after
//! the loop passes; gcc's `convert_mult_to_fma` runs after
//! `pass_lim` and only where the target has the instruction; owner x87-c, exit
//! 2026-11-24).
//!
//! # See also
//! `-ffp-contract=off` for every call gave the same hoist and cost nbody_single
//! +3.2% clocks and +289 B at -O2: the split of a call whose product varies
//! changes the unrolling price of the loop that holds it. Only the invariant
//! product is split here.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, Loops, PreservedAnalyses};

/// Every `llvm.fmuladd` of a loop of `unit` whose factors are defined outside
/// the loop and whose addend is not, made an `fmul` and an `fadd`; whether
/// there was one.
pub fn split(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
) -> bool {
    let Some(tti) = analyses.outer().target().tti() else { return false };
    if !tti.splits_invariant_fmuladd() {
        return false;
    }
    let outer = std::rc::Rc::clone(analyses.outer());
    let found = candidates(unit, analyses, &outer.globals);
    for &call in &found {
        let function = &mut *unit.function;
        let instruction = function.instruction(call).clone();
        let [a, b, addend, ..] = instruction.operands[..] else { continue };
        let Some(result) = instruction.result else { continue };
        let product = function.create_instruction(
            Opcode::Binary(BinaryOp::FMul),
            instruction.ty,
            vec![a, b],
            instruction.flags,
            None,
        );
        function.insert(product, llrm_mir::edit::Position::Before(call)).expect("the product stands before the call");
        let product = Operand::Value(function.instruction(product).result.expect("a value"));
        let sum = function.create_instruction(
            Opcode::Binary(BinaryOp::FAdd),
            instruction.ty,
            vec![product, addend],
            instruction.flags,
            None,
        );
        function.insert(sum, llrm_mir::edit::Position::Before(call)).expect("the sum stands before the call");
        let sum = Operand::Value(function.instruction(sum).result.expect("a value"));
        function.replace_all_uses_with(result, sum);
        function.erase(call).expect("the call has no use left");
    }
    !found.is_empty()
}

/// The calls of `llvm.fmuladd` `split` makes two instructions of.
fn candidates(
    unit: &passes::Unit,
    analyses: &mut Analyses,
    globals: &[llrm_mir::module::GlobalValue],
) -> Vec<InstId> {
    let function = &*unit.function;
    let info = analyses.get::<Loops>(&*unit.context, unit.layout, function);
    let loops = cfg::natural(function, &info);
    if loops.is_empty() {
        return Vec::new();
    }
    let block_of: BTreeMap<InstId, BlockId> = function.walk().map(|(block, inst)| (inst, block)).collect();
    let mut found = Vec::new();
    for (&inst, &block) in &block_of {
        let Some(callee) = llrm_mir::memory::callee(unit.context, function, inst) else { continue };
        if globals.get(callee.0 as usize).and_then(|global| global.intrinsic()) != Some(Intrinsic::FMulAdd) {
            continue;
        }
        let [a, b, addend, ..] = function.instruction(inst).operands[..] else { continue };
        // The innermost loop that holds the call comes first, the loops
        // being in order of size.
        let Some(inner) = loops.iter().find(|one| one.body.contains(&i64::from(block.0))) else { continue };
        let outside = |operand: Operand| match operand {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Argument(_) => true,
                ValueDef::Instruction(def) => {
                    block_of.get(&def).is_some_and(|at| !inner.body.contains(&i64::from(at.0)))
                }
            },
            _ => true,
        };
        if outside(a) && outside(b) && !outside(addend) {
            found.push(inst);
        }
    }
    found
}

/// The split as a pass of its own, before the first LICM. Not where
/// code is built for size: an x87 product held across the loop is popped on
/// the way out (`fstp st(0)`, a block of its own), 2 bytes the hoist does not
/// repay (x_ld_to_ulong m32 -Os 170 -> 172 B).
pub struct FmuladdSplit {
    pub size: bool,
}

impl passes::FunctionPass for FmuladdSplit {
    fn name(&self) -> &'static str {
        "fmuladdsplit"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if !self.size && split(unit, analyses) {
            // Instructions only: no block or edge changes.
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "fmuladdsplit_tests.rs"]
mod tests;
