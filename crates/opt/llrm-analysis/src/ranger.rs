//! The bounds the branch edges give a value at a block, worked out when asked
//! and kept by (value, block), in place of a table of every value at every
//! block (`ranges::edges_solved`).
//!
//! # How it works
//! GCC's ranger asks for a name's range on entry to a block and walks the
//! dominating edges back to its definition, filling a cache on the way
//! (`ranger_cache::range_on_entry`). The same here: a value's interval at a
//! block is what the block's own instructions compute of it, else what the
//! block starts from (its sole predecessor's, narrowed by the edge; or its
//! immediate dominator's), else what the program declares of it. Nothing is
//! solved for a value nobody asks of, and an answer is the one the table
//! gives: `LLRM_CHECK_REPLAY` asks every value at every block both ways.
//!
//! The walk up the blocks stops at the definition and does not start for a
//! block the definition does not dominate (nothing above holds the value), and
//! each block it passes keeps the answer, so a value is walked to once from
//! each block that asks, not once per question.
//!
//! # Example
//! `if x < 5 { y = x + 1 }`:
//!
//! ```text
//!   b0:  %c = icmp slt %x, 5        b1 (taken):     %x in [lo, 4]   %y in [lo+1, 5]
//!        br %c, b1, b2              b2 (not taken): %x in [5, hi]
//!                                   b3 (join):      nothing known of %x
//! ```
//!
//! From unit test `a_guard_bounds_the_value_on_its_edge_and_not_at_the_join` in
//! `ranger_tests.rs`.
//!
//! # Provides
//! [`EdgeRanger::at`].
//!
//! # Needs
//! [`crate::ranges`] (the transfer functions), [`crate::cfg`].
//!
//! # See also
//! GCC `gimple-range-cache.cc`; LLVM `LazyValueInfo::getValueInBlock`.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::module::{Operand, ValueDef, ValueId};
use llrm_support::hash::HashMap;
use llrm_support::hash::SparseIdMap;

use crate::cfg;
use crate::memory::{Knowns, Unit};
use crate::ranges::{self, Interval, Intervals, Lookup};

/// What the ranger has worked out of one body: the answers it keeps.
#[derive(Default)]
struct Memo {
    /// (value, block) -> what holds after the block's own instructions.
    own: RefCell<SparseIdMap<ValueId, HashMap<i64, Option<Rc<Interval>>>>>,
    /// (value, block) -> what holds at the start of the block (`inherited`).
    inherits: RefCell<SparseIdMap<ValueId, HashMap<i64, Option<Rc<Interval>>>>>,
    /// Block -> what the edge from its sole predecessor sets, where it has one.
    edges: RefCell<HashMap<i64, Rc<Option<Intervals>>>>,
    /// Block -> what each of its assumes sets below it, in order.
    assumes: RefCell<HashMap<i64, Rc<Vec<Intervals>>>>,
    /// Block -> the predecessor its edge is taken from.
    sole: RefCell<HashMap<i64, Option<i64>>>,
    failed: RefCell<Option<String>>,
    /// Blocks `inherited` has stepped through, over every question.
    steps: std::cell::Cell<u64>,
}

/// The edges' facts of a body as the manager keeps them: nothing is worked out
/// until a pass asks (`ranger`), and an edit drops only the answers it can
/// change.
#[derive(Clone, Default)]
pub struct EdgeFacts {
    memo: Rc<Memo>,
}

/// Two bodies' facts are equal: what they hold is of what was asked, and a
/// check compares the answers (`check`).
impl PartialEq for EdgeFacts {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl std::fmt::Debug for EdgeFacts {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(out, "EdgeFacts({} answers)", self.answers())
    }
}

impl EdgeFacts {
    /// The answers about `unit`'s body, over its registers.
    pub fn over<'a, 'u>(
        &self,
        unit: &'a Unit<'u>,
        facts: &'a Knowns,
    ) -> EdgeRanger<'a, 'u> {
        EdgeRanger::with(Rc::clone(&self.memo), unit, facts)
    }

    /// The blocks stepped through, over every question asked: the work of
    /// finding what a block starts from.
    pub fn steps(&self) -> u64 {
        self.memo.steps.get()
    }

    /// How many answers are held.
    pub fn answers(&self) -> usize {
        self.memo.own.borrow().values().map(|held| held.len()).sum()
    }

