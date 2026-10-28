//! Split an internal variable into the pieces its loads read, then give a
//! piece one function writes before it reads to that function as a stack slot:
//! LLVM's `SRAGlobal` and the localizing branch of `processInternalGlobal`
//! (GlobalOpt.cpp, d1106deb71cc).
//!
//! # How it works
//! A variable qualifies when it is internal, initialized, not `constant`, not
//! externally initialized and not named by another global's initializer, and
//! every use in every body is a non-volatile load's or store's address, found
//! through geps and casts with only constant indices. Each load is a piece
//! (offset and integer type); two loads of one offset with two types, or
//! overlapping loads, leave the variable alone, as `collectSRATypes` does. A
//! store of the piece's type is a store of the piece. A store of a constant
//! that covers whole pieces is one store of each (DEVIATE GSRA-1, below); a
//! store that covers no piece is dead. A piece that is loaded and stored
//! beyond 16 leaves the variable alone. Each piece is a new internal variable
//! `name.N` initialized from the original's bytes.
//!
//! A variable with one accessing body that does not recurse, whose every load
//! a store of at least its size dominates
//! (`isPointerValueDeadOnEntryToFunction`, at most 100 load-store pairs), is
//! replaced by an `alloca` in that body's entry, stored with the initializer
//! first. The body passes then forward the slot. The pieces are made first, so
//! a piece localizes.
//!
//! DEVIATE GSRA-1: the pin merges consecutive stores in the DAG at selection,
//! after its second GlobalOpt, so its SRA sees `store i16`; the `Merge` slot
//! runs in MIR before the walk ends, so this one sees `store i32` over two
//! `i16` loads. A constant store over several pieces is split into one store
//! per piece (`collectSRATypes` requires one type per offset). Exit: store
//! merging at its place in selection, or a GlobalOpt slot before `Merge`.
//! Owner w-8, 2026-11-05.
//!
//! # Example
//! Before:
//!
//! ```mir
//! @c = internal global [4 x i16] zeroinitializer
//!
//! define i16 @f() {
//! b0:
//!   store i32 65537, ptr @c
//!   %a = load i16, ptr @c
//!   %b = load i16, ptr getelementptr inbounds (i8, ptr @c, i32 2)
//!   %r = add i16 %a, %b
//!   ret i16 %r
//! }
//! ```
//!
//! After (`@c.0`, `@c.1` are the pieces, then slots):
//!
//! ```mir
//! define i16 @f() {
//! b0:
//!   %c.0 = alloca i16
//!   store i16 0, ptr %c.0
//!   %c.1 = alloca i16
//!   store i16 0, ptr %c.1
//!   store i16 1, ptr %c.0
//!   store i16 1, ptr %c.1
//!   %a = load i16, ptr %c.0
//!   %b = load i16, ptr %c.1
//!   ...
//! }
//! ```
//!
//! # Runs
//! A stage of the interprocedural step, `globalsra.`, after the bodies are
//! stamped (a body's `norecurse` is stated there); the bodies it changed go
//! back through the pipeline.
//!
//! # See also
//! [`crate::globalopt`], LLVM's `SRAGlobal`, `collectSRATypes`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::memory::constant_bytes;
use llrm_mir::context::{Constant, ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::facts::Facts;
use llrm_mir::module::{GlobalKind, GlobalValue, GlobalVariable, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::pointer_base_with_constant_offset;

use crate::globalopt::Reach;

/// LLVM's cap on pieces both loaded and stored.
const MAX_PIECES: usize = 16;
/// LLVM's cap on load-store pairs examined by
/// `isPointerValueDeadOnEntryToFunction`.
const MAX_PAIRS: usize = 100;

/// One load or store of a variable.
#[derive(Clone)]
struct Access {
    function: GlobalId,
    inst: InstId,
    offset: i64,
    store: bool,
    ty: TypeId,
    size: u64,
    /// The integer a store writes, where it is a constant.
    constant: Option<u128>,
}

/// Each function's uses of each global.
fn reaching(module: &Module) -> BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>> {
    let mut named = BTreeMap::new();
    let mut all = BTreeMap::new();
    for (function_id, _, function) in module.functions() {
        if !function.is_declaration() {
            all.insert(function_id, crate::globalopt::reach_all(&module.context, function, &mut named));
        }
    }
    all
}

/// The functions changed.
pub fn split(
    module: &mut Module,
    layout: &DataLayout,
    spaces: llrm_mir::spaces::Spaces,
) -> Vec<GlobalId> {
    // A body that forges a pointer into the data segment (`DEF SEG = VARSEG(w)`
    // then POKE) may write any variable by its address: none is a register
    // (GlobalsAA's `forged`, 188fd3905).
    if llrm_analysis::forging::forges_selector(module, spaces) {
        return Vec::new();
    }
    let mut changed = BTreeSet::new();
    let held = held(module);
    let candidates: Vec<GlobalId> = module
        .globals
        .iter()
        .enumerate()
        .filter_map(|(at, global)| {
            let GlobalKind::Variable(variable) = &global.kind else { return None };
            let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
            let id = GlobalId(at as u32);
            (internal
                && global.address_space == 0
                && !variable.constant
                && !variable.externally_initialized
                && variable.initializer.is_some()
                && !held.contains(&id))
            .then_some(id)
        })
        .collect();
    let reached = reaching(module);
    for id in candidates {
        let Some(accesses) = accesses(module, layout, id, &reached) else { continue };
        let localizes = localizing(module, layout, id, &reached, &mut BTreeMap::new());
        let pieces = sliced(module, layout, id, &accesses, localizes);
        let Some(pieces) = pieces else { continue };
        let moved: BTreeMap<GlobalId, Vec<InstId>> = reached
            .iter()
            .filter_map(|(&function, table)| Some((function, table.get(&id)?.as_ref()?.moved.clone())))
            .collect();
        changed.extend(rewrite(module, id, &accesses, &pieces, &moved));
    }
    let reached = reaching(module);
    let mut trees = BTreeMap::new();
    for id in locals(module) {
        if let Some(function) = localize(module, layout, id, &reached, &mut trees) {
            changed.insert(function);
        }
    }
    let reached = reaching(module);
    changed.extend(forwarded(module, &reached, &mut trees));
    changed.into_iter().collect()
}

/// `forwardStoredOnceStore`: an internal variable stored once, a constant that
/// is not its initializer, has each load in the storing function that the store
/// dominates replaced by the constant.
fn forwarded(
    module: &mut Module,
    reached: &BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>>,
    trees: &mut BTreeMap<GlobalId, DomTree>,
) -> BTreeSet<GlobalId> {
    let held = held(module);
    let mut plans: Vec<(GlobalId, Vec<InstId>, llrm_mir::context::ConstantId)> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let id = GlobalId(at as u32);
        let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
        let Some(initializer) = variable.initializer else { continue };
        if !internal || variable.constant || variable.externally_initialized || held.contains(&id) {
            continue;
        }
        // Every use a direct, simple load or store (`GlobalStatus`), one store.
        let Some(direct) = direct_uses(module, id, reached) else { continue };
        let mut stores = direct.iter().flat_map(|one| one.stores.iter().map(|&store| (one.function, store)));
        let (Some((owner, store)), None) = (stores.next(), stores.next()) else { continue };
        let GlobalKind::Function(owning) = &module.globals[owner.0 as usize].kind else { continue };
        let Operand::Constant(value) = owning.instruction(store).operands[0] else { continue };
        if value == initializer || direct.iter().all(|one| one.loads.is_empty()) {
            continue;
        }
        let loads: Vec<(GlobalId, InstId)> =
            direct.iter().flat_map(|one| one.loads.iter().map(|&load| (one.function, load))).collect();
        let GlobalKind::Function(function) = &module.globals[owner.0 as usize].kind else { continue };
        let value_ty = module.context.get(value).ty;
        let tree = trees.entry(owner).or_insert_with(|| DomTree::of(function));
        let dominated: Vec<InstId> = loads
            .iter()
            .filter(|&&(function_id, load)| {
                function_id == owner
                    && function.instruction(load).ty == value_ty
                    && tree.instruction_dominates(function, store, load)
            })
            .map(|&(_, load)| load)
            .collect();
        if !dominated.is_empty() {
            plans.push((owner, dominated, value));
        }
    }
    let mut changed = BTreeSet::new();
    for (owner, loads, value) in plans {
        let GlobalKind::Function(function) = &mut module.globals[owner.0 as usize].kind else { continue };
        for load in loads {
            let Some(result) = function.instruction(load).result else { continue };
            function.replace_all_uses_with(result, Operand::Constant(value));
            if function.erase(load).is_ok() {
                changed.insert(owner);
            }
        }
        trees.remove(&owner);
    }
    changed
}

/// The loads and stores one function makes of a variable.
struct Direct {
    function: GlobalId,
    loads: Vec<InstId>,
    stores: Vec<InstId>,
}

/// Whether `constant` is `id`'s own address, through casts alone.
fn is_the_variable(
    context: &llrm_mir::context::Context,
    constant: llrm_mir::context::ConstantId,
    id: GlobalId,
) -> bool {
    use llrm_mir::context::{ConstantExpr, ConstantKind};
    use llrm_mir::opcode::CastOp;
    match &context.get(constant).kind {
        ConstantKind::Global(one) => *one == id,
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
            is_the_variable(context, *value, id)
        }
        _ => false,
    }
}

