//! Loop distribution: a loop whose stores do not need each other's trips
//! becomes one loop for each group of them, so each loop is smaller, holds
//! fewer values live and may be an idiom (a fill, a copy) of its own.
//!
//! # How it works
//! A candidate is a counted loop in canonical form
//! ([`llrm_analysis::cfg::Canonical`]) whose one exit is where the loop tests
//! (`loopshape::controls`), whose body is one path of blocks with no call and
//! nothing that may trap, whose header, if it tests before each trip, touches
//! no memory (it runs once more than the body), and from which no value is
//! read outside (`loopshape`). Its accesses are affine in the counter
//! ([`llrm_analysis::access`]).
//!
//! Each store seeds a group. The loads a store's value comes from, through any
//! chain of arithmetic and phis, are in its group, so a load two stores read
//! joins them: values pass between loops only through memory. Two accesses of
//! one body, at least one a store, order their groups where they meet
//! (`dependence::meetings`): `first` meeting `second` at an earlier or the
//! same trip puts `first`'s group before `second`'s, a later trip the other
//! way, and both ways one group. Accesses through roots that may be one object
//! (`access::Apart`) meet both ways. The strong components of the groups are
//! the partitions, in an order that keeps every edge forward, the first store
//! first where free: LLVM's `InstPartitionContainer` with its dependence
//! graph.
//!
//! Splitting a loop costs a loop's control, so it pays only for a partition
//! that is no loop: one `loopidioms` takes whole (`loopidioms::takes`: a fill
//! of a value the loop does not change, or a copy), as in gcc's pattern
//! distribution. Those stand alone and what lies between them is one loop, as
//! it was.
//!
//! ```text
//!   for i: a[i] = 0; b[i] = c[i] * 3        memset(a); for i: b[i] = c[i] * 3
//! ```
//!
//! Cheap checks come before the copy: what the idioms' stores and their
//! operands cost, as often as they run (an upper bound on what a split saves),
//! must be `MIN_PERCENT` of the body's price. The loop is then copied once for
//! each partition but the last ([`crate::loopclone`]'s `copied`), each copy
//! keeps its partition's stores and the original the last's, `loopidioms` runs
//! over the parts and what nothing reads is removed, and the result is priced
//! beside the body. Only then is the pipeline run over it, as in
//! [`crate::loopversion`], and [`crate::candidate`] keeps it when its price,
//! spills included, is `MIN_PERCENT` smaller (`Gain::CheaperBy`). At most
//! `MAX_TRIALS` loop is tried per body, the most nested, a loop of at most
//! `MAX_LOOP_OPS` operations and `MAX_ACCESSES` accesses, in at most
//! `MAX_PARTITIONS` partitions.
//!
//! `LLRM_DEBUG=distribute` prints each loop split and why each other loop
//! was not.
//!
//! # Example
//! Before:
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
//!   %more = icmp slt i16 %i, %n
//!   br i1 %more, label %b2, label %b3
//!
//! b2:
//!   %o = mul i16 %i, 2
//!   %pa = getelementptr i8, ptr %a, i16 %o
//!   store i16 0, ptr %pa
//!   %pc = getelementptr i8, ptr %c, i16 %o
//!   %x = load i16, ptr %pc
//!   %y = mul i16 %x, 3
//!   %pb = getelementptr i8, ptr %b, i16 %o
//!   store i16 %y, ptr %pb
//!   %i.next = add i16 %i, 1
//!   br label %b1
//! ```
//! After, the loop of the store to `a` and the loop of the one to `b`:
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
//!   ; ... the test of %n
//! b2:
//!   store i16 0, ptr %pa
//!   ; ... the counter
//!
//! 3:
//!   %4 = phi i16 [ 0, %b1 ], [ %12, %5 ]
//!   ; ... the test, %x, %y and the store to %b
//! ```
//! The first loop is a fill, which the pipeline makes a `memset`.
//!
//! From unit test `two_independent_stores_become_two_loops` in
//! `loopdistribute_tests.rs`.
//!
//! # Runs
//! Once per body, after `unswitch`, `split` and `fuse`, at -O3 and -Omax
//! (`-ftree-loop-distribution`). Not run where one pass is selected by name.
//!
//! # Needs
//! [`crate::candidate`], [`crate::loopclone`] (`copied`), [`crate::loopshape`],
//! [`crate::loopidioms`] (`takes`), [`crate::profit`],
//! [`llrm_analysis::access`], [`llrm_analysis::dependence`] (`meetings`),
//! `induction` (`counted`).
//!
//! # Keeps
//! Nothing when it splits: the function is replaced by the candidate.
//!
//! # See also
//! gcc `tree-loop-distribution.cc` (`loop_distribution::distribute_loop`,
//! on at -O3); LLVM `LoopDistribute.cpp` (`InstPartitionContainer`,
//! `LoopDistributeForLoop::processLoop`; off by default).
//! [`crate::loopfuse`] is the opposite and judges by the same price; it runs
//! first, so what it joins is not parted again.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::access::{self, Apart, Reference, Roots};
use llrm_analysis::cfg::{self, Canonical};
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::manager::Registers;
use llrm_analysis::{dependence, induction, memory};
use llrm_mir::context::Context;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};
use num_traits::ToPrimitive;

