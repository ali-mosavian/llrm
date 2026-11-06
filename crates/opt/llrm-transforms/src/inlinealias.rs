//! `AddAliasScopeMetadata`
//! ([L1217](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1217)):
//! the `noalias` arguments of an inlined call become alias scopes.
//!
//! # How it works
//! Each `noalias` argument the callee uses gets a new scope in a new domain
//! and a `llvm.experimental.noalias.scope.decl` before the call
//! ([`declare_scopes`]). In the copy, an access whose pointers are all based on
//! `noalias` arguments is in the scopes of those it is based on, and an access
//! is `!noalias` the scopes of the arguments it is not based on, unless that
//! argument may have been captured before it ([`scoped`]).
//!
//! # Example
//! `f(noalias %p, %q)` with `store %p` and `load %q` inlined: the store is in
//! `p`'s scope and the load is `!noalias` it.
//!
//! # Deviations
//! - names are the callee's where the module's declarations are at hand, else
//!   generic.
//! - IA2 (owner w9, exit 2026-12-15): `CalleeAAR` is the call's and the
//!   callee's attributes and `GlobalsAA`'s function effects (where the walk has
//!   them), not a query of every alias analysis of the callee.
//! - A copy is never simplified while it is made (`isSimplified`).
//!
//! # Provides
//! [`Scopes`], [`declare_scopes`], [`scoped`].
//!
//! # Needs
//! [`crate::inlinemetadata`] for the lists, `llrm_analysis::capturetracking`.

use llrm_analysis::aa::{IRMemLocation, MemoryEffects};
use llrm_analysis::capturetracking::{CaptureComponents, pointer_may_be_captured_before};
use llrm_analysis::globalsmodref::GlobalsModRef;
use llrm_mir::Context;
use llrm_mir::context::{Constant, ConstantExpr, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::facts::Fact;
use llrm_mir::module::{
    Function, GlobalValue, InstId, MetadataId, MetadataNode, MetadataOperand, Operand, ValueDef, ValueId,
};
use llrm_mir::opcode::{AtomicKind, CallInfo, CastOp, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::underlying::{MAX_LOOKUP_SEARCH_DEPTH, get_underlying_objects};

use crate::inlinemetadata::{DECL, NOALIAS, Nodes, SCOPE};

/// The scope made for each `noalias` argument of a call, by parameter.
pub struct Scopes(Vec<(usize, MetadataId)>);

fn names(
    declared: &Declared,
    name: Option<&str>,
) -> String {
    let _ = declared;
    name.unwrap_or("inlined").to_owned()
}

/// The scopes for the `noalias` arguments of `call` that `body` uses, and the
/// declaration of each before `call`.
pub fn declare_scopes(
    context: &mut Context,
    function: &mut Function,
    call: InstId,
    body: &Function,
    declared: &mut Declared,
) -> Option<Scopes> {
    let Opcode::Call(info) = &function.instruction(call).opcode else { return None };
    let site = info.argument_attrs.clone();
    let mut arguments = Vec::new();
    for (index, &parameter) in body.parameters().iter().enumerate() {
        let stated = site.get(index).is_some_and(|attrs| Fact::NoAlias.stated_by(attrs))
            || body.parameter_attrs.get(index).is_some_and(|attrs| Fact::NoAlias.stated_by(attrs));
        if stated && !body.users(parameter).is_empty() {
            arguments.push((index, parameter));
        }
    }
    if arguments.is_empty() {
        return None;
    }
    let callee = llrm_mir::memory::callee(context, function, call).and_then(|id| {
        declared
            .declarations()
            .and_then(|all| all.get(id.0 as usize))
            .and_then(|one| one.name.as_deref().map(str::to_owned))
    });
    let callee = names(declared, callee.as_deref());
    let this = declared.next_node();
    let domain = declared.node(MetadataNode {
        distinct: true,
        operands: vec![MetadataOperand::Node(this), MetadataOperand::String(callee.clone())],
    });
    let void = context.types.void();
    let ty = context.types.intern(Type::Function { returns: void, parameters: Vec::new(), variadic: false });
    let decl = declared.declare("llvm.experimental.noalias.scope.decl", ty);
    let target = context.constant(Constant { ty: context.types.ptr(0), kind: ConstantKind::Global(decl) });
    let mut made = Vec::new();
    for (position, &(index, parameter)) in arguments.iter().enumerate() {
        let name = match body.value(parameter).name.as_deref() {
            Some(name) => format!("{callee}: %{name}"),
            None => format!("{callee}: argument {position}"),
        };
        let this = declared.next_node();
        let scope = declared.node(MetadataNode {
            distinct: true,
            operands: vec![MetadataOperand::Node(this), MetadataOperand::Node(domain), MetadataOperand::String(name)],
        });
        let list = declared.node(MetadataNode { distinct: false, operands: vec![MetadataOperand::Node(scope)] });
        let info = CallInfo {
            function_type: ty,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: Vec::new(),
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let inst = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![Operand::Constant(target)],
            Flags::default(),
            None,
        );
        function.insert(inst, Position::Before(call)).expect("a placed call");
        function.set_metadata(inst, DECL, Some(list));
        made.push((index, scope));
    }
    Some(Scopes(made))
}

/// `isEscapeSource`.
fn escape_source(
    function: &Function,
    context: &Context,
    globals: &[GlobalValue],
    value: Operand,
) -> bool {
    match value {
        Operand::Value(one) => {
            let ValueDef::Instruction(inst) = function.value(one).def else { return false };
            let op = function.instruction(inst);
            match &op.opcode {
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    // `isIntrinsicReturningPointerAliasingArgumentWithoutCapturing`
                    let name = llrm_mir::memory::callee(context, function, inst)
                        .and_then(|id| globals.get(id.0 as usize))
                        .and_then(|one| one.name.as_deref());
                    !matches!(
                        name,
                        Some("llvm.launder.invariant.group" | "llvm.ptrmask" | "llvm.threadlocal.address")
                    )
                }
                Opcode::Load { .. } | Opcode::ExtractValue(_) | Opcode::Cast(CastOp::IntToPtr) => true,
                _ => false,
            }
        }
        Operand::Constant(id) => {
            matches!(
                &context.get(id).kind,
                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, .. })
            )
        }
        Operand::Block(_) => false,
    }
}

