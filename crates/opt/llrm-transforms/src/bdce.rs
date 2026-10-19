//! Bit-tracking dead code elimination: a translation of LLVM's BDCE
//! (`BDCE.cpp` at d1106deb71cc). An instruction whose result bits nobody
//! reads goes, a sign extension whose extension bits are unread becomes a
//! zero extension, a mask that clears only unread bits is its operand, and an
//! operand none of whose bits is read becomes zero.
//!
//! # How it works
//! [`llrm_analysis::demandedbits::DemandedBits`] says, per instruction, which
//! result bits are read and which uses read nothing. The pass walks the
//! function once: it queues dead instructions, rewrites the three cases above,
//! and drops the poison-generating flags of everything downstream of a rewrite
//! that reads fewer than all bits (`clearAssumptionsOfUsers`), since `nsw`,
//! `nuw` and `exact` were proved of operands that changed. Queued
//! instructions are erased at the end.
//!
//! ```text
//!  %a = add i32 %x, 1
//!  %m = and i32 %a, 255     store i32 %m   ==>  nothing changes: all of %m is read
//!  %t = trunc i32 %m to i8
//!  %e = sext i8 %t to i32
//!  %w = and i32 %e, 255                    ==>  %e = zext i8 %t to i32
//! ```
//!
//! # DEVIATEs
//! 1. The demanded-bits analysis has no vectors and no `bswap`, `ctlz` and kin
//!    (see its header), so BDCE has none either.
//! 2. `dropPoisonGeneratingFlags` sets `ctlz`, `cttz` and `abs`'s
//!    poison-on-zero operand false; MIR has no such intrinsics.
//!
//! # Example
//! Before:
//! ```mir
//! define i32 @f(i8 %x) {
//! b0:
//!   %s = sext i8 %x to i32
//!   %m = and i32 %s, 255
//!   ret i32 %m
//! }
//! ```
//! After:
//! ```mir
//! define i32 @f(i8 %x) {
//! b0:
//!   %s = zext i8 %x to i32
//!   %m = and i32 %s, 255
//!   ret i32 %m
//! }
//! ```
//! From unit test
//! `test_a_sign_extension_nobody_reads_the_top_of_is_a_zero_extension`
//! in `bdce_tests.rs`.
//!
//! # Runs
//! LLVM runs it after SCCP, before ADCE (slot 34 of the pipeline). Not in
//! llrm's pipeline yet: `llrm-pass bdce FILE` runs it alone.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # See also
//! [`crate::dead`] removes the instructions nothing reads;
//! [`llrm_analysis::demandedbits`] is the analysis.

use llrm_analysis::demandedbits::{DemandedBits, Environment};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};

pub struct Bdce;

impl FunctionPass for Bdce {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "bdce"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("bdce")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if bit_tracking_dce(unit.context, unit.layout, analyses.outer(), unit.function) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

fn mask(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1_u128 << width) - 1 }
}

fn width_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).filter(|&width| width <= 128)
}

/// What one instruction becomes.
enum Rewrite {
    Remove,
    /// A sign extension of `Operand` read below its source's width.
    ZeroExtend(Operand),
    /// A mask that changes no read bit: the operand stands for it.
    Operand(Operand),
    /// These operands (by position) read no bit.
    Zero(Vec<usize>),
}

