//! A `switch` whose only predecessor compares the same value, simplified by
//! what that predecessor decided: LLVM's
//! [`simplifyEqualityComparisonWithOnlyPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L973),
//! the first step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
//! with
//! [`isValueEqualityComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L866),
//! [`getValueEqualityComparisonCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L899),
//! `eliminateBlockCases`
//! ([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L929))
//! and `valuesOverlap`
//! ([L935](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L935)).
//!
//! # How it works
//! A block with one predecessor that is a `switch` (or a compare of the value
//! with a constant) on the same value knows which constants reach it. Reached
//! by the predecessor's default, the cases the predecessor lists cannot
//! happen: they are removed from the `switch`. Reached by one case, the
//! constant is known: the `switch` becomes a `br` to the block that constant
//! picks. Dead edges lose their phi inputs.
//!
//! # Example
//! ```text
//!   b0: switch i16 %x, label %b1 [ 1, %ca  4, %ca ]       b0: switch i16 %x, label %b1 [ 1, %ca  4, %ca ]
//!   b1: switch i16 %x, label %cd [ 1, %cb  3, %cc ]   ==>  b1: switch i16 %x, label %cd [ 3, %cc ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights, no `DomTreeUpdater`. |
//! | D2 | A pointer compared with `inttoptr` of a constant is not a value comparison (`getConstantInt`'s `inttoptr` arm); `null` is. |
//! | D3 | The calls from `simplifyCondBranch` ([L8963](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8963)) are not made: only a `switch` is rewritten, a conditional branch is only read as the predecessor. Exit: call them from `simplifycfg::block_simplified` (llrm-w-4, 2026-10-27). |
//!
//! # Used by
//! `switchopt::simplified`, the first step.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::Type;
use llrm_support::hash::HashSet;

use crate::edges;
use crate::simplifycfg::{Options, condition_deleted};
use crate::twoentryphi::Speculation;

/// `ValueEqualityComparisonCase`: a constant the value may be, and where that
/// goes.
struct Case {
    value: u128,
    dest: BlockId,
}

/// `getConstantInt`: the bits of the integer `operand` is; a null pointer is 0.
fn constant_int(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match (&context.get(id).kind, context.types.get(context.get(id).ty)) {
        (ConstantKind::Int(bits), Type::Int(_)) => Some(*bits),
        (ConstantKind::Zero | ConstantKind::Null, Type::Int(_) | Type::Pointer(_)) => Some(0),
        _ => None,
    }
}

/// The instruction defining `operand`.
fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

/// `block` has at least `n` edges in, `hasNPredecessorsOrMore`.
fn has_predecessors_or_more(
    function: &Function,
    block: BlockId,
    n: usize,
) -> bool {
    function
        .block_users(block)
        .into_iter()
        .filter(|one| function.instruction(one.user).opcode.is_terminator())
        .take(n)
        .count()
        == n
}

/// `isValueEqualityComparison`: the value `terminator` compares with
/// constants.
fn comparison_value(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    terminator: InstId,
) -> Option<Operand> {
    let instruction = function.instruction(terminator);
    let value = match instruction.opcode {
        Opcode::Switch => {
            let edges = instruction.operands.iter().filter(|one| matches!(one, Operand::Block(_))).count();
            // Large switches are not merged into predecessors.
            let block = function.parent(terminator)?;
            (!has_predecessors_or_more(function, block, 128 / edges)).then_some(instruction.operands[0])
        }
        Opcode::Br if instruction.operands.len() == 3 => {
            let Operand::Value(condition) = instruction.operands[0] else { return None };
            if !function.users(condition).has_n_uses(1) {
                return None;
            }
            let def = function.instruction(defining(function, instruction.operands[0])?);
            match def.opcode {
                Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne) => {
                    constant_int(context, def.operands[1])?;
                    Some(def.operands[0])
                }
                Opcode::Cast(CastOp::Trunc) if def.flags.contains(Flags::NUW) => Some(def.operands[0]),
                _ => None,
            }
        }
        _ => None,
    }?;
    // Unwrap any lossless `ptrtoint`.
    if let Some(def) = defining(function, value).map(|inst| function.instruction(inst))
        && def.opcode == Opcode::Cast(CastOp::PtrToInt)
        && let Some(Type::Pointer(space)) =
            function.operand_type(context, def.operands[0]).map(|ty| context.types.get(ty))
        && context.types.int_bits(function.operand_type(context, value)?) == Some(layout.pointer(*space).bits)
    {
        return Some(def.operands[0]);
    }
    Some(value)
}

