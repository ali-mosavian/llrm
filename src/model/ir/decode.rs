//! Port of `qbopt/model/ir.py`: `emit` through `decode_module`.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::{Mnemonic, OpKind, Register};

use super::nodes::{Call, Data, Long, Node, Opaque, RESTORE_EFFECTS, Restore, TableKind, span};
use super::semantics::{instruction_effects, instruction_semantics};
use super::{Imm, Loc, NO_EFFECT, Operation, Reg, Semantics};
use crate::abi::machine;
use crate::frontends::bc::blocks::{Block, CodeMap, INLINE_TABLE, code_map, partition as block_partition};
use crate::frontends::bc::declen::Insn;
use crate::frontends::bc::extent::{Body, Partition, partition as body_partition};
use crate::legacy::lift::{FIXUP, classify_with as classify};
use crate::objectfile::module::Module;
use crate::support::hash::IndexMap;

/// A node list's own bytes, verbatim -- sliced from the original code by each
/// node's own span.
pub fn emit(module: &Module, nodes: &[Arc<Node>]) -> Vec<u8> {
    nodes
        .iter()
        .flat_map(|node| {
            let (lo, hi) = span(node);
            module.code[lo..hi].iter().copied()
        })
        .collect()
}

/// A restore idiom starting exactly at `at`, or `None`.
pub fn _restore_at(module: &Module, insns_by_at: &IndexMap<usize, &Insn>, at: usize, hi: usize) -> Option<Restore> {
    if at + 4 > hi {
        return None;
    }
    for (&pair, pattern) in FIXUP.iter() {
        if module.code.get(at..at + 4) != Some(&pattern[..]) {
            continue;
        }
        let (second, third) = (insns_by_at.get(&(at + 2)), insns_by_at.get(&(at + 3)));
        let (Some(second), Some(third)) = (second, third) else {
            continue;
        };
        if second.end() != at + 3 || third.end() != at + 4 {
            continue;
        }
        return Some(Restore::new(at, at + 4, pair, RESTORE_EFFECTS[&pair].clone()));
    }
    None
}

pub fn _table_node(module: &Module, last: Option<&Node>, lo: usize, hi: usize) -> Data {
    let mut kind = TableKind::Map;
    if let Some(Node::Call(last)) = last {
        if last.insn.end() == lo && INLINE_TABLE.contains(last.name.as_str()) {
            kind = TableKind::Jump;
        }
    }
    let mut entries: Vec<usize> =
        module.operands.keys().filter(|&&at| lo as i64 <= at && at < hi as i64).map(|&at| at as usize).collect();
    entries.sort_unstable();
    Data::new(lo, hi, kind, entries, NO_EFFECT.clone())
}

pub fn _instruction_node(module: &Module, insn: &Insn) -> Node {
    let resolve = |field_offset: i64, literal: i64| module.resolve(field_offset, literal);
    let effects = instruction_effects(insn, &resolve);
    let semantics = instruction_semantics(insn, &resolve);
    if let Some(name) = module.calls.get(&(insn.at as i64)) {
        return Node::Call(Call { insn: insn.clone(), name: name.clone(), effects, semantics });
    }
    if let Some(decoded) = classify(insn, &resolve) {
        return Node::Long(Long { insn: insn.clone(), decoded, effects, semantics });
    }
    Node::Opaque(Opaque { insn: insn.clone(), effects, semantics })
}

