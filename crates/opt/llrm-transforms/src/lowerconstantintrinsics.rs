//! `llvm.is.constant` made a constant: true where its argument is a manifest
//! constant at this point of the pipeline, false otherwise.
//!
//! # How it works
//! Each `is.constant` call of a reachable block, in order, is replaced by its
//! answer and the users are simplified in turn
//! (`replaceAndRecursivelySimplify`). A conditional branch whose condition
//! becomes a constant takes its one edge, and the blocks that leaves
//! unreachable are deleted.
//!
//! `llvm.objectsize` is the pin's second case (`lowerObjectSizeCall`) and is
//! answered as unknown: all ones, or zero where the minimum is asked.
//!
//! DEVIATE 1: the size of a known object (an alloca, a global) is not
//! computed, so `objectsize` of one is also unknown. The answer is sound,
//! as it is for the pin with no object visitor result. Exit: the object-size
//! visitor of `MemoryBuiltins.cpp` on top of `getBaseObjectSize`.
//!
//! # Example
//! ```text
//! %c = call i1 @llvm.is.constant.i16(i16 %x)    ->    br i1 false, ...   ->   br label %slow
//! ```
//!
//! # Runs
//! After Float2Int in the optimizing pipeline (slot 1597), and once more before
//! instruction selection at every level, as `PreISelIntrinsicLowering` does:
//! the backend has no `is.constant`.
//!
//! # See also
//! LLVM's `lowerConstantIntrinsics`
//! ([LowerConstantIntrinsics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LowerConstantIntrinsics.cpp#L101)).

use llrm_mir::constantfolding::is_manifest_constant;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::simplify::simplify_instruction_in;

use crate::simplifycfg::dead_blocks_deleted;

pub struct LowerConstantIntrinsics;

impl FunctionPass for LowerConstantIntrinsics {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "lower-constant-intrinsics"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("lower-constant-intrinsics")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let globals = &analyses.outer().globals;
        if !lowered(unit.context, unit.layout, globals, unit.function) {
            return PreservedAnalyses::all();
        }
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Lowered {
    IsConstant,
    ObjectSize,
}

fn constant_intrinsic(
    context: &Context,
    globals: &[GlobalValue],
    function: &Function,
    inst: InstId,
) -> Option<Lowered> {
    let callee = llrm_mir::memory::callee(context, function, inst)?;
    let name = globals.get(callee.0 as usize)?.name.as_deref()?;
    let named = |base: &str| name.strip_prefix(base).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
    if named("llvm.is.constant") {
        Some(Lowered::IsConstant)
    } else if named("llvm.objectsize") {
        Some(Lowered::ObjectSize)
    } else {
        None
    }
}

fn reachable(function: &Function) -> IdSet<BlockId> {
    let mut reached = IdSet::new();
    let Some(entry) = function.entry() else { return reached };
    reached.insert(entry);
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        for successor in function.successors(block) {
            if reached.insert(successor) {
                pending.push(successor);
            }
        }
    }
    reached
}

/// `lowerConstantIntrinsics`, for `is.constant`: whether any call was lowered.
pub fn lowered(
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &mut Function,
) -> bool {
    let reached = reachable(function);
    let calls: Vec<(InstId, Lowered)> = function
        .layout()
        .iter()
        .copied()
        .filter(|block| reached.contains(block))
        .flat_map(|block| function.iter_block(block).collect::<Vec<_>>())
        .filter_map(|inst| Some((inst, constant_intrinsic(context, globals, function, inst)?)))
        .collect();
    if calls.is_empty() {
        return false;
    }
    let mut branches = Vec::new();
    for (call, which) in calls {
        if function.is_erased(call) {
            continue;
        }
        let instruction = function.instruction(call);
        let value = match which {
            Lowered::IsConstant => u128::from(matches!(
                instruction.operands.first(),
                Some(Operand::Constant(id)) if is_manifest_constant(context, *id)
            )),
            // Unknown: all ones, or zero where the minimum is asked (operand 1).
            Lowered::ObjectSize => match instruction.operands.get(1) {
                Some(Operand::Constant(id)) if !matches!(context.get(*id).kind, ConstantKind::Int(0)) => 0,
                _ => u128::MAX,
            },
        };
        let bits = match context.types.get(instruction.ty) {
            llrm_mir::types::Type::Int(bits) => u32::from(*bits),
            _ => 1,
        };
        let answer = context.constant(Constant { ty: instruction.ty, kind: ConstantKind::Int(value & (u128::MAX >> (128 - bits))) });
        replaced_and_simplified(context, layout, globals, function, call, Operand::Constant(answer), &mut branches);
    }
    let mut dead_blocks = false;
    for branch in branches {
        if function.is_erased(branch) {
            continue;
        }
        dead_blocks |= branch_folded(context, function, branch);
    }
    if dead_blocks {
        let reached = reachable(function);
        let dead: Vec<BlockId> = function.layout().iter().copied().filter(|block| !reached.contains(block)).collect();
        dead_blocks_deleted(context, function, &dead);
    }
    true
}

