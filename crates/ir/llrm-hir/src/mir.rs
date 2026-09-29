//! HIR to MIR, as clang's CodeGen emits LLVM IR: data objects become
//! globals, local places allocas, HIR values SSA values. What MIR cannot
//! hold yet is refused whole with the reason: a function is left declared,
//! a data object an external global, so what refers to them still verifies.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use llrm_mir::build::Builder;
use llrm_mir::datalayout::{DataLayout, float_bits};
use llrm_mir::{
    Attribute, BinaryOp, BlockId, CastOp, Constant, ConstantExpr, ConstantId, ConstantKind, FloatKind, FloatPredicate, Flags, GlobalId, GlobalVariable, IntPredicate,
    Function, Linkage, MetadataId, MetadataNode, MetadataOperand, Module, Opcode, Operand as Value, Position, Type, TypeId, Types,
};

use crate::model::{self, AddressKind, Number, Op, Operand, Storage, TerminatorKind, TypeKind};
use crate::onerror::{self, Handled};

mod handling;

/// The layout BC's objects fix: 16-bit near pointers, 32-bit far ones
/// indexing by 16 bits, 16-bit segments, and 16-bit alignment.
pub const DATALAYOUT: &str = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32";

/// A far pointer's address space.
pub const FAR: u32 = 1;
/// A segment's: a cast from a far pointer gives its segment, and one back
/// gives segment:0.
pub const SEGMENT: u32 = 2;

/// The prefix of a runtime routine's name: a callee the module does not
/// declare.
pub const RUNTIME: &str = "llrm.qb.";

/// One HIR module's MIR, and what it could not hold.
pub struct Emitted {
    pub module: Module,
    /// Each refused global's name, and why.
    pub refused: Vec<(String, String)>,
    /// Each HIR data object's global, by the object's id.
    pub data: HashMap<i64, GlobalId>,
}

pub fn emit(program: &model::Program) -> Vec<Emitted> {
    // A procedure that frames itself zeroes locals with its own stores; its
    // frame holds garbage.
    let zeroed = program.zeroed_locals && program.frames == model::Frames::Runtime;
    program.modules.iter().map(|one| emit_module(one, program.array_order, zeroed, &program.promises.nounwind, program.runtime)).collect()
}

/// The runtime `emitted` links against, as `promises` states it: the
/// named cells are the external data each HIR module marks unaddressed.
pub fn runtime(emitted: &[(&Emitted, &model::Module)], promises: &model::RuntimePromises) -> Emit<Module> {
    let named = emitted.iter().map(|(emitted, hir)| {
        let cells = hir.data.iter().filter(|one| one.linkage == model::DataLinkage::External && !one.addressed);
        (&emitted.module, cells.map(|one| (one.name.as_str(), emitted.data[&one.id])).collect())
    });
    promised(&named.collect::<Vec<_>>(), promises)
}

/// The declarations-only runtime module `modules` link against, with
/// `promises` of each one's named cells, those only a reference naming
/// them reaches: `!llrm.named` lists them, and each routine a module
/// declares that runs no program code is `nocallback` with an
/// `!llrm.writes` node of the named cells it writes; one that raises no
/// error is `nounwind`.
pub fn promised(modules: &[(&Module, HashMap<&str, GlobalId>)], promises: &model::RuntimePromises) -> Emit<Module> {
    let mut out = Module { datalayout: modules.first().and_then(|(module, _)| module.datalayout.clone()), ..Module::default() };
    let node = |out: &mut Module, globals: Vec<GlobalId>| {
        let operands = globals.into_iter().map(|one| MetadataOperand::Constant(out.reference(one))).collect();
        out.metadata.push(MetadataNode { distinct: false, operands });
        MetadataId(out.metadata.len() as u32 - 1)
    };
    let mut declared = HashMap::new();
    for (module, named) in modules {
        let mut cells = named.iter().map(|(&name, &cell)| (cell, name)).collect::<Vec<_>>();
        cells.sort();
        for (cell, name) in cells {
            let Some(global) = module.global(cell).name.as_deref() else { continue };
            let one = match out.named(global) {
                Some(one) => one,
                None => match out.declared(module, cell)? {
                    Some(one) => one,
                    None => continue,
                },
            };
            declared.insert(name, one);
        }
    }
    if !declared.is_empty() {
        let mut listed = declared.values().copied().collect::<Vec<_>>();
        listed.sort();
        let listed = node(&mut out, listed);
        out.named_metadata.push(("llrm.named".to_owned(), vec![listed]));
    }
    let routines = modules.iter().flat_map(|(module, _)| {
        let declared = module.globals.iter().enumerate().filter(|(_, one)| one.function().is_some_and(llrm_mir::Function::is_declaration));
        declared.filter_map(move |(at, one)| Some((*module, GlobalId(at as u32), one.name.as_deref()?, one.name.as_deref()?.strip_prefix(RUNTIME)?)))
    });
    let mut writes = Vec::new();
    for (module, global, name, routine) in routines {
        let cells = promises.writes(routine);
        let nounwind = promises.nounwind.iter().any(|one| one == routine);
        if cells.is_none() && !nounwind || out.named(name).is_some() {
            continue;
        }
        let Some(one) = out.declared(module, global)? else { continue };
        let llrm_mir::GlobalKind::Function(function) = &mut out.globals[one.0 as usize].kind else { unreachable!("a routine") };
        function.attrs.extend(cells.is_some().then(|| Attribute::Flag("nocallback".to_owned())));
        function.attrs.extend(nounwind.then(|| Attribute::Flag("nounwind".to_owned())));
        if let Some(cells) = cells {
            let written = std::iter::once(one).chain(cells.iter().filter_map(|cell| declared.get(cell.as_str()).copied())).collect();
            writes.push(node(&mut out, written));
        }
    }
    if !writes.is_empty() {
        out.named_metadata.push(("llrm.writes".to_owned(), writes));
    }
    for (module, global) in modules.iter().flat_map(|(module, _)| module.functions().filter(|(_, _, one)| one.is_declaration()).map(move |(id, _, _)| (*module, id))) {
        let Some(name) = module.global(global).name.as_deref() else { continue };
        if !promises.reads_arguments.iter().any(|one| one == name.strip_prefix(RUNTIME).unwrap_or(name)) {
            continue;
        }
        let Some(one) = (match out.named(name) {
            Some(one) => Some(one),
            None => out.declared(module, global)?,
        }) else {
            continue;
        };
        let llrm_mir::GlobalKind::Function(function) = &mut out.globals[one.0 as usize].kind else { unreachable!("a routine") };
        function.attrs.push(Attribute::Memory(vec![(Some("argmem".to_owned()), "read".to_owned())]));
        for at in 0..function.parameters().len() {
            if matches!(out.context.types.get(function.value(function.parameters()[at]).ty), Type::Pointer(_)) {
                function.parameter_attrs[at].push(Attribute::Flag("nocapture".to_owned()));
            }
        }
    }
    Ok(out)
}

type Emit<T> = Result<T, String>;

/// A HIR type's MIR type as a value.
fn value_type(types: &mut Types, hir: &model::Type) -> Emit<TypeId> {
    let bits = u32::try_from(hir.width * 8).map_err(|_| format!("type {} is {} bytes wide", hir.name, hir.width))?;
    Ok(match hir.kind {
        TypeKind::Void => types.void(),
        TypeKind::Boolean | TypeKind::Integer => types.int(bits),
        TypeKind::Float => match hir.width {
            4 => types.intern(Type::Float(FloatKind::Float)),
            8 => types.intern(Type::Float(FloatKind::Double)),
            10 => types.intern(Type::Float(FloatKind::X86Fp80)),
            _ => return Err(format!("a {}-byte float", hir.width)),
        },
        TypeKind::Pointer if hir.address == AddressKind::Segment => types.ptr(SEGMENT),
        TypeKind::Pointer => types.ptr(if hir.width == 4 { FAR } else { 0 }),
        TypeKind::Array | TypeKind::Opaque => return Err(format!("a value of type {}", hir.name)),
    })
}

/// A HIR type's MIR type in memory: bytes where it is no value.
fn stored_type(types: &mut Types, hir: &model::Type) -> Emit<TypeId> {
    match hir.kind {
        TypeKind::Array | TypeKind::Opaque => {
            let byte = types.int(8);
            Ok(types.intern(Type::Array { element: byte, count: hir.width as u64 }))
        }
        _ => value_type(types, hir),
    }
}

/// The `!tbaa` access tags that mark a frontend's promise that a far
/// allocation is disjoint from every place: two siblings under one root.
pub struct Tags {
    pub place: MetadataId,
    pub allocation: MetadataId,
}

impl Tags {
    pub fn new(module: &mut Module) -> Self {
        let zero = module.context.types.int(64);
        let zero = MetadataOperand::Constant(module.context.int(zero, 0));
        let mut node = |operands: Vec<MetadataOperand>| {
            module.metadata.push(MetadataNode { distinct: false, operands });
            MetadataId(module.metadata.len() as u32 - 1)
        };
        let root = node(vec![MetadataOperand::String("llrm hir".to_owned())]);
        let mut tag = |name: &str| {
            let ty = node(vec![MetadataOperand::String(name.to_owned()), MetadataOperand::Node(root), zero.clone()]);
            node(vec![MetadataOperand::Node(ty), MetadataOperand::Node(ty), zero.clone()])
        };
        Tags { place: tag("place"), allocation: tag("allocation") }
    }
}

/// The `!tbaa` access tag of each type the language's aliasing classes
/// cover, by HIR type id: a type node per class under its parent's, as
/// clang builds C's.
fn class_tags(module: &mut Module, classes: &[model::AliasClass]) -> Emit<HashMap<i64, MetadataId>> {
    let zero = module.context.types.int(64);
    let zero = MetadataOperand::Constant(module.context.int(zero, 0));
    let mut nodes: HashMap<&str, MetadataId> = HashMap::new();
    let mut tags = HashMap::new();
    for class in classes {
        let mut operands = vec![MetadataOperand::String(class.name.clone())];
        if let Some(parent) = &class.parent {
            let parent = *nodes.get(parent.as_str()).ok_or_else(|| format!("alias class {} before its parent {parent}", class.name))?;
            operands.extend([MetadataOperand::Node(parent), zero.clone()]);
        }
        module.metadata.push(MetadataNode { distinct: false, operands });
        let node = MetadataId(module.metadata.len() as u32 - 1);
        nodes.insert(&class.name, node);
        if class.parent.is_some() {
            module.metadata.push(MetadataNode { distinct: false, operands: vec![MetadataOperand::Node(node), MetadataOperand::Node(node), zero.clone()] });
            let tag = MetadataId(module.metadata.len() as u32 - 1);
            tags.extend(class.types.iter().map(|&one| (one, tag)));
        }
    }
    Ok(tags)
}