/// `isIdentifiedObject`.
fn identified(
    body: &Function,
    context: &Context,
    value: Operand,
) -> bool {
    match value {
        Operand::Value(one) => match body.value(one).def {
            ValueDef::Instruction(inst) => match &body.instruction(inst).opcode {
                Opcode::Alloca { .. } => true,
                Opcode::Call(info) | Opcode::Invoke(info) => Fact::NoAlias.stated_by(&info.return_attrs),
                _ => false,
            },
            _ => body.parameters().iter().position(|&one_| one_ == one).is_some_and(|at| {
                body.parameter_attrs
                    .get(at)
                    .is_some_and(
                        |attrs| Fact::NoAlias.stated_by(attrs)
                            || attrs.iter().any(|a| {
                                matches!(
                                    a,
                                    llrm_mir::opcode::Attribute::Flag(n) | llrm_mir::opcode::Attribute::Type(n, _) if n == "byval"
                                )
                            }),
                    )
            }),
        },
        Operand::Constant(id) => matches!(context.get(id).kind, ConstantKind::Global(_)),
        Operand::Block(_) => false,
    }
}

/// The effects of the call `inst` of `body`: its attributes and its callee's.
fn call_effects(
    body: &Function,
    context: &Context,
    globals: &[GlobalValue],
    known: Option<&GlobalsModRef>,
    inst: InstId,
) -> MemoryEffects {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &body.instruction(inst).opcode else {
        return MemoryEffects::unknown();
    };
    let site = MemoryEffects::of_attrs(&info.attrs);
    let direct = llrm_mir::memory::callee(context, body, inst);
    // `AAResults::getMemoryEffects(Call)`: the call's, the callee's, and what
    // `GlobalsAA` worked out.
    let site = match (direct, known) {
        (Some(id), Some(known)) => site.intersect(known.get_memory_effects(id)),
        _ => site,
    };
    match direct.and_then(|id| globals.get(id.0 as usize)).and_then(|one| one.function()) {
        Some(callee) => site.intersect(MemoryEffects::of_attrs(&callee.attrs)),
        None => site,
    }
}

