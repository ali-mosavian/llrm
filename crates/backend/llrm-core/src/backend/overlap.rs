//! Whether two machine memory operands may overlap, read from their
//! addresses alone.
//!
//! # How it works
//! The answer is syntactic. A pass that needs a semantic answer for memory
//! outside the frame (two globals, a pointer) gets it from the MIR, carried on
//! the instruction as `CallMemory` carries a call's effects, never re-derived
//! here.
//!
//! `may_overlap` tries three things in order.
//!
//! 1. Two fixed frame cells (no base register) meet when their byte ranges do.
//!    This is the fast path for a body that copies a large struct.
//! 2. Two frame addresses in different slots (`slot_home`) never meet, even
//!    where one has a base register: an indexed access stays in its slot.
//! 3. Otherwise each address becomes a set of spans (region, origin, low, high)
//!    and they meet if any pair does.
//!
//! A region is a path of nested parts, and a coarser prefix meets every child
//! below it:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   root["<b>root</b><br/><small>literal, far without a name,<br/>group</small>"]:::mid
//!   stack["<b>Stack</b><br/><small>stack and frame cells</small>"]:::mid
//!   dg["<b>Dgroup / Linked</b><br/><small>external symbols</small>"]:::mid
//!   seg["<b>Segment(n)</b><br/><small>a named data segment</small>"]:::mid
//!   named["<b>Named</b><br/><small>a far address with an index</small>"]:::mid
//!   root --> stack
//!   root --> dg
//!   dg --> seg
//!   root --> named
//! ```
//!
//! *Regions of a memory address*
//!
//! Spans in one region compare by offset and width when they count from the
//! same origin (SP, BP, here, or an external symbol); in different origins
//! they may meet. An address with a base register, or in the root or `Named`
//! region, covers its whole region. A root-region address is known not to be
//! the stack, so stack spans are removed from the comparison. An unknown
//! address or an end past 2^63 may overlap anything.
//!
//! # Example
//! No honest example: a yes-or-no answer about two operands, with no LIR of its
//! own. Read `tests` in this file for the operand pairs and the answers.
//!
//! # Used by
//! `spiller`, `floatassign`, `storedhomes`.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;

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
    fn under(
        &self,
        other: &Self,
    ) -> bool {
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

fn region(
    space: Space,
    index: i64,
) -> (Region, Origin) {
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
fn spans(
    address: Option<Addr>,
    width: u32,
) -> Option<BTreeSet<Span>> {
    let Some(address) = address else {
        return Some(BTreeSet::from([Span {
            region: Region::default(),
            origin: Origin::Here,
            low: WHOLE.0,
            high: WHOLE.1,
        }]));
    };
    let (region, origin) = region(address.space, address.index);
    let (low, high) =
        if address.base != (RegId::None).iced() || region == Region::default() || region == Region(vec![Part::Named]) {
            WHOLE
        } else {
            (address.disp, address.disp.checked_add(i64::from(width.max(1)))?)
        };
    Some(BTreeSet::from([Span { region, origin, low, high }]))
}

/// What an address in the root region cannot be: the stack.
fn holes(address: Option<Addr>) -> BTreeSet<Span> {
    match address {
        Some(address) if region(address.space, address.index).0 == Region::default() => BTreeSet::from([Span {
            region: Region(vec![Part::Stack]),
            origin: Origin::Sp,
            low: WHOLE.0,
            high: WHOLE.1,
        }]),
        _ => BTreeSet::new(),
    }
}

fn meets(
    one: &Span,
    other: &Span,
) -> bool {
    if !one.region.under(&other.region) {
        return false;
    }
    if one.region != other.region || one.origin != other.origin {
        return true;
    }
    one.low < other.high && other.low < one.high
}

/// `spans` less those a single hole covers.
fn surviving<'s>(
    spans: &'s BTreeSet<Span>,
    holes: &BTreeSet<Span>,
) -> Vec<&'s Span> {
    spans
        .iter()
        .filter(|one| {
            !holes
                .iter()
                .any(
                    |hole| one.region.0.starts_with(&hole.region.0)
                        && hole.origin == one.origin
                        && hole.low <= one.low
                        && one.high <= hole.high,
                )
        })
        .collect()
}

/// Whether the `one_width` bytes at `one` may overlap the `other_width`
/// bytes at `other`; an end past 2^63 may.
pub fn may_overlap(
    one: Option<Addr>,
    one_width: u32,
    other: Option<Addr>,
    other_width: u32,
) -> bool {
    if let Some(answer) = frame_bytes(one, one_width, other, other_width) {
        return answer;
    }
    if different_slots(one, other) {
        return false;
    }
    may_overlap_by_sets(one, one_width, other, other_width)
}

