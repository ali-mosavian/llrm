//! Non-wrapping integer intervals, scoped to the taken body of a counted loop.
//!
//! Port of `qbopt/analysis/ranges.py`.

use std::borrow::Cow;
use std::collections::BTreeMap;

use num_bigint::BigInt;

use crate::model::mir::{MemRef, Value, symbolic_ref};
use crate::objectfile::module::Space;

/// A non-wrapping mathematical interval at a fixed width.
///
/// Direct port of `qbopt.analysis.ranges:Interval`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Interval {
    pub low: BigInt,
    pub high: BigInt,
    pub width: u32,
}

/// A non-wrapping near indexed access as the static byte interval it can touch.
///
/// Direct port of `qbopt.analysis.ranges:covering`.
pub fn covering<'a>(reference: &'a MemRef, known: &BTreeMap<Value, Interval>) -> Cow<'a, MemRef> {
    let reference = symbolic_ref(reference);
    let Some(address) = reference.addr else {
        return reference;
    };
    if address.space != Space::Segment || reference.segment.is_some() {
        return reference;
    }
    // Python's `known.get(ref.base)` returns no interval when `base` is None.
    let interval = reference.base.and_then(|base| known.get(&base));
    let Some(interval) = interval else {
        return reference;
    };
    if interval.width != reference.base_width || reference.base_width != 2 {
        return reference;
    }

    let low = BigInt::from(address.disp) + &interval.low;
    let end = BigInt::from(address.disp) + &interval.high + BigInt::from(reference.width);
    let limit = BigInt::from(1_u8) << (8 * reference.base_width);
    if low < BigInt::from(0_u8) || low >= end || end > limit {
        return reference;
    }

    // `Addr` and `MemRef` use host-sized representations, unlike Python's
    // integers. Refuse rather than truncate if a supported Python result does
    // not fit those representations.
    let width = &end - &low;
    let (Ok(disp), Ok(width)) = (i64::try_from(&low), u32::try_from(&width)) else {
        return reference;
    };
    let mut covered = reference.into_owned();
    covered.addr = Some(crate::objectfile::module::Addr {
        disp,
        base: iced_x86::Register::None,
        ..address
    });
    covered.base = None;
    covered.width = width;
    Cow::Owned(covered)
}
