//! Values known to be numbers, replaced by those numbers: LLVM's SCCP and
//! InstSimplify. Adapted from llrm-core's `optimize/transform.rs` `Fold`
//! (`folded`, `_folded_op`, `_folded_division`, `_folded_phi_edges`,
//! `_constant_operands`). What is known is consts' answer.
//!
//! What changed with the IR:
//! - An operation whose answer was known became a move of that number, and
//!   each reader's register operand became the number. Here every use of
//!   the value is the constant, and Dead takes the definition. So a store's
//!   value, a call's argument (`_constant_argument`), a fill's byte and count
//!   (`_constant_fill`) and an address's index (`_constant_based`) need no
//!   case of their own, and a load from a known cell is a value consts knows.
//! - Operand order is no machine's: an ordered operand is replaced like any
//!   other. A commutative operation still takes its constant on the right.
//! - An edge fold put a copy of each number in its parent; a phi here takes
//!   the constant, so the parent's terminator no longer matters.
//! - consts reads the module's globals through the outer proxy, and what
//!   each call writes from the manager's `Writes`.
//! - The old Fold left floatfold out of a body with loops, whose x87
//!   observation points belonged to floatloop; the rich MIR observes no FP
//!   exception, so it folds every body.
//!
//! Dropped, no rich MIR analogue: `_constant_update` (a read-modify-write of
//! a cell); `_symbol_copies` and the symbol half of `_literal_of` (a
//! global's address is already a constant operand); the checks for a
//! second result something reads (`wanted`: one result per instruction).
//!
//! Tests, in `fold_tests.rs`: `divisor_constants_propagate_without_reordering`,
//! and consts' tests that waited for this port:
//! `test_signed_widening_produces_a_whole_long_constant`'s folding half,
//! `test_pointer_displacement_constants_preserve_order_and_width`,
//! `test_constant_subtraction_preserves_operand_order` and
//! `test_a_known_factor_becomes_a_multiply_operand`. Skipped:
//! `test_constant_operand_keeps_its_memory_address_dependency` (every use is
//! replaced at once, so no reader keeps an orphaned value), and the width
//! halves (a fact here is as wide as its value).

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::consts::{self, Calls, Known, masked};
use llrm_analysis::floatfacts;
use llrm_analysis::manager;
use llrm_analysis::memory::Unit;
use llrm_analysis::graph::loops;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::canonical;
use crate::edges;
use crate::floatfold;
use crate::lcssa::{arms, from_arms, operations};

/// `folded`, then the canonical forms, as the old Fold.
pub struct Fold;

