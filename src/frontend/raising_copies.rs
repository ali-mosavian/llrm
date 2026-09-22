//! Port of `qbopt/frontend/raising_copies.py`.
//!
//! Scalarize a string copy when its addresses and traversal are explicit.
//!
//! An unknown direction or selector remains unmodelled. Each element is loaded
//! before it is stored, including overlapping copies; pointer results preserve
//! their high halves. Nothing about those machine requirements reaches a pass.

use std::collections::{BTreeMap, BTreeSet};

use iced_x86::{Code, Mnemonic, OpKind, Register, RflagsBits};

use crate::analysis::flags::CLOBBERS;
use crate::analysis::ssa;
use crate::frontend::declen::Insn;
use crate::model::ir::Operation;
use crate::model::ir::nodes::Node;
use crate::model::mir::{self, Arg, Cell, Held, Kind, MemRef, Op, OpCode, OrderedMap, RaisedBody, Symbol, Value};
use crate::objectfile::module::{Addr, Group, Module, Space};
use crate::objectfile::omf;
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// (direction, same segment, data segment).
type State = (Option<i64>, bool, bool);

const UNKNOWN: State = (None, false, false);

fn _after(op: &Op, state: State, pushed_data: bool) -> (State, bool) {
    let (mut direction, mut same_segment, mut data_segment) = state;
    let node = op.node();
    let decoded: Option<&Insn> = node.and_then(|node| match &**node {
        Node::Opaque(one) => Some(&one.insn),
        Node::Long(one) => Some(&one.insn),
        Node::Call(one) => Some(&one.insn),
        Node::Restore(_) | Node::Data(_) => None,
    });
    let (Some(node), Some(decoded)) = (node, decoded) else {
        return (UNKNOWN, false);
    };
    let insn = &decoded.insn;
    if matches!(insn.mnemonic(), Mnemonic::Cld | Mnemonic::Std) {
        direction = Some(if insn.mnemonic() == Mnemonic::Cld { 1 } else { -1 });
    } else if CLOBBERS.contains(&decoded.flow()) || insn.rflags_modified() & RflagsBits::DF != 0 {
        direction = None;
    }
    let defs = &node.effects().defs;
    if defs.as_ref().is_none_or(|defs| defs.contains(&Register::DS)) {
        data_segment = false;
    }
    if insn.mnemonic() == Mnemonic::Pop && insn.op0_register() == Register::ES && pushed_data {
        same_segment = true;
    } else if defs.as_ref().is_none_or(|defs| defs.contains(&Register::DS) || defs.contains(&Register::ES)) {
        same_segment = false;
    }
    (
        (direction, same_segment, data_segment),
        insn.mnemonic() == Mnemonic::Push && insn.op0_register() == Register::DS,
    )
}

fn _entries(body: &RaisedBody) -> IndexMap<i64, State> {
    let mut predecessors: IndexMap<i64, Vec<i64>> = body.blocks.iter().map(|block| (block.at, Vec::new())).collect();
    for block in &body.blocks {
        for successor in &block.succ {
            if let Some(found) = predecessors.get_mut(successor) {
                found.push(block.at);
            }
        }
    }
    let mut entries: IndexMap<i64, State> = predecessors.keys().map(|at| (*at, UNKNOWN)).collect();
    let mut exits = entries.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &body.blocks {
            let mut incoming: Vec<State> = predecessors[&block.at].iter().map(|at| exits[at]).collect();
            if block.at == body.entry {
                incoming.push((None, false, true));
            }
            let mut state = match incoming.first() {
                Some(&(direction, same_segment, data_segment)) => (
                    if incoming.iter().all(|one| one.0 == direction) { direction } else { UNKNOWN.0 },
                    if incoming.iter().all(|one| one.1 == same_segment) { same_segment } else { UNKNOWN.1 },
                    if incoming.iter().all(|one| one.2 == data_segment) { data_segment } else { UNKNOWN.2 },
                ),
                None => UNKNOWN,
            };
            entries.insert(block.at, state);
            let mut pushed_data = false;
            for op in &block.ops {
                (state, pushed_data) = _after(op, state, pushed_data);
            }
            if exits[&block.at] != state {
                exits.insert(block.at, state);
                changed = true;
            }
        }
    }
    entries
}

