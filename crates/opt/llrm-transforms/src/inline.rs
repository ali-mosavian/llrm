//! Adapted from llrm-core's `optimize/inline.rs`, the port of
//! `qbopt/optimize/inline.py`: selective whole-module MIR inlining.
//!
//! Inlining is a CFG operation, not a call peephole: split the caller at the
//! call, clone the callee's blocks, bind formal parameters to the actual
//! operands, and join every return back to the continuation.  The ordinary
//! body pipeline then simplifies the result.
//!
//! A callee is a candidate when its memory effects are stated (`memory::known`)
//! and cloning it is safe; whether it is worth it is cost: the body's priced
//! work copied against the calls it removes, bounded by the `Threshold`.  A
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
//! old `sealed` flag is a definition; stated effects replace admitting only
//! formal loads.  SSA is checked by the verifier
//! after each pass, not here.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::memory::{Callees, callee, has, known};
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Module, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;

use crate::profit::{OperationCosts, operation};

/// How much inlining may copy: LLVM's inline threshold, 225 at -O2 and 0
/// for none. A callee's budget, in semantic operations, scales with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Threshold(pub i64);

impl Default for Threshold {
    fn default() -> Self {
        Self(225)
    }
}

impl Threshold {
    /// The budget for a call priced `call_cost`; None when nothing inlines.
    fn budget(self, call_cost: i64) -> Option<i64> {
        (self.0 > 0).then(|| 6.max(24.min(call_cost.div_euclid(2))) * self.0 / Self::default().0)
    }
}

/// Direct call counts by callee.
pub type Counter = IndexMap<GlobalId, i64>;

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub body: Rc<Function>,
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

/// Functions whose address is taken: named anywhere but as a callee.
fn addressed(module: &Module) -> BTreeSet<GlobalId> {
    let context = &module.context;
    let mut out = BTreeSet::new();
    let mut work = Vec::new();
    for (_, _, function) in module.functions() {
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let skip = usize::from(callee(context, function, inst).is_some());
            let kept = instruction.operands.len() - skip;
            work.extend(instruction.operands[..kept].iter().filter_map(|&operand| if let Operand::Constant(id) = operand { Some(id) } else { None }));
        }
    }
    work.extend(module.globals.iter().filter_map(|global| if let GlobalKind::Variable(variable) = &global.kind { variable.initializer } else { None }));
    while let Some(id) = work.pop() {
        match &context.get(id).kind {
            ConstantKind::Global(global) => {
                out.insert(*global);
            }
            ConstantKind::Aggregate(members) => work.extend(members),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => work.extend(operands),
            ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => work.push(*value),
            _ => {}
        }
    }
    out
}

/// Intrinsics that name the running frame: a clone would name the caller's.
const FRAME_INTRINSICS: [&str; 4] = ["llvm.stacksave", "llvm.stackrestore", "llvm.frameaddress", "llvm.returnaddress"];

/// Whether `body` may be cloned into another function: it returns, and it
/// has no handler, recursion, frame intrinsic or `setjmp`-like call whose
/// meaning a clone would change.  Its effects on memory must be stated.
fn cloneable(module: &Module, graph: &CallGraph, id: GlobalId, body: &Function) -> bool {
    let context = &module.context;
    if body.is_declaration() || !known(&body.attrs) || has(&body.attrs, "noinline") || graph.reaches(id, id) {
        return false;
    }
    let mut returns = false;
    for (_, inst) in body.walk() {
        let instruction = body.instruction(inst);
        returns |= instruction.opcode == Opcode::Ret;
        match instruction.opcode {
            Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume => return false,
            Opcode::Call(_) => {
                let Some(called) = callee(context, body, inst).map(|one| module.global(one)) else { continue };
                let name = called.name.as_deref().unwrap_or_default();
                if FRAME_INTRINSICS.contains(&name) || called.function().is_some_and(|one| has(&one.attrs, "returns_twice")) {
                    return false;
                }
            }
            _ => {}
        }
    }
    returns
}