struct Tables<'h> {
    array_order: model::ArrayOrder,
    /// The module body's ON ERROR GOTO handlers, which every procedure's pad calls.
    module_handler: Option<handling::ModuleHandler>,
    tags: Tags,
    /// The aliasing class tag of an access as a type, by its id.
    classes: HashMap<i64, MetadataId>,
    /// Whether a frame starts zeroed.
    zeroed: bool,
    layout: DataLayout,
    types: HashMap<i64, &'h model::Type>,
    callables: HashMap<&'h str, &'h model::Callable>,
    /// Each data object's global, by its id: a place's symbol.
    data: HashMap<i64, ConstantId>,
    /// Each callee's function and its declared type, by HIR name.
    callees: HashMap<String, ConstantId>,
    /// Each callee's calling convention, which its calls repeat.
    conventions: HashMap<String, u32>,
    /// The runtime routines that raise no error.
    nounwind: &'h [String],
}

fn emit_module<'h>(hir: &'h model::Module, array_order: model::ArrayOrder, zeroed: bool, nounwind: &'h [String], runtime: model::RuntimeProfile) -> Emitted {
    let mut module = Module { datalayout: Some(DATALAYOUT.to_owned()), ..Module::default() };
    let mut refused = Vec::new();
    let mut tables = Tables {
        array_order,
        module_handler: None,
        zeroed,
        layout: DataLayout::parse(DATALAYOUT).expect("llrm's layout"),
        types: hir.types.iter().map(|one| (one.id, one)).collect(),
        callables: hir.callables.iter().map(|one| (one.name.as_str(), one)).collect(),
        data: HashMap::new(),
        callees: HashMap::new(),
        conventions: HashMap::new(),
        tags: Tags::new(&mut module),
        classes: HashMap::new(),
        nounwind,
    };
    match class_tags(&mut module, &hir.alias_classes) {
        Ok(classes) => tables.classes = classes,
        Err(why) => refused.push((hir.name.clone(), why)),
    }
    let objects: HashMap<i64, &model::DataObject> = hir.data.iter().map(|one| (one.id, one)).collect();
    let sizes = sizes(hir, &tables.types);
    let mut defined = Vec::new();
    let mut data = HashMap::new();
    for object in &hir.data {
        let layout = data_type(&mut module.context.types, object, sizes[&object.id], &objects);
        let global = declare_data(&mut module, object, layout.as_ref().ok().copied());
        data.insert(object.id, global);
        match layout {
            // Another module's object, however much of it this module reads: declared only.
            Ok(_) if object.linkage == model::DataLinkage::External => {}
            Ok(_) => defined.push((object, global)),
            Err(why) => refused.push((object.name.clone(), why)),
        }
        let reference = module.reference(global);
        tables.data.insert(object.id, reference);
    }
    let mut functions = Vec::new();
    for function in &hir.functions {
        match declare(&mut module, &tables, function) {
            Ok((global, convention)) => {
                let reference = module.reference(global);
                tables.callees.insert(function.name.clone(), reference);
                tables.conventions.insert(function.name.clone(), convention);
                functions.push((function, Some(global)));
            }
            Err(why) => {
                refused.push((function.name.clone(), why));
                functions.push((function, None));
            }
        }
    }
    for (function, global) in &mut functions {
        if let Err(why) = declare_outside(&mut module, &mut tables, function) {
            // What it calls is undeclared: it stays a declaration, external as LLVM's `deleteBody` leaves one.
            if let Some(global) = global.take() {
                module.globals[global.0 as usize].linkage = Linkage::External;
            }
            refused.push((function.name.clone(), why));
        }
    }
    // Initialized once every function its data addresses is declared: one only addressed, far and C's.
    let mut code = HashMap::new();
    for callable in hir.data.iter().flat_map(|one| &one.relocations).filter(|one| one.code).filter_map(|one| hir.callables.iter().find(|callable| callable.id == one.target)) {
        let reference = match tables.callees.get(&callable.name) {
            Some(&reference) => reference,
            None => {
                let returns = module.context.types.void();
                let ty = function_type(&mut module.context.types, returns, Vec::new());
                match module.add_function(&callable.name, ty, Linkage::External) {
                    Ok(global) => {
                        place_function(&mut module, global, (0, FAR));
                        let reference = module.reference(global);
                        tables.callees.insert(callable.name.clone(), reference);
                        tables.conventions.insert(callable.name.clone(), 0);
                        reference
                    }
                    Err(why) => {
                        refused.push((callable.name.clone(), why));
                        continue;
                    }
                }
            }
        };
        code.insert(callable.id, reference);
    }
    for (object, global) in defined {
        if object.relocations.iter().any(|one| one.code && !code.contains_key(&one.target)) {
            refused.push((object.name.clone(), "an address of undeclared code".to_owned()));
            continue;
        }
        let initializer = data_initializer(&mut module, object, sizes[&object.id], &objects, &tables.data, &code);
        let llrm_mir::GlobalKind::Variable(variable) = &mut module.globals[global.0 as usize].kind else { unreachable!("a variable") };
        variable.initializer = Some(initializer);
    }
    let statements = hir.statements();
    let outlined = match module_handler(&mut module, &functions, runtime) {
        Ok(outlined) => outlined,
        Err((name, why)) => {
            refused.push((name, why));
            None
        }
    };
    tables.module_handler = outlined.as_ref().map(|(handler, _, _)| handler.clone());
    for (function, global) in functions {
        let Some(global) = global else { continue };
        let handled = match (function.error_handler.is_some() || tables.module_handler.is_some(), &statements) {
            (false, _) => Ok(None),
            (true, Ok(rows)) => {
                let lines: Vec<i64> = handling::numbered(rows, function.id).iter().map(|one| one.line).collect();
                onerror::handled(&mut module, global, &lines, function.error_handler_local).map(Some)
            }
            (true, Err(why)) => Err(why.clone()),
        };
        let handled = match handled {
            Ok(handled) => handled,
            Err(why) => {
                module.globals[global.0 as usize].linkage = Linkage::External;
                refused.push((function.name.clone(), why));
                continue;
            }
        };
        let mut builder = module.builder(global);
        let rows = statements.as_deref().unwrap_or_default();
        let emitted = Body::new(&mut builder, &tables, function).and_then(|mut body| body.run(handled, rows));
        if emitted.is_err() {
            builder.function.delete_body();
        }
        builder.function.take_changes();
        if let Err(why) = emitted {
            // A declaration is external, as LLVM's `deleteBody` leaves it.
            module.globals[global.0 as usize].linkage = Linkage::External;
            refused.push((function.name.clone(), why));
        }
    }
    if let Some((handler, (global, handled), owner)) = outlined {
        let name = module.global(global).name.clone().unwrap_or_default();
        let mut builder = module.builder(global);
        let emitted = Body::new(&mut builder, &tables, owner).and_then(|mut body| {
            // The handler is its own function; the owner's parameters are not its.
            body.destination = None;
            body.outline(handler, handled)
        });
        if emitted.is_err() {
            builder.function.delete_body();
        }
        builder.function.take_changes();
        if let Err(why) = emitted {
            module.globals[global.0 as usize].linkage = Linkage::External;
            refused.push((name, why));
        }
    }
    Emitted { module, refused, data }
}

/// The module body's ON ERROR GOTO handlers, declared as the function they
/// run as, with what its code refers to, and the body's HIR function.
#[allow(clippy::type_complexity)]
fn module_handler<'h>(module: &mut Module, functions: &[(&'h model::Function, Option<GlobalId>)], runtime: model::RuntimeProfile) -> Result<Option<(handling::ModuleHandler, (GlobalId, Handled), &'h model::Function)>, (String, String)> {
    let Some(&(owner, Some(_))) = functions.iter().find(|(one, _)| one.error_handler.is_some() && !one.error_handler_local) else { return Ok(None) };
    let refusal = |why: String| (owner.name.clone(), why);
    let i16 = module.context.types.int(16);
    let ty = function_type(&mut module.context.types, i16, vec![i16, i16]);
    let global = module.add_function(&format!("{}$handler", owner.name), ty, Linkage::Internal).map_err(refusal)?;
    place_function(module, global, (0, FAR));
    let handled = onerror::handled(module, global, &[], false).map_err(refusal)?;
    let active = Value::Constant(onerror::active_global(module).map_err(refusal)?);
    let last_erl = Value::Constant(onerror::last_erl_global(module).map_err(refusal)?);
    let outlined = (Value::Constant(module.reference(global)), ty);
    let handler = handling::ModuleHandler::of(owner, active, (last_erl, runtime.resume_clears_erl()), outlined).map_err(refusal)?;
    Ok(Some((handler, (global, handled), owner)))
}