/// `AddAliasScopeMetadata` over the copies of `body`'s instructions.
#[allow(clippy::too_many_arguments)]
pub fn scoped(
    function: &mut Function,
    context: &Context,
    layout: &DataLayout,
    body: &Function,
    copies: &[InstId],
    scopes: &Scopes,
    module: &[MetadataNode],
    effects: Option<&GlobalsModRef>,
    declared: &mut Declared,
) {
    let globals = declared.declarations().map(<[GlobalValue]>::to_vec).unwrap_or_default();
    let tree = DomTree::of(body);
    let originals: Vec<InstId> =
        body.walk().filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret).map(|(_, inst)| inst).collect();
    if originals.len() != copies.len() {
        return;
    }
    let arguments: Vec<(ValueId, MetadataId)> =
        scopes.0.iter().map(|&(index, scope)| (body.parameters()[index], scope)).collect();
    let site_noalias: Vec<bool> = {
        // A parameter is `noalias` for the site when it is one of `scopes`.
        body.parameters().iter().map(|&p| arguments.iter().any(|&(a, _)| a == p)).collect()
    };
    let query = Query {
        context,
        globals: &globals,
        metadata: module,
        function: body,
        layout,
        dt: None,
        assumptions: None,
        dom_conditions: None,
        cond_context: None,
        cxt: None,
    };
    let first = declared.next_node().0 - declared.nodes().len() as u32;
    let mut nodes = Nodes::new(module, first);
    for (&original, &copy) in originals.iter().zip(copies) {
        let op = body.instruction(original);
        let mut pointers: Vec<Operand> = Vec::new();
        let (mut arg_mem_only, mut is_call) = (false, false);
        match &op.opcode {
            Opcode::Load { .. } => pointers.push(op.operands[0]),
            Opcode::Store { .. } => pointers.push(op.operands[1]),
            Opcode::Atomic(info) => match info.kind {
                AtomicKind::Fence => {}
                AtomicKind::Load | AtomicKind::Rmw(_) | AtomicKind::CmpXchg { .. } => pointers.push(op.operands[0]),
                AtomicKind::Store => pointers.push(op.operands[1]),
            },
            Opcode::Call(_) | Opcode::Invoke(_) => {
                let effects = call_effects(body, context, &globals, effects, original);
                if effects.does_not_access_memory() {
                    continue;
                }
                is_call = true;
                let only = |at: &[IRMemLocation]| at.iter().all(|&one| effects.mod_ref(one).is_no_mod_ref());
                if only(&[IRMemLocation::ArgMem, IRMemLocation::ErrnoMem, IRMemLocation::Other]) {
                    continue; // only inaccessible memory
                }
                arg_mem_only = effects.only_accesses_arg_memory();
                for &argument in &op.operands[..op.operands.len() - 1] {
                    let pointer = body
                        .operand_type(context, argument)
                        .is_some_and(|ty| matches!(context.types.get(ty), Type::Pointer(_)));
                    if pointer {
                        pointers.push(argument);
                    }
                }
            }
            _ => continue,
        }
        if pointers.is_empty() && !is_call {
            continue;
        }
        let mut objects: Vec<Operand> = Vec::new();
        for &pointer in &pointers {
            for found in get_underlying_objects(&query, pointer, None, MAX_LOOKUP_SEARCH_DEPTH) {
                if !objects.contains(&found) {
                    objects.push(found);
                }
            }
        }
        let (mut needs_no_capture, mut aliasing, mut unknown) = (false, false, false);
        for &object in &objects {
            let non_pointer = match object {
                Operand::Constant(id) => matches!(
                    context.get(id).kind,
                    ConstantKind::Int(_)
                        | ConstantKind::Float(_)
                        | ConstantKind::Float80(_)
                        | ConstantKind::Float128(_)
                        | ConstantKind::Null
                        | ConstantKind::Undef
                        | ConstantKind::Poison
                ),
                _ => false,
            };
            if non_pointer {
                continue;
            }
            let parameter = match object {
                Operand::Value(one) => body.parameters().iter().position(|&p| p == one),
                _ => None,
            };
            match parameter {
                Some(at) => aliasing |= !site_noalias[at],
                None => aliasing = true,
            }
            if escape_source(body, context, &globals, object) {
                needs_no_capture = true;
            } else if parameter.is_none() && !identified(body, context, object) {
                unknown = true;
            }
        }
        if unknown {
            continue;
        }
        if is_call && !arg_mem_only {
            needs_no_capture = true;
        }
        let mut no_alias: Vec<MetadataId> = Vec::new();
        for &(argument, scope) in &arguments {
            if objects.contains(&Operand::Value(argument)) {
                continue;
            }
            if !needs_no_capture
                || pointer_may_be_captured_before(
                    body,
                    &globals,
                    context,
                    &tree,
                    argument,
                    false,
                    original,
                    false,
                    CaptureComponents::PROVENANCE,
                )
                .captures_nothing()
            {
                no_alias.push(scope);
            }
        }
        if !no_alias.is_empty() {
            let list = nodes.list(declared, no_alias);
            let joined = nodes.concatenated(declared, attached(function, copy, NOALIAS), Some(list));
            function.set_metadata(copy, NOALIAS, joined);
        }
        let mut in_scope: Vec<MetadataId> = Vec::new();
        if !aliasing && (!is_call || arg_mem_only) {
            for &(argument, scope) in &arguments {
                if objects.contains(&Operand::Value(argument)) {
                    in_scope.push(scope);
                }
            }
        }
        if !in_scope.is_empty() {
            let list = nodes.list(declared, in_scope);
            let joined = nodes.concatenated(declared, attached(function, copy, SCOPE), Some(list));
            function.set_metadata(copy, SCOPE, joined);
        }
    }
}

fn attached(
    function: &Function,
    inst: InstId,
    kind: &str,
) -> Option<MetadataId> {
    function.instruction(inst).metadata.iter().find(|(one, _)| one == kind).map(|&(_, node)| node)
}
