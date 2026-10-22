//! Replaces every integer value [`llrm_analysis::consts`] knows by its
//! constant, folds a pure operation on each edge of a join, and puts compares
//! and identities in canonical form. A constant operand is free to encode and
//! lets the later passes see through the computation; the definition it
//! replaces is left dead.
//!
//! # How it works
//! Facts come from `consts`: "the low `width` bits of this value are `n`",
//! solved through registers, phis and memory (a load of a cell every reaching
//! store fills with one number is that number). What each call writes is
//! `manager::writes`, so a constant stored before a `readonly` call survives
//! it. A fact replaces its value only when it is as wide as the value. A
//! division or remainder of two known numbers is computed here too.
//!
//! Each use of a known value becomes the constant at once, so a store's value,
//! a call argument, a fill's byte or an address index need no case of their
//! own, and the definition is left unread for [`crate::dead`] to take. A
//! commutative operation that ends up with the constant on the left gets it
//! back on the right; an ordered one (`sub`, shifts) keeps its operand order.
//!
//! A call kills only the cells its writes may overlap
//! (`alias::calls_by_attributes`, from the call's `memory(...)`, `readonly` and
//! `readnone` attributes and `GlobalsModRef`); one that states writes without
//! an effect list clears every cell. A fact about reaching an edge says nothing
//! about a callee, so edge facts clear all cells at a call.
//!
//! ```text
//!  store i16 9, ptr @g          store i16 9, ptr @g
//!  %a = load i16, ptr @g   ==>  call void @clobber()   ; writes @g
//!  call void @clobber()         %b = load i16, ptr @g  ; unknown again
//!  %b = load i16, ptr @g        (%a is 9; with @clobber readonly, %b too)
//! ```
//!
//! One edge fold runs per call. At a block with two or more predecessors, a
//! pure `Binary` or `Cast` in the block (or in the single-predecessor chain
//! below it) that reads one of its phis, and whose answer is a number on every
//! incoming edge, becomes a phi of those numbers. An operation that also reads
//! an unknown value stays.
//!
//! A phi in one arm of a `br i1` that takes the branch's own condition from the
//! branch's block reads that arm's constant (GVN's `propagateEquality`, over
//! the edge's phi uses): loop rotation and `lcssa` make `phi i1 [%c, %latch]`
//! for a test after the loop, and no GVN runs after them. The branch on the phi
//! then decides itself, with no `setcc` and `test` for the flag.
//!
//! Then [`crate::floatfold`] runs on the same body (exact floats, and the
//! integers of exact conversions), and [`crate::canonical`] swaps a compare
//! whose constant is on the left and simplifies identities. The pass reports
//! every analysis invalid when anything changed.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @f(i1 %c, i16 %x) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   br label %b3
//!
//! b2:
//!   br label %b3
//!
//! b3:
//!   %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
//!   %y = add i16 %p, %x
//!   %s = add i16 %p, 10
//!   %t = add i16 %s, %y
//!   ret i16 %t
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! b3:
//!   %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
//!   %s1 = phi i16 [ 11, %b1 ], [ 12, %b2 ]
//!   %y = add i16 %p, %x
//!   %t = add i16 %s1, %y
//!   ret i16 %t
//! ```
//!
//! `%s` is a number on both edges, so it becomes the phi `%s1`; `%y` reads
//! the unknown `%x` and stays. Blocks `b0` to `b2` are unchanged.
//!
//! From unit test `test_an_operation_on_a_join_is_folded_on_its_edges` in
//! `fold_tests.rs`.
//!
//! # Runs
//! Second in the per-body pipeline, after `sroa`, and again after the header
//! copy of `rotate` (to clean the test it leaves in the latch). Part of the
//! fixed point; on at -O1 and up, off at -O0. `Options::fold` turns it off
//! for bisection. See `docs/reference/levels.md`.
//!
//! # Needs
//! [`llrm_analysis::consts`], [`llrm_analysis::floatfacts`],
//! `manager::writes` (`Summaries`).
//!
//! # See also
//! LLVM's
//! [`SCCP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp)
//! and `InstSimplify`; GCC's `tree-ssa-ccp.cc`.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::consts::{self, Calls, Known, masked};
use llrm_analysis::floatfacts;
use llrm_analysis::graph::loops;
use llrm_analysis::manager;
use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;

