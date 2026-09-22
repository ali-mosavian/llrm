//! Port of `qbopt/analysis/pointerfacts.py`: relative byte offsets of whole
//! MIR pointers; no pointer encoding is assumed.
//!
//! Only accesses already proven inside the same allocation may use relative
//! offsets to establish disjointness. Unrelated pointer values never suffice.
//! Facts are rebuilt from the current SSA, not attached to operands that a
//! later transformation could retarget.

use std::collections::BTreeSet;

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::model::mir::{self, Arg, Const, Held, Kind, MemRef, MirBody, Op, Value};

#[derive(Clone, Debug)]
pub struct Offsets<'a> {
    pub definitions: IndexMap<Value, &'a Op>,
}

impl Offsets<'_> {
    pub fn relative(&self, r#ref: &MemRef) -> Option<(Value, i64)> {
        if !r#ref.pointer
            || r#ref.base.is_none()
            || r#ref.base_width != 4
            || r#ref.addr.is_some()
            || r#ref.segment.is_some()
        {
            return None;
        }
        let (mut value, mut offset) = (r#ref.base?, 0_i64);
        let mut seen = BTreeSet::new();
        while !seen.contains(&value) {
            seen.insert(value);
            let op = match self.definitions.get(&value) {
                Some(op) => *op,
                None => return Some((value, offset)),
            };
            if op.results != [Arg::Held(Held { value, width: 4 })]
                || !op.merges.is_empty()
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
            {
                return Some((value, offset));
            }
            match (op.kind, &op.args[..]) {
                (Kind::Copy, [Arg::Held(Held { value: source, width: 4 })]) => {
                    value = *source;
                }
                (
                    Kind::PtrOffset,
                    [Arg::Held(Held { value: source, width: 4 }), Arg::Const(Const { n: amount, width: 4 })],
                ) => {
                    let low: BigInt = amount & BigInt::from(0xffff_ffff_u32);
                    let signed = i64::from(u32::try_from(low).expect("masked to 32 bits") as i32);
                    offset += signed;
                    if !(-0x8000_0000..=0x7fff_ffff).contains(&offset) {
                        return None;
                    }
                    value = *source;
                }
                _ => return Some((value, offset)),
            }
        }
        None
    }

    pub fn comparable(&self, one: &MemRef, other: &MemRef) -> Option<(i64, i64)> {
        if one.allocation.is_none() || one.allocation != other.allocation {
            return None;
        }
        let (left, right) = (self.relative(one), self.relative(other));
        let (Some(left), Some(right)) = (left, right) else {
            return None;
        };
        if left.0 != right.0 {
            return None;
        }
        Some((left.1, right.1))
    }

    pub fn disjoint(&self, one: &MemRef, other: &MemRef) -> bool {
        let offsets = self.comparable(one, other);
        let Some((left, right)) = offsets else {
            return false;
        };
        if one.width == 0 || other.width == 0 {
            return false;
        }
        left + i64::from(one.width) <= right || right + i64::from(other.width) <= left
    }

    pub fn same_bytes(&self, one: &MemRef, other: &MemRef) -> bool {
        if mir::same_bytes(one, other) {
            return true;
        }
        let offsets = self.comparable(one, other);
        offsets.is_some_and(|(left, right)| left == right) && one.width == other.width
    }
}

pub fn offsets(body: &MirBody) -> Offsets<'_> {
    Offsets {
        definitions: body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
            .collect(),
    }
}
