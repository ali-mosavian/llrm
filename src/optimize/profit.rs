//! Machine-neutral profitability shared by MIR transforms.
//!
//! Port of `qbopt/optimize/profit.py`.

// ---- early port (agent E) ----
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::{liveness, loops};
use crate::model::mir::{Arg, Kind, MirBlock, MirBody, Op, Value};
use crate::model::passes::OperationCosts;

const _ALU: [Kind; 21] = [
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
const _MOVES: [Kind; 9] = [
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

fn _block(block: &MirBlock, costs: &OperationCosts) -> Option<i64> {
    let priced = block
        .ops
        .iter()
        .map(|one| operation(one, costs))
        .collect::<Option<Vec<_>>>()?;
    Some(block.phis.len() as i64 * costs.r#move + priced.iter().sum::<i64>())
}

/// Profile-free block frequencies, or `None` for conflicting proofs.
fn _frequencies(body: &MirBody, trips: Option<&IndexMap<i64, i64>>) -> Option<IndexMap<i64, i64>> {
    let mut frequency = body
        .blocks
        .iter()
        .map(|block| (block.at, 1))
        .collect::<IndexMap<i64, i64>>();
    let empty = IndexMap::new();
    let trips = trips.unwrap_or(&empty);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let exact = loop_
            .latches
            .iter()
            .filter_map(|at| trips.get(at).copied())
            .collect::<BTreeSet<i64>>();
        if exact.len() > 1 {
            return None;
        }
        let factor = exact.first().copied().unwrap_or(10);
        for at in &loop_.body {
            if let Some(found) = frequency.get_mut(at) {
                *found *= factor;
            }
        }
    }
    Some(frequency)
}

/// Profile-free expected work, using exact or ten trips per loop level.
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
    let mut definitions = BTreeMap::<Value, i64>::new();
    let mut uses = BTreeMap::<Value, i64>::new();
    let mut recipes = BTreeMap::<Value, Vec<&Op>>::new();
    let mut floating = BTreeSet::<Value>::new();
    for block in &body.blocks {
        let each = frequency[&block.at];
        for phi in &block.phis {
            *definitions.entry(phi.result).or_insert(0) += each;
            for value in phi.incoming.values() {
                *uses.entry(*value).or_insert(0) += each;
            }
        }
        for op in &block.ops {
            floating.extend(op.args.iter().chain(op.results.iter()).filter_map(|arg| match arg {
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
        let found = recipes.get(value)?;
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

    let mut traffic = BTreeMap::<Value, i64>::new();
    for value in definitions.keys().chain(uses.keys()).collect::<BTreeSet<_>>() {
        let used = uses.get(value).copied().unwrap_or(0);
        let slot = definitions.get(value).copied().unwrap_or(0) * costs.store + used * costs.load;
        let rematerialize = reconstruction(value);
        traffic.insert(
            *value,
            match rematerialize {
                None => slot,
                Some(rematerialize) => slot.min(used * rematerialize),
            },
        );
    }

    let found = liveness::live(body);
    let mut spilled = BTreeSet::<Value>::new();
    let mut risk = 0;

    let mut account = |alive: &BTreeSet<Value>| {
        let values = alive
            .iter()
            .filter(|value| !value.flags && !floating.contains(value) && !spilled.contains(value))
            .copied()
            .collect::<Vec<_>>();
        let excess = values.len() as i64 - capacity;
        if excess > 0 {
            let mut selected = values;
            selected.sort_by_key(|value| (traffic.get(value).copied().unwrap_or(0), value.id));
            selected.truncate(excess as usize);
            risk += selected
                .iter()
                .map(|value| traffic.get(value).copied().unwrap_or(0))
                .sum::<i64>();
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
