//! The live intervals the allocator holds of the body it is rewriting, and the
//! spill weight each value gets from them.
//!
//! # How it works
//! An interval is worked out once and kept while the instructions that name
//! its value stay. Instruction numbers are stable keys (`Indexes::patched`), so
//! a rewrite moves none of them. What a rewrite does change is the size of an
//! interval that an added or removed instruction lies inside, and with it the
//! weight, which is a function of the size. The weight is therefore not stored
//! but worked out when it is read, from what it is made of.
//!
//! ```text
//!             total        sibling    fold
//!   weight = (-------  -  -------  - -----)  floored at 0
//!             size         size       size
//!
//!   size   = slots the value is live for + the grace the numbering adds   (Interval::spill_size)
//!   pinned = INF                                    (never spilled)
//! ```
//!
//! `total` is the weighted references the value has. `sibling` and `fold` are
//! what its copies and its foldable loads save if it stays in a register; they
//! come from the allocator (`priced`). A value in `pinned` (a spiller product,
//! or one the caller protects) weighs infinity.
//!
//! After an edit, [`LiveRanges::edited`] drops the intervals of the touched
//! values and puts the recomputed ones in; the rest are shared by `Arc`. The
//! allocator's own facts are carried to the next round and edited where they
//! are (`edit`, `Indexes::patch`): nothing else holds them (`release` clears
//! the spiller's copy, `snapshot` keeps the touched values' old intervals for
//! the allocator's refresh), so a round copies nothing of the values it did
//! not name. The
//! allocator also leaves its latest intervals in a thread-local (`hold`); the
//! spiller it calls takes them back (`held`) when the body is exactly the one
//! they were made of, instead of working them out again.
//!
//! # Example
//! No example: the intervals and weights are internal to the allocator and
//! printed by no dump. See the `RegAlloc` stage in `lirtext.rs` for the
//! allocator's result.
//!
//! # Used by
//! `allocate` builds and edits it; `spiller`, `splitkit` and `regclass`
//! read it (through the [`Ranges`] trait where only the intervals
//! matter).

use std::sync::Arc;

use crate::analysis::intervals::{Indexes, Interval};
use crate::support::hash::IndexMap;

/// Anything that says where a value is live.
pub trait Ranges {
    fn range(
        &self,
        value: u32,
    ) -> Option<&Interval>;
}

impl<T: Ranges + ?Sized> Ranges for Arc<T> {
    #[inline(always)]
    fn range(
        &self,
        value: u32,
    ) -> Option<&Interval> {
        (**self).range(value)
    }
}

impl Ranges for IndexMap<u32, Interval> {
    fn range(
        &self,
        value: u32,
    ) -> Option<&Interval> {
        self.get(&value)
    }
}

/// Some values' intervals as they were before an edit.
pub struct Snapshot(IndexMap<u32, Arc<Interval>>);

impl Ranges for Snapshot {
    fn range(
        &self,
        value: u32,
    ) -> Option<&Interval> {
        self.0.get(&value).map(|one| &**one)
    }
}

/// The positive infinity a value that must not be spilled weighs.
pub const INF: f64 = f64::INFINITY;

#[derive(Clone)]
pub struct LiveRanges {
    ranges: IndexMap<u32, Arc<Interval>>,
    index: Arc<Indexes>,
    total: Arc<IndexMap<u32, f64>>,
    sibling: Arc<IndexMap<u32, f64>>,
    fold: Arc<IndexMap<u32, f64>>,
    pinned: Arc<llrm_mir::dense::IdSet<u32>>,
    /// Weights a test states, which are then what `weight` says.
    #[cfg(test)]
    forced: Option<IndexMap<u32, f64>>,
}

impl Ranges for LiveRanges {
    #[inline(always)]
    fn range(
        &self,
        value: u32,
    ) -> Option<&Interval> {
        self.get(&value)
    }
}

impl std::ops::Index<&u32> for LiveRanges {
    type Output = Interval;
    fn index(
        &self,
        value: &u32,
    ) -> &Interval {
        self.get(value).expect("a live value")
    }
}

impl LiveRanges {
    /// Holding nothing: what stands in a place whose intervals were taken.
    pub fn empty() -> Self {
        Self::new(IndexMap::default(), Arc::new(Indexes::plain()), IndexMap::default())
    }

    pub fn new(
        ranges: IndexMap<u32, Interval>,
        index: Arc<Indexes>,
        total: IndexMap<u32, f64>,
    ) -> Self {
        Self {
            ranges: ranges.into_iter().map(|(value, one)| (value, Arc::new(one))).collect(),
            index,
            total: Arc::new(total),
            sibling: Arc::default(),
            fold: Arc::default(),
            pinned: Arc::default(),
            #[cfg(test)]
            forced: None,
        }
    }

    /// The intervals of the body this one was made of, less those of `touched`,
    /// with `again` put in.
    pub fn edited(
        &self,
        touched: &crate::support::hash::HashSet<u32>,
        again: IndexMap<u32, Interval>,
        totals: IndexMap<u32, f64>,
        index: Arc<Indexes>,
    ) -> Self {
        let mut ranges = self.ranges.clone();
        for value in touched {
            ranges.swap_remove(value);
        }
        ranges.extend(again.into_iter().map(|(value, one)| (value, Arc::new(one))));
        let mut total = (*self.total).clone();
        for value in touched {
            total.swap_remove(value);
        }
        total.extend(totals);
        Self {
            ranges,
            index,
            total: Arc::new(total),
            sibling: Arc::clone(&self.sibling),
            fold: Arc::clone(&self.fold),
            pinned: Arc::clone(&self.pinned),
            #[cfg(test)]
            forced: None,
        }
    }

