//! Adapted from llrm-core's `optimize/profit.rs`, the port of
//! `qbopt/optimize/profit.py`: machine-neutral profitability shared by MIR
//! transforms.
//!
//! Everything here prices semantic work only: opcodes, memory effects and
//! CFG frequency.  A kind without a price makes the answer unknown rather
//! than cheap.
//!
//! The machine's facts come in as parameters: each operation's price
//! (`OperationCosts`, from the target: `costs`), how many integer values fit in
//! registers (`capacity`), and that floating values do not take that room --
//! the old MIR's x87 width, here a floating type.
//!
//! What changed with the IR: only a load or store touches memory, so no
//! operation carries a folded memory operand and `memory_update` prices
//! nothing; `llvm.memset` is the old fill, its bytes the cells; `select`,
//! `frem` and `landingpad` had no old kind and stay unpriced. `spill_risk`
//! is the spill model's (`spill`) over the whole body. `trips` is induction's
//! proven counts (`proven_trips`); a loop it does not name gets the
//! conventional ten.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::branchprob;
use llrm_analysis::cfg;
use llrm_analysis::consts::Known;
use llrm_analysis::effects::Declarations;
use llrm_analysis::{induction, memory};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::{Callees, callee};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexMap;
use num_traits::ToPrimitive;

use crate::spill::{self, Room};

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

fn floating(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> bool {
    function.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), Type::Float(_)))
}

/// What advancing a pointer of type `ty` costs: `plain`, the price of the
/// advance as its caller makes it, or the carry into the selector where its
/// space's displacement has one (`DataLayout::carries`), less by a
/// `constant` displacement. The one place the carry's price is stated.
pub fn advance(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
    plain: i64,
    constant: bool,
    costs: &OperationCosts,
) -> i64 {
    match context.types.get(ty) {
        Type::Pointer(space) if layout.carries(*space) && constant => costs.carry_step,
        Type::Pointer(space) if layout.carries(*space) => costs.carry,
        _ => plain,
    }
}

/// Target price for semantic work, or None when it cannot be priced.
pub fn operation(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    one: InstId,
    costs: &OperationCosts,
) -> Option<i64> {
    let instruction = function.instruction(one);
    let price = match &instruction.opcode {
        Opcode::Load { .. }
            if instruction.result.is_some_and(|value| floating(context, function, Operand::Value(value))) =>
        {
            costs.float_load
        }
        Opcode::Load { .. } => costs.load,
        Opcode::Store { .. } if floating(context, function, instruction.operands[0]) => costs.float_store,
        Opcode::Store { .. } => costs.store,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
        | Opcode::ICmp(_) => costs.add,
        Opcode::Cast(_) | Opcode::ExtractValue(_) | Opcode::InsertValue(_) | Opcode::Phi | Opcode::Freeze => {
            costs.r#move
        }
        Opcode::Binary(BinaryOp::Mul) => costs.multiply,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => costs.divide,
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) => costs.shift,
        Opcode::GetElementPtr { .. } => {
            let constant = instruction.operands[1..].iter().all(|&index| matches!(index, Operand::Constant(_)));
            advance(context, layout, function.value(instruction.result?).ty, costs.address, constant, costs)
        }
        Opcode::Alloca { .. } => costs.address,
        // Lowered as a jump around a move, the compare priced on its own.
        Opcode::Select => costs.branch + costs.r#move,
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub) | Opcode::FNeg | Opcode::FCmp(_) => costs.float_add,
        Opcode::Binary(BinaryOp::FMul) => costs.float_multiply,
        Opcode::Binary(BinaryOp::FDiv) => costs.float_divide,
        Opcode::Call(_) | Opcode::Invoke(_)
            if callee(context, function, one).and_then(|id| callees.get(&id)).is_some_and(|summary| summary.memset) =>
        {
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
        Opcode::Br => costs.branch,
        // Lowered as a compare and a jump for each case, then the jump for the
        // rest.
        Opcode::Switch => costs.branch + (instruction.operands.len() as i64 - 2) / 2 * (costs.add + costs.branch),
        _ => return None,
    };
    Some(price)
}

pub fn _block(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    block: i64,
    costs: &OperationCosts,
) -> Option<i64> {
    function
        .block(cfg::block(block))
        .instructions()
        .iter()
        .map(|&one| operation(context, layout, function, callees, one, costs))
        .sum()
}

