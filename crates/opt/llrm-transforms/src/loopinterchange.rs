//! Loop interchange: the two loops of a nest swap places, so the inner loop
//! walks the memory the outer one did (gcc's `gimple-loop-interchange.cc`,
//! LLVM's `LoopInterchange`).
//!
//! # How it works
//! A nest (`loopnest`) whose loops each hold only a counter: a header of a phi,
//! the test of it against a bound, and the branch, and a step of the counter
//! by a constant. The counters are one type, and read nowhere outside their
//! loop; the bounds are defined outside the nest. What the outer loop does
//! before the inner is pure work that only the inner body reads.
//!
//! The loops swap places when
//! - the outer count is a constant in range (a bound on the dependence
//!   distances) and the accesses of the nest are affine over identified objects
//!   (`loopnest::accesses`);
//! - every access moves with both loops (one that does not is a repeat, for
//!   jamming or promotion);
//! - `dependence::interchangeable`: no pair with a write has a vector `(x, y)`
//!   of `x > 0`, `y < 0`;
//! - `dependence::walk_cost` of the outer loop run innermost is less than that
//!   of the inner, with the cache line the target states (`cache_line`; 64
//!   where it states none, as gcc's and LLVM's defaults): the bytes an access
//!   steps, at most a line, none where it does not move.
//!
//! The blocks stay where they are. The starts, the steps and the tests of the
//! counters change places, the body's uses of one counter become the other's,
//! and the outer loop's work before the inner moves into the inner body.
//!
//! ```text
//!   for i in 0..N                for j in 0..M            (the outer loop's
//!     for j in 0..M      ==>       for i in 0..N           header now counts j)
//!       a[j][i] = ..                a[j][i] = ..
//! ```
//!
//! `LLRM_DEBUG=interchange` says what each nest was refused for.
//!
//! # Example
//! `a[j][i] = i + j` over `int a[64][64]`, `i` outer: the inner loop steps 256
//! bytes. Interchanged, `i` is the inner counter and steps 4.
//!
//! From unit test `a_column_walk_is_swapped` in `loopinterchange_tests.rs`.
//!
//! # Runs
//! Once per body, before `unroll-jam`; a swapped nest goes through `sroa` and
//! the fixed point again. -O3 and -Omax: `-floop-interchange`.
//!
//! # Needs
//! `loopnest`, `dependence`, `induction` (the counts), the target's
//! `cache_line`.
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! gcc `tree_loop_interchange` (`gimple-loop-interchange.cc:1605`),
//! `should_interchange_loops` (:1467); LLVM
//! `LoopInterchangeLegality::canInterchangeLoops`
//! and `LoopInterchangeProfitability::isProfitable`.

use std::collections::BTreeSet;

use llrm_analysis::dependence::{self, Innermost};
use llrm_analysis::manager::Registers;
use llrm_analysis::occurrence::{arms, from_arms};
use llrm_analysis::{cfg, induction, memory};
use llrm_mir::context::Context;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use num_traits::ToPrimitive;

use crate::loopnest::{Nest, accesses, defined, inner_is_invariant, nest_of, work};

/// A cache line where the target states none: gcc's `l1-cache-line-size` and
/// LLVM's `cache-line-size` default.
const DEFAULT_LINE: i64 = 64;
/// Outer trips tried: the dependence test walks each distance up to them.
const MAX_OUTER_TRIPS: i64 = 1024;
/// Accesses tried: the test is over pairs.
const MAX_ACCESSES: usize = 16;
/// Fewest trips of the loop that becomes the inner one: fewer would make the
/// loop's own work more than its stride saves.
const MIN_INNER_TRIPS: i64 = 4;

pub struct Interchange;