/// The functions that use `id`, or none where a use is not a direct, simple
/// load or store: a gep on the way is an offset another pass folds.
fn direct_uses(
    module: &Module,
    id: GlobalId,
    reached: &BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>>,
) -> Option<Vec<Direct>> {
    let mut found = Vec::new();
    for (&function_id, table) in reached {
        let Some(reach) = table.get(&id) else { continue };
        let reach = reach.as_ref()?;
        let GlobalKind::Function(function) = &module.globals[function_id.0 as usize].kind else { continue };
        if reach.uses.is_empty() {
            continue;
        }
        if !reach.moved.is_empty() {
            return None;
        }
        let mut direct = Direct { function: function_id, loads: Vec::new(), stores: Vec::new() };
        for &(inst, index) in &reach.uses {
            // `GlobalStatus` sees through `stripPointerCasts`, not through a
            // gep: a constant gep names a byte of the variable,
            // whatever its value, not the variable.
            let Operand::Constant(operand) = function.instruction(inst).operands[index] else { return None };
            if !is_the_variable(&module.context, operand, id) {
                return None;
            }
            match function.instruction(inst).opcode {
                Opcode::Load { volatile: false, .. } if index == 0 => direct.loads.push(inst),
                Opcode::Store { volatile: false, .. } if index == 1 => direct.stores.push(inst),
                _ => return None,
            }
        }
        found.push(direct);
    }
    Some(found)
}

