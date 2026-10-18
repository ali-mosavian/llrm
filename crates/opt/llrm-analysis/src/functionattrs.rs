//! `norecurse`, `nounwind`, `willreturn` and `noreturn` of one call-graph SCC,
//! as LLVM's `PostOrderFunctionAttrs` infers them from a body and the
//! attributes of its callees.
//!
//! # How it works
//! `infer` takes the members of an SCC, callees already stated, and answers
//! what each may be stamped with. It reads no analysis: a call is judged by
//! its site's attributes and its callee's declaration.
//!
//! - `norecurse` (`addNoRecurseAttrs`): a one-member SCC, every call of which
//!   goes to a function other than itself that states `norecurse` or is a
//!   `nocallback` declaration or an intrinsic (an indirect call with `!callees`
//!   counts each listed target, DEVIATE F2).
//! - `nounwind` (`inferAttrsFromFunctionBodies`): all or none of the SCC. No
//!   member may be a declaration or inexact; no instruction may throw (a call
//!   without `nounwind`, an `invoke`, `resume`) except a `call` to another
//!   member.
//! - `willreturn` (`addWillReturn`): `mustprogress` and no writes, or no back
//!   edge and every call states `willreturn`.
//! - `noreturn` (`addNoReturnAttrs`): no path from the entry reaches a `ret`
//!   without a call that states `noreturn` in its block.
//!
//! # Example
//! A function that calls a `norecurse` function and adds is itself
//! `norecurse`, `nounwind` and `willreturn`:
//!
//! ```mir
//! declare i32 @leaf(i32) norecurse nounwind willreturn
//!
//! define i32 @f(i32 %x) {
//! b0:
//!   %a = call i32 @leaf(i32 %x)
//!   %b = add i32 %a, 1
//!   ret i32 %b
//! }
//! ```
//!
//! From unit test `test_a_body_over_norecurse_callees_is_stamped_all_three` in
//! `functionattrs_tests.rs`.
//!
//! # Runs
//! `interprocedural::stamped` once per SCC, callees first.
//!
//! # See also
//! LLVM `FunctionAttrs.cpp` (`addNoRecurseAttrs`, `addNoReturnAttrs`,
//! `addWillReturn`, `inferAttrsFromFunctionBodies`), `CFG.cpp` (`canReturn`).

use std::collections::BTreeSet;

use llrm_mir::facts::{Fact, Facts};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Linkage, Module};
use llrm_mir::opcode::Opcode;
use llrm_mir::{BlockId, Context, GlobalId, memory};

use crate::effects::{self, Declarations};

/// What one member of an SCC may be stamped with.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Inferred {
    pub no_recurse: bool,
    pub no_unwind: bool,
    pub will_return: bool,
    pub no_return: bool,
}

/// `hasExactDefinition`: the body here is the one that runs.
fn exact(linkage: Linkage) -> bool {
    matches!(
        linkage,
        Linkage::External | Linkage::Internal | Linkage::Private
    )
}

fn calls(function: &Function) -> impl Iterator<Item = InstId> + '_ {
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        ))
}

/// `mayHaveRecursiveCallee` (FunctionAttrs.cpp:2030) with
/// `AnyFunctionsAddressIsTaken` true, its default.
fn may_have_recursive_callee(
    module: &Module,
    declarations: &Declarations,
    id: GlobalId,
    function: &Function,
) -> bool {
    calls(function).any(|inst| {
        // DEVIATE F2: an indirect call whose `!callees` lists its targets is as
        // recursive as they are; LLVM takes any indirect call to be.
        let Some(targets) = llrm_mir::callgraph::reached(module, function, inst) else { return true };
        targets
            .into_iter()
            .any(
                |callee| {
                    if callee == id {
                        return true;
                    }
                    let named = module.global(callee).name.as_deref().and_then(Intrinsic::named);
                    // DEVIATE F1: an intrinsic carries `norecurse` in LLVM's
                    // attribute tables; llrm's carry none, so the name says it.
                    if named.is_some_and(|one| !matches!(one, Intrinsic::Code | Intrinsic::Asm)) {
                        return false;
                    }
                    let Some(declared) = declarations.get(callee.0 as usize).and_then(|one| one.function()) else {
                        return true;
                    };
                    let facts = Facts::of(&declared.attrs);
                    !(facts.no_recurse() || (declared.is_declaration() && facts.no_callback()))
                },
            )
    })
}

