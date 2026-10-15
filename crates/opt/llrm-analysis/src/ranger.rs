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

use llrm_mir::module::{BlockId, Operand, ValueDef, ValueId};
use llrm_support::hash::{HashMap, IndexMap};

use crate::cfg;
use crate::consts::Known;
use crate::memory::Unit;
use crate::ranges::{self, Interval, Intervals, Lookup};

/// Edge facts of one body, asked of by value and block.
pub struct EdgeRanger<'a, 'u> {
    unit: &'a Unit<'u>,
    facts: &'a IndexMap<ValueId, Known>,
    entry: Option<BlockId>,
    /// (value, block) -> what holds after the block's own instructions.
    own: RefCell<HashMap<(ValueId, i64), Option<Rc<Interval>>>>,
    /// Block -> what the edge from its sole predecessor sets, where it has one.
    edges: RefCell<HashMap<i64, Rc<Option<Intervals>>>>,
    /// Block -> what each of its assumes sets below it, in order.
    assumes: RefCell<HashMap<i64, Rc<Vec<Intervals>>>>,
    failed: RefCell<Option<String>>,
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
        facts: &'a IndexMap<ValueId, Known>,
    ) -> Self {
        Self {
            unit,
            facts,
            entry: unit.function.entry(),
            own: RefCell::default(),
            edges: RefCell::default(),
            assumes: RefCell::default(),
            failed: RefCell::default(),
        }
    }

    /// The first error a transfer function gave, if one did: answers after it
    /// are not to be trusted.
    pub fn error(&self) -> Option<String> {
        self.failed.borrow().clone()
    }

    /// What holds of `value` at block `at` after its instructions, as
    /// `ranges::edges_solved`'s `own` has it.
    pub fn at(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Rc<Interval>> {
        if let Some(found) = self.own.borrow().get(&(value, at)) {
            return found.clone();
        }
        let found = self.worked(value, at).map(Rc::new);
        self.own.borrow_mut().insert((value, at), found.clone());
        found
    }

    fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.entry.map(cfg::id) == Some(at) || self.unit.shape().dominance.immediate(at).is_some()
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

    /// What `value` holds at the start of `at`.
    fn inherited(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Rc<Interval>> {
        let parent = self.sole(at);
        if let Some(parent) = parent {
            let delta = self.edge(parent, at);
            if let Some(found) = delta.as_ref().as_ref().and_then(|delta| delta.get(&value)) {
                return Some(Rc::new(found.clone()));
            }
            return self.below(value, parent);
        }
        match self.unit.shape().dominance.immediate(at) {
            Some(up) => self.below(value, up),
            None if self.entry.map(cfg::id) == Some(at) => ranges::declared_argument(self.unit, value).map(Rc::new),
            None => None,
        }
    }

    /// The one predecessor whose edge `at` starts from, if it has one that
    /// names it once and comes before it.
    fn sole(
        &self,
        at: i64,
    ) -> Option<i64> {
        if self.entry.map(cfg::id) == Some(at) {
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
        if let Some(found) = self.edges.borrow().get(&at) {
            return Rc::clone(found);
        }
        let known = Below { ranger: self, block: parent, assumed: &self.assumed(parent) };
        let delta = match ranges::edge_delta_of(self.unit, cfg::block(parent), cfg::block(at), &known, Some(self.facts))
        {
            Ok(delta) => delta,
            Err(why) => {
                self.failed.borrow_mut().get_or_insert(why);
                None
            }
        };
        let delta = Rc::new(delta);
        self.edges.borrow_mut().insert(at, Rc::clone(&delta));
        delta
    }

    fn below(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Rc<Interval>> {
        let assumed = self.assumed(at);
        match assumed.iter().rev().find_map(|delta| delta.get(&value)) {
            Some(found) => Some(Rc::new(found.clone())),
            None => self.at(value, at),
        }
    }

    fn assumed(
        &self,
        at: i64,
    ) -> Rc<Vec<Intervals>> {
        if let Some(found) = self.assumes.borrow().get(&at) {
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
        self.assumes.borrow_mut().insert(at, Rc::clone(&deltas));
        deltas
    }
}

/// `LLRM_CHECK_REPLAY`: every value at every block, asked of the ranger, is
/// what the table `states` holds there.
pub fn check(
    unit: &Unit,
    facts: &IndexMap<ValueId, Known>,
    states: &ranges::EdgeStates,
) {
    use llrm_mir::dense::Dense;
    let ranger = EdgeRanger::new(unit, facts);
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
