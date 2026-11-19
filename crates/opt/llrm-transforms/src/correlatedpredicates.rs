//! A sum of selects on predicates of one operand pair, made one value of the pair's order.
//!
//! `(a<b) + 2*(a<=b) + 4*(a>b) + 8*(a>=b) + 16*(x<y) + ...` (x, y the signed `a`, `b`) is eight
//! selects on four compares of `a` and `b`. Each compare is a function of the order of the pair:
//! unsigned `<`, `=`, `>` and signed `<`, `=`, `>`, five of the nine combinations being possible.
//! The sum is then a function of the order, a constant for each: two nested selects on the
//! compares and one add replace the eight selects.
//!
//! # How it works
//! In a block, each `icmp` of integers is a truth table over the five orders, by the operands
//! (swapped ones with the predicate swapped). A `select`, `zext` or `sext` of such a compare, and
//! the `add`, `sub` and disjoint `or` of those with each other, with constants and with at most
//! one value from outside, are tracked as `base + constant[order]`. A value used outside the
//! tracked ones is replaced by `base + K`, where `K` is the nested selects on `ult`, `ugt`, `slt`
//! and `sgt` that give `constant[order]`: a sum of a function of the unsigned order and a function
//! of the signed one where the constants allow it, else the tree of the five orders. Fewer than
//! [`MIN_TERMS`] selects or extensions on the compares leave the block alone: `(a>b)-(a<b)`, the
//! three-way compare, is two.
//!
//! MIRROR: the outcome of gcc's jump threading on the branches this expands to
//! (`cfgcleanup.cc` `thread_jump`, which threads a jump whose condition the earlier one implies:
//! `a<b` makes `a<=b` true and `a>b` false), reached here on values. The pin's InstCombine
//! (`foldAndOrOfICmps`, `simplifySelectWithICmpCond`) folds one pair of compares at a time.
//!
//! DEVIATE CORR-PRED (owner wasm-3, exit 2026-12-17): a pass the pin has no counterpart of, on
//! where [`llrm_mir::tti::Tti::folds_correlated_predicates`] says so.

use std::collections::HashMap;

use llrm_mir::context::{ConstantKind, Context, mask, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;

/// Selects and extensions on the pair's compares a block must hold to be rewritten.
const MIN_TERMS: usize = 3;

/// The orders: equal, then (unsigned, signed) as less/less, less/greater, greater/less,
/// greater/greater. Less unsigned with greater signed is `a` large and negative.
const ORDERS: usize = 5;

type Truth = [bool; ORDERS];
type Table = [i128; ORDERS];

/// Unsigned and signed relation of an order, `-1` less, `0` equal, `1` greater.
fn relations(order: usize) -> (i8, i8) {
    [(0, 0), (-1, -1), (-1, 1), (1, -1), (1, 1)][order]
}

/// Whether `a PREDICATE b` holds at `order`.
fn holds(
    predicate: IntPredicate,
    order: usize,
) -> bool {
    let (u, s) = relations(order);
    match predicate {
        IntPredicate::Eq => u == 0,
        IntPredicate::Ne => u != 0,
        IntPredicate::Ult => u < 0,
        IntPredicate::Ule => u <= 0,
        IntPredicate::Ugt => u > 0,
        IntPredicate::Uge => u >= 0,
        IntPredicate::Slt => s < 0,
        IntPredicate::Sle => s <= 0,
        IntPredicate::Sgt => s > 0,
        IntPredicate::Sge => s >= 0,
    }
}

fn swapped(predicate: IntPredicate) -> IntPredicate {
    match predicate {
        IntPredicate::Ult => IntPredicate::Ugt,
        IntPredicate::Ule => IntPredicate::Uge,
        IntPredicate::Ugt => IntPredicate::Ult,
        IntPredicate::Uge => IntPredicate::Ule,
        IntPredicate::Slt => IntPredicate::Sgt,
        IntPredicate::Sle => IntPredicate::Sge,
        IntPredicate::Sgt => IntPredicate::Slt,
        IntPredicate::Sge => IntPredicate::Sle,
        other => other,
    }
}

/// `base + table[order]`.
#[derive(Clone, Copy)]
struct Linear {
    base: Option<Operand>,
    table: Table,
}

/// What a tracked value is.
#[derive(Clone, Copy)]
enum Tracked {
    /// An `i1` that is a function of the order.
    Predicate(Truth),
    Integer(Linear),
}

struct Pair {
    first: Operand,
    second: Operand,
}

struct Block<'a> {
    context: &'a Context,
    pair: Option<Pair>,
    tracked: HashMap<ValueId, Tracked>,
    /// The instructions that made a tracked value, in order, and the terms they hold.
    cone: Vec<InstId>,
    terms: usize,
}

