//! `turnSwitchRangeIntoICmp`, a step of `simplifySwitch`
//! ([L6205](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6205)),
//! with `findContiguousCases`
//! ([L6124](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6124)).
//!
//! A switch with two destinations, one of which gets a run of consecutive
//! case values, becomes a range compare and a branch.
//!
//! # How it works
//! The cases are split by destination, the default counting as the first. A
//! third destination ends the fold. A destination with one case, or whose
//! cases run without a gap (`findContiguousCases`), is the range: `x - min u<
//! count` (an `icmp eq` for one case) branches to it. When the value's range
//! spans exactly the first and last case of a set, the gap between the two
//! runs is the other destination's run. The phis of both destinations drop
//! the inputs of the cases that merged into one edge.
//!
//! # Example
//! ```text
//!   switch i16 %x, label %b [            %x.off = add i16 %x, -1
//!     i16 1, label %a                    %switch = icmp ult i16 %x.off, 3
//!     i16 2, label %a                    br i1 %switch, label %a, label %b
//!     i16 3, label %a ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights. |
//! | D2 | No `DomTreeUpdater`. |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_mir::context::{Constant, ConstantKind, Context, mask};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::range::compute_constant_range;

use crate::simplifycfg::{Options, integer, only_unreachable};
use crate::twoentryphi::Speculation;

/// `ContiguousCasesResult`; only the sizes of the case sets are read after.
struct Contiguous {
    min: u128,
    max: u128,
    dest: BlockId,
    other_dest: BlockId,
    cases: usize,
    other_cases: usize,
}

/// `findContiguousCases`; `cases` is sorted, largest first.
fn contiguous(
    q: &Query,
    condition: Operand,
    width: u32,
    cases: &mut [u128],
    other_cases: &[u128],
    dest: BlockId,
    other_dest: BlockId,
) -> Option<Contiguous> {
    let m = mask(width);
    cases.sort_unstable_by(|a, b| b.cmp(a));
    let (min, max) = (cases[cases.len() - 1], cases[0]);
    if max.wrapping_sub(min) & m == cases.len() as u128 - 1 {
        return Some(Contiguous { min, max, dest, other_dest, cases: cases.len(), other_cases: other_cases.len() });
    }
    let range = compute_constant_range(q, condition, false, 0);
    // A wrapping run, [min, other_min] and [other_max, max]: what lies
    // between is the other destination's run.
    if max == range.get_unsigned_max() && min == range.get_unsigned_min() {
        debug_assert!(cases.len() >= 2);
        let gap = cases.windows(2).position(|pair| pair[0] != pair[1].wrapping_add(1) & m)?;
        let (other_max, other_min) = (cases[gap], cases[gap + 1]);
        let covered = (max.wrapping_sub(other_max) & m) + (other_min.wrapping_sub(min) & m);
        if covered == cases.len() as u128 - 2 {
            return Some(Contiguous {
                min: other_min.wrapping_add(1) & m,
                max: other_max.wrapping_sub(1) & m,
                dest: other_dest,
                other_dest: dest,
                cases: other_cases.len(),
                other_cases: cases.len(),
            });
        }
    }
    None
}