/// `getValueEqualityComparisonCases`: the cases `terminator` decodes to and
/// its default.
fn cases_of(
    context: &Context,
    function: &Function,
    terminator: InstId,
) -> (Vec<Case>, BlockId) {
    let instruction = function.instruction(terminator);
    let block = |operand: Operand| match operand {
        Operand::Block(block) => block,
        _ => unreachable!("a block operand"),
    };
    if instruction.opcode == Opcode::Switch {
        let cases = instruction.operands[2..]
            .chunks_exact(2)
            .map(|pair| {
                let value = constant_int(context, pair[0]).expect("a case is an integer");
                Case { value, dest: block(pair[1]) }
            })
            .collect();
        return (cases, block(instruction.operands[1]));
    }
    // `br i1 %c, T, F`: equal goes to T for `eq`, to F for `ne`.
    let (value, equal_is_true) =
        match defining(function, instruction.operands[0]).map(|inst| function.instruction(inst)) {
            Some(def) if matches!(def.opcode, Opcode::ICmp(_)) => {
                let value = constant_int(context, def.operands[1]).expect("a constant compare");
                (value, def.opcode == Opcode::ICmp(IntPredicate::Eq))
            }
            Some(_) => (0, false),
            None => unreachable!("a value comparison"),
        };
    let (equal, other) = (
        block(instruction.operands[1 + usize::from(!equal_is_true)]),
        block(instruction.operands[1 + usize::from(equal_is_true)]),
    );
    (vec![Case { value, dest: equal }], other)
}

/// `eliminateBlockCases`.
fn cases_without(
    cases: &mut Vec<Case>,
    block: BlockId,
) {
    cases.retain(|case| case.dest != block);
}

/// `valuesOverlap`: some constant is a case of both.
fn values_overlap(
    first: &[Case],
    second: &[Case],
) -> bool {
    let (smaller, larger) = if first.len() > second.len() { (second, first) } else { (first, second) };
    let seen: HashSet<u128> = smaller.iter().map(|case| case.value).collect();
    larger.iter().any(|case| seen.contains(&case.value))
}

/// `switch` (ending `block`) replaced by a jump to `dest`, its condition
/// deleted if nothing else reads it.
fn jump_made(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    switch: InstId,
    dest: BlockId,
) {
    let condition = function.instruction(switch).operands[0];
    let void = function.instruction(switch).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(dest)], Flags::default(), None);
    function.insert(jump, Position::Before(switch)).expect("a placed terminator");
    function.erase(switch).expect("a terminator defines nothing");
    condition_deleted(context, spec.outer.callees(), function, condition, true);
}

/// `simplifyEqualityComparisonWithOnlyPredecessor` for the `switch` of
/// `block`, whose one predecessor is `pred`: whether it was rewritten.
fn only_predecessor_simplified(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    switch: InstId,
    pred: BlockId,
) -> bool {
    let layout = spec.layout;
    let Some(pred_terminator) = function.terminator(pred) else { return false };
    let Some(pred_value) = comparison_value(context, layout, function, pred_terminator) else { return false };
    if comparison_value(context, layout, function, switch) != Some(pred_value) {
        return false;
    }

    // When control moves from `pred` to `block`.
    let (mut pred_cases, pred_default) = cases_of(context, function, pred_terminator);
    cases_without(&mut pred_cases, pred_default);
    // How control leaves `block`.
    let (mut this_cases, this_default) = cases_of(context, function, switch);
    cases_without(&mut this_cases, this_default);

    if pred_default == block {
        // The value is none of the predecessor's cases: those of this
        // switch are dead.
        if !values_overlap(&pred_cases, &this_cases) {
            return false;
        }
        let dead: HashSet<u128> = pred_cases.iter().map(|case| case.value).collect();
        let mut operands = function.instruction(switch).operands.clone();
        let mut at = operands.len();
        while at > 2 {
            at -= 2;
            let Operand::Block(dest) = operands[at + 1] else { unreachable!("a case target") };
            let value = constant_int(context, operands[at]).expect("a case is an integer");
            if dead.contains(&value) {
                function.predecessor_removed(context, dest, pred_default, false);
                // `removeCase`: the last case takes the removed one's place.
                let last = operands.len() - 2;
                operands.swap(at, last);
                operands.swap(at + 1, last + 1);
                operands.truncate(last);
            }
        }
        function.set_operands(switch, operands);
        return true;
    }

    // `block` is reached by the one constant of a case.
    let mut reaching = pred_cases.iter().filter(|case| case.dest == block);
    let (Some(known), None) = (reaching.next(), reaching.next()) else { return false };
    let real = this_cases.iter().find(|case| case.value == known.value).map_or(this_default, |case| case.dest);

    // Dead edges lose their phi inputs: one edge to `real` stays.
    let mut keep = Some(real);
    let targets: Vec<BlockId> = function
        .instruction(switch)
        .operands
        .iter()
        .filter_map(|one| if let Operand::Block(target) = one { Some(*target) } else { None })
        .collect();
    for target in targets {
        if Some(target) == keep {
            keep = None;
        } else {
            function.predecessor_removed(context, target, block, false);
        }
    }
    jump_made(context, spec, function, switch, real);
    true
}

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    _options: &Options,
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(switch) = function.terminator(block) else { return false };
    if comparison_value(context, spec.layout, function, switch).is_none() {
        return false;
    }
    // One predecessor that compares the same value.
    edges::single_predecessor(function, block)
        .is_some_and(|pred| only_predecessor_simplified(context, spec, function, block, switch, pred))
}

#[cfg(test)]
#[path = "switchvalue_tests.rs"]
mod tests;
