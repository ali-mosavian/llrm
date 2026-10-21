//! A `switch` on a value its predecessors compare with constants too, folded
//! with them: LLVM's
//! [`simplifyEqualityComparisonWithOnlyPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L973)
//! and
//! [`foldValueComparisonIntoPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1475)
//! with
//! [`performValueComparisonIntoPredecessorFolding`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1251),
//! the first steps of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
//! and their helpers
//! [`isValueEqualityComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L866),
//! [`getValueEqualityComparisonCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L899),
//! `eliminateBlockCases` ([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L929))
//! and `valuesOverlap` ([L935](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L935)).
//!
//! # How it works
//! A block with one predecessor that compares the same value (a `switch`, or
//! a branch on `x == C`) knows which constants reach it. Reached by the
//! predecessor's default, the cases the predecessor lists cannot happen: they
//! are removed. Reached by one case, the constant is known: the `switch`
//! becomes a `br` to the block that constant picks. Dead edges lose their phi
//! inputs.
//!
//! A block of only the `switch` is folded into every predecessor that
//! compares the same value: the predecessor becomes one `switch` that sends
//! each constant where the two together did (a pointer is cast to an integer
//! first). A successor both reach whose phis disagree gets a block of its
//! own on the `switch`'s edge first (`SplitBlockPredecessors`); an edge back
//! to the folded block goes to a new block that loops.
//!
//! # Example
//! ```text
//!   b0: switch i16 %x, label %b1 [ 1, %ca ]          b0: switch i16 %x, label %cd [ 1, %ca
//!   b1: switch i16 %x, label %cd [ 3, %cc ]    ==>                                  3, %cc ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights, no `DomTreeUpdater`. |
//! | D2 | A pointer compared with `inttoptr` of a constant is not a value comparison (`getConstantInt`'s `inttoptr` arm); `null` is. |
//! | D3 | The calls from `simplifyCondBranch` ([L8963](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8963)) are not made: only a `switch` is rewritten, a conditional branch is only read as a predecessor. Exit: call them from `simplifycfg::block_simplified` (llrm-w-4, 2026-10-27). |
//!
//! # Used by
//! `switchopt::simplified`, the first step.

use std::collections::BTreeMap;

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::Type;
use llrm_support::hash::HashSet;

use crate::branchtocommon::incompatible_successors;
use crate::edges;
use crate::simplifycfg::{Options, condition_deleted};
use crate::twoentryphi::Speculation;

/// `ValueEqualityComparisonCase`: a constant the value may be, and where that
/// goes.
#[derive(Clone, Copy)]
struct Case {
    constant: Operand,
    value: u128,
    dest: BlockId,
}

/// `getConstantInt`: the integer `operand` is, as an operand of an integer
/// type, and its bits; a null pointer is 0 of the pointer's width.
fn constant_int(
    context: &Context,
    layout: &DataLayout,
    operand: Operand,
) -> Option<(Operand, u128)> {
    let Operand::Constant(id) = operand else { return None };
    let constant = context.get(id);
    match (&constant.kind, context.types.get(constant.ty)) {
        (ConstantKind::Int(bits), Type::Int(_)) => Some((operand, *bits)),
        (ConstantKind::Zero | ConstantKind::Null, Type::Int(_)) => Some((operand, 0)),
        (ConstantKind::Zero | ConstantKind::Null, Type::Pointer(space)) => {
            let ty = context.types.int(layout.pointer(*space).bits);
            Some((Operand::Constant(context.int(ty, 0)), 0))
        }
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
                    constant_int(context, layout, def.operands[1])?;
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
    layout: &DataLayout,
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
                let (constant, value) = constant_int(context, layout, pair[0]).expect("a case is an integer");
                Case { constant, value, dest: block(pair[1]) }
            })
            .collect();
        return (cases, block(instruction.operands[1]));
    }
    // `br i1 %c, T, F`: equal goes to T for `eq`, to F for `ne`.
    let (constant, value, equal_is_true) =
        match defining(function, instruction.operands[0]).map(|inst| function.instruction(inst)) {
            Some(def) if matches!(def.opcode, Opcode::ICmp(_)) => {
                let (constant, value) = constant_int(context, layout, def.operands[1]).expect("a constant compare");
                (constant, value, def.opcode == Opcode::ICmp(IntPredicate::Eq))
            }
            Some(def) => {
                let ty = function.operand_type(context, def.operands[0]).expect("a truncated value");
                (Operand::Constant(context.int(ty, 0)), 0, false)
            }
            None => unreachable!("a value comparison"),
        };
    let (equal, other) = (
        block(instruction.operands[1 + usize::from(!equal_is_true)]),
        block(instruction.operands[1 + usize::from(equal_is_true)]),
    );
    (vec![Case { constant, value, dest: equal }], other)
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
    let (mut pred_cases, pred_default) = cases_of(context, layout, function, pred_terminator);
    cases_without(&mut pred_cases, pred_default);
    // How control leaves `block`.
    let (mut this_cases, this_default) = cases_of(context, layout, function, switch);
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
            let (_, value) = constant_int(context, layout, operands[at]).expect("a case is an integer");
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

