// A loop's scopes: what `Bounds` knows at a block of it, made from the loop's
// `known` by the edges that narrow it there. The solve of the loop and the
// consumer that asks later make them the same way, from the same state kept
// with the loop (`Caches`).

use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_support::hash::{HashMap, IndexMap};

use super::{_computed, Interval, Intervals, Prefix, Prefixes, check_scopes, edge_delta, narrow, narrowed_delta};
use crate::cfg;
use crate::consts::Known;
use crate::graph::loops;
use crate::memory::Unit;

/// What every loop of a solve reads alike.
pub(super) struct Solve<'a, 'u> {
    pub unit: &'a Unit<'u>,
    pub facts: &'a IndexMap<ValueId, Known>,
    pub graph: &'a [cfg::Block],
    pub predecessors: &'a BTreeMap<i64, BTreeSet<i64>>,
    pub positions: &'a BTreeMap<i64, usize>,
    pub shape: &'a cfg::Shape,
    pub assumed: &'a crate::assumptions::Assumptions,
}

/// What the scopes of one loop keep for the next: the swept state, the prefixes
/// of its edge chains, what the edges above it narrow, how many blocks each
/// block of it dominates and the operations that touch each value.
#[derive(Default)]
pub(super) struct Caches {
    pub swept: RefCell<Option<(u64, Rc<Intervals>)>>,
    pub prefixes: RefCell<Prefixes>,
    pub outer_delta: RefCell<Option<Rc<Intervals>>>,
    pub led: OnceCell<HashMap<i64, usize>>,
    pub above_header: RefCell<Option<Rc<Vec<(usize, usize, i64)>>>>,
    pub touching: OnceCell<llrm_mir::dense::IdMap<ValueId, Vec<usize>>>,
}

/// One loop's scopes.
pub(super) struct LoopScope<'a, 'u> {
    pub solve: &'a Solve<'a, 'u>,
    pub loop_: &'a loops::Loop,
    pub operations: &'a [InstId],
    pub caches: &'a Caches,
}

impl LoopScope<'_, '_> {
    fn apply(
        &self,
        scoped: &mut Intervals,
        inst: InstId,
    ) -> Option<(ValueId, Option<Rc<Interval>>)> {
        let Solve { unit, facts, .. } = *self.solve;
        let function = self.solve.unit.function;
        #[cfg(test)]
        super::OPS_APPLIED.with(|count| count.set(count.get() + 1));
        let mut interval = _computed(unit, inst, scoped, facts)?;
        let result = function.instruction(inst).result.expect("_computed answers a result");
        if let Some(previous) = scoped.get(&result).filter(|previous| previous.width == interval.width) {
            let (low, high) = (previous.low.clone().max(interval.low), previous.high.clone().min(interval.high));
            if low > high {
                return None;
            }
            interval = Interval { low, high, width: interval.width };
        }
        Some((result, scoped.replace(result, interval)))
    }

    fn sweep(
        &self,
        mut scoped: Intervals,
    ) -> Intervals {
        let operations = self.operations;
        loop {
            // What each value set in this sweep held before it.
            let mut before = IndexMap::<ValueId, Option<Rc<Interval>>>::default();
            for &inst in operations {
                if let Some((result, previous)) = self.apply(&mut scoped, inst) {
                    before.entry(result).or_insert(previous);
                }
            }
            if before.iter().all(|(value, was)| scoped.get(value) == was.as_deref()) {
                return scoped;
            }
        }
    }

    fn touching(&self) -> &llrm_mir::dense::IdMap<ValueId, Vec<usize>> {
        self.caches
            .touching
            .get_or_init(
                || {
                    let (function, operations) = (self.solve.unit.function, self.operations);
                    let mut touching = llrm_mir::dense::IdMap::<ValueId, Vec<usize>>::new();
                    for (place, &inst) in operations.iter().enumerate() {
                        let op = function.instruction(inst);
                        for value in op
                            .operands
                            .iter()
                            .filter_map(|one| if let Operand::Value(value) = one { Some(*value) } else { None })
                            .chain(op.result)
                        {
                            let places = touching.get_or_insert_with(value, Vec::new);
                            if places.last() != Some(&place) {
                                places.push(place);
                            }
                        }
                    }
                    touching
                },
            )
    }