/// Semantic work present once in the body, independent of frequency.
pub fn r#static(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
) -> Option<i64> {
    function.layout().iter().map(|&block| _block(context, layout, function, callees, cfg::id(block), costs)).sum()
}

/// Where `loop_` stands in the unit's function: how often it is entered for
/// each entry of the function, in `UNIT`ths (what its outside predecessors
/// weigh), and whether it calls a function that may touch memory.
///
/// The frequencies are those of the whole function: `frequencies` holds them
/// for as many loops as ask, which a caller makes once per version of the
/// function.
pub fn site(
    unit: &memory::Unit,
    outer: &Outer,
    loop_: &llrm_analysis::graph::loops::Loop,
    frequencies: &Frequencies,
) -> llrm_analysis::peelsize::Site {
    let entries = match frequencies.0.get_or_init(|| {
        FREQUENCIES.with(|runs| runs.set(runs.get() + 1));
        let trips = proven_trips(unit, &unit.registers());
        _frequencies(unit.context, unit.metadata, &outer.globals, unit.function, Some(&trips))
    }) {
        Some(frequency) => cfg::graph(unit.function)
            .iter()
            .filter(|block| !loop_.body.contains(&block.at) && block.succ.contains(&loop_.header))
            .map(|block| frequency.get(&block.at).copied().unwrap_or(UNIT))
            .sum::<i64>()
            .max(1),
        None => UNIT,
    };
    let callees = outer.callees();
    let writes = unit
        .function
        .walk()
        .filter(|(block, _)| loop_.body.contains(&cfg::id(*block)))
        .any(
            |(_, inst)| unit.calls_out(inst)
                && !callee(unit.context, unit.function, inst)
                    .and_then(|id| callees.get(&id))
                    .is_some_and(|summary| summary.effects == llrm_mir::memory::Effects::NONE),
        );
    llrm_support::debug!(
        "peelsite",
        "callees of the loop at b{}: {:?}",
        loop_.header,
        unit.function
            .walk()
            .filter(|(block, _)| loop_.body.contains(&cfg::id(*block)))
            .filter(|(_, inst)| unit.calls_out(*inst))
            .map(|(_, inst)| callee(unit.context, unit.function, inst)
                .map(|id| (id, callees.get(&id).map(|one| one.effects))))
            .collect::<Vec<_>>()
    );
    llrm_analysis::peelsize::Site { entries, writes }
}

/// A function's block frequencies, worked out when the first loop asks (`site`)
/// and kept for the rest.
#[derive(Default)]
pub struct Frequencies(std::cell::OnceCell<Option<BTreeMap<i64, i64>>>);

