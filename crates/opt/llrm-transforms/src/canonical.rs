//! Canonical forms, as LLVM's InstCombine puts them: compares with the constant
//! on the right, no neutral terms, and a zero test at the width of what it
//! tests, so that every later pass matches one shape of each.
//!
//! # How it works
//! Two steps, run together by `Canonical` and, as functions, by `fold` and
//! `rotate`.
//!
//! `compares` swaps every `icmp C, v` (an integer constant on the left, a
//! non-constant on the right) to `icmp v, C` with the predicate mirrored. No
//! machine compares an immediate against a register in that order, and folding
//! makes this shape whenever a compare's left operand becomes known. A compare
//! of two constants is left to folding, and one of two values stays.
//!
//! `identities` then visits each instruction once:
//!
//! - `algebraic::identity`: neutral terms (`x + 0`, `x * 1`, `x & -1`, ...) are
//!   their operand. Rewrites that state a result from a proof in full (a
//!   rotated loop's trip count is `bound - start + inclusive` for any start)
//!   leave the neutral terms to this step, so none folds its own.
//! - a zero test of an extension tests what was extended: `x == 0` and `x != 0`
//!   of `zext`/`sext` of `y` are the same test of `y`, and of an `i1`, `y != 0`
//!   is `y` and `y == 0` is `not y`.
//! - `x <=u 0` is `x == 0` and `x >u 0` is `x != 0`: nothing is below zero.
//!
//! ```text
//!  %e = sext i1 %go to i8                  br i1 %go, ...
//!  %c = icmp ne i8 %e, 0         ==>
//!  br i1 %c, ...                           (the compare induction reads)
//! ```
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %a = add i16 0, %x
//!   %m = mul i16 1, %a
//!   %n = sub i16 0, %m
//!   ret i16 %n
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %n = sub i16 0, %x
//!   ret i16 %n
//! }
//! ```
//! The neutral `add 0` and `mul 1` are their operands; `sub 0, x` is not
//! neutral and stays.
//!
//! From unit test `neutral_terms_fold_on_either_side_but_zero_minus_x_stays` in
//! `canonical_tests.rs`.
//!
//! # Runs
//! Not a step of the fixed point on its own: `fold` runs `compares` and
//! `identities` after folding, and `rotate` runs `identities` after rotating a
//! loop. Both are on at every level above -O0 (`fold` is off only with the pass
//! disabled by name).
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # See also
//! [`crate::algebraic`], [`crate::fold`]; LLVM InstCombine.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

use crate::algebraic;

/// `compares` then `identities`, as the old fold applied them.
pub struct Canonical;

impl FunctionPass for Canonical {
    fn name(&self) -> &'static str {
        "canonical"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        let swapped = compares(unit.context, unit.function);
        if identities(unit.context, unit.function) | swapped {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Whether any compare was swapped.
pub fn compares(
    context: &Context,
    function: &mut Function,
) -> bool {
    let swapped = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| _constant_left(context, function, inst))
        .collect::<Vec<_>>();
    for &inst in &swapped {
        let instruction = function.instruction(inst);
        let Opcode::ICmp(predicate) = instruction.opcode else { unreachable!("a compare") };
        let operands = vec![instruction.operands[1], instruction.operands[0]];
        replaced(function, inst, Opcode::ICmp(predicate.swapped()), operands);
    }
    !swapped.is_empty()
}

fn _constant_left(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let constant = |operand| {
        matches!(
            operand,
            Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_))
        )
    };
    matches!(instruction.opcode, Opcode::ICmp(_))
        && constant(instruction.operands[0])
        && !matches!(instruction.operands[1], Operand::Constant(_))
}

/// `inst` rebuilt with `opcode` and `operands`, where it was.
fn replaced(
    function: &mut Function,
    inst: InstId,
    opcode: Opcode,
    operands: Vec<Operand>,
) {
    let instruction = function.instruction(inst);
    let new = function.create_instruction(opcode, instruction.ty, operands, instruction.flags, None);
    function.insert(new, Position::Before(inst)).expect("a placed instruction");
    let (old, value) =
        (function.instruction(inst).result.expect("a value"), function.instruction(new).result.expect("a value"));
    function.replace_value(old, Operand::Value(value));
    function.erase(inst).expect("its uses were replaced");
}