impl FunctionPass for Interchange {
    fn name(&self) -> &'static str {
        "interchange"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if swapped(unit, analyses) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// One loop's control: the counter, how it is tested, how it steps.
struct Control {
    phi: InstId,
    /// The counter's value.
    counter: llrm_mir::module::ValueId,
    /// The start and the arm of the block it comes from.
    start: Operand,
    /// `counter PREDICATE bound`, and the cmp and the branch it feeds.
    predicate: IntPredicate,
    bound: Operand,
    test: InstId,
    branch: InstId,
    /// `counter + step`, its constant's operand index, and the constant.
    next: InstId,
    step_at: usize,
    step: Operand,
}

/// Every nest this can swap, once each; whether any was.
fn swapped(
    unit: &mut passes::Unit,
    analyses: &Analyses,
) -> bool {
    let mut changed = false;
    let mut refused = BTreeSet::new();
    while let Some(plan) = planned(unit, analyses, &mut refused) {
        apply(unit.function, &plan);
        llrm_support::debug!("interchange", "swapped the loops of a nest");
        changed = true;
    }
    changed
}

struct Plan {
    nest: Nest,
    outer: Control,
    inner: Control,
    /// The outer loop's work ahead of the inner, which moves into its body.
    moved: Vec<InstId>,
}

/// The first nest to swap, and what it is swapped by.
fn planned(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    refused: &mut BTreeSet<(i64, i64)>,
) -> Option<Plan> {
    let graph = cfg::graph(unit.function);
    let shape = cfg::Shape::of(unit.function);
    let mut facts = None;
    for inner in &shape.loops {
        if shape.loops.iter().any(|other| other.header != inner.header && inner.body.is_superset(&other.body)) {
            continue;
        }
        let Some(outer) = shape
            .loops
            .iter()
            .filter(|other| other.header != inner.header && other.body.is_superset(&inner.body))
            .min_by_key(|other| other.body.len())
        else {
            continue;
        };
        let key = (outer.header, inner.header);
        if refused.contains(&key) {
            continue;
        }
        let Some(nest) = nest_of(unit.function, &graph, outer, inner) else { continue };
        let found = (|| {
            if !inner_is_invariant(unit.function, &nest) {
                return Err("the inner loop counts differently each outer trip");
            }
            let (outer_control, inner_control, moved) = controls(unit.function, unit.context, &nest)?;
            let facts = facts
                .get_or_insert_with(|| analyses.fresh().get::<Registers>(unit.context, unit.layout, unit.function));
            let costs = analyses.outer().target().costs();
            let memory_unit = memory::Unit::within(unit.context, unit.layout, unit.function, analyses.outer())
                .with_registers(facts)
                .with_shape(&shape);
            let outer_trips = induction::trip_count(&memory_unit, outer, facts)
                .and_then(|count| count.to_i64())
                .filter(|trips| (MIN_INNER_TRIPS..=MAX_OUTER_TRIPS).contains(trips))
                .ok_or("the outer count is not a constant in range")?;
            let inner_trips = induction::trip_count(&memory_unit, inner, facts).and_then(|count| count.to_i64());
            let list = accesses(&memory_unit, &nest)
                .filter(|list| !list.is_empty() && list.len() <= MAX_ACCESSES)
                .ok_or("the accesses are not affine over objects")?;
            // What does not move with both loops is a repeat that unrolling and
            // jamming or promotion are for, which a swap would not keep.
            if list.iter().any(|one| one.subscripts.first().is_none_or(|sub| sub.outer == 0 || sub.inner == 0)) {
                return Err("an access does not move with both loops");
            }
            let line = if costs.cache_line > 0 { costs.cache_line } else { DEFAULT_LINE };
            let (now, swapped) = (
                dependence::walk_cost(&list, line, Innermost::Inner),
                dependence::walk_cost(&list, line, Innermost::Outer),
            );
            if swapped >= now {
                return Err("the inner loop does not walk further than the outer");
            }
            if !dependence::interchangeable(&list, outer_trips, inner_trips) {
                return Err("a dependence forbids it");
            }
            Ok(Plan { nest: nest.clone(), outer: outer_control, inner: inner_control, moved })
        })();
        match found {
            // A swapped nest is not swapped back.
            Ok(plan) => {
                refused.insert(key);
                return Some(plan);
            }
            Err(why) => {
                llrm_support::debug!("interchange", "b{}: not swapped, {why}", outer.header);
                refused.insert(key);
            }
        }
    }
    None
}

/// The counter, test and step of the loop with header `header`, whose latch is
/// `latch`, entered from `entry`; none where it is more than that.
fn control(
    function: &Function,
    header: llrm_mir::module::BlockId,
    latch: llrm_mir::module::BlockId,
    entry: &dyn Fn(llrm_mir::module::BlockId) -> bool,
) -> Result<Control, &'static str> {
    let [phi, test, branch] = function.iter_block(header).collect::<Vec<_>>()[..] else {
        return Err("a header holds more than a counter and its test");
    };
    if function.instruction(phi).opcode != Opcode::Phi || function.instruction(branch).opcode != Opcode::Br {
        return Err("a header holds more than a counter and its test");
    }
    let counter = function.instruction(phi).result.expect("a phi's value");
    let Opcode::ICmp(predicate) = function.instruction(test).opcode else {
        return Err("the test is no integer compare");
    };
    let compared = &function.instruction(test).operands;
    let (predicate, bound) = match (compared[0] == Operand::Value(counter), compared[1] == Operand::Value(counter)) {
        (true, false) => (predicate, compared[1]),
        (false, true) => (predicate.swapped(), compared[0]),
        _ => return Err("the test is not of the counter and a bound"),
    };
    let result = function.instruction(test).result.expect("a compare's value");
    if !function.users(result).has_one_use() || function.instruction(branch).operands[0] != Operand::Value(result) {
        return Err("the test is read by more than the branch");
    }
    let mut start = None;
    let mut next = None;
    for (value, from) in arms(function, phi) {
        if from == latch {
            next = Some(value);
        } else if entry(from) && start.is_none() {
            start = Some(value);
        } else {
            return Err("the counter has other arms than a start and a step");
        }
    }
    let (Some(start), Some(Operand::Value(next))) = (start, next) else {
        return Err("the counter has no start and step");
    };
    let step_inst = defined(function, next).ok_or("the step is no instruction")?;
    let op = function.instruction(step_inst);
    if op.opcode != Opcode::Binary(BinaryOp::Add) {
        return Err("the counter steps by something other than an add");
    }
    let step_at = match (op.operands[0] == Operand::Value(counter), op.operands[1] == Operand::Value(counter)) {
        (true, false) => 1,
        (false, true) => 0,
        _ => return Err("the step is not the counter and a constant"),
    };
    let step = op.operands[step_at];
    if !matches!(step, Operand::Constant(_)) {
        return Err("the step is not constant");
    }
    if !function.users(next).has_one_use() {
        return Err("the next counter is read by more than the counter");
    }
    Ok(Control { phi, counter, start, predicate, bound, test, branch, next: step_inst, step_at, step })
}

