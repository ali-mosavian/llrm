//! Whether two values interfere, asked of the pairs a pass has a question
//! about. LLVM structure: `LiveRange::liveAt`
//! ([`LiveInterval.h#L411-L414`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveInterval.h#L411-L414))
//! and `LiveRange::overlaps(other)`
//! ([`#L458-L462`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveInterval.h#L458-L462))
//! asked of two registers' ranges, with `overlaps(other, CP, Indexes)` letting
//! a copy's two ends meet. Ours: the same question asked of the values'
//! occurrences and segments, because the ranges are not kept (DEVIATE until
//! `liveness::LiveRange` lands: then this is `overlaps_cp`);
//! `coalesce::_interference_among` builds the graph of every pair of the asked
//! values live together (quadratic in the values live at once, a row of bits
//! per value for each block they cross), which this replaces for the pairs
//! asked.
//!
//! # How it works
//! Two values interfere where one is defined while the other lives on (a copy
//! between values of one width does not make its source and destination
//! interfere), where both are live together after or before a parallel copy
//! (which reads all its sources before it writes any destination), and at the
//! entry of a block nothing branches to. The first is a point query on the
//! values' occurrences; the others are found from the values' live segments
//! (numbers `base[block] + n` for the point before instruction n) by a search
//! for a copy or entry point inside both. A body with phis is the graph's to
//! answer. `LLRM_CHECK_TWOADDR` compares each answer with the graph's.
//!
//! # Example
//! `a` is defined at 3 and read at 9, `b` is defined at 5: `b`'s definition is
//! inside `a`'s range, so they interfere; had `b` been defined at 10 they would
//! not. If `b := a` is a copy of one width at 5, they do not either: `b` is
//! `a` there.
//!
//! # Used by
//! `twoaddr` (which source of a commutative operation to tie). The coalescer's
//! graph users move here when the coalescing policy is decided.

use std::collections::BTreeSet;

use crate::analysis::intervals::Place;
use crate::analysis::occurrences::{Named, Occurrences};
use crate::backend::allocate::{LiveAt, WebRows};
use crate::backend::coalesce;
use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::hash::HashMap;

