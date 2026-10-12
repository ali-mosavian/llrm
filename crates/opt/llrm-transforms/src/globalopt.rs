//! Mark internal globals that are only ever loaded from as `constant`.
//!
//! # How it works
//! A variable qualifies when it is internal or private, initialized, not yet
//! `constant`, and not named by any other global's initializer. Every use of it
//! in every function must then be a load's address, reached through constant
//! `gep`s and `bitcast`/`addrspacecast`s of its address, and through the same
//! instructions at run time as long as all their users load. Any other use (a
//! store, a call argument, a compare, an escape into a value) disqualifies it.
//!
//! A qualifying variable gets `constant = true`. A load of it is then its
//! initializer's bytes (`memory::constant_bits`), which the body passes fold.
//! Every defined function is reported as changed so each body is run through
//! its pipeline again.
//!
//! # Example
//! Before:
//!
//! ```mir
//! @c = internal global [2 x i16] [i16 5, i16 12]
//! @w = internal global i16 7
//!
//! define i16 @f(i16 %x) {
//! b0:
//!   %at = getelementptr i8, ptr @c, i16 2
//!   %a = load i16, ptr %at
//!   %b = load i16, ptr @w
//!   %s = add i16 %a, %b
//!   ret i16 %s
//! }
//!
//! define void @g(i16 %x) {
//! b0:
//!   store i16 %x, ptr @w
//!   ret void
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! @c = internal constant [2 x i16] [i16 5, i16 12]
//! @w = internal global i16 7
//! ; ... @f and @g unchanged
//! ```
//!
//! `@c` is only loaded and becomes `constant`; `@w` is stored by `@g` and stays
//! a variable. The body passes then fold the load of `@c` to 12.
//!
//! From the fixture of unit test `a_global_only_loaded_is_its_initializer` in
//! `globalopt_tests.rs`, run through `GlobalOpt` alone.
//!
//! # Runs
//! Once, as a module pass before the per-body pipeline and before the
//! interprocedural step. Not run at -O0.
//!
//! # See also
//! LLVM's GlobalOpt, which also marks globals it finds never stored.

use llrm_analysis::globalsaa::embedded;
use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

pub struct GlobalOpt;

impl ModulePass for GlobalOpt {
    fn name(&self) -> &'static str {
        "globalopt"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let found = never_stored(module);
        for &id in &found {
            if let GlobalKind::Variable(variable) = &mut module.globals[id.0 as usize].kind {
                variable.constant = true;
            }
        }
        if found.is_empty() {
            Vec::new()
        } else {
            module.functions().filter(|(_, _, one)| !one.is_declaration()).map(|(id, _, _)| id).collect()
        }
    }
}

/// The internal, initialized, not yet constant globals every use of which,
/// through address arithmetic and casts, is a load's address.
pub fn never_stored(module: &Module) -> Vec<GlobalId> {
    let mut held = std::collections::BTreeSet::new();
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut held));
        }
    }
    let candidates = module.globals.iter().enumerate().filter_map(|(at, global)| {
        let GlobalKind::Variable(variable) = &global.kind else { return None };
        let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
        (internal && !variable.constant && variable.initializer.is_some() && !held.contains(&GlobalId(at as u32)))
            .then_some(GlobalId(at as u32))
    });
    candidates
        .filter(|&id| module.functions().all(|(_, _, function)| only_loaded(&module.context, function, id)))
        .collect()
}

/// Whether every use of `global` in `function` only loads through it.
fn only_loaded(
    context: &Context,
    function: &Function,
    global: GlobalId,
) -> bool {
    function
        .walk()
        .all(
            |(_, inst)| {
                let op = function.instruction(inst);
                op.operands.iter().enumerate().all(|(index, &operand)| {
                    let Operand::Constant(id) = operand else { return true };
                    let mut named = std::collections::BTreeSet::new();
                    embedded(context, id, &mut named);
                    !named.contains(&global) || (addresses(context, id, global) && loads_through(function, inst, index))
                })
            },
        )
}

/// Whether `constant` is `global`'s address, moved or cast.
fn addresses(
    context: &Context,
    constant: ConstantId,
    global: GlobalId,
) -> bool {
    match &context.get(constant).kind {
        ConstantKind::Global(one) => *one == global,
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
            operands.first().is_some_and(|&base| addresses(context, base, global))
        }
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
            addresses(context, *value, global)
        }
        _ => false,
    }
}

/// Whether operand `index` of `inst`, an address, is only loaded from.
fn loads_through(
    function: &Function,
    inst: InstId,
    index: usize,
) -> bool {
    let op = function.instruction(inst);
    match op.opcode {
        Opcode::Load { .. } => index == 0,
        Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if index == 0 => {
            op.result.is_some_and(|value| {
                function.users(value).iter().all(|one| loads_through(function, one.user, one.index as usize))
            })
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "globalopt_tests.rs"]
mod tests;
