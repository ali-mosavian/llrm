//! Branches whose way is known, decided: LLVM's JumpThreading and
//! SimplifyCFG on known conditions. Adapted from llrm-core's
//! `optimize/transform.rs` `Decide` (`decided`, `_threaded`,
//! `_executable_successors`, `_outcome`, `_switch_target`, `_signed`,
//! `_TAKEN`). What is known is consts' answer, carried across loops by
//! constant_cycles along the edges that run.
//!
//! What changed with the IR:
//! - A branch reads an `i1`: the flags a compare left are the `icmp`
//!   `_comparison` finds, and a condition consts knows outright decides the
//!   branch too.
//! - A branch not taken became an inert owner falling through; here it is
//!   a jump to the other arm.
//! - Threading bypasses a block holding only a jump, for a `br` or a
//!   `switch`; explicit and fall-through edges are one kind here. A
//!   conditional branch both of whose arms go one way is a jump, as the
//!   old one was when threading made it so.
//! - Decide runs over the module, as Fold does.
//!
//! Dropped, no rich MIR analogue: the memory a compare's operand read
//! (`held`: a load is its own instruction, and its fact consts'), and the
//! case width checks (a switch's cases are its condition's type). Waiting
//! for their ports: a pointer compared with null (`alias`'s nonnull), and
//! `ranges::bounded` (induction's); until then a two-way branch one arm of
//! which the dominating edges rule out asks `ranges::dominated_edges`.
//!
//! Tests, in `decide_tests.rs`: `test_empty_jump_threading_preserves_phi_inputs_and_effects`
//! is ported. The other old tests of `_outcome`, `_switch_target` and
//! `_executable_successors` read BC objects through the raise
//! (`raising_dispatch_tests`) or belong to `peelsize`, not yet ported.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::constant_cycles::{self, State};
use llrm_analysis::consts::{self, Calls, Known, masked};
use llrm_analysis::memory::Unit;
use llrm_analysis::ranges;
use llrm_graph::loops;
use llrm_mir::context::{Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueId};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::ModulePass;
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::edges;
use crate::interprocedural::function_mut;
use crate::transform::{self, bodies, layout};

/// `decided` on every body, then its chains merged, as the old Decide.
pub struct Decide;

impl ModulePass for Decide {
    fn name(&self) -> &'static str {
        "decide"
    }

    fn run(&mut self, module: &mut Module) -> Vec<GlobalId> {
        let layout = layout(module).unwrap_or_else(|error| panic!("decide: {error}"));
        bodies(module)
            .into_iter()
            .filter(|&id| {
                let decided = decided(module, &layout, id).unwrap_or_else(|error| panic!("decide: {error}"));
                decided | crate::cfg::merged(function_mut(module, id).1)
            })
            .collect()
    }
}

/// Body `id` threaded, and each branch whose way is known a jump that
/// way. Whether anything changed.
pub fn decided(module: &mut Module, layout: &DataLayout, id: GlobalId) -> Result<bool, String> {
    let threaded = {
        let (context, function) = function_mut(module, id);
        _threaded(context, function)
    };
    let decisions = {
        let function = module.global(id).function().expect("a body");
        _decisions(&Unit::of(module, layout, function))?
    };
    if decisions.is_empty() {
        return Ok(threaded);
    }
    let (context, function) = function_mut(module, id);
    for (block, target) in decisions {
        let last = function.terminator(block).expect("a terminator");
        _jump(function, last, target);
    }
    cfg::_unreachable(context, function);
    transform::_trivial_phis(function)?;
    Ok(true)
}

/// Each block whose terminator's way is known, with that way.
fn _decisions(unit: &Unit) -> Result<Vec<(BlockId, BlockId)>, String> {
    let function = unit.function;
    let facts = consts::known(unit, Some(&Calls::default()), None, None);
    let successors = |at: i64, values: &IndexMap<ValueId, Known>, states: &IndexMap<ValueId, State>| _executable_successors(unit, at, values, states);
    let facts = constant_cycles::propagated(unit, &facts, Some(&successors));
    let scoped = ranges::dominated_edges(unit)?;
    let mut out = Vec::new();
    for &block in function.layout() {
        let Some(last) = function.terminator(block) else {
            continue;
        };
        let instruction = function.instruction(last);
        if instruction.opcode == Opcode::Switch {
            out.extend(_switch_target(unit, last, &facts).map(|target| (block, target)));
            continue;
        }
        let [_, Operand::Block(taken), Operand::Block(other)] = instruction.operands[..] else {
            continue;
        };
        let mut answer = _outcome(unit, block, &facts);
        if answer.is_none()
            && let Some(scope) = scoped.get(&cfg::id(block))
        {
            let mut possible = Vec::new();
            for target in function.successors(block) {
                if ranges::on_edge(unit, block, target, scope, Some(&facts))?.is_some() {
                    possible.push(target);
                }
            }
            if let [one] = possible[..] {
                answer = Some(one == taken);
            }
        }
        out.extend(answer.map(|answer| (block, if answer { taken } else { other })));
    }
    Ok(out)
}

