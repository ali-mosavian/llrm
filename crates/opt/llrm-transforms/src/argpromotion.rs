//! LLVM's ArgumentPromotion: a function only direct calls reach takes, in place of a pointer it only
//! reads, the fields it reads through it. Each caller loads them before the call; a recursive call passes
//! its own on. `&mut [T]` in Nib, a struct by pointer in C, a BASIC array descriptor are one case.
//!
//! A parameter is promoted when every use of it is a load of a fixed offset (or the same parameter passed on
//! to the function itself), the loads are safe where the callers make them (`dereferenceable` covers the
//! fields) and read what nothing writes while the function runs (`noalias readonly`, or a function that
//! writes nothing), and there are few of them. Priced by the target for the level being built: the loads
//! each caller gains against the loads the function loses, in clocks or in bytes.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::pointerfacts::offsets;
use llrm_mir::callgraph::{CallGraph, direct_calls, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::facts::Facts;
use llrm_mir::memory;
use llrm_mir::dominators::DominatorTree;
use llrm_mir::loops::LoopInfo;
use llrm_mir::module::{GlobalKind, InstId, MetadataId, Module, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::target::OperationCosts;
use llrm_mir::types::{Type, TypeId};

/// What a function reads through one parameter before it is worth passing the fields.
const MOST_FIELDS: usize = 4;

/// A field read: where, as what, how many bytes, by which loads (the first's metadata is kept).
struct Field {
    offset: i64,
    ty: TypeId,
    bytes: u64,
    loads: Vec<InstId>,
}

struct Plan {
    parameter: usize,
    fields: Vec<Field>,
    /// The function's own calls that pass the parameter itself.
    passed_on: Vec<InstId>,
    /// The addresses it steps to the fields, outermost last.
    steps: Vec<InstId>,
}

/// The promotions of every function of `module` that has them, callees first; the functions changed.
pub fn promoted(module: &mut Module, layout: &DataLayout, costs: &OperationCosts, size: bool) -> Vec<GlobalId> {
    let only = direct_only(module);
    let llrm_mir::callgraph::DirectCalls { sites, refused } = direct_calls(module);
    let callees = memory::callees(module);
    let mut changed = BTreeSet::new();
    for id in CallGraph::new(module).bottom_up() {
        if !only.contains(&id) || refused.contains(&id) {
            continue;
        }
        let Some(calls) = sites.get(&id) else { continue };
        let count = module.global(id).function().map_or(0, |one| one.parameters().len());
        for parameter in (0..count).rev() {
            let Some(plan) = planned(module, layout, &callees, id, parameter, calls, costs, size) else { continue };
            changed.extend(applied(module, id, &plan, calls));
            changed.insert(id);
        }
    }
    changed.into_iter().collect()
}

fn words(layout: &DataLayout, module: &Module, ty: TypeId) -> i64 {
    (layout.alloc_size(&module.context.types, ty).max(2) as i64 + 1) / 2
}

#[allow(clippy::too_many_arguments)]
fn planned(module: &Module, layout: &DataLayout, callees: &memory::Callees, id: GlobalId, parameter: usize, calls: &[(GlobalId, InstId)], costs: &OperationCosts, size: bool) -> Option<Plan> {
    let function = module.global(id).function()?;
    let context = &module.context;
    let value = *function.parameters().get(parameter)?;
    if !matches!(context.types.get(function.value(value).ty), Type::Pointer(_)) || function.calling_convention == llrm_mir::opcode::X86_INTR {
        return None;
    }
    let offsets = offsets(context, layout, function);
    let mut fields: BTreeMap<i64, Field> = BTreeMap::new();
    let mut passed_on = Vec::new();
    let mut steps = Vec::new();
    let mut pending = vec![value];
    let mut seen = BTreeSet::new();
    while let Some(held) = pending.pop() {
        if !seen.insert(held) {
            continue;
        }
        for one in function.users(held) {
            let instruction = function.instruction(one.user);
            match &instruction.opcode {
                Opcode::Load { volatile: false, .. } if one.index == 0 => {
                    let (Operand::Value(base), offset) = offsets.fixed(Operand::Value(held))? else { return None };
                    if base != value || offset < 0 {
                        return None;
                    }
                    let field = fields.entry(offset).or_insert_with(|| Field { offset, ty: instruction.ty, bytes: layout.alloc_size(&context.types, instruction.ty), loads: Vec::new() });
                    if field.ty != instruction.ty {
                        return None;
                    }
                    field.loads.push(one.user);
                }
                Opcode::GetElementPtr { .. } if one.index == 0 => {
                    let result = instruction.result?;
                    let (Operand::Value(base), _) = offsets.fixed(Operand::Value(result))? else { return None };
                    if base != value {
                        return None;
                    }
                    steps.push(one.user);
                    pending.push(result);
                }
                Opcode::Call(_) if held == value && one.index as usize == parameter && memory::callee(context, function, one.user) == Some(id) => {
                    if instruction.operands.iter().filter(|&&operand| operand == Operand::Value(value)).count() != 1 {
                        return None;
                    }
                    passed_on.push(one.user);
                }
                _ => return None,
            }
        }
    }
    let fields: Vec<Field> = fields.into_values().collect();
    if fields.is_empty() || fields.len() > MOST_FIELDS || fields.windows(2).any(|pair| pair[0].offset + pair[0].bytes as i64 > pair[1].offset) {
        return None;
    }
    // The callers load before the call what the function loaded after it: where the function loads it
    // before it writes or calls anything, the value is the same and the load happens either way; else
    // its own promise that the bytes are there (`dereferenceable`) and that nothing writes them holds.
    let entry = function.entry()?;
    let early: BTreeSet<InstId> = function
        .block(entry)
        .instructions()
        .iter()
        .copied()
        .take_while(|&inst| !matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)) && !memory::of(context, callees, function, inst).writes)
        .collect();
    let covered = Facts::of(function.parameter_attrs.get(parameter).map_or(&[][..], |one| &one[..])).dereferenceable();
    let unwritten = memory::invariant(context, layout, function, Operand::Value(value)) || function.walk().all(|(_, inst)| !memory::of(context, callees, function, inst).writes);
    for field in &fields {
        let first = field.loads.iter().all(|load| early.contains(load));
        let promised = covered.is_some_and(|bytes| field.offset as u64 + field.bytes <= bytes) && unwritten;
        if !first && !promised {
            return None;
        }
    }
    let (before, after) = (words(layout, module, function.value(value).ty), fields.iter().map(|one| words(layout, module, one.ty)).sum::<i64>());
    let extra = (after - before) * costs.argument;
    let loads = fields.len() as i64 * costs.load;
    // What each call comes to: the loads it makes (none where it passes its own on, or where a loop of
    // the caller leaves them to be hoisted, which only clocks credit) and the words it pushes more.
    let delta: i64 = calls
        .iter()
        .map(|&(caller, inst)| {
            let passes_on = caller == id && passed_on.contains(&inst);
            let own = if passes_on || (!size && hoistable(module, caller, inst, parameter)) { 0 } else { loads };
            if size { own + extra } else { own + extra - loads }
        })
        .sum::<i64>()
        - if size { loads } else { 0 };
    // A recursive function is entered mostly by its own calls: each saves what it loaded, and the first
    // call's loads are paid once.
    let worth = if !size && !passed_on.is_empty() { extra <= loads } else { delta <= 0 };
    worth.then_some(Plan { parameter, fields, passed_on, steps })
}

