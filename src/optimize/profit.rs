//! Port of `qbopt/optimize/profit.py`: machine-neutral profitability shared
//! by MIR transforms.
//!
//! Everything here prices semantic work only: MIR kinds, memory effects and
//! CFG frequency.  A kind without a price makes the answer unknown rather
//! than cheap.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::{HashMap, HashSet, IndexMap};

use crate::analysis::{liveness, loops};
use crate::model::mir::{Arg, Kind, MirBlock, MirBody, Op, Value};
use crate::model::passes::OperationCosts;

pub(crate) const _ALU: [Kind; 21] = [
    Kind::Add,
    Kind::Sub,
    Kind::AddCarry,
    Kind::SubBorrow,
    Kind::Increment,
    Kind::Decrement,
    Kind::And,
    Kind::Or,
    Kind::Xor,
    Kind::Neg,
    Kind::Not,
    Kind::Lt,
    Kind::Le,
    Kind::Gt,
    Kind::Ge,
    Kind::Eq,
    Kind::Ne,
    Kind::Below,
    Kind::BelowEq,
    Kind::Above,
    Kind::AboveEq,
];
pub(crate) const _MOVES: [Kind; 9] = [
    Kind::Copy,
    Kind::Convert,
    Kind::SignExtend,
    Kind::ZeroExtend,
    Kind::Extract,
    Kind::Concat,
    Kind::Join,
    Kind::Arg,
    Kind::Result,
];

/// Target price for semantic work, or None when it cannot be priced.
pub(crate) fn operation(one: &Op, costs: &OperationCosts) -> Option<i64> {
    if one.kind == Kind::Nothing {
        return Some(0);
    }
    if one.kind == Kind::Fload {
        return Some(costs.float_load);
    }
    if one.kind == Kind::Fstore {
        return Some(costs.float_store);
    }
    let folded_update = one.loads.len() == 1 && one.stores.len() == 1 && one.loads == one.stores;
    let memory = if folded_update {
        costs.memory_update
    } else {
        one.loads.len() as i64 * costs.load + one.stores.len() as i64 * costs.store
    };
    if matches!(one.kind, Kind::Load | Kind::Store) {
        return Some(memory);
    }
    if folded_update {
        return Some(memory);
    }
    let work = if _ALU.contains(&one.kind) {
        costs.add
    } else if _MOVES.contains(&one.kind) {
        costs.r#move
    } else if matches!(one.kind, Kind::Mul | Kind::Smulhi) {
        costs.multiply
    } else if matches!(one.kind, Kind::Div | Kind::Rem | Kind::Divmod | Kind::Udivmod) {
        costs.divide
    // A fixed-point product is a widening multiply then a shift back; a
    // quotient, the shift first. Unpriced, one left nbody's whole body
    // unpriceable and every loop candidate was built only to be refused.
    } else if one.kind == Kind::FixedMul {
        costs.multiply + costs.shift
    } else if one.kind == Kind::FixedDiv {
        costs.divide + costs.shift
    } else if matches!(one.kind, Kind::Shl | Kind::Shr | Kind::Sar) {
        costs.shift
    } else if matches!(one.kind, Kind::Address | Kind::PtrOffset) {
        costs.address
    } else if matches!(one.kind, Kind::Fadd | Kind::Fsub | Kind::Fneg | Kind::Fabs | Kind::Fcompare) {
        costs.float_add
    } else if one.kind == Kind::Fmul {
        costs.float_multiply
    } else if matches!(one.kind, Kind::Fdiv | Kind::Fsqrt) {
        costs.float_divide
    } else if one.kind == Kind::Fcheck {
        costs.float_store
    } else if one.kind == Kind::Call {
        costs.call
    } else if matches!(one.kind, Kind::Return | Kind::Escape) {
        costs.return_
    } else if matches!(one.kind, Kind::Branch | Kind::Switch | Kind::Jump) {
        costs.branch
    } else {
        return None;
    };
    Some(work + memory)
}

pub(crate) fn _block(block: &MirBlock, costs: &OperationCosts) -> Option<i64> {
    let priced = block.ops.iter().map(|one| operation(one, costs)).collect::<Vec<_>>();
    if priced.iter().any(Option::is_none) {
        return None;
    }
    Some(block.phis.len() as i64 * costs.r#move + priced.into_iter().flatten().sum::<i64>())
}

/// Semantic work present once in the body, independent of frequency.
pub(crate) fn r#static(body: &MirBody, costs: &OperationCosts) -> Option<i64> {
    let priced = body.blocks.iter().map(|block| _block(block, costs)).collect::<Vec<_>>();
    if priced.iter().any(Option::is_none) { None } else { Some(priced.into_iter().flatten().sum()) }
}