/// The globals another global's initializer names.
fn held(module: &Module) -> BTreeSet<GlobalId> {
    crate::globalopt::held_by_live(module)
}

/// Every use of `id`, or none where one is not a plain load or store at a
/// constant offset.
fn accesses(
    module: &Module,
    layout: &DataLayout,
    id: GlobalId,
    reached: &BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>>,
) -> Option<Vec<Access>> {
    let context = &module.context;
    let mut found = Vec::new();
    for (&function_id, table) in reached {
        let Some(reached) = table.get(&id) else { continue };
        let reached = reached.as_ref()?;
        let GlobalKind::Function(function) = &module.globals[function_id.0 as usize].kind else { continue };
        for &(inst, index) in &reached.uses {
            let instruction = function.instruction(inst);
            let (store, pointer) = match instruction.opcode {
                Opcode::Load { volatile: false, .. } if index == 0 => (false, instruction.operands[0]),
                Opcode::Store { volatile: false, .. } if index == 1 => (true, instruction.operands[1]),
                _ => return None,
            };
            let (base, offset) = pointer_base_with_constant_offset(context, layout, function, pointer);
            let Operand::Constant(base) = base else { return None };
            if context.get(base).kind != ConstantKind::Global(id) {
                return None;
            }
            let (ty, constant) = if store {
                let value = instruction.operands[0];
                let ty = function.operand_type(context, value)?;
                let constant = match value {
                    Operand::Constant(one) => match context.get(one).kind {
                        ConstantKind::Int(bits) => Some(bits),
                        ConstantKind::Zero => Some(0),
                        _ => None,
                    },
                    _ => None,
                };
                (ty, constant)
            } else {
                (instruction.ty, None)
            };
            let size = layout.store_size(&context.types, ty);
            found.push(Access { function: function_id, inst, offset, store, ty, size, constant });
        }
    }
    Some(found)
}

