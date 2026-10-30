//! The code that opens the arms of a branch in the same way runs before the
//! branch once: `hoistCommonCodeFromSuccessors` of SimplifyCFG.
//!
//! # How it works
//! `hoisted` takes the terminator `branch` of a block whose successors each
//! have `branch`'s block as their only predecessor. It walks the successors in
//! step from their first instruction:
//!
//! 1. Instructions that are the same in every successor (the same operation,
//!    type and operands, a compare with its operands swapped, a commutative
//!    operation with them swapped) and that may move above the instructions
//!    skipped so far run once in the branch's block, and the copies go.
//! 2. Any other instruction is skipped, at most `SKIP_LIMIT` of them; what it
//!    reads, writes or does restricts what may move over it.
//! 3. When the terminators are reached and nothing was skipped, they are the
//!    same too: the branch is replaced by one terminator, and the phis of the
//!    successors it reaches take a `select` on the branch's condition for each
//!    pair of inputs that differ.
//!
//! With `all_equal_only` (the default slots) the walk starts only if every
//! successor is the same list of instructions, so nothing is skipped and the
//! successors empty out.
//!
//! # Example
//! The instructions both arms open with run once, before the branch:
//!
//! ```text
//!   b0: br %c, b1, b2            b0: %t = add %x, 1
//!   b1: %t1 = add %x, 1                br %j
//!       br %j              ==>   b1, b2: gone (no predecessor)
//!   b2: %t2 = add %x, 1
//!       br %j
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`hoistCommonCodeFromSuccessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1873) | `hoisted` | MIRROR; D1, D2, D3 |
//! | [`hoistSuccIdenticalTerminatorToSwitchOrIf`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L2103) | `terminator_hoisted` | MIRROR; D4 |
//! | [`skippedInstrFlags`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1538) | `skipped_flags` | MIRROR |
//! | [`isSafeToHoistInstr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1556) | `safe_to_hoist` | MIRROR |
//! | [`shouldHoistCommonInstructions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1594) | `should_hoist` | MIRROR; D2 |
//! | [`areIdenticalUpToCommutativity`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1680) | `identical_up_to_commutativity` | MIRROR; D3 |
//! | `simplifycfg-hoist-common-skip-limit` (20) | `SKIP_LIMIT` | MIRROR |
//!
//! - D1. No block has its address taken in the MIR, no pseudo probes, no
//!   debug-record lists to hoist in step (the debug records of a hoisted
//!   instruction move with it); `!mmra` tags are compared as written.
//! - D2. `TargetTransformInfo::isProfitableToHoist` is true (the base
//!   implementation, which the x86 target does not change); `nomerge` and
//!   `convergent` are not attributes of the MIR.
//! - D3. Instructions are compared by operation, type and operands (a call by
//!   its type, convention and tail kind, its attributes only intersecting); the
//!   flags are `and`ed.
//! - D4. No `invoke` and no `callbr` in the MIR; no
//!   `llvm.experimental.deoptimize`.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Opcode, Tail};

use crate::twoentryphi::Speculation;

/// `simplifycfg-hoist-common-skip-limit`.
const SKIP_LIMIT: u32 = 20;

const SKIP_READ_MEMORY: u32 = 1;
const SKIP_SIDE_EFFECT: u32 = 2;
const SKIP_IMPLICIT_CONTROL_FLOW: u32 = 4;

/// What an instruction left behind forbids to move over it.
fn skipped_flags(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
) -> u32 {
    let callees = spec.outer.callees();
    let mut flags = 0;
    if llrm_mir::memory::of(context, callees, function, inst).reads {
        flags |= SKIP_READ_MEMORY;
    }
    if crate::local::may_have_side_effects(context, callees, &spec.outer.globals, function, inst)
        || matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
    {
        flags |= SKIP_SIDE_EFFECT;
    }
    if !crate::local::is_guaranteed_to_transfer_execution_to_successor(context, &spec.outer.globals, function, inst) {
        flags |= SKIP_IMPLICIT_CONTROL_FLOW;
    }
    flags
}

