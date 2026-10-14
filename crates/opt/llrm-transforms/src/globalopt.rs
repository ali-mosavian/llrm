//! Mark internal globals that are only ever loaded from as `constant`, and drop
//! the stores of those only ever stored to.
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
//! `WriteOnly`, after the pipeline has forwarded what loads it could, is the
//! other half (LLVM's GlobalOpt: "if the global is never loaded (but may be
//! stored to), it is dead"; gcc's ipa `write-only` flag): a variable that is
//! internal or private, not named by another global's initializer, and whose
//! every use in every function is a non-volatile store's address (through the
//! same `gep`s and casts) loses those stores. Nothing names it then, and
//! `globaldce` deletes it.
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
use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Module, Operand, ValueDef};
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
    only_used(context, function, global, &|op, index| matches!(op.opcode, Opcode::Load { .. }) && index == 0)
}

/// Whether every use of `global` in `function` is, through address arithmetic
/// and casts, the address `access` says a use is.
fn only_used(
    context: &Context,
    function: &Function,
    global: GlobalId,
    access: &dyn Fn(&llrm_mir::module::Instruction, usize) -> bool,
) -> bool {
    function.walk().all(|(_, inst)| {
        let op = function.instruction(inst);
        op.operands.iter().enumerate().all(|(index, &operand)| {
            let Operand::Constant(id) = operand else { return true };
            let mut named = std::collections::BTreeSet::new();
            embedded(context, id, &mut named);
            !named.contains(&global) || (addresses(context, id, global) && used_through(function, inst, index, access))
        })
    })
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

/// Whether operand `index` of `inst`, an address, is only used as `access`
/// says, through address arithmetic and casts.
fn used_through(
    function: &Function,
    inst: InstId,
    index: usize,
    access: &dyn Fn(&llrm_mir::module::Instruction, usize) -> bool,
) -> bool {
    let op = function.instruction(inst);
    match op.opcode {
        Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if index == 0 => {
            op.result.is_some_and(|value| {
                function.users(value).iter().all(|one| used_through(function, one.user, one.index as usize, access))
            })
        }
        _ => access(op, index),
    }
}

/// The stores of the globals nothing loads: removed from every body, which are
/// the returned.
pub struct WriteOnly;

impl ModulePass for WriteOnly {
    fn name(&self) -> &'static str {
        "writeonly"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let found = never_loaded(module);
        if found.is_empty() {
            return Vec::new();
        }
        let Module { context, globals, .. } = module;
        let mut changed = Vec::new();
        for (at, global) in globals.iter_mut().enumerate() {
            let GlobalKind::Function(function) = &mut global.kind else { continue };
            let dead: Vec<InstId> = function
                .walk()
                .filter(|&(_, inst)| {
                    let op = function.instruction(inst);
                    matches!(op.opcode, Opcode::Store { volatile: false, .. })
                        && op.operands.get(1).is_some_and(|&address| {
                            found.iter().any(|&global| address_of(context, function, address, global))
                        })
                })
                .map(|(_, inst)| inst)
                .collect();
            let dead = with_orphaned_addresses(context, function, &found, dead);
            if !dead.is_empty() && function.erase_all(&dead).is_ok() {
                changed.push(GlobalId(at as u32));
            }
        }
        changed
    }
}

/// `dead` and the address arithmetic of `found` globals that nothing uses once
/// they are gone, whether they were the stores' or were left unused before:
/// what still names a global would keep it.
fn with_orphaned_addresses(
    context: &Context,
    function: &Function,
    found: &[GlobalId],
    mut dead: Vec<InstId>,
) -> Vec<InstId> {
    loop {
        let more: Vec<InstId> = function
            .walk()
            .map(|(_, inst)| inst)
            .filter(|inst| !dead.contains(inst))
            .filter(|&inst| {
                let op = function.instruction(inst);
                matches!(
                    op.opcode,
                    Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                ) && op.result.is_some_and(|value| function.users(value).iter().all(|one| dead.contains(&one.user)))
                    && found.iter().any(|&global| {
                        op.operands.first().is_some_and(|&base| address_of(context, function, base, global))
                    })
            })
            .collect();
        if more.is_empty() {
            return dead;
        }
        dead.extend(more);
    }
}

/// The internal variables no body loads from, and whose every use is a
/// non-volatile store's address.
pub fn never_loaded(module: &Module) -> Vec<GlobalId> {
    let mut held = std::collections::BTreeSet::new();
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut held));
        }
    }
    let stores = |op: &llrm_mir::module::Instruction, index: usize| {
        matches!(op.opcode, Opcode::Store { volatile: false, .. }) && index == 1
    };
    let candidates = module
        .globals
        .iter()
        .enumerate()
        .filter_map(
            |(at, global)| {
                let GlobalKind::Variable(variable) = &global.kind else { return None };
                let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
                (internal && !variable.constant && !held.contains(&GlobalId(at as u32))).then_some(GlobalId(at as u32))
            },
        );
    candidates
        .filter(|&id| module.functions().all(|(_, _, function)| only_used(&module.context, function, id, &stores)))
        .collect()
}

/// Whether `address`, an operand, is `global`'s address, moved or cast: a
/// constant, or a value made of one by `gep`s and casts.
fn address_of(
    context: &Context,
    function: &Function,
    address: Operand,
    global: GlobalId,
) -> bool {
    match address {
        Operand::Constant(id) => addresses(context, id, global),
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => {
                let op = function.instruction(def);
                matches!(
                    op.opcode,
                    Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                ) && op.operands.first().is_some_and(|&base| address_of(context, function, base, global))
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
#[path = "globalopt_tests.rs"]
mod tests;
