//! LLVM's `LowerExpectIntrinsic`: `llvm.expect` becomes branch weights and
//! goes.
//!
//! A translation of
//! [`LowerExpectIntrinsic.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LowerExpectIntrinsic.cpp)
//! at d1106deb71cc: a `br`, `switch` or `select` whose condition is an expect
//! call (or an `==`/`!=` of one against a constant) gets `!prof` weights, 2000
//! for the expected outcome and 1 for the others; the phi that defines the
//! value an expect call tests marks the branch that feeds it a constant it
//! does not expect; each call is replaced by its first argument.
//!
//! # How it works
//! `lower` visits the blocks in layout order, as LLVM's pass does: the block's
//! terminator first (`handleBrSelExpect`, `handleSwitchExpect`), then its
//! instructions backwards (a select before the call it tests is removed, a
//! phi definition before its call is erased).
//!
//! # Deviations
//! - `llvm.expect.with.probability` is not in MIR, so `getBranchWeight`'s
//!   probability arm, `Confidence` and `BranchCount - 1` are not translated.
//! - `misexpect::checkExpectAnnotations` and `checkFrontendInstrumentation`
//!   (profile-guided diagnostics) are not translated: llrm has no profile data.
//! - `setBranchWeights` states `!prof` `branch_weights` without the `expected`
//!   origin operand: no pass here reads it.
//!
//! # Example
//! ```mir
//! %e = call i32 @llvm.expect.i32(i32 %x, i32 0)
//! %c = icmp ne i32 %e, 0
//! br i1 %c, label %t, label %f
//! ```
//! becomes `%c = icmp ne i32 %x, 0` and `br i1 %c, ..., !prof
//! !{!"branch_weights", i32 1, i32 2000}`: the false edge is the likely one.
//!
//! # Used by
//! `pipeline` (`Phase::Early`, the first slot, as `EarlyFPM.addPass` at
//! PassBuilderPipelines.cpp:1177).

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, MetadataOperand, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, PreservedAnalyses, Unit};

/// `LikelyBranchWeight`.
const LIKELY: u32 = 2000;
/// `UnlikelyBranchWeight`.
const UNLIKELY: u32 = 1;

pub struct LowerExpect;

impl FunctionPass for LowerExpect {
    fn name(&self) -> &'static str {
        "lowerexpect"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("lower-expect")
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if lower(unit.context, outer.callees(), unit.function, unit.declared) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// The instruction that defines `operand`, if one does.
fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// `dyn_cast<ConstantInt>`: the zero-extended value.
fn constant_int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// Whether `inst` is a call of `llvm.expect`.
fn is_expect(
    context: &Context,
    callees: &Callees,
    function: &Function,
    inst: InstId,
) -> bool {
    llrm_mir::memory::callee(context, function, inst)
        .is_some_and(|callee| callees.get(&callee).is_some_and(|one| one.expect))
}

/// `MDB.createBranchWeights`, set as `inst`'s `!prof`.
fn set_weights(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    inst: InstId,
    weights: &[u32],
) {
    let word = context.types.int(32);
    let mut operands = vec![MetadataOperand::String("branch_weights".to_owned())];
    for weight in weights {
        operands.push(MetadataOperand::Constant(context.int(word, i128::from(*weight))));
    }
    let node = declared.node(MetadataNode { distinct: false, operands });
    function.set_metadata(inst, "prof", Some(node));
}

/// `lowerExpectIntrinsic`: whether anything changed.
fn lower(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    declared: &mut Declared,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        if let Some(terminator) = function.terminator(block) {
            match function.instruction(terminator).opcode {
                Opcode::Br if function.instruction(terminator).operands.len() == 3 => {
                    br_select(context, callees, function, declared, terminator);
                }
                Opcode::Switch => {
                    switch(context, callees, function, declared, terminator);
                }
                _ => {}
            }
        }
        // The instructions backwards: a select is handled before the call it
        // tests is removed.
        let mut at = function.block(block).last();
        while let Some(inst) = at {
            at = function.prev_in_block(inst);
            if function.instruction(inst).opcode == Opcode::Select {
                br_select(context, callees, function, declared, inst);
                continue;
            }
            if !is_expect(context, callees, function, inst) {
                continue;
            }
            phi_def(context, callees, function, declared, inst);
            let argument = function.instruction(inst).operands[0];
            if let Some(result) = function.instruction(inst).result {
                function.replace_all_uses_with(result, argument);
            }
            function.erase(inst).expect("an expect call whose uses were replaced erases");
            changed = true;
        }
    }
    changed
}