thread_local! {
    static FREQUENCIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out a function's frequencies for
/// `site`.
pub fn frequencies_worked() -> usize {
    FREQUENCIES.with(std::cell::Cell::get)
}

/// Whether the target prices every instruction here, which a copy's cost needs.
pub fn priced(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
) -> bool {
    r#static(context, layout, function, callees, costs).is_some()
}

/// Each loop's trips by latch, where induction proves them: the `trips`
/// that replaces the conventional ten.
pub fn proven_trips(
    unit: &memory::Unit,
    facts: &IndexMap<ValueId, Known>,
) -> IndexMap<i64, i64> {
    let mut trips = IndexMap::default();
    for loop_ in &unit.shape().loops {
        if let Some(count) = induction::trip_count(unit, loop_, facts).and_then(|count| count.to_i64()) {
            trips.extend(loop_.latches.iter().map(|&latch| (latch, count)));
        }
    }
    trips
}

/// What one execution of the entry weighs in `_frequencies`: a block taken a
/// third of the time weighs a third of it.
pub const UNIT: i64 = 256;

/// Block frequencies as `branchprob` estimates them (the heuristics, a loop's
/// proven `trips` by latch), per entry, in `UNIT`ths of an execution and never
/// below one, so a cold arm is near free but still ordered. `None`
/// for conflicting proofs.
pub fn _frequencies(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    let shape = cfg::Shape::of(function);
    let empty = IndexMap::default();
    let trips = trips.unwrap_or(&empty);
    let mut counted = BTreeMap::new();
    for loop_ in &shape.loops {
        let exact = loop_.latches.iter().filter_map(|at| trips.get(at).copied()).collect::<BTreeSet<_>>();
        if exact.len() > 1 {
            return None;
        }
        if let Some(count) = exact.into_iter().next() {
            counted.insert(loop_.header, count);
        }
    }
    let odds = branchprob::estimated(context, metadata, globals, function, &shape, &counted);
    Some(
        cfg::graph(function)
            .iter()
            .map(|block| {
                (
                    block.at,
                    odds.frequency.get(&block.at).map_or(UNIT, |one| ((one * UNIT as f64).round() as i64).max(1)),
                )
            })
            .collect(),
    )
}

/// Block frequencies as a product of the trips of each loop around a block, ten
/// where none is proven: the model lsr's and gvn's prices were tuned on (#203,
/// #202), until they are retuned on `_frequencies`. `None` for conflicting
/// proofs.
pub fn _loop_products(
    function: &Function,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
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

/// `_loop_products`, each block weighed by the share of its innermost loop's
/// trips that reach it, as `_frequencies` finds the share: the latch runs once
/// a trip, so a block behind a branch runs `odds[block] / odds[latch]` of them
/// and a block every trip passes through weighs the whole product, as before.
pub fn _loop_products_by_branch(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    let mut weight = _loop_products(function, trips)?;
    let odds = _frequencies(context, metadata, globals, function, trips)?;
    let loops = cfg::Shape::of(function).loops;
    for (at, count) in weight.iter_mut() {
        let Some(innermost) = loops.iter().filter(|one| one.body.contains(at)).min_by_key(|one| one.body.len()) else {
            continue;
        };
        let trip: i64 = innermost.latches.iter().map(|latch| odds.get(latch).copied().unwrap_or(UNIT)).sum();
        let here = odds.get(at).copied().unwrap_or(UNIT);
        if here < trip {
            *count = (*count * here / trip).max(1);
        }
    }
    Some(weight)
}

/// Profile-free expected work at `frequency`, a block's executions per entry
/// (`_frequencies`, or the older `_loop_products`).
pub fn weighted(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
    frequency: &BTreeMap<i64, i64>,
) -> Option<i64> {
    #[cfg(test)]
    PRICED.with(|count| count.set((count.get().0 + 1, count.get().1)));
    let mut total = 0;
    for &block in function.layout() {
        let priced = _block(context, layout, function, callees, cfg::id(block), costs)?;
        total += frequency[&cfg::id(block)] * priced;
    }
    Some(total)
}

/// What fitting MIR within `room` spills, a call keeping what `across`
/// says, as the one spill model (`spill`) forecasts it.
pub fn spill_forecast(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    frequency: &BTreeMap<i64, i64>,
) -> Option<spill::Forecast<llrm_mir::dense::IdSet<ValueId>>> {
    #[cfg(test)]
    PRICED.with(|count| count.set((count.get().0, count.get().1 + 1)));
    if !room.priced() {
        return Some(spill::Forecast { cost: 0, spilled: Default::default(), peak: 0 });
    }
    Some(spill::View::of(context, layout, function, room, across).forecast(costs, frequency))
}

/// Semantic work plus finite-capacity whole-range spill traffic.
#[allow(clippy::too_many_arguments)]
pub fn pressure_adjusted(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    frequency: &BTreeMap<i64, i64>,
) -> Option<i64> {
    let work = weighted(context, layout, function, callees, costs, frequency)?;
    Some(work + spill_forecast(context, layout, function, costs, room, across, frequency)?.cost)
}

/// What `function` costs at `frequency` under `costs` and `room`: the one
/// price a motion is judged by, with the motion and without it.
pub fn motion_price(
    context: &Context,
    layout: &DataLayout,
    outer: &Outer,
    function: &Function,
    costs: &OperationCosts,
    room: Room,
    frequency: &BTreeMap<i64, i64>,
) -> Option<i64> {
    pressure_adjusted(
        context,
        layout,
        function,
        outer.callees(),
        costs,
        room,
        &|inst| spill::kept_across(outer, context, function, inst),
        frequency,
    )
}

#[cfg(test)]
thread_local! {
    /// (`weighted`, `spill_forecast`) calls, for a test that a price is the
    /// work and one forecast.
    pub(crate) static PRICED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

#[cfg(test)]
#[path = "profit_tests.rs"]
mod tests;