/// The loops' controls, and the outer loop's work to move; or why not.
fn controls(
    function: &Function,
    context: &Context,
    nest: &Nest,
) -> Result<(Control, Control, Vec<InstId>), &'static str> {
    let inside_outer = |at: llrm_mir::module::BlockId| nest.outer.body.contains(&cfg::id(at));
    let outer =
        control(function, nest.outer_header(), cfg::block(*nest.outer.latches.first().expect("one latch")), &|from| {
            !inside_outer(from)
        })?;
    let inner = control(function, nest.inner_header(), nest.inner_latch(), &|from| from == nest.entry())?;
    let stays = |header, body: &llrm_mir::forest::Body| {
        function.successors(header).first().is_some_and(|to| body.contains(&cfg::id(*to)))
    };
    if !stays(nest.outer_header(), &nest.outer.body) || !stays(nest.inner_header(), &nest.inner.body) {
        return Err("a loop is left by its test being true");
    }
    // One type of counter, step and bound, so each loop can take the other's.
    let ty = |operand: Operand| function.operand_type(context, operand);
    if ty(Operand::Value(outer.counter)) != ty(Operand::Value(inner.counter))
        || ty(outer.bound) != ty(inner.bound)
        || ty(outer.step) != ty(inner.step)
        || ty(outer.start) != ty(inner.start)
    {
        return Err("the counters are of different types");
    }
    // Bounds and starts come from outside the outer loop.
    let outside = |operand: Operand| match operand {
        Operand::Value(value) => defined(function, value)
            .and_then(|inst| function.parent(inst))
            .is_none_or(|block| !nest.outer.body.contains(&cfg::id(block))),
        _ => true,
    };
    if ![outer.bound, outer.start, inner.bound, inner.start].into_iter().all(outside) {
        return Err("a bound or a start is computed in the nest");
    }
    // The counters are read inside the nest only, and the steps by the
    // counters only: nothing else of the nest's loops is read outside it.
    let in_inner = |at: llrm_mir::module::BlockId| nest.inner.body.contains(&cfg::id(at));
    let in_pre = |at: llrm_mir::module::BlockId| nest.pre.contains(&at);
    for user in function.users(outer.counter) {
        let at = function.parent(user.user).expect("a placed user");
        if user.user != outer.test && user.user != outer.next && !in_inner(at) && !in_pre(at) {
            return Err("the outer counter is read outside the loops' bodies");
        }
    }
    for user in function.users(inner.counter) {
        let at = function.parent(user.user).expect("a placed user");
        if user.user != inner.test && user.user != inner.next && !in_inner(at) {
            return Err("the inner counter is read outside its body");
        }
    }
    // After the inner loop there is the outer step and nothing else.
    for &block in &nest.post {
        if work(function, block).iter().any(|&inst| inst != outer.next) {
            return Err("work follows the inner loop");
        }
    }
    // Before it, pure work that the inner body alone reads moves into it.
    let mut moved = Vec::new();
    for &block in &nest.pre {
        for inst in work(function, block) {
            let read_outside = function.instruction(inst).result.is_some_and(|result| {
                function
                    .users(result)
                    .iter()
                    .any(|user| function.parent(user.user).is_some_and(|at| !in_inner(at) && !nest.pre.contains(&at)))
            });
            if read_outside {
                return Err("work before the inner loop is read outside it");
            }
            moved.push(inst);
        }
    }
    Ok((outer, inner, moved))
}

