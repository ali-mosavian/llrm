//! Whether two machine memory operands may overlap, read from their
//! addresses alone: the same space and segment compared by offset and
//! width, and "may overlap" for anything else.
//!
//! This stays syntactic. A pass that needs a semantic answer for memory
//! outside the frame (two globals, a pointer) gets it from the rich MIR,
//! carried on the instruction as `CallMemory` carries a call's effects,
//! never re-derived here.

use std::collections::BTreeSet;

use crate::objectfile::module::{Addr, Space};

const WHOLE: (i64, i64) = (-(1_i64 << 31), 1_i64 << 31);

/// A path of nested regions: a coarser prefix meets each child below it.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
struct Region(Vec<Part>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Part {
    Stack,
    Dgroup,
    Linked,
    Named,
    Segment(i64),
}

impl Region {
    fn under(&self, other: &Self) -> bool {
        self.0.starts_with(&other.0) || other.0.starts_with(&self.0)
    }
}

/// What a displacement counts from.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Origin {
    Here,
    Sp,
    Bp,
    External(i64),
}

/// A half-open byte interval in one region, counted from `origin`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Span {
    region: Region,
    origin: Origin,
    low: i64,
    high: i64,
}

fn region(space: Space, index: i64) -> (Region, Origin) {
    match space {
        Space::Stack => (Region(vec![Part::Stack]), Origin::Sp),
        Space::Frame => (Region(vec![Part::Stack]), Origin::Bp),
        Space::Segment => (Region(vec![Part::Dgroup, Part::Linked, Part::Segment(index)]), Origin::Here),
        Space::External => (Region(vec![Part::Dgroup, Part::Linked]), Origin::External(index)),
        Space::Far if index != 0 => (Region(vec![Part::Named]), Origin::Here),
        // Literal, Far without an index, Group: the root.
        _ => (Region::default(), Origin::Here),
    }
}

/// The bytes `address` may reach, `width` long; with no address, anything.
fn spans(address: Option<Addr>, width: u32) -> Option<BTreeSet<Span>> {
    let Some(address) = address else {
        return Some(BTreeSet::from([Span { region: Region::default(), origin: Origin::Here, low: WHOLE.0, high: WHOLE.1 }]));
    };
    let (region, origin) = region(address.space, address.index);
    let (low, high) = if address.base != iced_x86::Register::None || region == Region::default() || region == Region(vec![Part::Named]) {
        WHOLE
    } else {
        (address.disp, address.disp.checked_add(i64::from(width.max(1)))?)
    };
    Some(BTreeSet::from([Span { region, origin, low, high }]))
}

/// What an address in the root region cannot be: the stack.
fn holes(address: Option<Addr>) -> BTreeSet<Span> {
    match address {
        Some(address) if region(address.space, address.index).0 == Region::default() => {
            BTreeSet::from([Span { region: Region(vec![Part::Stack]), origin: Origin::Sp, low: WHOLE.0, high: WHOLE.1 }])
        }
        _ => BTreeSet::new(),
    }
}

fn meets(one: &Span, other: &Span) -> bool {
    if !one.region.under(&other.region) {
        return false;
    }
    if one.region != other.region || one.origin != other.origin {
        return true;
    }
    one.low < other.high && other.low < one.high
}

/// `spans` less those a single hole covers.
fn surviving<'s>(spans: &'s BTreeSet<Span>, holes: &BTreeSet<Span>) -> Vec<&'s Span> {
    spans
        .iter()
        .filter(|one| {
            !holes.iter().any(|hole| one.region.0.starts_with(&hole.region.0) && hole.origin == one.origin && hole.low <= one.low && one.high <= hole.high)
        })
        .collect()
}

/// Whether the `one_width` bytes at `one` may overlap the `other_width`
/// bytes at `other`; an end past 2^63 may.
pub fn may_overlap(one: Option<Addr>, one_width: u32, other: Option<Addr>, other_width: u32) -> bool {
    let (Some(first), Some(second)) = (spans(one, one_width), spans(other, other_width)) else {
        return true;
    };
    let holes: BTreeSet<Span> = holes(one).union(&holes(other)).cloned().collect();
    surviving(&first, &holes).iter().any(|a| surviving(&second, &holes).iter().any(|b| meets(a, b)))
}

#[cfg(test)]
mod tests {
    use super::may_overlap;
    use crate::objectfile::module::{Addr, Space};

    fn frame(disp: i64) -> Option<Addr> {
        Some(Addr::new(Space::Frame, disp))
    }

    /// Frame bytes meet by offset and width; a based address reaches its
    /// whole region; a literal address is never SP-relative stack.
    #[test]
    fn overlap_reads_offsets_widths_and_regions() {
        assert!(!may_overlap(frame(-4), 2, frame(-2), 2));
        assert!(may_overlap(frame(-4), 4, frame(-2), 2));
        let based = Some(Addr { base: iced_x86::Register::BX, ..Addr::new(Space::Frame, 0) });
        assert!(may_overlap(based, 1, frame(-40), 2));
        let literal = Some(Addr::new(Space::Literal, 0x400));
        assert!(!may_overlap(literal, 2, Some(Addr::new(Space::Stack, 2)), 2));
        assert!(may_overlap(literal, 2, frame(-2), 2));
        assert!(may_overlap(None, 2, frame(-2), 2));
    }
}