    /// Every block's facts, as the table gave them: asked of every value at
    /// every block, for a check or a test.
    pub fn blocks(
        &self,
        unit: &Unit,
        facts: &Knowns,
    ) -> llrm_support::hash::IndexMap<i64, Intervals> {
        use llrm_mir::dense::Dense;
        let ranger = self.over(unit, facts);
        let function = unit.function;
        let mut found = llrm_support::hash::IndexMap::default();
        for &block in function.layout() {
            let at = cfg::id(block);
            let scope: Intervals = (0..function.value_count())
                .filter_map(|index| {
                    let value = ValueId::at(index);
                    ranger.at(value, at).map(|interval| (value, (*interval).clone()))
                })
                .collect();
            if !scope.is_empty() {
                found.insert(at, scope);
            }
        }
        found
    }

    /// What is left of the answers once the blocks `keep` is false of are
    /// asked again. The answers are worked on in place where nobody else holds
    /// them.
    pub fn keeping(
        mut self,
        keep: impl Fn(i64) -> bool,
    ) -> EdgeFacts {
        // Each block is judged once, not once for each answer it has.
        let mut judged: HashMap<i64, bool> = HashMap::default();
        let mut kept = |at: i64| *judged.entry(at).or_insert_with(|| keep(at));
        match Rc::get_mut(&mut self.memo) {
            Some(memo) => {
                for held in memo.own.get_mut().values_mut() {
                    held.retain(|at, _| kept(*at));
                }
                for held in memo.inherits.get_mut().values_mut() {
                    held.retain(|at, _| kept(*at));
                }
                memo.edges.get_mut().retain(|at, _| kept(*at));
                memo.assumes.get_mut().retain(|at, _| kept(*at));
                memo.sole.get_mut().retain(|at, _| kept(*at));
                self
            }
            None => {
                let memo = Memo::default();
                for (value, held) in self.memo.own.borrow().iter() {
                    let stands: HashMap<i64, Option<Rc<Interval>>> =
                        held.iter().filter(|(at, _)| kept(**at)).map(|(at, found)| (*at, found.clone())).collect();
                    memo.own.borrow_mut().insert(*value, stands);
                }
                for (value, held) in self.memo.inherits.borrow().iter() {
                    let stands: HashMap<i64, Option<Rc<Interval>>> =
                        held.iter().filter(|(at, _)| kept(**at)).map(|(at, found)| (*at, found.clone())).collect();
                    memo.inherits.borrow_mut().insert(*value, stands);
                }
                *memo.edges.borrow_mut() = self
                    .memo
                    .edges
                    .borrow()
                    .iter()
                    .filter(|(at, _)| kept(**at))
                    .map(|(k, v)| (*k, Rc::clone(v)))
                    .collect();
                *memo.assumes.borrow_mut() = self
                    .memo
                    .assumes
                    .borrow()
                    .iter()
                    .filter(|(at, _)| kept(**at))
                    .map(|(k, v)| (*k, Rc::clone(v)))
                    .collect();
                *memo.sole.borrow_mut() =
                    self.memo.sole.borrow().iter().filter(|(at, _)| kept(**at)).map(|(k, v)| (*k, *v)).collect();
                EdgeFacts { memo: Rc::new(memo) }
            }
        }
    }
}

/// Edge facts of one body, asked of by value and block.
pub struct EdgeRanger<'a, 'u> {
    unit: &'a Unit<'u>,
    facts: &'a Knowns,
    shape: std::cell::OnceCell<Cow<'u, cfg::Shape>>,
    memo: Rc<Memo>,
}

/// What holds at `.1` once its instructions have run.
struct After<'r, 'a, 'u>(&'r EdgeRanger<'a, 'u>, i64);

impl Lookup for After<'_, '_, '_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        self.0.at(*value, self.1).map(|found| Cow::Owned((*found).clone()))
    }
}

/// What holds below `.1`: after it, and the assumes taken in order.
struct Below<'r, 'a, 'u> {
    ranger: &'r EdgeRanger<'a, 'u>,
    block: i64,
    assumed: &'r [Intervals],
}