use crate::canonical;
use crate::edges;
use crate::floatfold;
use crate::lcssa::{arms, from_arms, operations};

/// `folded`, then the canonical forms, as the old Fold.
pub struct Fold;

impl FunctionPass for Fold {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "fold"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let calls = manager::writes(unit.context, unit.layout, unit.function, analyses);
        let folded = _folded(unit.context, unit.layout, unit.function, analyses, &calls);
        let swapped = canonical::compares(unit.context, unit.function);
        if folded | swapped | canonical::identities(unit.context, unit.function) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// One pure operation a join feeds, evaluated on each incoming edge.
struct _EdgeFold {
    op: InstId,
    block: BlockId,
    numbers: Vec<(BlockId, BigInt)>,
}

/// Each known value of `function` replaced by its number, one join
/// expression folded on its edges, then floatfold; `outer` is its module
/// and target, `calls` what each call writes. Whether anything changed.
pub fn folded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    calls: &Calls,
) -> bool {
    _folded(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())), calls)
}

/// `folded`, `analyses` holding what is known of `function`.
fn _folded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    calls: &Calls,
) -> bool {
    // Before the facts are asked, so that they see the constants.
    let edges = edges::conditions_decided(context, function);
    if edges {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let numbers = _numbers(context, layout, function, analyses, calls);
    if numbers {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    floatfold::_folded(context, layout, function, analyses, calls) | numbers | edges
}

/// `folded`'s integers: consts' answers, a counted float loop's exit
/// cells among them.
fn _numbers(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    calls: &Calls,
) -> bool {
    let (values, edge) = {
        let held = manager::Held::of(context, layout, function, analyses, true);
        let outer = std::rc::Rc::clone(analyses.outer());
        let unit = held.unit(context, layout, function, &outer);
        let exits = floatfacts::loop_exits(&unit, calls);
        // With no loop to leave the answer is the manager's, where it was of
        // these writes.
        let shared = (exits.is_empty() && *calls == manager::writes(context, layout, function, analyses))
            .then(|| analyses.get_exact::<manager::ThroughMemory>(context, layout, function));
        let facts = match shared.as_deref() {
            Some(Ok(through)) => through.clone(),
            _ => consts::known_walked_over(&unit, calls, &exits),
        };
        (_known_values(&unit, &facts), _folded_phi_edges(&unit, &facts))
    };
    let mut rewritten = BTreeSet::new();
    for (value, number) in &values {
        rewritten.extend(function.users(*value).iter().map(|one| one.user));
        let constant = context.int(function.value(*value).ty, _bits(number));
        function.replace_value(*value, Operand::Constant(constant));
    }
    for &inst in &rewritten {
        let operands = &function.instruction(inst).operands;
        let commutes = matches!(
            function.instruction(inst).opcode,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
        );
        if commutes && matches!(operands[..], [Operand::Constant(_), Operand::Value(_)]) {
            let swapped = vec![operands[1], operands[0]];
            function.set_operands(inst, swapped);
        }
    }
    if edge.is_empty() {
        return !values.is_empty();
    }
    for edge in edge {
        let instruction = function.instruction(edge.op);
        let (ty, result) = (instruction.ty, instruction.result.expect("an integer result"));
        let name = function.value(result).name.clone();
        let incoming = edge
            .numbers
            .iter()
            .map(|(parent, number)| (Operand::Constant(context.int(ty, _bits(number))), *parent))
            .collect::<Vec<_>>();
        let phi = function.create_instruction(Opcode::Phi, ty, from_arms(&incoming), Flags::default(), name.as_deref());
        let first = operations(function, edge.block)[0];
        function.insert(phi, Position::Before(first)).expect("a placed block");
        function.replace_value(result, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
        function.erase(edge.op).expect("its uses were replaced");
    }
    true
}

/// A masked number as the bits `Context::int` takes.
fn _bits(number: &BigInt) -> i128 {
    u128::try_from(number).expect("a masked number") as i128
}

/// Every value something reads whose number consts knows at its full
/// width, or that a division of two known numbers computes.
fn _known_values(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Vec<(ValueId, BigInt)> {
    let function = unit.function;
    let mut out = Vec::new();
    for (_, inst) in function.walk() {
        let Some(result) = consts::_defined(unit, inst).filter(|&result| !function.users(result).is_empty()) else {
            continue;
        };
        let width = unit.int_bits(Operand::Value(result)).expect("an integer");
        if let Some(fact) = facts.get(&result).filter(|fact| fact.width >= width) {
            out.push((result, masked(&fact.n, width)));
        } else if let Some((quotient, remainder)) = consts::division(unit, inst, facts) {
            let divides = matches!(
                function.instruction(inst).opcode,
                Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv)
            );
            out.push((result, if divides { quotient } else { remainder }));
        }
    }
    out
}

/// Every pure operation a join's phis feed, in the join or the blocks only it
/// leads to, whose answer is a number on every incoming edge: all from the one
/// set of facts, since replacing one by a phi of its numbers leaves the others'
/// answers true (LLVM's SCCP and GCC's CCP settle every one in a single
/// propagation).
fn _folded_phi_edges(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
) -> Vec<_EdgeFold> {
    let function = unit.function;
    let mut found = Vec::new();
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let successors = graph.iter().map(|block| (block.at, &block.succ)).collect::<IndexMap<_, _>>();
    let none: &[i64] = &[];
    for &block in function.layout() {
        let at = cfg::id(block);
        let parents = predecessors.get(&at).unwrap_or(none);
        if parents.len() < 2 {
            continue;
        }
        let phis = edges::phis(function, block)
            .into_iter()
            .filter(|&phi| {
                let mut arms: Vec<i64> = arms(function, phi).iter().map(|&(_, from)| cfg::id(from)).collect();
                arms.sort_unstable();
                arms.dedup();
                arms == *parents
            })
            .collect::<Vec<_>>();
        let Some(&first) = phis.first() else {
            continue;
        };
        let order = arms(function, first).into_iter().map(|(_, from)| from).collect::<Vec<_>>();
        let joined =
            phis.iter().map(|&phi| function.instruction(phi).result.expect("a phi's value")).collect::<BTreeSet<_>>();

        let mut corridor = vec![at];
        while let [next] = successors[corridor.last().expect("nonempty")][..] {
            if corridor.contains(&next) || !predecessors.are(&next, &[*corridor.last().expect("nonempty")]) {
                break;
            }
            corridor.push(next);
        }

        let mut edge_facts = facts.clone();
        for &inside in &corridor {
            for inst in operations(function, cfg::block(inside)) {
                let op = function.instruction(inst);
                if !matches!(op.opcode, Opcode::Binary(_) | Opcode::Cast(_)) {
                    continue;
                }
                let Some(result) = consts::_defined(unit, inst).filter(|result| !facts.contains_key(result)) else {
                    continue;
                };
                if !op.operands.iter().any(|one| matches!(one, Operand::Value(value) if joined.contains(value))) {
                    continue;
                }
                let width = unit.int_bits(Operand::Value(result)).expect("an integer");
                let mut numbers = Vec::new();
                for &parent in &order {
                    for &phi in &phis {
                        let value = function.instruction(phi).result.expect("a phi's value");
                        let (incoming, _) =
                            arms(function, phi).into_iter().find(|&(_, from)| from == parent).expect("a complete phi");
                        match consts::_operand(unit, incoming, facts, None) {
                            Some(fact) => edge_facts.insert(value, fact),
                            None => edge_facts.shift_remove(&value),
                        };
                    }
                    let Some(fact) = consts::_result(unit, inst, &edge_facts, None).filter(|fact| fact.width >= width)
                    else {
                        break;
                    };
                    numbers.push((parent, masked(&fact.n, width)));
                }
                if numbers.len() == order.len() {
                    found.push(_EdgeFold { op: inst, block, numbers });
                }
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
