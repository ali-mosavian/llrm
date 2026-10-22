//! One extending load for a narrow load and its extension: LLVM's
//! DAGCombiner
//! [`ExtendUsesToFormExtLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15365)
//! and
//! [`ExtendSetCCUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15421),
//! as the rewrite of MIR before isel.
//!
//! # How it works
//! For a load whose value `E = sext/zext L` is made in the same block and
//! whose other readers are all compares with a constant in that block, those
//! compares compare `E` instead, with the constant extended the same way. `L`
//! is then read only by `E`, and isel selects the pair as one `movsx`/`movzx`
//! from memory: the compare is on the wide register, not on a byte register
//! that `E` must extend again.
//!
//! A `zext` is not made where a signed compare reads `L` (its sign bits would
//! be lost). `E` is moved to just after `L`, so that it precedes the compares
//! it now feeds.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | Done on MIR before isel, not in the DAG: isel selects one instruction at a time (`isel/combined.rs`) and has no combine stage. Owner mir-undef; revisit when isel gains one. |
//! | D2 | Any other reader of the load (a store, a call, an extension in another block) leaves the load alone. The pin gives it `trunc E` (`isTruncateFree`), and the narrow value then lives beside the wide one: bench/grep.nib `_main` grew 58 B of spills when this was done. Owner mir-undef; revisit when isel can fold the trunc. |
//!
//! # Example
//! Before:
//!
//! ```mir
//! b0:
//!   %c = load i8, ptr %p
//!   %lt = icmp sgt i8 %c, 0
//!   %w = sext i8 %c to i16
//!   br i1 %lt, label %b1, label %b2
//! ```
//!
//! After:
//!
//! ```mir
//! b0:
//!   %c = load i8, ptr %p
//!   %w = sext i8 %c to i16
//!   %lt = icmp sgt i16 %w, 0
//!   br i1 %lt, label %b1, label %b2
//! ```
//!
//! From unit test `test_a_compare_of_a_loaded_byte_compares_its_extension` in
//! `extload_tests.rs`.
//!
//! # Runs
//! Once per body, after `addresssink`, where the other passes that make the
//! forms isel sees run (the pin has none: its DAGCombiner does it). With
//! `wanted("extload")`.
//!
//! # Needs
//! Nothing.
//!
//! # Keeps
//! Blocks, dominators and loops.
//!
//! # See also
//! Tests: `extload_tests.rs`. Levels: `docs/reference/levels.md`.

use llrm_mir::context::{ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct ExtLoad;

impl FunctionPass for ExtLoad {
    fn name(&self) -> &'static str {
        "extload"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        _analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if widened(unit.context, unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// What the compares and the other readers of one load become.
struct Plan {
    extension: InstId,
    /// `(compare, index of the load in its operands, index of the constant)`.
    compares: Vec<(InstId, u32, u32)>,
}

/// `load`'s plan, or `None` where the pin's `ExtendUsesToFormExtLoad` says no
/// or there is nothing to change.
fn plan(
    context: &Context,
    function: &Function,
    home: BlockId,
    load: InstId,
) -> Option<Plan> {
    let Opcode::Load { volatile: false, .. } = function.instruction(load).opcode else { return None };
    let result = function.instruction(load).result?;
    context.types.int_bits(function.instruction(load).ty)?;
    let extension = function.iter_block(home).skip_while(|&one| one != load).skip(1).find(|&one| {
        let made = function.instruction(one);
        matches!(made.opcode, Opcode::Cast(CastOp::SExt | CastOp::ZExt)) && made.operands[0] == Operand::Value(result)
    })?;
    let zero_extended = function.instruction(extension).opcode == Opcode::Cast(CastOp::ZExt);
    let extended = function.instruction(extension).result?;
    let mut compares = Vec::new();
    for used in function.users(result).snapshot() {
        if used.user == extension {
            continue;
        }
        let user = function.instruction(used.user);
        let here = function.parent(used.user) == Some(home);
        if let (Opcode::ICmp(predicate), true) = (&user.opcode, here) {
            let other = 1 - used.index;
            if int_constant(context, user.operands[other as usize]).is_some() {
                // A zero extension loses the sign bits a signed compare reads.
                if zero_extended && is_signed(*predicate) {
                    return None;
                }
                compares.push((used.user, used.index, other));
                continue;
            }
        }
        // Another reader would read `trunc E`: the narrow value stays live
        // beside the wide one and costs a register (D3).
        return None;
    }
    if compares.is_empty() {
        return None;
    }
    Some(Plan { extension, compares })
}

fn int_constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

fn is_signed(predicate: IntPredicate) -> bool {
    matches!(
        predicate,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

fn widened(
    context: &Context,
    function: &mut Function,
) -> bool {
    let loads: Vec<(BlockId, InstId)> =
        function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Load { .. })).collect();
    let mut changed = false;
    for (home, load) in loads {
        if function.is_erased(load) {
            continue;
        }
        let Some(plan) = plan(context, function, home, load) else { continue };
        let result = function.instruction(load).result.expect("a value");
        let narrow = function.instruction(load).ty;
        let narrow_bits = context.types.int_bits(narrow).expect("an integer");
        let wide = function.instruction(plan.extension).ty;
        let extended = Operand::Value(function.instruction(plan.extension).result.expect("a value"));
        let zero_extended = function.instruction(plan.extension).opcode == Opcode::Cast(CastOp::ZExt);
        function.move_to(plan.extension, Position::After(load)).expect("a placed instruction");
        for (compare, at, constant) in plan.compares {
            let mut operands = function.instruction(compare).operands.clone();
            let bits = int_constant(context, operands[constant as usize]).expect("an integer constant");
            let value = if zero_extended { bits as i128 } else { signed(bits, narrow_bits) };
            operands[at as usize] = extended;
            operands[constant as usize] = Operand::Constant(context.int(wide, value));
            function.set_operands(compare, operands);
        }
        debug_assert!(function.users(result).snapshot().iter().all(|used| used.user == plan.extension));
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "extload_tests.rs"]
mod tests;