/// Whether the `switch` ending `block` became a compare and a branch.
pub fn folded(
    _options: &Options,
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let operands = function.instruction(last).operands.clone();
    let (condition, Operand::Block(default)) = (operands[0], operands[1]) else { return false };
    let switch_cases: Vec<(u128, BlockId)> = operands[2..]
        .chunks(2)
        .map(|pair| match *pair {
            [case, Operand::Block(target)] => (integer(context, case).expect("a case is an integer"), target),
            _ => unreachable!("a switch case is a value and a block"),
        })
        .collect();
    // Folded away before LLVM gets here.
    if switch_cases.len() < 2 {
        return false;
    }
    let has_default = !only_unreachable(function, default);

    // Partition the cases into two sets with different destinations.
    let mut dest_a = has_default.then_some(default);
    let mut dest_b = None;
    let (mut cases_a, mut cases_b) = (Vec::new(), Vec::new());
    for &(value, dest) in &switch_cases {
        if dest == *dest_a.get_or_insert(dest) {
            cases_a.push(value);
        } else if dest == *dest_b.get_or_insert(dest) {
            cases_b.push(value);
        } else {
            return false; // More than two destinations.
        }
    }
    // All destinations are the same and the default is unreachable.
    let (Some(dest_a), Some(dest_b)) = (dest_a, dest_b) else { return false };

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
        cxt: Some(last),
    };
    let ty = q.type_of(condition).expect("a switched value has a type");
    let width = q.scalar_bits(condition);
    let only = |value, dest, other_dest, cases, other_cases| Contiguous {
        min: value,
        max: value,
        dest,
        other_dest,
        cases,
        other_cases,
    };
    // Only one icmp is needed when there is only one case.
    let found = if !has_default && cases_a.len() == 1 {
        Some(only(cases_a[0], dest_a, dest_b, 1, cases_b.len()))
    } else if cases_b.len() == 1 {
        Some(only(cases_b[0], dest_b, dest_a, 1, cases_a.len()))
    } else if !has_default {
        // Cases to the default destination cannot be contiguous cases.
        contiguous(&q, condition, width, &mut cases_a, &cases_b, dest_a, dest_b)
    } else {
        None
    }
    .or_else(|| contiguous(&q, condition, width, &mut cases_b, &cases_a, dest_b, dest_a));
    let Some(Contiguous { min, max, dest, other_dest, cases, other_cases }) = found else { return false };
    let name = match condition {
        Operand::Value(value) => function.value(value).name.clone().unwrap_or_default(),
        _ => String::new(),
    };

    // Start building the compare and branch.
    let void = function.instruction(last).ty;
    let i1 = context.types.int(1);
    let int = |value: u128| Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }));
    let place = |function: &mut Function, opcode, ty, operands, name: Option<&str>| {
        let new = function.create_instruction(opcode, ty, operands, Flags::default(), name);
        function.insert(new, Position::Before(last)).expect("a placed terminator");
        function.instruction(new).result.map(Operand::Value)
    };
    let count = max.wrapping_sub(min).wrapping_add(1) & mask(width);
    // If the count overflowed, every value jumps to the successor.
    let unconditional = count == 0 && cases != 0;
    let jump = if count == 1 {
        let compare = place(function, Opcode::ICmp(IntPredicate::Eq), i1, vec![condition, int(min)], None);
        vec![compare.expect("a compare's value"), Operand::Block(dest), Operand::Block(other_dest)]
    } else if unconditional {
        vec![Operand::Block(dest)]
    } else {
        let mut sub = condition;
        if min != 0 {
            let offset = vec![condition, int(min.wrapping_neg() & mask(width))];
            sub = place(function, Opcode::Binary(BinaryOp::Add), ty, offset, Some(&format!("{name}.off")))
                .expect("an add's value");
        }
        let compare = place(function, Opcode::ICmp(IntPredicate::Ult), i1, vec![sub, int(count)], Some("switch"));
        vec![compare.expect("a compare's value"), Operand::Block(dest), Operand::Block(other_dest)]
    };
    place(function, Opcode::Br, void, jump, None);

    // Prune obsolete incoming values off the successors' phis.
    let prune = |function: &mut Function, target: BlockId, edges: usize| {
        for phi in crate::edges::phis(function, target) {
            for _ in 0..edges {
                function.remove_incoming_value(context, phi, block, true);
            }
        }
    };
    prune(function, dest, (cases + usize::from(dest == default)).saturating_sub(1));
    let other_edges = (other_cases + usize::from(other_dest == default)).saturating_sub(1);
    prune(function, other_dest, other_edges + usize::from(unconditional));

    // Clean up the default block.
    if !has_default {
        function.predecessor_removed(context, default, block, false);
    }
    function.erase(last).expect("a terminator defines nothing");
    true
}

#[cfg(test)]
#[path = "switchrange_tests.rs"]
mod tests;
