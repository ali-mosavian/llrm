//! An ordered set of memory slices that holds most of itself as a run shared
//! with other sets, so the callback effects every call summary repeats are
//! built, compared and listed once.
//!
//! # How it works
//! A function that may call back into the module (an address-taken function or
//! an entry) has, in its summary, the effects of all those callbacks. That run
//! of slices is the same for every such summary, every instantiation of it at a
//! call and every call's effect. Building, comparing, filtering and listing a
//! tree set holding it each time is wasted work.
//!
//! `Shared` is one immutable run of slices behind an `Rc`, with facts computed
//! once: whether none is in a `Frame` or `Stack` object (`outlives`), whether
//! any is a `Parameter`'s, and which objects it holds whole or in part. A
//! `SliceSet` is `own` (a `BTreeSet<Slice>`) plus an optional `base:
//! Rc<Shared>`, the two disjoint. Seen as a set, it is exactly the union.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   ss["<b>SliceSet</b><br/><small>own, few</small>"]:::src
//!   sh["<b>Shared</b><br/><small>(one allocation, many holders)<br/>set, outlives, parameters,<br/>wholes, parts, coalesced</small>"]:::opt
//!   other["<b>another SliceSet</b>"]:::mid
//!   ss -->|"base"| sh
//!   other -->|"base (same Rc)"| sh
//! ```
//!
//! *A SliceSet is own plus a base; another SliceSet's base is the same Rc, so
//! ptr_eq is a shortcut*
//!
//! `absorb` takes the other set's run by reference when this set has none or
//! the same one, and copies slices otherwise. `retain` and `retain_outliving`
//! ask the run as a whole where `shared_kept` can answer, and `materialize` it
//! into `own` where only per-slice tests can. `coalesced` drops the subranges
//! of an object the set has every byte of; the run is materialized only if its
//! own wholes and parts disagree or the set's own wholes cover a part of it.
//! `mapped` rewrites `own` and keeps the run, so the map must not change what
//! the run holds. Equality compares by `Rc::ptr_eq` and `own` when the runs are
//! the same.
//!
//! # Example
//! None: this is a container, with no MIR input and no printed output. Its
//! test, `a_set_with_a_shared_run_is_the_union_of_it_and_its_own` in
//! `sliceset.rs`, checks a `SliceSet` holding a shared run against a `BTreeSet`
//! of the same slices.
//!
//! # Provides
//! `SliceSet`, `Shared` and `whole`. Consumers: [`crate::alias`] (call
//! summaries and `callbacks_over`) and
//! the `interprocedural` pass in llrm-transforms.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::memory::{MemoryKind, ObjectRef, Slice, WHOLE_HIGH, WHOLE_LOW};

/// Every byte of an object, as `_coalesced` takes it.
pub fn whole(one: &Slice) -> bool {
    one.low == WHOLE_LOW && one.high == WHOLE_HIGH && one.stride == 1 && one.width == 1
}

/// A run of slices shared by the sets that hold it, and what asking those sets
/// needs to know of it.
#[derive(Debug)]
pub struct Shared {
    set: BTreeSet<Slice>,
    /// No slice of it is in a frame: it outlives the function that calls.
    outlives: bool,
    /// Whether any is a parameter's, which a call instantiates.
    parameters: bool,
    /// The objects it has every byte of, and those it has only some bytes of.
    wholes: BTreeSet<ObjectRef>,
    parts: BTreeSet<ObjectRef>,
    /// Coalesced alone: no object has both a whole slice and a part.
    coalesced: bool,
}