    /// `edited`, taking the intervals this holds instead of copying them: what
    /// is the same in every other body is not touched.
    pub fn edit(
        mut self,
        touched: &crate::support::hash::HashSet<u32>,
        again: IndexMap<u32, Interval>,
        totals: IndexMap<u32, f64>,
        index: Arc<Indexes>,
    ) -> Self {
        for value in touched {
            self.ranges.swap_remove(value);
        }
        self.ranges.extend(again.into_iter().map(|(value, one)| (value, Arc::new(one))));
        let total = Arc::make_mut(&mut self.total);
        for value in touched {
            total.swap_remove(value);
        }
        total.extend(totals);
        self.index = index;
        self
    }

    /// These intervals without the numbering they were made against, which
    /// whoever edits them holds: it is edited where it is only if no other copy
    /// of it stands.
    pub fn apart(mut self) -> Self {
        self.index = Arc::new(Indexes::plain());
        self.sibling = Arc::default();
        self.fold = Arc::default();
        self
    }

    /// The intervals of `touched` as they are, to be asked after this is edited
    /// (the allocator's refresh asks where a value was).
    pub fn snapshot(
        &self,
        touched: &crate::support::hash::HashSet<u32>,
    ) -> Snapshot {
        Snapshot(
            touched.iter().filter_map(|value| self.ranges.get(value).map(|one| (*value, Arc::clone(one)))).collect(),
        )
    }

    /// With what its copies and folds save, and the values that may not be
    /// spilled.
    pub fn priced(
        mut self,
        sibling: Arc<IndexMap<u32, f64>>,
        fold: Arc<IndexMap<u32, f64>>,
        pinned: Arc<llrm_mir::dense::IdSet<u32>>,
    ) -> Self {
        self.sibling = sibling;
        self.fold = fold;
        self.pinned = pinned;
        self
    }

    #[inline(always)]
    pub fn get(
        &self,
        value: &u32,
    ) -> Option<&Interval> {
        self.ranges.get(value).map(|one| &**one)
    }

    pub fn contains_key(
        &self,
        value: &u32,
    ) -> bool {
        self.ranges.contains_key(value)
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &u32> + '_ {
        self.ranges.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &Interval)> + '_ {
        self.ranges.iter().map(|(value, one)| (value, &**one))
    }

    pub fn values(&self) -> impl Iterator<Item = &Interval> + '_ {
        self.ranges.values().map(|one| &**one)
    }

    pub fn index(&self) -> &Arc<Indexes> {
        &self.index
    }

    /// What spilling `value` costs, per slot it is live for: its references,
    /// less what its copies and folds save, over its size and the grace
    /// every value has.
    pub fn weight(
        &self,
        value: u32,
    ) -> f64 {
        #[cfg(test)]
        if let Some(forced) = &self.forced {
            return forced.get(&value).copied().unwrap_or(0.0);
        }
        if self.pinned.contains(&value) {
            return INF;
        }
        let Some(one) = self.get(&value) else { return 0.0 };
        let size = one.spill_size(&self.index) as f64;
        let mut weight = match self.total.get(&value) {
            Some(found) => *found / size,
            None => 0.0,
        };
        for saved in [&self.sibling, &self.fold] {
            if let Some(found) = saved.get(&value) {
                let adjusted = weight - found / size;
                weight = if adjusted > 0.0 { adjusted } else { 0.0 };
            }
        }
        weight
    }
}

#[cfg(test)]
impl LiveRanges {
    /// Intervals made by hand, each weighing what its `weight` says.
    pub fn stated(ranges: IndexMap<u32, Interval>) -> Self {
        let forced = ranges.iter().map(|(value, one)| (*value, one.weight)).collect();
        let mut live = Self::new(ranges, Arc::new(Indexes::plain()), IndexMap::default());
        live.forced = Some(forced);
        live
    }
}

thread_local! {
    /// The intervals the allocator holds, with the instruction lists of the
    /// body they are of.
    static HELD: std::cell::RefCell<Option<(Vec<(i64, crate::model::lir::Insns)>, Arc<LiveRanges>)>> =
        const { std::cell::RefCell::new(None) };
}

/// Say that `live` is the intervals of `body`, for the spiller the allocator
/// calls to ask rather than work them out again.
pub fn hold(
    body: &crate::model::lir::LirBody,
    live: &Arc<LiveRanges>,
) {
    HELD.with(|held| {
        *held.borrow_mut() =
            Some((body.blocks.iter().map(|block| (block.at, block.insns.clone())).collect(), Arc::clone(live)));
    });
}

/// Forget the intervals held: the allocator is about to edit them where they
/// are, which it can only do when no one else holds them.
pub fn release() {
    HELD.with(|held| *held.borrow_mut() = None);
}

/// The intervals the allocator holds of exactly `body`, if it does.
pub fn held(body: &crate::model::lir::LirBody) -> Option<Arc<LiveRanges>> {
    HELD.with(|held| {
        held.borrow()
            .as_ref()
            .and_then(
                |(blocks, live)| (blocks.len() == body.blocks.len()
                    && blocks
                        .iter()
                        .zip(&body.blocks)
                        .all(|((at, insns), block)| *at == block.at && insns.same_insns(&block.insns)))
                .then(|| Arc::clone(live)),
            )
    })
}