/// Whether `inst` may be reordered across the instructions `flags` says were
/// skipped, and above its own block.
fn safe_to_hoist(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    inst: InstId,
    flags: u32,
) -> bool {
    let callees = spec.outer.callees();
    let memory = llrm_mir::memory::of(context, callees, function, inst);
    if flags & SKIP_READ_MEMORY != 0 && memory.writes {
        return false;
    }
    let side_effects = || {
        crate::local::may_have_side_effects(context, callees, &spec.outer.globals, function, inst)
            || matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })
    };
    if flags & SKIP_SIDE_EFFECT != 0 && (memory.reads || side_effects()) {
        return false;
    }
    if flags & SKIP_IMPLICIT_CONTROL_FLOW != 0
        && !crate::licm::hoist::is_safe_to_speculatively_execute_for_cfg(
            context,
            spec.layout,
            function,
            spec.outer,
            inst,
        )
    {
        return false;
    }
    // It cannot go above an operand its own block defines.
    let block = function.parent(inst);
    !function
        .instruction(inst)
        .operands
        .iter()
        .any(
            |operand| match operand {
                Operand::Value(value) => {
                    matches!(
                        function.value(*value).def,
                        ValueDef::Instruction(def) if function.parent(def) == block
                    )
                }
                _ => false,
            },
        )
}

/// Identical instructions `first` and `second` that can and should be hoisted
/// together.
fn should_hoist(
    function: &Function,
    first: InstId,
    second: InstId,
) -> bool {
    let tail = |inst: InstId| match &function.instruction(inst).opcode {
        Opcode::Call(info) => Some(info.tail == Tail::MustTail),
        _ => None,
    };
    !matches!((tail(first), tail(second)), (Some(a), Some(b)) if a != b)
}

fn commutative(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
    )
        || matches!(opcode, Opcode::Binary(BinaryOp::FAdd | BinaryOp::FMul))
}

/// The `!mmra` tags of `inst`: two instructions with different ones are not
/// hoisted together.
pub(crate) fn mmra(
    function: &Function,
    inst: InstId,
) -> Option<llrm_mir::module::MetadataId> {
    function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "mmra").map(|&(_, value)| value)
}

/// The same operation and type, and operands of the same types: two calls need
/// only attribute lists that intersect (`isSameOperationAs` with
/// `CompareUsingIntersectedAttrs`).
pub(crate) fn same_operation(
    context: &Context,
    function: &Function,
    first: InstId,
    second: InstId,
) -> bool {
    let (one, two) = (function.instruction(first), function.instruction(second));
    if one.ty != two.ty || one.operands.len() != two.operands.len() {
        return false;
    }
    // The operands' types are the same too: a store of an `i8` is not a store
    // of an `i16`.
    if one
        .operands
        .iter()
        .zip(&two.operands)
        .any(|(&a, &b)| function.operand_type(context, a) != function.operand_type(context, b))
    {
        return false;
    }
    match (&one.opcode, &two.opcode) {
        (Opcode::Call(a), Opcode::Call(b)) => {
            a.function_type == b.function_type
                && a.calling_convention == b.calling_convention
                && a.tail == b.tail
                && crate::local::attribute_list_intersect_with(
                    (&a.attrs, &a.return_attrs, &a.argument_attrs),
                    (&b.attrs, &b.return_attrs, &b.argument_attrs),
                )
                .is_some()
        }
        (a, b) => a == b,
    }
}

/// The same operation, type and operands, or a compare or a commutative
/// operation with its two operands swapped.
fn identical_up_to_commutativity(
    function: &Function,
    first: InstId,
    second: InstId,
) -> bool {
    let (one, two) = (function.instruction(first), function.instruction(second));
    if one.opcode == two.opcode && one.ty == two.ty && one.operands == two.operands {
        return true;
    }
    // Two calls whose attribute lists differ are the same where the lists
    // intersect (`isIdenticalToWhenDefined` with `IntersectAttrs`).
    if let (Opcode::Call(a), Opcode::Call(b)) = (&one.opcode, &two.opcode) {
        return one.ty == two.ty
            && one.operands == two.operands
            && a.function_type == b.function_type
            && a.calling_convention == b.calling_convention
            && a.tail == b.tail
            && crate::local::attribute_list_intersect_with(
                (&a.attrs, &a.return_attrs, &a.argument_attrs),
                (&b.attrs, &b.return_attrs, &b.argument_attrs),
            )
            .is_some();
    }
    if one.operands.len() < 2 || one.operands.len() != two.operands.len() || one.ty != two.ty {
        return false;
    }
    let swapped = one.operands[0] == two.operands[1] && one.operands[1] == two.operands[0];
    match (&one.opcode, &two.opcode) {
        (Opcode::ICmp(a), Opcode::ICmp(b)) => swapped && *a == b.swapped(),
        (Opcode::FCmp(a), Opcode::FCmp(b)) => swapped && *a == b.swapped(),
        (a, b) => commutative(a) && a == b && swapped && one.operands[2..] == two.operands[2..],
    }
}

