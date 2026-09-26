//! Constant arguments across calls, as LLVM's IPSCCP finds them: where
//! every call to an internal function passes one constant for a
//! parameter, and nothing else takes the function's address, the parameter
//! is that constant.

use crate::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use crate::memory::callee;
use crate::module::{GlobalKind, Linkage, Module, Operand};
use crate::passes::ModulePass;

pub struct Ipsccp;

impl ModulePass for Ipsccp {
    fn name(&self) -> &'static str {
        "ipsccp"
    }

    fn run(&mut self, module: &mut Module) -> Vec<GlobalId> {
        let mut changed = Vec::new();
        for (at, global) in module.globals.iter().enumerate() {
            let id = GlobalId(at as u32);
            let GlobalKind::Function(function) = &global.kind else { continue };
            if global.linkage != Linkage::Internal || function.is_declaration() {
                continue;
            }
            let Some(sites) = sites(module, id) else { continue };
            let count = function.parameters().len();
            let known: Vec<Option<Operand>> = (0..count)
                .map(|index| {
                    let first = *sites.first()?.get(index)?;
                    (matches!(first, Operand::Constant(_)) && sites.iter().all(|one| one.get(index) == Some(&first))).then_some(first)
                })
                .collect();
            if known.iter().any(Option::is_some) {
                changed.push((id, known));
            }
        }
        let mut out = Vec::new();
        for (id, known) in changed {
            let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { unreachable!("a function") };
            let parameters = function.parameters().to_vec();
            let mut replaced = false;
            for (parameter, constant) in parameters.into_iter().zip(known) {
                if let Some(constant) = constant.filter(|_| !function.users(parameter).is_empty()) {
                    function.replace_all_uses_with(parameter, constant);
                    replaced = true;
                }
            }
            if replaced {
                out.push(id);
            }
        }
        out
    }
}

/// Every call's arguments to `id`, if calls are all that name it.
fn sites(module: &Module, id: GlobalId) -> Option<Vec<Vec<Operand>>> {
    let context = &module.context;
    let mut sites = Vec::new();
    for (_, _, function) in module.functions() {
        for (_, inst) in function.walk() {
            let operands = &function.instruction(inst).operands;
            let called = callee(context, function, inst) == Some(id);
            for (index, &operand) in operands.iter().enumerate() {
                let Operand::Constant(constant) = operand else { continue };
                let as_callee = called && index + 1 == operands.len();
                if !as_callee && mentions(context, constant, id) {
                    return None;
                }
            }
            if called {
                sites.push(operands[..operands.len() - 1].to_vec());
            }
        }
    }
    let initialized = module.globals.iter().filter_map(|one| match &one.kind {
        GlobalKind::Variable(variable) => variable.initializer,
        GlobalKind::Function(_) => None,
    });
    for initializer in initialized {
        if mentions(context, initializer, id) {
            return None;
        }
    }
    Some(sites)
}

/// Whether `constant` names the global `id`, however deep.
fn mentions(context: &Context, constant: ConstantId, id: GlobalId) -> bool {
    match &context.get(constant).kind {
        ConstantKind::Global(global) => *global == id,
        ConstantKind::Aggregate(members) => members.iter().any(|&one| mentions(context, one, id)),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => operands.iter().any(|&one| mentions(context, one, id)),
        ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => mentions(context, *value, id),
        _ => false,
    }
}