thread_local! {
    static PAIRS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many pairs this thread has asked about, for a test that a pass asks.
pub fn pairs_asked() -> usize {
    PAIRS.with(std::cell::Cell::get)
}

/// The pairs of `pairs` that interfere, each as (smaller, larger). None where
/// the body has phis: the pairs are then the graph's to answer.
pub fn among_pairs(
    body: &LirBody,
    pairs: &BTreeSet<(u32, u32)>,
) -> Option<BTreeSet<(u32, u32)>> {
    PAIRS.with(|asked| asked.set(asked.get() + pairs.len()));
    let mut partners: HashMap<u32, Vec<u32>> = HashMap::default();
    for &(a, b) in pairs {
        if a != b {
            partners.entry(a).or_default().push(b);
            partners.entry(b).or_default().push(a);
        }
    }
    let asked: BTreeSet<u32> = partners.keys().copied().collect();
    if asked.is_empty() {
        return Some(BTreeSet::new());
    }
    let found = crate::backend::postings::following(body, |postings| Occurrences::of(postings, &asked));
    let rows = found.rows(body)?;
    let widths = found.widths(body, &|value| asked.contains(&value));
    let seen = collapsed(body, &found, &asked);
    let at = Asking { body, seen: &seen, rows: &rows };
    let mut meet: BTreeSet<(u32, u32)> = BTreeSet::new();

    // A definition of a value while another is live after it.
    for &value in &asked {
        for made in found.named(value).iter().filter(|made| made.defined) {
            let one = &body.blocks[made.place.0].insns[made.place.1];
            if one.group.is_some() {
                continue;
            }
            let equal = equal_to(one, &widths);
            for &other in &partners[&value] {
                if Some(other) != equal && other != value && at.live_after(other, made.place) {
                    meet.insert((value.min(other), value.max(other)));
                }
            }
        }
    }

    // Where values are live together without one being defined in the other's
    // range: the entry of a block nothing branches to, and before and after a
    // parallel copy. Two values meet there if both are live at one of these
    // points, found from their segments by a search, not from the list of
    // every value live at each.
    let targets: BTreeSet<i64> = body.blocks.iter().flat_map(|block| block.succ.iter().copied()).collect();
    let mut base: Vec<u64> = Vec::with_capacity(body.blocks.len() + 1);
    let mut size = 0u64;
    for block in &body.blocks {
        base.push(size);
        size += block.insns.len() as u64 + 1;
    }
    let mut points: Vec<u64> = Vec::new();
    for (index, block) in body.blocks.iter().enumerate() {
        if block.at == body.entry || !targets.contains(&block.at) {
            points.push(base[index]);
        }
        let mut next = 0;
        while next < block.insns.len() {
            let group = block.insns[next].group;
            if group.is_none() {
                next += 1;
                continue;
            }
            let first = next;
            while next + 1 < block.insns.len() && block.insns[next + 1].group == group {
                next += 1;
            }
            points.push(base[index] + first as u64);
            points.push(base[index] + next as u64 + 1);
            next += 1;
        }
    }
    if !points.is_empty() {
        points.sort_unstable();
        points.dedup();
        let live = Live::of(body, &seen, &rows, &asked, &base);
        for &(a, b) in pairs {
            if a != b && !meet.contains(&(a.min(b), a.max(b))) && live.meet_at(a, b, &points) {
                meet.insert((a.min(b), a.max(b)));
            }
        }
    }
    Some(
        pairs
            .iter()
            .filter(|(a, b)| a != b && meet.contains(&(*a.min(b), *a.max(b))))
            .map(|(a, b)| (*a.min(b), *a.max(b)))
            .collect(),
    )
}

/// The points of the body each asked value is live at, as segments of the
/// numbers `base[block] + n` gives the point before instruction n of a block
/// (n at the block's length: its end).
struct Live {
    segments: HashMap<u32, Vec<(u64, u64)>>,
}

impl Live {
    fn of(
        body: &LirBody,
        seen: &HashMap<u32, Vec<Named>>,
        rows: &WebRows,
        asked: &BTreeSet<u32>,
        base: &[u64],
    ) -> Self {
        // The blocks each value is live out of, ascending: live through one
        // it is named in neither way.
        let highest = asked.last().map_or(0, |value| *value as usize);
        let mut leaving: Vec<Vec<usize>> = vec![Vec::new(); highest + 1];
        for (index, block) in body.blocks.iter().enumerate() {
            for value in rows.leaving(block.at) {
                leaving[value as usize].push(index);
            }
        }
        let mut segments: HashMap<u32, Vec<(u64, u64)>> = HashMap::default();
        for &value in asked {
            let through = &leaving[value as usize];
            let named = seen[&value].as_slice();
            // The blocks it is named in or live out of, once each, ascending.
            let mut list: Vec<(usize, bool)> = Vec::with_capacity(through.len() + named.len());
            let (mut x, mut y) = (0, 0);
            while x < through.len() || y < named.len() {
                let next = match (through.get(x), named.get(y)) {
                    (Some(a), Some(b)) => (*a).min(b.place.0),
                    (Some(a), None) => *a,
                    (None, Some(b)) => b.place.0,
                    (None, None) => break,
                };
                let out = through.get(x) == Some(&next);
                x += usize::from(out);
                while named.get(y).is_some_and(|made| made.place.0 == next) {
                    y += 1;
                }
                list.push((next, out));
            }
            let mut out: Vec<(u64, u64)> = Vec::new();
            let mut put = |from: u64, to: u64| match out.last_mut() {
                Some(last) if last.1 + 1 == from => last.1 = to,
                _ => out.push((from, to)),
            };
            let mut from = 0;
            for (index, live_out) in list {
                let mut to = from;
                while named.get(to).is_some_and(|made| made.place.0 == index) {
                    to += 1;
                }
                let mine = &named[from..to];
                from = to;
                let mut before = 0u64;
                for made in mine {
                    let at = made.place.1 as u64;
                    // Live up to this instruction where it reads the value.
                    if made.used {
                        put(base[index] + before, base[index] + at);
                    }
                    before = at + 1;
                }
                if live_out {
                    put(base[index] + before, base[index] + body.blocks[index].insns.len() as u64);
                }
            }
            segments.insert(value, out);
        }
        Self { segments }
    }

    /// Whether `one` and `other` are both live at one of `points` (ascending).
    fn meet_at(
        &self,
        one: u32,
        other: u32,
        points: &[u64],
    ) -> bool {
        let (Some(one), Some(other)) = (self.segments.get(&one), self.segments.get(&other)) else { return false };
        let (mut i, mut j) = (0, 0);
        while i < one.len() && j < other.len() {
            let (from, to) = (one[i].0.max(other[j].0), one[i].1.min(other[j].1));
            if from <= to {
                let at = points.partition_point(|point| *point < from);
                if points.get(at).is_some_and(|point| *point <= to) {
                    return true;
                }
            }
            if one[i].1 < other[j].1 {
                i += 1;
            } else {
                j += 1;
            }
        }
        false
    }
}

/// The value a copy of equal widths leaves its destination equal to: it does
/// not interfere with it.
fn equal_to(
    one: &crate::model::lir::Insn,
    widths: &crate::support::hash::IndexMap<u32, u32>,
) -> Option<u32> {
    let copy = coalesce::_copy(one)?;
    if one.defines != [copy.0] || one.uses != [copy.1] {
        return None;
    }
    let what = one.what.as_ref()?;
    let (Loc::Held(into), Loc::Held(source)) = (what.dests.first()?, what.sources.first()?) else { return None };
    let wide = |value: u32| widths.get(&value).copied();
    (into.width == source.width && Some(source.width) == wide(copy.0) && wide(copy.0) == wide(copy.1)).then_some(copy.1)
}

struct Asking<'a> {
    body: &'a LirBody,
    seen: &'a HashMap<u32, Vec<Named>>,
    rows: &'a WebRows,
}