/// `InstrBreaksNonThrowing` (FunctionAttrs.cpp:1873): the instruction may
/// throw, and is not a `call` to a member.
fn breaks_non_throwing(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
    members: &BTreeSet<GlobalId>,
) -> bool {
    match &function.instruction(inst).opcode {
        Opcode::Resume | Opcode::Invoke(_) => true,
        Opcode::Call(_) => {
            if effects::states(context, declarations, function, inst, Fact::NoUnwind) {
                return false;
            }
            !memory::callee(context, function, inst).is_some_and(|callee| members.contains(&callee))
        }
        _ => false,
    }
}

/// `FindFunctionBackedges` is empty: a depth-first walk from the entry meets no
/// block still on its stack.
fn has_back_edge(function: &Function) -> bool {
    let Some(entry) = function.entry() else { return false };
    let mut state = vec![0_u8; function.layout().iter().map(|one| one.0 as usize + 1).max().unwrap_or(0)];
    let mut stack: Vec<(BlockId, Vec<BlockId>)> = vec![(entry, function.successors(entry).iter().copied().collect())];
    state[entry.0 as usize] = 1;
    while let Some((block, pending)) = stack.last_mut() {
        match pending.pop() {
            Some(next) => match state[next.0 as usize] {
                1 => return true,
                0 => {
                    state[next.0 as usize] = 1;
                    let following = function.successors(next).iter().copied().collect();
                    stack.push((next, following));
                }
                _ => {}
            },
            None => {
                state[block.0 as usize] = 2;
                stack.pop();
            }
        }
    }
    false
}

/// `functionWillReturn` (FunctionAttrs.cpp:2141).
fn function_will_return(
    context: &Context,
    declarations: &Declarations,
    linkage: Linkage,
    function: &Function,
) -> bool {
    if !exact(linkage) {
        return false;
    }
    if Facts::of(&function.attrs).must_progress() && !memory::stated(&function.attrs).writes {
        return true;
    }
    if function.is_declaration() || has_back_edge(function) {
        return false;
    }
    calls(function).all(|inst| effects::states(context, declarations, function, inst, Fact::WillReturn))
}

/// `canReturn` (CFG.cpp:405): some block reachable from the entry ends in
/// `ret` and holds no call that states `noreturn`.
fn can_return(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
) -> bool {
    let Some(entry) = function.entry() else { return true };
    let mut seen = BTreeSet::from([entry]);
    let mut work = vec![entry];
    while let Some(block) = work.pop() {
        let ends_in_ret =
            function.terminator(block).is_some_and(|last| function.instruction(last).opcode == Opcode::Ret);
        if ends_in_ret
            && !function
                .block(block)
                .instructions()
                .iter()
                .any(|&inst| effects::states(context, declarations, function, inst, Fact::NoReturn))
        {
            return true;
        }
        for next in function.successors(block).iter().copied() {
            if seen.insert(next) {
                work.push(next);
            }
        }
    }
    false
}

/// What each member of one SCC may be stamped with. `declarations` carry the
/// attributes of everything below it, and of the members as they stand.
pub fn infer(
    module: &Module,
    declarations: &Declarations,
    members: &[GlobalId],
) -> Vec<(GlobalId, Inferred)> {
    let context = &module.context;
    let set: BTreeSet<GlobalId> = members.iter().copied().collect();
    let body = |id: GlobalId| {
        let global = module.global(id);
        global.function().filter(|one| !one.is_declaration()).map(|one| (global.linkage, one))
    };
    // All or none: one member that cannot be scanned, or one throwing
    // instruction, takes it from the SCC.
    let unwinds = members.iter().all(|&id| {
        let Some((linkage, function)) = body(id) else {
            return module.global(id).function().is_some_and(|one| Facts::of(&one.attrs).no_unwind());
        };
        if Facts::of(&function.attrs).no_unwind() {
            return true;
        }
        exact(linkage)
            && function.walk().all(|(_, inst)| !breaks_non_throwing(context, declarations, function, inst, &set))
    });
    let no_recurse_scc = members.len() == 1;
    members
        .iter()
        .map(|&id| {
            let Some((linkage, function)) = body(id) else { return (id, Inferred::default()) };
            let stamped = Facts::of(&function.attrs);
            let exact = exact(linkage);
            let inferred = Inferred {
                no_recurse: !stamped.no_recurse()
                    && exact
                    && no_recurse_scc
                    && !may_have_recursive_callee(module, declarations, id, function),
                no_unwind: !stamped.no_unwind() && unwinds,
                will_return: !stamped.will_return() && function_will_return(context, declarations, linkage, function),
                no_return: !stamped.no_return() && exact && !can_return(context, declarations, function),
            };
            (id, inferred)
        })
        .collect()
}

#[cfg(test)]
#[path = "functionattrs_tests.rs"]
mod tests;
