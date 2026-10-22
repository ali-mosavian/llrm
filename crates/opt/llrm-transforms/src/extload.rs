//! One extending load for a narrow load and its extension: LLVM's
//! DAGCombiner
//! [`ExtendUsesToFormExtLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15365)
//! and
//! [`ExtendSetCCUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15421),
//! as the rewrite of MIR before isel.
//!
//! # How it works
//! For a load whose value `E = sext/zext L` is made in the same block, the
//! compares of `L` with a constant in that block compare `E` instead, with the
//! constant extended the same way; every other reader of `L` reads
//! `trunc E`. `L` is then read only by `E`, and isel selects the pair as one
//! `movsx`/`movzx` from memory: the compare is on the wide register, not on a
//! byte register that `E` must extend again.
//!
//! A `zext` is not made where a signed compare reads `L` (its sign bits would
//! be lost), and a load whose value and extension are both read in other
//! blocks is left unless a compare is widened (`BothLiveOut`). `E` is moved to
//! just after `L`, so that it precedes the compares it now feeds.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | Done on MIR before isel, not in the DAG: isel selects one instruction at a time (`isel/combined.rs`) and has no combine stage. Owner mir-undef; revisit when isel gains one. |
//! | D2 | `isTruncateFree` is taken as true: a truncation to a narrower integer reads the low part of the register on every x86 mode, so the compare of an unextendable use is not a reason to stay. |
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
    /// The readers of the load other than the extension and the compares.
    others: Vec<(InstId, u32)>,
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
    let mut others = Vec::new();
    let mut live_out = false;
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
        live_out |= !here || user.opcode == Opcode::Phi;
        others.push((used.user, used.index));
    }
    if compares.is_empty() && others.is_empty() {
        return None;
    }
    // `BothLiveOut`: both the load and its extension are read in other
    // blocks, and only a widened compare makes the change worth it.
    if live_out
        && compares.is_empty()
        && function.users(extended).snapshot().iter().any(|used| {
            function.parent(used.user) != Some(home) || function.instruction(used.user).opcode == Opcode::Phi
        })
    {
        return None;
    }
    Some(Plan { extension, compares, others })
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
        if !plan.others.is_empty() {
            let cut = function.create_instruction(
                Opcode::Cast(CastOp::Trunc),
                narrow,
                vec![extended],
                Flags::default(),
                None,
            );
            function.insert(cut, Position::After(plan.extension)).expect("a placed instruction");
            let cut = Operand::Value(function.instruction(cut).result.expect("a value"));
            for (user, at) in plan.others {
                let mut operands = function.instruction(user).operands.clone();
                operands[at as usize] = cut;
                function.set_operands(user, operands);
            }
        }
        debug_assert!(function.users(result).snapshot().iter().all(|used| used.user == plan.extension));
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "extload_tests.rs"]
mod tests;
