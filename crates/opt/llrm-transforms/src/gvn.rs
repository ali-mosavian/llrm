//! Reuses a value the function already has, so the same operation is not
//! computed twice and the same bytes are not read again: LLVM's GVN and gcc's
//! FRE/PRE. One pass owns reuse of a value, whether it came from arithmetic, a
//! load or a store. `Gvn` is the pass; `EqualCalls` is its call part alone.
//!
//! # How it works
//! `Gvn` runs these steps on a body, in order.
//!
//! 1. The scalar pass (`pass::run_impl`, `table`: LLVM's `GVNPass` and
//!    `ValueTable` without the load and PRE parts). Blocks with one way in are
//!    merged into it, then each block is met in reverse postorder, its
//!    instructions simplified or numbered by opcode, type and operand numbers
//!    (a commutative pair ordered, a compare's predicate swapped with its
//!    operands, a GEP by the bytes it adds, flags in no key): one that a
//!    dominating leader has is replaced, keeping the flags both have. In what
//!    an edge of a branch, a switch case or an assume dominates, the condition
//!    is that edge's constant (`propagateEquality`), and the arm of a branch on
//!    a constant is dead: its join's phis read poison from it. Two calls with
//!    the same callee and arguments that read and write no memory are one.
//! 2. Load numbering (`transform::forwarded`, `transform::subexpressions`,
//!    `transform::equal_calls`). A load a known value serves becomes that
//!    value: a dominating store of the same bytes and type, or an earlier load
//!    of them, with nothing between that may write them. The blocks are met
//!    dominators first, not in layout order, which a removed join leaves behind
//!    the blocks it came to dominate.
//! 3. Join PRE (`joined`). An expression every predecessor of a join already
//!    computes becomes a phi of those values. With `pre`, a predecessor that
//!    lacks it gets the computation inserted before its unconditional `br`, if
//!    no loop is entered or left by the edge and the operands are available
//!    there. Divisions (which may trap on a path that did not divide), loads
//!    and floating expressions are never inserted here.
//! 4. Load PRE (`loadjoins::reused`). A load at a join whose bytes every
//!    predecessor holds becomes a phi of those values; with `pre`, a
//!    predecessor holding none gets its own load on its edge where that
//!    speculates nothing.
//!
//! A volatile access is never numbered. Floats are numbered like integers; a
//! value serves a read only of its own type, so a float store reaches a float
//! load exactly and nothing rounds. Which floats are exact constants is
//! [`crate::floatfold`]'s question.
//!
//! What serves a load is a MemorySSA walk (`avail::forwardable_walk`).
//! `memoryssa::Accesses` says what each instruction reads and writes,
//! `alias::PointValues` what each pointer may point to, and a call's footprint
//! is its `CallEffects` from `Summaries`; without `Summaries` every callee is
//! unknown. A call that may write the bytes, or a store that may overlap, ends
//! the walk, and a walk past `MAX_NEAREST_WRITES` (100) writes gives up.
//!
//! A value held across a store saves a load but lengthens a live range. Where a
//! load was served across a store the function is numbered twice, once crossing
//! stores and once avoiding them, and the crossing one stands unless the
//! careful one prices lower (`profit::motion_price`: work weighted by block
//! frequency plus the spill forecast against the target's registers). Where
//! nothing spills the crossing one stands. Trip counts are the proven ones
//! (`profit::proven_trips`). Without `pre` there is no pricing and a load is
//! served wherever it is redundant on every path.
//!
//! `EqualCalls` runs the call numbering alone, before `tailrec`, so two equal
//! calls are one before one of them becomes the loop's next trip (EarlyCSE
//! precedes tail-recursion elimination in LLVM, FRE in gcc). It asks for no
//! analysis unless the body has two such calls.
//!
//! # Example
//! A store's value reaches the load after it, and the load below a join takes
//! the value each arm stored. Before:
//!
//! ```mir
//! @g = global i16 0
//!
//! define i16 @f(i16 %x, i16 %y, i1 %c) {
//! b0:
//!   store i16 %x, ptr @g
//!   %r = load i16, ptr @g
//!   ret i16 %r
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %x, i16 %y, i1 %c) {
//! b0:
//!   store i16 %x, ptr @g
//!   ret i16 %x
//! }
//! ```
//!
//! From unit test `test_gvn_forwards_a_stored_value_to_its_load` in
//! `gvn_tests.rs`. Before:
//!
//! ```mir
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   store i16 %x, ptr @g
//!   br label %b3
//!
//! b2:
//!   store i16 %y, ptr @g
//!   br label %b3
//!
//! b3:
//!   %r = load i16, ptr @g
//!   ret i16 %r
//! ```
//!
//! After:
//!
//! ```mir
//! b3:
//!   %r1 = phi i16 [ %x, %b1 ], [ %y, %b2 ]
//!   ret i16 %r1
//! ```
//!
//! `b0` to `b2` are unchanged. From unit test
//! `test_gvn_joins_the_values_each_arm_stored` in `gvn_tests.rs`.
//!
//! # Runs
//! `EqualCalls` runs early in the body pipeline, before `tailrec`; `Gvn` after
//! `dse`, before `promote`. Both are on at every optimizing level and are
//! turned off together by `-fno-gcse`. `Gvn`'s PRE steps and pricing (`pre`)
//! are on from -O2, the -O1 level being FRE, and off with `-fno-tree-pre`. Each
//! runs once per body pass.
//!
//! # Needs
//! [`crate::transform`], [`crate::loadjoins`], `memoryssa::Accesses`,
//! `alias::PointValues`, `Summaries`, registers and loop trips for pricing.
//!
//! # Keeps
//! Dominators and loops, unless a join split an edge.
//!
//! # See also
//! Tests: `gvn_tests.rs`, `loadjoins_tests.rs`. Levels:
//! `docs/reference/levels.md`. LLVM `GVN.cpp`; gcc `tree-ssa-pre.cc`.

use std::collections::BTreeMap;

use llrm_analysis::alias::PointValues;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::manager::{PointerValues, Registers};
use llrm_analysis::memory;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::{cfg, ssa};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_support::hash::IndexMap;

pub mod pass;
pub mod table;

use crate::lcssa::{arms, from_arms, place_phi};
use crate::profit::{self, OperationCosts};
use crate::{edges, loadjoins, transform};

/// The single value-reuse pass: scalar GVN and memory-aware PRE. The
/// target's registers price it (`profit::registers`); none leaves pricing
/// out. Without `pre` it only eliminates what is redundant on every path
/// (gcc's `-ftree-fre`, -O1): nothing is inserted to make a partial
/// redundancy full.
pub struct Gvn {
    pub pre: bool,
}

impl Default for Gvn {
    fn default() -> Self {
        Self { pre: true }
    }
}

impl FunctionPass for Gvn {
    fn name(&self) -> &'static str {
        "gvn"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        // The scalar pass first: it may merge blocks and split edges, and what
        // the rest reads is read of the body it leaves.
        let blocks = unit.function.layout().len();
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let tree = shape.dominance.tree(unit.function);
        let (scalar, cfg_changed) =
            pass::run_impl_cfg(unit.context, unit.layout, unit.function, analyses.outer(), Some(tree));
        drop(shape);
        // Edits to operands leave what the manager holds of the body whole but
        // for its registers; a changed CFG leaves none of it.
        if cfg_changed {
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        } else if scalar {
            analyses.invalidate(unit.function, &operands_edited());
        }
        let accesses = Accesses::managed(unit.context, unit.layout, unit.function, analyses);
        let pointers = analyses.get::<PointerValues>(unit.context, unit.layout, unit.function);
        // Only the scalar pass and `loadjoins` change the CFG: by merging
        // blocks, and by splitting edges.
        let pointers = Result::as_ref(&*pointers).map_err(String::clone);
        // Numbering leaves the CFG alone: every candidate has these trips.
        let registers = analyses.get::<Registers>(unit.context, unit.layout, unit.function);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let trips = if !self.pre || !profit::registers(analyses.outer()).priced() {
            IndexMap::default()
        } else {
            let counted = analyses.get::<llrm_analysis::manager::Counted>(unit.context, unit.layout, unit.function);
            profit::proven_trips(
                &memory::Unit::within(unit.context, unit.layout, unit.function, analyses.outer())
                    .with_registers(&registers)
                    .with_shape(&shape)
                    .with_counted(&counted),
            )
        };
        match accesses.and_then(|accesses| {
            optimized(unit, analyses.outer(), &accesses, pointers?, &trips, &registers, &shape, self.pre, scalar)
        }) {
            Ok((true, numbering_changed_cfg))
                if cfg_changed || numbering_changed_cfg || unit.function.layout().len() != blocks =>
            {
                PreservedAnalyses::none()
            }
            Ok((true, _)) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok((false, _)) => PreservedAnalyses::all(),
            Err(error) => panic!("gvn: {error}"),
        }
    }
}

