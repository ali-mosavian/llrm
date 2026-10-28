//! Points that destroy a register without naming it, and the question "may
//! this value sit in that register over its interval" they answer.
//!
//! # How it works
//! A call (its clobbers), a 16-bit callee (the upper half only) and a point
//! that needs the data segment register destroy registers no instruction
//! operand names. [`Masks`] holds those points of a body; [`_clobbered`] asks
//! whether one of them lies inside an interval, comparing registers by root so
//! `AX` and `EAX` are one register. A mask on the upper half matters only to a
//! value wider than a word.
//!
//! # Example
//! A value live across `call f`, which clobbers `eax`, `ecx` and `edx`: the
//! call is a mask point inside its interval, so [`_clobbered`] is true for
//! those three and the value takes `ebx`, `esi` or `edi`.
//!
//! # Used by
//! `backend::allocate`, the greedy allocator's inputs, and the passes that
//! place values after it.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals::{self as ranges, Indexes, Interval};
use crate::backend::target::{self, Segments};
use crate::model::ir;
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// A point that destroys registers without naming them: `during` the
/// instruction, their high halves only, or `before` it reads its operands.
pub struct Mask {
    pub slot: i64,
    pub during: BTreeSet<RegId>,
    pub high: BTreeSet<RegId>,
    pub before: BTreeSet<RegId>,
}

/// The points that destroy registers, and, for each register, where, in order:
/// whether a value is live across one is asked of every value and every
/// register, over and over, and answered from the slots by bisection, not by a
/// look at every point.
#[derive(Default)]
pub struct Masks {
    list: Vec<Arc<Mask>>,
    /// The instruction each point is of, where the points came from a body:
    /// what `after` finds the points of the instructions a rewrite removed
    /// by.
    owners: Vec<usize>,
    /// Points handed out by `iter`, for a test that a query by span does not
    /// walk them all.
    walked: std::cell::Cell<usize>,
    reaching: std::cell::OnceCell<crate::support::hash::HashMap<RegId, Reaching>>,
    /// The points a rewrite added, where `after` made these from the ones
    /// before it: what a register held across the rewrite may now be
    /// destroyed at.
    added: Option<Vec<Arc<Mask>>>,
}

/// The points that destroy one register, as `Masks::destroyed_within` asks.
pub struct Destroyers<'a> {
    lists: [&'a [i64]; 3],
}

impl Destroyers<'_> {
    /// The first and last point in `lo..hi`.
    pub fn within(
        &self,
        lo: i64,
        hi: i64,
    ) -> Option<(i64, i64)> {
        let mut found: Option<(i64, i64)> = None;
        for list in self.lists {
            let from = list.partition_point(|slot| *slot < lo);
            let to = from + list[from..].partition_point(|slot| *slot < hi);
            if from < to {
                let (first, last) = (list[from], list[to - 1]);
                found = Some(found.map_or((first, last), |(low, high)| (low.min(first), high.max(last))));
            }
        }
        found
    }
}

/// Where one register is destroyed, sorted: before the point's own reads
/// (`read`), during it (`during`), or only its high half (`high`).
#[derive(Clone, Default)]
struct Reaching {
    read: Vec<i64>,
    during: Vec<i64>,
    high: Vec<i64>,
}

impl Masks {
    pub fn new(list: Vec<Mask>) -> Self {
        Self {
            list: list.into_iter().map(Arc::new).collect(),
            owners: Vec::new(),
            walked: std::cell::Cell::new(0),
            reaching: std::cell::OnceCell::new(),
            added: None,
        }
    }

    /// The points of `body`, which is the body of `before` with the
    /// instructions of `changes` removed and added, in the same numbering:
    /// the points of the instructions kept are kept, as LLVM's `SlotIndexes`
    /// keep an instruction's index across an edit. None where `before` did
    /// not come from a body.
    pub fn after(
        before: Masks,
        gone_slots: &IndexMap<usize, i64>,
        index: &Indexes,
        segments: &Segments,
        changes: &ranges::Changes,
    ) -> Option<Masks> {
        if before.owners.len() != before.list.len() {
            return None;
        }
        let Masks { mut list, mut owners, reaching, .. } = before;
        let owner_of = |slot: i64, owner: usize, list: &[Arc<Mask>], owners: &[usize]| -> Option<usize> {
            let from = list.partition_point(|mask| mask.slot < slot);
            (from..list.len()).take_while(|at| list[*at].slot == slot).find(|at| owners[*at] == owner)
        };
        let mut removed: Vec<usize> = Vec::new();
        for one in &changes.gone {
            let Some(slot) = gone_slots.get(&ranges::key(one)) else { continue };
            if let Some(at) = owner_of(*slot, ranges::key(one), &list, &owners) {
                removed.push(at);
            }
        }
        removed.sort_unstable();
        let mut added: Vec<(i64, usize, Arc<Mask>)> = Vec::new();
        for one in &changes.added {
            if let Some(mask) = _mask_of(one, index.at.get(&ranges::key(one)).copied()?, segments) {
                added.push((mask.slot, ranges::key(one), Arc::new(mask)));
            }
        }
        added.sort_by_key(|(slot, owner, _)| (*slot, *owner));
        let put: Vec<Arc<Mask>> = added.iter().map(|(_, _, mask)| Arc::clone(mask)).collect();
        // The points are edited where they stand: the ones the rewrite removed
        // go, the ones it added go in after the points of their slot, and the
        // rest are not touched, where a copy of the list cost a look at every
        // point at each of a function's rewrites.
        let mut taken: Vec<Arc<Mask>> = Vec::with_capacity(removed.len());
        for at in removed.iter().rev() {
            owners.remove(*at);
            taken.push(list.remove(*at));
        }
        taken.reverse();
        for (slot, owner, mask) in added {
            let at = list.partition_point(|point| point.slot <= slot);
            list.insert(at, mask);
            owners.insert(at, owner);
        }
        let found = Masks {
            list,
            owners,
            walked: std::cell::Cell::new(0),
            reaching: std::cell::OnceCell::new(),
            added: Some(put.clone()),
        };
        if let Some(mut reaching) = reaching.into_inner() {
            for mask in &taken {
                Self::unreach(&mut reaching, mask);
            }
            for mask in &put {
                Self::reach(&mut reaching, mask);
            }
            let _ = found.reaching.set(reaching);
        }
        Some(found)
    }

