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
//! 2. Loads (`pass::loads`: `processLoad`, `processNonLocalLoad`). Memory
//!    dependence (`llrm_analysis::memdep`) names what a load depends on in its
//!    block, or in every block that reaches it. A load a store or an earlier
//!    load of the same bytes serves becomes that value, cut or widened to its
//!    type (`vncoercion`); where several blocks serve it, a phi (`SsaUpdater`)
//!    merges them. A load with an unavailable block is left for step 4. Equal
//!    calls that touch no memory (`transform::equal_calls`) are one after it.
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
//! What serves a load is MemDep over `BasicAA`: the nearest store, load,
//! allocation or `memset` in its block that may touch its bytes, and a partial
//! overlap is a clobber the coercion may still cut a value from. The scan stops
//! at `BLOCK_SCAN_LIMIT` (100) instructions.
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
//! turned off together by `-fno-gcse`. `Gvn`'s PRE steps (`pre`)
//! are on from -O2, the -O1 level being FRE, and off with `-fno-tree-pre`. Each
//! runs once per body pass.
//!
//! # Needs
//! [`crate::transform`], [`crate::loadjoins`], `memoryssa::Accesses`,
//! `alias::PointValues`, `memdep`, `BasicAA`.
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
use llrm_analysis::manager::PointerValues;
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
use crate::{edges, loadjoins, transform};

/// The single value-reuse pass: scalar GVN, load elimination and PRE.
/// Without `pre` it only eliminates what is redundant on every path
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
        // the rest reads is read of the body it leaves. It numbers the loads
        // too (`processLoad`), so it removes memory accesses.
        let blocks = unit.function.layout().len();
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let tree = shape.dominance.tree(unit.function);
        let (scalar, cfg_changed) =
            pass::run_impl_cfg(unit.context, unit.layout, unit.function, analyses.outer(), Some(tree));
        drop(shape);
        // Equal calls that touch nothing are one, after the arithmetic that
        // makes their arguments equal.
        let called = match transform::equal_calls(unit.function, &silent(unit.context, analyses.outer())) {
            Ok(called) => called,
            Err(error) => panic!("gvn: {error}"),
        };
        // A changed CFG leaves none of what the manager holds; edited
        // instructions leave the shape.
        if cfg_changed {
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        } else if scalar || called {
            analyses.invalidate(
                unit.function,
                &PreservedAnalyses::none()
                    .preserve::<Dominators>()
                    .preserve::<Loops>()
                    .preserve::<llrm_analysis::cfg::Shape>(),
            );
        }
        let accesses = Accesses::managed(unit.context, unit.layout, unit.function, analyses);
        let pointers = analyses.get::<PointerValues>(unit.context, unit.layout, unit.function);
        let pointers = Result::as_ref(&*pointers).map_err(String::clone);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        match accesses.and_then(|accesses| {
            optimized(unit, analyses.outer(), &accesses, pointers?, &shape, self.pre, scalar || called)
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

/// Complete join PRE and load PRE after the numbering; whether anything
/// changed, and whether the CFG did. `outer` is what the analyses read of the
/// module and target, `accesses` what each instruction of `unit` touches,
/// `pointers` what each pointer points to. `numbered` says the numbering
/// changed something; `subexpressed` that a join's PRE should only complete.
///
/// The numbering removes loads and adds no memory access, so `accesses` stays
/// true of what is left.
pub fn optimized(
    unit: &mut Unit,
    outer: &Outer,
    accesses: &Accesses,
    pointers: &PointValues,
    shape: &cfg::Shape,
    pre: bool,
    numbered: bool,
) -> Result<(bool, bool), String> {
    // PRE may add work to a previously missing path.  Do that only after
    // local numbering has stabilized.
    let combined = joined(unit.function, pre && !numbered)?;
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
    Ok((numbered || combined || loaded, false))
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
    let cut_index = function.block(parent).instructions().iter().position(|&one| one == cut)? as i64;
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
        for (index, &inst) in function.block(block).instructions().iter().enumerate() {
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
        for inst in function.block(block).instructions().to_vec() {
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