/// Profile-free block frequencies, or `None` for conflicting proofs.
pub(crate) fn _frequencies(body: &MirBody, trips: Option<&IndexMap<i64, i64>>) -> Option<BTreeMap<i64, i64>> {
    let mut frequency = body.blocks.iter().map(|block| (block.at, 1_i64)).collect::<BTreeMap<_, _>>();
    let empty = IndexMap::default();
    let trips = trips.unwrap_or(&empty);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let exact = loop_.latches.iter().filter_map(|at| trips.get(at).copied()).collect::<BTreeSet<_>>();
        if exact.len() > 1 {
            return None;
        }
        let factor = exact.iter().next().copied().unwrap_or(10);
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
/// `trips` keys a proven count by latch address; every other loop retains
/// the conventional factor of ten.
pub(crate) fn weighted(body: &MirBody, costs: &OperationCosts, trips: Option<&IndexMap<i64, i64>>) -> Option<i64> {
    let frequency = _frequencies(body, trips)?;
    let mut total = 0;
    for block in &body.blocks {
        let priced = _block(block, costs)?;
        total += frequency[&block.at] * priced;
    }
    Some(total)
}

/// Whole-live-range traffic needed to fit MIR within `capacity`.
///
/// Walks every program point, chooses the cheapest still-resident values
/// needed to relieve that point, and retains those choices for the rest of
/// the body.  x87 values do not consume the integer capacity.  Literal and
/// fixed-address values use their cheaper reconstruction price.
pub(crate) fn spill_risk(
    body: &MirBody,
    costs: &OperationCosts,
    capacity: i64,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<i64> {
    if capacity <= 0 {
        return Some(0);
    }
    let frequency = _frequencies(body, trips)?;
    let mut definitions: BTreeMap<Value, i64> = BTreeMap::new();
    let mut uses: BTreeMap<Value, i64> = BTreeMap::new();
    let mut recipes: BTreeMap<Value, Vec<&Op>> = BTreeMap::new();
    let mut floating: BTreeSet<Value> = BTreeSet::new();
    for block in &body.blocks {
        let each = frequency[&block.at];
        for phi in &block.phis {
            *definitions.entry(phi.result).or_insert(0) += each;
            for value in phi.incoming.values() {
                *uses.entry(*value).or_insert(0) += each;
            }
        }
        for op in &block.ops {
            floating.extend(op.args.iter().chain(&op.results).filter_map(|arg| match arg {
                Arg::Held(held) if held.width == 10 => Some(held.value),
                _ => None,
            }));
            for value in &op.defines {
                *definitions.entry(*value).or_insert(0) += each;
                recipes.entry(*value).or_default().push(op);
            }
            for value in &op.uses {
                *uses.entry(*value).or_insert(0) += each;
            }
        }
    }

    loop {
        let before = floating.len();
        for block in &body.blocks {
            for phi in &block.phis {
                if floating.contains(&phi.result) || phi.incoming.values().any(|value| floating.contains(value)) {
                    floating.insert(phi.result);
                    floating.extend(phi.incoming.values().copied());
                }
            }
        }
        if floating.len() == before {
            break;
        }
    }

    let reconstruction = |value: &Value| -> Option<i64> {
        let found = recipes.get(value).map(Vec::as_slice).unwrap_or(&[]);
        if found.len() != 1 {
            return None;
        }
        let op = found[0];
        if !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() || op.results.len() != 1 {
            return None;
        }
        if op.kind == Kind::Copy && op.args.len() == 1 && matches!(op.args[0], Arg::Const(_)) {
            return Some(costs.r#move);
        }
        if op.kind == Kind::Address
            && op.args.len() == 1
            && matches!(op.args[0], Arg::FrameAddress(_) | Arg::Symbol(_))
        {
            return Some(costs.address);
        }
        None
    };

    let mut traffic: BTreeMap<Value, i64> = BTreeMap::new();
    for value in definitions.keys().chain(uses.keys()).collect::<BTreeSet<_>>() {
        let slot = definitions.get(value).copied().unwrap_or(0) * costs.store
            + uses.get(value).copied().unwrap_or(0) * costs.load;
        let rematerialize = reconstruction(value);
        traffic.insert(
            *value,
            match rematerialize {
                None => slot,
                Some(price) => slot.min(uses.get(value).copied().unwrap_or(0) * price),
            },
        );
    }

    let found = liveness::live(body);
    let mut risk = 0;

    let floating: HashSet<Value> = floating.into_iter().collect();
    let traffic: HashMap<Value, i64> = traffic.into_iter().collect();
    let mut spilled: HashSet<Value> = HashSet::default();
    let mut account = |alive: &BTreeSet<Value>| {
        let resident = |value: &&Value| !value.flags && !floating.contains(*value) && !spilled.contains(*value);
        // Counted first: most points fit, and then nothing is collected.
        let excess = alive.iter().filter(resident).count() as i64 - capacity;
        if excess > 0 {
            let mut selected = alive.iter().filter(resident).copied().collect::<Vec<_>>();
            selected.sort_by_key(|value| (traffic.get(value).copied().unwrap_or(0), value.id));
            selected.truncate(excess as usize);
            risk += selected.iter().map(|value| traffic.get(value).copied().unwrap_or(0)).sum::<i64>();
            spilled.extend(selected);
        }
    };

    for block in &body.blocks {
        let mut alive = found.live_out[&block.at].clone();
        account(&alive);
        for op in block.ops.iter().rev() {
            for value in &op.defines {
                alive.remove(value);
            }
            alive.extend(op.uses.iter().copied());
            account(&alive);
        }
    }
    Some(risk)
}

/// Semantic work plus finite-capacity whole-range spill traffic.
pub(crate) fn pressure_adjusted(
    body: &MirBody,
    costs: &OperationCosts,
    capacity: i64,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<i64> {
    let work = weighted(body, costs, trips);
    let pressure = spill_risk(body, costs, capacity, trips);
    match (work, pressure) {
        (Some(work), Some(pressure)) => Some(work + pressure),
        _ => None,
    }
}

#[cfg(test)]
#[path = "profit_tests.rs"]
mod tests;