use crate::candidate::{self, Baseline, Gain, Pricing};
use crate::dead::Dead;
use crate::lcssa::operations;
use crate::loopidioms;
use crate::profit::{self, OperationCosts};
use crate::{loopclone, loopshape};

/// Operations of the loop that is copied.
const MAX_LOOP_OPS: usize = 128;
/// Loops tried per body, innermost first: each is a copy of the loop and the
/// passes that price it. A body of 7 such loops cost 5% more to compile with
/// four.
const MAX_TRIALS: usize = 1;
/// Accesses of a loop that are compared in pairs.
const MAX_ACCESSES: usize = 32;
/// What a split must save of the price, in percent: the price is a model,
/// and a split costs a loop's control and may keep a later pass from
/// seeing the loop whole. A fill of 200 words beside a loop was priced 0.2%
/// cheaper and measured 0.4% dearer.
const MIN_PERCENT: i64 = 5;
/// Loops one loop is split into.
const MAX_PARTITIONS: usize = 8;

pub struct Distribute {
    /// What a split candidate goes through before it is judged.
    pub passes: Vec<Box<dyn FunctionPass>>,
}

impl FunctionPass for Distribute {
    fn name(&self) -> &'static str {
        "distribute"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let costs = profit::costs(&outer);
        let passes = &mut self.passes;
        let module = std::rc::Rc::clone(&outer);
        let mut reoptimize = |candidate: &mut Unit| {
            for pass in passes.iter_mut() {
                pass.run(candidate, &mut Analyses::new(std::rc::Rc::clone(&module)));
            }
        };
        match optimized(unit, analyses, &outer, &costs, &mut reoptimize) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("distribute: {error}"),
        }
    }
}