impl Block<'_> {
    fn width(
        &self,
        ty: TypeId,
    ) -> Option<u32> {
        self.context.types.int_bits(ty).filter(|&bits| bits <= 64)
    }

    fn wrap(
        &self,
        value: i128,
        width: u32,
    ) -> i128 {
        signed(value as u128 & mask(width), width)
    }

    /// The constant an operand is, or the tracked integer.
    fn linear(
        &self,
        operand: Operand,
        width: u32,
    ) -> Linear {
        if let Operand::Value(value) = operand {
            if let Some(Tracked::Integer(linear)) = self.tracked.get(&value) {
                return *linear;
            }
        } else if let Operand::Constant(id) = operand {
            let value = match self.context.get(id).kind {
                ConstantKind::Int(bits) => Some(signed(bits, width)),
                ConstantKind::Zero => Some(0),
                _ => None,
            };
            if let Some(value) = value {
                return Linear { base: None, table: [self.wrap(value, width); ORDERS] };
            }
        }
        Linear { base: Some(operand), table: [0; ORDERS] }
    }

    fn truth(
        &self,
        operand: Operand,
    ) -> Option<Truth> {
        let Operand::Value(value) = operand else { return None };
        match self.tracked.get(&value)? {
            Tracked::Predicate(truth) => Some(*truth),
            Tracked::Integer(_) => None,
        }
    }
}

/// `table` as a sum of a function of the unsigned order and one of the signed: `(unsigned,
/// signed)` for less, equal, greater of each, the signed one zero at less and equal.
fn separated(table: &Table) -> Option<([i128; 3], [i128; 3])> {
    let [equal, less_less, less_greater, greater_less, greater_greater] = *table;
    if less_less - less_greater - greater_less + greater_greater != 0 {
        return None;
    }
    let unsigned = [less_less, equal, greater_less];
    let signed = [0, 0, less_greater - less_less];
    Some((unsigned, signed))
}

/// The nested selects that give `values[less]`, `values[equal]`, `values[greater]` of the
/// compares `less`, `greater`: a constant where all three agree.
fn chain(
    function: &mut Function,
    context: &Context,
    before: InstId,
    ty: TypeId,
    values: [i128; 3],
    less: Operand,
    greater: Operand,
) -> Result<Operand, String> {
    let wide = |value: i128| Operand::Constant(context.int(ty, value));
    let [lo, eq, hi] = values;
    if hi == eq {
        return if lo == eq { Ok(wide(eq)) } else { select(function, before, ty, less, wide(lo), wide(eq)) };
    }
    let upper = select(function, before, ty, greater, wide(hi), wide(eq))?;
    if lo == eq { Ok(upper) } else { select(function, before, ty, less, wide(lo), upper) }
}

fn select(
    function: &mut Function,
    before: InstId,
    ty: TypeId,
    condition: Operand,
    on_true: Operand,
    on_false: Operand,
) -> Result<Operand, String> {
    insert(function, before, Opcode::Select, ty, vec![condition, on_true, on_false])
}

fn insert(
    function: &mut Function,
    before: InstId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Result<Operand, String> {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, Position::Before(before))?;
    Ok(Operand::Value(function.instruction(made).result.ok_or("a value")?))
}