/// `last`, a terminator, replaced by a jump to `target`.
fn _jump(function: &mut Function, last: InstId, target: BlockId) {
    let jump = function.create_instruction(Opcode::Br, function.instruction(last).ty, vec![Operand::Block(target)], Flags::default(), None);
    function.insert(jump, Position::Before(last)).expect("a placed terminator");
    function.erase(last).expect("a terminator defines nothing");
}

/// A known number as the machine compares it signed.
pub fn _signed(fact: &Known) -> BigInt {
    let top = BigInt::from(1) << (fact.width - 1);
    let number = masked(&fact.n, fact.width);
    if (&number & &top) != BigInt::from(0) { number - (top << 1) } else { number }
}

/// Whether `predicate` holds of two known numbers.
pub fn _taken(predicate: IntPredicate, left: &Known, right: &Known) -> bool {
    let width = left.width.max(right.width);
    let (a, b) = (_signed(left), _signed(right));
    let (ua, ub) = (masked(&left.n, width), masked(&right.n, width));
    match predicate {
        IntPredicate::Eq => a == b,
        IntPredicate::Ne => a != b,
        IntPredicate::Slt => a < b,
        IntPredicate::Sle => a <= b,
        IntPredicate::Sgt => a > b,
        IntPredicate::Sge => a >= b,
        IntPredicate::Ult => ua < ub,
        IntPredicate::Ule => ua <= ub,
        IntPredicate::Ugt => ua > ub,
        IntPredicate::Uge => ua >= ub,
    }
}

/// Whether `block`'s conditional branch is taken: its condition is known,
/// or the compare that makes it has two known operands.
pub fn _outcome(unit: &Unit, block: BlockId, facts: &IndexMap<ValueId, Known>) -> Option<bool> {
    let branch = unit.function.terminator(block)?;
    let instruction = unit.function.instruction(branch);
    if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
        return None;
    }
    if let Some(condition) = consts::_operand(unit, instruction.operands[0], facts, None) {
        return Some(masked(&condition.n, 1) != BigInt::from(0));
    }
    let compare = unit.function.instruction(transform::_comparison(unit.function, block, branch)?);
    let Opcode::ICmp(predicate) = compare.opcode else {
        unreachable!("_comparison finds an icmp")
    };
    let left = consts::_operand(unit, compare.operands[0], facts, None)?;
    let right = consts::_operand(unit, compare.operands[1], facts, None)?;
    Some(_taken(predicate, &left, &right))
}

