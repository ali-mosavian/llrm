//! The raise: `qbopt/model/mir.py` from `_RaisedOp` through `bodies`.
//!
//! Python keywords map to plain Rust: `detached(op, **changes)` is the caller
//! applying `changes` to a clone and handing it here.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::{Code, Register};

use super::{FLAGS, MirBlock, MirBody, Op, RaisedBody, Raising, TRACKED};
use crate::support::hash::IndexMap;
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

/// Registers as Python's frozensets of `Register_`: iteration is sorted,
/// which is `sorted(..., key=lambda o: (o is not FLAGS, o))` since FLAGS is 0.
pub type Registers = BTreeSet<Register>;

/// Python `_call_touches`: what a call disturbs and reads, where established.
pub fn call_touches(name: Option<&str>, routine: Option<&runtime::Contract>) -> Option<(Registers, Registers)> {
    let owned;
    let routine = match routine {
        Some(one) => one,
        None => {
            owned = runtime::contract(name);
            &owned
        }
    };
    if !routine.established && !runtime::established_inputs(routine) {
        return None;
    }
    let changed: Registers = runtime::disturbs(routine).into_iter().filter_map(from_contract).collect();
    let mut disturbed: Registers = TRACKED.into_iter().filter(|one| changed.contains(one)).collect();
    disturbed.insert(FLAGS);
    if !runtime::established_inputs(routine) {
        let mut every: Registers = TRACKED.into_iter().collect();
        every.insert(FLAGS);
        return Some((disturbed, every));
    }
    let direct = if routine.direct_inputs.is_none() { &routine.inputs } else { &routine.direct_inputs };
    let reads = direct.iter().flatten().copied().filter_map(from_contract).collect();
    Some((disturbed, reads))
}

/// Python `_restore_touches`: what the restore idiom really disturbs.
pub fn restore_touches(node: &Node) -> Option<(Registers, Registers)> {
    let Node::Restore(node) = node else {
        return None;
    };
    let (source, into) = super::restore_pair(node.pair as i64)?;
    Some((BTreeSet::from([into]), BTreeSet::from([source, into])))
}

/// Python `_touched`: (defines, uses) as tracked variables, flags as FLAGS.
pub fn touched(
    node: &Node,
    calls: Option<&IndexMap<i64, String>>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
) -> (Registers, Registers) {
    if matches!(node, Node::Opaque(opaque) if opaque.insn.insn.code() == Code::Into) {
        return (BTreeSet::new(), BTreeSet::from([FLAGS]));
    }
    if let (Some(calls), Node::Call(call)) = (calls, node) {
        let at = call.insn.at as i64;
        let known = call_touches(
            calls.get(&at).map(String::as_str),
            contracts.and_then(|contracts| contracts.get(&at)),
        );
        if let Some(known) = known {
            return known;
        }
    }
    if let Some(halves) = restore_touches(node) {
        return halves;
    }
    let effects = node.effects();
    let mut defines: Registers = match &effects.defs {
        None => TRACKED.into_iter().collect(),
        Some(defs) => defs.iter().copied().filter(|one| TRACKED.contains(one)).collect(),
    };
    let mut uses: Registers = match &effects.uses {
        None => TRACKED.into_iter().collect(),
        Some(used) => used.iter().copied().filter(|one| TRACKED.contains(one)).collect(),
    };
    if effects.defs.is_none() || !effects.flags_written.is_empty() {
        defines.insert(FLAGS);
    }
    if effects.uses.is_none() || !effects.flags_read.is_empty() {
        uses.insert(FLAGS);
    }
    (defines, uses)
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