/// `clearAssumptionsOfUsers`: the users, below `inst`, whose poison-generating
/// flags were proved of operands that are about to change.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/BDCE.cpp#L44>
fn assumptions_to_clear(
    environment: &Environment,
    bits: &DemandedBits,
    function: &Function,
    inst: InstId,
) -> Vec<InstId> {
    let Some(result) = function.instruction(inst).result else { return Vec::new() };
    let Some(width) = width_of(environment.context, function, Operand::Value(result)) else { return Vec::new() };
    if bits.demanded_bits(environment, function, inst) == mask(width) {
        return Vec::new();
    }
    let integer_user = |user: InstId| {
        function
            .instruction(user)
            .result
            .is_some_and(|result| width_of(environment.context, function, Operand::Value(result)).is_some())
    };
    let mut visited = llrm_mir::dense::IdSet::<InstId>::new();
    let mut worklist: Vec<InstId> = Vec::new();
    for user in function.users(result).iter().map(|one| one.user) {
        if integer_user(user) {
            visited.insert(user);
            worklist.push(user);
        }
    }
    let mut cleared = Vec::new();
    while let Some(user) = worklist.pop() {
        cleared.push(user);
        let user_result = function.instruction(user).result.expect("an integer user has a result");
        let user_width = width_of(environment.context, function, Operand::Value(user_result)).expect("an integer");
        if bits.demanded_bits(environment, function, user) == mask(user_width) {
            continue;
        }
        for next in function.users(user_result).iter().map(|one| one.user) {
            if visited.insert(next) && integer_user(next) {
                worklist.push(next);
            }
        }
    }
    cleared
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/BDCE.cpp#L96>
fn rewrite_of(
    environment: &Environment,
    bits: &DemandedBits,
    function: &Function,
    inst: InstId,
) -> Option<Rewrite> {
    let instruction = function.instruction(inst);
    let integer = instruction.result.and_then(|result| width_of(environment.context, function, Operand::Value(result)));
    let removable = memory::only_value(environment.context, environment.callees, function, inst);
    // Skip what has side effects and no reader: nothing can be learned of it.
    if !removable && instruction.result.is_none_or(|result| function.users(result).is_empty()) {
        return None;
    }
    if bits.is_instruction_dead(environment, function, inst)
        || (integer.is_some() && bits.demanded_bits(environment, function, inst) == 0 && removable)
    {
        return Some(Rewrite::Remove);
    }
    let result = instruction.result.map(Operand::Value);
    if let (Opcode::Cast(CastOp::SExt), Some(destination)) = (&instruction.opcode, integer) {
        let demanded = bits.demanded_bits(environment, function, inst);
        let source = width_of(environment.context, function, instruction.operands[0]).expect("an integer source");
        let leading_zeros = destination - (128 - demanded.leading_zeros());
        if Some(instruction.operands[0]) != result && leading_zeros >= destination - source {
            return Some(Rewrite::ZeroExtend(instruction.operands[0]));
        }
    }
    if let (Opcode::Binary(op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)), Some(width)) =
        (&instruction.opcode, integer)
    {
        let demanded = bits.demanded_bits(environment, function, inst);
        if demanded != mask(width)
            && let Operand::Constant(id) = instruction.operands[1]
            && let ConstantKind::Int(constant) = environment.context.get(id).kind
        {
            let constant = constant & mask(width);
            let simplified = match op {
                BinaryOp::Or | BinaryOp::Xor => demanded & constant == 0,
                _ => demanded & !constant == 0,
            };
            if simplified && result != Some(instruction.operands[0]) {
                return Some(Rewrite::Operand(instruction.operands[0]));
            }
        }
    }
    let dead: Vec<usize> = instruction
        .operands
        .iter()
        .enumerate()
        .filter(|&(index, &operand)| {
            matches!(operand, Operand::Value(_))
                && width_of(environment.context, function, operand).is_some()
                && bits.is_use_dead(environment, function, llrm_mir::module::Use { user: inst, index: index as u32 })
        })
        .map(|(index, _)| index)
        .collect();
    (!dead.is_empty()).then_some(Rewrite::Zero(dead))
}

/// `bitTrackingDCE`: whether anything changed.
pub fn bit_tracking_dce(
    context: &mut Context,
    layout: &DataLayout,
    outer: &Outer,
    function: &mut Function,
) -> bool {
    let callees = outer.callees();
    let globals: &[_] = &outer.globals;
    let metadata: &[_] = &outer.metadata;
    let bits = DemandedBits::of(&Environment { context, layout, callees, globals, metadata }, function);
    let mut removed: Vec<InstId> = Vec::new();
    let mut changed = false;
    for inst in function.walk().map(|(_, inst)| inst).collect::<Vec<_>>() {
        let environment = Environment { context, layout, callees, globals, metadata };
        let Some(rewrite) = rewrite_of(&environment, &bits, function, inst) else { continue };
        let clearing = match &rewrite {
            Rewrite::Remove => Vec::new(),
            _ => assumptions_to_clear(&environment, &bits, function, inst),
        };
        for user in clearing {
            function.drop_poison_generating_annotations(user);
        }
        changed = true;
        match rewrite {
            Rewrite::Remove => removed.push(inst),
            Rewrite::ZeroExtend(source) => {
                let instruction = function.instruction(inst);
                let (ty, result) = (instruction.ty, instruction.result.expect("a value"));
                let name = function.value(result).name.clone();
                let widened = function.create_instruction(
                    Opcode::Cast(CastOp::ZExt),
                    ty,
                    vec![source],
                    Flags::default(),
                    name.as_deref(),
                );
                function.insert(widened, llrm_mir::edit::Position::Before(inst)).expect("placed before the original");
                let widened_result = function.instruction(widened).result.expect("a value");
                function.replace_all_uses_with(result, Operand::Value(widened_result));
                removed.push(inst);
            }
            Rewrite::Operand(operand) => {
                let result = function.instruction(inst).result.expect("a value");
                function.replace_all_uses_with(result, operand);
                removed.push(inst);
            }
            Rewrite::Zero(positions) => {
                for index in positions {
                    let ty = function.operand_type(context, function.instruction(inst).operands[index]).expect("typed");
                    let zero = context.constant(Constant { ty, kind: ConstantKind::Int(0) });
                    function.set_operand(inst, index, Operand::Constant(zero));
                }
            }
        }
    }
    for &inst in removed.iter().rev() {
        function.set_operands(inst, Vec::new());
    }
    for &inst in &removed {
        function.erase(inst).expect("nothing reads it");
    }
    changed
}

#[cfg(test)]
#[path = "bdce_tests.rs"]
mod tests;
