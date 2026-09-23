//! Port of `qbopt/frontend/addressfacts.py`: non-wrapping symbolic
//! near-address ranges recognized while raising.

use num_bigint::BigInt;

use crate::analysis::ranges::Interval;
use crate::objectfile::module::{Addr, Space};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Region {
    pub anchor: Addr,
    pub offset: Interval,
}

/// Python `overlaps` converts an `Addr` to the region at offset zero.
impl From<Addr> for Region {
    fn from(anchor: Addr) -> Self {
        Region { anchor, offset: Interval { low: 0.into(), high: 0.into(), width: 2 } }
    }
}

impl Region {
    pub(crate) fn shifted(&self, delta: &Interval) -> Option<Region> {
        if delta.width != 2 || self.offset.width != 2 {
            return None;
        }
        let (low, high) = (&self.offset.low + &delta.low, &self.offset.high + &delta.high);
        let disp = BigInt::from(self.anchor.disp);
        if !(BigInt::from(0) <= &disp + &low && &disp + &low <= &disp + &high && &disp + &high < BigInt::from(65536)) {
            return None;
        }
        Some(Region { anchor: self.anchor, offset: Interval { low, high, width: 2 } })
    }

    pub(crate) fn overlaps(&self, width: u32, other: impl Into<Region>, size: u32) -> bool {
        let other = other.into();
        if self.anchor.space != other.anchor.space || self.anchor.index != other.anchor.index {
            return false;
        }
        let (disp, other_disp) = (BigInt::from(self.anchor.disp), BigInt::from(other.anchor.disp));
        &disp + &self.offset.low < &other_disp + &other.offset.high + size
            && &other_disp + &other.offset.low < &disp + &self.offset.high + width
    }
}

pub(crate) fn region(anchor: Addr, offset: Interval, width: i64) -> Option<Region> {
    if anchor.space != Space::Segment || offset.width != 2 || width <= 0 {
        return None;
    }
    let disp = BigInt::from(anchor.disp);
    let (low, high) = (&disp + &offset.low, &disp + &offset.high);
    if !(BigInt::from(0) <= low && low <= high && high <= BigInt::from(65536 - width)) {
        return None;
    }
    Some(Region { anchor, offset })
}
