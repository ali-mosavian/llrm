//! Adapted from llrm-core's `optimize/profit.rs`, the port of
//! `qbopt/optimize/profit.py`: machine-neutral profitability shared by MIR
//! transforms.
//!
//! Everything here prices semantic work only: opcodes, memory effects and
//! CFG frequency.  A kind without a price makes the answer unknown rather
//! than cheap.
//!
//! The machine's facts come in as parameters: each operation's price
//! (`OperationCosts`, from the target: `costs`), how many integer values fit in registers
//! (`capacity`), and that floating values do not take that room -- the old
//! MIR's x87 width, here a floating type.
//!
//! What changed with the IR: only a load or store touches memory, so no
//! operation carries a folded memory operand and `memory_update` prices
//! nothing; `llvm.memset` is the old fill, its bytes the cells; `select`,
//! `frem` and `landingpad` had no old kind and stay unpriced. `spill_risk`
//! takes liveness's live-out sets. `trips` is induction's proven counts
//! (`proven_trips`); a loop it does not name gets the conventional ten.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::consts::Known;
use llrm_analysis::{induction, memory};
use llrm_graph::loops;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::memory::{Callees, callee};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::target::Machine;
use llrm_mir::types::Type;
use llrm_support::hash::{HashMap, HashSet, IndexMap};
use num_traits::ToPrimitive;

// Trips assumed of a loop, and cells of a fill, whose count is not a number.
pub const UNKNOWN_TRIPS: i64 = 10;

pub use llrm_mir::target::OperationCosts;

/// What `outer`'s target prices each operation at; neutral unit prices
/// where it names no target.
pub fn costs(outer: &Outer) -> OperationCosts {
    target_costs(outer.target.as_deref())
}

/// What `target` prices each operation at; neutral unit prices without one.
pub fn target_costs(target: Option<&dyn Machine>) -> OperationCosts {
    target.map_or_else(OperationCosts::default, Machine::costs)
}

/// How many integer values `outer`'s target holds in registers, and how
/// many across a call; 0 each where it names no target, which leaves
/// pressure unpriced.
pub fn registers(outer: &Outer) -> (i64, i64) {
    outer.target.as_ref().map_or((0, 0), |target| (target.registers(), target.call_registers()))
}

fn floating(context: &Context, function: &Function, operand: Operand) -> bool {
    function.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), Type::Float(_)))
}

/// Target price for semantic work, or None when it cannot be priced.
pub fn operation(context: &Context, function: &Function, callees: &Callees, one: InstId, costs: &OperationCosts) -> Option<i64> {
    let instruction = function.instruction(one);
    let price = match &instruction.opcode {
        Opcode::Load { .. } if instruction.result.is_some_and(|value| floating(context, function, Operand::Value(value))) => costs.float_load,
        Opcode::Load { .. } => costs.load,
        Opcode::Store { .. } if floating(context, function, instruction.operands[0]) => costs.float_store,
        Opcode::Store { .. } => costs.store,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) | Opcode::ICmp(_) => costs.add,
        Opcode::Cast(_) | Opcode::ExtractValue(_) | Opcode::InsertValue(_) | Opcode::Phi | Opcode::Freeze => costs.r#move,
        Opcode::Binary(BinaryOp::Mul) => costs.multiply,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => costs.divide,
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) => costs.shift,
        Opcode::Alloca { .. } | Opcode::GetElementPtr { .. } => costs.address,
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub) | Opcode::FNeg | Opcode::FCmp(_) => costs.float_add,
        Opcode::Binary(BinaryOp::FMul) => costs.float_multiply,
        Opcode::Binary(BinaryOp::FDiv) => costs.float_divide,
        Opcode::Call(_) | Opcode::Invoke(_) if callee(context, function, one).and_then(|id| callees.get(&id)).is_some_and(|summary| summary.memset) => {
            let cells = match instruction.operands[2] {
                Operand::Constant(id) => match context.get(id).kind {
                    ConstantKind::Int(count) => i64::try_from(count).expect("a cell count"),
                    _ => UNKNOWN_TRIPS,
                },
                _ => UNKNOWN_TRIPS,
            };
            return Some(costs.fill + cells * costs.fill_cell);
        }
        Opcode::Call(_) | Opcode::Invoke(_) => costs.call,
        Opcode::Ret | Opcode::Resume | Opcode::Unreachable => costs.return_,
        Opcode::Br | Opcode::Switch => costs.branch,
        _ => return None,
    };
    Some(price)
}