    fn reach(
        found: &mut crate::support::hash::HashMap<RegId, Reaching>,
        mask: &Mask,
    ) {
        for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
            let one = found.entry(*register).or_default();
            let list = if mask.before.contains(register) {
                &mut one.read
            } else if mask.during.contains(register) {
                &mut one.during
            } else {
                &mut one.high
            };
            let at = list.partition_point(|slot| *slot < mask.slot);
            list.insert(at, mask.slot);
        }
    }

    fn unreach(
        found: &mut crate::support::hash::HashMap<RegId, Reaching>,
        mask: &Mask,
    ) {
        for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
            let Some(one) = found.get_mut(register) else { continue };
            let list = if mask.before.contains(register) {
                &mut one.read
            } else if mask.during.contains(register) {
                &mut one.during
            } else {
                &mut one.high
            };
            let at = list.partition_point(|slot| *slot < mask.slot);
            if list.get(at) == Some(&mask.slot) {
                list.remove(at);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The points the rewrite this was made after added; None where it was made
    /// from a body.
    pub fn added(&self) -> Option<&[Arc<Mask>]> {
        self.added.as_deref()
    }

    /// Whether these are the same points as `other`, in the same order, and
    /// the registers are reached at the same slots.
    pub fn same_points(
        &self,
        other: &Masks,
    ) -> bool {
        let key = |mask: &Mask| (mask.slot, mask.during.clone(), mask.high.clone(), mask.before.clone());
        let mut mine: Vec<_> = self.list.iter().zip(&self.owners).map(|(mask, owner)| (key(mask), *owner)).collect();
        let mut theirs: Vec<_> =
            other.list.iter().zip(&other.owners).map(|(mask, owner)| (key(mask), *owner)).collect();
        mine.sort_by_key(|(key, owner)| (key.0, *owner));
        theirs.sort_by_key(|(key, owner)| (key.0, *owner));
        let reached = |found: &Masks| -> Vec<(RegId, Vec<i64>, Vec<i64>, Vec<i64>)> {
            let mut all: Vec<_> = found
                .list
                .iter()
                .flat_map(|mask| mask.before.iter().chain(&mask.during).chain(&mask.high))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter_map(|register| {
                    found
                        .reaching(*register)
                        .map(|one| (*register, one.read.clone(), one.during.clone(), one.high.clone()))
                })
                .collect();
            all.sort_by_key(|one| format!("{:?}", one.0));
            all
        };
        mine == theirs && reached(self) == reached(other)
    }

    /// Room for `more` points, and the room there is, for a test that `after`
    /// edits the list it is given and does not build another.
    #[cfg(test)]
    pub fn reserve_points(
        &mut self,
        more: usize,
    ) {
        self.list.reserve(more);
        self.owners.reserve(more);
    }

    #[cfg(test)]
    pub fn point_capacity(&self) -> usize {
        self.list.capacity().min(self.owners.capacity())
    }

    /// How many points `iter` has handed out.
    pub fn walked(&self) -> usize {
        self.walked.get()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mask> + '_ {
        self.walked.set(self.walked.get() + self.list.len());
        self.list.iter().map(|mask| &**mask)
    }

    /// The first and last point in `lo..hi` that destroys `register` (its high
    /// half only counts for a register wider than a word): the points of one
    /// register are sorted, so a block's span asks two searches, not every
    /// point of the function.
    pub fn destroyed_within(
        &self,
        register: RegId,
        width: u32,
        lo: i64,
        hi: i64,
    ) -> Option<(i64, i64)> {
        self.destroyers(register, width).within(lo, hi)
    }

    /// Where `register` is destroyed, found once for the many spans a
    /// candidate asks of it.
    pub fn destroyers(
        &self,
        register: RegId,
        width: u32,
    ) -> Destroyers<'_> {
        match self.reaching(register) {
            Some(one) => Destroyers { lists: [&one.read, &one.during, if width > 2 { &one.high } else { &[] }] },
            None => Destroyers { lists: [&[], &[], &[]] },
        }
    }

    fn reaching(
        &self,
        register: RegId,
    ) -> Option<&Reaching> {
        self.reaching
            .get_or_init(|| {
                let mut found: crate::support::hash::HashMap<RegId, Reaching> =
                    crate::support::hash::HashMap::default();
                for mask in self.list.iter().map(|mask| &**mask) {
                    for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
                        let one = found.entry(*register).or_default();
                        if mask.before.contains(register) {
                            one.read.push(mask.slot);
                        } else if mask.during.contains(register) {
                            one.during.push(mask.slot);
                        } else {
                            one.high.push(mask.slot);
                        }
                    }
                }
                for one in found.values_mut() {
                    one.read.sort_unstable();
                    one.during.sort_unstable();
                    one.high.sort_unstable();
                }
                found
            })
            .get(&register)
    }
}