impl Lookup for Below<'_, '_, '_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        self.assumed
            .iter()
            .rev()
            .find_map(|delta| delta.get(value))
            .map(Cow::Borrowed)
            .or_else(|| After(self.ranger, self.block).lookup(value).map(|found| Cow::Owned(found.into_owned())))
    }
}

impl<'a, 'u> EdgeRanger<'a, 'u> {
    pub fn new(
        unit: &'a Unit<'u>,
        facts: &'a Knowns,
    ) -> Self {
        Self::with(Rc::default(), unit, facts)
    }

    fn with(
        memo: Rc<Memo>,
        unit: &'a Unit<'u>,
        facts: &'a Knowns,
    ) -> Self {
        Self { unit, facts, shape: std::cell::OnceCell::new(), memo }
    }

    fn shape(&self) -> &cfg::Shape {
        self.shape.get_or_init(|| self.unit.shape())
    }

    /// The first error a transfer function gave, if one did: answers after it
    /// are not to be trusted.
    pub fn error(&self) -> Option<String> {
        self.memo.failed.borrow().clone()
    }

    /// What holds of `value` at block `at` after its instructions, as
    /// `ranges::edges_solved`'s `own` has it.
    pub fn at(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Rc<Interval>> {
        if let Some(found) = self.memo.own.borrow().get(&value).and_then(|held| held.get(&at)) {
            return found.clone();
        }
        let found = self.worked(value, at).map(Rc::new);
        self.memo.own.borrow_mut().entry(value).or_default().insert(at, found.clone());
        found
    }

    fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.unit.function.entry().map(cfg::id) == Some(at)
            || self.shape().dominance.immediate(self.unit.function, at).is_some()
    }