/// Every loop this can split and the price likes, up to the trial budget;
/// whether any was.
pub fn optimized(
    unit: &mut Unit,
    analyses: &mut Analyses,
    outer: &Outer,
    costs: &OperationCosts,
    reoptimize: &mut dyn FnMut(&mut Unit),
) -> Result<bool, String> {
    let mut refused = BTreeSet::new();
    let mut changed = false;
    // What the body costs whole, until a loop of it is split.
    let mut whole: Option<(Option<BTreeMap<i64, i64>>, Option<i64>)> = None;
    for _ in 0..MAX_TRIALS {
        let Some(split) = found(unit, analyses, &refused) else { break };
        let header = split.shape.loop_.header;
        // The least the idioms can save, an upper bound, against what the
        // body costs: a loop that is little of it is not worth a copy.
        let (frequency, before) = whole.get_or_insert_with(|| {
            let frequency = candidate::frequencies(unit, outer, unit.function);
            let price = frequency
                .as_ref()
                .and_then(|at| candidate::priced_at(unit, outer, costs, unit.function, Pricing::Work, at));
            (frequency, price)
        });
        let (Some(frequency), Some(before)) = (frequency.clone(), *before) else { break };
        if saved(unit, outer, costs, &split, &frequency) * 100 < before * MIN_PERCENT {
            llrm_support::debug!("distribute", "b{header}: not split, what an idiom saves is too little of the body");
            refused.insert(header);
            continue;
        }
        let mut copy = unit.function.clone();
        if let Err(error) = distributed(&mut copy, &split) {
            llrm_support::debug!("distribute", "b{header}: not split, {error}");
            refused.insert(header);
            continue;
        }
        // Cheap first: the split with its idioms, priced beside the loop whole,
        // before the pipeline is run over it.
        let copy = with_idioms(unit, analyses, &split, copy);
        let after = candidate::priced(unit, outer, costs, &copy, Pricing::Work);
        if !matches!(
            after,
            Some(after) if after * 100 < before * (100 - MIN_PERCENT)
        ) {
            llrm_support::debug!("distribute", "b{header}: not split, no part of it is cheaper as an idiom");
            refused.insert(header);
            continue;
        }
        if candidate::adopted(
            unit,
            outer,
            costs,
            copy,
            reoptimize,
            Pricing::WithSpills,
            Gain::CheaperBy(MIN_PERCENT),
            Baseline::Kept,
        ) {
            llrm_support::debug!("distribute", "split the loop with header b{header} in {}", split.partitions.len());
            changed = true;
            whole = None;
            refused.clear();
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        } else {
            llrm_support::debug!("distribute", "b{header}: not split, the candidate is not cheaper");
            refused.insert(header);
        }
    }
    Ok(changed)
}

/// What the idiom parts of `split` take out of the loop, at most: the price of
/// everything their stores are made from, as often as it runs.
fn saved(
    unit: &Unit,
    outer: &Outer,
    costs: &OperationCosts,
    split: &Split,
    frequency: &BTreeMap<i64, i64>,
) -> i64 {
    let function: &Function = unit.function;
    split
        .partitions
        .iter()
        .zip(&split.idioms)
        .filter(|(_, idiom)| **idiom)
        .flat_map(|(stores, _)| stores)
        .flat_map(|&store| behind(function, &split.shape, store))
        .map(|inst| {
            let at = function.parent(inst).map_or(0, cfg::id);
            profit::operation(unit.context, unit.layout, function, outer.callees(), inst, costs).unwrap_or(0)
                * frequency.get(&at).copied().unwrap_or(0)
        })
        .sum()
}

/// `split`, the copy `distributed` made of `split.shape`, with what takes a
/// loop's part out of it done: `loopidioms` over the loops of the parts and
/// what that leaves unused removed, which is the work of the pipeline that
/// makes a split pay, and all of it that the pay needs. Splitting pays only so,
/// as in gcc's pattern distribution: a loop's control costs what a part of the
/// work saves unless the part is done in one instruction.
fn with_idioms(
    unit: &mut Unit,
    analyses: &Analyses,
    split: &Split,
    copy: Function,
) -> Function {
    let mut probe = copy;
    // The loops the copy added are those with a header that is not the
    // original's: the parts but the last.
    let before = cfg::Shape::of(unit.function).loops.iter().map(|loop_| loop_.header).collect::<BTreeSet<_>>();
    let mut unit_of = |function: &mut Function| {
        let outer = std::rc::Rc::clone(analyses.outer());
        let registers = analyses.fresh().get::<Registers>(unit.context, unit.layout, function);
        let (callees, declared) = (outer.callees(), &mut *unit.declared);
        crate::loopidioms::filled_with(
            unit.context,
            unit.layout,
            callees,
            function,
            &outer,
            declared,
            false,
            &mut llrm_analysis::memory::Standing::held(&registers),
            &|loop_| !before.contains(&loop_.header) || loop_.header == split.shape.loop_.header,
        )
    };
    unit_of(&mut probe);
    Dead.run(
        &mut Unit {
            context: &mut *unit.context,
            layout: unit.layout,
            function: &mut probe,
            id: None,
            metadata: unit.metadata,
            declared: &mut *unit.declared,
        },
        &mut analyses.fresh(),
    );
    probe
}