/// The bytes of a type a piece may be: an integer, a `float` or a `double`
/// (`collectSRATypes` takes any first-class scalar; the x87 `fp80` is left).
fn piece_bytes(
    context: &llrm_mir::context::Context,
    ty: TypeId,
) -> Option<u64> {
    use llrm_mir::types::{FloatKind, Type};
    match context.types.get(ty) {
        Type::Int(bits) => Some(u64::from(*bits).div_ceil(8)),
        Type::Float(FloatKind::Float) => Some(4),
        Type::Float(FloatKind::Double) => Some(8),
        _ => None,
    }
}

/// The pieces to split into, sorted by offset: (offset, type, initializer).
fn sliced(
    module: &Module,
    layout: &DataLayout,
    id: GlobalId,
    accesses: &[Access],
    localizes: bool,
) -> Option<Vec<(i64, TypeId, Constant)>> {
    let context = &module.context;
    let GlobalKind::Variable(variable) = &module.globals[id.0 as usize].kind else { return None };
    let mut pieces: BTreeMap<i64, TypeId> = BTreeMap::new();
    for access in accesses.iter().filter(|one| !one.store) {
        piece_bytes(context, access.ty)?;
        if *pieces.entry(access.offset).or_insert(access.ty) != access.ty {
            return None;
        }
    }
    if pieces.is_empty() {
        return None;
    }
    // Not back to the same layout: one piece that is all of the variable (a
    // scalar the frontend spelled as bytes) is no split, unless it becomes a
    // stack slot: renamed and moved in the data it cost d_faces 623 bytes.
    let whole = layout.alloc_size(&context.types, variable.ty) as i64;
    if pieces.len() == 1
        && (pieces.values().next() == Some(&variable.ty)
            || (!localizes
                && pieces
                    .iter()
                    .next()
                    .is_some_and(|(&at, &ty)| at == 0 && layout.store_size(&context.types, ty) as i64 == whole)))
    {
        return None;
    }
    let mut end = 0;
    for (&offset, &ty) in &pieces {
        if offset < end {
            return None;
        }
        end = offset + layout.store_size(&context.types, ty) as i64;
    }
    if end > whole {
        return None;
    }
    let size = |offset: i64| layout.store_size(&context.types, pieces[&offset]) as i64;
    let mut stored: BTreeSet<i64> = BTreeSet::new();
    for access in accesses.iter().filter(|one| one.store) {
        let (low, high) = (access.offset, access.offset + access.size as i64);
        let inside: Vec<i64> = pieces.keys().copied().filter(|&at| at < high && at + size(at) > low).collect();
        match inside.as_slice() {
            [] => {}
            [only] if *only == low && size(*only) == access.size as i64 && pieces[only] == access.ty => {
                stored.insert(*only);
            }
            several => {
                // A constant store over whole pieces.
                access.constant?;
                let (first, last) = (*several.first()?, *several.last()?);
                if first < low || last + size(last) > high {
                    return None;
                }
                stored.extend(several.iter().copied());
            }
        }
    }
    let both = pieces.keys().filter(|at| stored.contains(at)).count();
    if both > MAX_PIECES {
        return None;
    }
    let bytes = constant_bytes(context, layout, variable.initializer?)?;
    let mut out = Vec::new();
    for (&offset, &ty) in &pieces {
        let width = size(offset) as usize;
        let read = bytes.get(usize::try_from(offset).ok()?..usize::try_from(offset).ok()? + width)?;
        if width > 16 {
            return None;
        }
        let bits = read.iter().rev().fold(0u128, |bits, &byte| (bits << 8) | u128::from(byte));
        let kind = if context.types.int_bits(ty).is_some() {
            ConstantKind::Int(bits)
        } else {
            ConstantKind::Float(bits as u64)
        };
        out.push((offset, ty, Constant { ty, kind }));
    }
    Some(out)
}