impl FunctionPass for Fold {
    fn name(&self) -> &'static str {
        "fold"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let calls = manager::writes(unit.context, unit.layout, unit.function, analyses);
        let folded = _folded(unit.context, unit.layout, unit.function, analyses, &calls);
        let swapped = canonical::compares(unit.context, unit.function);
        if folded | swapped | canonical::identities(unit.context, unit.function) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
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
pub fn folded(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer, calls: &Calls) -> bool {
    _folded(context, layout, function, &mut Analyses::new(std::rc::Rc::new(outer.clone())), calls)
}

/// `folded`, `analyses` holding what is known of `function`.
fn _folded(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &mut Analyses, calls: &Calls) -> bool {
    let numbers = _numbers(context, layout, function, analyses, calls);
    if numbers {
        analyses.invalidate(&PreservedAnalyses::none());
    }
    floatfold::_folded(context, layout, function, analyses, calls) | numbers
}

/// `folded`'s integers: consts' answers, a counted float loop's exit
/// cells among them.
fn _numbers(context: &mut Context, layout: &DataLayout, function: &mut Function, analyses: &mut Analyses, calls: &Calls) -> bool {
    let (values, edge) = {
        let held = manager::Held::of(context, layout, function, analyses, true);
        let outer = std::rc::Rc::clone(analyses.outer());
        let unit = held.unit(context, layout, function, &outer);
        let edges = floatfacts::exit_cells(&unit, calls);
        // With no edges the answer is the manager's, where it was of these writes.
        let shared = (edges.is_empty() && *calls == manager::writes(context, layout, function, analyses)).then(|| analyses.get::<manager::ThroughMemory>(context, layout, function));
        let facts = match shared.as_deref() {
            Some(Ok(through)) => {
                if std::env::var_os("LLRM_CHECK_FACTS").is_some() {
                    assert!(*through == consts::known(&unit, Some(calls), Some(&edges), None), "ThroughMemory's integers are not those fold derives for itself");
                }
                through.clone()
            }
            _ => consts::known(&unit, Some(calls), Some(&edges), None),
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
        let commutes = matches!(function.instruction(inst).opcode, Opcode::Binary(BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor));
        if commutes && matches!(operands[..], [Operand::Constant(_), Operand::Value(_)]) {
            let swapped = vec![operands[1], operands[0]];
            function.set_operands(inst, swapped);
        }
    }
    let Some(edge) = edge else {
        return !values.is_empty();
    };
    let instruction = function.instruction(edge.op);
    let (ty, result) = (instruction.ty, instruction.result.expect("an integer result"));
    let name = function.value(result).name.clone();
    let incoming = edge.numbers.iter().map(|(parent, number)| (Operand::Constant(context.int(ty, _bits(number))), *parent)).collect::<Vec<_>>();
    let phi = function.create_instruction(Opcode::Phi, ty, from_arms(&incoming), Flags::default(), name.as_deref());
    let first = operations(function, edge.block)[0];
    function.insert(phi, Position::Before(first)).expect("a placed block");
    function.replace_value(result, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
    function.erase(edge.op).expect("its uses were replaced");
    true
}

/// A masked number as the bits `Context::int` takes.
fn _bits(number: &BigInt) -> i128 {
    u128::try_from(number).expect("a masked number") as i128
}

/// Every value something reads whose number consts knows at its full
/// width, or that a division of two known numbers computes.
fn _known_values(unit: &Unit, facts: &IndexMap<ValueId, Known>) -> Vec<(ValueId, BigInt)> {
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
            let divides = matches!(function.instruction(inst).opcode, Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv));
            out.push((result, if divides { quotient } else { remainder }));
        }
    }
    out
}

/// The first pure operation a join's phis feed, in the join or the blocks
/// only it leads to, whose answer is a number on every incoming edge.
fn _folded_phi_edges(unit: &Unit, facts: &IndexMap<ValueId, Known>) -> Option<_EdgeFold> {
    let function = unit.function;
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let successors = graph.iter().map(|block| (block.at, &block.succ)).collect::<IndexMap<_, _>>();
    let none = BTreeSet::new();
    for &block in function.layout() {
        let at = cfg::id(block);
        let parents = predecessors.get(&at).unwrap_or(&none);
        if parents.len() < 2 {
            continue;
        }
        let phis = edges::phis(function, block)
            .into_iter()
            .filter(|&phi| arms(function, phi).iter().map(|&(_, from)| cfg::id(from)).collect::<BTreeSet<_>>() == *parents)
            .collect::<Vec<_>>();
        let Some(&first) = phis.first() else {
            continue;
        };
        let order = arms(function, first).into_iter().map(|(_, from)| from).collect::<Vec<_>>();
        let joined = phis.iter().map(|&phi| function.instruction(phi).result.expect("a phi's value")).collect::<BTreeSet<_>>();

        let mut corridor = vec![at];
        while let [next] = successors[corridor.last().expect("nonempty")][..] {
            if corridor.contains(&next) || predecessors.get(&next) != Some(&BTreeSet::from([*corridor.last().expect("nonempty")])) {
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
                        let (incoming, _) = arms(function, phi).into_iter().find(|&(_, from)| from == parent).expect("a complete phi");
                        match consts::_operand(unit, incoming, facts, None) {
                            Some(fact) => edge_facts.insert(value, fact),
                            None => edge_facts.shift_remove(&value),
                        };
                    }
                    let Some(fact) = consts::_result(unit, inst, &edge_facts, None).filter(|fact| fact.width >= width) else {
                        break;
                    };
                    numbers.push((parent, masked(&fact.n, width)));
                }
                if numbers.len() == order.len() {
                    return Some(_EdgeFold { op: inst, block, numbers });
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
