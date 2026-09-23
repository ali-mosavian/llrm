//! Port of `qbopt/frontend/raising_fields.py`: name bounded indexed fields
//! directly instead of retaining address arithmetic.

use std::collections::BTreeSet;

use crate::model::mir::{self, Arg, Cell, Held, Kind, MemRef, RaisedBody, Symbol, Value};
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::IndexMap;

pub fn named(body: RaisedBody) -> RaisedBody {
    let definitions: IndexMap<Value, &mir::Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect();

    fn parts(
        definitions: &IndexMap<Value, &mir::Op>,
        value: Value,
        seen: &BTreeSet<Value>,
    ) -> Option<(Symbol, Value, i64)> {
        if seen.contains(&value) {
            return None;
        }
        let op = definitions.get(&value)?;
        if op.results != [Arg::Held(Held { value, width: 2 })]
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || mir::partial(op)
        {
            return None;
        }
        let mut inner = seen.clone();
        inner.insert(value);
        if op.kind == Kind::Copy && op.args.len() == 1 {
            if let Arg::Held(source) = &op.args[0] {
                return if source.width == 2 { parts(definitions, source.value, &inner) } else { None };
            }
        }
        if op.kind != Kind::Add || op.args.len() != 2 {
            return None;
        }
        for (base, offset) in [(&op.args[0], &op.args[1]), (&op.args[1], &op.args[0])] {
            let Arg::Held(base) = base else {
                continue;
            };
            if base.width != 2 {
                continue;
            }
            if let Arg::Symbol(offset) = offset {
                if offset.space == Space::Segment && offset.width == 2 {
                    return Some((*offset, base.value, 0));
                }
            }
            if let Arg::Const(offset) = offset {
                if offset.width == 2 {
                    if let Some((symbol, index, displacement)) = parts(definitions, base.value, &inner) {
                        let low: i64 = i64::try_from(&offset.n & num_bigint::BigInt::from(65535)).expect("16 bits");
                        return Some((symbol, index, displacement + ((low ^ 32768) - 32768)));
                    }
                }
            }
        }
        None
    }

    let reference = |reference: &MemRef| -> MemRef {
        let Some(addr) = reference.addr else {
            return reference.clone();
        };
        let Some(base) = reference.base else {
            return reference.clone();
        };
        if addr.space != Space::Literal
            || reference.base_width != 2
            || reference.segment.is_some()
            || reference.excludes.is_empty()
        {
            return reference.clone();
        }
        let Some((symbol, index, displacement)) = parts(&definitions, base, &BTreeSet::new()) else {
            return reference.clone();
        };
        let displacement = displacement + symbol.offset + symbol.addend + addr.disp;
        if !(0 <= displacement && displacement <= 65536 - i64::from(reference.width)) {
            return reference.clone();
        }
        MemRef {
            addr: Some(Addr { index: symbol.index, base: addr.base, ..Addr::new(Space::Segment, displacement) }),
            base: Some(index),
            symbolic: None,
            ..reference.clone()
        }
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            if !matches!(op.kind, Kind::Load | Kind::Store) || op.barrier() || mir::partial(op) {
                ops.push(op.clone());
                continue;
            }
            let mut refs: IndexMap<MemRef, MemRef> = IndexMap::default();
            for one in op.loads.iter().chain(&op.stores) {
                refs.insert(one.clone(), reference(one));
            }
            if refs.iter().all(|(old, new)| old == new) {
                ops.push(op.clone());
                continue;
            }
            let arg = |one: &Arg| match one {
                Arg::Cell(cell) if refs.contains_key(&cell.r#ref) => Arg::Cell(Cell { r#ref: refs[&cell.r#ref].clone() }),
                _ => one.clone(),
            };
            let (args, results): (Vec<Arg>, Vec<Arg>) =
                (op.args.iter().map(arg).collect(), op.results.iter().map(arg).collect());
            let mut uses: Vec<Value> = Vec::new();
            let read = args
                .iter()
                .filter_map(|one| match one {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                })
                .chain(refs.values().flat_map(|one| [one.base, one.segment]).flatten());
            for value in read {
                if !uses.contains(&value) {
                    uses.push(value);
                }
            }
            let mut changed = op.clone();
            changed.args = args;
            changed.results = results;
            changed.uses = uses;
            changed.loads = op.loads.iter().map(|one| refs[one].clone()).collect();
            changed.stores = op.stores.iter().map(|one| refs[one].clone()).collect();
            changed.raised = None;
            changed.symbol = Some(!op.stores.is_empty());
            ops.push(mir::detached(changed));
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}
