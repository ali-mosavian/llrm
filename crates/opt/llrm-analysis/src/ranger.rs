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
//! A loop's blocks are asked the same way (`LoopView`): the facts the edges and
//! assumes above a block give, and the loop's operations worked from them, one
//! value at a block. An answer keeps the boxed values it read, so the next
//! round of `ranges::inductive_boxes` works out only what a changed box
//! reaches.
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
//! [`EdgeRanger::at`], [`LoopView::at`].
//!
//! # Needs
//! [`crate::ranges`] (the transfer functions), [`crate::cfg`].
//!
//! # See also
//! GCC `gimple-range-cache.cc`; LLVM `LazyValueInfo::getValueInBlock`.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::module::{InstId, Operand, ValueDef, ValueId};
use llrm_support::hash::HashMap;
use llrm_support::hash::SparseIdMap;

use crate::cfg;
use crate::memory::{Knowns, Unit};
use crate::ranges::{self, Interval, Intervals, Lookup};

thread_local! {
    static VIEW_EDGES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many edges this thread's loop views have worked out, for a test that a
/// box search reads the edges inside its loop and not those above it.
pub fn view_edges() -> usize {
    VIEW_EDGES.with(std::cell::Cell::get)
}

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

/// What a loop's blocks are asked of: the body above, the loop's operations
/// and the graph, shared by every `LoopView` of it.
pub struct LoopContext<'a, 'u> {
    pub unit: &'a Unit<'u>,
    pub facts: &'a Knowns,
    pub shape: &'a cfg::Shape,
    pub assumed: &'a crate::assumptions::Assumptions,
    pub graph: &'a [cfg::Block],
    pub predecessors: &'a llrm_support::graph::Predecessors,
    pub positions: &'a llrm_mir::dense::IdMap<i64, usize>,
    /// The loop's operations that compute an interval.
    pub operations: &'a [InstId],
    /// The blocks of the loop.
    pub body: &'a llrm_mir::forest::Body,
    /// What the edges above the loop say of a value made outside it.
    pub above: &'a dyn Fn(ValueId) -> Option<Interval>,
    made: std::cell::OnceCell<SparseIdMap<ValueId, InstId>>,
}

impl<'a, 'u> LoopContext<'a, 'u> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        unit: &'a Unit<'u>,
        facts: &'a Knowns,
        shape: &'a cfg::Shape,
        assumed: &'a crate::assumptions::Assumptions,
        graph: &'a [cfg::Block],
        predecessors: &'a llrm_support::graph::Predecessors,
        positions: &'a llrm_mir::dense::IdMap<i64, usize>,
        operations: &'a [InstId],
        body: &'a llrm_mir::forest::Body,
        above: &'a dyn Fn(ValueId) -> Option<Interval>,
    ) -> Self {
        Self {
            unit,
            facts,
            shape,
            assumed,
            graph,
            predecessors,
            positions,
            operations,
            body,
            above,
            made: Default::default(),
        }
    }

    /// The value each operation makes, found when first asked.
    fn makers(&self) -> &SparseIdMap<ValueId, InstId> {
        self.made.get_or_init(|| {
            let mut found = SparseIdMap::default();
            for &inst in self.operations {
                if let Some(result) = self.unit.function.instruction(inst).result {
                    found.insert(result, inst);
                }
            }
            found
        })
    }
}

/// An answer, the round it was worked out in, and the boxed values it read,
/// directly or through the answers it was made of.
struct Held<T> {
    answer: T,
    round: u32,
    reads: Rc<Vec<ValueId>>,
}

/// What `scope_at` gives a block of a loop, one value at a time: `known` and
/// the boxes assumed, closed under the operations, narrowed by the edges and
/// assumes above the block, and the operations worked from that
/// (`ranges::bounded_solved`'s sweep). The operations of a loop make no cycle
/// but through a phi, which they do not compute, so a value's interval is its
/// own base and what its operands' give, found when asked.
///
/// The boxes change from round to round. An answer keeps the boxed values it
/// read (as an on-entry cache entry keeps the names it was made of), and
/// stands in the next round unless one of them changed: the edges above a latch
/// mostly do not read the loop's phis.
pub struct LoopView<'r, 'a, 'u> {
    ctx: &'r LoopContext<'a, 'u>,
    known: &'r Intervals,
    boxes: RefCell<Intervals>,
    round: std::cell::Cell<u32>,
    /// The round each box last changed in.
    changed: RefCell<SparseIdMap<ValueId, u32>>,
    /// The boxed values read by the answers being worked out, innermost last.
    reading: RefCell<Vec<Vec<ValueId>>>,
    closed: RefCell<SparseIdMap<ValueId, Held<Option<Rc<Interval>>>>>,
    layers: RefCell<HashMap<i64, Held<Rc<Vec<Intervals>>>>>,
    swept: RefCell<SparseIdMap<ValueId, HashMap<i64, Held<Option<Rc<Interval>>>>>>,
    failed: RefCell<Option<String>>,
}

