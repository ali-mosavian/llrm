//! CodeGenPrepare's `dupRetToEnableTailCallOpts`: a return block that only
//! returns what a tail call produced is copied into the predecessors that end
//! in that call
//! ([CodeGenPrepare.cpp#L3014](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L3014)).
//!
//! # How it works
//! A block holding a phi, an optional `bitcast` and `extractvalue` of it, and
//! the `ret` (nothing else), or a `ret` alone, is a candidate. A predecessor
//! counts when it ends in `call; br ret-block` and the call is a `tail` call
//! whose return attributes the caller's permit (`attributesPermitTailCall`),
//! and either produces the phi's arm, or (a `ret void`, or a returned first
//! argument of `memcpy`, `memmove`, `memset`, `strcpy`, `strncpy`, `strcat`,
//! `strncat`) leaves its result unused. Each such predecessor gets its own
//! `ret` in place of the branch (`FoldReturnIntoUncondBranch`); the block goes
//! when nothing reaches it.
//!
//! ```text
//!  b1: tail call void @llvm.memcpy(..)      b1: tail call void @llvm.memcpy(..)
//!      br label %ret                            ret void
//!  b0: br i1 %c, label %ret, ..             b0: br i1 %c, label %ret, ..
//!  ret: ret void                            ret: ret void
//! ```
//!
//! Why it matters here: two returns give shrink wrapping two exits; one shared
//! return lets the pin's `ShrinkWrap` find no restore point where ours found
//! one (`x_mergesort` `msort`, -Os m32 +9 B).
//!
//! DEVIATE RD1: `mayBeEmittedAsTailCall` is `CI->isTailCall()` (x86's), read
//! from the call's marker; no bitcast of a pointer is looked through (MIR's
//! pointers are opaque), and there are no fake uses or pseudo probes.
//!
//! # Example
//! Before:
//!
//! ```mir
//! b1:
//!   tail call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 8, i1 false)
//!   br label %ret
//!
//! b0:
//!   br i1 %c, label %ret, label %b1
//!
//! ret:
//!   ret void
//! ```
//!
//! After:
//!
//! ```mir
//! b1:
//!   tail call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 8, i1 false)
//!   ret void
//!
//! b0:
//!   br i1 %c, label %ret, label %b1
//!
//! ret:
//!   ret void
//! ```
//!
//! # Runs
//! In `cgpaddr`'s run, after the empty blocks go.

use llrm_mir::context::{ConstantKind, Context, GlobalId};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::memory;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand};
use llrm_mir::opcode::{Attribute, CastOp, Opcode, Tail};

use crate::lcssa::arms;
use crate::tailcallelim::fold_return_into_uncond_branch;

/// Return attributes that cannot change how a call returns
/// (`attributesPermitTailCall`'s first loop).
const BENIGN: [&str; 8] =
    ["align", "dereferenceable", "dereferenceable_or_null", "noalias", "nonnull", "noundef", "range", "nofpclass"];

fn name_of(attribute: &Attribute) -> &str {
    match attribute {
        Attribute::Flag(name) | Attribute::Int(name, _) | Attribute::Type(name, _) | Attribute::Str(name, _) => name,
        Attribute::Range { .. } => "range",
        _ => "",
    }
}

/// `attributesPermitTailCall` for a call whose result is `used` or not.
fn attributes_permit_tail_call(
    caller: &[Attribute],
    callee: &[Attribute],
    used: bool,
) -> bool {
    let kept = |attrs: &[Attribute]| -> Vec<Attribute> {
        attrs.iter().filter(|one| !BENIGN.contains(&name_of(one))).cloned().collect()
    };
    let (mut caller, mut callee) = (kept(caller), kept(callee));
    let has = |attrs: &[Attribute], name: &str| attrs.iter().any(|one| name_of(one) == name);
    for extension in ["zeroext", "signext"] {
        if has(&caller, extension) {
            if !has(&callee, extension) {
                return false;
            }
            caller.retain(|one| name_of(one) != extension);
            callee.retain(|one| name_of(one) != extension);
            break;
        }
    }
    if !used {
        callee.retain(|one| !matches!(name_of(one), "zeroext" | "signext"));
    }
    caller == callee
}

/// `isIntrinsicOrLFToBeTailCalled`: a call whose result is its first argument.
fn returns_first_argument(
    globals: &[GlobalValue],
    callee: GlobalId,
) -> bool {
    let Some(global) = globals.get(callee.0 as usize) else { return false };
    if let Some(intrinsic) = global.intrinsic() {
        return matches!(
            intrinsic,
            Intrinsic::MemCpy | Intrinsic::MemMove | Intrinsic::MemSet
        );
    }
    matches!(
        global.name.as_deref(),
        Some("strcpy" | "strncpy" | "strcat" | "strncat")
    )
}

fn used(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|value| function.users(value).into_iter().next().is_some())
}

fn one_use(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|value| function.users(value).into_iter().count() == 1)
}

fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(inst) => Some(inst),
            llrm_mir::module::ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

