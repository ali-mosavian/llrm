//! Adapted from llrm-core's `optimize/inline.rs`, the port of
//! `qbopt/optimize/inline.py`: selective whole-module MIR inlining.
//!
//! Inlining is a CFG operation, not a call peephole: split the caller at the
//! call, clone the callee's blocks, bind formal parameters to the actual
//! operands, and join every return back to the continuation.  The ordinary
//! body pipeline then simplifies the result.
//!
//! A callee is a candidate when cloning it is safe; whether it is worth it is
//! cost: the body's priced work copied against the calls it removes, in the
//! target's clocks, or its code bytes at -Os, bounded by the `Threshold`
//! (nominal: a body's budget is at most 24 operations).  A
//! body that only reads or writes through its arguments or its own frame
//! inlines like any other: the clone keeps its memory operations and the
//! ordinary body pipeline turns the caller's argument cells into values.  A
//! public callee is inlined at its sites and stays defined; a private one
//! called nowhere else goes with its last site.  MIR chooses from semantic
//! costs and never sees opcodes or registers.  The call's price is profit's
//! `OperationCosts::call`.
//!
//! What changed with the IR: a call names its callee and carries its
//! actuals, a formal is a parameter value, and the call's one result is what
//! `ret` returns, so the old call-site and ARG tables, formal entry cells
//! (`_parameter`), materialized actual copies, unmodelled extra call results
//! and width checks have no counterpart; the function type decides whether a
//! call fits its callee.  A candidate is a snapshot of its callee, as the old
//! one held the body it was chosen from.  A clone keeps its original's
//! metadata, which replaces merging the pointer and range side tables.  The
//! old `sealed` flag is a definition; a clone keeps its memory operations,
//! which replaces admitting only formal loads.  The clone is llrm-mir's
//! `splice`.  SSA is checked by the verifier
//! after each pass, not here.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::facts::{Facts, Inlining};
use llrm_mir::memory::{Callees, callee};
use llrm_mir::module::{Function, GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::splice::{carries, splice};
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;

use llrm_analysis::consts;
use llrm_analysis::memory::Unit;

use llrm_analysis::cfg;

use crate::profit::{self, OperationCosts, operation};

/// How much inlining may copy: LLVM's inline threshold, 225 at -O2 and 0
/// for none. A callee's budget, in semantic operations, scales with it.
/// `hint` is the ratio a routine the language marks worth inlining may grow
/// by: LLVM's inline-hint threshold, 325 against 225; 1/1 where code size
/// outranks speed. `hot` is the same for a call in a loop: LLVM's
/// locally-hot call site threshold, 525 against 225.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Threshold {
    pub limit: i64,
    pub hint: (i64, i64),
    pub hot: (i64, i64),
}

impl Threshold {
    pub fn new(limit: i64) -> Self {
        Self { limit, hint: (325, 225), hot: (525, 225) }
    }

    /// The same where code size outranks speed: a hint or a loop buys nothing.
    pub fn for_size(self) -> Self {
        Self { hint: (1, 1), hot: (1, 1), ..self }
    }
}

impl Default for Threshold {
    fn default() -> Self {
        Self::new(225)
    }
}

impl Threshold {
    /// The budget for a call priced `call_cost`; None when nothing inlines.
    fn budget(self, call_cost: i64) -> Option<i64> {
        (self.limit > 0).then(|| 6.max(24.min(call_cost.div_euclid(2))) * self.limit / Self::default().limit)
    }
}

/// Direct call counts by callee.
pub type Counter = IndexMap<GlobalId, i64>;

/// What the language says of inlining `body`.
fn stated(body: &Function) -> Option<Inlining> {
    Facts::of(&body.attrs).inline()
}

/// The stack, in bytes, inlining may add to one function: a copy's frame is
/// a cell of its own wherever it sits, so copies add up, and in a recursive
/// function they add up per level.  LLVM bounds the same way.
const FRAME_LIMIT: u64 = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub body: Rc<Function>,
    /// Bytes of stack the body allocates.
    pub frame: u64,
}

/// What a caller says of itself that bounds what may be copied into it.
pub struct Caller<'a> {
    pub layout: &'a DataLayout,
    pub recursive: bool,
}

/// Whether `inst` does semantic work: not a phi, a jump or a return.
fn semantic(function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Phi | Opcode::Ret => false,
        Opcode::Br => instruction.operands.len() != 1,
        _ => true,
    }
}

fn semantic_count(body: &Function) -> i64 {
    body.walk().filter(|&(_, inst)| semantic(body, inst)).count() as i64
}

/// The defined function `id`.
fn body(module: &Module, id: GlobalId) -> Option<&Function> {
    module.global(id).function().filter(|function| !function.is_declaration())
}

/// Bytes of stack `function` allocates.
fn frame(context: &Context, layout: &DataLayout, function: &Function) -> u64 {
    function
        .walk()
        .filter_map(|(_, inst)| if let Opcode::Alloca { allocated, .. } = function.instruction(inst).opcode { Some(layout.alloc_size(&context.types, allocated)) } else { None })
        .sum()
}

