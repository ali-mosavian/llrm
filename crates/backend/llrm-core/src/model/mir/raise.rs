//! The raise: `qbopt/model/mir.py` from `_RaisedOp` through `bodies`.
//!
//! Python keywords map to plain Rust: `detached(op, **changes)` is the caller
//! applying `changes` to a clone and handing it here.

use std::collections::BTreeSet;

use iced_x86::{Code, Register};

use super::{
    FLAGS, TRACKED,
};
use crate::abi::runtime;

use crate::model::ir::nodes::Node;
use crate::model::ir::root;
use crate::support::hash::IndexMap;

/// Python `FROM_CONTRACT[one]`: a contract register's root, or `FLAGS`.
pub fn from_contract(one: runtime::Reg) -> Option<Register> {
    super::as_named(one).map(root)
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