/// Where the switch `inst` goes, where its condition is known and its
/// cases distinct.
pub fn _switch_target(unit: &Unit, inst: InstId, facts: &IndexMap<ValueId, Known>) -> Option<BlockId> {
    let op = unit.function.instruction(inst);
    if op.opcode != Opcode::Switch {
        return None;
    }
    let value = consts::_operand(unit, op.operands[0], facts, None)?;
    let cases = op.operands[2..]
        .chunks(2)
        .map(|pair| match pair {
            [case, Operand::Block(target)] => Some((consts::_operand(unit, *case, facts, None)?.n, *target)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if cases.iter().map(|(number, _)| number).collect::<BTreeSet<_>>().len() != cases.len() {
        return None;
    }
    let Operand::Block(default) = op.operands[1] else {
        return None;
    };
    Some(cases.into_iter().find(|(number, _)| *number == value.n).map_or(default, |(_, target)| target))
}

/// The successors of block `at` that can run, given what is known so far;
/// `None` while its condition is still pending.
pub fn _executable_successors(unit: &Unit, at: i64, facts: &IndexMap<ValueId, Known>, states: &IndexMap<ValueId, State>) -> Option<Vec<i64>> {
    let function = unit.function;
    let block = cfg::block(at);
    let all = function.successors(block).into_iter().map(cfg::id).collect::<Vec<_>>();
    let pending = |operands: &[Operand]| operands.iter().any(|operand| matches!(operand, Operand::Value(value) if states.get(value) == Some(&State::Pending)));
    let Some(last) = function.terminator(block) else {
        return Some(all);
    };
    let instruction = function.instruction(last);
    if instruction.opcode == Opcode::Switch {
        if let Some(target) = _switch_target(unit, last, facts) {
            return Some(vec![cfg::id(target)]);
        }
        return if pending(&instruction.operands[..1]) { None } else { Some(all) };
    }
    let [condition, Operand::Block(taken), Operand::Block(other)] = instruction.operands[..] else {
        return Some(all);
    };
    if let Some(answer) = _outcome(unit, block, facts) {
        return Some(vec![cfg::id(if answer { taken } else { other })]);
    }
    let compared = transform::_comparison(function, block, last).map(|compare| function.instruction(compare).operands.clone()).unwrap_or_default();
    if pending(&[condition]) || pending(&compared) {
        return None;
    }
    Some(all)
}

/// Blocks holding only a jump bypassed, where what they jump to has no
/// phis to tell the edges apart. Whether anything changed.
///
/// Loop-simplify form keeps a loop's one entry edge, its one back edge and
/// its dedicated exits as blocks of their own.
pub fn _threaded(context: &mut Context, function: &mut Function) -> bool {
    let Some(entry) = function.entry() else {
        return false;
    };
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    let known = graph.iter().map(|block| (block.at, &block.succ)).collect::<BTreeMap<_, _>>();
    let none = BTreeSet::new();
    let mut loop_edges = BTreeSet::new();
    for loop_ in loops::loops(&graph, Some(cfg::id(entry))) {
        let outside = predecessors.get(&loop_.header).unwrap_or(&none).difference(&loop_.body).copied().collect::<Vec<_>>();
        if let [parent] = outside[..]
            && *known[&parent] == [loop_.header]
        {
            loop_edges.insert(parent);
        }
        if let Some(&latch) = loop_.latches.iter().next().filter(|_| loop_.latches.len() == 1)
            && *known[&latch] == [loop_.header]
        {
            loop_edges.insert(latch);
        }
        loop_edges.extend(
            graph
                .iter()
                .filter(|block| {
                    let parents = predecessors.get(&block.at).unwrap_or(&none);
                    !loop_.body.contains(&block.at) && !parents.is_empty() && parents.is_subset(&loop_.body) && block.succ.len() == 1 && !loop_.body.contains(&block.succ[0])
                })
                .map(|block| block.at),
        );
    }
    let mut redirects = BTreeMap::new();
    for &block in function.layout() {
        let at = cfg::id(block);
        let [only] = function.block(block).instructions()[..] else {
            continue;
        };
        let instruction = function.instruction(only);
        if let (Opcode::Br, [Operand::Block(next)]) = (&instruction.opcode, &instruction.operands[..])
            && !loop_edges.contains(&at)
            && edges::phis(function, *next).is_empty()
        {
            redirects.insert(at, cfg::id(*next));
        }
    }
    let destination = |start: i64, source: i64| {
        let (mut target, mut seen) = (start, BTreeSet::from([source]));
        while let Some(&next) = redirects.get(&target) {
            if !seen.insert(target) {
                break;
            }
            target = next;
        }
        if seen.contains(&target) { start } else { target }
    };

    let mut changed = false;
    for block in function.layout().to_vec() {
        let Some(last) = function.terminator(block) else {
            continue;
        };
        if !matches!(function.instruction(last).opcode, Opcode::Br | Opcode::Switch) {
            continue;
        }
        for (index, operand) in function.instruction(last).operands.clone().into_iter().enumerate() {
            if let Operand::Block(target) = operand {
                let to = destination(cfg::id(target), cfg::id(block));
                if to != cfg::id(target) {
                    function.set_operand(last, index, Operand::Block(cfg::block(to)));
                    changed = true;
                }
            }
        }
        // Both arms one way: the condition chooses nothing.
        if let [_, Operand::Block(taken), Operand::Block(other)] = function.instruction(last).operands[..]
            && taken == other
        {
            _jump(function, last, taken);
            changed = true;
        }
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

#[cfg(test)]
#[path = "decide_tests.rs"]
mod tests;