/// A data object's type: its bytes, with each relocation a pointer, a
/// segment, or, for a near one into far data, the far address's offset. A
/// far pointer's integer form is segment:offset, so its low word is the
/// offset.
/// Each data object's size: its bytes, or as far as a place over it
/// reaches, since the frontend puts each place inside its object. A
/// runtime's variable has no bytes here.
fn sizes(hir: &model::Module, types: &HashMap<i64, &model::Type>) -> HashMap<i64, i64> {
    let mut sizes: HashMap<i64, i64> = hir.data.iter().map(|one| (one.id, one.bytes.len() as i64)).collect();
    let places = hir.functions.iter().flat_map(|function| &function.places).filter(|place| !matches!(place.storage, Storage::Local | Storage::Parameter));
    for place in places {
        if let Some(size) = sizes.get_mut(&place.symbol) {
            *size = (*size).max(place.offset + place.extent.unwrap_or(types[&place.r#type].width));
        }
    }
    sizes
}

fn data_type(types: &mut Types, object: &model::DataObject, size: i64, objects: &HashMap<i64, &model::DataObject>) -> Emit<TypeId> {
    let byte = types.int(8);
    let mut fields = Vec::new();
    let mut at = 0;
    for relocation in relocations(object)? {
        if relocation.at > at {
            fields.push(types.intern(Type::Array { element: byte, count: (relocation.at - at) as u64 }));
        }
        // Code is never addressed from DS: a near address of it is its offset.
        let space = if relocation.code {
            AddressKind::Far
        } else {
            objects.get(&relocation.target).ok_or_else(|| format!("a relocation to object {}", relocation.target))?.address
        };
        fields.push(match (relocation.address, space) {
            (AddressKind::Near, AddressKind::Far) => types.int(16),
            (AddressKind::Near, _) => types.ptr(0),
            (AddressKind::Far, _) => types.ptr(FAR),
            (AddressKind::Segment, _) => types.ptr(SEGMENT),
            (other, _) => return Err(format!("a {other} relocation in its data")),
        });
        at = relocation.at + relocation_width(relocation.address);
    }
    if fields.is_empty() || size > at {
        fields.push(types.intern(Type::Array { element: byte, count: (size - at) as u64 }));
    }
    Ok(if fields.len() == 1 { fields[0] } else { types.intern(Type::Struct { fields, packed: true }) })
}

fn relocation_width(address: AddressKind) -> i64 {
    if address == AddressKind::Far { 4 } else { 2 }
}

/// A data object's relocations in order, each over zero bytes: its addend
/// is the whole offset.
fn relocations(object: &model::DataObject) -> Emit<Vec<&model::DataRelocation>> {
    let mut out: Vec<&model::DataRelocation> = object.relocations.iter().collect();
    out.sort_by_key(|one| one.at);
    let mut end = 0;
    for relocation in &out {
        let width = relocation_width(relocation.address);
        let site = object.bytes.get(relocation.at as usize..(relocation.at + width) as usize).ok_or("a relocation past its data")?;
        if relocation.at < end || site.iter().any(|&one| one != 0) {
            return Err("overlapping or pre-added relocations".to_owned());
        }
        end = relocation.at + width;
    }
    Ok(out)
}

/// A data object's global, its initializer set once every global exists;
/// an external `[n x i8]` when its type is refused.
fn declare_data(module: &mut Module, object: &model::DataObject, ty: Option<TypeId>) -> GlobalId {
    let byte = module.context.types.int(8);
    let bytes = module.context.types.intern(Type::Array { element: byte, count: object.bytes.len() as u64 });
    let linkage = match (ty, object.linkage) {
        (None, _) | (Some(_), model::DataLinkage::External | model::DataLinkage::Exported) => Linkage::External,
        (Some(_), model::DataLinkage::Internal) => Linkage::Internal,
        (Some(_), model::DataLinkage::Private) => Linkage::Private,
    };
    let align = object.align.map(|one| one as u64);
    let variable = GlobalVariable { ty: ty.unwrap_or(bytes), constant: object.readonly && ty.is_some(), initializer: None, align };
    let global = add_unique(module, &object.name, |module, name| module.add_variable(name, variable.clone(), linkage));
    module.globals[global.0 as usize].address_space = if object.address == AddressKind::Far { FAR } else { 0 };
    global
}

fn data_initializer(module: &mut Module, object: &model::DataObject, size: i64, objects: &HashMap<i64, &model::DataObject>, data: &HashMap<i64, ConstantId>, code: &HashMap<i64, ConstantId>) -> ConstantId {
    let context = &mut module.context;
    let (byte, i16) = (context.types.int(8), context.types.int(16));
    let bytes = |context: &mut llrm_mir::Context, from: i64, to: i64| {
        let slice: Vec<u8> = (from as usize..to as usize).map(|at| object.bytes.get(at).map_or(0, |&one| one as u8)).collect();
        let ty = context.types.intern(Type::Array { element: byte, count: slice.len() as u64 });
        let kind = if slice.iter().all(|&one| one == 0) { ConstantKind::Zero } else { ConstantKind::Bytes(slice) };
        context.constant(Constant { ty, kind })
    };
    let mut members = Vec::new();
    let mut at = 0;
    for relocation in relocations(object).expect("its type was laid out") {
        if relocation.at > at {
            members.push(bytes(context, at, relocation.at));
        }
        let (target, space) = if relocation.code {
            let target = code[&relocation.target];
            let space = match context.types.get(context.get(target).ty) {
                Type::Pointer(space) => *space,
                _ => FAR,
            };
            (target, space)
        } else {
            (data[&relocation.target], if objects[&relocation.target].address == AddressKind::Far { FAR } else { 0 })
        };
        let mut address = target;
        if relocation.addend != 0 {
            let ty = context.types.ptr(space);
            let index = context.int(i16, i128::from(relocation.addend));
            let operands = vec![target, index];
            address = context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::GetElementPtr { source: byte, inbounds: false, operands }) });
        }
        let cast = |context: &mut llrm_mir::Context, op, ty| context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::Cast { op, value: address }) });
        let wanted = match relocation.address {
            AddressKind::Far => FAR,
            AddressKind::Segment => SEGMENT,
            _ => space,
        };
        members.push(match (relocation.address, space) {
            (AddressKind::Near, FAR) => cast(context, CastOp::PtrToInt, i16),
            (AddressKind::Near, _) if relocation.code => cast(context, CastOp::PtrToInt, i16),
            _ if wanted != space => {
                let ty = context.types.ptr(wanted);
                cast(context, CastOp::AddrSpaceCast, ty)
            }
            _ => address,
        });
        at = relocation.at + relocation_width(relocation.address);
    }
    if members.is_empty() || size > at {
        members.push(bytes(context, at, size));
    }
    if members.len() == 1 {
        return members[0];
    }
    let fields = members.iter().map(|&one| context.get(one).ty).collect();
    let ty = context.types.intern(Type::Struct { fields, packed: true });
    context.constant(Constant { ty, kind: ConstantKind::Aggregate(members) })
}

/// `name`, or `name.N` for the least N free, as LLVM uniques names.
fn add_unique(module: &mut Module, name: &str, mut add: impl FnMut(&mut Module, &str) -> Emit<GlobalId>) -> GlobalId {
    std::iter::once(name.to_owned()).chain((1..).map(|n| format!("{name}.{n}"))).find_map(|one| add(module, &one).ok()).expect("some name is free")
}

fn function_type(types: &mut Types, returns: TypeId, parameters: Vec<TypeId>) -> TypeId {
    types.intern(Type::Function { returns, parameters, variadic: false })
}

/// The calling convention and code address space an ABI gives: BASIC's
/// when the callee pops its arguments, C's when the caller does; a far
/// procedure's code in address space 1, as its pointers are.
fn convention(cleanup: model::StackCleanup, distance: model::CallDistance) -> Emit<(u32, u32)> {
    let convention = match cleanup {
        model::StackCleanup::Callee => llrm_mir::opcode::BASIC,
        model::StackCleanup::Caller => 0,
    };
    let space = match distance {
        model::CallDistance::Near => 0,
        model::CallDistance::Far => FAR,
        // Entered with the flags pushed and left by iret, as LLVM's x86_intrcc.
        model::CallDistance::Interrupt => return Ok((llrm_mir::opcode::X86_INTR, FAR)),
    };
    Ok((convention, space))
}