    fn propagate(
        &self,
        scoped: &mut Intervals,
        changed: &mut dyn Iterator<Item = ValueId>,
    ) {
        let operations = self.operations;
        let touching = self.touching();
        let mut queue: BTreeSet<usize> =
            changed.flat_map(|value| touching.get(&value).into_iter().flatten().copied().collect::<Vec<_>>()).collect();
        while let Some(place) = queue.pop_first() {
            if let Some((result, previous)) = self.apply(scoped, operations[place])
                && previous.as_deref() != scoped.get(&result)
            {
                queue.extend(touching.get(&result).into_iter().flatten().copied());
            }
        }
    }

    fn settle(
        &self,
        mut scoped: Intervals,
        known: &Intervals,
        id: u64,
    ) -> Intervals {
        let operations = self.operations;
        // The cheaper of the two by the work each counts: a sweep evaluates
        // every operation and then once more to see nothing
        // change (2 x operations); settling finds what the block's edges
        // narrowed and puts the swept facts over the rest (one
        // pass over `scoped`'s facts), then evaluates only what that
        // reaches.
        if 2 * operations.len() <= scoped.len() {
            return self.sweep(scoped);
        }
        let base = {
            let held =
                self.caches.swept.borrow().as_ref().filter(|(was, _)| *was == id).map(|(_, base)| Rc::clone(base));
            held.unwrap_or_else(|| {
                let base = Rc::new(self.sweep(known.clone()));
                *self.caches.swept.borrow_mut() = Some((id, Rc::clone(&base)));
                base
            })
        };
        let narrowed: llrm_mir::dense::IdSet<ValueId> = scoped
            .iter()
            .filter(|(value, interval)| known.get(&*value) != Some(*interval))
            .map(|(value, _)| value)
            .collect();
        for (value, interval) in base.iter() {
            if !narrowed.contains(&value) {
                scoped.insert(value, interval.clone());
            }
        }
        self.propagate(&mut scoped, &mut narrowed.iter());
        scoped
    }

    fn led_to(
        &self,
        block: i64,
    ) -> usize {
        let Solve { shape, .. } = *self.solve;
        let loop_ = self.loop_;
        self.caches
            .led
            .get_or_init(|| {
                let mut counts = HashMap::<i64, usize>::default();
                for &member in &loop_.body {
                    let mut above = Some(member);
                    while let Some(one) = above.filter(|one| loop_.body.contains(one)) {
                        *counts.entry(one).or_default() += 1;
                        above = shape.dominance.immediate(one);
                    }
                }
                counts
            })
            .get(&block)
            .copied()
            .unwrap_or(0)
    }

    fn edges_into(
        &self,
        one: i64,
        found: &mut Vec<(usize, usize, i64)>,
    ) {
        let Solve { graph, predecessors, positions, .. } = *self.solve;
        if let Some(parents) = predecessors.get(&one).filter(|parents| parents.len() == 1)
            && let Some(&from) = parents.iter().next().and_then(|parent| positions.get(parent))
        {
            for (index, &successor) in graph[from].succ.iter().enumerate() {
                if successor == one {
                    found.push((from, index, successor));
                }
            }
        }
    }

    fn outer_chain(&self) -> Rc<Vec<(usize, usize, i64)>> {
        let Solve { shape, .. } = *self.solve;
        let loop_ = self.loop_;
        Rc::clone(self.caches.above_header.borrow_mut().get_or_insert_with(|| {
            let mut found = Vec::new();
            let mut above = shape.dominance.immediate(loop_.header);
            while let Some(one) = above {
                self.edges_into(one, &mut found);
                above = shape.dominance.immediate(one);
            }
            found.sort_unstable();
            Rc::new(found)
        }))
    }