/// Whether call `inst` of `caller` is in a loop its pointer argument at `parameter` does not change in.
fn hoistable(module: &Module, caller: GlobalId, inst: InstId, parameter: usize) -> bool {
    let Some(function) = module.global(caller).function() else { return false };
    let Some(block) = function.parent(inst) else { return false };
    let loops = LoopInfo::new(function, &DominatorTree::new(function));
    let Some(held) = loops.loop_of(block) else { return false };
    match function.instruction(inst).operands[parameter] {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function.parent(def).is_none_or(|at| !held.blocks.contains(&at)),
            ValueDef::Argument(_) => true,
        },
        _ => true,
    }
}

/// `plan` made: the function's parameter replaced, each call's argument too. The functions changed.
fn applied(module: &mut Module, id: GlobalId, plan: &Plan, calls: &[(GlobalId, InstId)]) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    let types: Vec<TypeId> = plan.fields.iter().map(|one| one.ty).collect();
    let (ty, first) = {
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let first: Vec<Vec<(String, MetadataId)>> = plan.fields.iter().map(|one| function.instruction(one.loads[0]).metadata.clone()).collect();
        let made = function.insert_parameters(context, plan.parameter, &types);
        for (field, &held) in plan.fields.iter().zip(&made) {
            for &load in &field.loads {
                let result = function.instruction(load).result.expect("a load's value");
                function.replace_all_uses_with(result, Operand::Value(held));
                function.erase(load).expect("an unused load goes");
            }
        }
        for &step in plan.steps.iter().rev() {
            function.erase(step).expect("an unused step goes");
        }
        // A call that passes the parameter on passes what it was given instead.
        let held: Vec<Operand> = made.iter().map(|&one| Operand::Value(one)).collect();
        for &call in &plan.passed_on {
            function.replace_argument(call, plan.parameter, &held, function.ty);
        }
        function.remove_parameter(context, plan.parameter + made.len());
        for &call in &plan.passed_on {
            function.set_call_type(call, function.ty);
        }
        (function.ty, first)
    };
    for &(caller, call) in calls {
        if caller == id && plan.passed_on.contains(&call) {
            continue;
        }
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[caller.0 as usize].kind else { unreachable!("a caller") };
        let pointer = function.instruction(call).operands[plan.parameter];
        let pointer_ty = function.operand_type(context, pointer).expect("a typed argument");
        let (bytes, index) = (context.types.int(8), context.types.int(16));
        let mut loaded = Vec::new();
        for (field, metadata) in plan.fields.iter().zip(&first) {
            let at = if field.offset == 0 {
                pointer
            } else {
                let offset = Operand::Constant(context.int(index, i128::from(field.offset)));
                let step = function.create_instruction(Opcode::GetElementPtr { source: bytes }, pointer_ty, vec![pointer, offset], Flags::default(), None);
                function.insert(step, Position::Before(call)).expect("placed");
                Operand::Value(function.instruction(step).result.expect("an address"))
            };
            let load = function.create_instruction(Opcode::Load { align: None, volatile: false }, field.ty, vec![at], Flags::default(), None);
            function.insert(load, Position::Before(call)).expect("placed");
            for (kind, node) in metadata {
                function.annotate(load, kind, *node);
            }
            loaded.push(Operand::Value(function.instruction(load).result.expect("a load's value")));
        }
        function.replace_argument(call, plan.parameter, &loaded, ty);
        changed.insert(caller);
    }
    changed
}