impl Asking<'_> {
    /// The first instruction of `value`'s from `from` on (in its block).
    fn next(
        &self,
        value: u32,
        from: Place,
    ) -> Option<&Named> {
        let named = self.seen.get(&value).map_or(&[][..], Vec::as_slice);
        let at = named.partition_point(|made| made.place < from);
        named.get(at).filter(|made| made.place.0 == from.0)
    }

    /// Whether `value` is live after the instruction at `place`.
    fn live_after(
        &self,
        value: u32,
        place: Place,
    ) -> bool {
        match self.next(value, (place.0, place.1 + 1)) {
            Some(made) => made.used,
            None => self.rows.live_out(self.body.blocks[place.0].at, value),
        }
    }
}

/// Where each asked value is named, for liveness: a parallel copy reads all its
/// sources before it writes any destination, so its instructions are one place,
/// the first, that reads what any of them reads and then writes what any
/// writes.
fn collapsed(
    body: &LirBody,
    found: &Occurrences,
    asked: &BTreeSet<u32>,
) -> HashMap<u32, Vec<Named>> {
    let group = |place: Place| body.blocks[place.0].insns[place.1].group;
    asked
        .iter()
        .map(|value| {
            let mut out: Vec<Named> = Vec::new();
            let mut member: Option<(usize, i64)> = None;
            for made in found.named(*value) {
                match (group(made.place), out.last_mut()) {
                    (Some(id), Some(last)) if member == Some((made.place.0, id)) => {
                        last.defined |= made.defined;
                        last.used |= made.used;
                        last.mentions += made.mentions;
                    }
                    (Some(id), _) => {
                        member = Some((made.place.0, id));
                        out.push(*made);
                    }
                    (None, _) => {
                        member = None;
                        out.push(*made);
                    }
                }
            }
            (*value, out)
        })
        .collect()
}