    pub(super) fn scope_at(
        &self,
        at: i64,
        known: &Intervals,
        id: u64,
        every_block: bool,
    ) -> Result<Rc<Intervals>, String> {
        super::SCOPES_MADE.with(|count| count.set(count.get() + 1));
        let Solve { unit, facts, graph, shape, assumed, .. } = *self.solve;
        let (loop_, operations) = (self.loop_, self.operations);
        let mut scoped = known.clone();
        // The edges into a block with no other way in, from a block that
        // dominates `at`: those of the dominator chain of `at`,
        // in the order of the blocks' layout, each narrowing in place.
        // Walked up the loop only: the edges above its header are the same
        // for every block of it, found once.
        let mut chain = Vec::new();
        let mut above = Some(at);
        while let Some(one) = above.filter(|one| loop_.body.contains(one)) {
            self.edges_into(one, &mut chain);
            above = shape.dominance.immediate(one);
        }
        chain.sort_unstable();
        let outer_edges = self.outer_chain();
        // The edges from outside the loop read only values made outside it,
        // which `known` holds as it did the last time: what
        // they narrow is kept, and only the edges inside the loop are
        // worked out again. That needs those edges to come first in the
        // order of the layout and all the rest to be inside the loop.
        let separable = chain.iter().all(|(from, _, _)| loop_.body.contains(&graph[*from].at))
            && outer_edges.last().zip(chain.first()).is_none_or(|(last, first)| last < first);
        let (outer_edges, chain) = if separable {
            (outer_edges, chain)
        } else {
            let mut every = Vec::new();
            let mut above = Some(at);
            while let Some(one) = above {
                self.edges_into(one, &mut every);
                above = shape.dominance.immediate(one);
            }
            every.sort_unstable();
            let outer = every.iter().take_while(|(from, _, _)| !loop_.body.contains(&graph[*from].at)).count();
            let rest = every.split_off(outer);
            (Rc::new(every), rest)
        };
        let narrow_by = |edges: &[(usize, usize, i64)], scoped: &mut Intervals| -> Result<(), String> {
            for &(from, _, successor) in edges {
                if let Some(delta) =
                    edge_delta(unit, cfg::block(graph[from].at), cfg::block(successor), scoped, Some(&facts))?
                {
                    scoped.extend(delta);
                }
            }
            Ok(())
        };
        let outer_narrowing = |edges: &[(usize, usize, i64)], known: &Intervals| -> Result<Rc<Intervals>, String> {
            if let Some(found) = self.caches.outer_delta.borrow().as_ref() {
                return Ok(Rc::clone(found));
            }
            let mut narrowed = known.clone();
            narrow_by(edges, &mut narrowed)?;
            let mut delta = Intervals::default();
            for (value, interval) in narrowed.iter() {
                if known.get(&value) != Some(interval) {
                    delta.insert(value, interval.clone());
                }
            }
            let delta = Rc::new(delta);
            *self.caches.outer_delta.borrow_mut() = Some(Rc::clone(&delta));
            Ok(delta)
        };
        // A prefix of the edges inside the loop is kept where the blocks it
        // leads to are more than the passes over the state it
        // costs to keep: a copy of the parent's, the edge, a diff against
        // it for what the edge changed, and a copy of the
        // parent's swept state to put that over. Fewer, and applying
        // the edges one after another is the cheaper.
        const TRIE_PASSES: usize = 4;
        // Only a `known` every block of the loop is asked of in turn has
        // prefixes that blocks share: the few latches a round
        // of boxes asks of would pay for a trie and use none of it.
        if separable && every_block {
            let mut held = self.caches.prefixes.borrow_mut();
            if held.known != id {
                *held = Prefixes { known: id, ..Prefixes::default() };
            }
            let root = match held.roots.get(&[][..]).copied() {
                Some(root) => root,
                None => {
                    let mut narrowed = known.clone();
                    narrowed.extend(outer_narrowing(&outer_edges, known)?.iter().map(|(v, i)| (v, i.clone())));
                    held.nodes.push(Prefix {
                        state: Rc::new(narrowed),
                        next: HashMap::default(),
                        parent: None,
                        delta: Vec::new(),
                        settled: None,
                    });
                    let root = held.nodes.len() - 1;
                    held.roots.insert(Vec::new(), root);
                    root
                }
            };
            let mut node = root;
            let mut kept = 0;
            for edge in &chain {
                node = match held.nodes[node].next.get(edge).copied() {
                    Some(next) => next,
                    None => {
                        if self.led_to(edge.2) < TRIE_PASSES {
                            break;
                        }
                        let mut state = (*held.nodes[node].state).clone();
                        narrow_by(std::slice::from_ref(edge), &mut state)?;
                        let delta = state
                            .iter()
                            .filter(|(value, interval)| held.nodes[node].state.get(&*value) != Some(*interval))
                            .map(|(value, _)| value)
                            .collect();
                        held.nodes.push(Prefix {
                            state: Rc::new(state),
                            next: HashMap::default(),
                            parent: Some(node),
                            delta,
                            settled: None,
                        });
                        let next = held.nodes.len() - 1;
                        held.nodes[node].next.insert(*edge, next);
                        next
                    }
                };
                kept += 1;
            }
            scoped = (*held.nodes[node].state).clone();
            let rest = &chain[kept..];
            narrow_by(rest, &mut scoped)?;
            // A block no assume narrows, all of whose edges are kept, is
            // settled from its parent prefix's, with the
            // one edge's values put over it: sweeping the narrowed state is
            // that state's fixpoint, and the fixpoint of a
            // state between this one and its fixpoint is the same.
            if rest.is_empty()
                && kept > 0
                && 2 * operations.len() > scoped.len()
                && assumed.above(&shape, at).is_empty()
            {
                let mut path = Vec::new();
                let mut at_node = Some(node);
                while let Some(one) = at_node.filter(|one| held.nodes[*one].settled.is_none()) {
                    path.push(one);
                    at_node = held.nodes[one].parent;
                }
                for one in path.into_iter().rev() {
                    let settled = match held.nodes[one].parent {
                        None => self.settle(held.nodes[one].state.as_ref().clone(), known, id),
                        Some(parent) => {
                            let mut y = held.nodes[parent]
                                .settled
                                .as_ref()
                                .expect("the parent is settled first")
                                .as_ref()
                                .clone();
                            for value in &held.nodes[one].delta {
                                narrow(
                                    &mut y,
                                    *value,
                                    held.nodes[one].state.get(value).expect("a delta value has a state").clone(),
                                );
                            }
                            self.propagate(&mut y, &mut held.nodes[one].delta.iter().copied());
                            y
                        }
                    };
                    held.nodes[one].settled = Some(Rc::new(settled));
                }
                let settled = Rc::clone(held.nodes[node].settled.as_ref().expect("settled"));
                if check_scopes() {
                    let whole = self.sweep(scoped.clone());
                    if whole != *settled {
                        let diff: Vec<String> = whole
                            .iter()
                            .filter(|(v, i)| settled.get(&*v) != Some(*i))
                            .map(|(v, i)| {
                                format!(
                                    "{:?}: sweep [{},{}] vs incremental {:?}",
                                    v,
                                    i.low,
                                    i.high,
                                    settled.get(&v).map(|x| (x.low.to_string(), x.high.to_string()))
                                )
                            })
                            .chain(
                                settled
                                    .iter()
                                    .filter(|(v, _)| !whole.contains_key(&*v))
                                    .map(|(v, i)| format!("{:?}: only incremental [{},{}]", v, i.low, i.high)),
                            )
                            .collect();
                        panic!(
                            "a block settled from its parent prefix's is not what sweeping its edges' state gives: {} (known {}, state {}, ops {})",
                            diff.join("; "),
                            known.len(),
                            scoped.len(),
                            operations.len()
                        );
                    }
                }
                return Ok(settled);
            }
        } else if separable {
            scoped.extend(outer_narrowing(&outer_edges, known)?.iter().map(|(v, i)| (v, i.clone())));
            narrow_by(&chain, &mut scoped)?;
        } else {
            narrow_by(&outer_edges, &mut scoped)?;
            narrow_by(&chain, &mut scoped)?;
        }
        if check_scopes() {
            let mut every = known.clone();
            narrow_by(&outer_edges, &mut every)?;
            narrow_by(&chain, &mut every)?;
            assert!(every.iter().eq(scoped.iter()), "the edges above a loop narrow `known` as they did before");
        }
        // What the blocks above it assume.
        for condition in assumed.above(&shape, at) {
            if let Some(delta) = narrowed_delta(unit, condition, true, &scoped, facts) {
                scoped.extend(delta);
            }
        }
        if check_scopes() {
            let whole = self.sweep(scoped.clone());
            let quick = self.settle(scoped, known, id);
            assert!(
                whole.iter().eq(quick.iter()),
                "the operations a block's edges reach, worked again alone, give what sweeping them all does"
            );
            return Ok(Rc::new(quick));
        }
        Ok(Rc::new(self.settle(scoped, known, id)))
    }
}
