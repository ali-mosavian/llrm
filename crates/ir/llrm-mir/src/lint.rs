//! What a frontend's MIR must not hold before any pass can exploit it:
//! poison the source language defines a value for. A flag on an instruction
//! is the language's stated promise (`nsw`, `inbounds`, ...), not poison; a
//! raise holds no `poison` constant and reads no alloca that nothing stores
//! to before the read.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::intrinsics::Intrinsic;
use llrm_support::hash::{HashMap, HashSet};

use crate::module::{BlockId, Function, GlobalKind, InstId, Module, Operand, Use};
use crate::opcode::Opcode;

pub fn poison(module: &Module) -> Vec<String> {
    let mut out = Vec::new();
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.as_deref().unwrap_or("<unnamed>");
        out.extend(function_poison(module, function).into_iter().map(|one| format!("@{name}: {one}")));
    }
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind
            && variable.initializer.is_some_and(|one| holds_poison(&module.context, one))
        {
            out.push(format!("@{}: a poison initializer", global.name.as_deref().unwrap_or("<unnamed>")));
        }
    }
    out
}

fn holds_poison(context: &Context, constant: crate::context::ConstantId) -> bool {
    match &context.get(constant).kind {
        ConstantKind::Poison => true,
        ConstantKind::Aggregate(members) => members.iter().any(|&one| holds_poison(context, one)),
        ConstantKind::Expr(crate::context::ConstantExpr::GetElementPtr { operands, .. }) => operands.iter().any(|&one| holds_poison(context, one)),
        ConstantKind::Expr(crate::context::ConstantExpr::Cast { value, .. }) => holds_poison(context, *value),
        _ => false,
    }
}

fn function_poison(module: &Module, function: &Function) -> Vec<String> {
    let context = &module.context;
    let layout = module.datalayout.as_deref().and_then(|one| DataLayout::parse(one).ok());
    let mut out = Vec::new();
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let mnemonic = instruction.opcode.mnemonic();
        if instruction.operands.iter().any(|&one| matches!(one, Operand::Constant(id) if holds_poison(context, id))) {
            out.push(format!("{mnemonic} uses poison"));
        }
        if let Opcode::Alloca { allocated, .. } = instruction.opcode {
            let slot = instruction.result.expect("an alloca's pointer");
            // The slot and every pointer derived from it by an address computation.
            let mut pointers = vec![slot];
            let mut at = 0;
            while at < pointers.len() {
                for &Use { user, .. } in function.users(pointers[at]) {
                    let derived = function.instruction(user);
                    if matches!(derived.opcode, Opcode::GetElementPtr { .. } | Opcode::Cast(_)) {
                        pointers.extend(derived.result.filter(|one| !pointers.contains(one)));
                    }
                }
                at += 1;
            }
            // What fills all of it: a store of the whole type through the slot, a memset of
            // all of it, a call it is handed to. And what fills part: any other store through
            // the slot or a pointer computed from it (an array is filled an element at a
            // time, a loop at a time).
            let (mut whole, mut parts) = (Vec::new(), Vec::new());
            for &pointer in &pointers {
                for &one in function.users(pointer) {
                    let stores = one.index == 1 && matches!(function.instruction(one.user).opcode, Opcode::Store { .. });
                    if (pointer == slot && is_store_of(function, context, one, allocated)) || is_fill_of(module, layout.as_ref(), function, one, allocated) || (matches!(function.instruction(one.user).opcode, Opcode::Call(_)) && !is_memset(module, function, one.user)) {
                        whole.push(one.user);
                    } else if stores {
                        parts.push(one.user);
                    }
                }
            }
            // A load is read after a whole fill on every path, or after a part that can reach it
            // (a loop's stores are not on the path that skips the loop); one reached by neither
            // reads what nothing stored.
            let after_whole = stored_on_every_path(function, &whole);
            let after_part = may_follow(function, &parts);
            for &pointer in &pointers {
                for &Use { user, .. } in function.users(pointer) {
                    if !matches!(function.instruction(user).opcode, Opcode::Load { .. }) {
                        continue;
                    }
                    let block = function.parent(user).expect("a placed load");
                    let before = |stores: &[InstId]| function.block(block).instructions().iter().take_while(|&&one| one != user).any(|one| stores.contains(one));
                    let whole_first = before(&whole) || after_whole.get(&block).copied().unwrap_or(false);
                    let part_first = before(&parts) || after_part.contains(&block);
                    // A load of all of it needs all of it stored: bytes a constant-offset store
                    // leaves out (a tag stored, its payload and padding not) are read unstored,
                    // whatever the language says of them; a store at an offset not known could
                    // be any of them and is taken to cover the rest.
                    let uncovered = layout.as_ref().is_some_and(|layout| {
                        let size = layout.alloc_size(&context.types, allocated);
                        let reads_all = matches!(crate::valuetracking::underlying(context, layout, function, function.instruction(user).operands[0]), (Operand::Value(base), Some(0)) if base == slot) && u64::from(layout.store_size(&context.types, function.instruction(user).ty)) >= size;
                        reads_all && !whole_first && !stores_cover(context, layout, function, slot, &parts, size)
                    });
                    if uncovered || (!whole_first && !part_first) {
                        let name = function.value(slot).name.clone().map_or_else(|| "an alloca".to_owned(), |one| format!("%{one}"));
                        out.push(format!("load uses {name} before it is stored"));
                    }
                }
            }
        }
    }
    out
}