/// `known` closed under the operations: what is known, else what an operation
/// makes of its operands'.
struct Closed<'v, 'r, 'a, 'u>(&'v LoopView<'r, 'a, 'u>);

impl Lookup for Closed<'_, '_, '_, '_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        self.0.closed_of(*value).map(|found| Cow::Owned((*found).clone()))
    }
}

/// `Closed`, under the deltas of the edges and assumes so far.
struct Layered<'v, 'r, 'a, 'u> {
    view: &'v LoopView<'r, 'a, 'u>,
    deltas: &'v [Intervals],
}

impl Lookup for Layered<'_, '_, '_, '_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        match self.deltas.iter().rev().find_map(|delta| delta.get(value)) {
            Some(found) => Some(Cow::Borrowed(found)),
            None => self.view.closed_of(*value).map(|found| Cow::Owned((*found).clone())),
        }
    }
}

/// What holds at block `.1`, the operations swept.
pub struct Swept<'v, 'r, 'a, 'u>(&'v LoopView<'r, 'a, 'u>, i64);

impl Lookup for Swept<'_, '_, '_, '_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        self.0.at(*value, self.1).map(|found| Cow::Owned((*found).clone()))
    }
}

/// What holds on leaving a block along an edge: the edge's facts over those at
/// the block.
pub struct Leaving<'v, 'r, 'a, 'u> {
    pub delta: &'v Intervals,
    pub at: Swept<'v, 'r, 'a, 'u>,
}

impl Lookup for Leaving<'_, '_, '_, '_> {
    fn lookup(
        &self,
        value: &ValueId,
    ) -> Option<Cow<'_, Interval>> {
        match self.delta.get(value) {
            Some(found) => Some(Cow::Borrowed(found)),
            None => self.at.lookup(value),
        }
    }
}

impl<'r, 'a, 'u> LoopView<'r, 'a, 'u> {
    pub fn new(
        ctx: &'r LoopContext<'a, 'u>,
        known: &'r Intervals,
    ) -> Self {
        Self {
            ctx,
            known,
            boxes: RefCell::default(),
            round: std::cell::Cell::new(0),
            changed: RefCell::default(),
            reading: RefCell::default(),
            closed: RefCell::default(),
            layers: RefCell::default(),
            swept: RefCell::default(),
            failed: RefCell::default(),
        }
    }

    pub fn error(&self) -> Option<String> {
        self.failed.borrow().clone()
    }

    /// The next round, over `boxes`: what answers read of the boxes that
    /// changed is worked out again.
    pub fn assume(
        &self,
        boxes: &Intervals,
    ) {
        let round = self.round.get() + 1;
        self.round.set(round);
        let mut held = self.boxes.borrow_mut();
        let mut changed = self.changed.borrow_mut();
        for (value, interval) in boxes.iter() {
            if held.get(&value) != Some(interval) {
                changed.insert(value, round);
            }
        }
        for (value, _) in held.iter() {
            if !boxes.contains_key(&value) {
                changed.insert(value, round);
            }
        }
        *held = boxes.clone();
    }

    pub fn swept_at(
        &self,
        at: i64,
    ) -> Swept<'_, 'r, 'a, 'u> {
        Swept(self, at)
    }

    /// Whether nothing `reads` names changed since `round`.
    fn stands(
        &self,
        round: u32,
        reads: &[ValueId],
    ) -> bool {
        let changed = self.changed.borrow();
        reads.iter().all(|value| changed.get(value).is_none_or(|&at| at <= round))
    }

    fn note(
        &self,
        reads: &[ValueId],
    ) {
        if let Some(top) = self.reading.borrow_mut().last_mut() {
            top.extend_from_slice(reads);
        }
    }

    fn begin(&self) {
        self.reading.borrow_mut().push(Vec::new());
    }

    fn end(&self) -> Rc<Vec<ValueId>> {
        let mut reads = self.reading.borrow_mut().pop().unwrap_or_default();
        reads.sort_unstable_by_key(|value| llrm_mir::dense::Dense::index(*value));
        reads.dedup();
        self.note(&reads);
        Rc::new(reads)
    }

    fn closed_of(
        &self,
        value: ValueId,
    ) -> Option<Rc<Interval>> {
        if let Some(found) = self.boxes.borrow().get(&value) {
            self.note(&[value]);
            return Some(Rc::new(found.clone()));
        }
        let outside = self.ctx.makers().get(&value).is_none().then(|| (self.ctx.above)(value)).flatten();
        if let Some(found) = self.known.get(&value) {
            // Narrowed by what the edges above the loop say of it.
            return Some(Rc::new(match outside.filter(|one| one.width == found.width) {
                Some(above) => {
                    let (low, high) = (found.low.clone().max(above.low), found.high.clone().min(above.high));
                    if low > high { found.clone() } else { Interval { low, high, width: found.width } }
                }
                None => found.clone(),
            }));
        }
        if let Some(above) = outside {
            return Some(Rc::new(above));
        }
        let kept = self
            .closed
            .borrow()
            .get(&value)
            .filter(|held| self.stands(held.round, &held.reads))
            .map(|held| (held.answer.clone(), Rc::clone(&held.reads)));
        if let Some((answer, reads)) = kept {
            self.note(&reads);
            return answer;
        }
        self.begin();
        let made = self
            .ctx
            .makers()
            .get(&value)
            .and_then(|&inst| ranges::_computed(self.ctx.unit, inst, &Closed(self), self.ctx.facts))
            .map(Rc::new);
        let reads = self.end();
        self.closed.borrow_mut().insert(value, Held { answer: made.clone(), round: self.round.get(), reads });
        made
    }