/// `handleBrSelExpect`: a `br` or a `select` on `expect(x, c)`, or on an
/// `==`/`!=` of it against a constant.
fn br_select(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    declared: &mut Declared,
    inst: InstId,
) -> bool {
    let condition = function.instruction(inst).operands[0];
    let compare =
        defining(function, condition).filter(|one| matches!(function.instruction(*one).opcode, Opcode::ICmp(_)));
    let (call, equal, compared) = match compare {
        None => {
            let Some(call) = defining(function, condition) else { return false };
            (call, false, 0_u128)
        }
        Some(cmp) => {
            let Opcode::ICmp(predicate) = function.instruction(cmp).opcode else { return false };
            if !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
                return false;
            }
            let operands = &function.instruction(cmp).operands;
            let Some(value) = constant_int(context, operands[1]) else { return false };
            if int_width(context, function, operands[1]) > 64 {
                return false;
            }
            let Some(call) = defining(function, operands[0]) else { return false };
            (call, predicate == IntPredicate::Eq, value)
        }
    };
    if !is_expect(context, callees, function, call) {
        return false;
    }
    let argument = function.instruction(call).operands[0];
    let Some(expected) = constant_int(context, function.instruction(call).operands[1]) else { return false };
    // `Predicate == ICMP_EQ` is false for the call alone, which is tested `!=
    // 0`.
    let weights = if (expected == compared) == (equal) { [LIKELY, UNLIKELY] } else { [UNLIKELY, LIKELY] };
    match compare {
        Some(cmp) => function.set_operand(cmp, 0, argument),
        None => function.set_operand(inst, 0, argument),
    }
    set_weights(context, declared, function, inst, &weights);
    true
}

/// The width of the integer `operand`.
fn int_width(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> u32 {
    function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty)).unwrap_or(0)
}

/// `handleSwitchExpect`.
fn switch(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    declared: &mut Declared,
    inst: InstId,
) -> bool {
    let Some(call) = defining(function, function.instruction(inst).operands[0]) else { return false };
    if !is_expect(context, callees, function, call) {
        return false;
    }
    let argument = function.instruction(call).operands[0];
    let Some(expected) = constant_int(context, function.instruction(call).operands[1]) else { return false };
    let cases = function.instruction(inst).operands[2..]
        .chunks_exact(2)
        .map(|pair| constant_int(context, pair[0]))
        .collect::<Option<Vec<_>>>();
    let Some(cases) = cases else { return false };
    let n = cases.len();
    let mut weights = vec![UNLIKELY; n + 1];
    // Weights are the default's, then each case's, in order; a value no case
    // names is the default.
    let index = cases.iter().position(|one| *one == expected).map_or(0, |at| at + 1);
    weights[index] = LIKELY;
    function.set_operand(inst, 0, argument);
    set_weights(context, declared, function, inst, &weights);
    true
}