pub fn _block(context: &Context, function: &Function, callees: &Callees, block: i64, costs: &OperationCosts) -> Option<i64> {
    function.block(cfg::block(block)).instructions().iter().map(|&one| operation(context, function, callees, one, costs)).sum()
}

/// Semantic work present once in the body, independent of frequency.
pub fn r#static(context: &Context, function: &Function, callees: &Callees, costs: &OperationCosts) -> Option<i64> {
    function.layout().iter().map(|&block| _block(context, function, callees, cfg::id(block), costs)).sum()
}

/// Whether the target prices every instruction here, which a copy's cost needs.
pub fn priced(context: &Context, function: &Function, callees: &Callees, costs: &OperationCosts) -> bool {
    r#static(context, function, callees, costs).is_some()
}

/// Each loop's trips by latch, where induction proves them: the `trips`
/// that replaces the conventional ten.
pub fn proven_trips(unit: &memory::Unit, facts: &IndexMap<ValueId, Known>) -> IndexMap<i64, i64> {
    let mut trips = IndexMap::default();
    for loop_ in loops::loops(&cfg::graph(unit.function), unit.function.entry().map(cfg::id)) {
        if let Some(count) = induction::trip_count(unit, &loop_, facts).and_then(|count| count.to_i64()) {
            trips.extend(loop_.latches.iter().map(|&latch| (latch, count)));
        }
    }
    trips
}

/// Profile-free block frequencies, or `None` for conflicting proofs.
pub fn _frequencies(function: &Function, trips: Option<&IndexMap<i64, i64>>) -> Option<BTreeMap<i64, i64>> {
    let graph = cfg::graph(function);
    let mut frequency = graph.iter().map(|block| (block.at, 1_i64)).collect::<BTreeMap<_, _>>();
    let empty = IndexMap::default();
    let trips = trips.unwrap_or(&empty);
    for loop_ in loops::loops(&graph, function.entry().map(cfg::id)) {
        let exact = loop_.latches.iter().filter_map(|at| trips.get(at).copied()).collect::<BTreeSet<_>>();
        if exact.len() > 1 {
            return None;
        }
        let factor = exact.iter().next().copied().unwrap_or(UNKNOWN_TRIPS);
        for at in &loop_.body {
            if let Some(count) = frequency.get_mut(at) {
                *count *= factor;
            }
        }
    }
    Some(frequency)
}

/// Profile-free expected work, using exact or ten trips per loop level.
///
/// `trips` keys a proven count by latch block; every other loop retains
/// the conventional factor of ten.
pub fn weighted(context: &Context, function: &Function, callees: &Callees, costs: &OperationCosts, trips: Option<&IndexMap<i64, i64>>) -> Option<i64> {
    let frequency = _frequencies(function, trips)?;
    let mut total = 0;
    for &block in function.layout() {
        let priced = _block(context, function, callees, cfg::id(block), costs)?;
        total += frequency[&cfg::id(block)] * priced;
    }
    Some(total)
}