    /// The edges into blocks with no other way in, on the dominators of `at`
    /// inside the loop, in the order of the layout. The edges above the loop
    /// narrow only values made outside it, which `known` holds narrowed by them
    /// already (`bounded_solved` reads each at the loop's dominator), so they
    /// are not worked out again for each loop.
    fn chain(
        &self,
        at: i64,
    ) -> Vec<(usize, usize, i64)> {
        let mut found = Vec::new();
        let mut above = Some(at).filter(|one| self.ctx.body.contains(one));
        while let Some(one) = above {
            if let Some(parents) = self.ctx.predecessors.get(&one).filter(|parents| parents.len() == 1)
                && let Some(&from) = parents.iter().next().and_then(|parent| self.ctx.positions.get(parent))
            {
                for (index, &successor) in self.ctx.graph[from].succ.iter().enumerate() {
                    if successor == one {
                        found.push((from, index, successor));
                    }
                }
            }
            above =
                self.ctx.shape.dominance.immediate(self.ctx.unit.function, one).filter(|up| self.ctx.body.contains(up));
        }
        found.sort_unstable();
        found
    }

    /// What the edges and assumes above `at` set, one delta after another.
    fn layers(
        &self,
        at: i64,
    ) -> Rc<Vec<Intervals>> {
        let kept = self
            .layers
            .borrow()
            .get(&at)
            .filter(|held| self.stands(held.round, &held.reads))
            .map(|held| (Rc::clone(&held.answer), Rc::clone(&held.reads)));
        if let Some((answer, reads)) = kept {
            self.note(&reads);
            return answer;
        }
        self.begin();
        let ctx = self.ctx;
        let mut deltas: Vec<Intervals> = Vec::new();
        for (from, _, successor) in self.chain(at) {
            VIEW_EDGES.with(|count| count.set(count.get() + 1));
            let delta = ranges::edge_delta(
                ctx.unit,
                cfg::block(ctx.graph[from].at),
                cfg::block(successor),
                &Layered { view: self, deltas: &deltas },
                Some(ctx.facts),
            );
            match delta {
                Ok(Some(delta)) => deltas.push(delta),
                Ok(None) => {}
                Err(why) => {
                    self.failed.borrow_mut().get_or_insert(why);
                }
            }
        }
        for condition in ctx.assumed.above(ctx.shape, ctx.unit.function, at) {
            if let Some(delta) =
                ranges::narrowed_delta(ctx.unit, condition, true, &Layered { view: self, deltas: &deltas }, ctx.facts)
            {
                deltas.push(delta);
            }
        }
        let reads = self.end();
        let deltas = Rc::new(deltas);
        self.layers.borrow_mut().insert(at, Held { answer: Rc::clone(&deltas), round: self.round.get(), reads });
        deltas
    }

    /// What `value` holds at block `at`, as sweeping the operations over
    /// `known` narrowed by the edges and assumes above gives.
    pub fn at(
        &self,
        value: ValueId,
        at: i64,
    ) -> Option<Rc<Interval>> {
        let kept = self
            .swept
            .borrow()
            .get(&value)
            .and_then(|held| held.get(&at))
            .filter(|held| self.stands(held.round, &held.reads))
            .map(|held| (held.answer.clone(), Rc::clone(&held.reads)));
        if let Some((answer, reads)) = kept {
            self.note(&reads);
            return answer;
        }
        self.begin();
        let layers = self.layers(at);
        let base = match layers.iter().rev().find_map(|delta| delta.get(&value)) {
            Some(found) => Some(Rc::new(found.clone())),
            None => self.closed_of(value),
        };
        let found = match self.ctx.makers().get(&value) {
            Some(&inst) => match ranges::_computed(self.ctx.unit, inst, &Swept(self, at), self.ctx.facts) {
                Some(interval) => match base.as_deref().filter(|previous| previous.width == interval.width) {
                    Some(previous) => {
                        let (low, high) =
                            (previous.low.clone().max(interval.low), previous.high.clone().min(interval.high));
                        if low > high { base } else { Some(Rc::new(Interval { low, high, width: interval.width })) }
                    }
                    None => Some(Rc::new(interval)),
                },
                None => base,
            },
            None => base,
        };
        let reads = self.end();
        self.swept
            .borrow_mut()
            .entry(value)
            .or_default()
            .insert(at, Held { answer: found.clone(), round: self.round.get(), reads });
        found
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