/// A loop and how it splits: the stores of each partition, in the order the
/// loops run.
struct Split {
    shape: Canonical,
    partitions: Vec<Vec<InstId>>,
    /// Which partitions `loopidioms` takes whole.
    idioms: Vec<bool>,
}

/// The first loop not in `refused` that splits, if any.
fn found(
    unit: &mut Unit,
    analyses: &mut Analyses,
    refused: &BTreeSet<i64>,
) -> Option<Split> {
    let function: &Function = unit.function;
    let shape = cfg::Shape::of(function);
    // The most nested first: the hottest.
    let depth = |loop_: &Loop| loop_.body.depth() as usize;
    let mut candidates = shape
        .loops
        .iter()
        .filter(|loop_| !refused.contains(&loop_.header))
        .filter_map(|loop_| Canonical::of(function, loop_))
        .filter(|one| stores(function, one) >= 2)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|one| std::cmp::Reverse(depth(&one.loop_)));
    if candidates.is_empty() {
        return None;
    }
    let facts = analyses.get::<Registers>(unit.context, unit.layout, function);
    let outer = std::rc::Rc::clone(analyses.outer());
    let within =
        memory::Unit::within(unit.context, unit.layout, function, &outer).with_registers(&facts).with_shape(&shape);
    let view = View { function, context: unit.context, outer: &outer, within: &within };
    candidates.into_iter().find_map(|one| view.partitioned(one))
}

/// The stores of the loop that are not volatile: a loop with fewer than two
/// has nothing to split.
fn stores(
    function: &Function,
    shape: &Canonical,
) -> usize {
    shape
        .loop_
        .body
        .iter()
        .flat_map(|&at| function.iter_block(cfg::block(at)))
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Store { volatile: false, .. }
        ))
        .count()
}

struct View<'a> {
    function: &'a Function,
    context: &'a Context,
    outer: &'a Outer,
    within: &'a memory::Unit<'a>,
}