/// Two fixed frame cells meet when their byte ranges do: the general answer for
/// them, without the sets built to give it (a body that copies a large struct
/// asks it of every cell against every store).
fn frame_bytes(
    one: Option<Addr>,
    one_width: u32,
    other: Option<Addr>,
    other_width: u32,
) -> Option<bool> {
    let (one, other) = (one?, other?);
    let fixed = |address: &Addr| address.space == Space::Frame && address.base == (RegId::None).iced();
    if !fixed(&one) || !fixed(&other) {
        return None;
    }
    let end = |address: &Addr, width: u32| address.disp.checked_add(i64::from(width.max(1)));
    let (one_end, other_end) = (end(&one, one_width)?, end(&other, other_width)?);
    Some(one.disp < other_end && other.disp < one_end)
}

/// A frame address that reaches its whole object (one with a base register,
/// an indexed array's) stays in the slot it was made from, and meets no cell
/// of another slot.
fn different_slots(
    one: Option<Addr>,
    other: Option<Addr>,
) -> bool {
    let (Some(one), Some(other)) = (one, other) else { return false };
    one.space == Space::Frame
        && other.space == Space::Frame
        && matches!(
            (one.slot_home(), other.slot_home()),
            (Some(a), Some(b)) if a != b
        )
}

fn may_overlap_by_sets(
    one: Option<Addr>,
    one_width: u32,
    other: Option<Addr>,
    other_width: u32,
) -> bool {
    let (Some(first), Some(second)) = (spans(one, one_width), spans(other, other_width)) else {
        return true;
    };
    let holes: BTreeSet<Span> = holes(one).union(&holes(other)).cloned().collect();
    surviving(&first, &holes).iter().any(|a| surviving(&second, &holes).iter().any(|b| meets(a, b)))
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::RegId;

    use super::may_overlap;
    use crate::objectfile::module::{Addr, Space};

    fn frame(disp: i64) -> Option<Addr> {
        Some(Addr::new(Space::Frame, disp))
    }

    /// An indexed array's cell (a frame address with a base register) reaches
    /// its whole object and no other slot: the store of a local beside it was
    /// kept for it, and a reload of a local was made after it.
    #[test]
    fn a_based_frame_address_meets_its_own_slot_and_no_other() {
        let array =
            |home: i64| Some(Addr { base: (RegId::BP).iced(), ..Addr::new(Space::Frame, home + 700).in_slot(home) });
        let local = |home: i64| Some(Addr::new(Space::Frame, home).in_slot(home));
        assert!(!may_overlap(array(-800), 1, local(-8), 4));
        assert!(may_overlap(array(-800), 1, local(-800), 2));
        assert!(may_overlap(array(-800), 1, array(-800), 1));
    }

    /// Frame bytes meet by offset and width; a based address reaches its
    /// whole region; a literal address is never SP-relative stack.
    #[test]
    fn overlap_reads_offsets_widths_and_regions() {
        assert!(!may_overlap(frame(-4), 2, frame(-2), 2));
        assert!(may_overlap(frame(-4), 4, frame(-2), 2));
        let based = Some(Addr { base: (RegId::BX).iced(), ..Addr::new(Space::Frame, 0) });
        assert!(may_overlap(based, 1, frame(-40), 2));
        let literal = Some(Addr::new(Space::Literal, 0x400));
        assert!(!may_overlap(literal, 2, Some(Addr::new(Space::Stack, 2)), 2));
        assert!(may_overlap(literal, 2, frame(-2), 2));
        assert!(may_overlap(None, 2, frame(-2), 2));
    }

    /// The fast answer for two fixed frame cells is the general one, over
    /// widths (0 counts as 1) and displacements at both ends of the range.
    #[test]
    fn frame_cells_meet_as_the_general_answer_says() {
        let edges = [
            i64::MIN,
            i64::MIN + 3,
            -(1 << 31) - 1,
            -100,
            -9,
            -8,
            -5,
            -4,
            -1,
            0,
            1,
            3,
            4,
            8,
            100,
            (1 << 31) + 1,
            i64::MAX - 3,
            i64::MAX,
        ];
        for a in edges {
            for b in edges {
                for (wa, wb) in [(0, 0), (1, 4), (4, 4), (8, 2), (u32::MAX, 1)] {
                    assert_eq!(
                        may_overlap(frame(a), wa, frame(b), wb),
                        super::may_overlap_by_sets(frame(a), wa, frame(b), wb),
                        "{a} {wa} {b} {wb}"
                    );
                }
            }
        }
    }
}
