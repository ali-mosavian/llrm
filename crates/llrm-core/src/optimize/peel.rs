//! Exact CFG loop peeling, priced before anything is cloned.
//!
//! Port of `qbopt/optimize/peel.py`.  Peeling is the general CFG counterpart
//! of full straight-line unrolling: clone every block of a proven exact loop,
//! retain the residual loop as a correctness fallback, and let the normal
//! fixed point prove the residual unreachable.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::analysis::loops::{self, Loop};
use crate::analysis::peelsize;
use crate::analysis::{consts, induction};
use crate::model::mir::{Kind, MirBlock, MirBody};
use crate::model::passes::{MIRTransform, Where};
use crate::optimize::{lcssa, loopclone, unroll};

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

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
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

/// Clone the first bounded exact loop `peelsize::admitted` prices as worth it, returning
/// body, latch and count.
fn _candidate(body: &Rc<MirBody>, r#where: &Where, skip: &BTreeSet<i64>) -> Result<Option<(Rc<MirBody>, i64, i64)>, String> {
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
        if !peelsize::admitted(&closed, &loop_, &count, &facts, r#where) {
            continue;
        }
        let emitted = closed
            .blocks
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| block.ops.iter())
            .filter(|op| op.kind != Kind::Nothing)
            .count();
        let size = &count * BigInt::from(emitted);
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
            return Ok(Some((Rc::new(candidate), latch, count)));
        }
    }
    Ok(None)
}

/// Peel every exact loop `peelsize::admitted` prices as worth it, once each; the
/// caller's fixed point settles the copies and proves each residual loop dead.
pub fn optimized(
    body: &Rc<MirBody>,
    r#where: &Where,
    mut watch: Option<&mut dyn FnMut(&str, &MirBody)>,
) -> Result<Rc<MirBody>, String> {
    if !unroll::priced(body, r#where) {
        return Ok(body.clone());
    }
    let mut body = body.clone();
    let mut peeled = BTreeSet::<i64>::new();
    while let Some((candidate, latch, _)) = _candidate(&body, r#where, &peeled)? {
        let candidate = crate::model::mir::identified(candidate);
        if let Some(watch) = watch.as_deref_mut() {
            watch("peel-accepted", &candidate);
        }
        llrm_support::debug!("peel", "peeled the loop with latch b{latch}");
        peeled.insert(latch);
        body = candidate;
    }
    Ok(body)
}