impl View<'_> {
    /// `shape`'s partitions, where it is a loop that splits.
    fn partitioned(
        &self,
        shape: Canonical,
    ) -> Option<Split> {
        let function = self.function;
        let header = shape.loop_.header;
        let refused = |reason: &str| -> Option<Split> {
            llrm_support::debug!("distribute", "b{header}: not split, {reason}");
            None
        };
        let ops = shape.loop_.body.iter().map(|&at| operations(function, cfg::block(at)).len()).sum::<usize>();
        if ops > MAX_LOOP_OPS {
            return refused("the loop is too big");
        }
        let Some((trips, pretested)) = self.counted(&shape) else { return refused("the loop is not counted") };
        let Some(order) = self.path(&shape) else { return refused("the body is not one path of blocks") };
        if !loopshape::quiet(self.context, self.outer, function, &shape) {
            return refused("a call or work that may trap");
        }
        if pretested && !loopshape::header_quiet(self.context, self.outer, function, &shape) {
            return refused("a header that runs once more than the body touches memory");
        }
        let defined = shape.loop_.body.iter().flat_map(|&at| function.iter_block(cfg::block(at)));
        if defined.into_iter().any(|inst| loopshape::read_outside(function, &shape, inst)) {
            return refused("a value of the loop is read outside it");
        }
        let mut position = IdMap::new();
        for (at, inst) in order.iter().flat_map(|&block| function.iter_block(block)).enumerate() {
            position.insert(inst, at);
        }
        let mut roots = Roots::default();
        let Some(mut found) =
            access::references(self.within, &[&shape.loop_], &shape.loop_.body, &shape.loop_.body, &mut roots)
        else {
            return refused("an access is not affine in the counter");
        };
        if found.len() > MAX_ACCESSES {
            return refused("too many accesses");
        }
        found.sort_by_key(|one| position.get(&one.inst).copied().unwrap_or(usize::MAX));
        // Parts that are idioms stand alone; what lies between them is one
        // loop, as it was: parting it further costs a loop's control for
        // nothing.
        let mut partitions: Vec<(Vec<InstId>, bool)> = Vec::new();
        for part in self.grouped(&shape, &found, trips) {
            let idiom = loopidioms::takes(self.within, self.outer.callees(), &shape.loop_, &part);
            match partitions.last_mut() {
                Some((last, false)) if !idiom => last.extend(part),
                _ => partitions.push((part, idiom)),
            }
        }
        if !partitions.iter().any(|(_, idiom)| *idiom) {
            return refused("no part is an idiom");
        }
        let idioms = partitions.iter().map(|(_, idiom)| *idiom).collect::<Vec<_>>();
        let partitions = partitions.into_iter().map(|(part, _)| part).collect::<Vec<_>>();
        if partitions.len() < 2 || partitions.len() > MAX_PARTITIONS {
            return refused("one partition, or too many");
        }
        Some(Split { shape, partitions, idioms })
    }

    /// The loop's trips (None where only symbolic) and whether it is tested
    /// before each trip, when induction counts it from its one exit.
    fn counted(
        &self,
        shape: &Canonical,
    ) -> Option<(Option<i64>, bool)> {
        let proof = induction::counted(self.within, &shape.loop_, None, false)
            .into_iter()
            .find(|proof| loopshape::controls(self.function, proof, shape))?;
        Some((proof.count.as_ref().and_then(|count| count.to_i64()), !proof.posttested))
    }

    /// The blocks of the loop in the order one trip runs them, where each has
    /// one way on inside the loop.
    fn path(
        &self,
        shape: &Canonical,
    ) -> Option<Vec<BlockId>> {
        let function = self.function;
        let mut order = vec![shape.header];
        loop {
            let at = *order.last()?;
            let [next] = function.successors(at).into_iter().filter(|to| shape.holds(*to)).collect::<Vec<_>>()[..]
            else {
                return None;
            };
            if next == shape.header {
                break;
            }
            if order.len() > shape.loop_.body.len() {
                return None;
            }
            order.push(next);
        }
        (order.len() == shape.loop_.body.len()).then_some(order)
    }

    /// The stores of each partition, in the order the loops run.
    fn grouped(
        &self,
        shape: &Canonical,
        found: &[Reference],
        trips: Option<i64>,
    ) -> Vec<Vec<InstId>> {
        // A group for each access; a store and the loads its value comes from
        // are one.
        let mut group = (0..found.len()).collect::<Vec<_>>();
        let mut index = IdMap::new();
        for (at, one) in found.iter().enumerate() {
            index.insert(one.inst, at);
        }
        fn root(
            group: &mut [usize],
            at: usize,
        ) -> usize {
            let mut top = at;
            while group[top] != top {
                top = group[top];
            }
            group[at] = top;
            top
        }
        for (at, one) in found.iter().enumerate().filter(|(_, one)| one.access.write) {
            for load in behind(self.function, shape, one.inst)
                .into_iter()
                .filter(|&inst| matches!(self.function.instruction(inst).opcode, Opcode::Load { .. }))
            {
                if let Some(&other) = index.get(&load) {
                    let (a, b) = (root(&mut group, at), root(&mut group, other));
                    group[a] = b;
                }
            }
        }
        let top = (0..found.len()).map(|at| root(&mut group, at)).collect::<Vec<_>>();
        // Dependences between groups, each from the one that must come first.
        let apart = Apart::new(self.within);
        let mut before = BTreeSet::new();
        for (first, x) in found.iter().enumerate() {
            for (second, y) in found.iter().enumerate().skip(first + 1) {
                if !(x.access.write || y.access.write) || top[first] == top[second] {
                    continue;
                }
                let met = if apart.unknown(x, y) {
                    dependence::Meetings { forward: true, backward: true }
                } else {
                    dependence::meetings(&x.access, &y.access, trips)
                };
                if met.forward {
                    before.insert((top[first], top[second]));
                }
                if met.backward {
                    before.insert((top[second], top[first]));
                }
            }
        }
        // The groups with a store, in the order of their first, joined where
        // they depend on each other both ways (a strong component), run in an
        // order that keeps each dependence forward.
        let mut heads = Vec::new();
        for at in (0..found.len()).filter(|&at| found[at].access.write) {
            if !heads.contains(&top[at]) {
                heads.push(top[at]);
            }
        }
        let mut after = vec![Vec::new(); heads.len()];
        for &(first, second) in &before {
            if let (Some(from), Some(to)) =
                (heads.iter().position(|&one| one == first), heads.iter().position(|&one| one == second))
            {
                after[from].push(to);
            }
        }
        in_order(&after)
            .into_iter()
            .map(|component| {
                (0..found.len())
                    .filter(|&at| {
                        found[at].access.write
                            && heads
                                .iter()
                                .position(|&one| one == top[at])
                                .is_some_and(|head| component.contains(&head))
                    })
                    .map(|at| found[at].inst)
                    .collect()
            })
            .collect()
    }
}

