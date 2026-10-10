//! The live intervals the allocator holds of the body it is rewriting.
//!
//! An interval is worked out once and kept as it is while the instructions that
//! name its value stay: slots are stable keys (`Indexes::patched`), so a
//! rewrite moves none of them. What does change with a rewrite is the size of
//! an interval an added or removed instruction lies inside, and with it the
//! weight, which is a function of the size. The weight is therefore not stored
//! but worked out when it is read, from what it is made of: the references the
//! value has (`total`), what its copies and folds save (`sibling`, `fold`),
//! whether it may be spilled (`pinned`), and its size in the numbering.

use std::collections::BTreeSet;
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

/// The positive infinity a value that must not be spilled weighs.
pub const INF: f64 = f64::INFINITY;

#[derive(Clone)]
pub struct LiveRanges {
    ranges: IndexMap<u32, Arc<Interval>>,
    index: Arc<Indexes>,
    total: Arc<IndexMap<u32, f64>>,
    sibling: Arc<IndexMap<u32, f64>>,
    fold: Arc<IndexMap<u32, f64>>,
    pinned: Arc<BTreeSet<u32>>,
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

    /// With what its copies and folds save, and the values that may not be
    /// spilled.
    pub fn priced(
        mut self,
        sibling: Arc<IndexMap<u32, f64>>,
        fold: Arc<IndexMap<u32, f64>>,
        pinned: BTreeSet<u32>,
    ) -> Self {
        self.sibling = sibling;
        self.fold = fold;
        self.pinned = Arc::new(pinned);
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
