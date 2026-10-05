//! Affine exit values evaluated, and finite counted loops with no effect
//! deleted: LLVM's IndVarSimplify exit-value rewriting and LoopDeletion.
//! Adapted from llrm-core's `optimize/loopexit.rs`, the port of
//! `qbopt/optimize/loopexit.py`.
//!
//! The integer equivalent of evaluating an AddRec at its backedge count: a
//! fixed increment sums to N * step; an affine increment also contributes
//! N(N-1)/2 times its stride. Results are modulo their own width. Only the
//! controlling recurrence must be proven not to wrap.
//!
//! What changed with the IR:
//! - A deleted loop's header computes its phis' exit values, placed by
//!   `counting::Seeds`, and branches to the exit; the latch goes as
//!   unreachable. The old header kept its cleared operations as byte owners.
//! - A constant exit value is an operand where the old one was a copy
//!   placed in the exit block, and an exit block always has a terminator.
//! - The preheader and exit are the counted proof's.
//! - `_disposable`'s kinds are `add`, `sub`, `icmp` and `br`: `Copy` and
//!   `Nothing` have no instruction, `Increment` and `Decrement` are `add`s.
//! - `_widened_counters` reads a `sext`; the old `SignExtend` it was.
//!
//! llrm-mir's `loopdeletion` deletes a loop of any shape whose exit values
//! come from outside it; it evaluates none, so a loop computing what is
//! read after it stays there.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::consts::{Known, masked};
use llrm_analysis::induction::{self, Affine, AffineOperand};
use llrm_analysis::memory::Unit;
use llrm_analysis::{cfg, occurrence, ranges};
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use crate::counting::{self, Seeds};
use crate::edges;
use crate::lcssa::{arms, operations};
use crate::transform::_trivial_phis;

pub struct LoopExit;

impl FunctionPass for LoopExit {
    fn name(&self) -> &'static str {
        "loopexit"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        match evaluated(unit.context, unit.layout, unit.function, analyses.outer()) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("loopexit: {error}"),
        }
    }
}

/// Each exit value's terms: a value or number, times its coefficient.
type Terms = Vec<(AffineOperand, BigInt)>;

/// What one loop comes to.
enum Evaluation {
    /// Every header phi's exit value: the loop goes.
    Deleted { header: BlockId, exit: BlockId, exits: IndexMap<ValueId, Terms> },
    /// Constant exit values read after the loop, which stays.
    Constant { exit: BlockId, following: BTreeSet<BlockId>, swap: BTreeMap<ValueId, BigInt> },
}

/// Loops evaluated, one at a time to a fixed point; whether any changed.
pub fn evaluated(context: &mut Context, layout: &DataLayout, function: &mut Function, outer: &Outer) -> Result<bool, String> {
    let mut changed = false;
    loop {
        let found = _evaluation(context, layout, function, outer)?;
        match found {
            None => return Ok(changed),
            Some(Evaluation::Deleted { header, exit, exits }) => _deleted(context, function, header, exit, &exits)?,
            Some(Evaluation::Constant { exit, following, swap }) => _substituted_exits(context, function, exit, &following, &swap)?,
        }
        changed = true;
    }
}

/// The first loop whose exit values change something: the old `evaluated`.
fn _evaluation(context: &Context, layout: &DataLayout, function: &Function, outer: &Outer) -> Result<Option<Evaluation>, String> {
    let unit = Unit::within(context, layout, function, outer);
    let facts = unit.registers();
    for loop_ in cfg::Shape::of(function).loops {
        let Some(&latch) = loop_.latches.first() else { continue };
        if loop_.body.len() != 2 || loop_.latches.len() != 1 || !edges::phis(function, cfg::block(latch)).is_empty() {
            continue;
        }
        let counters = induction::basics(&unit, &loop_);
        if counters.is_empty() {
            continue;
        }
        let proofs = induction::counted(&unit, &loop_, Some(&facts), false);
        // The exit terms below are the header's values as it leaves.
        let Some(count) = induction::agreed_count(&proofs).filter(|_| !proofs.iter().any(|proof| proof.posttested)) else { continue };
        let Some(proof) = proofs.iter().find(|proof| proof.preheader.is_some()) else { continue };
        let exits = _exit_terms(&unit, &loop_, &counters, &count, &facts)?;
        if exits.is_empty() {
            continue;
        }
        let (header, exit) = (cfg::block(loop_.header), cfg::block(proof.exit));
        let phied = edges::phis(function, header).iter().filter_map(|&phi| function.instruction(phi).result).collect::<BTreeSet<_>>();
        if exits.keys().copied().collect::<BTreeSet<_>>() != phied || !_disposable(&unit, &loop_) {
            if let Some(found) = _constant_exits(function, &loop_, &exits, exit, &facts)? {
                return Ok(Some(found));
            }
            continue;
        }
        return Ok(Some(Evaluation::Deleted { header, exit, exits }));
    }
    Ok(None)
}