/// The instructions of the loop that `store` stores a value or an address
/// from, itself included, through any work and phis between.
fn behind(
    function: &Function,
    shape: &Canonical,
    store: InstId,
) -> Vec<InstId> {
    let mut seen = IdSet::new();
    let mut pending = vec![store];
    let mut found = Vec::new();
    while let Some(inst) = pending.pop() {
        if !seen.insert(inst) {
            continue;
        }
        found.push(inst);
        for &operand in &function.instruction(inst).operands {
            let Operand::Value(value) = operand else { continue };
            if let ValueDef::Instruction(def) = function.value(value).def {
                if function.parent(def).is_some_and(|block| shape.holds(block)) {
                    pending.push(def);
                }
            }
        }
    }
    found
}

/// The strong components of the graph `after` (an edge from a group to each
/// that must come after it), in an order that keeps every edge forward, the
/// one holding the lowest group first where free: each a list of groups.
fn in_order(after: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let (component, _) = llrm_mir::callgraph::strong_components(after);
    let count = component.iter().max().map_or(0, |last| last + 1);
    let mut members = vec![Vec::new(); count];
    for (group, &one) in component.iter().enumerate() {
        members[one].push(group);
    }
    let mut waiting = vec![0usize; count];
    for (from, targets) in after.iter().enumerate() {
        for &to in targets.iter().filter(|&&to| component[to] != component[from]) {
            waiting[component[to]] += 1;
        }
    }
    let mut order = Vec::new();
    let mut left = (0..count).collect::<Vec<_>>();
    while !left.is_empty() {
        // A component nothing left comes before always exists: the components
        // of a graph have no cycle.
        let next = *left.iter().filter(|&&one| waiting[one] == 0).min_by_key(|&&one| members[one][0]).expect("a first");
        left.retain(|&one| one != next);
        for &group in &members[next] {
            for &to in after[group].iter().filter(|&&to| component[to] != next) {
                waiting[component[to]] -= 1;
            }
        }
        order.push(members[next].clone());
    }
    order
}

/// `function` with the loop of `split` copied once for each partition but the
/// last and each copy cut to its partition's stores.
fn distributed(
    function: &mut Function,
    split: &Split,
) -> Result<(), String> {
    let shape = &split.shape;
    let copies =
        loopclone::copied(function, &shape.loop_, split.partitions.len() - 1)?.ok_or("the loop is not copied")?;
    let all = split.partitions.iter().flatten().copied().collect::<Vec<_>>();
    for (k, partition) in split.partitions.iter().enumerate() {
        for &store in all.iter().filter(|store| !partition.contains(store)) {
            let inst = match copies.get(k) {
                Some(copy) => *copy.instructions.get(&store).ok_or("a store the copy lacks")?,
                None => store,
            };
            function.erase(inst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "loopdistribute_tests.rs"]
mod tests;
