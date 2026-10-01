//! Static branch probabilities and block frequencies, as LLVM's
//! `BranchProbabilityInfo` and `BlockFrequencyInfo` estimate them when no
//! profile says otherwise.
//!
//! Each block with more than one successor takes the first heuristic that
//! applies, in LLVM's order, then GCC's call and return heuristics (from
//! Ball and Larus; LLVM has neither):
//! - a successor every path of which ends in `unreachable`, a `noreturn`
//!   or a `cold` call (`noreturn::cold`) is all but never taken;
//! - in a loop, staying in it is taken 124 times to every 4 exits;
//! - `p == q` on pointers fails (20:12), as do `x == 0`, `x == -1`,
//!   `x < 0` and `x <= 0` on integers but truth values and one-bit
//!   tests, and `x == y` on floats; `isnan` is all but never;
//! - a successor that calls, where the other does not, is not taken (67%);
//! - a successor that returns, where the other does not, is not taken (66%).
//!
//! Frequencies are relative to the entry's 1. A loop header runs
//! 1 / (1 - p) times per entry, `p` the probability of coming back round,
//! capped as LLVM caps a loop's scale.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::module::{BlockId, Function, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::facts::Facts;
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
/// MachineBlockPlacement's `StaticLikelyProb`: how likely an edge must be
/// before placement trades the shorter layout for it.
pub const LIKELY: f64 = 0.8;
/// LLVM's cap on how many times a loop header runs per entry.
const LOOP_SCALE: f64 = 4096.0;

/// `function`'s odds. `declarations` are its module's globals; `trips` each
/// loop's header and the trips induction proves it, which stand in for the
/// heuristic's 31 in 32.
pub fn estimated(context: &Context, declarations: &Declarations, function: &Function, shape: &Shape, trips: &BTreeMap<i64, i64>) -> Odds {
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
                let (heuristic, weights) = weighed(context, declarations, function, shape, &cold, block, &successors);
                let total: f64 = weights.iter().sum();
                for (to, weight) in successors.iter().zip(&weights) {
                    *odds.taken.entry((id(block), *to)).or_default() += weight / total;
                }
                odds.by.insert(id(block), heuristic);
            }
        }
    }
    odds.frequency = frequencies(function, shape, &odds, trips);
    odds
}

/// The first heuristic that tells `block`'s successors apart, and their weights.
fn weighed(context: &Context, declarations: &Declarations, function: &Function, shape: &Shape, cold: &BTreeSet<i64>, block: BlockId, successors: &[i64]) -> (Heuristic, Vec<f64>) {
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
        if let Some((heuristic, likely, nan)) = compared(context, declarations, function, block) {
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
fn compared(context: &Context, declarations: &Declarations, function: &Function, block: BlockId) -> Option<(Heuristic, bool, bool)> {
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
            let (predicate, value, compared) = match (constant(left), constant(right)) {
                (_, Some(value)) => (predicate, value, left),
                (Some(value), None) => (predicate.swapped(), value, right),
                _ => return None,
            };
            // A flag is no quantity: BASIC's `IF a AND b` tests a 0/-1 truth
            // value against 0, `x AND 1` one bit; neither says how often.
            if flag(context, function, *compared) {
                return None;
            }
            // Nor is a three-way compare's sign: of its result only equality
            // with 0 says something, that the data are unlikely equal.
            if three_way(context, declarations, function, *compared) && !(value == 0 && matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)) {
                return None;
            }
            let likely = match (predicate, value) {
                (IntPredicate::Eq, 0 | -1) => false,
                (IntPredicate::Ne, 0 | -1) => true,
                (IntPredicate::Slt, 0 | 1) | (IntPredicate::Sle, -1 | 0) => false,
                (IntPredicate::Sgt, -1 | 0) | (IntPredicate::Sge, 0 | 1) => true,
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

/// Whether `operand` has at most one bit that can be set: a truth value,
/// or a one-bit mask. No known-bits analysis answers it, so these are the
/// two forms: the first BASIC's, the second LLVM's `(x & pow2)`.
fn flag(context: &Context, function: &Function, operand: Operand) -> bool {
    truth(function, operand, 4) || single_bit(context, function, operand)
}

/// Whether `operand` is `x & 2^n`, as LLVM's zero heuristic leaves
/// `(x & pow2) ==/!= 0` alone.
fn single_bit(context: &Context, function: &Function, operand: Operand) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let instruction = function.instruction(inst);
    let Opcode::Binary(BinaryOp::And) = instruction.opcode else { return false };
    instruction.operands.iter().any(|one| match one {
        Operand::Constant(at) => matches!(context.get(*at).kind, ConstantKind::Int(bits) if bits.is_power_of_two()),
        _ => false,
    })
}

/// Whether `operand` is the result of a call to a routine stated a
/// three-way compare, at the call or of the callee.
fn three_way(context: &Context, declarations: &Declarations, function: &Function, operand: Operand) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
    let declared = llrm_mir::memory::callee(context, function, inst).and_then(|one| declarations.get(one.0 as usize)).and_then(|one| one.function());
    Facts::of(&info.attrs).three_way_compare() || declared.is_some_and(|one| Facts::of(&one.attrs).three_way_compare())
}

/// Whether `operand` is provably 0 or all ones, or 0 or 1: a compare, one
/// widened, or bitwise logic of those, `depth` operations deep.
fn truth(function: &Function, operand: Operand, depth: u32) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        Opcode::ICmp(_) | Opcode::FCmp(_) => true,
        Opcode::Cast(CastOp::SExt | CastOp::ZExt) => instruction.operands.first().is_some_and(|one| truth(function, *one, depth)),
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) if depth > 0 => {
            instruction.operands.iter().all(|one| truth(function, *one, depth - 1))
        }
        _ => false,
    }
}