pub fn scalar(body: RaisedBody, found: &Module) -> RaisedBody {
    let extents: HashMap<i64, i64> = omf::segments(&found.records)
        .into_iter()
        .enumerate()
        .filter_map(|(index, segment)| segment.map(|segment| (index as i64, segment.1)))
        .collect();
    let mut symbols: HashMap<Value, Symbol> = HashMap::default();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.kind == Kind::Copy && !op.barrier() && op.results.len() == 1 && op.args.len() == 1 {
            if let (Arg::Held(result), Arg::Symbol(symbol)) = (&op.results[0], &op.args[0]) {
                symbols.insert(result.value, *symbol);
            }
        }
    }
    let mut values: BTreeSet<Value> = body.origin.keys().copied().collect();
    values.extend(ssa::values(&body));
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let definitions: BTreeMap<Value, &Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect();
    let (mut candidates, mut ancestors): (HashSet<u32>, HashMap<Value, Value>) =
        (HashSet::default(), HashMap::default());
    let mut blocks = Vec::new();
    let entries = _entries(&body);
    for block in &body.blocks {
        let (mut direction, mut same_segment, mut data_segment) = entries[&block.at];
        let mut pushed_data = false;
        let mut ops = Vec::new();
        for op in &block.ops {
            let insn = op.node().and_then(|node| match &**node {
                Node::Opaque(one) => Some(&one.insn.insn),
                Node::Long(one) => Some(&one.insn.insn),
                Node::Call(one) => Some(&one.insn.insn),
                Node::Restore(_) | Node::Data(_) => None,
            });
            let Some(insn) = insn else {
                (direction, same_segment, pushed_data, data_segment) = (None, false, false, false);
                ops.push(op.clone());
                continue;
            };
            if insn.code() == Code::Movsw_m16_m16
                && insn.op1_kind() == OpKind::MemorySegSI
                && insn.memory_segment() == Register::DS
                && !insn.has_rep_prefix()
                && !insn.has_repne_prefix()
                && same_segment
                && data_segment
            {
                if let Some(direction) = direction {
                    let pointers = _pointers(op, &body.origin, &symbols, &found.dgroup, &extents, 2 * direction);
                    if let Some(pointers) = pointers {
                        serial += 1;
                        variable += 1;
                        let temporary = Value { id: serial, at: op.at, flags: false, variable, version: 1 };
                        let [source, dest] = [pointers[0].2, pointers[1].2].map(|symbol| {
                            let mut addr = Addr::new(symbol.space, symbol.offset);
                            addr.index = symbol.index;
                            MemRef::new(Some(addr), 2)
                        });
                        let held = Held { value: temporary, width: 2 };
                        let mut load = Op::new(op.at, OpCode::Operation(Operation::Move), "mov", vec![temporary], vec![]);
                        load.kind = Kind::Load;
                        load.loads = vec![source.clone()];
                        load.args = vec![Arg::Cell(Cell { r#ref: source })];
                        load.results = vec![Arg::Held(held)];
                        load.id = Some(mir::next_id());
                        ops.push(mir::raising_owned(load, &[op]));
                        let mut store = Op::new(op.at, OpCode::Operation(Operation::Move), "mov", vec![], vec![temporary]);
                        store.kind = Kind::Store;
                        store.stores = vec![dest.clone()];
                        store.args = vec![Arg::Held(held)];
                        store.results = vec![Arg::Cell(Cell { r#ref: dest })];
                        store.id = Some(mir::next_id());
                        ops.push(store);
                        for (before, after, symbol) in pointers {
                            let advanced = Symbol { offset: symbol.offset + 2 * direction, ..symbol };
                            symbols.insert(after, advanced);
                            if let Some(setup) = definitions.get(&before) {
                                if let Some(id) = setup.id {
                                    if setup.kind == Kind::Copy
                                        && setup.loads.is_empty()
                                        && setup.stores.is_empty()
                                        && setup.raising.as_ref().is_none_or(|raising| raising.extra_covers.is_empty())
                                        && setup.defines == [before]
                                        && setup.args.len() == 1
                                        && matches!(setup.args[0], Arg::Symbol(_))
                                    {
                                        candidates.insert(id);
                                    }
                                }
                            }
                            let before = ancestors.get(&before).copied().unwrap_or(before);
                            ancestors.insert(after, before);
                            let identity = mir::next_id();
                            candidates.insert(identity);
                            let mut copy =
                                Op::new(op.at, OpCode::Operation(Operation::Move), "mov", vec![after], vec![before]);
                            copy.kind = Kind::Copy;
                            copy.args = vec![Arg::Symbol(advanced)];
                            copy.results = vec![Arg::Held(Held { value: after, width: 2 })];
                            copy.merges = [(before, after)].into_iter().collect::<OrderedMap<_, _>>();
                            copy.id = Some(identity);
                            ops.push(copy);
                        }
                        pushed_data = false;
                        continue;
                    }
                }
            }
            let state;
            (state, pushed_data) = _after(op, (direction, same_segment, data_segment), pushed_data);
            (direction, same_segment, data_segment) = state;
            ops.push(op.clone());
        }
        blocks.push(block.with_ops(ops));
    }
    _observed(body.with_blocks(blocks), &candidates)
}

fn _observed(body: RaisedBody, candidates: &HashSet<u32>) -> RaisedBody {
    if candidates.is_empty() {
        return body;
    }
    let chosen = |op: &Op| op.id.is_some_and(|id| candidates.contains(&id));
    let definitions: HashMap<Value, &Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| chosen(op))
        .map(|op| (op.defines[0], op))
        .collect();
    let mut wanted: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values().copied()).collect();
    for block in &body.blocks {
        for op in &block.ops {
            if !chosen(op) {
                wanted.extend(op.uses.iter().copied());
                wanted.extend(op.args.iter().filter_map(|arg| match arg {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                }));
                wanted.extend(op.loads.iter().chain(&op.stores).flat_map(|r#ref| [r#ref.base, r#ref.segment]).flatten());
            }
        }
    }
    let mut pending: Vec<Value> = wanted.iter().copied().collect();
    while let Some(next) = pending.pop() {
        if let Some(op) = definitions.get(&next) {
            let unseen: BTreeSet<Value> = op.uses.iter().copied().filter(|value| !wanted.contains(value)).collect();
            wanted.extend(unseen.iter().copied());
            pending.extend(unseen);
        }
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            if chosen(op) && !wanted.contains(&op.defines[0]) {
                if !op.inserted() {
                    let mut nothing = Op::new(op.at, OpCode::Operation(Operation::Nothing), "", vec![], vec![]);
                    nothing.kind = Kind::Nothing;
                    nothing.id = op.id;
                    ops.push(mir::raising_owned(nothing, &[op]));
                }
            } else {
                ops.push(op.clone());
            }
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

fn _pointers(
    op: &Op,
    origin: &OrderedMap<Value, Register>,
    symbols: &HashMap<Value, Symbol>,
    dgroup: &Group,
    extents: &HashMap<i64, i64>,
    step: i64,
) -> Option<Vec<(Value, Value, Symbol)>> {
    let mut pointers = Vec::new();
    for register in [Register::ESI, Register::EDI] {
        let reads: Vec<Value> = op.uses.iter().copied().filter(|value| origin.get(value) == Some(&register)).collect();
        let writes: Vec<Value> =
            op.defines.iter().copied().filter(|value| origin.get(value) == Some(&register)).collect();
        if reads.len() != 1 || writes.len() != 1 {
            return None;
        }
        let symbol = symbols.get(&reads[0])?;
        let extent = extents.get(&symbol.index).copied().unwrap_or(0);
        if symbol.space != Space::Segment
            || !dgroup.contains(symbol.index)
            || symbol.width != 2
            || symbol.addend != 0
            || !(0 <= symbol.offset && symbol.offset <= extent - 2)
            || !(0 <= symbol.offset + step && symbol.offset + step < extent.min(0x10000))
        {
            return None;
        }
        pointers.push((reads[0], writes[0], *symbol));
    }
    Some(pointers)
}

#[cfg(test)]
#[path = "raising_copies_tests.rs"]
mod tests;