/// `plan`'s nest with its loops swapped.
fn apply(
    function: &mut Function,
    plan: &Plan,
) {
    let (outer, inner, nest) = (&plan.outer, &plan.inner, &plan.nest);
    let in_inner = |function: &Function, inst: InstId| {
        function.parent(inst).is_some_and(|block| nest.inner.body.contains(&cfg::id(block)))
    };
    let moved: IdSet<InstId> = plan.moved.iter().copied().collect();
    // Each counter's uses in what stays inner and what moves into it.
    let reading = |function: &Function, control: &Control| {
        function
            .users(control.counter)
            .iter()
            .copied()
            .filter(|user| {
                user.user != control.test
                    && user.user != control.next
                    && (in_inner(function, user.user) || moved.contains(&user.user))
            })
            .collect::<Vec<_>>()
    };
    let (of_outer, of_inner) = (reading(function, outer), reading(function, inner));
    for user in of_outer {
        function.set_operand(user.user, user.index as usize, Operand::Value(inner.counter));
    }
    for user in of_inner {
        function.set_operand(user.user, user.index as usize, Operand::Value(outer.counter));
    }
    // The work that was before the inner loop is the first of its body.
    let first = function
        .successors(nest.inner_header())
        .into_iter()
        .find(|to| nest.inner.body.contains(&cfg::id(*to)))
        .expect("the inner body's first block");
    let at = function.iter_block(first).find(|&inst| function.instruction(inst).opcode != Opcode::Phi);
    for &inst in &plan.moved {
        match at {
            Some(at) => function.move_to(inst, Position::Before(at)).expect("a placed instruction"),
            None => function.move_to(inst, Position::End(first)).expect("a placed instruction"),
        }
    }
    // The starts change places, as do the steps.
    let restart = |function: &mut Function, control: &Control, start: Operand| {
        let arms_now = arms(function, control.phi);
        let changed: Vec<_> = arms_now
            .into_iter()
            .map(|(value, from)| (if value == control.start { start } else { value }, from))
            .collect();
        function.set_operands(control.phi, from_arms(&changed));
    };
    restart(function, outer, inner.start);
    restart(function, inner, outer.start);
    function.set_operand(outer.next, outer.step_at, inner.step);
    function.set_operand(inner.next, inner.step_at, outer.step);
    // Each header tests its counter as the other's did.
    for (control, other) in [(outer, inner), (inner, outer)] {
        let ty = function.instruction(control.test).ty;
        let test = function.create_instruction(
            Opcode::ICmp(other.predicate),
            ty,
            vec![Operand::Value(control.counter), other.bound],
            Flags::default(),
            None,
        );
        function.insert(test, Position::Before(control.branch)).expect("a placed branch");
        let result = Operand::Value(function.instruction(test).result.expect("a compare's value"));
        function.set_operand(control.branch, 0, result);
    }
    for control in [outer, inner] {
        function.erase(control.test).expect("a compare no one reads");
    }
}

#[cfg(test)]
#[path = "loopinterchange_tests.rs"]
mod tests;