/// `NewSuccessors`: the blocks a rebuilt `switch` reaches, in the order first
/// reached, with the number of edges to each.
#[derive(Default)]
struct NewSuccessors {
    order: Vec<BlockId>,
    edges: IdMap<BlockId, usize>,
}

impl NewSuccessors {
    fn add(
        &mut self,
        block: BlockId,
    ) {
        let count = self.edges.get_or_insert_with(block, || 0);
        if *count == 0 {
            self.order.push(block);
        }
        *count += 1;
    }
}

/// `performValueComparisonIntoPredecessorFolding`: the terminator `pred_terminator`
/// of a predecessor rebuilt as a `switch` that does what it and the `switch`
/// of `block` did together. A compared pointer is cast to an integer first,
/// and `value` becomes the cast.
fn folded_into_predecessor(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    switch: InstId,
    pred_terminator: InstId,
    value: &mut Operand,
) {
    let layout = spec.layout;
    let pred = function.parent(pred_terminator).expect("a placed terminator");
    let (block_cases, block_default) = cases_of(context, layout, function, switch);
    let (mut pred_cases, mut pred_default) = cases_of(context, layout, function, pred_terminator);
    let mut new_successors = NewSuccessors::default();

    if pred_default == block {
        // Only the edges of `block`'s switch that `pred` did not decide, or
        // that came here, are live.
        let mut handled: HashSet<u128> = HashSet::default();
        let mut at = 0;
        while at < pred_cases.len() {
            if pred_cases[at].dest == block {
                // The default is `block`: no explicit case.
                pred_cases.swap_remove(at);
            } else {
                handled.insert(pred_cases[at].value);
                at += 1;
            }
        }
        if pred_default != block_default {
            function.predecessor_removed(context, pred_default, pred, false);
            pred_default = block_default;
            new_successors.add(block_default);
        }
        for case in &block_cases {
            if !handled.contains(&case.value) && case.dest != block_default {
                pred_cases.push(*case);
                new_successors.add(case.dest);
            }
        }
    } else {
        // Only the constants `pred` sent here are live: where `block` sends
        // them, else its default.
        let mut handled: BTreeMap<u128, Operand> = BTreeMap::new();
        let mut at = 0;
        while at < pred_cases.len() {
            if pred_cases[at].dest == block {
                handled.insert(pred_cases[at].value, pred_cases[at].constant);
                pred_cases.swap_remove(at);
            } else {
                at += 1;
            }
        }
        for case in &block_cases {
            if handled.remove(&case.value).is_some() {
                pred_cases.push(*case);
                new_successors.add(case.dest);
            }
        }
        for (value, constant) in handled {
            pred_cases.push(Case { constant, value, dest: block_default });
            new_successors.add(block_default);
        }
    }

    // `pred` gains an edge, and each phi an input, for every new successor.
    for &successor in &new_successors.order {
        for _ in 0..new_successors.edges.get(&successor).copied().unwrap_or(0) {
            edges::predecessor_added(function, successor, pred, block);
        }
    }

    // Convert a pointer to an integer before the switch.
    let ty = function.operand_type(context, *value).expect("a compared value");
    if let Type::Pointer(space) = context.types.get(ty) {
        let int_ptr = context.types.int(layout.pointer(*space).bits);
        let cast = function.create_instruction(
            Opcode::Cast(CastOp::PtrToInt),
            int_ptr,
            vec![*value],
            Flags::default(),
            Some("magicptr"),
        );
        function.insert(cast, Position::Before(pred_terminator)).expect("a placed terminator");
        *value = Operand::Value(function.instruction(cast).result.expect("a cast's value"));
    }

    // The new switch is unpredictable only if both comparisons were.
    let unpredictable = |inst: InstId| {
        function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "unpredictable").map(|(_, node)| *node)
    };
    let unpredictable = unpredictable(pred_terminator).filter(|_| unpredictable(switch).is_some());
    let void = function.instruction(pred_terminator).ty;
    let mut operands = vec![*value, Operand::Block(pred_default)];
    operands.extend(pred_cases.iter().flat_map(|case| [case.constant, Operand::Block(case.dest)]));
    let new = function.create_instruction(Opcode::Switch, void, operands, Flags::default(), None);
    function.insert(new, Position::Before(pred_terminator)).expect("a placed terminator");
    if let Some(node) = unpredictable {
        function.annotate(new, "unpredictable", node);
    }
    let condition = function.instruction(pred_terminator).operands[0];
    function.erase(pred_terminator).expect("a terminator defines nothing");
    condition_deleted(context, spec.outer.callees(), function, condition, true);

    // `block` is still a successor only if it loops on itself: give that
    // path a block that loops.
    let mut infloop = None;
    for at in 1..function.instruction(new).operands.len() {
        if function.instruction(new).operands[at] != Operand::Block(block) {
            continue;
        }
        let looping = *infloop.get_or_insert_with(|| {
            let looping = function.create_block(Some("infloop"));
            function.insert_block(looping, None).expect("a new block");
            let jump =
                function.create_instruction(Opcode::Br, void, vec![Operand::Block(looping)], Flags::default(), None);
            function.insert(jump, Position::End(looping)).expect("a placed block");
            looping
        });
        function.set_operand(new, at, Operand::Block(looping));
    }
}