    fn worked(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Interval> {
        if !self.reachable(at) {
            return None;
        }
        let inherited = self.inherited(value, at);
        let function = self.unit.function;
        if let ValueDef::Instruction(inst) = function.value(value).def
            && function.parent(inst).map(cfg::id) == Some(at)
            && let Some(interval) = ranges::_computed(self.unit, inst, &After(self, at), self.facts)
        {
            let interval = match inherited.as_deref() {
                Some(previous) if previous.width == interval.width => Interval {
                    low: previous.low.clone().max(interval.low),
                    high: previous.high.clone().min(interval.high),
                    width: interval.width,
                },
                _ => interval,
            };
            if interval.low <= interval.high {
                return Some(interval);
            }
        }
        inherited.map(|found| (*found).clone())
    }

    /// What `value` holds at the start of `at`: walked up the blocks that start
    /// each from the one above, to the first that says something of it (an edge
    /// or an assume) or makes it, as GCC's `range_on_entry` finds the nearest
    /// block with one. Every block passed has the same answer and keeps it, so
    /// a block is passed once for a value.
    fn inherited(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Rc<Interval>> {
        let function = self.unit.function;
        let made_in = match function.value(value).def {
            ValueDef::Instruction(inst) => function.parent(inst).map(cfg::id),
            ValueDef::Argument(_) => None,
        };
        // Nothing above a definition holds the value, and a block the
        // definition does not dominate has not seen it.
        if made_in.is_some_and(|def| def == at || !self.shape().dominance.dominates(self.unit.function, def, at)) {
            return None;
        }
        let mut passed = Vec::new();
        let found = self.walked(value, at, made_in, &mut passed);
        if !passed.is_empty() {
            let mut held = self.memo.inherits.borrow_mut();
            let held = held.entry(value).or_default();
            for block in passed {
                held.insert(block, found.clone());
            }
        }
        found
    }

    /// `inherited`'s walk from `at`, with the blocks it passed.
    fn walked(
        &self,
        value: ValueId,
        at: i64,
        made_in: Option<i64>,
        passed: &mut Vec<i64>,
    ) -> Option<Rc<Interval>> {
        let mut here = at;
        loop {
            if let Some(found) = self.memo.inherits.borrow().get(&value).and_then(|held| held.get(&here)) {
                return found.clone();
            }
            passed.push(here);
            self.memo.steps.set(self.memo.steps.get() + 1);
            let above = match self.sole(here) {
                Some(parent) => {
                    let delta = self.edge(parent, here);
                    if let Some(found) = delta.as_ref().as_ref().and_then(|delta| delta.get(&value)) {
                        return Some(Rc::new(found.clone()));
                    }
                    parent
                }
                None => match self.shape().dominance.immediate(self.unit.function, here) {
                    Some(up) => up,
                    None if self.unit.function.entry().map(cfg::id) == Some(here)
                        && self.unit.function.parameters().contains(&value) =>
                    {
                        return ranges::declared_argument(self.unit, value).map(Rc::new);
                    }
                    None => return None,
                },
            };
            let assumed = self.assumed(above);
            if let Some(found) = assumed.iter().rev().find_map(|delta| delta.get(&value)) {
                return Some(Rc::new(found.clone()));
            }
            if made_in == Some(above) {
                return self.at(value, above);
            }
            here = above;
        }
    }

    /// The one predecessor whose edge `at` starts from, if it has one that
    /// names it once and comes before it.
    fn sole(
        &self,
        at: i64,
    ) -> Option<i64> {
        if let Some(found) = self.memo.sole.borrow().get(&at) {
            return *found;
        }
        let found = self.sole_of(at);
        self.memo.sole.borrow_mut().insert(at, found);
        found
    }

    fn sole_of(
        &self,
        at: i64,
    ) -> Option<i64> {
        if self.unit.function.entry().map(cfg::id) == Some(at) {
            return None;
        }
        let function = self.unit.function;
        let parents = function.predecessors(cfg::block(at));
        let [parent] = parents[..] else { return None };
        let parent = cfg::id(parent);
        let twice = function.terminator(cfg::block(parent)).is_some_and(|last| {
            function.instruction(last).operands.iter().filter(|one| **one == Operand::Block(cfg::block(at))).count() > 1
        });
        (!twice && self.reachable(parent)).then_some(parent)
    }

    fn edge(
        &self,
        parent: i64,
        at: i64,
    ) -> Rc<Option<Intervals>> {
        if let Some(found) = self.memo.edges.borrow().get(&at) {
            return Rc::clone(found);
        }
        let known = Below { ranger: self, block: parent, assumed: &self.assumed(parent) };
        let delta = match ranges::edge_delta_of(self.unit, cfg::block(parent), cfg::block(at), &known, Some(self.facts))
        {
            Ok(delta) => delta,
            Err(why) => {
                self.memo.failed.borrow_mut().get_or_insert(why);
                None
            }
        };
        let delta = Rc::new(delta);
        self.memo.edges.borrow_mut().insert(at, Rc::clone(&delta));
        delta
    }

    fn assumed(
        &self,
        at: i64,
    ) -> Rc<Vec<Intervals>> {
        if let Some(found) = self.memo.assumes.borrow().get(&at) {
            return Rc::clone(found);
        }
        let mut deltas: Vec<Intervals> = Vec::new();
        let conditions = self.unit.assumptions().here(at).to_vec();
        for condition in conditions {
            let delta = {
                let known = Below { ranger: self, block: at, assumed: &deltas };
                ranges::narrowed_delta(self.unit, condition, true, &known, self.facts)
            };
            if let Some(delta) = delta {
                deltas.push(delta);
            }
        }
        let deltas = Rc::new(deltas);
        self.memo.assumes.borrow_mut().insert(at, Rc::clone(&deltas));
        deltas
    }
}

/// `LLRM_CHECK_REPLAY`: every value at every block, asked of the ranger, is
/// what the table `states` holds there.
pub fn check(
    unit: &Unit,
    facts: &Knowns,
    states: &ranges::EdgeStates,
) {
    check_against(&EdgeRanger::new(unit, facts), unit, states);
}

/// `check`, of answers already kept.
pub fn check_against(
    ranger: &EdgeRanger,
    unit: &Unit,
    states: &ranges::EdgeStates,
) {
    use llrm_mir::dense::Dense;
    let function = unit.function;
    for &block in function.layout() {
        let at = cfg::id(block);
        for index in 0..function.value_count() {
            let value = ValueId::at(index);
            let table = states.scope_of(at).and_then(|scope| scope.get(&value));
            let asked = ranger.at(value, at);
            assert!(
                table == asked.as_deref(),
                "the ranger gives {asked:?} of {value:?} at block {at}, the table {table:?}"
            );
        }
    }
    assert!(ranger.error().is_none(), "the ranger failed where the table did not: {:?}", ranger.error());
}

#[cfg(test)]
#[path = "ranger_tests.rs"]
mod tests;
