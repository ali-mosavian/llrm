//! Port of `qbopt/optimize/profit.py`: machine-neutral profitability shared
//! by MIR transforms.
//!
//! Everything here prices semantic work only: MIR kinds, memory effects and
//! CFG frequency.  A kind without a price makes the answer unknown rather
//! than cheap.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::analysis::{consts, induction, loops};
use crate::model::mir::{Arg, Kind, MirBlock, MirBody, Op};
use crate::model::passes::OperationCosts;
use std::rc::Rc;
use num_traits::ToPrimitive;

// Trips assumed of a loop, and cells of a fill, whose count is not a number.
pub const UNKNOWN_TRIPS: i64 = 10;

pub const _ALU: [Kind; 21] = [
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
pub const _MOVES: [Kind; 9] = [
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
pub fn operation(one: &Op, costs: &OperationCosts) -> Option<i64> {
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
    } else if one.kind == Kind::Fill {
        let cells = match &one.args[1] {
            Arg::Const(count) => count.n.to_i64().expect("a cell count"),
            _ => UNKNOWN_TRIPS,
        };
        return Some(costs.fill + cells * costs.fill_cell);
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

pub fn _block(block: &MirBlock, costs: &OperationCosts) -> Option<i64> {
    let priced = block.ops.iter().map(|one| operation(one, costs)).collect::<Vec<_>>();
    if priced.iter().any(Option::is_none) {
        return None;
    }
    Some(block.phis.len() as i64 * costs.r#move + priced.into_iter().flatten().sum::<i64>())
}

/// Semantic work present once in the body, independent of frequency.
pub fn r#static(body: &MirBody, costs: &OperationCosts) -> Option<i64> {
    let priced = body.blocks.iter().map(|block| _block(block, costs)).collect::<Vec<_>>();
    if priced.iter().any(Option::is_none) { None } else { Some(priced.into_iter().flatten().sum()) }
}

/// Profile-free block frequencies, or `None` for conflicting proofs.
pub fn _frequencies(body: &Rc<MirBody>, trips: Option<&IndexMap<i64, i64>>) -> Option<BTreeMap<i64, i64>> {
    let mut frequency = body.blocks.iter().map(|block| (block.at, 1_i64)).collect::<BTreeMap<_, _>>();
    let empty = IndexMap::default();
    let trips = trips.unwrap_or(&empty);
    let mut facts = None;
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let mut exact = loop_.latches.iter().filter_map(|at| trips.get(at).copied()).collect::<BTreeSet<_>>();
        if exact.len() > 1 {
            return None;
        }
        if exact.is_empty() {
            // A loop nobody named still has its proven count: guessing ten for
            // an 8-trip loop priced unrolling its inner loop above a 64-cell fill.
            let facts = facts.get_or_insert_with(|| consts::known(body, None, None, None, None));
            exact.extend(
                induction::trips_unless_stopped(body, &loop_, facts).map(|count| count.to_i64().expect("a trip count fits")),
            );
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
/// `trips` keys a proven count by latch address; every other loop retains
/// the conventional factor of ten.
pub fn weighted(body: &Rc<MirBody>, costs: &OperationCosts, trips: Option<&IndexMap<i64, i64>>) -> Option<i64> {
    let frequency = _frequencies(body, trips)?;
    let mut total = 0;
    for block in &body.blocks {
        let priced = _block(block, costs)?;
        total += frequency[&block.at] * priced;
    }
    Some(total)
}
