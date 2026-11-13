//! `WebAssemblyFixFunctionBitcasts`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblyFixFunctionBitcasts.cpp)):
//! a call whose function type is not its callee's goes through a private
//! wrapper of the call's type, which calls the callee with the arguments that
//! fit (extra ones dropped, missing ones poison) and returns what fits.
//! wasm's `call` is typed, so the mismatch the C of `int f();` allows would
//! otherwise be an invalid module.
//!
//! A zero-argument `int main()` gets the wrapper `main(i32, ptr)` that C
//! startup calls; the body is renamed `__original_main`, and a `__main_void`
//! alias follows the body.
//!
//! DEVIATE: LLVM visits the uses of a function in use-list order, which is
//! not source order; the wrappers here are made in source order. A wrapper's
//! parameter attributes beyond its own parameters are dropped. Exit: none,
//! only the order of the wrappers in the module differs.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{GlobalKind, Linkage, Module, Operand};
use llrm_mir::{CastOp, Constant, ConstantId, ConstantKind, GlobalId, InstId, Opcode, Type, TypeId};
use llrm_support::hash::HashMap;

/// LLVM's `CallingConv::Swift` and `SwiftTail`: their bitcasts may differ.
const SWIFT: u32 = 16;
const SWIFT_TAIL: u32 = 20;

/// A call to rewrite: instruction `inst` of the function `caller`, or the
/// artificial call of `main` that makes its wrapper.
enum Site {
    Call { caller: GlobalId, inst: InstId },
    Main,
}

struct Signature {
    returns: TypeId,
    parameters: Vec<TypeId>,
    variadic: bool,
}

/// What `createWrapper` makes of a callee and a call type.
#[derive(PartialEq)]
enum Plan {
    /// Every difference is a no-op cast, or too structured to tell.
    Skip,
    /// The types can never agree: a body of `unreachable`.
    Invalid,
    Wrapper,
}

pub fn fix(module: &mut Module) -> Result<(), String> {
    let Some(layout) = module.datalayout.as_deref().map(DataLayout::parse).transpose()? else { return Ok(()) };
    let mut calls: HashMap<GlobalId, Vec<(GlobalId, InstId, TypeId)>> = HashMap::default();
    for (caller, _, function) in module.functions() {
        for &block in function.layout() {
            for inst in function.iter_block(block) {
                let instruction = function.instruction(inst);
                let Opcode::Call(info) = &instruction.opcode else { continue };
                let Some(&Operand::Constant(callee)) = instruction.operands.last() else { continue };
                let ConstantKind::Global(target) = module.context.get(callee).kind else { continue };
                if module.global(target).function().is_some_and(|callee| callee.ty != info.function_type) {
                    calls.entry(target).or_default().push((caller, inst, info.function_type));
                }
            }
        }
    }
    let ids: Vec<GlobalId> = module.functions().map(|(id, ..)| id).collect();
    let main_type = {
        let types = &module.context.types;
        types.intern(Type::Function {
            returns: types.int(32),
            parameters: vec![types.int(32), types.ptr(0)],
            variadic: false,
        })
    };
    let mut uses: Vec<(Site, GlobalId, TypeId)> = Vec::new();
    let mut main = None;
    for &id in &ids {
        let function = module.global(id).function().expect("a function");
        if matches!(function.calling_convention, SWIFT | SWIFT_TAIL) {
            continue;
        }
        for &(caller, inst, ty) in calls.get(&id).into_iter().flatten() {
            uses.push((Site::Call { caller, inst }, id, ty));
        }
        if module.global(id).name.as_deref() == Some("main") {
            main = Some(id);
            if wants_main_fix(module, function.ty, main_type) {
                uses.push((Site::Main, id, main_type));
            }
        }
    }
    let mut wrappers: HashMap<(GlobalId, TypeId), Option<GlobalId>> = HashMap::default();
    let mut main_call = false;
    for (site, callee, ty) in uses {
        let wrapper = match wrappers.get(&(callee, ty)) {
            Some(&made) => made,
            None => {
                let made = wrap(module, &layout, callee, ty)?;
                wrappers.insert((callee, ty), made);
                made
            }
        };
        match site {
            Site::Main => main_call = true,
            Site::Call { caller, inst } => {
                let Some(wrapper) = wrapper else { continue };
                let reference = reference(module, wrapper);
                let GlobalKind::Function(body) = &mut module.globals[caller.0 as usize].kind else { unreachable!() };
                let callee = body.instruction(inst).operands.len() - 1;
                body.set_operand(inst, callee, Operand::Constant(reference));
            }
        }
    }
    // The wrapper is what startup calls: it takes the name, the body the other.
    if main_call && let Some(main) = main {
        let declaration = module.global(main).function().is_some_and(|one| one.is_declaration());
        module.globals[main.0 as usize].name = "__original_main".to_owned().into();
        if !declaration && let Some(wrapper) = wrappers[&(main, main_type)] {
            let (linkage, visibility) = (module.global(main).linkage, module.global(main).visibility);
            let made = &mut module.globals[wrapper.0 as usize];
            made.name = "main".to_owned().into();
            made.linkage = linkage;
            made.visibility = visibility;
        }
    }
    Ok(())
}

fn reference(
    module: &Module,
    global: GlobalId,
) -> ConstantId {
    let ty = module.context.types.ptr(module.global(global).address_space);
    module.context.constant(Constant { ty, kind: ConstantKind::Global(global) })
}

