//! What a frontend's MIR must not hold before any pass can exploit it:
//! poison the source language defines a value for. A raise may mark its
//! language's promise that an access stays in its object -- `inbounds` on
//! a GEP -- and no other flag; it holds no `poison` constant, and uses no
//! alloca before a store of its whole type.

use crate::context::{ConstantKind, Context};
use crate::datalayout::DataLayout;
use crate::dominators::DominatorTree;
use crate::intrinsics::Intrinsic;
use crate::module::{Function, GlobalKind, Module, Operand, Use};
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
    let tree = DominatorTree::new(function);
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let mnemonic = instruction.opcode.mnemonic();
        if instruction.operands.iter().any(|&one| matches!(one, Operand::Constant(id) if holds_poison(context, id))) {
            out.push(format!("{mnemonic} uses poison"));
        }
        let gep = matches!(instruction.opcode, Opcode::GetElementPtr { .. });
        for word in instruction.flags.words().into_iter().filter(|&word| !(gep && word == "inbounds")) {
            out.push(format!("{mnemonic} carries {word}"));
        }
        if let Opcode::Alloca { allocated, .. } = instruction.opcode {
            let slot = instruction.result.expect("an alloca's pointer");
            let initializers: Vec<_> = function
                .users(slot)
                .iter()
                .filter(|one| is_store_of(function, context, **one, allocated) || is_fill_of(module, layout.as_ref(), function, **one, allocated))
                .map(|one| one.user)
                .collect();
            for &Use { user, .. } in function.users(slot) {
                if initializers.contains(&user) {
                    continue;
                }
                if !initializers.iter().any(|&store| store != user && tree.instruction_dominates(function, store, user)) {
                    let name = function.value(slot).name.clone().map_or_else(|| "an alloca".to_owned(), |one| format!("%{one}"));
                    out.push(format!("{} uses {name} before it is stored", function.instruction(user).opcode.mnemonic()));
                }
            }
        }
    }
    out
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
    let memset = module.global(callee).name.as_deref().and_then(Intrinsic::named) == Some(Intrinsic::MemSet);
    let length = match instruction.operands.get(2) {
        Some(&Operand::Constant(id)) => match module.context.get(id).kind {
            ConstantKind::Int(bits) => bits,
            _ => return false,
        },
        _ => return false,
    };
    memset && at.index == 0 && length >= u128::from(layout.alloc_size(&module.context.types, ty))
}