/// The functions whose stores and loads of `id` now name its pieces.
fn rewrite(
    module: &mut Module,
    id: GlobalId,
    accesses: &[Access],
    pieces: &[(i64, TypeId, Constant)],
    moved: &BTreeMap<GlobalId, Vec<InstId>>,
) -> BTreeSet<GlobalId> {
    let layout_sizes: Vec<i64> =
        pieces.iter().map(|(_, ty, _)| piece_bytes(&module.context, *ty).unwrap_or(1) as i64).collect();
    let name = module.globals[id.0 as usize].name.as_deref().unwrap_or("").to_owned();
    let (align, space) = match &module.globals[id.0 as usize].kind {
        GlobalKind::Variable(variable) => (variable.align, module.globals[id.0 as usize].address_space),
        GlobalKind::Function(_) => return BTreeSet::new(),
    };
    let mut made: Vec<(GlobalId, llrm_mir::context::ConstantId)> = Vec::new();
    for (n, (_, ty, initializer)) in pieces.iter().enumerate() {
        let mut unique = format!("{name}.{n}");
        let mut suffix = n;
        while module.named(&unique).is_some() {
            suffix += pieces.len();
            unique = format!("{name}.{suffix}");
        }
        let initializer = module.context.constant(initializer.clone());
        let piece = GlobalId(module.globals.len() as u32);
        module
            .globals
            .push(
                GlobalValue {
                    name: unique.into(),
                    linkage: Linkage::Internal,
                    unnamed_addr: Default::default(),
                    address_space: space,
                    kind: GlobalKind::Variable(GlobalVariable {
                        ty: *ty,
                        constant: false,
                        externally_initialized: false,
                        initializer: Some(initializer),
                        align,
                    }),
                },
            );
        let pointer = module.context.types.ptr(space);
        let address = module.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(piece) });
        made.push((piece, address));
    }
    let at = |offset: i64| pieces.iter().position(|(one, _, _)| *one == offset);
    let mut changed = BTreeSet::new();
    let Module { context, globals, .. } = module;
    let reached: BTreeMap<GlobalId, Vec<InstId>> = accesses
        .iter()
        .map(|one| one.function)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|function| (function, moved.get(&function).cloned().unwrap_or_default()))
        .collect();
    for access in accesses {
        let void = context.types.void();
        let GlobalKind::Function(function) = &mut globals[access.function.0 as usize].kind else { continue };
        changed.insert(access.function);
        if let Some(piece) = at(access.offset).filter(|&piece| {
            let exact = access.store && pieces[piece].1 == access.ty;
            exact || !access.store
        }) {
            let index = if access.store { 1 } else { 0 };
            function.set_operand(access.inst, index, Operand::Constant(made[piece].1));
            continue;
        }
        // A store over whole pieces, or over none.
        let (low, high) = (access.offset, access.offset + access.size as i64);
        let inside: Vec<usize> = pieces
            .iter()
            .enumerate()
            .filter(|(n, (offset, _, _))| *offset < high && *offset + layout_sizes[*n] > low)
            .map(|(n, _)| n)
            .collect();
        let bits = access.constant.unwrap_or(0);
        for n in inside {
            let (offset, ty, _) = &pieces[n];
            let width = (layout_sizes[n] * 8) as u32;
            let part = (bits >> ((*offset - low) * 8)) & llrm_mir::context::mask(width);
            let kind = if context.types.int_bits(*ty).is_some() {
                ConstantKind::Int(part)
            } else {
                ConstantKind::Float(part as u64)
            };
            let value = context.constant(Constant { ty: *ty, kind });
            let store = function.create_instruction(
                Opcode::Store { align: None, volatile: false },
                void,
                vec![Operand::Constant(value), Operand::Constant(made[n].1)],
                Flags::default(),
                None,
            );
            let _ = function.insert(store, Position::Before(access.inst));
        }
        let _ = function.erase(access.inst);
    }
    for (function, moved) in reached {
        let GlobalKind::Function(body) = &mut globals[function.0 as usize].kind else { continue };
        let mut left: Vec<InstId> = moved.into_iter().filter(|&one| !body.is_erased(one)).collect();
        while !left.is_empty() {
            let dead: Vec<InstId> = left
                .iter()
                .copied()
                .filter(|&one| body.instruction(one).result.is_none_or(|result| body.users(result).is_empty()))
                .collect();
            if dead.is_empty() || body.erase_all(&dead).is_err() {
                break;
            }
            left.retain(|one| !dead.contains(one));
        }
    }
    changed
}