/// `foldValueComparisonIntoPredecessors` for the `switch` of `block`: every
/// predecessor that compares the same value is rebuilt to decide for both.
fn folded_into_predecessors(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    switch: InstId,
    mut value: Operand,
) -> bool {
    let mut changed = false;
    let mut preds: Vec<BlockId> = function.predecessors(block).iter().copied().collect();
    while let Some(pred) = preds.pop() {
        // Not into itself.
        if pred == block {
            continue;
        }
        let Some(pred_terminator) = function.terminator(pred) else { continue };
        if comparison_value(context, spec.layout, function, pred_terminator) != Some(value) {
            continue;
        }
        for successor in incompatible_successors(function, block, pred) {
            let split = edges::split_block_predecessors(function, successor, &[block]);
            // `SplitBlockPredecessors(.., ".fold.split")` places it before `successor`.
            let name = format!("{}.fold.split", function.block(successor).name.as_deref().unwrap_or(""));
            function.set_block_name(split, &name);
            let before = function.layout().iter().position(|&one| one == successor).and_then(|at| at.checked_sub(1));
            if let Some(after) = before.map(|at| function.layout()[at]) {
                function.move_block_after(split, after).expect("placed blocks");
            }
        }
        folded_into_predecessor(context, spec, function, block, switch, pred_terminator, &mut value);
        changed = true;
    }
    changed
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
    let Some(value) = comparison_value(context, spec.layout, function, switch) else { return false };
    // One predecessor that compares the same value.
    if edges::single_predecessor(function, block)
        .is_some_and(|pred| only_predecessor_simplified(context, spec, function, block, switch, pred))
    {
        return true;
    }
    // A block of only the switch folds into its predecessors.
    function.iter_block(block).next() == Some(switch)
        && folded_into_predecessors(context, spec, function, block, switch, value)
}

#[cfg(test)]
#[path = "switchvalue_tests.rs"]
mod tests;
