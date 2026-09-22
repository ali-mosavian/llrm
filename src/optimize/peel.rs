//! Exact CFG loop peeling, accepted only after ordinary MIR simplifies it.
//!
//! Port of `qbopt/optimize/peel.py`.  Peeling is the general CFG counterpart
//! of full straight-line unrolling: clone every block of a proven exact loop,
//! retain the residual loop as a correctness fallback, and let the normal
//! fixed point prove the residual unreachable.

// Its callers live in transform.py, not yet ported.

use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::analysis::loops::{self, Loop};
use crate::analysis::{consts, induction};
use crate::model::mir::{Kind, MirBlock, MirBody};
use crate::model::passes::{MIRTransform, Where};
use crate::optimize::{lcssa, loopclone, unroll};

// Bound the transient MIR of branchy and nested exact loops, leaving room to
// evaluate a small fixed outer loop after an eight-way inner specialization.
pub const MAX_SPECULATIVE_OPERATIONS: i64 = 4096;
// Every conditional floating copy crosses the full strict-FP fixed point
// before profitability can reject it: an analysis resource limit, not a
// claim that larger source loops are illegal.
pub const MAX_CONDITIONAL_FLOAT_OPERATIONS: i64 = 512;

pub struct Peel {
    pub r#where: Where,
}

impl Peel {
    pub fn new(r#where: Where) -> Self {
        Self { r#where }
    }
}

impl MIRTransform for Peel {
    fn class_name(&self) -> &'static str {
        "Peel"
    }

    fn name(&self) -> &str {
        "peel"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        let found = _candidate(&body, &self.r#where, &BTreeSet::new())?;
        Ok(match found {
            None => body,
            Some(found) => found.0,
        })
    }
}

/// Whether a loop can multiply strict floating CFG regions when cloned.
fn _conditional_floating(loop_: &Loop, blocks: &BTreeMap<i64, &MirBlock>) -> bool {
    loop_
        .body
        .iter()
        .filter_map(|at| blocks.get(at))
        .any(|block| {
            block.at != loop_.header
                && block.succ.len() > 1
                && block.ops.iter().any(|op| op.floating.is_some())
        })
}

/// Clone the first bounded exact loop, returning body, latch and count.
fn _candidate(body: &MirBody, r#where: &Where, skip: &BTreeSet<i64>) -> Result<Option<(MirBody, i64, i64)>, String> {
    let closed = lcssa::closed(body)?;
    let facts = consts::known(&closed, Some(&r#where.dgroup), Some(&r#where.named()), None, None);
    for loop_ in loops::loops(&closed.blocks, Some(closed.entry)) {
        if loop_.latches.len() != 1 {
            continue;
        }
        let latch = *loop_.latches.first().expect("one latch");
        if skip.contains(&latch) {
            continue;
        }
        let Some(count) = induction::trip_count(&closed, &loop_, &facts) else {
            continue;
        };
        if count < BigInt::from(2) {
            continue;
        }
        // A resource ceiling, not a profitability claim.  The optimized
        // candidate is accepted below using the selected CPU's semantic
        // costs; this only bounds the quadratic scalar analyses on cloned CFG.
        let emitted = closed
            .blocks
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| block.ops.iter())
            .filter(|op| op.kind != Kind::Nothing)
            .count();
        let size = &count * BigInt::from(emitted);
        if size > BigInt::from(MAX_SPECULATIVE_OPERATIONS) {
            continue;
        }
        let blocks = closed
            .blocks
            .iter()
            .map(|block| (block.at, block))
            .collect::<BTreeMap<i64, &MirBlock>>();
        if _conditional_floating(&loop_, &blocks) && size > BigInt::from(MAX_CONDITIONAL_FLOAT_OPERATIONS) {
            continue;
        }
        let count = count.to_i64().expect("count fits");
        if let Some(candidate) = loopclone::peeled(&closed, &loop_, count)? {
            return Ok(Some((candidate, latch, count)));
        }
    }
    Ok(None)
}

/// Peel exact loops transactionally and retain only target-priced wins.
pub fn optimized(
    body: &MirBody,
    r#where: &Where,
    optimize: &mut dyn FnMut(MirBody) -> Result<MirBody, String>,
    mut watch: Option<&mut dyn FnMut(&str, &MirBody)>,
) -> Result<MirBody, String> {
    let mut body = body.clone();
    let mut rejected = BTreeSet::<i64>::new();
    loop {
        let Some((candidate, latch, count)) = _candidate(&body, r#where, &rejected)? else {
            return Ok(body);
        };
        let result = optimize(candidate.clone())?;
        if let Some(rejection) = unroll::_rejection(&body, &result, latch, count, r#where) {
            if let Some(watch) = watch.as_deref_mut() {
                watch(&format!("peel-rejected-{rejection}"), &result);
            }
            rejected.insert(latch);
            continue;
        }
        if let Some(watch) = watch.as_deref_mut() {
            watch("peel-candidate", &candidate);
            watch("peel-accepted", &result);
        }
        body = result;
        rejected.clear();
    }
}