/// `replaceAndRecursivelySimplify`: `inst` replaced by `with`, then each user
/// that simplifies by what it equals. A conditional branch is left for the
/// caller in `unsimplified`.
fn replaced_and_simplified(
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &mut Function,
    inst: InstId,
    with: Operand,
    unsimplified: &mut Vec<InstId>,
) {
    let mut worklist = Vec::new();
    let mut pending = vec![(inst, with)];
    while let Some((one, value)) = pending.pop() {
        let Some(result) = function.instruction(one).result else { continue };
        worklist.extend(function.users(result).iter().map(|user| user.user));
        function.replace_all_uses_with(result, value);
        function.erase(one).expect("its uses were replaced");
        while let Some(user) = worklist.pop() {
            if function.is_erased(user) {
                continue;
            }
            match simplify_instruction_in(context, globals, layout, None, function, user) {
                Some(found) if function.instruction(user).result.is_some() => pending.push((user, found)),
                Some(_) => {}
                None => {
                    let branch = function.instruction(user);
                    if matches!(branch.opcode, Opcode::Br)
                        && branch.operands.len() == 3
                        && !unsimplified.contains(&user)
                    {
                        unsimplified.push(user);
                    }
                }
            }
        }
    }
}

/// The conditional branch `branch` on a constant as a jump to its one
/// successor. Whether the other successor lost its last predecessor.
fn branch_folded(
    context: &Context,
    function: &mut Function,
    branch: InstId,
) -> bool {
    let instruction = function.instruction(branch);
    let (Operand::Constant(condition), Operand::Block(then), Operand::Block(other)) =
        (instruction.operands[0], instruction.operands[1], instruction.operands[2])
    else {
        return false;
    };
    let (target, other) = match context.get(condition).kind {
        ConstantKind::Int(0) | ConstantKind::Zero => (other, then),
        ConstantKind::Int(1) => (then, other),
        _ => return false,
    };
    if target == other {
        return false;
    }
    let source = function.parent(branch).expect("a placed branch");
    function.predecessor_removed(context, other, source, false);
    let void = function.instruction(branch).ty;
    function.erase(branch).expect("a branch nothing reads");
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(target)], Flags::default(), None);
    function.insert(jump, Position::End(source)).expect("a block to end");
    function.predecessors(other).is_empty()
}

/// `PreISelIntrinsicLowering` for `is.constant`: every function of `module`
/// lowered, at any optimization level. A module with no call is untouched.
pub fn module_lowered(module: &mut llrm_mir::module::Module) -> Result<(), String> {
    let has_call = |module: &llrm_mir::module::Module| {
        module.globals.iter().any(|global| {
            let llrm_mir::module::GlobalKind::Function(function) = &global.kind else { return false };
            function.layout().iter().any(|&block| {
                function.iter_block(block).any(|inst| constant_intrinsic(&module.context, &module.globals, function, inst).is_some())
            })
        })
    };
    if !has_call(module) {
        return Ok(());
    }
    let layout = module.datalayout.as_deref().map_or_else(|| Ok(DataLayout::default()), DataLayout::parse)?;
    let snapshot = module.globals.clone();
    for global in &mut module.globals {
        if let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind {
            lowered(&module.context, &layout, &snapshot, function);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "lowerconstantintrinsics_tests.rs"]
mod tests;
