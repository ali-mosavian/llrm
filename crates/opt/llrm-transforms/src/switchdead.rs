//! Cases of a `switch` that cannot happen removed, and a switch over a
//! three-way compare made a branch: LLVM's
//! [`eliminateDeadSwitchCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6365)
//! with `createUnreachableSwitchDefault`
//! ([L6178](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6178))
//! and `collectPossibleValues`
//! ([L11052](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L11052)),
//! and
//! [`simplifySwitchOfCmpIntrinsic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8242),
//! steps of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).
//!
//! # How it works
//! A case is dead when the known bits of the value rule it out, it needs
//! more significant bits than the value has, or the value is one of at most
//! four constants (through `select` and `phi`) that it is not. With no case
//! dead, cases that cover every value the unknown bits allow make the
//! default unreachable, and one missing case is given to the default's
//! block. A `switch` on a `scmp`/`ucmp` with two destinations is a compare
//! and a conditional branch.
//!
//! # Example
//! ```text
//!   %c = and i32 %a, 3                   %c = and i32 %a, 3
//!   switch i32 %c, label %d [            switch i32 %c, label %d [
//!     i32 1, label %x                      i32 1, label %x
//!     i32 2, label %y                ==>   i32 2, label %y ]
//!     i32 5, label %z ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights, no `DomTreeUpdater`, no assumption cache. |
//! | D2 | `collectPossibleValues` takes only integer, zero, null, undef and poison constants as constants. |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context, signed};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::known::compute_known_bits;
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_mir::valuetracking::signbits::compute_max_significant_bits;
use llrm_support::hash::HashSet;

use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;

/// `getSignificantBits` of the `width`-bit `bits`.
fn significant_bits(
    bits: u128,
    width: u32,
) -> u32 {
    let value = signed(bits, width);
    let magnitude = if value < 0 { !value } else { value };
    128 - magnitude.leading_zeros() + 1
}

/// `collectPossibleValues(Cond, Values, MaxCount)`: the constants `cond` can
/// be, when it is a tree of `select`s and `phi`s over at most `max` of them.
fn possible_values(
    q: &Query,
    cond: Operand,
    max: usize,
) -> Option<HashSet<ConstantId>> {
    let mut constants = HashSet::default();
    let mut visited: IdSet<InstId> = IdSet::new();
    let mut worklist: Vec<InstId> = Vec::new();
    let push = |operand: Operand,
                constants: &mut HashSet<ConstantId>,
                visited: &mut IdSet<InstId>,
                worklist: &mut Vec<InstId>|
     -> bool {
        match operand {
            Operand::Constant(constant) => {
                if !matches!(
                    q.context.get(constant).kind,
                    ConstantKind::Int(_)
                        | ConstantKind::Zero
                        | ConstantKind::Null
                        | ConstantKind::Undef
                        | ConstantKind::Poison
                ) {
                    return false;
                }
                // Check existence first.
                if constants.contains(&constant) {
                    return true;
                }
                if constants.len() == max {
                    return false;
                }
                constants.insert(constant);
                true
            }
            _ => match q.instruction_of(operand) {
                Some((inst, _)) => {
                    if visited.insert(inst) {
                        worklist.push(inst);
                    }
                    true
                }
                None => false,
            },
        }
    };
    if !push(cond, &mut constants, &mut visited, &mut worklist) {
        return None;
    }
    while let Some(inst) = worklist.pop() {
        let instruction = q.function.instruction(inst);
        match instruction.opcode {
            Opcode::Select => {
                if !push(instruction.operands[1], &mut constants, &mut visited, &mut worklist)
                    || !push(instruction.operands[2], &mut constants, &mut visited, &mut worklist)
                {
                    return None;
                }
            }
            Opcode::Phi => {
                let own = instruction.result.map(Operand::Value);
                for pair in instruction.operands.chunks_exact(2) {
                    // Fast path for a recurrence phi.
                    if Some(pair[0]) == own {
                        continue;
                    }
                    if !push(pair[0], &mut constants, &mut visited, &mut worklist) {
                        return None;
                    }
                }
            }
            _ => return None,
        }
    }
    Some(constants)
}