/// Every byte of `body`'s own ranges, as ordered Nodes.
pub fn decode_body(module: &Module, mapped: &CodeMap, blocks: &[Block], body: &Body) -> Vec<Arc<Node>> {
    let insns_by_at: IndexMap<usize, &Insn> =
        blocks.iter().flat_map(|block| block.insns.iter()).map(|insn| (insn.at, insn)).collect();
    let tables_by_start: IndexMap<usize, usize> = mapped.tables.iter().copied().collect();

    let mut nodes: Vec<Arc<Node>> = Vec::new();
    let mut last: Option<Arc<Node>> = None;
    for &(lo, hi) in &body.ranges {
        let mut at = lo;
        while at < hi {
            let node = if let Some(&end) = tables_by_start.get(&at) {
                Node::Data(_table_node(module, last.as_deref(), at, end))
            } else if let Some(restore) = _restore_at(module, &insns_by_at, at, hi) {
                Node::Restore(restore)
            } else {
                _instruction_node(module, insns_by_at[&at])
            };
            let node = Arc::new(node);
            nodes.push(Arc::clone(&node));
            at = span(&node).1;
            last = Some(node);
        }
    }
    _at_devices(&nodes, &blocks.iter().map(|block| block.at).collect())
}

/// Each `in` and `out` whose port is a literal, at its device's memory reach.
pub fn _at_devices(nodes: &[Arc<Node>], starts: &BTreeSet<usize>) -> Vec<Arc<Node>> {
    let mut out: Vec<Arc<Node>> = nodes.to_vec();
    for (index, node) in nodes.iter().enumerate() {
        let Node::Opaque(opaque) = node.as_ref() else {
            continue;
        };
        let insn = &opaque.insn.insn;
        if !matches!(insn.mnemonic(), Mnemonic::In | Mnemonic::Out) {
            continue;
        }
        let operand = if insn.mnemonic() == Mnemonic::Out { 0 } else { 1 };
        let port = if insn.op_kind(operand) == OpKind::Immediate8 {
            Some(i64::from(insn.immediate8()))
        } else if insn.op_kind(operand) == OpKind::Register && insn.op_register(operand) == Register::DX {
            _literal_dx(nodes, index, starts)
        } else {
            continue;
        };
        if port.is_some_and(|port| machine::current().silent_port(port)) {
            let mut replaced = opaque.clone();
            replaced.effects.loads = Vec::new();
            replaced.effects.stores = Vec::new();
            replaced.effects.memory_complete = true;
            out[index] = Arc::new(Node::Opaque(replaced));
        }
    }
    out
}

/// The literal dx holds before `nodes[index]`, where its block says so.
pub fn _literal_dx(nodes: &[Arc<Node>], mut index: usize, starts: &BTreeSet<usize>) -> Option<i64> {
    while index > 0 && !starts.contains(&span(&nodes[index]).0) {
        let previous = &nodes[index - 1];
        let defs = previous.effects().defs.as_ref();
        if span(previous).1 != span(&nodes[index]).0 || defs.is_none() {
            return None;
        }
        if defs.is_some_and(|defs| defs.contains(&Register::EDX)) {
            let Semantics { op, dests, sources, .. } = previous.semantics();
            return match (op, dests.as_slice(), sources.as_slice()) {
                (
                    Operation::Move,
                    [Loc::Reg(Reg { register: Register::DX | Register::EDX, .. })],
                    [Loc::Imm(Imm { value, address: None, .. })],
                ) => Some(value & 0xFFFF),
                _ => None,
            };
        }
        index -= 1;
    }
    None
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyIR {
    pub body: Body,
    pub nodes: Vec<Arc<Node>>,
}

/// Every body of `module`, total-decoded -- or why it could not be.
pub fn decode_module(module: &Module) -> Result<Vec<BodyIR>, String> {
    let mapped = code_map(module)?;
    let found = body_partition(module)?;
    if !found.complete() {
        return Err(_incomplete(&found));
    }
    let blocks = block_partition(module, &mapped);
    Ok(found
        .bodies
        .iter()
        .map(|body| BodyIR { body: body.clone(), nodes: decode_body(module, &mapped, &blocks, body) })
        .collect())
}

pub fn _incomplete(found: &Partition) -> String {
    format!("{} unexplained range(s), {} conflicting", found.unexplained.len(), found.conflicts.len())
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod tests;