/// `base + table[order]` as selects on the pair's compares, before `before`.
fn materialized(
    function: &mut Function,
    context: &Context,
    before: InstId,
    pair: &Pair,
    ty: TypeId,
    linear: &Linear,
) -> Result<Operand, String> {
    let bool_ty = context.types.int(1);
    let mut compare = |function: &mut Function, predicate| {
        insert(
            function,
            before,
            Opcode::ICmp(predicate),
            bool_ty,
            vec![pair.first, pair.second],
        )
    };
    let value = match separated(&linear.table) {
        Some((unsigned, signed_part)) => {
            let ult = compare(function, IntPredicate::Ult)?;
            let ugt = compare(function, IntPredicate::Ugt)?;
            let first = chain(function, context, before, ty, unsigned, ult, ugt)?;
            if signed_part == [0, 0, 0] {
                first
            } else {
                let slt = compare(function, IntPredicate::Slt)?;
                let sgt = compare(function, IntPredicate::Sgt)?;
                let second = chain(function, context, before, ty, signed_part, slt, sgt)?;
                insert(function, before, Opcode::Binary(BinaryOp::Add), ty, vec![first, second])?
            }
        }
        None => {
            // the tree of the five orders
            let wide = |value: i128| Operand::Constant(context.int(ty, value));
            let [equal, less_less, less_greater, greater_less, greater_greater] = linear.table;
            let ult = compare(function, IntPredicate::Ult)?;
            let ugt = compare(function, IntPredicate::Ugt)?;
            let slt = compare(function, IntPredicate::Slt)?;
            let below = select(function, before, ty, slt, wide(less_less), wide(less_greater))?;
            let above = select(function, before, ty, slt, wide(greater_less), wide(greater_greater))?;
            let upper = select(function, before, ty, ugt, above, wide(equal))?;
            select(function, before, ty, ult, below, upper)?
        }
    };
    match linear.base {
        Some(base) => insert(function, before, Opcode::Binary(BinaryOp::Add), ty, vec![base, value]),
        None => Ok(value),
    }
}

