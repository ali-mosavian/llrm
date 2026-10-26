//! Finds what the two sides of a direct call make certain: constant call
//! actuals, constant returns, purity and termination. Intra-procedural passes
//! cannot see across a call, so these facts are written into the bodies for
//! them to fold.
//!
//! # How it works
//! A direct call is opaque to ordinary SCCP even when every return of the named
//! body has the same value. These functions compute that over a module (or,
//! with `program_*`, a whole program) and write it into the body, where the
//! ordinary pipeline folds the consumers.
//!
//! Arguments. `current_call_constants` lists, per call or `invoke` with a known
//! callee, the integer-constant actuals (what `ipacp` clones for).
//!
//! Returns. `constant_returns` finds, for each exactly defined body, the one
//! integer every `ret` returns. `propagate_returns` replaces uses of a direct
//! call's result by it; the call stays, and only a separate purity proof erases
//! it.
//!
//! ```text
//!  caller:  %r = call @g(i32 5)        g(%x): ... ret i32 7
//!                                      (every call passes 5, every ret 7)
//!  after:   %r = call @g(i32 5)        g(%x): uses of %x become 5
//!           uses of %r become 7
//! ```
//!
//! Purity and termination, as stated by attributes the whole-module step
//! stamped on each body. `stated_pure` is the exactly defined bodies that state
//! no memory effect, `willreturn` and `nounwind`. `erasable` asks the same of
//! one call (LLVM's `wouldInstructionBeTriviallyDead`), and
//! `remove_dead_pure_calls` erases the calls with no used result that are
//! erasable.
//!
//! Noreturn. `noreturn_procedures` and `program_noreturn` are
//! [`crate::noreturn`]'s greatest fixed point over the eligible bodies;
//! `terminal_calls` applies its cut after the calls to them.
//!
//! A body is exactly defined when its linkage says the definition here is the
//! one that runs (LLVM's `hasExactDefinition`).
//!
//! # Example
//! `constant_returns` over a module of two private bodies:
//!
//! ```mir
//! define internal i16 @yes() {
//! b1:
//!   ret i16 37
//! }
//!
//! define internal i16 @no(i1 %c) {
//! b1:
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   ret i16 37
//!
//! b3:
//!   ret i16 38
//! }
//! ```
//!
//! | body   | constant return                |
//! |--------|--------------------------------|
//! | `@yes` | 37                             |
//! | `@no`  | none (its two `ret`s disagree) |
//!
//! From unit test `test_module_constant_returns_require_every_exit_to_agree` in
//! `interprocedural_tests.rs`.
//!
//! # Provides
//! The functions above, and the re-export of `noreturn::terminal_sites`.
//! Consumers: the `interprocedural` program pass and `ipacp` in
//! llrm-transforms.
//!
//! # Needs
//! [`crate::effects`] (`callee`, `states`, `writes_memory`), [`crate::cfg`],
//! [`crate::noreturn`], [`crate::frameescape`], [`crate::pointerfacts`].
//!
//! # See also
//! LLVM's IPSCCP.

use std::collections::BTreeSet;

use llrm_mir::callgraph::Defined;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::memory;
use llrm_mir::module::{Function, InstId, Linkage, Module, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::program::Program;
use llrm_mir::types::Type;
use llrm_mir::{ConstantId, ConstantKind, Context, GlobalId};
use llrm_support::hash::IndexMap;

use crate::effects::{self, Declarations};
use crate::noreturn;
pub use crate::noreturn::terminal_sites;

pub type Returns = IndexMap<GlobalId, ConstantId>;

/// `constant_returns` of each module, as it names the bodies: its own, and
/// those another module defines, their constants imported.
pub fn program_returns(program: &mut Program) -> Vec<Returns> {
    let own: Vec<Returns> = program.modules.iter().map(constant_returns).collect();
    let mut out = own.clone();
    for at in 0..program.modules.len() {
        let declared: Vec<(GlobalId, Defined)> = (0..program.modules[at].globals.len() as u32)
            .map(GlobalId)
            .filter_map(|id| Some((id, program.definition(at, id).filter(|&(there, _)| there != at)?)))
            .collect();
        for (id, (there, defined)) in declared {
            if let Some(constant) = own[there].get(&defined).and_then(|&constant| program.imported(there, constant, at))
            {
                out[at].insert(id, constant);
            }
        }
    }
    out
}

/// The constant actuals of every direct call.
///
/// The result is deliberately per-call instead of per-callee: a costed
/// inlining decision may use one constant call even when a second dynamic
/// call prevents whole-body parameter specialization.
pub fn current_call_constants(
    context: &Context,
    function: &Function,
) -> IndexMap<InstId, Vec<Option<ConstantId>>> {
    let mut out = IndexMap::default();
    for (_, call) in function.walk() {
        let instruction = function.instruction(call);
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &instruction.opcode else { continue };
        if effects::callee(context, function, call).is_none() {
            continue;
        }
        let Type::Function { parameters, .. } = context.types.get(info.function_type) else { continue };
        let values = instruction.operands[..parameters.len()]
            .iter()
            .map(|&operand| _constant_argument(context, operand))
            .collect();
        out.insert(call, values);
    }
    out
}

fn _constant_argument(
    context: &Context,
    argument: Operand,
) -> Option<ConstantId> {
    match argument {
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_)) => Some(id),
        _ => None,
    }
}

