//! `swapProfMetadata`: a branch or select whose two arms change places
//! takes its weights with them.
//!
//! Translation of
//! [`Instruction::swapProfMetadata`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Instruction.cpp)
//! at d1106deb71cc, as `CondBrInst::swapSuccessors` and
//! `SelectInst::swapValues` call it. Weights that are not `branch_weights` with
//! one per arm stay.
//!
//! # Used by
//! Every site that swaps the two arms of a `br` or `select` in place.

use llrm_mir::context::Context;
use llrm_mir::module::{Function, InstId, MetadataNode, MetadataOperand};
use llrm_mir::passes::Declared;

/// Swap `inst`'s arms (operands 1 and 2) and its `!prof` weights.
pub fn swap_arms(
    context: &Context,
    metadata: &[MetadataNode],
    declared: &mut Declared,
    function: &mut Function,
    inst: InstId,
) {
    let mut operands = function.instruction(inst).operands.clone();
    operands.swap(1, 2);
    function.set_operands(inst, operands);
    swap_prof(context, metadata, declared, function, inst);
}

/// `swapProfMetadata`.
pub fn swap_prof(
    _context: &Context,
    metadata: &[MetadataNode],
    declared: &mut Declared,
    function: &mut Function,
    inst: InstId,
) {
    let Some((_, id)) = function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "prof").cloned() else {
        return;
    };
    let node = match (id.0 as usize).checked_sub(metadata.len()) {
        None => metadata.get(id.0 as usize),
        Some(made) => declared.nodes().get(made),
    };
    let Some(node) = node else { return };
    if node.operands.len() != 3 || node.operands[0] != MetadataOperand::String("branch_weights".to_owned()) {
        return;
    }
    let swapped = MetadataNode {
        distinct: false,
        operands: vec![node.operands[0].clone(), node.operands[2].clone(), node.operands[1].clone()],
    };
    let swapped = declared.node(swapped);
    function.unannotate(inst, "prof");
    function.annotate(inst, "prof", swapped);
}

/// `extractBranchWeights(I, True, False)`: both weights of a two-way
/// `branch_weights`.
pub fn weights_of(
    context: &Context,
    metadata: &[MetadataNode],
    declared: &Declared,
    function: &Function,
    inst: InstId,
) -> Option<(u64, u64)> {
    let (_, id) = function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "prof")?;
    let node = match (id.0 as usize).checked_sub(metadata.len()) {
        None => metadata.get(id.0 as usize),
        Some(made) => declared.nodes().get(made),
    }?;
    if node.operands.len() != 3 || node.operands[0] != MetadataOperand::String("branch_weights".to_owned()) {
        return None;
    }
    let weight = |one: &MetadataOperand| match one {
        MetadataOperand::Constant(at) => match context.get(*at).kind {
            llrm_mir::context::ConstantKind::Int(bits) => Some(bits as u64),
            llrm_mir::context::ConstantKind::Zero => Some(0),
            _ => None,
        },
        _ => None,
    };
    Some((weight(&node.operands[1])?, weight(&node.operands[2])?))
}

/// `setFittedBranchWeights(I, {true, false}, false, ElideAllZero = true)`:
/// pushed right to fit 32 bits, and none at all when both are zero.
pub fn set_fitted_weights(
    context: &Context,
    declared: &mut Declared,
    function: &mut Function,
    inst: InstId,
    weights: [u64; 2],
) {
    function.unannotate(inst, "prof");
    let max = weights[0].max(weights[1]);
    if max == 0 {
        return;
    }
    // `fitWeights`: push right until the largest fits.
    let offset = if max > u64::from(u32::MAX) { 32 - max.leading_zeros() } else { 0 };
    let word = context.types.int(32);
    let mut operands = vec![MetadataOperand::String("branch_weights".to_owned())];
    for weight in weights {
        operands.push(MetadataOperand::Constant(context.int(word, i128::from(weight >> offset))));
    }
    let node = declared.node(MetadataNode { distinct: false, operands });
    function.annotate(inst, "prof", node);
}