/// The functions that call themselves, directly or not.
pub fn recursive(module: &Module) -> BTreeSet<GlobalId> {
    let graph = CallGraph::new(module);
    module.functions().map(|(id, _, _)| id).filter(|&id| graph.recursive(id)).collect()
}

/// Whether `body` may be cloned into another function: it returns, `splice`
/// carries it, and it is not recursive, never to be inlined or `setjmp`-like.
fn cloneable(module: &Module, recursive: &BTreeSet<GlobalId>, id: GlobalId, body: &Function) -> bool {
    !body.is_declaration()
        && carries(body)
        && stated(body) != Some(Inlining::Never)
        && !recursive.contains(&id)
        && body.walk().any(|(_, inst)| body.instruction(inst).opcode == Opcode::Ret)
        && !llrm_mir::memory::calls_returns_twice(module, body)
}

/// The priced work `body` does once, unless something in it is unpriced.
fn work(module: &Module, body: &Function, callees: &Callees, costs: &OperationCosts) -> Option<i64> {
    let layout = module.datalayout.as_deref().map_or_else(DataLayout::default, |text| DataLayout::parse(text).unwrap_or_default());
    body.walk().filter(|&(_, inst)| semantic(body, inst)).map(|(_, inst)| operation(&module.context, &layout, body, callees, inst, costs)).sum()
}

/// The priced work of `body` that its known actuals fold away: what a copy at
/// such a site no longer does. Instructions whose inputs are all known, and
/// branches they decide; a lower bound, as control flow past a decided branch
/// is not followed.
fn folded(module: &Module, layout: &DataLayout, body: &Function, known: &[Option<ConstantId>], callees: &Callees, costs: &OperationCosts) -> i64 {
    let unit = Unit::of(module, layout, body);
    let mut values = IndexMap::default();
    for (&parameter, constant) in body.parameters().iter().zip(known) {
        if let Some(number) = constant.and_then(|id| consts::_operand(&unit, Operand::Constant(id), &values, None)) {
            values.insert(parameter, number);
        }
    }
    let mut saved = 0;
    for (_, inst) in body.walk().filter(|&(_, inst)| semantic(body, inst)) {
        let instruction = body.instruction(inst);
        let decided = instruction.opcode == Opcode::Br && consts::_operand(&unit, instruction.operands[0], &values, None).is_some();
        let result = consts::_result(&unit, inst, &values, None);
        if !decided && result.is_none() {
            continue;
        }
        if let (Some(value), Some(number)) = (consts::_defined(&unit, inst), result) {
            values.insert(value, number);
        }
        saved += operation(&module.context, layout, body, callees, inst, costs).unwrap_or(0);
    }
    saved
}

/// What a call costs beyond its own instruction, which inlining saves: the
/// return, and each argument pushed by the caller and read back by the callee.
fn call_overhead(costs: &OperationCosts, arguments: usize) -> i64 {
    costs.call + costs.return_ + arguments as i64 * (costs.store + costs.load)
}

/// Functions worth moving into their direct callers.
///
/// A candidate's size is bounded by the `Threshold`; a routine the language
/// says is `Always` inlined is a candidate at any size, and `Never` is none.
/// Inlining it at all
/// `count` sites leaves the body behind when it is public or its address is
/// taken; a private one goes with the last site, so one site costs nothing.
/// Each copy beyond that duplicates the body's priced work, which has to
/// stay below the calls removed.
pub fn candidates(module: &Module, layout: &DataLayout, calls: &Counter, private: &BTreeSet<GlobalId>, costs: &OperationCosts, threshold: Threshold) -> IndexMap<GlobalId, Candidate> {
    let call_cost = costs.call;
    let budget = threshold.budget(call_cost);
    let (recursive, callees, addressed) = (recursive(module), llrm_mir::memory::callees(module), llrm_mir::callgraph::addressed(module));
    let mut out = IndexMap::default();
    for (&name, &count) in calls {
        let Some(body) = body(module, name) else { continue };
        if count == 0 || !cloneable(module, &recursive, name, body) {
            continue;
        }
        let always = stated(body) == Some(Inlining::Always);
        // A hint is worth a larger body, and a larger duplication, by LLVM's ratio.
        let scale = |n: i64| if stated(body) == Some(Inlining::Hint) { n * threshold.hint.0 / threshold.hint.1 } else { n };
        let copies = if private.contains(&name) && !addressed.contains(&name) { count - 1 } else { count };
        let admitted = || {
            budget.is_some_and(|budget| semantic_count(body) <= scale(budget))
                && (copies == 0 || work(module, body, &callees, costs).is_some_and(|work| work * copies < scale(count * call_cost)))
        };
        let verdict = always || admitted();
        llrm_support::debug!(
            "inline",
            "{} x{count} ({copies} copies): {} ops, budget {budget:?}, work {:?}, call {call_cost}: {}",
            module.global(name).name.as_deref().unwrap_or("?"),
            semantic_count(body),
            work(module, body, &callees, costs),
            if verdict { "candidate" } else { "refused" }
        );
        if verdict {
            out.insert(name, Candidate { body: Rc::new(body.clone()), frame: frame(&module.context, layout, body) });
        }
    }
    out
}

