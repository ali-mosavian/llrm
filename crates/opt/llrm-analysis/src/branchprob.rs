//! Static branch probabilities and block frequencies, as LLVM's
//! `BranchProbabilityInfo` and `BlockFrequencyInfo` estimate them when no
//! profile says otherwise.
//!
//! Each block with more than one successor takes the first heuristic that
//! applies, in LLVM's order, then Ball and Larus's call and return
//! heuristics, which GCC's `predict.def` also uses:
//! - a successor every path of which ends in `unreachable`, a `noreturn`
//!   or a `cold` call (`noreturn::cold`) is all but never taken;
//! - in a loop, staying in it is taken 124 times to every 4 exits;
//! - `p == q` on pointers fails (20:12), as does `x == 0`, `x == -1` and
//!   `x < 0` on integers, and `x == y` on floats; `isnan` is all but never;
//! - a successor that calls, where the other does not, is not taken (67%);
//! - a successor that returns, where the other does not, is not taken (66%).
//!
//! Frequencies are relative to the entry's 1. A loop header runs
//! 1 / (1 - p) times per entry, `p` the probability of coming back round,
//! capped as LLVM caps a loop's scale.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::module::{BlockId, Function, Operand, ValueDef};
use llrm_mir::opcode::{FloatPredicate, IntPredicate, Opcode};
use llrm_mir::types::Type;
use llrm_mir::{ConstantKind, Context};

use crate::cfg::{self, Shape, id};
use crate::effects::Declarations;
use crate::noreturn;

/// Why a block's successors have the probabilities they do.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Heuristic {
    Unreachable,
    Loop,
    Pointer,
    Zero,
    Float,
    Call,
    Return,
    Even,
}

/// Every edge's probability and every block's frequency, by block id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Odds {
    pub taken: BTreeMap<(i64, i64), f64>,
    pub frequency: BTreeMap<i64, f64>,
    /// Which heuristic set each branching block's probabilities.
    pub by: BTreeMap<i64, Heuristic>,
}

impl Odds {
    /// The probability of `from`'s edge to `to`: certain for a lone successor.
    pub fn probability(&self, from: i64, to: i64) -> Option<f64> {
        self.taken.get(&(from, to)).copied()
    }
}

// LLVM's weights (BranchProbabilityInfo.cpp), and GCC's hit rates.
const UNREACHABLE: (f64, f64) = (1.0, ((1 << 20) - 1) as f64);
const LOOP: (f64, f64) = (124.0, 4.0);
const OPCODE: (f64, f64) = (20.0, 12.0);
const ORDERED: (f64, f64) = ((1024 * 1024 - 1) as f64, 1.0);
const CALL: (f64, f64) = (67.0, 33.0);
const RETURN: (f64, f64) = (66.0, 34.0);
/// LLVM's cap on how many times a loop header runs per entry.
const LOOP_SCALE: f64 = 4096.0;

/// `function`'s odds. `declarations` are its module's globals.
pub fn estimated(context: &Context, declarations: &Declarations, function: &Function, shape: &Shape) -> Odds {
    let terminal = noreturn::terminal_sites(context, declarations, function, &BTreeSet::new());
    let cold = noreturn::cold(context, declarations, function, &terminal);
    let mut odds = Odds::default();
    for &block in function.layout() {
        let successors: Vec<i64> = function.successors(block).into_iter().map(id).collect();
        match successors.as_slice() {
            [] => {}
            [only] => {
                odds.taken.insert((id(block), *only), 1.0);
            }
            _ => {
                let (heuristic, weights) = weighed(context, function, shape, &cold, block, &successors);
                let total: f64 = weights.iter().sum();
                for (to, weight) in successors.iter().zip(&weights) {
                    *odds.taken.entry((id(block), *to)).or_default() += weight / total;
                }
                odds.by.insert(id(block), heuristic);
            }
        }
    }
    odds.frequency = frequencies(function, shape, &odds);
    odds
}