/// The priced work `body` does once, unless something in it is unpriced.
fn work(module: &Module, body: &Function, callees: &Callees, costs: &OperationCosts) -> Option<i64> {
    body.walk().filter(|&(_, inst)| semantic(body, inst)).map(|(_, inst)| operation(&module.context, body, callees, inst, costs)).sum()
}

/// Functions worth moving into their direct callers.
///
/// A candidate's size is bounded by the `Threshold`.  Inlining it at all
/// `count` sites leaves the body behind when it is public or its address is
/// taken; a private one goes with the last site, so one site costs nothing.
/// Each copy beyond that duplicates the body's priced work, which has to
/// stay below the calls removed.
pub fn candidates(module: &Module, calls: &Counter, private: &BTreeSet<GlobalId>, costs: &OperationCosts, threshold: Threshold) -> IndexMap<GlobalId, Candidate> {
    let call_cost = costs.call;
    let Some(budget) = threshold.budget(call_cost) else { return IndexMap::default() };
    let (graph, callees, addressed) = (CallGraph::new(module), llrm_mir::memory::callees(module), addressed(module));
    let mut out = IndexMap::default();
    for (&name, &count) in calls {
        let Some(body) = body(module, name) else { continue };
        if count == 0 || semantic_count(body) > budget || !cloneable(module, &graph, name, body) {
            continue;
        }
        let copies = if private.contains(&name) && !addressed.contains(&name) { count - 1 } else { count };
        if copies == 0 || work(module, body, &callees, costs).is_some_and(|work| work * copies < count * call_cost) {
            out.insert(name, Candidate { body: Rc::new(body.clone()) });
        }
    }
    out
}