/// Whether the call `inst` of `function` reads and writes nothing.
fn silent(
    context: &llrm_mir::context::Context,
    outer: &Outer,
) -> impl Fn(&Function, InstId) -> bool {
    move |function, inst| {
        let effects = llrm_mir::memory::of(context, outer.callees(), function, inst);
        !effects.reads && !effects.writes
    }
}

/// What EarlyCSE does to calls: two equal calls that touch no memory are one,
/// before `tailrec` makes the second a loop's trip and leaves the first alone.
/// Arithmetic and loads stay for `Gvn`, which prices what a longer-lived value
/// costs in spills; a call is always worth merging. No analysis is asked for
/// unless the body has two such calls.
#[derive(Default)]
pub struct EqualCalls;

impl FunctionPass for EqualCalls {
    fn name(&self) -> &'static str {
        "equalcalls"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        match transform::equal_calls(unit.function, &silent(unit.context, &outer)) {
            Ok(true) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("equalcalls: {error}"),
        }
    }
}

/// Number values, reuse dominating providers, and complete join PRE;
/// whether anything changed. `outer` is what the analyses read of the
/// module and target, `accesses` what each instruction of `unit` touches,
/// `pointers` what each pointer points to, `trips` each loop's proven
/// trips by latch.
///
/// Every edit replaces a value with an equal one and adds no memory
/// access before `loadjoins`, so `accesses` stays true throughout.
pub fn optimized(
    unit: &mut Unit,
    outer: &Outer,
    accesses: &Accesses,
    pointers: &PointValues,
    trips: &IndexMap<i64, i64>,
    registers: &IndexMap<ValueId, llrm_analysis::consts::Known>,
    shape: &cfg::Shape,
    pre: bool,
    equal: bool,
) -> Result<(bool, bool), String> {
    let (numbered, subexpressed, cfg_numbered) =
        // Without `pre` this is -O1's FRE: gcc's has no profit pricing, so a
        // load is served wherever it is redundant and nothing is numbered
        // twice.
        _numbered(unit, outer, accesses, &profit::costs(outer), if pre { profit::registers(outer) } else { crate::spill::Room::default() }, trips, registers, shape)?;
    // Numbering merged a block or split an edge: the shape and the accesses
    // are of the body before, and PRE is the next round's, over fresh ones.
    if cfg_numbered {
        return Ok((true, true));
    }
    // PRE may add work to a previously missing path.  Do that only after
    // local numbering has stabilized.
    let combined = joined(unit.function, pre && !subexpressed)?;
    // Joining may split an edge, which the manager's shape has not seen.
    let joined_shape;
    let shape = if combined {
        joined_shape = cfg::Shape::of(unit.function);
        &joined_shape
    } else {
        shape
    };
    let loaded = loadjoins::reused(
        unit.context,
        unit.layout,
        unit.function,
        outer,
        outer.callees(),
        accesses,
        pointers,
        pre && !combined,
        shape,
    )?;
    Ok((equal || numbered || combined || loaded, false))
}