/// The first heuristic that tells `block`'s successors apart, and their weights.
fn weighed(context: &Context, function: &Function, shape: &Shape, cold: &BTreeSet<i64>, block: BlockId, successors: &[i64]) -> (Heuristic, Vec<f64>) {
    let split = |favoured: &dyn Fn(i64) -> bool, (yes, no): (f64, f64)| -> Option<Vec<f64>> {
        let count = successors.iter().filter(|&&at| favoured(at)).count();
        if count == 0 || count == successors.len() {
            return None;
        }
        let rest = successors.len() - count;
        Some(successors.iter().map(|&at| if favoured(at) { yes / count as f64 } else { no / rest as f64 }).collect())
    };
    if let Some(weights) = split(&|at| !cold.contains(&at), UNREACHABLE.swap()) {
        return (Heuristic::Unreachable, weights);
    }
    if let Some(found) = shape.loops.iter().find(|one| one.body.contains(&id(block))) {
        if let Some(weights) = split(&|at| found.body.contains(&at), LOOP) {
            return (Heuristic::Loop, weights);
        }
    }
    if successors.len() == 2 {
        if let Some((heuristic, likely, nan)) = compared(context, function, block) {
            let weights = if nan { ORDERED } else { OPCODE };
            let (when_true, when_false) = if likely { weights } else { weights.swap() };
            return (heuristic, vec![when_true, when_false]);
        }
    }
    let calls = |at: i64| function.block(cfg::block(at)).instructions().iter().any(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)));
    if let Some(weights) = split(&|at| !calls(at), CALL) {
        return (Heuristic::Call, weights);
    }
    let returns = |at: i64| function.terminator(cfg::block(at)).is_some_and(|last| function.instruction(last).opcode == Opcode::Ret);
    if let Some(weights) = split(&|at| !returns(at), RETURN) {
        return (Heuristic::Return, weights);
    }
    (Heuristic::Even, vec![1.0; successors.len()])
}

trait Swap {
    fn swap(self) -> Self;
}

impl Swap for (f64, f64) {
    fn swap(self) -> Self {
        (self.1, self.0)
    }
}

