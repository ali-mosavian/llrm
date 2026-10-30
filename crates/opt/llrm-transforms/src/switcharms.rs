//! `simplifyDuplicateSwitchArms`
//! ([L8564](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8564)),
//! `simplifySwitchWhenUMin`
//! ([L8027](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8027))
//! and `simplifySwitchDefaultBranch`
//! ([L8077](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8077)),
//! the last steps of `simplifySwitch`.
//!
//! # How it works
//! `folded` tries the three in LLVM's order and stops at the first that
//! changes the switch.
//! 1. Arms that are a jump alone to the same block, with the same phi inputs,
//!    are one: the switch goes to the first (`mergeIdenticalBBs`, shared with
//!    `sinkcommon`).
//! 2. A switch on `umin(a, C)` loses the cases above `C`. If its default is
//!    unreachable and it has the case `C`, that case becomes the default and
//!    the switch is on `a`.
//! 3. If the switched value is a known constant where the default runs, that
//!    value gets a case to the default's block and the default becomes
//!    unreachable.
//!
//! # Example
//! ```text
//!   %c = umin i16 %a, 3                      switch i16 %a, label %b3 [
//!   switch i16 %c, label %unreachable [        i16 0, label %b0
//!     i16 0, label %b0                         i16 1, label %b1 ]
//!     i16 1, label %b1
//!     i16 3, label %b3 ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights, no `DomTreeUpdater`. |
//! | D2 | Step 3 has no `AssumptionCache`, so only bits the value itself fixes are known. |
//! | D3 | Step 2 also removes the replaced default edge from the phis of the old default. |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_mir::context::Context;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::valuetracking::known::compute_known_bits;
use llrm_mir::valuetracking::{Query, pattern};

use crate::simplifycfg::{Options, integer, only_unreachable};
use crate::sinkcommon::{can_be_merged, identical_merged};
use crate::twoentryphi::Speculation;

/// A switch taken apart: the value, the default and the cases.
struct Parts {
    last: InstId,
    condition: Operand,
    default: BlockId,
    cases: Vec<(Operand, BlockId)>,
}

impl Parts {
    fn of(
        function: &Function,
        block: BlockId,
    ) -> Option<Self> {
        let last = function.terminator(block)?;
        let operands = &function.instruction(last).operands;
        let Operand::Block(default) = operands[1] else { return None };
        let cases = operands[2..]
            .chunks(2)
            .map(|pair| match *pair {
                [case, Operand::Block(target)] => (case, target),
                _ => unreachable!("a switch case is a value and a block"),
            })
            .collect();
        Some(Self { last, condition: operands[0], default, cases })
    }

    fn written(
        &self,
        function: &mut Function,
    ) {
        let mut operands = vec![self.condition, Operand::Block(self.default)];
        operands.extend(self.cases.iter().flat_map(|&(case, target)| [case, Operand::Block(target)]));
        function.set_operands(self.last, operands);
    }
}

/// `createUnreachableSwitchDefault`: the default is a new block of one
/// `unreachable`, before the old default. The old default loses its edge if
/// `remove_orig_default_block`.
fn unreachable_default_made(
    context: &Context,
    function: &mut Function,
    block: BlockId,
    parts: &mut Parts,
    remove_orig_default_block: bool,
) {
    let original = parts.default;
    if remove_orig_default_block {
        function.predecessor_removed(context, original, block, false);
    }
    let name = format!("{}.unreachabledefault", function.block(block).name.clone().unwrap_or_default());
    let new = function.create_block(Some(&name));
    function.insert_block_before(new, original).expect("a new block");
    let void = function.instruction(parts.last).ty;
    let trap = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(trap, llrm_mir::edit::Position::End(new)).expect("a placed block");
    parts.default = new;
    parts.written(function);
}

/// `simplifyDuplicateSwitchArms`.
fn duplicate_arms_merged(
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(parts) = Parts::of(function, block) else { return false };
    // The distinct successors, default first (`SmallSetVector`).
    let mut arms = vec![parts.default];
    for &(_, target) in &parts.cases {
        if !arms.contains(&target) {
            arms.push(target);
        }
    }
    arms.retain(|&arm| can_be_merged(function, arm));
    identical_merged(function, arms)
}