impl Shared {
    pub fn of(set: BTreeSet<Slice>) -> Rc<Self> {
        let outlives = set.iter().all(|one| !matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack));
        let parameters = set.iter().any(|one| one.object.kind == MemoryKind::Parameter);
        let wholes: BTreeSet<ObjectRef> = set.iter().filter(|one| whole(one)).map(|one| one.object).collect();
        let parts: BTreeSet<ObjectRef> = set.iter().filter(|one| !whole(one)).map(|one| one.object).collect();
        let coalesced = wholes.is_disjoint(&parts);
        Rc::new(Self { set, outlives, parameters, wholes, parts, coalesced })
    }

    pub fn set(&self) -> &BTreeSet<Slice> {
        &self.set
    }
}

/// The slices of `own` and of `base`, which have none in common.
#[derive(Clone, Debug, Default)]
pub struct SliceSet {
    own: BTreeSet<Slice>,
    base: Option<Rc<Shared>>,
}

impl SliceSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Exactly the slices of `shared`, held by reference.
    pub fn shared(shared: &Rc<Shared>) -> Self {
        Self { own: BTreeSet::new(), base: Some(Rc::clone(shared)) }
    }

    pub fn len(&self) -> usize {
        self.own.len() + self.base.as_ref().map_or(0, |base| base.set.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(
        &self,
        one: &Slice,
    ) -> bool {
        self.own.contains(one) || self.base.as_ref().is_some_and(|base| base.set.contains(one))
    }

    /// Whether `one` was not in it.
    pub fn insert(
        &mut self,
        one: Slice,
    ) -> bool {
        !self.base.as_ref().is_some_and(|base| base.set.contains(&one)) && self.own.insert(one)
    }

    /// The slices, ascending.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            own: self.own.iter().peekable(),
            base: self.base.as_ref().map(|base| base.set.iter()).into_iter().flatten().peekable(),
        }
    }

    /// What is held by reference put into the set itself.
    pub fn materialize(&mut self) {
        if let Some(base) = self.base.take() {
            self.own.extend(base.set.iter().copied());
        }
    }

    /// `other`'s slices in this set: its shared run by reference where this set
    /// has none or has the same, else the slices themselves.
    pub fn absorb(
        &mut self,
        other: &SliceSet,
    ) {
        if let Some(theirs) = &other.base {
            match &self.base {
                None => {
                    self.own.retain(|one| !theirs.set.contains(one));
                    self.base = Some(Rc::clone(theirs));
                }
                Some(mine) if Rc::ptr_eq(mine, theirs) => {}
                Some(_) => {
                    self.own.extend(theirs.set.iter().copied().filter(|one| !self.contains(one)).collect::<Vec<_>>())
                }
            }
        }
        let fresh: Vec<Slice> = other.own.iter().copied().filter(|one| !self.contains(one)).collect();
        self.own.extend(fresh);
    }

    /// Those of its slices `keep` says. `outlives` says what it asks of a
    /// shared run as a whole, or `None` where only the slices can say.
    pub fn retain(
        &mut self,
        keep: impl Fn(&Slice) -> bool,
        shared_kept: impl Fn(&Shared) -> Option<bool>,
    ) {
        self.own.retain(|one| keep(one));
        if let Some(base) = &self.base {
            match shared_kept(base) {
                Some(true) => {}
                _ => {
                    self.materialize();
                    self.own.retain(|one| keep(one));
                }
            }
        }
    }

    /// `retain` the slices that outlive their function.
    pub fn retain_outliving(&mut self) {
        self.retain(
            |one| !matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack),
            |base| Some(base.outlives),
        );
    }

    /// Without a subrange of an object it has every byte of (`_coalesced`). The
    /// shared run loses slices only to a whole object of the set's own, and
    /// then the run is put into the set.
    pub fn coalesced(&self) -> SliceSet {
        let mut out = self.clone();
        let owned: BTreeSet<ObjectRef> = out.own.iter().filter(|one| whole(one)).map(|one| one.object).collect();
        if out
            .base
            .as_ref()
            .is_some_and(|base| !base.coalesced || owned.iter().any(|object| base.parts.contains(object)))
        {
            out.materialize();
        }
        let wholes: BTreeSet<ObjectRef> = match &out.base {
            Some(base) => owned.union(&base.wholes).copied().collect(),
            None => out.own.iter().filter(|one| whole(one)).map(|one| one.object).collect(),
        };
        out.own.retain(|one| !wholes.contains(&one.object) || whole(one));
        out
    }

    /// Whether the shared run, if there is one, has a parameter's slice.
    pub fn has_parameter_run(&self) -> bool {
        self.base.as_ref().is_some_and(|base| base.parameters)
    }

    /// Each slice `map` gives for each of its own, and the shared run as it is,
    /// which `map` must leave alone.
    pub fn mapped(
        &self,
        map: impl Fn(&Slice, &mut Vec<Slice>),
    ) -> SliceSet {
        let mut out = SliceSet { own: BTreeSet::new(), base: self.base.clone() };
        let mut found = Vec::new();
        for one in &self.own {
            map(one, &mut found);
            for made in found.drain(..) {
                out.insert(made);
            }
        }
        out
    }

    pub fn is_subset(
        &self,
        other: &SliceSet,
    ) -> bool {
        self.iter().all(|one| other.contains(one))
    }

    pub fn to_set(&self) -> BTreeSet<Slice> {
        self.iter().copied().collect()
    }

    pub fn base(&self) -> Option<&Rc<Shared>> {
        self.base.as_ref()
    }

    pub fn own(&self) -> &BTreeSet<Slice> {
        &self.own
    }
}