/// The internal single-value variables (`isSingleValueType`: integer, float,
/// pointer, vector) a body of one function could keep on
/// its stack: every use a direct load or store.
fn locals(module: &Module) -> Vec<GlobalId> {
    let held = held(module);
    module
        .globals
        .iter()
        .enumerate()
        .filter_map(|(at, global)| {
            let GlobalKind::Variable(variable) = &global.kind else { return None };
            let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
            let single = matches!(
                module.context.types.get(variable.ty),
                Type::Int(_) | Type::Float(_) | Type::Pointer(_) | Type::Vector { .. }
            );
            let id = GlobalId(at as u32);
            (internal
                && single
                && global.address_space == 0
                && !variable.constant
                && !variable.externally_initialized
                && variable.initializer.is_some()
                && !held.contains(&id))
            .then_some(id)
        })
        .collect()
}

/// The one function that could keep `id` on its stack and the loads and stores
/// it makes of it: the function does not recurse and writes it before it reads
/// it.
fn slot_plan(
    module: &Module,
    layout: &DataLayout,
    id: GlobalId,
    reached: &BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>>,
    trees: &mut BTreeMap<GlobalId, DomTree>,
) -> Option<(GlobalId, Vec<InstId>, Vec<InstId>)> {
    let context = &module.context;
    let [Direct { function: owner, loads, stores }] = direct_uses(module, id, reached)?.try_into().ok()?;
    if loads.len() * stores.len() > MAX_PAIRS {
        return None;
    }
    let GlobalKind::Function(function) = &module.globals[owner.0 as usize].kind else { return None };
    if !Facts::of(&function.attrs).no_recurse() {
        return None;
    }
    let tree = trees.entry(owner).or_insert_with(|| DomTree::of(function));
    let size = |inst: InstId, index: usize| {
        let ty = function.operand_type(context, function.instruction(inst).operands[index])?;
        Some(layout.store_size(&context.types, ty))
    };
    for &load in &loads {
        let loaded = layout.store_size(&context.types, function.instruction(load).ty);
        let written = stores.iter().any(|&store| {
            tree.instruction_dominates(function, store, load) && size(store, 0).is_some_and(|stored| loaded <= stored)
        });
        if !written {
            return None;
        }
    }
    Some((owner, loads, stores))
}

/// Whether `id` could be a stack slot (`slot_plan`).
fn localizing(
    module: &Module,
    layout: &DataLayout,
    id: GlobalId,
    reached: &BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>>,
    trees: &mut BTreeMap<GlobalId, DomTree>,
) -> bool {
    slot_plan(module, layout, id, reached, trees).is_some()
}

/// Replaces `id` by a stack slot of the one function that uses it, if the
/// function does not recurse and writes it before it reads it.
fn localize(
    module: &mut Module,
    layout: &DataLayout,
    id: GlobalId,
    reached: &BTreeMap<GlobalId, BTreeMap<GlobalId, Option<Reach>>>,
    trees: &mut BTreeMap<GlobalId, DomTree>,
) -> Option<GlobalId> {
    let (owner, loads, stores) = slot_plan(module, layout, id, reached, trees)?;
    let GlobalKind::Variable(variable) = &module.globals[id.0 as usize].kind else { return None };
    let (ty, initializer, align) = (variable.ty, variable.initializer?, variable.align);
    let GlobalKind::Function(function) = &module.globals[owner.0 as usize].kind else { return None };
    let entry = function.entry()?;
    let first = function.block(entry).first()?;
    let Module { context, globals, .. } = module;
    let void = context.types.void();
    let GlobalKind::Function(function) = &mut globals[owner.0 as usize].kind else { return None };
    let pointer = context.types.ptr(0);
    let slot = function.create_instruction(
        Opcode::Alloca { allocated: ty, align, address_space: 0 },
        pointer,
        Vec::new(),
        Flags::default(),
        None,
    );
    function.insert(slot, Position::Before(first)).ok()?;
    let result = function.instruction(slot).result?;
    if !matches!(
        context.get(initializer).kind,
        ConstantKind::Undef | ConstantKind::Poison
    ) {
        let init = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![Operand::Constant(initializer), Operand::Value(result)],
            Flags::default(),
            None,
        );
        function.insert(init, Position::Before(first)).ok()?;
    }
    for inst in loads {
        function.set_operand(inst, 0, Operand::Value(result));
    }
    for inst in stores {
        function.set_operand(inst, 1, Operand::Value(result));
    }
    Some(owner)
}

#[cfg(test)]
#[path = "globalsra_tests.rs"]
mod tests;