/// `handlePhiDef`: a phi that defines the value `expect` tests, reached
/// through `zext`, `sext` and `xor` with a constant, marks the conditional
/// branch above each constant incoming value that contradicts the expected one.
fn phi_def(
    context: &mut Context,
    callees: &Callees,
    function: &mut Function,
    declared: &mut Declared,
    expect: InstId,
) {
    let _ = callees;
    let argument = function.instruction(expect).operands[0];
    let Some(expected) = constant_int(context, function.instruction(expect).operands[1]) else { return };
    // Walk up the copies to a phi; negation by `xor` of a constant is handled.
    let mut value = argument;
    let mut operations: Vec<InstId> = Vec::new();
    let phi = loop {
        let Some(def) = defining(function, value) else { return };
        match function.instruction(def).opcode {
            Opcode::Phi => break def,
            Opcode::Cast(CastOp::ZExt | CastOp::SExt) => {
                value = function.instruction(def).operands[0];
                operations.push(def);
            }
            Opcode::Binary(BinaryOp::Xor) => {
                if constant_int(context, function.instruction(def).operands[1]).is_none() {
                    return;
                }
                value = function.instruction(def).operands[0];
                operations.push(def);
            }
            _ => return,
        }
    };
    let phi_block = function.parent(phi);
    let pairs: Vec<(Operand, BlockId)> = function
        .instruction(phi)
        .operands
        .chunks(2)
        .filter_map(|pair| match pair[1] {
            Operand::Block(block) => Some((pair[0], block)),
            _ => None,
        })
        .collect();
    for (incoming, from) in pairs {
        let Some(constant) = constant_int(context, incoming) else { continue };
        // The recorded operations, innermost first, applied to the constant.
        let mut current = constant;
        let mut width = int_width(context, function, incoming);
        for &operation in operations.iter().rev() {
            let instruction = function.instruction(operation);
            match instruction.opcode {
                Opcode::Binary(BinaryOp::Xor) => {
                    let Some(mask) = constant_int(context, instruction.operands[1]) else { return };
                    current ^= mask;
                }
                Opcode::Cast(CastOp::ZExt) => width = result_width(context, function, operation),
                Opcode::Cast(CastOp::SExt) => {
                    let to = result_width(context, function, operation);
                    if width > 0 && width < 128 && (current >> (width - 1)) & 1 == 1 && to > width {
                        let high = if to >= 128 { u128::MAX } else { (1_u128 << to) - 1 };
                        current |= high & !((1_u128 << width) - 1);
                    }
                    width = to;
                }
                _ => return,
            }
        }
        // (1) the operand matches what is expected: nothing to infer.
        if current == expected {
            continue;
        }
        // The first conditional branch above the incoming block.
        let branch = conditional_branch(function, from);
        let Some(branch) = branch else { continue };
        let targets = {
            let operands = &function.instruction(branch).operands;
            [operands[1], operands[2]]
        };
        let comes_from = |successor: Operand| {
            let Operand::Block(successor) = successor else { return false };
            from == successor || (Some(from) == function.parent(branch) && Some(successor) == phi_block)
        };
        // The expected outcome is likely: the edge that carries the
        // contradicting constant is the unlikely one.
        let weights = if comes_from(targets[1]) {
            Some([LIKELY, UNLIKELY])
        } else if comes_from(targets[0]) {
            Some([UNLIKELY, LIKELY])
        } else {
            None
        };
        if let Some(weights) = weights {
            set_weights(context, declared, function, branch, &weights);
        }
    }
}

/// The width of the integer `inst` makes.
fn result_width(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> u32 {
    context.types.int_bits(function.instruction(inst).ty).unwrap_or(0)
}

/// `GetDomConditional`: the block's own conditional branch, else its sole
/// predecessor's.
fn conditional_branch(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    let conditional = |block: BlockId| {
        function
            .terminator(block)
            .filter(
                |one| function.instruction(*one).opcode == Opcode::Br && function.instruction(*one).operands.len() == 3,
            )
    };
    conditional(block).or_else(|| function.sole_predecessor(block).and_then(conditional))
}

/// `module`'s function `name` through the pass, the nodes it made placed.
#[cfg(test)]
pub fn lowered(
    module: &mut llrm_mir::Module,
    name: &str,
) -> Result<bool, String> {
    let callees = llrm_mir::memory::callees(module);
    let mut declared = Declared::of(module);
    let changed = {
        let (context, function) = module.function_mut(name).ok_or_else(|| format!("no @{name}"))?;
        lower(context, &callees, function, &mut declared)
    };
    declared.place(module)?;
    Ok(changed)
}

#[cfg(test)]
#[path = "lowerexpect_tests.rs"]
mod tests;