impl PartialEq for SliceSet {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        match (&self.base, &other.base) {
            (Some(one), Some(two)) if Rc::ptr_eq(one, two) => self.own == other.own,
            (None, None) => self.own == other.own,
            _ => self.len() == other.len() && self.iter().eq(other.iter()),
        }
    }
}

impl Eq for SliceSet {}

/// The slices in order, as `eq` compares them.
impl std::hash::Hash for SliceSet {
    fn hash<H: std::hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.len().hash(state);
        self.iter().for_each(|one| one.hash(state));
    }
}

impl From<BTreeSet<Slice>> for SliceSet {
    fn from(own: BTreeSet<Slice>) -> Self {
        Self { own, base: None }
    }
}

impl FromIterator<Slice> for SliceSet {
    fn from_iter<I: IntoIterator<Item = Slice>>(slices: I) -> Self {
        Self { own: slices.into_iter().collect(), base: None }
    }
}

impl Extend<Slice> for SliceSet {
    fn extend<I: IntoIterator<Item = Slice>>(
        &mut self,
        slices: I,
    ) {
        for one in slices {
            self.insert(one);
        }
    }
}

pub struct Iter<'a> {
    own: std::iter::Peekable<std::collections::btree_set::Iter<'a, Slice>>,
    base: std::iter::Peekable<std::iter::Flatten<std::option::IntoIter<std::collections::btree_set::Iter<'a, Slice>>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Slice;

    fn next(&mut self) -> Option<&'a Slice> {
        match (self.own.peek(), self.base.peek()) {
            (Some(one), Some(other)) => {
                if one < other {
                    self.own.next()
                } else {
                    self.base.next()
                }
            }
            (Some(_), None) => self.own.next(),
            (None, _) => self.base.next(),
        }
    }
}