/// The terminator of `branch`'s block, where the successors' instructions
/// before it were all hoisted, taken from the successors: `terminators` are
/// theirs, `first` the first.
fn terminator_hoisted(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    branch: InstId,
    terminators: &[InstId],
    unique: &[BlockId],
) -> bool {
    let first = terminators[0];
    let block = function.parent(branch).expect("a placed branch");
    let first_block = function.parent(first).expect("a placed terminator");
    let conditional = matches!(
        function.instruction(branch).operands[..],
        [_, Operand::Block(_), Operand::Block(_)]
    )
        && function.instruction(branch).opcode == Opcode::Br;
    if matches!(function.instruction(first).opcode, Opcode::Invoke(_)) {
        return false;
    }
    let successors = function.successors(first_block);
    for &successor in successors.iter() {
        for phi in crate::edges::phis(function, successor) {
            let arms = llrm_analysis::occurrence::arms(function, phi);
            let from = |at: BlockId| arms.iter().find(|&&(_, one)| one == at).map(|&(value, _)| value);
            let Some(value) = from(first_block) else { return false };
            for &other in &terminators[1..] {
                let at = function.parent(other).expect("a placed terminator");
                let Some(other_value) = from(at) else { return false };
                if value == other_value {
                    continue;
                }
                if !conditional
                    || crate::alwaysundef::always_undefined(context, spec, function, value, phi, false)
                    || crate::alwaysundef::always_undefined(context, spec, function, other_value, phi, false)
                {
                    return false;
                }
            }
        }
    }

    let new = function.clone_instruction(first);
    function.insert(new, Position::Before(branch)).expect("a new terminator");

    if conditional {
        let Operand::Block(second_block) = function.instruction(branch).operands[2] else { return false };
        let condition = function.instruction(branch).operands[0];
        let mut selected: Vec<((Operand, Operand), Operand)> = Vec::new();
        for &successor in successors.iter() {
            for phi in crate::edges::phis(function, successor) {
                let arms = llrm_analysis::occurrence::arms(function, phi);
                let at = |from: BlockId| arms.iter().position(|&(_, one)| one == from).expect("an arm");
                let (first_at, second_at) = (at(first_block), at(second_block));
                let (on_true, on_false) = (arms[first_at].0, arms[second_at].0);
                if on_true == on_false {
                    continue;
                }
                let chosen = match selected.iter().find(|(key, _)| *key == (on_true, on_false)) {
                    Some((_, chosen)) => *chosen,
                    None => {
                        let operation = function.instruction(phi);
                        let (ty, flags) = (operation.ty, operation.flags);
                        let select = function.create_instruction(
                            Opcode::Select,
                            ty,
                            vec![condition, on_true, on_false],
                            flags,
                            None,
                        );
                        function.insert(select, Position::Before(new)).expect("a new select");
                        let chosen = Operand::Value(function.instruction(select).result.expect("a select's value"));
                        selected.push(((on_true, on_false), chosen));
                        chosen
                    }
                };
                function.set_operand(phi, 2 * first_at, chosen);
                function.set_operand(phi, 2 * second_at, chosen);
            }
        }
    }

    // The successors the new terminator reaches have `block` for a
    // predecessor, with what `first_block` gave them.
    for &successor in successors.iter() {
        for phi in crate::edges::phis(function, successor) {
            let arms = llrm_analysis::occurrence::arms(function, phi);
            let value = arms.iter().find(|&&(_, one)| one == first_block).map(|&(value, _)| value).expect("an arm");
            let mut operands = function.instruction(phi).operands.clone();
            operands.push(value);
            operands.push(Operand::Block(block));
            function.set_operands(phi, operands);
        }
    }
    let condition = function.instruction(branch).operands.first().copied();
    function.erase(branch).expect("a terminator defines nothing");
    if let Some(Operand::Value(value)) = condition {
        crate::dead::dead_from(context, spec.outer.callees(), function, [value]);
    }
    let _ = unique;
    true
}