/// Every point a register is destroyed without being named, and which. The
/// data segment register is reloaded ahead of a point that needs the data
/// group, so it holds none of that point's operands either.
pub fn _masks(
    body: &LirBody,
    index: &Indexes,
    segments: &Segments,
) -> Masks {
    let mut list: Vec<Arc<Mask>> = Vec::new();
    let mut owners: Vec<usize> = Vec::new();
    for block in &body.blocks {
        for one in &block.insns {
            if let Some(mask) = _mask_of(one, index.at[&ranges::key(one)], segments) {
                list.push(Arc::new(mask));
                owners.push(ranges::key(one));
            }
        }
    }
    Masks { list, owners, walked: std::cell::Cell::new(0), reaching: std::cell::OnceCell::new(), added: None }
}

/// The point `one` is at `slot`, if it destroys a register without naming it.
fn _mask_of(
    one: &Arc<Insn>,
    slot: i64,
    segments: &Segments,
) -> Option<Mask> {
    let during: BTreeSet<RegId> = one.clobbers.iter().map(|register| _whole(*register)).collect();
    let high: BTreeSet<RegId> = one.clobbers_high.iter().map(|register| _whole(*register)).collect();
    let before: BTreeSet<RegId> = target::needs_data_group(one).then_some(segments.data).into_iter().collect();
    (!during.is_empty() || !high.is_empty() || !before.is_empty()).then_some(Mask { slot, during, high, before })
}

/// The 32-bit register this one is part of.
pub fn _whole(register: RegId) -> RegId {
    ir::root(register)
}

/// Whether this range is live across a point that destroys the register, or
/// into one that destroys it before reading.
pub fn _clobbered(
    one: &Interval,
    register: RegId,
    masks: &Masks,
    width: u32,
) -> bool {
    let answer = _clobbered_from(one, register, masks, width);
    if check_clobbered() {
        assert_eq!(
            answer,
            _clobbered_reference(one, register, masks, width),
            "LLRM_CHECK_CLOBBERED: {register:?} over {:?}",
            one.segments
        );
    }
    answer
}

fn check_clobbered() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_CLOBBERED"))
}

/// `_clobbered`, from where each register is destroyed: a segment meets a point
/// where the point is after its start and, by `reaches`, no later than its end.
fn _clobbered_from(
    one: &Interval,
    register: RegId,
    masks: &Masks,
    width: u32,
) -> bool {
    let Some(places) = masks.reaching(_whole(register)) else { return false };
    // The first slot after `start` is the best candidate: any later one reaches
    // no further.
    let after = |slots: &[i64], start: i64| slots.get(slots.partition_point(|slot| *slot <= start)).copied();
    one.segments
        .iter()
        .any(
            |seg| after(&places.read, seg.start).is_some_and(|slot| seg.end >= ranges::def_point(slot))
                || after(&places.during, seg.start).is_some_and(|slot| seg.end > ranges::def_point(slot))
                || (width > 2 && after(&places.high, seg.start).is_some_and(|slot| seg.end > ranges::def_point(slot))),
        )
}

/// What `_clobbered` was: a look at every point.
pub fn _clobbered_reference(
    one: &Interval,
    register: RegId,
    masks: &Masks,
    width: u32,
) -> bool {
    let mine = _whole(register);
    for mask in masks.iter() {
        let slot = mask.slot;
        let read = mask.before.contains(&mine);
        if !read && !mask.during.contains(&mine) && (!mask.high.contains(&mine) || width <= 2) {
            continue;
        }
        // A use keeps its value alive to `slot + DEF`.
        let reaches = |end: i64| if read { end >= ranges::def_point(slot) } else { end > ranges::def_point(slot) };
        if one.segments.iter().any(|seg| seg.start < slot && reaches(seg.end)) {
            return true;
        }
    }
    false
}