/// Each of `algebraic::identity`'s identities, a test `x <=u 0` is
/// `x == 0`, and `_zero_tested`'s. Whether anything changed.
///
/// A rewrite states what it computes from a proof in full -- rotation's
/// trip count is `bound - start + inclusive` for any start -- and the
/// neutral terms go here, so no rewrite folds its own.
pub fn identities(
    context: &Context,
    function: &mut Function,
) -> bool {
    let mut changed = false;
    for inst in function.walk().map(|(_, inst)| inst).collect::<Vec<_>>() {
        if let Some(kept) = algebraic::identity(context, function, inst) {
            let result = function.instruction(inst).result.expect("a value");
            function.replace_value(result, kept);
            function.erase(inst).expect("its uses were replaced");
            changed = true;
        } else if _zero_tested(context, function, inst) {
            changed = true;
        } else if _zero_test(context, function, inst) {
            let instruction = function.instruction(inst);
            // Nothing is below zero.
            let predicate = match instruction.opcode {
                Opcode::ICmp(IntPredicate::Ule) => IntPredicate::Eq,
                Opcode::ICmp(IntPredicate::Ugt) => IntPredicate::Ne,
                _ => continue,
            };
            let operands = instruction.operands.clone();
            replaced(function, inst, Opcode::ICmp(predicate), operands);
            changed = true;
        }
    }
    changed
}

/// `x == 0` or `x != 0` of an extended `x` tests `x` at its own width, an
/// extension keeping whether a value is zero; and of an `i1`, `x != 0` is
/// `x` and `x == 0` is `not x`. A frontend's truth, a sign-extended `i1`,
/// is tested so, and induction reads the compare beneath it.
fn _zero_tested(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::ICmp(predicate @ (IntPredicate::Eq | IntPredicate::Ne)) = instruction.opcode else { return false };
    let (tested, result) = (instruction.operands[0], instruction.result.expect("a value"));
    if _integer(context, instruction.operands[1]) != Some(0) {
        return false;
    }
    let extended = match tested {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => match function.instruction(def).opcode {
                Opcode::Cast(CastOp::ZExt | CastOp::SExt) => Some(function.instruction(def).operands[0]),
                _ => None,
            },
            ValueDef::Argument(_) => None,
        },
        _ => None,
    };
    let x = extended.unwrap_or(tested);
    let ty = function.operand_type(context, x).expect("a typed operand");
    let with = match (context.types.int_bits(ty), predicate) {
        (Some(1), IntPredicate::Ne) => x,
        (Some(1), _) => {
            let all = Operand::Constant(context.int(ty, 1));
            _before(function, inst, Opcode::Binary(BinaryOp::Xor), ty, vec![x, all])
        }
        _ if extended.is_some() => {
            let zero = Operand::Constant(context.int(ty, 0));
            let truth = function.instruction(inst).ty;
            _before(function, inst, Opcode::ICmp(predicate), truth, vec![x, zero])
        }
        _ => return false,
    };
    function.replace_value(result, with);
    function.erase(inst).expect("its uses were replaced");
    true
}

/// A new `opcode` of type `ty` placed before `at`: its value.
fn _before(
    function: &mut Function,
    at: InstId,
    opcode: Opcode,
    ty: llrm_mir::types::TypeId,
    operands: Vec<Operand>,
) -> Operand {
    let new = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(new, Position::Before(at)).expect("a placed instruction");
    Operand::Value(function.instruction(new).result.expect("a value"))
}

/// An `icmp` against zero on the right.
fn _zero_test(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    matches!(instruction.opcode, Opcode::ICmp(_)) && _integer(context, instruction.operands[1]) == Some(0)
}

/// An integer constant's bits.
fn _integer(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "canonical_tests.rs"]
mod tests;