/// The common integer produced by every return of each body.
///
/// Absence is the conservative answer for void, floating or disagreeing
/// returns. SCCP's fixed point is in the operands, so promoted locals, phis
/// and folded expressions need no special cases here.
pub fn constant_returns(module: &Module) -> Returns {
    let mut out = Returns::default();
    for (name, _, function) in
        module.functions().filter(|(_, global, function)| _exact(global.linkage) && !llrm_mir::facts::opaque(function))
    {
        let mut returned = Vec::new();
        let mut complete = true;
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            if instruction.opcode != Opcode::Ret {
                continue;
            }
            let Some(&operand) = instruction.operands.first() else { continue };
            match _constant_argument(&module.context, operand) {
                Some(constant) => returned.push(constant),
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if complete && !returned.is_empty() && returned.iter().collect::<BTreeSet<_>>().len() == 1 {
            out.insert(name, returned[0]);
        }
    }
    out
}

/// Define a direct call's known result from its module-level summary.
///
/// Every use of the call's result reads the constant instead, allowing the
/// ordinary body pipeline to fold consumers; the call stays.
pub fn propagate_returns(
    context: &Context,
    function: &mut Function,
    returns: &Returns,
) -> bool {
    let mut known: Vec<(ValueId, ConstantId)> = Vec::new();
    for (_, inst) in function.walk() {
        let Some(result) = function.instruction(inst).result else { continue };
        if let Some(&constant) = effects::callee(context, function, inst).and_then(|target| returns.get(&target))
            && context.get(constant).ty == function.value(result).ty
            && !function.users(result).is_empty()
        {
            known.push((result, constant));
        }
    }
    for &(result, constant) in &known {
        function.replace_all_uses_with(result, Operand::Constant(constant));
    }
    !known.is_empty()
}

/// Whether the body here is the one that runs: LLVM's `hasExactDefinition`.
/// The linker may swap any other for a different one.
fn _exact(linkage: Linkage) -> bool {
    matches!(
        linkage,
        Linkage::External | Linkage::Internal | Linkage::Private
    )
}

/// Each exactly defined function, for the fixed points.
fn _bodies(module: &Module) -> Vec<(GlobalId, &Function)> {
    module
        .functions()
        .filter(|(_, global, function)| _exact(global.linkage) && !function.is_declaration())
        .map(|(id, _, function)| (id, function))
        .collect()
}

/// The pointer operand of a load or store, and whether it is volatile.
fn _access(
    function: &Function,
    inst: InstId,
) -> Option<(Operand, bool, bool)> {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile, .. } => Some((instruction.operands[0], volatile, false)),
        Opcode::Store { volatile, .. } => Some((instruction.operands[1], volatile, true)),
        _ => None,
    }
}

/// Exactly defined bodies whose attributes state they touch no memory and
/// always come back normally.
pub fn stated_pure(module: &Module) -> BTreeSet<GlobalId> {
    _bodies(module)
        .into_iter()
        .filter(|(_, function)| {
            let attrs = &function.attrs;
            memory::stated(attrs) == memory::Effects::NONE
                && Facts::of(attrs).will_return()
                && Facts::of(attrs).no_unwind()
        })
        .map(|(id, _)| id)
        .collect()
}

/// Whether the call `inst` could go unnoticed: it states it writes no
/// memory and always comes back normally, as LLVM's
/// `wouldInstructionBeTriviallyDead` asks.
pub fn erasable(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    !effects::writes_memory(context, declarations, function, inst)
        && effects::states(context, declarations, function, inst, Fact::WillReturn)
        && effects::states(context, declarations, function, inst, Fact::NoUnwind)
}

/// Direct private procedures that cannot reach a normal return: noreturn's
/// fixed point over the `eligible` bodies.  An unknown, external or public
/// callee stays a returning edge.
/// `noreturn_procedures` over a program.
pub fn program_noreturn(
    program: &Program,
    declarations: &[&Declarations],
    eligible: &BTreeSet<Defined>,
) -> BTreeSet<Defined> {
    noreturn::inferred_in(program, declarations, eligible)
}

pub fn noreturn_procedures(
    module: &Module,
    declarations: &Declarations,
    eligible: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    noreturn::inferred(module, declarations, eligible)
}

/// Noreturn's terminal-call cut, at the direct calls to `noreturn` bodies.
pub fn terminal_calls(
    context: &Context,
    declarations: &Declarations,
    function: &mut Function,
    noreturn: &BTreeSet<GlobalId>,
) -> bool {
    let sites = terminal_sites(context, declarations, function, noreturn);
    noreturn::after_terminal_calls(context, function, &sites)
}

/// Remove the calls whose result nothing still reads and that could go
/// unnoticed (`erasable`).
pub fn remove_dead_pure_calls(
    context: &Context,
    declarations: &Declarations,
    function: &mut Function,
) -> bool {
    let removed: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            let instruction = function.instruction(inst);
            matches!(instruction.opcode, Opcode::Call(_))
                && erasable(context, declarations, function, inst)
                && instruction.result.is_none_or(|result| function.users(result).is_empty())
        })
        .collect();
    for &inst in &removed {
        function.erase(inst).expect("an unused call");
    }
    !removed.is_empty()
}

#[cfg(test)]
#[path = "interprocedural_tests.rs"]
mod tests;
