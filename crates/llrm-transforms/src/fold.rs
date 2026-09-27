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
//! - Fold runs over the module: consts solves memory with its globals, which
//!   a function pass's `Unit` does not carry.
//!
//! Dropped, no rich MIR analogue: `_constant_update` (a read-modify-write of
//! a cell); `_symbol_copies` and the symbol half of `_literal_of` (a
//! global's address is already a constant operand); the checks for a
//! second result something reads (`wanted`: one result per instruction).
//! Waiting for their ports: the float folds (`floatfacts`, `floatfold`) and
//! the exit cells they supplied.
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
use llrm_analysis::memory::Unit;
use llrm_graph::loops;
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Module, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::ModulePass;
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::canonical;
use crate::edges;
use crate::interprocedural::function_mut;
use crate::lcssa::{arms, from_arms, operations};
use crate::transform::{bodies, layout};

/// `folded` on every body, then the canonical forms, as the old Fold.
pub struct Fold;

impl ModulePass for Fold {
    fn name(&self) -> &'static str {
        "fold"
    }

    fn run(&mut self, module: &mut Module) -> Vec<GlobalId> {
        let layout = layout(module).unwrap_or_else(|error| panic!("fold: {error}"));
        bodies(module)
            .into_iter()
            .filter(|&id| {
                let folded = folded(module, &layout, id);
                let (context, function) = function_mut(module, id);
                let swapped = canonical::compares(context, function);
                folded | swapped | canonical::identities(context, function)
            })
            .collect()
    }
}

/// One pure operation a join feeds, evaluated on each incoming edge.
struct _EdgeFold {
    op: InstId,
    block: BlockId,
    numbers: Vec<(BlockId, BigInt)>,
}

/// Each known value of body `id` replaced by its number, and one join
/// expression folded on its edges. Whether anything changed.
pub fn folded(module: &mut Module, layout: &DataLayout, id: GlobalId) -> bool {
    let (values, edge) = {
        let function = module.global(id).function().expect("a body");
        let unit = Unit::of(module, layout, function);
        let facts = consts::known(&unit, Some(&Calls::default()), None, None);
        (_known_values(&unit, &facts), _folded_phi_edges(&unit, &facts))
    };
    let (context, function) = function_mut(module, id);
    let mut rewritten = BTreeSet::new();
    for (value, number) in &values {
        rewritten.extend(function.users(*value).iter().map(|one| one.user));
        let constant = context.int(function.value(*value).ty, _bits(number));
        function.replace_all_uses_with(*value, Operand::Constant(constant));
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
    function.replace_all_uses_with(result, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
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