/// Functions worth cloning at one call site whose actual is a known
/// constant.
///
/// Whole-body parameter specialization needs every caller to agree.  This
/// narrower policy instead admits a call whose known actual exposes local
/// SCCP after the normal MIR clone.  The original body remains for dynamic
/// callers, so no source-level calling convention or symbol changes.
/// As with repeated inlining, the target profile must price the call
/// above the cloned semantic work.
pub fn constant_sites(module: &Module, caller: &Function, constants: &IndexMap<InstId, Vec<Option<ConstantId>>>, costs: &OperationCosts, threshold: Threshold) -> IndexMap<InstId, Candidate> {
    let call_cost = costs.call;
    let Some(budget) = threshold.budget(call_cost) else { return IndexMap::default() };
    let graph = CallGraph::new(module);
    let mut out = IndexMap::default();
    for (_, at) in caller.walk() {
        let Some(name) = callee(&module.context, caller, at) else { continue };
        let known = constants.get(&at).map_or(&[][..], Vec::as_slice);
        if !known.iter().any(Option::is_some) {
            continue;
        }
        let Some(body) = body(module, name) else { continue };
        let semantic = semantic_count(body);
        if semantic < call_cost && semantic <= budget && cloneable(module, &graph, name, body) {
            out.insert(at, Candidate { body: Rc::new(body.clone()) });
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
    context: &Context,
    function: &mut Function,
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
        if _at(context, function, call, candidate)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn _at(context: &Context, function: &mut Function, call: InstId, candidate: &Candidate) -> Result<bool, String> {
    let callee = &*candidate.body;
    let Opcode::Call(info) = &function.instruction(call).opcode else { return Ok(false) };
    if info.function_type != callee.ty || matches!(context.types.get(callee.ty), Type::Function { variadic: true, .. }) {
        return Ok(false);
    }
    let returns = callee.walk().filter(|&(_, inst)| callee.instruction(inst).opcode == Opcode::Ret).collect::<Vec<_>>();
    if returns.is_empty() {
        return Ok(false);
    }
    let void = callee.instruction(returns[0].1).ty;
    let caller = function.parent(call).ok_or("the call is not placed")?;

    // Each formal reads its actual; each callee value its clone.
    let mut swap = callee.parameters().iter().zip(&function.instruction(call).operands).map(|(&formal, &actual)| (formal, actual)).collect::<BTreeMap<ValueId, Operand>>();
    let mut labels = BTreeMap::<BlockId, BlockId>::new();
    let mut after = caller;
    for &block in callee.layout() {
        let made = function.create_block(callee.block(block).name.as_deref());
        function.insert_block(made, Some(after))?;
        labels.insert(block, made);
        after = made;
    }
    let continuation = function.create_block(None);
    function.insert_block(continuation, Some(after))?;

    let mut cloned = Vec::new();
    let mut return_edges = Vec::<(BlockId, Option<Operand>)>::new();
    for &block in callee.layout() {
        for &inst in callee.block(block).instructions() {
            let instruction = callee.instruction(inst);
            if instruction.opcode == Opcode::Ret {
                return_edges.push((labels[&block], instruction.operands.first().copied()));
                continue;
            }
            let name = instruction.result.and_then(|value| callee.value(value).name.clone());
            let made = function.create_instruction(instruction.opcode.clone(), instruction.ty, Vec::new(), instruction.flags.clone(), name.as_deref());
            for (kind, node) in &instruction.metadata {
                function.annotate(made, kind, *node);
            }
            if let (Some(from), Some(to)) = (instruction.result, function.instruction(made).result) {
                swap.insert(from, Operand::Value(to));
            }
            cloned.push((block, inst, made));
        }
    }
    let read = |operand: Operand| match operand {
        Operand::Value(value) => swap[&value],
        Operand::Block(block) => Operand::Block(labels[&block]),
        Operand::Constant(_) => operand,
    };
    // A frame cell is one per activation: it goes to the caller's entry, not
    // into a loop that would grow the stack.
    let first = function.entry().and_then(|entry| function.block(entry).instructions().first().copied());
    for (block, inst, made) in cloned {
        function.set_operands(made, callee.instruction(inst).operands.iter().map(|&operand| read(operand)).collect());
        match (&callee.instruction(inst).opcode, first) {
            (Opcode::Alloca { .. }, Some(first)) => function.insert(made, Position::Before(first))?,
            _ => function.insert(made, Position::End(labels[&block]))?,
        }
    }
    let mut returned = Vec::new();
    for (at, value) in &return_edges {
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(continuation)], Flags::default(), None);
        function.insert(jump, Position::End(*at))?;
        returned.push((value.map(read), *at));
    }

    // What followed the call now follows the inlined body.
    let position = function.block(caller).instructions().iter().position(|&one| one == call).expect("the call's block holds it");
    let tail = function.block(caller).instructions()[position + 1..].to_vec();
    if let Some(result) = function.instruction(call).result {
        let value = if let [(Some(value), _)] = returned[..] {
            value
        } else {
            let operands = returned.iter().flat_map(|&(value, at)| [value.expect("a value returned"), Operand::Block(at)]).collect();
            let phi = function.create_instruction(Opcode::Phi, function.value(result).ty, operands, Flags::default(), None);
            function.insert(phi, Position::End(continuation))?;
            Operand::Value(function.instruction(phi).result.expect("a phi's value"))
        };
        function.replace_all_uses_with(result, value);
    }
    for inst in tail {
        function.move_to(inst, Position::End(continuation))?;
    }
    for successor in function.successors(continuation) {
        let phis = function.block(successor).instructions().iter().copied().take_while(|&one| function.instruction(one).opcode == Opcode::Phi).collect::<Vec<_>>();
        for phi in phis {
            let operands = function.instruction(phi).operands.iter().map(|&one| if one == Operand::Block(caller) { Operand::Block(continuation) } else { one }).collect();
            function.set_operands(phi, operands);
        }
    }
    function.erase(call)?;
    let entry = callee.entry().expect("a defined callee");
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(labels[&entry])], Flags::default(), None);
    function.insert(jump, Position::End(caller))?;
    Ok(true)
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
