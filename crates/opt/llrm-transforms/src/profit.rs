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
//! is the spill model's (`spill`) over the whole body. `trips` is induction's proven counts
//! (`proven_trips`); a loop it does not name gets the conventional ten.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::consts::Known;
use llrm_analysis::liveness::Liveness;
use llrm_analysis::{induction, memory};

use crate::spill::{self, Room};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::{Callees, callee};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;
use num_traits::ToPrimitive;

// Trips assumed of a loop, and cells of a fill, whose count is not a number.
pub const UNKNOWN_TRIPS: i64 = 10;

pub use llrm_mir::target::OperationCosts;

/// What `outer`'s target prices each operation at.
pub fn costs(outer: &Outer) -> OperationCosts {
    outer.target().costs()
}

/// How many integer values `outer`'s target holds in registers, and how
/// many across a call.
pub fn registers(outer: &Outer) -> Room {
    Room::of(outer)
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
    for loop_ in &unit.shape().loops {
        if let Some(count) = induction::trip_count(unit, loop_, facts).and_then(|count| count.to_i64()) {
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
    for loop_ in cfg::Shape::of(function).loops {
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

/// Whole-live-range traffic needed to fit MIR within `room`, a call
/// keeping what `across` says, as the one spill model (`spill`) prices it.
#[allow(clippy::too_many_arguments)]
pub fn spill_risk(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    trips: Option<&IndexMap<i64, i64>>,
    found: &Liveness,
) -> Option<i64> {
    if !room.priced() {
        return Some(0);
    }
    let frequency = _frequencies(function, trips)?;
    let cells = spill::cells(function);
    let traffic = spill::traffic(function, &frequency, &cells, costs, &|_| true, &|value| spill::words(context, layout, function, value));
    let counted = |value: ValueId| spill::integer(context, function, value);
    let points = function.layout().iter().flat_map(|&block| spill::sites(function, found, block, room, across, &cells, &counted)).flat_map(spill::Site::points);
    Some(spill::spilled(points, |cell| traffic.get(&cell).map_or(0, |one| one.price(costs))))
}

/// Semantic work plus finite-capacity whole-range spill traffic.
pub fn pressure_adjusted(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    trips: Option<&IndexMap<i64, i64>>,
    found: &Liveness,
) -> Option<i64> {
    let work = weighted(context, function, callees, costs, trips);
    let pressure = spill_risk(context, layout, function, costs, room, across, trips, found);
    match (work, pressure) {
        (Some(work), Some(pressure)) => Some(work + pressure),
        _ => None,
    }
}

#[cfg(test)]
#[path = "profit_tests.rs"]
mod tests;