/// Whole-live-range traffic needed to fit MIR within `capacity`.
///
/// Walks every program point, chooses the cheapest still-resident values
/// needed to relieve that point, and retains those choices for the rest of
/// the body.  Floating values do not consume the integer capacity.  Values
/// cheaper to rebuild than to reload use that reconstruction price.
/// `live_out` is each block's live-out set.
pub fn spill_risk(
    context: &Context,
    function: &Function,
    costs: &OperationCosts,
    capacity: i64,
    trips: Option<&IndexMap<i64, i64>>,
    live_out: &BTreeMap<i64, BTreeSet<ValueId>>,
) -> Option<i64> {
    if capacity <= 0 {
        return Some(0);
    }
    let frequency = _frequencies(function, trips)?;
    let mut definitions: BTreeMap<ValueId, i64> = BTreeMap::new();
    let mut uses: BTreeMap<ValueId, i64> = BTreeMap::new();
    let mut recipes: BTreeMap<ValueId, Vec<InstId>> = BTreeMap::new();
    let mut floats: BTreeSet<ValueId> = BTreeSet::new();
    for &block in function.layout() {
        let each = frequency[&cfg::id(block)];
        for &one in function.block(block).instructions() {
            let instruction = function.instruction(one);
            for operand in &instruction.operands {
                if let Operand::Value(value) = *operand {
                    *uses.entry(value).or_insert(0) += each;
                }
            }
            if let Some(value) = instruction.result {
                *definitions.entry(value).or_insert(0) += each;
                recipes.entry(value).or_default().push(one);
            }
            // A floating type is on every value, phis' included: the old
            // widening through phis has nothing left to find.
            floats.extend(instruction.operands.iter().copied().chain(instruction.result.map(Operand::Value)).filter_map(|operand| match operand {
                Operand::Value(value) if floating(context, function, operand) => Some(value),
                _ => None,
            }));
        }
    }

    let reconstruction = |value: &ValueId| -> Option<i64> {
        let found = recipes.get(value).map(Vec::as_slice).unwrap_or(&[]);
        if found.len() != 1 {
            return None;
        }
        let instruction = function.instruction(found[0]);
        let constant = instruction.operands.iter().all(|operand| matches!(operand, Operand::Constant(_)));
        match instruction.opcode {
            // A frame address, or a symbol's.
            Opcode::Alloca { .. } => Some(costs.address),
            Opcode::GetElementPtr { .. } if constant => Some(costs.address),
            _ => None,
        }
    };

    let mut traffic: BTreeMap<ValueId, i64> = BTreeMap::new();
    for value in definitions.keys().chain(uses.keys()).collect::<BTreeSet<_>>() {
        let slot = definitions.get(value).copied().unwrap_or(0) * costs.store + uses.get(value).copied().unwrap_or(0) * costs.load;
        let rematerialize = reconstruction(value);
        traffic.insert(
            *value,
            match rematerialize {
                None => slot,
                Some(price) => slot.min(uses.get(value).copied().unwrap_or(0) * price),
            },
        );
    }

    let mut risk = 0;

    let floats: HashSet<ValueId> = floats.into_iter().collect();
    let traffic: HashMap<ValueId, i64> = traffic.into_iter().collect();
    let mut spilled: HashSet<ValueId> = HashSet::default();
    let mut account = |alive: &BTreeSet<ValueId>| {
        let resident = |value: &&ValueId| !floats.contains(*value) && !spilled.contains(*value);
        // Counted first: most points fit, and then nothing is collected.
        let excess = alive.iter().filter(resident).count() as i64 - capacity;
        if excess > 0 {
            let mut selected = alive.iter().filter(resident).copied().collect::<Vec<_>>();
            selected.sort_by_key(|value| (traffic.get(value).copied().unwrap_or(0), *value));
            selected.truncate(excess as usize);
            risk += selected.iter().map(|value| traffic.get(value).copied().unwrap_or(0)).sum::<i64>();
            spilled.extend(selected);
        }
    };

    for &block in function.layout() {
        let mut alive = live_out[&cfg::id(block)].clone();
        account(&alive);
        // A phi defines at the block's top and reads on its edges.
        for &one in function.block(block).instructions().iter().rev() {
            let instruction = function.instruction(one);
            if instruction.opcode == Opcode::Phi {
                continue;
            }
            if let Some(value) = instruction.result {
                alive.remove(&value);
            }
            alive.extend(instruction.operands.iter().filter_map(|operand| match operand {
                Operand::Value(value) => Some(*value),
                _ => None,
            }));
            account(&alive);
        }
    }
    Some(risk)
}

/// Semantic work plus finite-capacity whole-range spill traffic.
pub fn pressure_adjusted(
    context: &Context,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
    capacity: i64,
    trips: Option<&IndexMap<i64, i64>>,
    live_out: &BTreeMap<i64, BTreeSet<ValueId>>,
) -> Option<i64> {
    let work = weighted(context, function, callees, costs, trips);
    let pressure = spill_risk(context, function, costs, capacity, trips, live_out);
    match (work, pressure) {
        (Some(work), Some(pressure)) => Some(work + pressure),
        _ => None,
    }
}

#[cfg(test)]
#[path = "profit_tests.rs"]
mod tests;
