//! Port of `qbopt/frontend/raising_literals.py`: expose explicit BC literal
//! initializer bytes at module entry.
//!
//! BC_CN also contains descriptors and relocatable data. Only direct scalar
//! reads backed by complete, nonoverlapping, unrelocated bytes are admitted.
//! These are initial memory values, not immutable loads; subsequent writes and
//! calls still kill them. Procedure entries do not inherit loader state.

use std::collections::{BTreeSet, HashMap};

use num_bigint::BigInt;

use crate::abi::runtime::{self, Contract, Memory};
use crate::frontend::blocks::ENTRY;
use crate::model::mir::{self, Const, Kind, MemRef, Op, RaisedBody};
use crate::objectfile::module::{self, Addr, Module, Space};
use crate::objectfile::omf::{self, Fixup};
use crate::support::hash::IndexMap;

type Data = HashMap<(i64, i64), u8>;

pub fn initialized(
    body: RaisedBody,
    found: &Module,
    contracts: Option<&IndexMap<i64, Contract>>,
) -> Result<RaisedBody, String> {
    if body.entry != ENTRY as i64 {
        return Ok(body);
    }
    let segments = omf::segments(&found.records);
    let named = |segment: &str| -> Result<BTreeSet<i64>, String> {
        let mut out = BTreeSet::new();
        for (index, one) in segments.iter().enumerate() {
            if let Some((name, _)) = one {
                if name == segment && omf::pubdef_names(&found.records, index as i64).map_err(|e| e.0)?.is_empty() {
                    out.insert(index as i64);
                }
            }
        }
        Ok(out)
    };
    let pools = named("BC_CN")?;
    if pools.is_empty() {
        return Ok(body);
    }
    let far_strings = named("FSL_CONST")?;
    let readable: BTreeSet<i64> = pools.union(&far_strings).copied().collect();
    let fixups = omf::fixups(&found.records);
    let (mut data, mut ambiguous): (Data, BTreeSet<(i64, i64)>) = (HashMap::new(), BTreeSet::new());
    let mut unknown = BTreeSet::new();
    for fixup in &fixups {
        let Some(seg) = fixup.seg.filter(|seg| readable.contains(seg)) else {
            continue;
        };
        let width = match fixup.loc {
            omf::LOC_OFF16 | omf::LOC_BASE => 2,
            omf::LOC_PTR32 => 4,
            _ => {
                unknown.insert(seg);
                continue;
            }
        };
        ambiguous.extend((0..width).map(|byte| (seg, fixup.offset + byte)));
    }
    for (_, index, start, payload) in omf::ledata(&found.records) {
        if !readable.contains(&index) || unknown.contains(&index) {
            continue;
        }
        for (offset, &byte) in (start..).zip(&payload) {
            let key = (index, offset);
            if data.contains_key(&key) {
                ambiguous.insert(key);
            }
            data.insert(key, byte);
        }
    }
    let mut values: IndexMap<MemRef, Const> = IndexMap::default();
    for block in &body.blocks {
        for op in &block.ops {
            for reference in &op.loads {
                let reference = mir::symbolic_ref(reference);
                let Some(addr) = reference.addr else {
                    continue;
                };
                if addr.space != Space::Segment
                    || !pools.contains(&addr.index)
                    || reference.base.is_some()
                    || reference.segment.is_some()
                    || ![1, 2, 4, 8].contains(&reference.width)
                {
                    continue;
                }
                let keys: Vec<(i64, i64)> =
                    (addr.disp..addr.disp + i64::from(reference.width)).map(|offset| (addr.index, offset)).collect();
                if keys.iter().all(|key| data.contains_key(key) && !ambiguous.contains(key)) {
                    let bytes: Vec<u8> = keys.iter().map(|key| data[key]).collect();
                    values.insert(
                        reference.clone().into_owned(),
                        Const::new(BigInt::from_bytes_le(num_bigint::Sign::Plus, &bytes), reference.width),
                    );
                }
            }
        }
    }
    if values.is_empty() {
        return Ok(body);
    }
    let protected = _numeric_ranges(&values, &data, &ambiguous, &fixups, &module::escaped(found), &far_strings);
    let owned;
    let selected = match contracts {
        Some(one) => one,
        None => {
            owned = runtime::for_module(found, None).map_err(|e| e.0)?;
            &owned
        }
    };
    let handles_errors = runtime::handles_errors(selected.values());

    let annotate = |op: &Op| -> Op {
        if op.kind != Kind::Call || protected.is_empty() {
            return op.clone();
        }
        let Some(contract) = selected.get(&op.at) else {
            return op.clone();
        };
        if runtime::barrier(contract) || contract.writes != Memory::Own || (contract.raises_error && handles_errors) {
            return op.clone();
        }
        let mut made = op.clone();
        for reference in &mut made.stores {
            if reference.addr.is_none() {
                let mut excludes = Vec::new();
                for excluded in reference.excludes.iter().chain(&protected) {
                    if !excludes.contains(excluded) {
                        excludes.push(*excluded);
                    }
                }
                reference.excludes = excludes;
            }
        }
        made
    };

    let blocks = body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(annotate).collect())).collect();
    let mut made = body.with_blocks(blocks);
    made.body_mut().initial = values.into_iter().collect();
    Ok(made)
}