/// A natural loop as `propagated` reads it.
pub struct Cycle<'a> {
    pub header: i64,
    pub latches: &'a BTreeSet<i64>,
    pub body: &'a BTreeSet<i64>,
    /// The trips induction proves, in place of the heuristic's odds.
    pub trips: Option<i64>,
}

/// Each block's frequency, the entry's 1, loops scaled by their back edges.
fn frequencies(function: &Function, shape: &Shape, odds: &Odds, trips: &BTreeMap<i64, i64>) -> BTreeMap<i64, f64> {
    let order = reverse_postorder(function);
    let cycles: Vec<Cycle> = shape.loops.iter().map(|one| Cycle { header: one.header, latches: &one.latches, body: &one.body, trips: trips.get(&one.header).copied() }).collect();
    let predecessors = |at: i64| function.predecessors(cfg::block(at)).into_iter().map(id).collect::<Vec<_>>();
    let successors = |at: i64| function.successors(cfg::block(at)).into_iter().map(id).collect::<Vec<_>>();
    propagated(&order, &predecessors, &successors, &cycles, &|from, to| odds.probability(from, to).unwrap_or(0.0))
}

/// Frequencies over any CFG: `order` its reachable blocks in reverse
/// postorder, the entry first; `cycles` its natural loops, innermost first;
/// `edge` each edge's probability. A loop with proven `trips` stays in as
/// many times as they say: its exit test, where the loop has one exit or
/// where it is tested at the header or a latch, takes `1 / (trips + 1 - tested)`
/// out, `tested` being 1 when the test follows a trip and 0 when it precedes
/// one. Only here, so that MIR and LIR estimates agree.
pub fn propagated(order: &[i64], predecessors: &dyn Fn(i64) -> Vec<i64>, successors: &dyn Fn(i64) -> Vec<i64>, cycles: &[Cycle], given: &dyn Fn(i64, i64) -> f64) -> BTreeMap<i64, f64> {
    let counted = |from: i64, to: i64| -> Option<f64> {
        let next = successors(from);
        for one in cycles.iter().filter(|one| one.body.contains(&from)) {
            let inside = next.iter().filter(|at| one.body.contains(at)).count();
            let outside = next.len() - inside;
            if inside == 0 || outside == 0 {
                continue;
            }
            let trips = one.trips?;
            let exiting = one.body.iter().filter(|&&at| successors(at).iter().any(|to| !one.body.contains(to))).count();
            if exiting != 1 && from != one.header && !one.latches.contains(&from) {
                return None;
            }
            let tested = if from == one.header && !one.latches.contains(&from) { 0.0 } else { 1.0 };
            let stay = (trips as f64 - tested) / (trips as f64 + 1.0 - tested);
            return Some(if one.body.contains(&to) { stay / inside as f64 } else { (1.0 - stay) / outside as f64 });
        }
        None
    };
    let edge = |from: i64, to: i64| counted(from, to).unwrap_or_else(|| given(from, to));
    // `to` is a loop header and `from` is in its loop.
    let backward = |from: i64, to: i64| cycles.iter().any(|one| one.header == to && one.body.contains(&from));
    // Innermost first: an inner header's scale is known when its outer loop is weighed.
    let mut scale: BTreeMap<i64, f64> = BTreeMap::new();
    for found in cycles {
        let mut mass: BTreeMap<i64, f64> = BTreeMap::new();
        for &at in order.iter().filter(|at| found.body.contains(at)) {
            let entering: f64 = if at == found.header {
                1.0
            } else {
                predecessors(at)
                    .into_iter()
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
    for (index, &at) in order.iter().enumerate() {
        let entering: f64 = if index == 0 {
            1.0
        } else {
            predecessors(at)
                .into_iter()
                .filter(|from| !backward(*from, at))
                .map(|from| frequency.get(&from).copied().unwrap_or(0.0) * edge(from, at))
                .sum()
        };
        frequency.insert(at, entering * scale.get(&at).copied().unwrap_or(1.0));
    }
    frequency
}

/// Blocks in reverse postorder from `entry`, by `successors`.
pub fn reverse_postorder_of(entry: i64, successors: &dyn Fn(i64) -> Vec<i64>) -> Vec<i64> {
    let mut seen = BTreeSet::from([entry]);
    let mut post = Vec::new();
    let mut stack = vec![(entry, 0usize)];
    while let Some((block, next)) = stack.pop() {
        let all = successors(block);
        if let Some(&to) = all.get(next) {
            stack.push((block, next + 1));
            if seen.insert(to) {
                stack.push((to, 0));
            }
        } else {
            post.push(block);
        }
    }
    post.reverse();
    post
}

/// `function`'s reachable blocks, each before its successors but back edges.
fn reverse_postorder(function: &Function) -> Vec<i64> {
    let Some(entry) = function.entry() else { return Vec::new() };
    reverse_postorder_of(id(entry), &|at| function.successors(cfg::block(at)).into_iter().map(id).collect())
}

#[cfg(test)]
#[path = "branchprob_tests.rs"]
mod tests;