impl<'a> IntoIterator for &'a SliceSet {
    type Item = &'a Slice;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl IntoIterator for SliceSet {
    type Item = Slice;
    type IntoIter = std::vec::IntoIter<Slice>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter().copied().collect::<Vec<_>>().into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Identity, MemoryObject, ObjectRef};

    fn object(
        number: i64,
        kind: MemoryKind,
    ) -> ObjectRef {
        ObjectRef::from(MemoryObject { identity: Some(Identity::Int(number)), ..MemoryObject::new(kind) })
    }

    fn part(
        number: i64,
        low: i64,
        high: i64,
        kind: MemoryKind,
    ) -> Slice {
        Slice::new(object(number, kind), low, high, 1, 1).expect("a slice")
    }

    /// Slices from a few objects, some whole, some in a frame or a parameter's.
    fn slices(seed: u64) -> Vec<Slice> {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move |most: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % most
        };
        (0..(next(14) + 1))
            .map(|_| {
                let kind = [MemoryKind::Global, MemoryKind::Global, MemoryKind::Frame, MemoryKind::Parameter]
                    [next(4) as usize];
                let number = next(5) as i64;
                if next(3) == 0 {
                    Slice::whole(object(number, kind))
                } else {
                    part(number, next(8) as i64, 8 + next(8) as i64, kind)
                }
            })
            .collect()
    }

    fn coalesced(items: &BTreeSet<Slice>) -> BTreeSet<Slice> {
        let wholes: BTreeSet<ObjectRef> = items.iter().filter(|one| whole(one)).map(|one| one.object).collect();
        items.iter().filter(|one| !wholes.contains(&one.object) || whole(one)).copied().collect()
    }

    /// A set holding a run by reference is the union: it lists, counts, finds,
    /// compares, absorbs, filters and coalesces as the tree set of the same
    /// slices does, whatever the run and the rest hold.
    #[test]
    fn a_set_with_a_shared_run_is_the_union_of_it_and_its_own() {
        for seed in 1..400u64 {
            let (run, rest, more) = (slices(seed), slices(seed + 1000), slices(seed + 2000));
            let shared = Shared::of(run.iter().copied().collect());
            let mut ours = SliceSet::shared(&shared);
            ours.extend(rest.iter().copied());
            let mut theirs: BTreeSet<Slice> = run.iter().chain(&rest).copied().collect();
            assert_eq!(ours.len(), theirs.len(), "{seed}");
            assert!(ours.iter().copied().eq(theirs.iter().copied()), "{seed}: ascending, once each");
            assert!(theirs.iter().all(|one| ours.contains(one)));

            let others: SliceSet = more.iter().copied().collect();
            ours.absorb(&others);
            theirs.extend(more.iter().copied());
            assert_eq!(ours.to_set(), theirs, "{seed}: absorbed");

            let mut kept = ours.clone();
            kept.retain_outliving();
            let outliving: BTreeSet<Slice> = theirs
                .iter()
                .filter(|one| !matches!(one.object.kind, MemoryKind::Frame | MemoryKind::Stack))
                .copied()
                .collect();
            assert_eq!(kept.to_set(), outliving, "{seed}: outliving");

            assert_eq!(kept.coalesced().to_set(), coalesced(&outliving), "{seed}: coalesced");
            assert_eq!(ours, SliceSet::from(theirs.clone()), "{seed}: equal as sets");
        }
    }

    /// Two sets holding one run compare their own slices alone; a run put into
    /// a set is the same slices.
    #[test]
    fn a_run_put_into_a_set_changes_nothing_it_holds() {
        let shared = Shared::of(slices(7).into_iter().collect());
        let mut held = SliceSet::shared(&shared);
        held.extend(slices(8));
        let mut put = held.clone();
        put.materialize();
        assert_eq!(held, put);
        assert!(put.base().is_none());
        assert_eq!(held.len(), put.len());
    }

    /// Two sets that are equal hash alike, held by reference or put into the
    /// set itself: the facts a body read are told by their hashes.
    #[test]
    fn sets_that_are_equal_hash_alike_held_by_reference_or_not() {
        use std::hash::{Hash, Hasher};
        let hashed = |set: &SliceSet| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            set.hash(&mut hasher);
            hasher.finish()
        };
        let mut set = SliceSet::new();
        for one in slices(3) {
            set.insert(one);
        }
        let shared = Rc::new(Shared::of(set.iter().copied().collect()));
        let held = SliceSet::shared(&shared);
        let mut put = held.clone();
        put.materialize();
        assert_eq!(held, put);
        assert_eq!(hashed(&held), hashed(&put));
    }
}