/// The block's sums of correlated selects, rewritten. How many values were.
pub fn fold_block(
    context: &Context,
    function: &mut Function,
    block: llrm_mir::module::BlockId,
) -> usize {
    let instructions: Vec<InstId> = function.iter_block(block).collect();
    let mut state = Block { context, pair: None, tracked: HashMap::new(), cone: Vec::new(), terms: 0 };
    // the compares of the first integer compare's pair
    for &inst in &instructions {
        let instruction = function.instruction(inst).clone();
        let Opcode::ICmp(predicate) = instruction.opcode else { continue };
        let [a, b] = instruction.operands[..] else { continue };
        let Some(result) = instruction.result else { continue };
        if function.operand_type(context, a).and_then(|ty| context.types.int_bits(ty)).is_none() {
            continue;
        }
        let (first, second) = state.pair.as_ref().map_or((a, b), |pair| (pair.first, pair.second));
        let predicate = if (a, b) == (first, second) {
            predicate
        } else if (b, a) == (first, second) {
            swapped(predicate)
        } else {
            continue;
        };
        if state.pair.is_none() {
            state.pair = Some(Pair { first, second });
        }
        let mut truth = [false; ORDERS];
        for (order, slot) in truth.iter_mut().enumerate() {
            *slot = holds(predicate, order);
        }
        state.tracked.insert(result, Tracked::Predicate(truth));
        state.cone.push(inst);
    }
    let Some(pair) = state.pair.as_ref().map(|pair| Pair { first: pair.first, second: pair.second }) else {
        return 0;
    };
    // the selects, extensions and sums of them
    let mut roots: Vec<InstId> = Vec::new();
    for &inst in &instructions {
        let instruction = function.instruction(inst).clone();
        let Some(result) = instruction.result else { continue };
        let Some(width) = state.width(instruction.ty) else { continue };
        let tracked = match instruction.opcode {
            Opcode::Select => {
                let [condition, on_true, on_false] = instruction.operands[..] else { continue };
                let Some(truth) = state.truth(condition) else { continue };
                let (yes, no) = (state.linear(on_true, width), state.linear(on_false, width));
                if yes.base.is_some() || no.base.is_some() {
                    continue;
                }
                let mut table = [0; ORDERS];
                for (order, slot) in table.iter_mut().enumerate() {
                    *slot = if truth[order] { yes.table[order] } else { no.table[order] };
                }
                state.terms += 1;
                Tracked::Integer(Linear { base: None, table })
            }
            Opcode::Cast(op @ (CastOp::ZExt | CastOp::SExt)) => {
                let Some(truth) = state.truth(instruction.operands[0]) else { continue };
                let one = if op == CastOp::ZExt { 1 } else { -1 };
                let mut table = [0; ORDERS];
                for (order, slot) in table.iter_mut().enumerate() {
                    *slot = if truth[order] { state.wrap(one, width) } else { 0 };
                }
                state.terms += 1;
                Tracked::Integer(Linear { base: None, table })
            }
            Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Or)) => {
                if op == BinaryOp::Or && !instruction.flags.contains(Flags::DISJOINT) {
                    continue;
                }
                let [x, y] = instruction.operands[..] else { continue };
                let (left, right) = (state.linear(x, width), state.linear(y, width));
                let tracked_operand = |operand: Operand| {
                    matches!(operand, Operand::Value(value) if matches!(state.tracked.get(&value), Some(Tracked::Integer(_))))
                };
                if !tracked_operand(x) && !tracked_operand(y) {
                    continue;
                }
                let subtract = op == BinaryOp::Sub;
                if (subtract && right.base.is_some()) || (left.base.is_some() && right.base.is_some()) {
                    continue;
                }
                let mut table = [0; ORDERS];
                for (order, slot) in table.iter_mut().enumerate() {
                    let rhs = if subtract { -right.table[order] } else { right.table[order] };
                    *slot = state.wrap(left.table[order] + rhs, width);
                }
                Tracked::Integer(Linear { base: left.base.or(right.base), table })
            }
            _ => continue,
        };
        state.tracked.insert(result, tracked);
        state.cone.push(inst);
        roots.push(inst);
    }
    if state.terms < MIN_TERMS {
        return 0;
    }
    // the tracked integers read outside the tracked values: by the instructions of the function
    // that are not in the cone
    let in_cone: std::collections::HashSet<InstId> = state.cone.iter().copied().collect();
    let mut escaping: Vec<InstId> = Vec::new();
    for (_, inst) in function.walk().collect::<Vec<_>>() {
        if in_cone.contains(&inst) {
            continue;
        }
        for operand in &function.instruction(inst).operands {
            if let Operand::Value(value) = operand {
                if let Some(&producer) =
                    roots.iter().find(|&&root| function.instruction(root).result == Some(*value))
                {
                    if !escaping.contains(&producer) {
                        escaping.push(producer);
                    }
                }
            }
        }
    }
    let mut rewritten = 0;
    for root in escaping {
        let instruction = function.instruction(root).clone();
        let (Some(result), Some(Tracked::Integer(linear))) =
            (instruction.result, instruction.result.and_then(|result| state.tracked.get(&result).copied()))
        else {
            continue;
        };
        let Ok(new) = materialized(function, context, root, &pair, instruction.ty, &linear) else {
            continue;
        };
        function.replace_all_uses_with(result, new);
        rewritten += 1;
    }
    // the dead tracked instructions, last first
    for &inst in state.cone.iter().rev() {
        if let Some(result) = function.instruction(inst).result {
            if function.users(result).iter().next().is_none() {
                let _ = function.erase(inst);
            }
        }
    }
    rewritten
}

/// [`fold_block`] over a function. How many values were rewritten.
pub fn fold_function(
    context: &Context,
    function: &mut Function,
) -> usize {
    let blocks: Vec<_> = function.layout().to_vec();
    blocks.into_iter().map(|block| fold_block(context, function, block)).sum()
}

/// The pass: where the target asks for it.
pub struct CorrelatedPredicates;

impl llrm_mir::passes::FunctionPass for CorrelatedPredicates {
    fn name(&self) -> &'static str {
        "correlatedpredicates"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_mir::passes::PreservedAnalyses;
        let wanted = analyses.outer().target().tti().is_some_and(|tti| tti.folds_correlated_predicates())
            || llrm_support::debug::enabled("correlated-all");
        if !wanted {
            return PreservedAnalyses::all();
        }
        let rewritten = fold_function(&*unit.context, unit.function);
        if rewritten > 0 && llrm_support::debug::enabled("correlated") {
            eprintln!("correlated: {rewritten} values rewritten");
        }
        if rewritten == 0 { PreservedAnalyses::all() } else { PreservedAnalyses::none() }
    }
}

#[cfg(test)]
#[path = "correlatedpredicates_tests.rs"]
mod tests;