/// `createUnreachableSwitchDefault`: the default of `switch` is a new block
/// of one `unreachable`, the old default losing the edge unless `keep`.
fn unreachable_default(
    context: &Context,
    function: &mut Function,
    block: BlockId,
    switch: InstId,
    keep: bool,
) {
    let Operand::Block(orig) = function.instruction(switch).operands[1] else { unreachable!("a default") };
    if !keep {
        function.predecessor_removed(context, orig, block, false);
    }
    let name = format!("{}.unreachabledefault", function.block(block).name.as_deref().unwrap_or(""));
    let new = function.create_block(Some(&name));
    function.insert_block_before(new, orig).expect("a new block");
    let void = function.instruction(switch).ty;
    let stop = function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
    function.insert(stop, Position::End(new)).expect("a placed block");
    function.set_operand(switch, 1, Operand::Block(new));
}

/// `eliminateDeadSwitchCases`.
fn dead_cases_eliminated(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    switch: InstId,
) -> bool {
    let operands = function.instruction(switch).operands.clone();
    let cond = operands[0];
    let ty = function.operand_type(context, cond).expect("a switch value");
    let width = context.types.int_bits(ty).expect("an integer switch");
    let q = Query {
        context,
        globals: &spec.outer.globals,
        metadata: &spec.outer.metadata,
        function: &*function,
        layout: spec.layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: Some(switch),
    };
    let known = compute_known_bits(&q, cond, 0);
    let mut known_values = possible_values(&q, cond, 4);
    // Cases outside the significant bits of the value are dead too.
    let max_significant = compute_max_significant_bits(&q, cond, 0);

    let case_values: Vec<ConstantId> = operands[2..]
        .chunks_exact(2)
        .map(|pair| match pair[0] {
            Operand::Constant(constant) => constant,
            _ => unreachable!("a case value is constant"),
        })
        .collect();
    let bits_of = |constant: ConstantId| match context.get(constant).kind {
        ConstantKind::Int(bits) => bits,
        _ => 0,
    };
    let mut dead: Vec<ConstantId> = Vec::new();
    for &case in &case_values {
        let bits = bits_of(case);
        let is_dead = known.zero & bits != 0
            || known.one & !bits != 0
            || significant_bits(bits, width) > max_significant
            || known_values.as_ref().is_some_and(|values| !values.contains(&case));
        if is_dead {
            dead.push(case);
        } else if let Some(values) = known_values.as_mut() {
            values.remove(&case);
        }
    }

    // Cases that cover every possible value make the default dead.
    let Operand::Block(default) = operands[1] else { unreachable!("a default") };
    let has_default = !crate::simplifycfg::only_unreachable(function, default);
    let unknown_bits = width - (known.zero | known.one).count_ones();
    if has_default && dead.is_empty() {
        if known_values.as_ref().is_some_and(|values| {
            values
                .iter()
                .all(
                    |&value| matches!(
                        context.get(value).kind,
                        ConstantKind::Undef | ConstantKind::Poison
                    ),
                )
        }) {
            unreachable_default(context, function, block, switch, false);
            return true;
        }
        if unknown_bits < 64 {
            let all_cases = 1u64 << unknown_bits;
            let cases = case_values.len() as u64;
            if cases == all_cases {
                unreachable_default(context, function, block, switch, false);
                return true;
            }
            // One value is missing: the default takes it as a case, which
            // opens the way to lookup tables.
            if cases == all_cases - 1 {
                if width > 64 || width > spec.layout.largest_legal_integer() {
                    return false;
                }
                let missing = case_values.iter().fold(0u128, |xor, &case| xor ^ bits_of(case));
                let missing = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(missing) }));
                let mut now = operands.clone();
                now.extend([missing, Operand::Block(default)]);
                function.set_operands(switch, now);
                unreachable_default(context, function, block, switch, true);
                return true;
            }
        }
    }

    if dead.is_empty() {
        return false;
    }
    for case in dead {
        let mut now = function.instruction(switch).operands.clone();
        let at =
            (2..now.len()).step_by(2).find(|&at| now[at] == Operand::Constant(case)).expect("a dead case is a case");
        // Prune the values the case's block took from this one.
        let Operand::Block(target) = now[at + 1] else { unreachable!("a case target") };
        function.predecessor_removed(context, target, block, false);
        // `removeCase`: the last case takes the removed one's place.
        let len = now.len();
        now.swap(at, len - 2);
        now.swap(at + 1, len - 1);
        now.truncate(len - 2);
        function.set_operands(switch, now);
    }
    true
}

