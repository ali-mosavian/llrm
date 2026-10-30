//! Argument promotion (LLVM's ArgumentPromotion): a pointer parameter a
//! function only reads at fixed offsets is replaced by the fields read.
//!
//! # How it works
//! Only functions `direct_only` accepts are candidates: internal, never named
//! but as a callee, and every call a plain call with as many arguments as
//! parameters. Functions are visited callees first, and each pointer parameter
//! from the last to the first.
//!
//! A parameter is promoted when every use of it is one of
//!
//! - a non-volatile load at a fixed non-negative offset (through GEPs with
//!   constant offsets, which are erased);
//! - the same parameter passed on, in that position, to the function's own
//!   recursive call.
//!
//! The loads must be of at most four distinct fields (`MOST_FIELDS`), each read
//! as one type, and not overlap. A caller makes the loads before the call, so
//! they must be safe there and read what the function would have read:
//!
//! - a field the function loads in its entry block before any call or write is
//!   the same load, or
//! - the parameter is `dereferenceable` for the field's bytes and nothing
//!   writes them while the function runs (the pointer is `memory::invariant`,
//!   or the function writes no memory).
//!
//! ```text
//!  caller:  call f(ptr %d, i16 %k)           %n = load i16, ptr %d+4
//!  f:       %n = load i16, ptr %d+4  ==>     call f(i16 %n, i16 %k)
//!           ... uses %n                f:    ... uses the parameter %n
//! ```
//!
//! The function's loads of a field are replaced by the new parameter, the
//! parameter list is rewritten, and each call is rewritten to match; a call
//! from the function to itself passing the parameter on passes the fields it
//! holds instead, with no load. The first load's metadata is kept on the
//! caller's load.
//!
//! Profitability is priced by the target for the level being built
//! (`OperationCosts`): the words each call pushes more (`argument`) and the
//! loads each caller gains (`load`), against the loads the function loses. At
//! -Os the price is in bytes and the loads count at every call; otherwise in
//! clocks, where a call inside a loop whose pointer the loop does not change
//! counts no loads (they will be hoisted). A recursive function is promoted
//! when the extra words cost no more than the loads it saves.
//!
//! A slice descriptor read for its length (a Nib `&mut [T]`, a struct by
//! pointer in C, a BASIC array descriptor) is the case it was written for: each
//! call pushed a far pointer that the callee loaded one word through.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(6) nocapture %d,
//!                          i16 %n) {
//! b0:
//!   %at = getelementptr i8, ptr addrspace(1) %d, i16 4
//!   %len = load i16, ptr addrspace(1) %at
//!   %done = icmp sle i16 %n, 0
//!   br i1 %done, label %out, label %again
//! again:
//!   %m = sub i16 %n, 1
//!   %r = call i16 @sum(ptr addrspace(1) %d, i16 %m)
//!   %s = add i16 %r, %len
//!   ret i16 %s
//! out:
//!   ret i16 %len
//! }
//! define i16 @top(ptr addrspace(1) %p) {
//! b0:
//!   %r = call i16 @sum(ptr addrspace(1) %p, i16 3)
//!   ret i16 %r
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define internal i16 @sum(i16 %0, i16 %n) {
//! b0:
//!   %done = icmp sle i16 %n, 0
//!   br i1 %done, label %out, label %again
//!
//! again:
//!   %m = sub i16 %n, 1
//!   %r = call i16 @sum(i16 %0, i16 %m)
//!   %s = add i16 %r, %0
//!   ret i16 %s
//!
//! out:
//!   ret i16 %0
//! }
//!
//! define i16 @top(ptr addrspace(1) %p) {
//! b0:
//!   %0 = getelementptr i8, ptr addrspace(1) %p, i16 4
//!   %1 = load i16, ptr addrspace(1) %0
//!   %r = call i16 @sum(i16 %1, i16 3)
//!   ret i16 %r
//! }
//! ```
//!
//! `@top` loads the length once and `@sum` takes it in the descriptor's place;
//! the recursive call passes it on.
//!
//! From unit test
//! `a_recursive_function_reading_a_descriptor_takes_its_fields_instead` in
//! `argpromotion_tests.rs`.
//!
//! # Runs
//! In the interprocedural step, once, after its inlining loop and before
//! constant propagation of the callers' arguments, so that a length that now
//! crosses the call as a value is propagated into the body. Every body it
//! changes is run through the function pipeline again. Not at -O0.
//!
//! # See also
//! [`crate::deadargs`], [`crate::narrowspace`]; LLVM's ArgumentPromotion.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::pointerfacts::offsets;
use llrm_mir::callgraph::{CallGraph, direct_calls, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::facts::Facts;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::memory;
use llrm_mir::module::{GlobalKind, InstId, MetadataId, Module, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::target::OperationCosts;
use llrm_mir::types::{Type, TypeId};

/// What a function reads through one parameter before it is worth passing the
/// fields.
const MOST_FIELDS: usize = 4;

/// A field read: where, as what, how many bytes, by which loads (the first's
/// metadata is kept).
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

/// The promotions of every function of `module` that has them, callees first;
/// the functions changed.
pub fn promoted(
    module: &mut Module,
    layout: &DataLayout,
    costs: &OperationCosts,
    size: bool,
) -> Vec<GlobalId> {
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
            memory::unsplit(
                &mut crate::interprocedural::function_mut(module, id).1.attrs,
                memory::Moved::IntoArguments,
            );
        }
    }
    changed.into_iter().collect()
}

fn words(
    layout: &DataLayout,
    module: &Module,
    ty: TypeId,
) -> i64 {
    (layout.alloc_size(&module.context.types, ty).max(2) as i64 + 1) / 2
}

#[allow(clippy::too_many_arguments)]
fn planned(
    module: &Module,
    layout: &DataLayout,
    callees: &memory::Callees,
    id: GlobalId,
    parameter: usize,
    calls: &[(GlobalId, InstId)],
    costs: &OperationCosts,
    size: bool,
) -> Option<Plan> {
    let function = module.global(id).function()?;
    let context = &module.context;
    let value = *function.parameters().get(parameter)?;
    if !matches!(context.types.get(function.value(value).ty), Type::Pointer(_))
        || function.calling_convention == llrm_mir::opcode::X86_INTR
    {
        return None;
    }
    let offsets = offsets(context, layout, function);
    let mut fields: BTreeMap<i64, Field> = BTreeMap::new();
    let mut passed_on = Vec::new();
    let mut steps = Vec::new();
    let recursive = calls.iter().any(|&(caller, _)| caller == id);
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
                    // `IsRecursive && Ty->isPointerTy()` in HandleEndUser:
                    // promoting a pointer of a recursive
                    // function might lead to recursive promotion.
                    if recursive && matches!(context.types.get(instruction.ty), Type::Pointer(_)) {
                        return None;
                    }
                    let field = fields
                        .entry(offset)
                        .or_insert_with(
                            || Field {
                                offset,
                                ty: instruction.ty,
                                bytes: layout.alloc_size(&context.types, instruction.ty),
                                loads: Vec::new(),
                            },
                        );
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
                Opcode::Call(_)
                    if held == value
                        && one.index as usize == parameter
                        && memory::callee(context, function, one.user) == Some(id) =>
                {
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
    if fields.is_empty()
        || fields.len() > MOST_FIELDS
        || fields.windows(2).any(|pair| pair[0].offset + pair[0].bytes as i64 > pair[1].offset)
    {
        return None;
    }
    // The callers load before the call what the function loaded after it: where
    // the function loads it before it writes or calls anything, the value
    // is the same and the load happens either way; else its own promise
    // that the bytes are there (`dereferenceable`) and that nothing writes them
    // holds.
    let entry = function.entry()?;
    let early: BTreeSet<InstId> = function
        .iter_block(entry)
        .take_while(|&inst| {
            !matches!(
                function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            )
                && !memory::of(context, callees, function, inst).writes
        })
        .collect();
    let covered = Facts::of(function.parameter_attrs.get(parameter).map_or(&[][..], |one| &one[..])).dereferenceable();
    let unwritten = memory::invariant(context, layout, function, Operand::Value(value))
        || function.walk().all(|(_, inst)| !memory::of(context, callees, function, inst).writes);
    for field in &fields {
        let first = field.loads.iter().all(|load| early.contains(load));
        let promised = covered.is_some_and(|bytes| field.offset as u64 + field.bytes <= bytes) && unwritten;
        if !first && !promised {
            return None;
        }
    }
    let (before, after) = (
        words(layout, module, function.value(value).ty),
        fields.iter().map(|one| words(layout, module, one.ty)).sum::<i64>(),
    );
    let extra = (after - before) * costs.argument;
    let loads = fields.len() as i64 * costs.load;
    // What each call comes to: the loads it makes (none where it passes its own
    // on, or where a loop of the caller leaves them to be hoisted, which
    // only clocks credit) and the words it pushes more.
    let delta: i64 = calls
        .iter()
        .map(|&(caller, inst)| {
            let passes_on = caller == id && passed_on.contains(&inst);
            let own = if passes_on || (!size && hoistable(module, caller, inst, parameter)) { 0 } else { loads };
            if size { own + extra } else { own + extra - loads }
        })
        .sum::<i64>()
        - if size { loads } else { 0 };
    // A recursive function is entered mostly by its own calls: each saves what
    // it loaded, and the first call's loads are paid once.
    let worth = if !size && !passed_on.is_empty() { extra <= loads } else { delta <= 0 };
    worth.then_some(Plan { parameter, fields, passed_on, steps })
}

/// Whether call `inst` of `caller` is in a loop its pointer argument at
/// `parameter` does not change in.
fn hoistable(
    module: &Module,
    caller: GlobalId,
    inst: InstId,
    parameter: usize,
) -> bool {
    let Some(function) = module.global(caller).function() else { return false };
    let Some(block) = function.parent(inst) else { return false };
    let loops = LoopInfo::analyze(function, || DomTree::of(function));
    let Some(held) = loops.get_loop_for(block) else { return false };
    match function.instruction(inst).operands[parameter] {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function.parent(def).is_none_or(|at| !held.contains(at)),
            ValueDef::Argument(_) => true,
        },
        _ => true,
    }
}

/// `plan` made: the function's parameter replaced, each call's argument too.
/// The functions changed.
fn applied(
    module: &mut Module,
    id: GlobalId,
    plan: &Plan,
    calls: &[(GlobalId, InstId)],
) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    let types: Vec<TypeId> = plan.fields.iter().map(|one| one.ty).collect();
    let (ty, first) = {
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let first: Vec<Vec<(String, MetadataId)>> =
            plan.fields.iter().map(|one| function.instruction(one.loads[0]).metadata.clone()).collect();
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
                let step = function.create_instruction(
                    Opcode::GetElementPtr { source: bytes },
                    pointer_ty,
                    vec![pointer, offset],
                    Flags::default(),
                    None,
                );
                function.insert(step, Position::Before(call)).expect("placed");
                Operand::Value(function.instruction(step).result.expect("an address"))
            };
            let load = function.create_instruction(
                Opcode::Load { align: None, volatile: false },
                field.ty,
                vec![at],
                Flags::default(),
                None,
            );
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