/// `m_UMin(m_Value(A), m_ConstantInt(C))`: `A` and `C`.
fn umin_match(
    q: &Query,
    declared: &mut Declared,
    value: Operand,
) -> Option<(Operand, u128)> {
    let (inst, instruction) = q.instruction_of(value)?;
    let width = q.scalar_bits(value);
    let constant = |operand| q.int_constant(operand).map(|(bits, _)| bits);
    if matches!(instruction.opcode, Opcode::Call(_)) {
        let callee = llrm_mir::memory::callee(q.context, q.function, inst)?;
        if declared.find(&format!("llvm.umin.i{width}")) != Some(callee) {
            return None;
        }
        return Some((instruction.operands[0], constant(instruction.operands[1])?));
    }
    // `(x pred y) ? x : y` is a minimum when `pred` is `ult` or `ule`.
    let (_, _, condition, if_true, if_false) = pattern::select(q, value)?;
    let (_, _, predicate, left, right) = pattern::icmp(q, condition)?;
    if (if_true != left || if_false != right) && (if_true != right || if_false != left) {
        return None;
    }
    let pred = if left == if_true { predicate.pred } else { predicate.pred.inverse() };
    matches!(pred, IntPredicate::Ult | IntPredicate::Ule).then_some(())?;
    Some((left, constant(right)?))
}

/// `simplifySwitchWhenUMin`.
fn umin_simplified(
    context: &Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(mut parts) = Parts::of(function, block) else { return false };
    let found = {
        let q = Query {
            context,
            globals: &[],
            metadata: &[],
            function: &*function,
            layout: spec.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: Some(parts.last),
        };
        umin_match(&q, declared, parts.condition)
    };
    let Some((a, constant)) = found else { return false };

    // Dead cases are removed even when the simplification fails: those above
    // the constant.
    let before = parts.cases.len();
    let mut kept = Vec::with_capacity(before);
    for (case, target) in std::mem::take(&mut parts.cases) {
        if integer(context, case).expect("a case is an integer") > constant {
            function.predecessor_removed(context, target, block, false);
        } else {
            kept.push((case, target));
        }
    }
    parts.cases = kept;
    let removed = parts.cases.len() != before;

    let at = parts.cases.iter().position(|&(case, _)| integer(context, case) == Some(constant));
    // Without the case `constant`, or with a default that can run, it fails.
    let Some(at) = at.filter(|_| only_unreachable(function, parts.default)) else {
        if removed {
            parts.written(function);
        }
        return removed;
    };
    let (_, target) = parts.cases.remove(at);
    function.predecessor_removed(context, parts.default, block, false);
    parts.default = target;
    parts.condition = a;
    parts.written(function);
    true
}

/// `simplifySwitchDefaultBranch`.
fn default_branch_simplified(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(mut parts) = Parts::of(function, block) else { return false };
    if only_unreachable(function, parts.default) {
        return false;
    }
    // If the value is some constant where the default runs, only that value
    // can reach it.
    let known = {
        let Some(first) =
            function.iter_block(parts.default).find(|&one| function.instruction(one).opcode != Opcode::Phi)
        else {
            return false;
        };
        let q = Query {
            context,
            globals: &[],
            metadata: &[],
            function: &*function,
            layout: spec.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: Some(first),
        };
        compute_known_bits(&q, parts.condition, 0)
    };
    if !known.is_constant() {
        return false;
    }
    let value = known.one;
    let exists = parts.cases.iter().any(|&(case, _)| integer(context, case) == Some(value));
    if !exists {
        let ty = function.operand_type(context, parts.condition).expect("a switched value has a type");
        let case = Operand::Constant(
            context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Int(value) }),
        );
        parts.cases.push((case, parts.default));
    }
    // With a case already there, the default edge goes instead of moving.
    unreachable_default_made(context, function, block, &mut parts, exists);
    true
}

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    _options: &Options,
    context: &Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
) -> bool {
    duplicate_arms_merged(function, block)
        || umin_simplified(context, spec, declared, function, block)
        || default_branch_simplified(context, spec, function, block)
}

#[cfg(test)]
#[path = "switcharms_tests.rs"]
mod tests;