/// The loop headed by `header` replaced by its phis' exit values, computed
/// in the header, which then branches to `exit`.
fn _deleted(context: &mut Context, function: &mut Function, header: BlockId, exit: BlockId, exits: &IndexMap<ValueId, Terms>) -> Result<(), String> {
    let phis = edges::phis(function, header);
    let work = operations(function, header);
    let branch = *work.last().expect("a terminator");
    let mut seeds = Seeds { context, function, at: work[0], width: 0 };
    let mut totals = Vec::new();
    for &phi in &phis {
        let result = seeds.function.instruction(phi).result.expect("a phi's value");
        let terms = &exits[&result];
        seeds.width = terms[0].0.width();
        let width = seeds.width;
        let mut total = None;
        for (arg, coefficient) in terms {
            let product = match arg {
                AffineOperand::Const(known) => AffineOperand::constant(&known.n * coefficient, width),
                _ if *coefficient == BigInt::from(1) => arg.clone(),
                _ => seeds.computed(BinaryOp::Mul, vec![arg.clone(), AffineOperand::constant(coefficient.clone(), width)]),
            };
            total = Some(match total {
                None => product,
                Some(sum) => seeds.computed(BinaryOp::Add, vec![sum, product]),
            });
        }
        let total = total.expect("a start term");
        totals.push((result, seeds.operand(&total)));
    }
    for ((result, total), &phi) in totals.into_iter().zip(&phis) {
        function.replace_all_uses_with(result, total);
        function.erase(phi)?;
    }
    let void = function.instruction(branch).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(exit)], Flags::default(), None);
    function.insert(jump, Position::Before(branch))?;
    function.set_operands(branch, Vec::new());
    function.erase(branch)?;
    cfg::_unreachable(context, function);
    for &inst in work[..work.len() - 1].iter().rev() {
        function.erase(inst)?;
    }
    _trivial_phis(function)
}

/// Sum a linear increment over N iterations using N(N-1)/2, before modular
/// reduction.
fn _exit_terms(unit: &Unit, loop_: &Loop, counters: &IndexMap<ValueId, Affine>, count: &BigInt, facts: &IndexMap<ValueId, Known>) -> Result<IndexMap<ValueId, Terms>, String> {
    let function = unit.function;
    let phis = edges::phis(function, cfg::block(loop_.header));
    let still = induction::invariant(function, &loop_.body);
    let mut headers = phis.iter().filter_map(|&phi| function.instruction(phi).result).collect::<BTreeSet<_>>();
    // An invariant is a term, whatever computes it.
    for &at in &loop_.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            headers.extend(function.instruction(inst).operands.iter().filter_map(|one| match one {
                Operand::Value(value) if still.contains(*value) => Some(*value),
                _ => None,
            }));
        }
    }
    let widened = _widened_counters(unit, loop_, counters, facts)?;
    headers.extend(widened.keys().copied());
    let mut counters = counters.clone();
    counters.extend(widened);
    // Each value affine in a counter is `start + step * trip`, summed over the trips.
    let recurrences = induction::recurrences(unit, loop_, &counters)
        .values
        .into_iter()
        .filter(|(_, one)| one.pointer.is_none())
        .collect::<BTreeMap<_, _>>();
    headers.extend(recurrences.keys().copied());
    let within = |value: ValueId| unit.defining(Operand::Value(value)).and_then(|(inst, _)| function.parent(inst)).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    let mut exits = IndexMap::default();
    for phi in phis {
        let result = function.instruction(phi).result.expect("a phi's value");
        if let Some(counter) = counters.get(&result) {
            exits.insert(result, vec![(counter.start.clone(), BigInt::from(1)), (counter.step.clone(), count.clone())]);
            continue;
        }
        let (inside, outside): (Vec<_>, Vec<_>) = arms(function, phi).into_iter().partition(|(_, block)| loop_.body.contains(&cfg::id(*block)));
        let [(start, _)] = outside[..] else { return Err(format!("{} entries into a counted loop's header", outside.len())) };
        let [(Operand::Value(update), _)] = inside[..] else { continue };
        if !within(update) {
            continue;
        }
        let Some(width) = unit.int_bits(Operand::Value(update)) else { continue };
        let linear = induction::linear(unit, &AffineOperand::Value(update, width), &headers, width, false, &BTreeSet::new(), &mut IndexMap::default());
        let Some(mut linear) = linear else { continue };
        if linear.shift_remove(&AffineOperand::Value(result, width)) != Some(BigInt::from(1)) {
            continue;
        }
        let Some(start) = induction::term(unit, start) else { continue };
        let mut terms = vec![(start, BigInt::from(1))];
        let mut complete = true;
        for (arg, coefficient) in &linear {
            match arg {
                AffineOperand::Const(_) => terms.push((arg.clone(), coefficient * count)),
                AffineOperand::Value(value, _) if still.contains(*value) => terms.push((arg.clone(), coefficient * count)),
                AffineOperand::Value(value, _) if counters.get(value).is_some_and(|counter| counter.start.width() == width) => {
                    let counter = &counters[value];
                    terms.push((counter.start.clone(), coefficient * count));
                    terms.push((counter.step.clone(), induction::floor_div(&(coefficient * count * (count - 1)), &BigInt::from(2))));
                }
                AffineOperand::Value(value, _) if recurrences.get(value).is_some_and(|of| of.width() == width) => {
                    let of = &recurrences[value];
                    let sums = [(&of.start, coefficient * count), (&of.step, induction::floor_div(&(coefficient * count * (count - 1)), &BigInt::from(2)))];
                    for (sum, times) in sums {
                        terms.push((AffineOperand::constant(sum.constant.clone(), width), times.clone()));
                        for (product, factor) in &sum.terms {
                            // A term is one value: a product of unknowns is not an operand.
                            let Some(term) = product.single() else {
                                complete = false;
                                break;
                            };
                            terms.push((AffineOperand::Value(term, width), factor * &times));
                        }
                    }
                }
                AffineOperand::Value(..) => {
                    complete = false;
                    break;
                }
            }
        }
        if complete {
            exits.insert(result, terms);
        }
    }
    Ok(exits)
}

