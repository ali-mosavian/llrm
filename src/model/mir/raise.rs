//! The raise: `qbopt/model/mir.py` from `_RaisedOp` through `bodies`.
//!
//! Python keywords map to plain Rust: `detached(op, **changes)` is the caller
//! applying `changes` to a clone and handing it here.

use std::sync::Arc;

use iced_x86::Register;

use super::{MirBlock, MirBody, Op, RaisedBody, Raising};
use crate::abi::runtime;
use crate::model::ir::nodes::Node;
use crate::model::ir::root;

/// Python `PHYSICAL`: the frame, the stack and the segment registers.
pub const PHYSICAL: [Register; 8] = [
    Register::SP,
    Register::ESP,
    Register::BP,
    Register::EBP,
    Register::DS,
    Register::ES,
    Register::SS,
    Register::CS,
];

/// Python `FROM_CONTRACT[one]`: a contract register's root, or `FLAGS`.
pub fn from_contract(one: runtime::Reg) -> Option<Register> {
    super::as_named(one).map(root)
}

/// Python `detached`: a rewritten raising operation without its decoded node.
pub fn detached(mut operation: Op) -> Op {
    operation.source_backed = false;
    if let Some(raising) = &mut operation.raising {
        raising.node = None;
    }
    operation
}

/// Python `source_free`: a raising rewrite that owns no input occurrence.
pub fn source_free(mut operation: Op) -> Op {
    operation.raising = None;
    operation.source_backed = false;
    operation.absorbed = Vec::new();
    operation
}

/// Python `raising_occurrence`: raw ownership on an operation still inside the raise.
pub fn raising_occurrence(
    operation: &Op,
    covers: (i64, i64),
    extra: Vec<(i64, i64)>,
    node: Option<Arc<Node>>,
) -> Op {
    let mut made = operation.clone();
    made.raising = Some(Box::new(Raising { node, covers: Some(covers), extra_covers: extra }));
    made
}

/// Python `_raising_ranges`: concrete ownership while recognition is inside the raise.
pub fn raising_ranges(op: &Op) -> Vec<(i64, i64)> {
    let Some(raising) = &op.raising else {
        return Vec::new();
    };
    raising.covers.into_iter().chain(raising.extra_covers.iter().copied()).collect()
}

/// Python `raising_owned`: the exact occurrences of `owners`, merged.
pub fn raising_owned(operation: Op, owners: &[&Op]) -> Op {
    let mut ranges: Vec<(i64, i64)> =
        owners.iter().flat_map(|owner| raising_ranges(owner)).filter(|span| span.0 < span.1).collect();
    ranges.sort();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (low, high) in ranges {
        match merged.last_mut() {
            Some(last) if low <= last.1 => last.1 = last.1.max(high),
            _ => merged.push((low, high)),
        }
    }
    if merged.is_empty() {
        return operation;
    }
    let node = operation.node().cloned();
    let mut made = operation;
    made.raising = Some(Box::new(Raising { node, covers: Some(merged[0]), extra_covers: merged[1..].to_vec() }));
    made
}

/// Python `raising_adjacent`: two single source occurrences that touch.
pub fn raising_adjacent(first: &Op, second: &Op) -> bool {
    let (before, after) = (raising_ranges(first), raising_ranges(second));
    before.len() == 1 && after.len() == 1 && before[0].1 == after[0].0
}

impl RaisedBody {
    /// Python `replace(body, blocks=blocks)` on a `_RaisedBody`.
    pub fn with_blocks(&self, blocks: Vec<MirBlock>) -> RaisedBody {
        RaisedBody { body: self.body.with_blocks(blocks), origin: self.origin.clone(), pins: self.pins.clone() }
    }

    /// Python `replace(body, ...)` for fields other than blocks.
    pub fn body_mut(&mut self) -> &mut MirBody {
        &mut self.body
    }
}