/// Functions worth cloning at one call site whose actual is a known
/// constant; `recursive` is `recursive(module)`.
///
/// Whole-body parameter specialization needs every caller to agree.  This
/// narrower policy instead admits a call whose known actual exposes local
/// SCCP after the normal MIR clone.  The original body remains for dynamic
/// callers, so no source-level calling convention or symbol changes.
/// The work a copy keeps, priced in the target's clocks after what the known
/// actuals fold, must stay below the call it replaces.
pub fn constant_sites(
    module: &Module,
    layout: &DataLayout,
    recursive: &BTreeSet<GlobalId>,
    caller: &Function,
    constants: &IndexMap<InstId, Vec<Option<ConstantId>>>,
    costs: &OperationCosts,
    threshold: Threshold,
) -> IndexMap<InstId, Candidate> {
    let call_cost = costs.call;
    let Some(budget) = threshold.budget(call_cost) else { return IndexMap::default() };
    let callees = llrm_mir::memory::callees(module);
    let frequency = profit::_frequencies(&module.context, &module.metadata, &module.globals, caller, None).unwrap_or_default();
    let mut out = IndexMap::default();
    for (block, at) in caller.walk() {
        let Some(name) = callee(&module.context, caller, at) else { continue };
        let known = constants.get(&at).map_or(&[][..], Vec::as_slice);
        if !known.iter().any(Option::is_some) {
            continue;
        }
        let Some(body) = body(module, name) else { continue };
        let semantic = semantic_count(body);
        // What stays of the copy, in clocks, against the call it replaces.
        let kept = work(module, body, &callees, costs).map(|all| all - folded(module, layout, body, known, &callees, costs));
        // A call in a loop saves its overhead on every trip, which LLVM's hot-site threshold weighs.
        let hot = frequency.get(&cfg::id(block)).is_some_and(|&one| one > profit::UNIT);
        let overhead = call_overhead(costs, known.len()) * if hot { threshold.hot.0 } else { 1 } / if hot { threshold.hot.1 } else { 1 };
        let verdict = kept.is_some_and(|kept| kept < overhead) && semantic <= budget && cloneable(module, recursive, name, body);
        llrm_support::debug!(
            "inline",
            "constant site of {}: {semantic} ops, {kept:?} clocks kept, budget {budget}, call {overhead}, {} of {} actuals known: {}",
            module.global(name).name.as_deref().unwrap_or("?"),
            known.iter().flatten().count(),
            known.len(),
            if verdict { "candidate" } else { "refused" }
        );
        if verdict {
            out.insert(at, Candidate { body: Rc::new(body.clone()), frame: frame(&module.context, layout, body) });
        }
    }
    out
}

/// Surviving direct call counts.
pub fn call_counts(module: &Module) -> Counter {
    let mut counts = Counter::default();
    for (_, _, function) in module.functions() {
        for (_, inst) in function.walk() {
            if matches!(function.instruction(inst).opcode, Opcode::Call(_))
                && let Some(name) = callee(&module.context, function, inst)
                && body(module, name).is_some()
            {
                *counts.entry(name).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Inline the first legal call site in `function`; whether one was.
pub fn expanded(
    context: &mut Context,
    function: &mut Function,
    caller: &Caller,
    available: &IndexMap<GlobalId, Candidate>,
    constant: Option<&IndexMap<InstId, Candidate>>,
) -> Result<bool, String> {
    let empty = IndexMap::default();
    let constant = constant.unwrap_or(&empty);
    let calls = function.walk().map(|(_, inst)| inst).collect::<Vec<_>>();
    for call in calls {
        if !matches!(function.instruction(call).opcode, Opcode::Call(_)) {
            continue;
        }
        let candidate = constant.get(&call).or_else(|| callee(context, function, call).and_then(|name| available.get(&name)));
        let Some(candidate) = candidate else {
            continue;
        };
        if fits(context, function, caller, call, candidate) {
            splice(context, function, call, &candidate.body);
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether the call fits its callee, which returns, and the stack the copy
/// adds stays within `FRAME_LIMIT`, and in a recursive function none.
fn fits(context: &Context, function: &Function, caller: &Caller, call: InstId, candidate: &Candidate) -> bool {
    let callee = &*candidate.body;
    let Opcode::Call(info) = &function.instruction(call).opcode else { return false };
    info.function_type == callee.ty
        && !matches!(context.types.get(callee.ty), Type::Function { variadic: true, .. })
        && (candidate.frame == 0 || (!caller.recursive && frame(context, caller.layout, function) + candidate.frame <= FRAME_LIMIT))
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