/// Each counter a `sext` widens where ranges bound it at its own width,
/// as the wide recurrence it is.
fn _widened_counters(unit: &Unit, loop_: &Loop, counters: &IndexMap<ValueId, Affine>, facts: &IndexMap<ValueId, Known>) -> Result<IndexMap<ValueId, Affine>, String> {
    let function = unit.function;
    let extensions = loop_
        .body
        .iter()
        .flat_map(|&at| function.block(cfg::block(at)).instructions().iter().map(move |&inst| (at, inst)))
        .filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Cast(CastOp::SExt))
        .collect::<Vec<_>>();
    if extensions.is_empty() {
        return Ok(IndexMap::default());
    }
    let bounds = ranges::bounded(unit)?;
    let mut widened = IndexMap::default();
    for (at, inst) in extensions {
        let op = function.instruction(inst);
        let (Operand::Value(source), Some(result)) = (op.operands[0], op.result) else { continue };
        let (Some(narrow), Some(wide)) = (unit.int_bits(Operand::Value(source)), unit.int_bits(Operand::Value(result))) else { continue };
        if narrow >= wide {
            continue;
        }
        let (Some(counter), Some(interval)) = (counters.get(&source), bounds.get(&at).and_then(|known| known.get(&source))) else { continue };
        if interval.width != narrow {
            continue;
        }
        let start = induction::_signed(&counter.start, facts, narrow);
        let step = induction::_signed(&counter.step, facts, narrow);
        if let (Some(start), Some(step)) = (start, step) {
            widened.insert(result, Affine { value: result, start: AffineOperand::constant(start, wide), step: AffineOperand::constant(step, wide), header: loop_.header });
        }
    }
    Ok(widened)
}