/// Separate scalar literals from validated escaped string descriptors and their payloads.
///
/// OWN runtime effects may compact strings, not write arbitrary numeric
/// objects. Unknown escaped layouts prevent any exclusion for their pool.
/// Explicit program writes and unknown calls still invalidate entry facts.
fn _numeric_ranges(
    values: &IndexMap<MemRef, Const>,
    data: &Data,
    ambiguous: &BTreeSet<(i64, i64)>,
    fixups: &[Fixup],
    escaped: &BTreeSet<(i64, i64)>,
    far_strings: &BTreeSet<i64>,
) -> Vec<(Addr, u32)> {
    let mut protected = Vec::new();
    'values: for reference in values.keys() {
        let addr = reference.addr.expect("a literal has an address");
        let index = addr.index;
        let mut occupied = BTreeSet::new();
        for &(segment, offset) in escaped {
            if segment != index {
                continue;
            }
            if let Some(far) = _far_descriptor(index, offset, data, ambiguous, fixups, far_strings) {
                occupied.extend(far);
                continue;
            }
            let pointer = _relocation(index, offset + 2, omf::LOC_OFF16, data, fixups);
            let Some(pointer) = pointer.filter(|pointer| pointer.index == index) else {
                continue 'values;
            };
            if [(index, offset), (index, offset + 1)].iter().any(|key| !data.contains_key(key) || ambiguous.contains(key)) {
                continue 'values;
            }
            let length = i64::from(data[&(index, offset)]) | i64::from(data[&(index, offset + 1)]) << 8;
            let target = pointer.disp;
            // The descriptor pointer itself is relocated; any other relocation
            // or missing payload byte makes this an unknown object layout.
            let payload = target..target + length;
            if payload.clone().any(|byte| !data.contains_key(&(index, byte)) || ambiguous.contains(&(index, byte))) {
                continue 'values;
            }
            occupied.extend(offset..offset + 4);
            occupied.extend(payload);
        }
        if !(addr.disp..addr.disp + i64::from(reference.width)).any(|byte| occupied.contains(&byte)) {
            protected.push((addr, reference.width));
        }
    }
    protected
}

/// VBDOS's two-word indirect literal descriptor, validated through its relocations.
fn _far_descriptor(
    index: i64,
    offset: i64,
    data: &Data,
    ambiguous: &BTreeSet<(i64, i64)>,
    fixups: &[Fixup],
    far_strings: &BTreeSet<i64>,
) -> Option<Vec<i64>> {
    let relocation = |segment: i64, at: i64, kind: i64| _relocation(segment, at, kind, data, fixups);

    let pointer = relocation(index, offset, omf::LOC_OFF16)?;
    let selector = relocation(index, offset + 2, omf::LOC_OFF16)?;
    if !far_strings.contains(&pointer.index) || selector.index != index {
        return None;
    }
    let base = relocation(index, selector.disp, omf::LOC_BASE)?;
    let indirect = relocation(pointer.index, pointer.disp, omf::LOC_OFF16)?;
    if base.index != pointer.index
        || base.disp != 0
        || indirect.index != pointer.index
        || indirect.disp != pointer.disp + 2
    {
        return None;
    }
    let length_at = indirect.disp;
    let length_bytes = [(pointer.index, length_at), (pointer.index, length_at + 1)];
    if length_bytes.iter().any(|key| !data.contains_key(key) || ambiguous.contains(key)) {
        return None;
    }
    let length = i64::from(u16::from_le_bytes([data[&length_bytes[0]], data[&length_bytes[1]]]));
    if (length_at + 2..length_at + 2 + length)
        .any(|byte| !data.contains_key(&(pointer.index, byte)) || ambiguous.contains(&(pointer.index, byte)))
    {
        return None;
    }
    Some((offset..offset + 4).chain(selector.disp..selector.disp + 2).collect())
}

fn _relocation<'a>(segment: i64, at: i64, kind: i64, data: &Data, fixups: &'a [Fixup]) -> Option<&'a Fixup> {
    let widths = |loc: i64| match loc {
        omf::LOC_OFF16 | omf::LOC_BASE => 2,
        omf::LOC_PTR32 => 4,
        _ => 4,
    };
    let fields: Vec<&Fixup> = fixups
        .iter()
        .filter(|fixup| fixup.seg == Some(segment) && fixup.offset < at + 2 && at < fixup.offset + widths(fixup.loc))
        .collect();
    if fields.len() != 1
        || fields[0].offset != at
        || fields[0].loc != kind
        || fields[0].target != "segment"
        || (0..2).any(|byte| data.get(&(segment, at + byte)) != Some(&0))
    {
        return None;
    }
    Some(fields[0])
}

#[cfg(test)]
#[path = "raising_literals_tests.rs"]
mod tests;
