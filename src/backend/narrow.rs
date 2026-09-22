//! Port of `qbopt/backend/narrow.py`.
//!
//! Load narrowing: a wide load read only through its word halves is those words loaded.
//!
//! LLVM's DAGCombiner does this for `trunc (load)` and `srl (load)`. It is
//! selection, not optimization: the MIR already says only the halves are read,
//! and a word is this target's natural load. Extracting the high word of a dword
//! register otherwise costs a push and two pops.

use std::collections::BTreeSet;
use crate::support::hash::HashMap;

use crate::support::hash::IndexMap;

use crate::model::mir::{self, Arg, Kind, MirBlock, MirBody, Op, Value};

const _WORD: u32 = 2;

// extract bit offset -> byte offset
fn _halves(bit: i64) -> Option<i64> {
    match bit {
        0 => Some(0),
        16 => Some(i64::from(_WORD)),
        _ => None,
    }
}

// Python's `id(op)`: an operation's position in the body.
type Id = (usize, usize);

/// `body` with every narrowable dword load split into its read words.
pub fn narrowed(body: &MirBody) -> MirBody {
    let mut readers: HashMap<Value, Vec<Id>> = HashMap::default();
    for (b, block) in body.blocks.iter().enumerate() {
        for (o, op) in block.ops.iter().enumerate() {
            for value in &op.uses {
                readers.entry(*value).or_default().push((b, o));
            }
        }
    }
    let phis: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values().copied()).collect();
    let op_at = |(b, o): Id| &body.blocks[b].ops[o];
    let mut halves: IndexMap<Id, Vec<(i64, Value)>> = IndexMap::default();
    let mut gone: BTreeSet<Id> = BTreeSet::new();
    for (b, block) in body.blocks.iter().enumerate() {
        for (o, op) in block.ops.iter().enumerate() {
            let Some(value) = _loaded(op) else {
                continue;
            };
            if phis.contains(&value) {
                continue;
            }
            let reading = readers.get(&value).cloned().unwrap_or_default();
            let offsets: Vec<Option<i64>> = reading.iter().map(|one| _half(op_at(*one), value)).collect();
            if reading.is_empty() || offsets.contains(&None) {
                continue;
            }
            halves.insert(
                (b, o),
                offsets
                    .iter()
                    .zip(&reading)
                    .map(|(offset, one)| (offset.expect("checked"), _result(op_at(*one))))
                    .collect(),
            );
            gone.extend(reading);
        }
    }
    if halves.is_empty() {
        return body.clone();
    }
    MirBody {
        blocks: body
            .blocks
            .iter()
            .enumerate()
            .map(|(b, block)| MirBlock {
                ops: block
                    .ops
                    .iter()
                    .enumerate()
                    .flat_map(|(o, op)| _rewritten(op, (b, o), &halves, &gone))
                    .collect(),
                ..block.clone()
            })
            .collect(),
        ..body.clone()
    }
}

fn _result(op: &Op) -> Value {
    match &op.results[0] {
        Arg::Held(held) => held.value,
        _ => unreachable!("_half matched a held result"),
    }
}

/// The dword a plain load of one exactly addressed cell defines.
fn _loaded(op: &Op) -> Option<Value> {
    if op.kind != Kind::Load {
        return None;
    }
    match (op.args.as_slice(), op.results.as_slice()) {
        ([Arg::Cell(mir::Cell { r#ref })], [Arg::Held(mir::Held { value, width: 4 })]) => {
            // A source-backed load's bytes belong to the source-map emitter.
            if r#ref.width == 4
                && r#ref.addr.is_some()
                && !r#ref.volatile
                && op.loads == [r#ref.clone()]
                && !op.source_backed
            {
                return Some(*value);
            }
            None
        }
        _ => None,
    }
}

/// The byte offset of the word this operation extracts from `value`.
fn _half(op: &Op, value: Value) -> Option<i64> {
    if op.kind != Kind::Extract {
        return None;
    }
    match (op.args.as_slice(), op.results.as_slice()) {
        ([Arg::Held(mir::Held { value: source, .. }), Arg::Const(mir::Const { n: bit, .. })], [Arg::Held(mir::Held { width: 2, .. })]) => {
            let offset = i64::try_from(bit).ok().and_then(_halves);
            if *source == value && offset.is_some() && op.uses == [value] {
                return offset;
            }
            None
        }
        _ => None,
    }
}

fn _rewritten(op: &Op, id: Id, halves: &IndexMap<Id, Vec<(i64, Value)>>, gone: &BTreeSet<Id>) -> Vec<Op> {
    if gone.contains(&id) {
        return Vec::new();
    }
    let Some(found) = halves.get(&id) else {
        return vec![op.clone()];
    };
    let [r#ref] = op.loads.as_slice() else {
        unreachable!("_loaded matched one load")
    };
    let mut made: HashMap<i64, Value> = HashMap::default();
    let mut out = Vec::new();
    let mut sorted = found.clone();
    sorted.sort_by_key(|one| one.0);
    for (offset, result) in sorted {
        if let Some(earlier) = made.get(&offset) {
            out.push(Op {
                kind: Kind::Copy,
                name: String::new(),
                defines: vec![result],
                uses: vec![*earlier],
                args: vec![Arg::Held(mir::Held { value: *earlier, width: _WORD })],
                results: vec![Arg::Held(mir::Held { value: result, width: _WORD })],
                loads: Vec::new(),
                ..op.clone()
            });
            continue;
        }
        made.insert(offset, result);
        let word = mir::MemRef {
            addr: r#ref.addr.map(|addr| addr.plus(offset)),
            width: _WORD,
            ..r#ref.clone()
        };
        out.push(Op {
            defines: vec![result],
            args: vec![Arg::Cell(mir::Cell { r#ref: word.clone() })],
            results: vec![Arg::Held(mir::Held { value: result, width: _WORD })],
            loads: vec![word],
            ..op.clone()
        });
    }
    out
}
