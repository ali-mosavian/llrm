//! Port of `qbopt/analysis/pointerfacts.py`.

// ---- early port (agent B) ----

use std::collections::BTreeSet;

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::model::mir::{self, Arg, Held, Kind, MemRef, MirBody, Op, Value};

/// Direct port of `pointerfacts.Offsets`.
#[derive(Clone, Debug)]
pub(crate) struct Offsets<'a> {
    pub definitions: IndexMap<Value, &'a Op>,
}

impl Offsets<'_> {
    pub(crate) fn relative(&self, reference: &MemRef) -> Option<(Value, i64)> {
        if !reference.pointer
            || reference.base.is_none()
            || reference.base_width != 4
            || reference.addr.is_some()
            || reference.segment.is_some()
        {
            return None;
        }
        let (mut value, mut offset) = (reference.base?, 0_i64);
        let mut seen = BTreeSet::new();
        while !seen.contains(&value) {
            seen.insert(value);
            let op = self.definitions.get(&value);
            let Some(op) = op.filter(|op| {
                op.results == [Arg::Held(Held { value, width: 4 })]
                    && op.merges.is_empty()
                    && op.loads.is_empty()
                    && op.stores.is_empty()
                    && !op.barrier()
            }) else {
                return Some((value, offset));
            };
            match (op.kind, op.args.as_slice()) {
                (Kind::Copy, [Arg::Held(Held { value: source, width: 4 })]) => value = *source,
                (Kind::PtrOffset, [Arg::Held(Held { value: source, width: 4 }), Arg::Const(amount)])
                    if amount.width == 4 =>
                {
                    let mask = BigInt::from(0xffff_ffff_u32);
                    let sign = BigInt::from(0x8000_0000_u32);
                    let signed = ((&amount.n & mask) ^ &sign) - sign;
                    let total = BigInt::from(offset) + signed;
                    if !(BigInt::from(-0x8000_0000_i64) <= total && total <= BigInt::from(0x7fff_ffff_i64)) {
                        return None;
                    }
                    offset = i64::try_from(total).expect("a checked 32-bit offset");
                    value = *source;
                }
                _ => return Some((value, offset)),
            }
        }
        None
    }

    pub(crate) fn comparable(&self, one: &MemRef, other: &MemRef) -> Option<(i64, i64)> {
        if one.allocation.is_none() || one.allocation != other.allocation {
            return None;
        }
        let (left, right) = (self.relative(one)?, self.relative(other)?);
        if left.0 != right.0 {
            return None;
        }
        Some((left.1, right.1))
    }

    pub(crate) fn disjoint(&self, one: &MemRef, other: &MemRef) -> bool {
        let offsets = self.comparable(one, other);
        let Some((left, right)) = offsets.filter(|_| one.width > 0 && other.width > 0) else {
            return false;
        };
        left + i64::from(one.width) <= right || right + i64::from(other.width) <= left
    }

    pub(crate) fn same_bytes(&self, one: &MemRef, other: &MemRef) -> bool {
        if mir::same_bytes(one, other) {
            return true;
        }
        self.comparable(one, other)
            .is_some_and(|offsets| offsets.0 == offsets.1 && one.width == other.width)
    }
}

pub(crate) fn offsets(body: &MirBody) -> Offsets<'_> {
    Offsets {
        definitions: body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter())
            .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
            .collect(),
    }
}