thread_local! {
    static NUMBERINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has numbered a function (once, or twice where a
/// load could be served across a store), for a test that a function with no
/// such load is numbered once.
pub fn numberings() -> usize {
    NUMBERINGS.with(std::cell::Cell::get)
}

/// Local numbering, crossing stores only where the whole function prices
/// lower for it: a provider held across a store saves loads but may spill.
/// Whether it changed anything, and whether `subexpressions` did.
fn _numbered(
    unit: &mut Unit,
    outer: &Outer,
    accesses: &Accesses,
    costs: &OperationCosts,
    room: crate::spill::Room,
    trips: &IndexMap<i64, i64>,
    registers: &IndexMap<ValueId, llrm_analysis::consts::Known>,
    shape: &cfg::Shape,
) -> Result<(bool, bool, bool), String> {
    // The walk over the function as it comes in: the same for both runs below,
    // each of which changes a copy.
    let forwarding = transform::Forwarding::default();
    // Whether some load was served across a store: the one thing the second
    // numbering does differently from the first.
    let crossed = std::cell::Cell::new(false);
    let numbered = |context: &mut llrm_mir::context::Context,
                    function: &Function,
                    avoid_store_crossing: bool|
     -> Result<(Function, (bool, bool, bool)), String> {
        NUMBERINGS.with(|runs| runs.set(runs.get() + 1));
        let mut function = function.fork();
        let forwarded = transform::forwarded(
            context,
            unit.layout,
            &mut function,
            outer,
            accesses,
            registers,
            shape,
            avoid_store_crossing,
            &forwarding,
            &crossed,
        )?;
        let mut subexpressed =
            transform::subexpressions(&mut function, accesses, avoid_store_crossing, Some(outer.program()), &crossed)?;
        // The scalar numbering, over the loads merged above, and the loads
        // again over the addresses it merged: `iterateOnFunction`'s
        // repeat until nothing changes (a few rounds here; a load's
        // address is one round deep).
        let (mut scalar, mut cfg) = (false, false);
        for _ in 0..4 {
            if !(forwarded || subexpressed) {
                break;
            }
            let tree = shape.dominance.tree(&function);
            let (changed, split) = pass::run_impl_cfg(context, unit.layout, &mut function, outer, Some(tree));
            scalar |= changed;
            cfg |= split;
            if !changed || split {
                break;
            }
            let again = transform::subexpressions(
                &mut function,
                accesses,
                avoid_store_crossing,
                Some(outer.program()),
                &crossed,
            )?;
            subexpressed |= again;
            if !again {
                break;
            }
        }
        // Equal calls that touch nothing are one, after the arithmetic that
        // makes their arguments equal.
        let called = transform::equal_calls(&mut function, &silent(context, outer))?;
        Ok((function, (forwarded || subexpressed || called || scalar, subexpressed || called || scalar, cfg)))
    };
    let crossing = numbered(unit.context, unit.function, false)?;
    // What `one` costs: its work, and the spills it forecasts. The work alone
    // is the lower bound of the price, and is the cheap half.
    let price = |context: &llrm_mir::context::Context, one: &Function, under: i64| {
        let frequency = profit::_frequencies(context, unit.metadata, &outer.globals, one, Some(trips))?;
        let work = profit::weighted(context, unit.layout, one, outer.callees(), costs, &frequency)?;
        if work >= under {
            return Some(work);
        }
        profit::motion_price(context, unit.layout, outer, one, costs, room, &frequency)
    };
    // Where no load crossed a store, the second numbering is the first: it is
    // neither made nor priced.
    let chosen = if !room.priced() || !crossed.get() {
        crossing
    } else {
        match crossing_price(unit, outer, costs, room, trips, &crossing.0) {
            // The careful numbering forwards no load crossing one forwards, so
            // where crossing spills nothing it can only be dearer in work: it
            // is not made.
            Some((_, true)) | None => crossing,
            Some((crossed, false)) => {
                let careful = numbered(unit.context, unit.function, true)?;
                if price(unit.context, &careful.0, crossed).is_some_and(|kept| kept < crossed) {
                    careful
                } else {
                    crossing
                }
            }
        }
    };
    unit.function.adopt(chosen.0);
    Ok(chosen.1)
}

/// What `function` costs, as `profit::motion_price` has it, and whether it fits
/// its registers everywhere: the work is not weighed where it does.
fn crossing_price(
    unit: &Unit,
    outer: &Outer,
    costs: &OperationCosts,
    room: crate::spill::Room,
    trips: &IndexMap<i64, i64>,
    function: &Function,
) -> Option<(i64, bool)> {
    let frequency = profit::_frequencies(unit.context, unit.metadata, &outer.globals, function, Some(trips))?;
    let forecast = profit::spill_forecast(
        unit.context,
        unit.layout,
        function,
        costs,
        room,
        &|inst| crate::spill::kept_across(outer, unit.context, function, inst),
        &frequency,
    )?;
    if forecast.peak <= 0 {
        return Some((0, true));
    }
    let work = profit::weighted(unit.context, unit.layout, function, outer.callees(), costs, &frequency)?;
    Some((work + forecast.cost, false))
}

/// Translate simultaneously: an incoming phi value belongs to the prior edge.
fn _on_edge(
    function: &Function,
    op: &Instruction,
    phis: &[InstId],
    predecessor: BlockId,
) -> Option<Instruction> {
    let incoming = phis
        .iter()
        .map(|&phi| {
            let value = arms(function, phi).into_iter().find(|&(_, at)| at == predecessor).map(|(value, _)| value);
            (function.instruction(phi).result.expect("a phi's value"), value)
        })
        .collect::<IndexMap<_, _>>();
    let mut operands = Vec::new();
    for &operand in &op.operands {
        match operand {
            Operand::Value(value) if incoming.contains_key(&value) => operands.push(incoming[&value]?),
            _ => operands.push(operand),
        }
    }
    Some(Instruction { operands, ..op.clone() })
}

/// Insert only on an unconditional edge, with available scalar inputs:
/// before which instruction.
fn _insertion(
    function: &Function,
    op: &Instruction,
    parent: BlockId,
    join: BlockId,
    definitions: &IndexMap<ValueId, (i64, i64)>,
    dominance: &cfg::Dominance,
    natural_loops: &[Loop],
) -> Option<InstId> {
    let (parent_at, join_at) = (cfg::id(parent), cfg::id(join));
    if function.successors(parent) != [join]
        || dominance.dominates(function, join_at, parent_at)
        || natural_loops.iter().any(|loop_| loop_.body.contains(&parent_at) != loop_.body.contains(&join_at))
        || op.operands.iter().any(|operand| !matches!(operand, Operand::Value(_) | Operand::Constant(_)))
    {
        return None;
    }
    let cut = function.terminator(parent)?;
    if function.instruction(cut).opcode != Opcode::Br || function.instruction(cut).operands.len() != 1 {
        return None;
    }
    let cut_index = function.block(parent).iter().position(|one| one == cut)? as i64;
    for operand in &op.operands {
        let Operand::Value(value) = operand else {
            continue;
        };
        let (at, index) = *definitions.get(value)?;
        if !dominance.dominates(function, at, parent_at) || (at == parent_at && index >= cut_index) {
            return None;
        }
    }
    Some(cut)
}

/// Eliminate scalar redundancy without adding execution to any path;
/// whether anything changed.
///
/// A phi combines independently dominating providers. Missing providers may
/// be inserted on unconditional incoming edges, but only when another edge
/// already supplies the result. Memory and floating expressions stay out.
pub fn joined(
    function: &mut Function,
    insert: bool,
) -> Result<bool, String> {
    let graph = cfg::graph(function);
    let predecessors = loops::predecessors(&graph);
    if !predecessors.values().any(|parents| parents.len() > 1) {
        return Ok(false);
    }
    let Some(entry) = function.entry() else {
        return Ok(false);
    };
    let shape = cfg::Shape::of(function);
    let dominance = &shape.dominance;
    let depths = dominance.depths(function);
    let natural_loops = shape.loops;

    let key = |op: &Instruction| {
        op.result?;
        if matches!(
            op.opcode,
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) | Opcode::Load { .. }
        ) || transform::_floating(&op.opcode)
        {
            return None;
        }
        transform::_computation(op, &BTreeMap::new())
    };

    let mut expressions = IndexMap::<_, Vec<(i64, usize, ValueId)>>::default();
    let mut definitions = IndexMap::<ValueId, (i64, i64)>::default();
    definitions.extend(function.parameters().iter().map(|&value| (value, (cfg::id(entry), -1))));
    for &block in function.layout() {
        for (index, inst) in function.block(block).iter().enumerate() {
            let op = function.instruction(inst);
            let Some(result) = op.result else {
                continue;
            };
            let index = if op.opcode == Opcode::Phi { -1 } else { index as i64 };
            definitions.insert(result, (cfg::id(block), index));
            if let Some(expression) = key(op) {
                expressions.entry(expression).or_default().push((cfg::id(block), index as usize, result));
            }
        }
    }

    let mut changed = false;
    let mut replacements = BTreeMap::<ValueId, Operand>::new();
    let mut erased = Vec::new();
    for block in function.layout().to_vec() {
        let at = cfg::id(block);
        let parents = &predecessors[&at];
        if block == entry || parents.len() < 2 {
            continue;
        }
        for inst in function.block(block).iter().collect::<Vec<_>>() {
            let op = function.instruction(inst).clone();
            if op.opcode == Opcode::Phi {
                continue;
            }
            let expression = key(&op);
            let mut incoming = Vec::<(Operand, BlockId)>::new();
            let mut missing = Vec::<(BlockId, InstId, Instruction)>::new();
            if expression.is_some() {
                for &parent in parents {
                    let parent = cfg::block(parent);
                    let substituted = Instruction {
                        operands: ssa::substituted(&op, &replacements).map_err(|error| error.to_string())?,
                        ..op.clone()
                    };
                    let translated = _on_edge(function, &substituted, &edges::phis(function, block), parent);
                    let edge_expression = translated.as_ref().and_then(key);
                    let candidates = edge_expression
                        .as_ref()
                        .and_then(|edge| expressions.get(edge))
                        .into_iter()
                        .flatten()
                        .filter(|(provider, _, _)| {
                            *provider != at
                                && dominance.dominates(function, *provider, cfg::id(parent))
                                && !dominance.dominates(function, at, *provider)
                                && natural_loops
                                    .iter()
                                    .all(|loop_| !loop_.body.contains(provider) || loop_.body.contains(&at))
                        })
                        .collect::<Vec<_>>();
                    if candidates.is_empty() {
                        if !insert {
                            break;
                        }
                        let cut = match (&edge_expression, &translated) {
                            (Some(_), Some(translated)) => {
                                _insertion(function, translated, parent, block, &definitions, dominance, &natural_loops)
                            }
                            _ => None,
                        };
                        let Some(cut) = cut else {
                            break;
                        };
                        missing.push((parent, cut, translated.expect("an edge expression has a translation")));
                        continue;
                    }
                    // Python's `max` keeps the first of equal keys.
                    let mut best = candidates[0];
                    for item in &candidates[1..] {
                        if (depths[&item.0], item.1) > (depths[&best.0], best.1) {
                            best = item;
                        }
                    }
                    incoming.push((Operand::Value(best.2), parent));
                }
            }
            if incoming.is_empty() || incoming.len() + missing.len() != parents.len() {
                continue;
            }
            let result = op.result.expect("key needs a result");
            let name = function.value(result).name.clone();
            for (parent, cut, translated) in missing {
                let made = function.create_instruction(
                    translated.opcode,
                    translated.ty,
                    translated.operands,
                    translated.flags,
                    name.as_ref().map(|name| format!("{name}.pre")).as_deref(),
                );
                function.insert(made, Position::Before(cut))?;
                incoming
                    .push((Operand::Value(function.instruction(made).result.expect("an expression's value")), parent));
            }
            let phi = function.create_instruction(
                Opcode::Phi,
                op.ty,
                from_arms(&incoming),
                Default::default(),
                name.as_ref().map(|name| format!("{name}.pre-phi")).as_deref(),
            );
            place_phi(function, block, phi)?;
            replacements.insert(result, Operand::Value(function.instruction(phi).result.expect("a phi's value")));
            erased.push(inst);
            changed = true;
        }
    }
    if !changed {
        return Ok(false);
    }
    // A provider replaced in turn reaches each phi naming it through this.
    for (&value, &with) in &replacements {
        function.replace_value(value, with);
    }
    for inst in erased {
        function.erase(inst)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "gvn_tests.rs"]
mod tests;

/// What the scalar pass leaves true of a body whose operands it edited and
/// whose blocks it kept: everything the manager holds but the facts about
/// values (`Registers`) and the counted-loop proofs built on them (`Counted`).
fn operands_edited() -> PreservedAnalyses {
    use llrm_analysis::manager::{
        Annotated, Bounded, CallEffects, DemandedBits, DominatedEdges, ExposedFrames, FloatFacts, Followers,
        PlainAccesses, Pointers, TableLoads, ThroughMemory, Writes,
    };
    PreservedAnalyses::none()
        .preserve::<Dominators>()
        .preserve::<Loops>()
        .preserve::<llrm_analysis::cfg::Shape>()
        .preserve::<Accesses>()
        .preserve::<ExposedFrames>()
        .preserve::<Pointers>()
        .preserve::<Annotated>()
        .preserve::<CallEffects>()
        .preserve::<TableLoads>()
        .preserve::<DemandedBits>()
        .preserve::<Followers>()
        .preserve::<Writes>()
        .preserve::<PlainAccesses>()
        .preserve::<FloatFacts>()
        .preserve::<ThroughMemory>()
        .preserve::<DominatedEdges>()
        .preserve::<Bounded>()
}