/// The parameter `function`'s floating result is stored through and
/// returned in its place, where its ABI has one.
fn result_destination(tables: &Tables, function: &model::Function) -> Option<usize> {
    let values: HashMap<i64, i64> = function.values.iter().map(|one| (one.id, one.r#type)).collect();
    let parameters: Vec<&model::Type> = function.parameters.iter().map(|one| tables.types[&values[one]]).collect();
    function.abi.as_ref()?.result_destination(tables.types[&function.result_type], &parameters)
}

/// `global`, now a function of `convention` in code address space `space`.
fn place_function(module: &mut Module, global: GlobalId, (convention, space): (u32, u32)) {
    let one = &mut module.globals[global.0 as usize];
    one.address_space = space;
    let llrm_mir::GlobalKind::Function(function) = &mut one.kind else { unreachable!("a function") };
    function.calling_convention = convention;
}

/// A defined function, far and C's unless its ABI says otherwise.
fn declare(module: &mut Module, tables: &Tables, function: &model::Function) -> Emit<(GlobalId, u32)> {
    let values: HashMap<i64, i64> = function.values.iter().map(|one| (one.id, one.r#type)).collect();
    let types = &mut module.context.types;
    // An interrupt handler is given the registers it saved, as LLVM's x86_intrcc its frame.
    let parameters: Vec<TypeId> = match interrupted(function) {
        true => vec![types.ptr(0)],
        false => function.parameters.iter().map(|one| value_type(types, tables.types[&values[one]])).collect::<Emit<_>>()?,
    };
    if function.abi.as_ref().is_some_and(|abi| abi.float_return == model::FloatReturn::Address) && tables.types[&function.result_type].kind == model::TypeKind::Float {
        return Err(format!("{}: a floating result returned as its address, which only a caller of Microsoft C's is", function.name));
    }
    let returns = match result_destination(tables, function) {
        Some(at) => parameters[at],
        None => value_type(types, tables.types[&function.result_type])?,
    };
    let variadic = function.abi.as_ref().is_some_and(|abi| abi.variadic);
    let ty = types.intern(Type::Function { returns, parameters, variadic });
    let linkage = match function.linkage {
        model::FunctionLinkage::Internal => Linkage::Internal,
        model::FunctionLinkage::External => Linkage::External,
    };
    let abi = match &function.abi {
        Some(abi) => convention(abi.cleanup, abi.distance)?,
        None => (0, FAR),
    };
    let global = module.add_function(&function.name, ty, linkage)?;
    place_function(module, global, abi);
    let llrm_mir::GlobalKind::Function(defined) = &mut module.globals[global.0 as usize].kind else { unreachable!("a function") };
    for promise in &function.promises {
        let at = function.parameters.iter().position(|&one| one == promise.parameter).ok_or("a promise of no parameter")?;
        let attrs = &mut defined.parameter_attrs[at];
        attrs.extend(promise.unaliased.then(|| Attribute::Flag("noalias".to_owned())));
        attrs.extend(promise.readonly.then(|| Attribute::Flag("readonly".to_owned())));
        if promise.bytes > 0 {
            attrs.push(Attribute::Int("dereferenceable".to_owned(), promise.bytes as u64));
        }
    }
    Ok((global, abi.0))
}

/// Local places that overlap, since they share their bytes: one alloca.
struct FrameGroup<'h> {
    start: i64,
    size: i64,
    places: Vec<&'h model::Place>,
    ty: TypeId,
}

fn frame_groups<'h>(types: &mut Types, tables: &Tables<'h>, function: &'h model::Function) -> Emit<Vec<FrameGroup<'h>>> {
    let mut locals: Vec<(i64, i64, &model::Place)> = function
        .places
        .iter()
        .filter(|one| one.storage == Storage::Local)
        .map(|one| (one.offset, one.offset + one.extent.unwrap_or(tables.types[&one.r#type].width), one))
        .collect();
    locals.sort_by_key(|&(start, end, place)| (start, end, place.id));
    let mut spans: Vec<(i64, i64, Vec<&model::Place>)> = Vec::new();
    for (start, end, place) in locals {
        match spans.last_mut() {
            Some(span) if start < span.1 => {
                span.1 = span.1.max(end);
                span.2.push(place);
            }
            _ => spans.push((start, end, vec![place])),
        }
    }
    spans
        .into_iter()
        .map(|(start, end, places)| {
            let ty = match places[..] {
                [place] => stored_type(types, tables.types[&place.r#type])?,
                _ => {
                    let byte = types.int(8);
                    types.intern(Type::Array { element: byte, count: (end - start) as u64 })
                }
            };
            Ok(FrameGroup { start, size: end - start, places, ty })
        })
        .collect()
}

/// The memset a zeroed aggregate local calls.
const MEMSET: &str = "llvm.memset.p0.i16";

fn memset_type(types: &mut Types) -> TypeId {
    let (void, pointer, byte, size, flag) = (types.void(), types.ptr(0), types.int(8), types.int(16), types.int(1));
    function_type(types, void, vec![pointer, byte, size, flag])
}

/// Declares what `function` calls or names outside the module: runtime
/// routines, typed as the first call gives them, and external places.
fn declare_outside(module: &mut Module, tables: &mut Tables, function: &model::Function) -> Emit<()> {
    let values: HashMap<i64, i64> = function.values.iter().map(|one| (one.id, one.r#type)).collect();
    let places: HashMap<i64, &model::Place> = function.places.iter().map(|one| (one.id, one)).collect();
    if function.error_handler.is_some() && !function.error_handler_local {
        // The module handler run to the module's end raises "No RESUME".
        declare_runtime(module, tables, handling::RAISE, 1)?;
    }
    for place in &function.places {
        if !matches!(place.storage, Storage::Local | Storage::Parameter) && !tables.data.contains_key(&place.symbol) {
            let ty = stored_type(&mut module.context.types, tables.types[&place.r#type])?;
            let variable = GlobalVariable { ty, constant: false, initializer: None, align: None };
            let global = add_unique(module, &place.name, |module, name| module.add_variable(name, variable.clone(), Linkage::External));
            let reference = module.reference(global);
            tables.data.insert(place.symbol, reference);
        }
    }
    if tables.zeroed && !tables.callees.contains_key(MEMSET) {
        let groups = frame_groups(&mut module.context.types, tables, function)?;
        if groups.iter().any(|group| !matches!(module.context.types.get(group.ty), Type::Int(_) | Type::Float(_) | Type::Pointer(_))) {
            let ty = memset_type(&mut module.context.types);
            let global = module.add_function(MEMSET, ty, Linkage::External)?;
            let reference = module.reference(global);
            tables.callees.insert(MEMSET.to_owned(), reference);
        }
    }
    if !interrupted(function) && function.places.iter().any(|one| one.storage == Storage::Parameter) && !tables.callees.contains_key(VA_START) {
        let types = &mut module.context.types;
        let (void, pointer) = (types.void(), types.ptr(0));
        let ty = function_type(types, void, vec![pointer]);
        let global = module.add_function(VA_START, ty, Linkage::External)?;
        let reference = module.reference(global);
        tables.callees.insert(VA_START.to_owned(), reference);
    }
    for instruction in function.blocks.iter().flat_map(|one| &one.instructions) {
        if let (Some(operand), Some(result)) = (instruction.operands.first(), instruction.results.first()) {
            let types = &mut module.context.types;
            let from = value_type(types, tables.types[&operand_type(operand, &values, &places)]);
            let to = value_type(types, tables.types[&values[result]]);
            if let (Ok(from), Ok(to)) = (from, to)
                && let Some((name, parameters)) = intrinsic(types, instruction.op, from, to).map(|name| (name, vec![from])).or_else(|| fixed(types, instruction.op, from))
                && let Entry::Vacant(slot) = tables.callees.entry(name)
            {
                let ty = function_type(types, to, parameters);
                let global = module.add_function(slot.key(), ty, Linkage::External)?;
                slot.insert(module.reference(global));
            }
        }
        if let Some(called) = called(instruction.op) {
            let types = &mut module.context.types;
            let hir = |operand| tables.types[&operand_type(operand, &values, &places)];
            let (parameters, returns) = called_type(types, instruction.operands.iter().map(hir).collect(), instruction.results.first().map(|one| tables.types[&values[one]]))?;
            let name = called_name(types, called, parameters.last().copied(), returns);
            if let Entry::Vacant(slot) = tables.callees.entry(name) {
                let ty = function_type(types, returns, parameters);
                let global = module.add_function(slot.key(), ty, Linkage::External)?;
                slot.insert(module.reference(global));
            }
        }
        let Some(callee) = instruction.callee.as_deref().filter(|_| instruction.op == Op::Call || three_way(instruction.op).is_some()) else { continue };
        if handling::resumes(callee) {
            // A RESUME fallen into raises its error as ERROR does.
            declare_runtime(module, tables, handling::RAISE, 1)?;
        }
        if handling::owns(callee) {
            continue;
        }
        let abi = match function.calls.iter().find(|one| one.instruction == instruction.id) {
            Some(site) => {
                let abi = convention(site.cleanup, site.distance)?;
                passed(site, instruction.operands.len()).ok_or_else(|| format!("a call to {callee} pushing {:?}", site.order))?;
                abi
            }
            None => (0, FAR),
        };
        if tables.callees.contains_key(callee) {
            if tables.conventions.get(callee) != Some(&abi.0) {
                return Err(format!("calls to {callee} by two conventions"));
            }
            continue;
        }
        let types = &mut module.context.types;
        let site = function.calls.iter().find(|one| one.instruction == instruction.id);
        let order = site.and_then(|site| passed(site, instruction.operands.len())).unwrap_or_else(|| (0..instruction.operands.len()).collect());
        let parameters: Vec<TypeId> = order.iter().map(|&one| value_type(types, tables.types[&operand_type(&instruction.operands[one], &values, &places)])).collect::<Emit<_>>()?;
        let answer = site.map_or(Answer::Value, |site| answer(site, &instruction.results, |result| tables.types[&values[&result]]));
        let returns = match instruction.results[..] {
            // A comparison's callee returns the sign of the first against the second.
            [_] if three_way(instruction.op).is_some() => types.int(16),
            [_] if answer == Answer::Through => *parameters.last().ok_or("a floating result with no destination")?,
            [_] if answer == Answer::Address => types.ptr(0),
            [result] => value_type(types, tables.types[&values[&result]])?,
            [] => types.void(),
            // Answered in several registers: one aggregate, as LLVM returns them.
            ref results => {
                let fields = results.iter().map(|result| value_type(types, tables.types[&values[result]])).collect::<Emit<_>>()?;
                types.intern(Type::Struct { fields, packed: false })
            }
        };
        let ty = function_type(types, returns, parameters);
        let name = if tables.callables.contains_key(callee) { callee.to_owned() } else { format!("{RUNTIME}{callee}") };
        let global = module.add_function(&name, ty, Linkage::External)?;
        place_function(module, global, abi);
        let reference = module.reference(global);
        tables.callees.insert(callee.to_owned(), reference);
        tables.conventions.insert(callee.to_owned(), abi.0);
    }
    Ok(())
}

/// The runtime routine `name`, far and cleaning up its `words` word
/// arguments, returning nothing, where the module does not declare it yet:
/// one the ON ERROR lowering calls where the HIR does not.
fn declare_runtime(module: &mut Module, tables: &mut Tables, name: &str, words: usize) -> Emit<()> {
    if tables.callees.contains_key(name) {
        return Ok(());
    }
    let abi = convention(model::StackCleanup::Callee, model::CallDistance::Far)?;
    let types = &mut module.context.types;
    let (void, word) = (types.void(), types.int(16));
    let ty = function_type(types, void, vec![word; words]);
    let global = module.add_function(&format!("{RUNTIME}{name}"), ty, Linkage::External)?;
    place_function(module, global, abi);
    tables.callees.insert(name.to_owned(), module.reference(global));
    tables.conventions.insert(name.to_owned(), abi.0);
    Ok(())
}

/// A call's operands in the order its arguments are passed: as `site`
/// pushes them, BASIC's first pushed first and C's last pushed first, so the
/// convention's own order pushes them as the site does. None where `order`
/// is no permutation of the operands.
fn passed(site: &model::CallAbi, count: usize) -> Option<Vec<usize>> {
    let mut order: Vec<usize> = site.order.iter().map(|&one| usize::try_from(one).ok()).collect::<Option<_>>()?;
    let mut sorted = order.clone();
    sorted.sort_unstable();
    if sorted != (0..count).collect::<Vec<_>>() {
        return None;
    }
    if site.cleanup == model::StackCleanup::Caller {
        order.reverse();
    }
    Some(order)
}

/// The intrinsic a HIR instruction from a `from` to a `to` calls, if any:
/// a float function, or a float converted to an integer, which rounds as
/// the machine's default mode does, to nearest, ties to even.
fn intrinsic(types: &Types, op: Op, from: TypeId, to: TypeId) -> Option<String> {
    let float = match types.get(from) {
        Type::Float(FloatKind::Float) => "f32",
        Type::Float(FloatKind::Double) => "f64",
        _ => return None,
    };
    let function = match op {
        Op::Convert => return types.int_bits(to).map(|bits| format!("llvm.lrint.i{bits}.{float}")),
        Op::Fabs => "fabs",
        Op::Fsqrt => "sqrt",
        Op::Fsin => "sin",
        Op::Fcos => "cos",
        Op::Fatan => "atan",
        Op::Flog2 => "log2",
        Op::Fexp2 => "exp2",
        Op::Fround => "rint",
        _ => return None,
    };
    Some(format!("llvm.{function}.{float}"))
}

/// The fixed-point intrinsic a HIR instruction on `ty` calls, and its
/// parameters: the two operands and the fraction's bits.
fn fixed(types: &mut Types, op: Op, ty: TypeId) -> Option<(String, Vec<TypeId>)> {
    let name = match op {
        Op::FixedMul => "llvm.smul.fix",
        Op::FixedDiv => "llvm.sdiv.fix",
        _ => return None,
    };
    let bits = types.int_bits(ty)?;
    Some((format!("{name}.i{bits}"), vec![ty, ty, types.int(32)]))
}

/// How a call's result comes back: as the value; as the address of the
/// destination the call passes last; or as the address of the callee's copy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Value,
    Through,
    Address,
}

/// How `site`'s one floating result, typed as `hir_type` says, comes back.
fn answer<'t>(site: &model::CallAbi, results: &[i64], hir_type: impl Fn(i64) -> &'t model::Type) -> Answer {
    match results {
        [result] if site.returns_through(hir_type(*result)) => Answer::Through,
        [result] if site.returns_address(hir_type(*result)) => Answer::Address,
        _ => Answer::Value,
    }
}

/// Whether an interrupt enters `function`.
fn interrupted(function: &model::Function) -> bool {
    function.abi.as_ref().is_some_and(|abi| abi.distance == model::CallDistance::Interrupt)
}

/// Where a variadic function's variadic arguments start, stored in a list.
const VA_START: &str = "llvm.va_start.p0";

/// The target intrinsic a HIR instruction is a call of, its operands the
/// arguments: an I/O port's.
fn called(op: Op) -> Option<&'static str> {
    const CALLED: [(Op, &str); 2] = [(Op::PortIn, "llrm.ia16.in"), (Op::PortOut, "llrm.ia16.out")];
    CALLED.iter().find(|&&(one, _)| one == op).map(|&(_, name)| name)
}

/// The parameters and result of a `called` intrinsic: a port is a word,
/// the data as HIR types it.
fn called_type(types: &mut Types, operands: Vec<&model::Type>, result: Option<&model::Type>) -> Emit<(Vec<TypeId>, TypeId)> {
    let mut parameters = operands.into_iter().map(|one| value_type(types, one)).collect::<Emit<Vec<_>>>()?;
    parameters[0] = types.int(16);
    let returns = match result {
        Some(one) => value_type(types, one)?,
        None => types.void(),
    };
    Ok((parameters, returns))
}

/// `called` mangled by its data's type: what it returns, or its last argument.
fn called_name(types: &Types, called: &str, last: Option<TypeId>, returns: TypeId) -> String {
    let data = if types.is_void(returns) { last.expect("a written value") } else { returns };
    format!("{called}.i{}", types.int_bits(data).unwrap_or(0))
}

/// The comparison a string comparison makes of its callee's sign with zero.
fn three_way(op: Op) -> Option<Op> {
    const THREE_WAY: [(Op, Op); 6] =
        [(Op::StringEq, Op::Eq), (Op::StringNe, Op::Ne), (Op::StringLt, Op::Lt), (Op::StringLe, Op::Le), (Op::StringGt, Op::Gt), (Op::StringGe, Op::Ge)];
    THREE_WAY.iter().find(|&&(one, _)| one == op).map(|&(_, compared)| compared)
}

/// An operand's HIR type: a place's is what it holds.
fn operand_type(operand: &Operand, values: &HashMap<i64, i64>, places: &HashMap<i64, &model::Place>) -> i64 {
    match operand {
        Operand::ValueRef(one) => values[&one.value],
        Operand::Constant(one) => one.r#type,
        Operand::PlaceRef(one) => places[&one.place].r#type,
        Operand::ArrayElement(one) => places[&one.place].r#type,
        Operand::ProjectedPlace(one) => one.r#type,
        Operand::IndirectPlace(one) => one.r#type,
        Operand::DescriptorPlace(one) => one.r#type,
    }
}

/// `function`'s blocks, each after its dominators -- reverse postorder, the
/// unreachable ones last -- so a value is emitted before its uses whatever
/// the HIR's block order.
fn emission_order(function: &model::Function) -> Vec<&model::Block> {
    let blocks: HashMap<i64, &model::Block> = function.blocks.iter().map(|block| (block.id, block)).collect();
    let successors = |block: &model::Block| {
        let terminator = &block.terminator;
        terminator.targets.iter().chain(terminator.cases.iter().map(|(_, target)| target)).copied().collect::<Vec<_>>()
    };
    let mut seen = std::collections::HashSet::from([function.entry]);
    let mut postorder = Vec::new();
    let mut stack = vec![(blocks[&function.entry], successors(blocks[&function.entry]).into_iter())];
    while let Some((block, next)) = stack.last_mut() {
        match next.find(|target| seen.insert(*target)) {
            Some(target) => {
                let target = blocks[&target];
                stack.push((target, successors(target).into_iter()));
            }
            None => {
                postorder.push(*block);
                stack.pop();
            }
        }
    }
    let unreachable = function.blocks.iter().filter(|block| !seen.contains(&block.id));
    postorder.into_iter().rev().chain(unreachable).collect()
}

/// Marks each call in `block` `cold`, as LLVM marks a call site on a path
/// the frontend expects never to run.
fn mark_cold(function: &mut Function, block: BlockId) {
    for inst in function.block(block).instructions().to_vec() {
        let old = function.instruction(inst).clone();
        let Opcode::Call(mut info) = old.opcode else { continue };
        info.attrs.push(Attribute::Flag("cold".to_owned()));
        let new = function.create_instruction(Opcode::Call(info), old.ty, old.operands, old.flags, None);
        function.insert(new, Position::Before(inst)).expect("its call is placed");
        for (kind, node) in old.metadata {
            function.annotate(new, &kind, node);
        }
        if let (Some(from), Some(to)) = (old.result, function.instruction(new).result) {
            function.replace_all_uses_with(from, Value::Value(to));
        }
        function.erase(inst).expect("its uses moved to the cold call");
    }
}

struct Body<'b, 'm, 'h> {
    b: &'b mut Builder<'m>,
    tables: &'b Tables<'h>,
    function: &'h model::Function,
    value_types: HashMap<i64, i64>,
    places: HashMap<i64, &'h model::Place>,
    blocks: HashMap<i64, BlockId>,
    values: HashMap<i64, Value>,
    /// Each local place's frame object, and its offset in it.
    frame: HashMap<i64, (usize, i64)>,
    /// Where a variadic function's variadic arguments start, if a parameter lives with them.
    passed: Option<Value>,
    objects: Vec<Value>,
    /// Each value that is a place's address, and that place's size.
    addresses: HashMap<i64, i64>,
    handling: Option<handling::Handling>,
    /// The module handler, where this emits its own function.
    outlined: Option<handling::Outlined>,
    /// Where a floating result is stored, returned in its place.
    destination: Option<Value>,
}

impl<'b, 'm, 'h> Body<'b, 'm, 'h> {
    fn new(b: &'b mut Builder<'m>, tables: &'b Tables<'h>, function: &'h model::Function) -> Emit<Self> {
        let values = match interrupted(function) {
            // Its parameters are only ever its saved registers' memory.
            true => HashMap::new(),
            false => function.parameters.iter().enumerate().map(|(at, &one)| (one, b.parameter(at))).collect(),
        };
        let destination = result_destination(tables, function).map(|at| b.parameter(at));
        Ok(Self {
            b,
            tables,
            function,
            value_types: function.values.iter().map(|one| (one.id, one.r#type)).collect(),
            places: function.places.iter().map(|one| (one.id, one)).collect(),
            blocks: HashMap::new(),
            values,
            frame: HashMap::new(),
            passed: None,
            objects: Vec::new(),
            addresses: HashMap::new(),
            handling: None,
            outlined: None,
            destination,
        })
    }

    fn run(&mut self, handled: Option<Handled>, statements: &[model::Statement]) -> Emit<()> {
        // The runtime enters a handled function's statements where RESUME
        // continues; any other entry is not selected.
        if handled.is_none() && self.function.external_entries.iter().any(|&one| one != self.function.entry) {
            return Err("an alternate entry".to_owned());
        }
        let elsewhere = self.elsewhere();
        // RESUME may continue at the first statement: an entry of its own
        // keeps the frame's allocas where it cannot.
        let frame = handled.is_some().then(|| self.b.block("frame"));
        let entry = self.function.blocks.iter().find(|one| one.id == self.function.entry).ok_or("no entry block")?;
        let order = std::iter::once(entry).chain(self.function.blocks.iter().filter(|one| one.id != self.function.entry && !elsewhere.contains(&one.id)));
        for block in order {
            let id = self.b.block(&format!("b{}", block.id));
            self.blocks.insert(block.id, id);
        }
        self.b.position(frame.unwrap_or(self.blocks[&entry.id]));
        self.allocate()?;
        if let Some(handled) = handled {
            self.handle(handled, statements)?;
            self.b.br(self.blocks[&entry.id]);
        }
        for block in emission_order(self.function).into_iter().filter(|one| !elsewhere.contains(&one.id)) {
            self.emit_block(block)?;
        }
        self.close_handling()
    }

    fn emit_block(&mut self, block: &model::Block) -> Emit<()> {
        self.b.position(self.blocks[&block.id]);
        self.enter_block(block.id);
        for instruction in &block.instructions {
            self.instruction(instruction)?;
        }
        // RESUME label ended the block itself.
        let current = self.b.current().expect("a placed block");
        if self.b.function.terminator(current).is_none() {
            self.terminator(&block.terminator)?;
        }
        if block.cold {
            mark_cold(self.b.function, self.blocks[&block.id]);
        }
        Ok(())
    }

    /// An alloca for each group of local places that overlap, each zeroed
    /// as HIR's frame starts: a scalar by a store, an aggregate by memset,
    /// as clang zeroes one.
    fn allocate(&mut self) -> Emit<()> {
        let groups = frame_groups(&mut self.b.context.types, self.tables, self.function)?;
        for group in &groups {
            for place in &group.places {
                self.frame.insert(place.id, (self.objects.len(), place.offset - group.start));
            }
            self.objects.push(self.b.alloca(group.ty, ""));
        }
        if interrupted(self.function) {
            self.passed = Some(self.b.parameter(0));
        } else if self.function.places.iter().any(|one| one.storage == Storage::Parameter) {
            let pointer = self.b.context.types.ptr(0);
            let list = self.b.alloca(pointer, "");
            let (void, callee) = (self.b.context.types.void(), Value::Constant(self.tables.callees[VA_START]));
            let ty = function_type(&mut self.b.context.types, void, vec![pointer]);
            self.b.call(ty, callee, &[list], "");
            self.passed = Some(self.b.load(pointer, list, false, ""));
        }
        if !self.tables.zeroed {
            return Ok(());
        }
        for (group, object) in groups.iter().zip(self.objects.clone()) {
            let kind = match self.b.context.types.get(group.ty) {
                Type::Int(_) => ConstantKind::Int(0),
                Type::Float(_) => ConstantKind::Float(0),
                Type::Pointer(_) => ConstantKind::Null,
                _ => {
                    let callee = Value::Constant(self.tables.callees[MEMSET]);
                    let ty = memset_type(&mut self.b.context.types);
                    let (zero, size, volatile) = (self.b.int(8, 0), self.b.int(16, i128::from(group.size)), self.b.int(1, 0));
                    self.b.call(ty, callee, &[object, zero, size, volatile], "");
                    continue;
                }
            };
            let zero = Value::Constant(self.b.context.constant(Constant { ty: group.ty, kind }));
            self.b.store(zero, object, false);
        }
        Ok(())
    }

    fn hir_type(&self, id: i64) -> &'h model::Type {
        self.tables.types[&id]
    }

    fn ty(&mut self, id: i64) -> Emit<TypeId> {
        value_type(&mut self.b.context.types, self.tables.types[&id])
    }

    fn result_type(&mut self, result: i64) -> Emit<TypeId> {
        self.ty(self.value_types[&result])
    }

    fn operand_hir_type(&self, operand: &Operand) -> &'h model::Type {
        self.hir_type(operand_type(operand, &self.value_types, &self.places))
    }

    fn block(&self, id: i64) -> BlockId {
        self.blocks[&id]
    }

    /// An operand's value; a place's is loaded.
    fn value(&mut self, operand: &Operand) -> Emit<Value> {
        match operand {
            Operand::ValueRef(one) => self.values.get(&one.value).copied().ok_or_else(|| format!("value {} used before its definition", one.value)),
            Operand::Constant(one) => self.constant(one),
            place => {
                let (pointer, ty, volatile, tag) = self.place(place)?;
                let loaded = self.b.load(ty, pointer, volatile, "");
                self.tagged(tag);
                Ok(loaded)
            }
        }
    }

    fn constant(&mut self, constant: &model::Constant) -> Emit<Value> {
        let ty = self.ty(constant.r#type)?;
        let kind = match (self.b.context.types.get(ty), constant.value) {
            (Type::Int(_), Number::Int(n)) => ConstantKind::Int(n as u128),
            (Type::Float(FloatKind::Float), Number::Int(n)) => ConstantKind::Float(u64::from((n as f32).to_bits())),
            (Type::Float(FloatKind::Float), Number::Float(x)) => ConstantKind::Float(u64::from((x as f32).to_bits())),
            (Type::Float(FloatKind::Double | FloatKind::X86Fp80), Number::Int(n)) => ConstantKind::Float((n as f64).to_bits()),
            (Type::Float(FloatKind::Double | FloatKind::X86Fp80), Number::Float(x)) => ConstantKind::Float(x.to_bits()),
            (Type::Pointer(_), Number::Int(0)) => ConstantKind::Null,
            (other, value) => return Err(format!("a constant {value:?} of {other:?}")),
        };
        let kind = match kind {
            ConstantKind::Int(bits) => ConstantKind::Int(bits & llrm_mir::context::mask(self.b.context.types.int_bits(ty).expect("an integer"))),
            other => other,
        };
        Ok(Value::Constant(self.b.context.constant(Constant { ty, kind })))
    }

    /// A place's address, what it holds, whether it is volatile, and the
    /// `!tbaa` tag its accesses carry.
    fn place(&mut self, operand: &Operand) -> Emit<(Value, TypeId, bool, Option<MetadataId>)> {
        let accessed = match operand {
            Operand::PlaceRef(one) => Some(self.places[&one.place].r#type),
            Operand::ArrayElement(one) => self.tables.types[&self.places[&one.place].r#type].element,
            Operand::ProjectedPlace(model::ProjectedPlace { r#type, .. }) | Operand::IndirectPlace(model::IndirectPlace { r#type, .. }) | Operand::DescriptorPlace(model::DescriptorPlace { r#type, .. }) => Some(*r#type),
            Operand::ValueRef(_) | Operand::Constant(_) => None,
        };
        let (pointer, ty, volatile, tag) = self.untyped_place(operand)?;
        // The language's aliasing class says more than where the place is.
        let class = accessed.and_then(|one| self.tables.classes.get(&one).copied());
        Ok((pointer, ty, volatile, class.or(tag)))
    }

    fn untyped_place(&mut self, operand: &Operand) -> Emit<(Value, TypeId, bool, Option<MetadataId>)> {
        match operand {
            Operand::PlaceRef(one) => {
                let place = self.places[&one.place];
                let ty = stored_type(&mut self.b.context.types, self.tables.types[&place.r#type])?;
                Ok((self.base(place)?, ty, place.volatile, Some(self.tables.tags.place)))
            }
            Operand::ArrayElement(one) => {
                let place = self.places[&one.place];
                let element = self.tables.types[&place.r#type].element.ok_or("an array element of a non-array")?;
                let (pointer, ty) = self.element(place, &one.indices, 0, element)?;
                Ok((pointer, ty, place.volatile, Some(self.tables.tags.place)))
            }
            Operand::ProjectedPlace(one) => {
                let place = self.places[&one.place];
                let (pointer, ty) = self.element(place, &one.indices, one.offset, one.r#type)?;
                Ok((pointer, ty, place.volatile, Some(self.tables.tags.place)))
            }
            Operand::IndirectPlace(one) => {
                let base = self.values.get(&one.base).copied().ok_or_else(|| format!("value {} used before its definition", one.base))?;
                let ty = stored_type(&mut self.b.context.types, self.tables.types[&one.r#type])?;
                let inside = self.addresses.get(&one.base).is_some_and(|&size| one.offset >= 0 && one.offset + self.tables.types[&one.r#type].width <= size);
                let tag = match one.allocation {
                    Some(_) => Some(self.tables.tags.allocation),
                    None => inside.then_some(self.tables.tags.place),
                };
                Ok((self.offset(base, one.offset, one.inbounds), ty, one.volatile, tag))
            }
            Operand::DescriptorPlace(one) => {
                let base = self.values.get(&one.base).copied().ok_or_else(|| format!("value {} used before its definition", one.base))?;
                let pointee = self.tables.types[&self.value_types[&one.base]].element.map(|element| self.tables.types[&element]);
                let ty = stored_type(&mut self.b.context.types, self.tables.types[&one.r#type])?;
                Ok((self.offset(base, one.offset(pointee), false), ty, false, None))
            }
            Operand::ValueRef(_) | Operand::Constant(_) => Err("a value where a place belongs".to_owned()),
        }
    }

    /// The access just emitted, tagged `tag`.
    fn tagged(&mut self, tag: Option<MetadataId>) {
        if let Some(tag) = tag {
            self.b.attach("tbaa", tag);
        }
    }

    /// Where a place starts.
    fn base(&mut self, place: &model::Place) -> Emit<Value> {
        match place.storage {
            Storage::Local => {
                let (object, offset) = self.frame[&place.id];
                Ok(self.offset(self.objects[object], offset, true))
            }
            Storage::Parameter => {
                let area = self.passed.ok_or("a parameter's storage in a function that is not variadic")?;
                Ok(self.offset(area, place.offset, false))
            }
            _ => {
                let global = Value::Constant(self.tables.data[&place.symbol]);
                Ok(self.offset(global, place.offset, false))
            }
        }
    }

    /// The element of an array place that `indices` name, `offset` bytes
    /// in, holding `ty`; no index names the place itself. The frontend
    /// promises the element is inside the array.
    fn element(&mut self, place: &model::Place, indices: &[Operand], offset: i64, ty: i64) -> Emit<(Value, TypeId)> {
        let base = self.base(place)?;
        let stored = stored_type(&mut self.b.context.types, self.tables.types[&ty])?;
        if indices.is_empty() {
            return Ok((self.offset(base, offset, true), stored));
        }
        let array = self.tables.types[&place.r#type];
        let element = self.tables.types[&array.element.ok_or("an indexed non-array")?];
        let element = stored_type(&mut self.b.context.types, element)?;
        let mut dimensions: Vec<(&Operand, &(i64, i64))> = indices.iter().zip(&array.bounds).collect();
        if self.tables.array_order == model::ArrayOrder::ColumnMajor {
            dimensions.reverse();
        }
        // An index is the pointer's index width, as C promotes one: a
        // narrower one a GEP would sign-extend.
        let space = match self.b.context.types.get(self.b.type_of(base)) {
            Type::Pointer(space) => *space,
            _ => 0,
        };
        let bits = self.tables.layout.pointer(space).index_bits;
        let width = self.b.context.types.int(bits);
        let mut linear: Option<Value> = None;
        for (operand, &(lower, upper)) in dimensions {
            let index = self.value(operand)?;
            let signed = self.operand_hir_type(operand).signed != Some(false);
            let index = self.convert(index, signed, width)?;
            let first = self.b.int(bits, i128::from(lower));
            let adjusted = self.b.binary(BinaryOp::Sub, index, first, Flags::default(), "");
            linear = Some(match linear {
                None => adjusted,
                Some(previous) => {
                    let count = self.b.int(bits, i128::from(upper - lower + 1));
                    let scaled = self.b.binary(BinaryOp::Mul, previous, count, Flags::default(), "");
                    self.b.binary(BinaryOp::Add, scaled, adjusted, Flags::default(), "")
                }
            });
        }
        let linear = linear.expect("an index");
        let pointer = self.b.gep(element, base, &[linear], Flags::INBOUNDS, "");
        Ok((self.offset(pointer, offset, true), stored))
    }

    /// `pointer` advanced by `offset` bytes.
    fn offset(&mut self, pointer: Value, offset: i64, inbounds: bool) -> Value {
        if offset == 0 {
            return pointer;
        }
        let byte = self.b.context.types.int(8);
        let index = self.b.int(16, i128::from(offset));
        let flags = if inbounds { Flags::INBOUNDS } else { Flags::default() };
        self.b.gep(byte, pointer, &[index], flags, "")
    }

    fn define(&mut self, instruction: &model::Instruction, value: Value) {
        if let Some(&result) = instruction.results.first() {
            self.values.insert(result, value);
        }
    }

    fn operands(&mut self, instruction: &model::Instruction) -> Emit<Vec<Value>> {
        instruction.operands.iter().map(|one| self.value(one)).collect()
    }

    fn instruction(&mut self, instruction: &model::Instruction) -> Emit<()> {
        let op = instruction.op;
        let binary = match op {
            Op::Add => Some(BinaryOp::Add),
            Op::Sub => Some(BinaryOp::Sub),
            Op::Mul => Some(BinaryOp::Mul),
            Op::Div => Some(BinaryOp::SDiv),
            Op::Rem => Some(BinaryOp::SRem),
            Op::Udiv => Some(BinaryOp::UDiv),
            Op::Urem => Some(BinaryOp::URem),
            Op::And => Some(BinaryOp::And),
            Op::Or => Some(BinaryOp::Or),
            Op::Xor => Some(BinaryOp::Xor),
            Op::Shl => Some(BinaryOp::Shl),
            Op::Shr => Some(BinaryOp::LShr),
            Op::Sar => Some(BinaryOp::AShr),
            Op::Fadd => Some(BinaryOp::FAdd),
            Op::Fsub => Some(BinaryOp::FSub),
            Op::Fmul => Some(BinaryOp::FMul),
            Op::Fdiv => Some(BinaryOp::FDiv),
            _ => None,
        };
        if let Some(binary) = binary {
            let [a, b] = self.operands(instruction)?[..] else { return Err(format!("{op} without two operands")) };
            // A shift count is its own width; LLVM's is the shifted value's.
            let b = if matches!(op, Op::Shl | Op::Shr | Op::Sar) { self.count(b, self.b.type_of(a))? } else { b };
            let flags = if instruction.nowrap && matches!(op, Op::Add | Op::Sub | Op::Mul) { Flags::NSW } else { Flags::default() };
            let result = self.b.binary(binary, a, b, flags, "");
            self.define(instruction, result);
            return Ok(());
        }
        if let Some((signed, unsigned, float)) = comparison(three_way(op).unwrap_or(op)) {
            let [mut a, mut b] = self.operands(instruction)?[..] else { return Err(format!("{op} without two operands")) };
            if three_way(op).is_some() {
                let callee = instruction.callee.as_deref().ok_or("a string comparison without a callee")?;
                let i16 = self.b.context.types.int(16);
                a = self.call(callee, &[a, b], i16).ok_or("a comparison's callee returns nothing")?;
                b = self.b.int(16, 0);
            }
            let is_float = matches!(self.b.context.types.get(self.b.type_of(a)), Type::Float(_));
            let truth = match (is_float, instruction.op) {
                (true, _) => self.b.fcmp(float, a, b, ""),
                (false, Op::Below | Op::BelowEq | Op::Above | Op::AboveEq) => self.b.icmp(unsigned, a, b, ""),
                (false, _) => self.b.icmp(signed, a, b, ""),
            };
            let ty = self.result_type(instruction.results[0])?;
            // BASIC's truth is all ones; an unsigned boolean's, as C's, is one.
            let one = self.hir_type(self.value_types[&instruction.results[0]]).signed == Some(false);
            let result = self.b.cast(if one { CastOp::ZExt } else { CastOp::SExt }, truth, ty, "");
            self.define(instruction, result);
            return Ok(());
        }
        if let Some(called) = called(op) {
            let mut arguments = self.operands(instruction)?;
            let i16 = self.b.context.types.int(16);
            arguments[0] = self.convert(arguments[0], false, i16)?;
            let returns = match instruction.results.first() {
                Some(&result) => self.result_type(result)?,
                None => self.b.context.types.void(),
            };
            let name = called_name(&self.b.context.types, called, arguments.last().map(|&one| self.b.type_of(one)), returns);
            let parameters = arguments.iter().map(|&one| self.b.type_of(one)).collect();
            let ty = function_type(&mut self.b.context.types, returns, parameters);
            let callee = Value::Constant(*self.tables.callees.get(&name).ok_or_else(|| format!("@{name} undeclared"))?);
            if let Some(result) = self.b.call(ty, callee, &arguments, "") {
                self.define(instruction, result);
            }
            return Ok(());
        }
        match op {
            Op::Copy | Op::Load => {
                let value = self.value(&instruction.operands[0])?;
                self.define(instruction, value);
            }
            Op::Store => {
                let (pointer, _, volatile, tag) = self.place(&instruction.operands[0])?;
                let value = self.value(&instruction.operands[1])?;
                self.b.store(value, pointer, volatile);
                self.tagged(tag);
            }
            Op::Address if instruction.operands.is_empty() => {
                let callee = instruction.callee.as_deref().ok_or("an address of nothing")?;
                let function = Value::Constant(*self.tables.callees.get(callee).ok_or_else(|| format!("@{callee} undeclared"))?);
                let ty = self.result_type(instruction.results[0])?;
                let pointer = match (self.b.context.types.get(self.b.type_of(function)), self.b.context.types.get(ty)) {
                    (Type::Pointer(from), Type::Pointer(to)) if from == to => function,
                    (Type::Pointer(_), Type::Pointer(_)) => self.b.cast(CastOp::AddrSpaceCast, function, ty, ""),
                    (_, other) => return Err(format!("a function's address as {other:?}")),
                };
                self.define(instruction, pointer);
            }
            Op::Address => {
                let (pointer, _, _, _) = self.place(&instruction.operands[0])?;
                let ty = self.result_type(instruction.results[0])?;
                let pointer = match (self.b.context.types.get(self.b.type_of(pointer)), self.b.context.types.get(ty)) {
                    (Type::Pointer(from), Type::Pointer(to)) if from == to => pointer,
                    (Type::Pointer(_), Type::Pointer(_)) => self.b.cast(CastOp::AddrSpaceCast, pointer, ty, ""),
                    (_, other) => return Err(format!("an address as {other:?}")),
                };
                if let (Operand::PlaceRef(one), Some(&result)) = (&instruction.operands[0], instruction.results.first()) {
                    let place = self.places[&one.place];
                    self.addresses.insert(result, place.extent.unwrap_or(self.tables.types[&place.r#type].width));
                }
                self.define(instruction, pointer);
            }
            Op::Truncate | Op::SignExtend | Op::ZeroExtend | Op::Convert => {
                let value = self.value(&instruction.operands[0])?;
                let signed = self.operand_hir_type(&instruction.operands[0]).signed != Some(false);
                let ty = self.result_type(instruction.results[0])?;
                let result = if op == Op::Truncate && matches!(self.b.context.types.get(self.b.type_of(value)), Type::Float(_)) {
                    // Toward zero; the result's signedness picks the cast.
                    let unsigned = self.hir_type(self.value_types[&instruction.results[0]]).signed == Some(false);
                    self.b.cast(if unsigned { CastOp::FPToUI } else { CastOp::FPToSI }, value, ty, "")
                } else {
                    self.convert(value, signed, ty)?
                };
                self.define(instruction, result);
            }
            Op::Divmod | Op::Udivmod => {
                let [a, b] = self.operands(instruction)?[..] else { return Err(format!("{op} without two operands")) };
                let (quotient, remainder) = if op == Op::Divmod { (BinaryOp::SDiv, BinaryOp::SRem) } else { (BinaryOp::UDiv, BinaryOp::URem) };
                let q = self.b.binary(quotient, a, b, Flags::default(), "");
                let r = self.b.binary(remainder, a, b, Flags::default(), "");
                self.values.insert(instruction.results[0], q);
                self.values.insert(instruction.results[1], r);
            }
            Op::Neg => {
                let value = self.value(&instruction.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a negated non-integer")?;
                let zero = self.b.int(bits, 0);
                let flags = if instruction.nowrap { Flags::NSW } else { Flags::default() };
                let result = self.b.binary(BinaryOp::Sub, zero, value, flags, "");
                self.define(instruction, result);
            }
            Op::Not => {
                let value = self.value(&instruction.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a complemented non-integer")?;
                let ones = self.b.int(bits, -1);
                let result = self.b.binary(BinaryOp::Xor, value, ones, Flags::default(), "");
                self.define(instruction, result);
            }
            // A segment's integer form is its selector, a far pointer's
            // segment:offset.
            Op::PointerSegment => {
                let far = self.value(&instruction.operands[0])?;
                let segment = self.b.context.types.ptr(SEGMENT);
                let segment = self.b.cast(CastOp::AddrSpaceCast, far, segment, "");
                let ty = self.result_type(instruction.results[0])?;
                let result = self.b.cast(CastOp::PtrToInt, segment, ty, "");
                self.define(instruction, result);
            }
            Op::PointerOffset => {
                let pointer = self.value(&instruction.operands[0])?;
                let ty = self.result_type(instruction.results[0])?;
                let result = self.b.cast(CastOp::PtrToInt, pointer, ty, "");
                self.define(instruction, result);
            }
            // At the 16-bit index width, which for a far pointer moves the
            // offset alone.
            Op::PtrOffset => {
                let [pointer, displacement] = self.operands(instruction)?[..] else { return Err("ptr_offset without two operands".to_owned()) };
                let signed = self.operand_hir_type(&instruction.operands[1]).signed != Some(false);
                let i16 = self.b.context.types.int(16);
                let displacement = self.convert(displacement, signed, i16)?;
                let byte = self.b.context.types.int(8);
                let flags = if instruction.inbounds { Flags::INBOUNDS } else { Flags::default() };
                let result = self.b.gep(byte, pointer, &[displacement], flags, "");
                self.define(instruction, result);
            }
            Op::Concat => {
                let [selector, offset] = self.operands(instruction)?[..] else { return Err("concat without two operands".to_owned()) };
                let segment = self.b.context.types.ptr(SEGMENT);
                let segment = self.b.cast(CastOp::IntToPtr, selector, segment, "");
                let ty = self.result_type(instruction.results[0])?;
                let base = self.b.cast(CastOp::AddrSpaceCast, segment, ty, "");
                let byte = self.b.context.types.int(8);
                let result = self.b.gep(byte, base, &[offset], Flags::default(), "");
                self.define(instruction, result);
            }
            Op::FixedMul | Op::FixedDiv => {
                let [a, b, _] = self.operands(instruction)?[..] else { return Err(format!("{op} without three operands")) };
                let fraction = match &instruction.operands[2] {
                    Operand::Constant(model::Constant { value: Number::Int(n), .. }) => i128::from(*n),
                    Operand::Constant(model::Constant { value: Number::Float(x), .. }) => *x as i128,
                    _ => return Err(format!("{op} by a variable fraction")),
                };
                let ty = self.b.type_of(a);
                let (name, parameters) = fixed(&mut self.b.context.types, op, ty).ok_or("fixed point of a non-integer")?;
                let callee = Value::Constant(*self.tables.callees.get(&name).ok_or_else(|| format!("@{name} undeclared"))?);
                let fraction = self.b.int(32, fraction);
                let function = function_type(&mut self.b.context.types, ty, parameters);
                let result = self.b.call(function, callee, &[a, b, fraction], "").expect("a fixed-point value");
                self.define(instruction, result);
            }
            Op::Fabs | Op::Fsqrt | Op::Fsin | Op::Fcos | Op::Fatan | Op::Flog2 | Op::Fexp2 | Op::Fround => {
                let value = self.value(&instruction.operands[0])?;
                let ty = self.result_type(instruction.results[0])?;
                let result = self.intrinsic(op, value, ty)?.ok_or_else(|| format!("{op} of a non-float"))?;
                self.define(instruction, result);
            }
            Op::Fneg => {
                let value = self.value(&instruction.operands[0])?;
                let result = self.b.fneg(value, "");
                self.define(instruction, result);
            }
            Op::Call if instruction.callee.is_none() => {
                let site = self.function.calls.iter().find(|one| one.instruction == instruction.id).ok_or("an indirect call without its ABI")?;
                let (convention, _) = convention(site.cleanup, site.distance)?;
                let operands = self.operands(instruction)?;
                let [callee, ref arguments @ ..] = operands[..] else { return Err("an indirect call of nothing".to_owned()) };
                let through = self.answer(instruction);
                let returns = match instruction.results.first() {
                    Some(_) if through == Answer::Through => self.b.type_of(*arguments.last().ok_or("a floating result with no destination")?),
                    Some(_) if through == Answer::Address => self.b.context.types.ptr(0),
                    Some(&result) => self.result_type(result)?,
                    None => self.b.context.types.void(),
                };
                let parameters = arguments.iter().map(|&one| self.b.type_of(one)).collect();
                let ty = function_type(&mut self.b.context.types, returns, parameters);
                let order: Vec<usize> = (1..instruction.operands.len()).collect();
                let attributes = self.extensions(instruction, &order);
                if let Some(answer) = self.raising_call(instruction.id, true, convention, ty, callee, arguments, &attributes)? {
                    let result = self.answered(instruction, through, answer)?;
                    self.define(instruction, result);
                }
            }
            Op::Call => {
                let callee = instruction.callee.as_deref().ok_or("a call without a callee")?;
                let operands = self.operands(instruction)?;
                let site = self.function.calls.iter().find(|one| one.instruction == instruction.id);
                let order = site.and_then(|site| passed(site, operands.len())).unwrap_or_else(|| (0..operands.len()).collect());
                let arguments: Vec<Value> = order.iter().map(|&one| operands[one]).collect();
                let filled = site
                    .map(|site| site.promises.iter().filter_map(|one| Some((order.iter().position(|&at| at as i64 == one.operand)?, one.bytes))).collect::<Vec<_>>())
                    .unwrap_or_default();
                let mut attributes = self.extensions(instruction, &order);
                for &(index, bytes) in &filled {
                    for attribute in [Attribute::Flag("nocapture".to_owned()), Attribute::Flag("writeonly".to_owned()), Attribute::Initializes(vec![(0, bytes)])] {
                        attributes.push((index, attribute));
                    }
                }
                let through = self.answer(instruction);
                let returns = match instruction.results[..] {
                    [_] if through == Answer::Through => self.b.type_of(*arguments.last().ok_or("a floating result with no destination")?),
                    [_] if through == Answer::Address => self.b.context.types.ptr(0),
                    [result] => self.result_type(result)?,
                    [] => self.b.context.types.void(),
                    ref results => {
                        let fields = results.iter().map(|&result| self.result_type(result)).collect::<Emit<_>>()?;
                        self.b.context.types.intern(Type::Struct { fields, packed: false })
                    }
                };
                if self.fallen_resume(callee) {
                    // Falling through, not by an error: "RESUME without
                    // error", raised as ERROR raises it, which the runtime
                    // places at this call.
                    let (convention, _) = convention(model::StackCleanup::Callee, model::CallDistance::Far)?;
                    let number = self.b.int(16, handling::RESUME_WITHOUT_ERROR);
                    let (void, word) = (self.b.context.types.void(), self.b.context.types.int(16));
                    let ty = function_type(&mut self.b.context.types, void, vec![word]);
                    let raise = Value::Constant(self.tables.callees[handling::RAISE]);
                    self.raising_call(instruction.id, true, convention, ty, raise, &[number], &[])?;
                    return Ok(());
                }
                let result = match self.handling_call(callee, returns)? {
                    Some(result) => result,
                    None => {
                        let parameters = arguments.iter().map(|&one| self.b.type_of(one)).collect();
                        let ty = function_type(&mut self.b.context.types, returns, parameters);
                        let convention = self.tables.conventions[callee];
                        let raises = !self.tables.nounwind.iter().any(|one| one == callee);
                        let callee = Value::Constant(self.tables.callees[callee]);
                        self.raising_call(instruction.id, raises, convention, ty, callee, &arguments, &attributes)?
                    }
                };
                if let Some(result) = result {
                    if instruction.results.len() > 1 {
                        for (index, &one) in instruction.results.iter().enumerate() {
                            let field = self.b.extract_value(result, index as u32, "");
                            self.values.insert(one, field);
                        }
                    } else {
                        let result = self.answered(instruction, through, result)?;
                        self.define(instruction, result);
                    }
                }
            }
            other => return Err(format!("HIR {other}")),
        }
        Ok(())
    }

    /// Each byte argument's extension to a stack word, as its HIR type's
    /// signedness says: LLVM's signext and zeroext. `order` is the operands
    /// in argument order.
    fn extensions(&self, instruction: &model::Instruction, order: &[usize]) -> Vec<(usize, Attribute)> {
        let values = self.function.values.iter().map(|one| (one.id, one.r#type)).collect();
        let places = self.function.places.iter().map(|one| (one.id, one)).collect();
        order
            .iter()
            .enumerate()
            .filter_map(|(index, &at)| {
                let ty = self.hir_type(operand_type(&instruction.operands[at], &values, &places));
                let signed = ty.signed.filter(|_| ty.kind == model::TypeKind::Integer && ty.width == 1)?;
                Some((index, Attribute::Flag(if signed { "signext" } else { "zeroext" }.to_owned())))
            })
            .collect()
    }

    /// How `instruction`'s result comes back.
    fn answer(&self, instruction: &model::Instruction) -> Answer {
        let Some(site) = self.function.calls.iter().find(|one| one.instruction == instruction.id) else { return Answer::Value };
        answer(site, &instruction.results, |result| self.hir_type(self.value_types[&result]))
    }

    /// The call's answer: the float loaded from where it returned the
    /// address of, where it returned one.
    fn answered(&mut self, instruction: &model::Instruction, through: Answer, answer: Value) -> Emit<Value> {
        if through == Answer::Value {
            return Ok(answer);
        }
        let ty = self.result_type(instruction.results[0])?;
        Ok(self.b.load(ty, answer, false, ""))
    }

    /// A call of the declared `callee` by its convention.
    fn call(&mut self, callee: &str, arguments: &[Value], returns: TypeId) -> Option<Value> {
        let parameters = arguments.iter().map(|&one| self.b.type_of(one)).collect();
        let ty = function_type(&mut self.b.context.types, returns, parameters);
        let convention = self.tables.conventions[callee];
        let callee = Value::Constant(self.tables.callees[callee]);
        self.b.call_as(convention, ty, callee, arguments, "")
    }

    /// The call of the intrinsic `op` from `value` to `to` makes, declared
    /// before the body.
    fn intrinsic(&mut self, op: Op, value: Value, to: TypeId) -> Emit<Option<Value>> {
        let from = self.b.type_of(value);
        let Some(name) = intrinsic(&self.b.context.types, op, from, to) else { return Ok(None) };
        let callee = Value::Constant(*self.tables.callees.get(&name).ok_or_else(|| format!("@{name} undeclared"))?);
        let ty = function_type(&mut self.b.context.types, to, vec![from]);
        Ok(self.b.call(ty, callee, &[value], ""))
    }

    /// A shift count at the shifted value's width.
    fn count(&mut self, value: Value, ty: TypeId) -> Emit<Value> {
        let from = self.b.type_of(value);
        if from == ty {
            return Ok(value);
        }
        let types = &self.b.context.types;
        match (types.int_bits(from), types.int_bits(ty)) {
            (Some(a), Some(b)) if a < b => Ok(self.b.cast(CastOp::ZExt, value, ty, "")),
            (Some(_), Some(_)) => Ok(self.b.cast(CastOp::Trunc, value, ty, "")),
            _ => Err(format!("operands of {} and {}", types.display(from), types.display(ty))),
        }
    }

    fn convert(&mut self, value: Value, signed: bool, to: TypeId) -> Emit<Value> {
        let from = self.b.type_of(value);
        if from == to {
            return Ok(value);
        }
        let types = &self.b.context.types;
        let op = match (types.get(from), types.get(to)) {
            (Type::Int(a), Type::Int(b)) if a > b => CastOp::Trunc,
            (Type::Int(_), Type::Int(_)) if signed => CastOp::SExt,
            (Type::Int(_), Type::Int(_)) => CastOp::ZExt,
            (Type::Int(_), Type::Float(_)) if signed => CastOp::SIToFP,
            (Type::Int(_), Type::Float(_)) => CastOp::UIToFP,
            (Type::Float(a), Type::Float(b)) if float_bits(*a) < float_bits(*b) => CastOp::FPExt,
            (Type::Float(a), Type::Float(b)) if float_bits(*a) > float_bits(*b) => CastOp::FPTrunc,
            (Type::Float(_), Type::Int(_)) => return Ok(self.intrinsic(Op::Convert, value, to)?.expect("a rounding")),
            (Type::Int(_), Type::Pointer(_)) => CastOp::IntToPtr,
            (Type::Pointer(_), Type::Int(_)) => CastOp::PtrToInt,
            (Type::Pointer(_), Type::Pointer(_)) => CastOp::AddrSpaceCast,
            (a, b) => return Err(format!("a conversion from {a:?} to {b:?}")),
        };
        Ok(self.b.cast(op, value, to, ""))
    }

    fn terminator(&mut self, terminator: &model::Terminator) -> Emit<()> {
        match terminator.kind {
            TerminatorKind::Jump => self.b.br(self.block(terminator.targets[0])),
            TerminatorKind::Branch => {
                let value = self.value(&terminator.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a branch on a non-integer")?;
                let zero = self.b.int(bits, 0);
                let condition = self.b.icmp(IntPredicate::Ne, value, zero, "");
                self.b.cond_br(condition, self.block(terminator.targets[0]), self.block(terminator.targets[1]));
            }
            TerminatorKind::Switch => {
                let value = self.value(&terminator.operands[0])?;
                let bits = self.b.context.types.int_bits(self.b.type_of(value)).ok_or("a switch on a non-integer")?;
                let cases: Vec<(Value, BlockId)> = terminator.cases.iter().map(|&(case, target)| (self.b.int(bits, i128::from(case)), self.block(target))).collect();
                self.b.switch(value, self.block(terminator.targets[0]), &cases);
            }
            TerminatorKind::Return if self.outlined.is_some() => {
                // The module handler run to the module's end is "No
                // RESUME", raised with trapping off: it ends the program.
                let (convention, _) = convention(model::StackCleanup::Callee, model::CallDistance::Far)?;
                let (void, word) = (self.b.context.types.void(), self.b.context.types.int(16));
                let ty = function_type(&mut self.b.context.types, void, vec![word]);
                let raise = Value::Constant(self.tables.callees[handling::RAISE]);
                let number = self.b.int(16, handling::NO_RESUME);
                self.b.call_as(convention, ty, raise, &[number], "");
                self.b.unreachable();
            }
            TerminatorKind::Return => {
                let mut value = terminator.operands.first().map(|one| self.value(one)).transpose()?;
                if let Some(destination) = self.destination {
                    // No value stores nothing, and the caller still loads through what comes back.
                    if let Some(result) = value {
                        self.b.store(result, destination, false);
                    }
                    value = Some(destination);
                }
                let Type::Function { returns, .. } = self.b.context.types.get(self.b.function.ty).clone() else { unreachable!("a function type") };
                if value.is_none() && !self.b.context.types.is_void(returns) {
                    // No value from a function that has one: nothing the caller may read.
                    value = Some(Value::Constant(self.b.context.constant(Constant { ty: returns, kind: ConstantKind::Poison })));
                }
                self.b.ret(value);
            }
            TerminatorKind::Unreachable => self.b.unreachable(),
        }
        Ok(())
    }
}

/// A comparison's signed, unsigned and floating predicates.
fn comparison(op: Op) -> Option<(IntPredicate, IntPredicate, FloatPredicate)> {
    Some(match op {
        Op::Eq => (IntPredicate::Eq, IntPredicate::Eq, FloatPredicate::Oeq),
        Op::Ne => (IntPredicate::Ne, IntPredicate::Ne, FloatPredicate::Une),
        Op::Lt | Op::Below => (IntPredicate::Slt, IntPredicate::Ult, FloatPredicate::Olt),
        Op::Le | Op::BelowEq => (IntPredicate::Sle, IntPredicate::Ule, FloatPredicate::Ole),
        Op::Gt | Op::Above => (IntPredicate::Sgt, IntPredicate::Ugt, FloatPredicate::Ogt),
        Op::Ge | Op::AboveEq => (IntPredicate::Sge, IntPredicate::Uge, FloatPredicate::Oge),
        _ => return None,
    })
}
