//! Adapted from llrm-core's `optimize/inline.rs`, the port of
//! `qbopt/optimize/inline.py`: selective whole-module MIR inlining.
//!
//! Inlining is a CFG operation, not a call peephole: split the caller at the
//! call, clone the callee's blocks, bind formal parameters to the actual
//! operands, and join every return back to the continuation.  The ordinary
//! body pipeline then simplifies the result.
//!
//! The initial policy covered leaf procedures called once.  It also admits a
//! straight-line private leaf at every direct call site when the target-priced
//! call work exceeds the semantic work duplicated by cloning.  In both cases
//! the ordinary body pipeline simplifies the result; MIR chooses from semantic
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
//! old `sealed` flag is a definition; a leaf touches no memory, which
//! replaces admitting only formal loads.  SSA is checked by the verifier
//! after each pass, not here.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::context::{ConstantId, Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::memory::callee;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;

use crate::profit::OperationCosts;

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

/// Private pure leaves worth moving into their direct callers.
///
/// A single-use body disappears after expansion, so it only has to fit the
/// normal CFG budget.  A repeated body duplicates its semantic work once per
/// additional caller.  Admit that only for a straight-line leaf, and only
/// when the profile's total direct-call cost is greater than the duplicate
/// work.  This lets a short arithmetic helper disappear at every site while
/// keeping branchy or code-growing helpers out of the allocator's region.
pub fn candidates(
    module: &Module,
    calls: &Counter,
    private: &BTreeSet<GlobalId>,
    pure: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    threshold: Threshold,
) -> IndexMap<GlobalId, Candidate> {
    let call_cost = costs.call;
    let Some(budget) = threshold.budget(call_cost) else { return IndexMap::default() };
    let mut out = IndexMap::default();
    for &name in private.intersection(pure) {
        let Some(body) = body(module, name) else { continue };
        let semantic = semantic_count(body);
        let count = calls.get(&name).copied().unwrap_or(0);
        let repeated = semantic * (count - 1);
        let profitable = count == 1 || (_straight(body) && repeated < count * call_cost);
        if count != 0 && semantic <= budget && profitable && _leaf(&module.context, body) {
            out.insert(name, Candidate { body: Rc::new(body.clone()) });
        }
    }
    out
}

/// Private pure leaves worth cloning at one constant direct-call site.
///
/// Whole-body parameter specialization needs every caller to agree.  This
/// narrower policy instead admits a call whose known actual exposes local
/// SCCP after the normal MIR clone.  The original body remains for dynamic
/// callers, so no source-level calling convention or symbol changes.
/// As with repeated-leaf inlining, the target profile must price the call
/// above the cloned semantic work.
pub fn constant_sites(
    module: &Module,
    caller: &Function,
    constants: &IndexMap<InstId, Vec<Option<ConstantId>>>,
    private: &BTreeSet<GlobalId>,
    pure: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    threshold: Threshold,
) -> IndexMap<InstId, Candidate> {
    let call_cost = costs.call;
    let Some(budget) = threshold.budget(call_cost) else { return IndexMap::default() };
    let mut out = IndexMap::default();
    for (_, at) in caller.walk() {
        let Some(name) = callee(&module.context, caller, at) else { continue };
        let known = constants.get(&at).map_or(&[][..], Vec::as_slice);
        if !known.iter().any(Option::is_some) {
            continue;
        }
        if !private.contains(&name) || !pure.contains(&name) {
            continue;
        }
        let Some(body) = body(module, name) else { continue };
        let semantic = semantic_count(body);
        if semantic < call_cost && semantic <= budget && _leaf(&module.context, body) {
            out.insert(at, Candidate { body: Rc::new(body.clone()) });
        }
    }
    out
}

/// Whether cloning the body duplicates no control-flow structure.
fn _straight(body: &Function) -> bool {
    body.layout().len() == 1
}

/// Whether a body does nothing but compute its result: no call, no memory,
/// nothing that may trap and no floating work.
fn _leaf(context: &Context, body: &Function) -> bool {
    if body.is_declaration() {
        return false;
    }
    let floating = |operand: Operand| body.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), Type::Float(_)));
    let mut returns = false;
    for (_, inst) in body.walk() {
        let instruction = body.instruction(inst);
        let forbidden = match instruction.opcode {
            Opcode::Call(_) | Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume => true,
            Opcode::Load { .. } | Opcode::Store { .. } | Opcode::Alloca { .. } => true,
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => true,
            _ => false,
        };
        if forbidden || instruction.result.is_some_and(|value| floating(Operand::Value(value))) || instruction.operands.iter().any(|&operand| floating(operand)) {
            return false;
        }
        returns |= instruction.opcode == Opcode::Ret;
    }
    returns
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
    for (block, inst, made) in cloned {
        function.set_operands(made, callee.instruction(inst).operands.iter().map(|&operand| read(operand)).collect());
        function.insert(made, Position::End(labels[&block]))?;
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
