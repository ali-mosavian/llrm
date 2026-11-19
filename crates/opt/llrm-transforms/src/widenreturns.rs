//! Return-value widening: a function only its own module calls, whose every
//! caller extends the result to one wider type, returns the wider value
//! itself. The extension is made once where the function returns, not at each
//! call.
//!
//! # How it works
//! Only functions `direct_only` accepts are candidates (internal, never named
//! but as a callee, every call a plain call). One qualifies when it returns an
//! integer and every call reads the result only through casts of one kind
//! (`zext` or `sext`) to one type. Then each `ret` returns the value cast to
//! that type, the function's type follows, and each call returns the wider
//! type: its casts read the call's result and are gone. The return and every
//! call are marked `zeroext` (`signext`), the ABI's own statement that the
//! callee extended: a call lowering that reads the returned registers as they
//! stand (the `AssertZext`/`AssertSext` of `lowerRangeToAssertZExt` and
//! `getCopyFromParts` are skipped for an attribute that already says so) does
//! not rebuild the high part from a `range` it infers.
//!
//! # Example
//! ```text
//!  define internal i16 @f(i16 %n)        define internal i32 @f(i16 %n)
//!    ret i16 %v                  ==>       %w = zext i16 %v to i32
//!  %r = call i16 @f(i16 %a)                ret i32 %w
//!  %w = zext i16 %r to i32               %w = call i32 @f(i16 %a)
//! ```
//!
//! It pays where the calls outnumber the `ret`s: the extensions move from the
//! calls to the returns. A target decides whether a wide return is cheap
//! (`Tti::widen_private_returns`).
//!
//! # Not in the pin
//! Nothing in LLVM changes a function's return type to a wider one:
//! DeadArgumentElimination
//! (`DeadArgumentEliminationPass::removeDeadStuffFromFunction`) drops dead
//! returns and arguments, IPSCCP (`IPSCCP.cpp`) propagates constant returns and
//! ranges without retyping, ArgumentPromotion changes parameters only, and the
//! Attributor deduces `zeroext` and `range` of the return without removing the
//! caller's cast. GCC's nearest is IPA-SRA's return-value rewrite (`ipa-sra.
//! cc`, `ipa_param_adjustments`), which narrows or removes a return, never
//! widens it. `DEVIATE WIDEN-RETURNS`.
//!
//! # Runs
//! In the interprocedural step, after dead-argument elimination, on a target
//! that opts in. Each changed body goes through the function pipeline again.

use std::collections::BTreeSet;

use llrm_mir::callgraph::{DirectCalls, direct_calls, direct_only};
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, Module};
use llrm_mir::opcode::{Attribute, CastOp, Flags, Opcode, Tail};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::{GlobalId, InstId, Operand};

/// Whether the functions `widened` edits are any, without editing.
pub fn any_widenable(module: &Module) -> bool {
    !candidates(module).is_empty()
}

/// The functions of `module` that returned a wider value now, and their
/// callers.
pub fn widened(module: &mut Module) -> Vec<GlobalId> {
    let mut changed = BTreeSet::new();
    for (id, kind, wide, calls) in candidates(module) {
        // the callee's extension is the ABI's `zeroext`/`signext`: a caller
        // reads the returned registers as they stand, as it reads a
        // return so marked
        let extension = || vec![Attribute::Flag(if kind == CastOp::ZExt { "zeroext" } else { "signext" }.into())];
        {
            let Module { context, globals, .. } = &mut *module;
            let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
            let rets: Vec<InstId> = function
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| function.instruction(inst).opcode == Opcode::Ret)
                .collect();
            for ret in rets {
                let value = function.instruction(ret).operands[0];
                let cast = function.create_instruction(Opcode::Cast(kind), wide, vec![value], Flags::default(), None);
                function.insert(cast, Position::Before(ret)).expect("a cast before a ret");
                let extended = Operand::Value(function.instruction(cast).result.expect("a value"));
                function.set_operands(ret, vec![extended]);
            }
            function.set_return_type(context, wide);
            function.return_attrs = extension();
            changed.insert(id);
        }
        let ty = module.global(id).function().expect("a function").ty;
        for (caller, call) in calls {
            let GlobalKind::Function(function) = &mut module.globals[caller.0 as usize].kind else {
                unreachable!("a caller")
            };
            let result = function.instruction(call).result.expect("a result");
            let mut casts: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
            casts.sort();
            casts.dedup();
            function.retype_call_result(call, wide, ty, extension());
            for cast in casts {
                let extended = function.instruction(cast).result.expect("a value");
                function.replace_value(extended, Operand::Value(result));
                function.erase(cast).expect("a cast nothing reads");
            }
            changed.insert(caller);
        }
    }
    changed.into_iter().collect()
}

/// The functions to widen: the cast every caller makes, the type it makes, and
/// the calls.
fn candidates(module: &Module) -> Vec<(GlobalId, CastOp, TypeId, Vec<(GlobalId, InstId)>)> {
    let only = direct_only(module);
    let DirectCalls { sites, refused } = direct_calls(module);
    let mut found = Vec::new();
    for &id in only.iter().filter(|id| !refused.contains(id)) {
        let Some(calls) = sites.get(&id) else { continue };
        let function = module.global(id).function().expect("a function");
        let Type::Function { returns, .. } = module.context.types.get(function.ty) else { continue };
        let Some(narrow) = module.context.types.int_bits(*returns) else { continue };
        let rets = function.walk().filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Ret).count();
        if calls.len() <= rets {
            continue;
        }
        let Some((kind, wide)) = extension_at_every_call(module, calls) else { continue };
        if module.context.types.int_bits(wide).is_some_and(|bits| bits > narrow) {
            found.push((id, kind, wide, calls.clone()));
        }
    }
    found
}

/// The one cast every call's result is read through, and the type it makes.
fn extension_at_every_call(
    module: &Module,
    calls: &[(GlobalId, InstId)],
) -> Option<(CastOp, TypeId)> {
    let mut seen: Option<(CastOp, TypeId)> = None;
    for &(caller, call) in calls {
        let function = module.global(caller).function()?;
        let Opcode::Call(info) = &function.instruction(call).opcode else { return None };
        if info.tail == Tail::MustTail {
            return None;
        }
        let result = function.instruction(call).result?;
        let mut read = false;
        for one in function.users(result).iter() {
            let user = function.instruction(one.user);
            let Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) = user.opcode else { return None };
            if seen.is_some_and(|before| before != (kind, user.ty)) {
                return None;
            }
            seen = Some((kind, user.ty));
            read = true;
        }
        if !read {
            return None;
        }
    }
    seen
}