/// The code the successors of `branch` share runs before it. Whether
/// anything moved.
pub fn hoisted(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    branch: InstId,
    all_equal_only: bool,
) -> bool {
    let block = function.parent(branch).expect("a placed branch");
    let unique: Vec<BlockId> = function.successors(block).iter().copied().collect();
    if unique.len() < 2
        && function.instruction(branch).operands.iter().filter(|one| matches!(one, Operand::Block(_))).count() < 2
    {
        return false;
    }
    if unique.len() < 2 {
        return false;
    }
    for &successor in &unique {
        let alone = function.predecessors(successor).iter().all(|&one| one == block);
        let unreachable = function
            .iter_block(successor)
            .next()
            .is_some_and(|first| function.instruction(first).opcode == Opcode::Unreachable);
        if !alone && !unreachable {
            return false;
        }
    }
    // `AllInstsEqOnly`: successors that end differently match nowhere, so no
    // body is collected for them.
    if all_equal_only {
        let end = |successor: BlockId| function.terminator(successor).map(|last| function.instruction(last));
        let first = end(unique[0]);
        if !unique[1..].iter().all(|&successor| {
            let other = end(successor);
            other.map(|one| (&one.opcode, &one.operands)) == first.map(|one| (&one.opcode, &one.operands))
        }) {
            return false;
        }
    }
    // The cursors: each successor's instructions, where the walk is in them,
    // and the flags of those skipped.
    let mut bodies: Vec<(Vec<InstId>, usize, u32)> = Vec::new();
    for &successor in &unique {
        #[cfg(test)]
        BODY_COLLECTS.with(|count| count.set(count.get() + 1));
        let body: Vec<InstId> = function.iter_block(successor).collect();
        if function.instruction(body[0]).opcode == Opcode::Phi {
            return false;
        }
        bodies.push((body, 0, 0));
    }

    if all_equal_only {
        let size = bodies[0].0.len();
        let last = |at: usize| *bodies[at].0.last().expect("a terminator");
        let same_end = (1..bodies.len()).all(|at| {
            bodies[at].0.len() == size
                && function.instruction(last(at)).opcode == function.instruction(last(0)).opcode
                && function.instruction(last(at)).operands == function.instruction(last(0)).operands
        });
        if !same_end {
            return false;
        }
        for position in (0..size).rev() {
            let first = bodies[0].0[position];
            if (1..bodies.len()).any(|at| !identical_up_to_commutativity(function, first, bodies[at].0[position])) {
                return false;
            }
        }
    }

    // Unreachable successors do not take part when there are more than two.
    if bodies.len() > 2 {
        bodies.retain(|(body, _, _)| function.instruction(body[0]).opcode != Opcode::Unreachable);
        if bodies.len() < 2 {
            return false;
        }
    }

    let mut skipped = 0;
    let mut changed = false;
    loop {
        let current: Vec<InstId> = bodies.iter().map(|(body, at, _)| body[*at]).collect();
        let first = current[0];
        let mut identical = true;
        let mut terminator = function.instruction(first).opcode.is_terminator();
        for &other in &current[1..] {
            terminator |= function.instruction(other).opcode.is_terminator();
            if identical
                && (!identical_up_to_commutativity(function, first, other)
                    || mmra(function, first) != mmra(function, other))
            {
                identical = false;
            }
        }
        if terminator {
            if skipped > 0 || !identical {
                return changed;
            }
            return terminator_hoisted(context, spec, function, branch, &current, &unique) || changed;
        }
        if identical {
            identical = safe_to_hoist(context, spec, function, first, bodies[0].2)
                && bodies[1..].iter().zip(&current[1..]).all(|((_, _, flags), &other)| {
                    safe_to_hoist(context, spec, function, other, *flags) && should_hoist(function, first, other)
                });
        }
        // A `musttail` call stays with its `ret`.
        if identical
            && matches!(
                &function.instruction(first).opcode,
                Opcode::Call(info) if info.tail == Tail::MustTail
            )
        {
            identical = skipped == 0
                && bodies.iter().all(|(body, at, _)| function.instruction(body[*at + 1]).opcode == Opcode::Ret);
        }
        if identical {
            function.move_to(first, Position::Before(branch)).expect("a placed instruction");
            for (index, &other) in current.iter().enumerate().skip(1) {
                if let Some(result) = function.instruction(other).result {
                    let with = function.instruction(first).result.expect("a value like the other's");
                    function.replace_all_uses_with(result, Operand::Value(with));
                }
                crate::local::and_ir_flags(context, function, first, other);
                crate::local::try_intersect_attributes(function, first, other);
                crate::local::combine_metadata_for_cse(&spec.outer.metadata, function, first, other, true);
                function.erase(other).expect("a hoisted copy nothing reads");
                bodies[index].1 += 1;
            }
            bodies[0].1 += 1;
            changed = true;
        } else {
            if skipped >= SKIP_LIMIT {
                return changed;
            }
            for (index, &inst) in current.iter().enumerate() {
                let flags = skipped_flags(context, spec, function, inst);
                bodies[index].2 |= flags;
                bodies[index].1 += 1;
            }
            skipped += 1;
        }
    }
}

#[cfg(test)]
#[path = "hoistcommon_tests.rs"]
mod tests;

#[cfg(test)]
thread_local! {
    /// Successor bodies `hoisted` has collected on this thread.
    pub(crate) static BODY_COLLECTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