/// `simplifySwitchOfCmpIntrinsic`.
fn cmp_intrinsic_folded(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    switch: InstId,
) -> bool {
    let operands = function.instruction(switch).operands.clone();
    let cond = operands[0];
    let (cmp, lhs, rhs, signed_cmp) = {
        let q = Query {
            context,
            globals: &spec.outer.globals,
            metadata: &spec.outer.metadata,
            function: &*function,
            layout: spec.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        };
        let found = [IntrinsicId::Scmp, IntrinsicId::Ucmp]
            .into_iter()
            .find_map(|id| pattern::intrinsic(&q, cond, id).map(|(inst, instruction)| (inst, instruction, id)));
        let Some((inst, instruction, id)) = found else { return false };
        let Operand::Value(result) = cond else { return false };
        if function.users(result).into_iter().count() != 1 {
            return false;
        }
        (inst, instruction.operands[0], instruction.operands[1], id == IntrinsicId::Scmp)
    };

    let Operand::Block(default) = operands[1] else { unreachable!("a default") };
    let cases: Vec<(i64, BlockId)> = operands[2..]
        .chunks_exact(2)
        .map(|pair| {
            let (Operand::Constant(value), Operand::Block(target)) = (pair[0], pair[1]) else { unreachable!("a case") };
            let ConstantKind::Int(bits) = context.get(value).kind else { unreachable!("an integer case") };
            let width = context.types.int_bits(context.get(value).ty).expect("an integer");
            // `trySExtValue`: only a value of at most 64 significant bits.
            (signed(bits, width).clamp(i64::MIN as i128, i64::MAX as i128) as i64, target)
        })
        .collect();
    let (succ, other, unreachable, res);
    if cases.len() == 2 {
        // Which of 1, 0 and -1 is missing is the default's.
        let mut missing = [1i64, 0, -1].to_vec();
        let mut other_succ: Option<BlockId> = None;
        for &(value, target) in &cases {
            let Some(at) = missing.iter().position(|&one| one == value) else { return false };
            missing.remove(at);
            if other_succ.is_some_and(|known| known != target) {
                return false;
            }
            other_succ = Some(target);
        }
        succ = default;
        other = other_succ.expect("two cases");
        unreachable = None;
        res = missing[0];
    } else if cases.len() == 3 && crate::simplifycfg::only_unreachable(function, default) {
        // Normalize so that `succ` is taken once and `other` twice.
        let (mut s, mut o): (Option<BlockId>, Option<BlockId>) = (None, None);
        for &(_, target) in &cases {
            if o.is_none() || o == Some(target) {
                o = Some(target);
            } else if s.is_none() {
                s = Some(target);
            } else if s == Some(target) {
                std::mem::swap(&mut s, &mut o);
            } else {
                return false;
            }
        }
        if cases.iter().any(|&(value, _)| !matches!(value, -1..=1)) {
            return false;
        }
        let Some(found) = s else { return false };
        succ = found;
        other = o.expect("three cases");
        unreachable = Some(default);
        res = cases.iter().find(|&&(_, target)| target == succ).expect("a case to succ").0;
    } else {
        return false;
    }

    // The predicate for the missing case.
    let mut pred = match res {
        1 => IntPredicate::Ugt,
        0 => IntPredicate::Eq,
        _ => IntPredicate::Ult,
    };
    if signed_cmp {
        pred = match pred {
            IntPredicate::Ugt => IntPredicate::Sgt,
            IntPredicate::Ult => IntPredicate::Slt,
            other => other,
        };
    }
    let i1 = context.types.int(1);
    let compare = function.create_instruction(Opcode::ICmp(pred), i1, vec![lhs, rhs], Flags::default(), None);
    function.insert(compare, Position::Before(switch)).expect("a placed terminator");
    let result = Operand::Value(function.instruction(compare).result.expect("a compare's value"));
    let void = function.instruction(switch).ty;
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![result, Operand::Block(succ), Operand::Block(other)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::Before(switch)).expect("a placed terminator");
    for (kind, node) in function.instruction(switch).metadata.clone() {
        if kind == "unpredictable" {
            function.annotate(branch, &kind, node);
        }
    }
    function.predecessor_removed(context, other, block, false);
    if let Some(unreachable) = unreachable {
        function.predecessor_removed(context, unreachable, block, false);
    }
    function.erase(switch).expect("a terminator defines nothing");
    function.erase(cmp).expect("a compare nothing reads");
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
    dead_cases_eliminated(context, spec, function, block, switch)
        || cmp_intrinsic_folded(context, spec, function, block, switch)
}

#[cfg(test)]
#[path = "switchdead_tests.rs"]
mod tests;