fn signature(
    module: &Module,
    ty: TypeId,
) -> Signature {
    let Type::Function { returns, parameters, variadic } = module.context.types.get(ty).clone() else {
        unreachable!("a function type")
    };
    Signature { returns, parameters, variadic }
}

/// `shouldFixMainFunction`: only the zero-argument form is fixed, so that the
/// linker reports every other mismatch.
fn wants_main_fix(
    module: &Module,
    ty: TypeId,
    main_type: TypeId,
) -> bool {
    let (main, wanted) = (signature(module, ty), signature(module, main_type));
    main.returns == wanted.returns && main.parameters.is_empty() && !main.variadic
}

fn is_struct(
    module: &Module,
    ty: TypeId,
) -> bool {
    matches!(
        module.context.types.get(ty),
        Type::Struct { .. } | Type::Named(_)
    )
}

/// `createWrapper`'s decisions, made before anything is emitted.
fn plan(
    module: &Module,
    layout: &DataLayout,
    callee: &Signature,
    wanted: &Signature,
) -> Plan {
    let types = &module.context.types;
    let castable = |from, to| layout.is_bit_or_noop_pointer_castable(types, from, to);
    let mut needed = callee.parameters.len() != wanted.parameters.len()
        || callee.variadic != wanted.variadic
        || callee.returns != wanted.returns;
    for (&given, &expected) in wanted.parameters.iter().zip(&callee.parameters) {
        if given == expected || castable(given, expected) {
            continue;
        }
        if is_struct(module, given) || is_struct(module, expected) {
            needed = false;
        } else {
            return Plan::Invalid;
        }
    }
    if needed {
        let (expected, returned, void) = (callee.returns, wanted.returns, types.void());
        if returned != void && expected != void && returned != expected && !castable(expected, returned) {
            if is_struct(module, returned) || is_struct(module, expected) {
                needed = false;
            } else {
                return Plan::Invalid;
            }
        }
    }
    if needed { Plan::Wrapper } else { Plan::Skip }
}

/// `CreateBitOrPointerCast` of `value` from `from` to `to`.
fn cast(
    builder: &mut llrm_mir::build::Builder<'_>,
    value: Operand,
    from: TypeId,
    to: TypeId,
) -> Operand {
    if from == to {
        return value;
    }
    let op = match (builder.context.types.get(from), builder.context.types.get(to)) {
        (Type::Pointer(_), Type::Int(_)) => CastOp::PtrToInt,
        (Type::Int(_), Type::Pointer(_)) => CastOp::IntToPtr,
        _ => CastOp::BitCast,
    };
    builder.cast(op, value, to, "cast")
}

fn unique(
    module: &Module,
    base: &str,
) -> String {
    let mut name = base.to_owned();
    let mut next = 1;
    while module.named(&name).is_some() {
        name = format!("{base}.{next}");
        next += 1;
    }
    name
}

fn wrap(
    module: &mut Module,
    layout: &DataLayout,
    callee: GlobalId,
    ty: TypeId,
) -> Result<Option<GlobalId>, String> {
    let function_type = module.global(callee).function().expect("a function").ty;
    let (given, wanted) = (signature(module, function_type), signature(module, ty));
    let plan = plan(module, layout, &given, &wanted);
    if plan == Plan::Skip {
        return Ok(None);
    }
    let base = module.global(callee).name.as_deref().unwrap_or("").to_owned();
    let suffix = if plan == Plan::Invalid { "_bitcast_invalid" } else { "_bitcast" };
    let name = unique(module, &format!("{base}{suffix}"));
    let wrapper = module.add_function(&name, ty, Linkage::Private)?;
    let target = module.global(callee).function().expect("a function");
    let (attrs, return_attrs, mut parameter_attrs) =
        (target.attrs.clone(), target.return_attrs.clone(), target.parameter_attrs.clone());
    parameter_attrs.resize(wanted.parameters.len(), Vec::new());
    let target = Operand::Constant(reference(module, callee));
    let poison = |ty| Operand::Constant(module.context.constant(Constant { ty, kind: ConstantKind::Poison }));
    let missing: Vec<Operand> = given.parameters.iter().skip(wanted.parameters.len()).map(|&ty| poison(ty)).collect();
    let dummy = poison(wanted.returns);
    {
        let GlobalKind::Function(body) = &mut module.globals[wrapper.0 as usize].kind else { unreachable!() };
        body.attrs = attrs;
        body.return_attrs = return_attrs;
        body.parameter_attrs = parameter_attrs;
    }
    let mut builder = module.builder(wrapper);
    let entry = builder.block("body");
    builder.position(entry);
    if plan == Plan::Invalid {
        builder.unreachable();
        return Ok(Some(wrapper));
    }
    let mut arguments: Vec<Operand> = Vec::new();
    for (at, &expected) in given.parameters.iter().enumerate().take(wanted.parameters.len()) {
        let argument = builder.parameter(at);
        arguments.push(cast(&mut builder, argument, wanted.parameters[at], expected));
    }
    arguments.extend(missing);
    if given.variadic {
        arguments.extend((given.parameters.len()..wanted.parameters.len()).map(|at| builder.parameter(at)));
    }
    let result = builder.call(function_type, target, &arguments, "");
    let void = builder.context.types.void();
    let returned = if wanted.returns == void {
        None
    } else if given.returns == void {
        Some(dummy)
    } else {
        let result = result.expect("a call that returns a value");
        Some(cast(&mut builder, result, given.returns, wanted.returns))
    };
    builder.ret(returned);
    Ok(Some(wrapper))
}