/// Whether `stores`, into `slot` of `size` bytes, cover all of it: their constant ranges
/// together do, or one of them is at an offset not known.
fn stores_cover(context: &Context, layout: &DataLayout, function: &Function, slot: crate::module::ValueId, stores: &[InstId], size: u64) -> bool {
    let mut covered = vec![false; size as usize];
    for &store in stores {
        let instruction = function.instruction(store);
        let (base, offset) = crate::valuetracking::underlying(context, layout, function, instruction.operands[1]);
        if base != Operand::Value(slot) {
            continue;
        }
        let Some(offset) = offset else { return true };
        let Some(ty) = function.operand_type(context, instruction.operands[0]) else { continue };
        let bytes = u64::from(layout.store_size(&context.types, ty));
        for at in offset.max(0) as u64..(offset.max(0) as u64 + bytes).min(size) {
            covered[at as usize] = true;
        }
    }
    covered.iter().all(|&one| one)
}

/// The blocks every path to which, from the entry, has run one of `stores` by the
/// time it enters them: the greatest fixed point from "none at the entry, all elsewhere".
fn stored_on_every_path(function: &Function, stores: &[InstId]) -> HashMap<BlockId, bool> {
    let entry = function.entry();
    let mut at_entry: HashMap<BlockId, bool> = function.layout().iter().map(|&block| (block, Some(block) != entry)).collect();
    let stores_in = |block| function.block(block).instructions().iter().any(|one| stores.contains(one));
    let mut changed = true;
    while changed {
        changed = false;
        for &block in function.layout() {
            if Some(block) == entry || !at_entry[&block] {
                continue;
            }
            let predecessors = function.predecessors(block);
            if predecessors.is_empty() || !predecessors.iter().all(|&one| at_entry.get(&one).copied().unwrap_or(false) || stores_in(one)) {
                at_entry.insert(block, false);
                changed = true;
            }
        }
    }
    at_entry
}

/// The blocks a block holding one of `stores` can reach, itself only round a loop:
/// where something may have been stored before.
fn may_follow(function: &Function, stores: &[InstId]) -> HashSet<BlockId> {
    let mut reached = HashSet::default();
    let mut work: Vec<_> = function.layout().iter().copied().filter(|&block| function.block(block).instructions().iter().any(|one| stores.contains(one))).flat_map(|block| function.successors(block)).collect();
    while let Some(block) = work.pop() {
        if reached.insert(block) {
            work.extend(function.successors(block));
        }
    }
    reached
}

/// Whether the call `inst` is `llvm.memset`.
fn is_memset(module: &Module, function: &Function, inst: InstId) -> bool {
    let Some(&Operand::Constant(callee)) = function.instruction(inst).operands.last() else { return false };
    let ConstantKind::Global(callee) = module.context.get(callee).kind else { return false };
    module.global(callee).name.as_deref().and_then(Intrinsic::named) == Some(Intrinsic::MemSet)
}

/// Whether `at` is a store's pointer operand, storing a whole `ty`.
fn is_store_of(function: &Function, context: &Context, at: Use, ty: crate::types::TypeId) -> bool {
    let instruction = function.instruction(at.user);
    at.index == 1 && matches!(instruction.opcode, Opcode::Store { .. }) && function.operand_type(context, instruction.operands[0]) == Some(ty)
}

/// Whether `at` is a memset's destination, setting every byte of a `ty`.
fn is_fill_of(module: &Module, layout: Option<&DataLayout>, function: &Function, at: Use, ty: crate::types::TypeId) -> bool {
    let instruction = function.instruction(at.user);
    let (Opcode::Call(_), Some(layout)) = (&instruction.opcode, layout) else { return false };
    let Some(&Operand::Constant(callee)) = instruction.operands.last() else { return false };
    let ConstantKind::Global(callee) = module.context.get(callee).kind else { return false };
    let kind = module.global(callee).name.as_deref().and_then(Intrinsic::named);
    let memset = matches!(kind, Some(Intrinsic::MemSet | Intrinsic::MemSetPattern));
    let cell = match (kind, instruction.operands.get(1).and_then(|&value| function.operand_type(&module.context, value))) {
        (Some(Intrinsic::MemSetPattern), Some(ty)) => u128::from(layout.alloc_size(&module.context.types, ty)),
        _ => 1,
    };
    let length = match instruction.operands.get(2) {
        Some(&Operand::Constant(id)) => match module.context.get(id).kind {
            ConstantKind::Int(bits) => bits,
            _ => return false,
        },
        _ => return false,
    };
    memset && at.index == 0 && length * cell >= u128::from(layout.alloc_size(&module.context.types, ty))
}