/// Constant live-outs replaced after the loop, leaving its observable
/// work intact. What the loop still computes for itself is `lsr`'s to keep.
fn _constant_exits(
    function: &Function,
    loop_: &Loop,
    exits: &IndexMap<ValueId, Terms>,
    exit: BlockId,
    facts: &IndexMap<ValueId, Known>,
) -> Result<Option<Evaluation>, String> {
    if function.predecessors(exit) != [cfg::block(loop_.header)] {
        return Ok(None);
    }
    let graph = cfg::graph(function);
    let dominance = cfg::Dominance::of(function);
    let following = graph
        .iter()
        .filter(|block| dominance.dominates(cfg::id(exit), block.at))
        .map(|block| cfg::block(block.at))
        .collect::<BTreeSet<_>>();
    let aliases = _aliases(function, exit, loop_);
    let mut used = BTreeSet::new();
    let mut unavailable = BTreeSet::<ValueId>::new();
    for (_, block, op) in occurrence::operations(function).chain(occurrence::phis(function)) {
        for (index, operand) in op.operands.iter().enumerate() {
            let Operand::Value(value) = *operand else { continue };
            let from = if op.opcode == Opcode::Phi {
                let Operand::Block(from) = op.operands[index + 1] else { unreachable!("a phi pairs values with blocks") };
                from
            } else {
                block
            };
            if following.contains(&from) {
                used.insert(value);
                used.extend(aliases.get(&value));
            } else {
                unavailable.extend(aliases.get(&value));
            }
        }
    }
    let mut swap = BTreeMap::new();
    for (value, terms) in exits {
        if !used.contains(value) || unavailable.contains(value) {
            continue;
        }
        let width = terms[0].0.width();
        let mut total = BigInt::from(0);
        let mut complete = true;
        for (arg, coefficient) in terms {
            let fact = match arg {
                AffineOperand::Value(one, _) => facts.get(one).cloned(),
                AffineOperand::Const(constant) => Some(constant.clone()),
            };
            let Some(fact) = fact.filter(|fact| fact.width >= arg.width()) else {
                complete = false;
                break;
            };
            total += masked(&fact.n, arg.width()) * coefficient;
        }
        if complete {
            swap.insert(*value, masked(&total, width));
        }
    }
    if swap.is_empty() {
        return Ok(None);
    }
    Ok(Some(Evaluation::Constant { exit, following, swap }))
}

/// The exit's phis that take one value from the header alone: each phi's
/// value, and the value it names after the loop.
fn _aliases(function: &Function, exit: BlockId, loop_: &Loop) -> BTreeMap<ValueId, ValueId> {
    edges::phis(function, exit)
        .into_iter()
        .filter_map(|phi| match arms(function, phi)[..] {
            [(Operand::Value(value), from)] if from == cfg::block(loop_.header) => Some((function.instruction(phi).result?, value)),
            _ => None,
        })
        .collect()
}

/// Each value in `swap`, and the exit phi naming it, read as its number
/// wherever `following` reads it.
fn _substituted_exits(context: &mut Context, function: &mut Function, exit: BlockId, following: &BTreeSet<BlockId>, swap: &BTreeMap<ValueId, BigInt>) -> Result<(), String> {
    let header = function.predecessors(exit)[0];
    let aliases = edges::phis(function, exit)
        .into_iter()
        .filter_map(|phi| match arms(function, phi)[..] {
            [(Operand::Value(value), from)] if from == header && swap.contains_key(&value) => Some((phi, value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (&value, number) in swap {
        let bits = context_bits(context, function.value(value).ty);
        let constant = counting::constant(context, number, bits);
        for one in function.users(value).to_vec() {
            let user = function.instruction(one.user);
            let from = if user.opcode == Opcode::Phi {
                match user.operands[one.index as usize + 1] {
                    Operand::Block(from) => Some(from),
                    _ => None,
                }
            } else {
                function.parent(one.user)
            };
            if from.is_some_and(|from| following.contains(&from)) && !aliases.iter().any(|&(phi, _)| phi == one.user) {
                function.set_operand(one.user, one.index as usize, constant);
            }
        }
    }
    for (phi, value) in aliases {
        let number = &swap[&value];
        let result = function.instruction(phi).result.expect("a phi's value");
        let bits = context_bits(context, function.value(result).ty);
        let constant = counting::constant(context, number, bits);
        function.replace_all_uses_with(result, constant);
        function.set_operands(phi, Vec::new());
        function.erase(phi)?;
    }
    Ok(())
}

fn context_bits(context: &Context, ty: llrm_mir::types::TypeId) -> u32 {
    context.types.int_bits(ty).expect("an integer exit value")
}

/// Whether the loop is its counting alone: nothing but arithmetic the exit
/// values replace, and no value of it read outside but its header's phis.
fn _disposable(unit: &Unit, loop_: &Loop) -> bool {
    let function = unit.function;
    let inside = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    loop_.body.iter().flat_map(|&at| function.block(cfg::block(at)).instructions().to_vec()).all(|inst| {
        let op = function.instruction(inst);
        match op.opcode {
            Opcode::Phi => true,
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) | Opcode::ICmp(_) | Opcode::Br => {
                op.result.is_none_or(|result| function.users(result).iter().all(|one| inside(one.user)))
            }
            _ => false,
        }
    })
}

#[cfg(test)]
#[path = "loopexit_tests.rs"]
mod tests;