/// `dupRetToEnableTailCallOpts` over every return block of `function`.
/// Whether a block was copied.
pub fn returns_duplicated(
    context: &Context,
    globals: &[GlobalValue],
    function: &mut Function,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        changed |= block_duplicated(context, globals, function, block);
    }
    changed
}

fn block_duplicated(
    context: &Context,
    globals: &[GlobalValue],
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(ret) = function.terminator(block).filter(|&one| function.instruction(one).opcode == Opcode::Ret) else {
        return false;
    };
    let value = function.instruction(ret).operands.first().copied();
    let (mut cast, mut extract, mut phi) = (None, None, None);
    let mut seen = value;
    if let Some(operand) = seen {
        if let Some(one) = defining(function, operand)
            .filter(|&one| matches!(
                function.instruction(one).opcode,
                Opcode::Cast(CastOp::BitCast)
            ))
        {
            cast = Some(one);
            seen = Some(function.instruction(one).operands[0]);
        }
        if let Some(one) = seen
            .and_then(|operand| defining(function, operand))
            .filter(|&one| matches!(function.instruction(one).opcode, Opcode::ExtractValue(_)))
        {
            let Opcode::ExtractValue(indices) = &function.instruction(one).opcode else { return false };
            if !indices.iter().all(|&index| index == 0) {
                return false;
            }
            extract = Some(one);
            seen = Some(function.instruction(one).operands[0]);
        }
        phi = seen
            .and_then(|operand| defining(function, operand))
            .filter(|&one| function.instruction(one).opcode == Opcode::Phi);
    }
    if phi.is_some_and(|one| function.parent(one) != Some(block)) {
        return false;
    }
    // Nothing between the phis and the return but the cast and the extract.
    let between = function.iter_block(block).find(|&inst| function.instruction(inst).opcode != Opcode::Phi);
    let mut at = between;
    while let Some(inst) = at {
        if Some(inst) == cast || Some(inst) == extract {
            at = function.next_in_block(inst);
        } else {
            break;
        }
    }
    if at != Some(ret) {
        return false;
    }
    let caller = function.return_attrs.clone();
    let permitted = |call: InstId| {
        let Opcode::Call(info) = &function.instruction(call).opcode else { return false };
        matches!(info.tail, Tail::Tail | Tail::MustTail)
            && attributes_permit_tail_call(&caller, &info.return_attrs, used(function, call))
    };
    let mut tail_calls: Vec<BlockId> = Vec::new();
    if let Some(phi) = phi {
        for (incoming, from) in arms(function, phi) {
            let produced = defining(function, incoming)
                .filter(|&call| matches!(function.instruction(call).opcode, Opcode::Call(_)))
                .filter(|&call| one_use(function, call) && function.parent(call) == Some(from) && permitted(call));
            if produced.is_some() {
                tail_calls.push(from);
                continue;
            }
            // The value is a memory intrinsic's first argument, its result
            // unused.
            if function.successors(from).len() == 1 && function.successors(from)[0] == block {
                let call = function.terminator(from).and_then(|one| function.prev_in_block(one));
                if let Some(call) = call
                    && matches!(function.instruction(call).opcode, Opcode::Call(_))
                    && !used(function, call)
                    && memory::callee(context, function, call).is_some_and(|one| returns_first_argument(globals, one))
                    && function.instruction(call).operands.first() == Some(&incoming)
                    && permitted(call)
                {
                    tail_calls.push(from);
                }
            }
        }
    } else {
        let mut visited: Vec<BlockId> = Vec::new();
        for pred in function.predecessors(block) {
            if visited.contains(&pred) {
                continue;
            }
            visited.push(pred);
            let Some(call) = function.terminator(pred).and_then(|one| function.prev_in_block(one)) else { continue };
            if !matches!(function.instruction(call).opcode, Opcode::Call(_)) || used(function, call) || !permitted(call)
            {
                continue;
            }
            let undef = value.is_some_and(|operand| {
                matches!(
                    operand,
                    Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Undef)
                )
            });
            let first = memory::callee(context, function, call).is_some_and(|one| returns_first_argument(globals, one))
                && value.is_some_and(|operand| function.instruction(call).operands.first() == Some(&operand));
            if value.is_none() || undef || first {
                tail_calls.push(pred);
            }
        }
    }
    let mut changed = false;
    for pred in tail_calls {
        let Some(branch) = function.terminator(pred) else { continue };
        let instruction = function.instruction(branch);
        if instruction.opcode != Opcode::Br
            || instruction.operands.len() != 1
            || instruction.operands[0] != Operand::Block(block)
        {
            continue;
        }
        fold_return_into_uncond_branch(function, ret, block, pred);
        changed = true;
    }
    if changed && function.predecessors(block).is_empty() {
        for inst in function.iter_block_rev(block).collect::<Vec<_>>() {
            function.erase(inst).expect("only the block's own instructions use them");
        }
        function.erase_block(block).expect("an empty block nothing names");
    }
    changed
}

#[cfg(test)]
#[path = "cgpretdup_tests.rs"]
mod tests;