/// The compare deciding `block`'s branch, if a heuristic reads it: which
/// one, whether the branch's true edge is the likely one, and whether it
/// tests for NaN, which takes the extreme weights.
fn compared(context: &Context, function: &Function, block: BlockId) -> Option<(Heuristic, bool, bool)> {
    let branch = function.instruction(function.terminator(block)?);
    let (Opcode::Br, [Operand::Value(condition), ..]) = (&branch.opcode, branch.operands.as_slice()) else { return None };
    let ValueDef::Instruction(inst) = function.value(*condition).def else { return None };
    let compare = function.instruction(inst);
    let [left, right] = compare.operands.as_slice() else { return None };
    match compare.opcode {
        Opcode::ICmp(predicate) => {
            let pointer = |one: &Operand| function.operand_type(context, *one).is_some_and(|ty| matches!(context.types.get(ty), Type::Pointer(_)));
            if pointer(left) || pointer(right) {
                return match predicate {
                    IntPredicate::Eq => Some((Heuristic::Pointer, false, false)),
                    IntPredicate::Ne => Some((Heuristic::Pointer, true, false)),
                    _ => None,
                };
            }
            let constant = |one: &Operand| match one {
                Operand::Constant(at) => match context.get(*at).kind {
                    ConstantKind::Int(bits) => {
                        let width = function.operand_type(context, *one).and_then(|ty| context.types.int_bits(ty))?;
                        let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                        Some(if bits & mask == mask { -1 } else { i128::try_from(bits & mask).ok()? })
                    }
                    ConstantKind::Null | ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            };
            // The compared value on the left: `0 == x` reads as `x == 0`.
            let (predicate, value) = match (constant(left), constant(right)) {
                (_, Some(value)) => (predicate, value),
                (Some(value), None) => (predicate.swapped(), value),
                _ => return None,
            };
            let likely = match (predicate, value) {
                (IntPredicate::Eq, 0 | -1) => false,
                (IntPredicate::Ne, 0 | -1) => true,
                (IntPredicate::Slt, 0) | (IntPredicate::Sle, -1) => false,
                (IntPredicate::Sgt, -1) | (IntPredicate::Sge, 0) => true,
                _ => return None,
            };
            Some((Heuristic::Zero, likely, false))
        }
        Opcode::FCmp(predicate) => match predicate {
            FloatPredicate::Oeq | FloatPredicate::Ueq => Some((Heuristic::Float, false, false)),
            FloatPredicate::One | FloatPredicate::Une => Some((Heuristic::Float, true, false)),
            FloatPredicate::Uno => Some((Heuristic::Float, false, true)),
            FloatPredicate::Ord => Some((Heuristic::Float, true, true)),
            _ => None,
        },
        _ => None,
    }
}

/// Each block's frequency, the entry's 1, loops scaled by their back edges.
fn frequencies(function: &Function, shape: &Shape, odds: &Odds) -> BTreeMap<i64, f64> {
    let order = reverse_postorder(function);
    // `to` is a loop header and `from` is in its loop.
    let backward = |from: i64, to: i64| shape.loops.iter().any(|one| one.header == to && one.body.contains(&from));
    let edge = |from: i64, to: i64| odds.probability(from, to).unwrap_or(0.0);
    // Innermost first: an inner header's scale is known when its outer loop is weighed.
    let mut scale: BTreeMap<i64, f64> = BTreeMap::new();
    for found in &shape.loops {
        let mut mass: BTreeMap<i64, f64> = BTreeMap::new();
        for &at in order.iter().filter(|at| found.body.contains(at)) {
            let entering: f64 = if at == found.header {
                1.0
            } else {
                function
                    .predecessors(cfg::block(at))
                    .into_iter()
                    .map(id)
                    .filter(|from| found.body.contains(from) && !backward(*from, at))
                    .map(|from| mass.get(&from).copied().unwrap_or(0.0) * edge(from, at))
                    .sum()
            };
            let inner = if at == found.header { 1.0 } else { scale.get(&at).copied().unwrap_or(1.0) };
            mass.insert(at, entering * inner);
        }
        let back: f64 = found.latches.iter().map(|latch| mass.get(latch).copied().unwrap_or(0.0) * edge(*latch, found.header)).sum();
        scale.insert(found.header, (1.0 / (1.0 - back.min(1.0 - 1.0 / LOOP_SCALE))).min(LOOP_SCALE));
    }
    let mut frequency: BTreeMap<i64, f64> = BTreeMap::new();
    for &at in &order {
        let entering: f64 = if Some(at) == function.entry().map(id) {
            1.0
        } else {
            function
                .predecessors(cfg::block(at))
                .into_iter()
                .map(id)
                .filter(|from| !backward(*from, at))
                .map(|from| frequency.get(&from).copied().unwrap_or(0.0) * edge(from, at))
                .sum()
        };
        frequency.insert(at, entering * scale.get(&at).copied().unwrap_or(1.0));
    }
    frequency
}

/// `function`'s reachable blocks, each before its successors but back edges.
fn reverse_postorder(function: &Function) -> Vec<i64> {
    let Some(entry) = function.entry() else { return Vec::new() };
    let mut seen = BTreeSet::new();
    let mut post = Vec::new();
    let mut stack = vec![(entry, 0usize)];
    seen.insert(entry);
    while let Some((block, next)) = stack.pop() {
        let successors = function.successors(block);
        if let Some(&to) = successors.get(next) {
            stack.push((block, next + 1));
            if seen.insert(to) {
                stack.push((to, 0));
            }
        } else {
            post.push(id(block));
        }
    }
    post.reverse();
    post
}

#[cfg(test)]
#[path = "branchprob_tests.rs"]
mod tests;
