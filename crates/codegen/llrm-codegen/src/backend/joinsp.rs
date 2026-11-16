//! A copy of the stack pointer that nothing moves it from is the stack pointer:
//! `RegisterCoalescer::joinReservedPhysReg`, the case where a virtual register
//! is copied from a reserved register.
//!
//! # How it works
//! `llvm.stacksave` is `v = mov sp` and `llvm.stackrestore` is `mov sp, v`.
//! When no instruction between the save and its restores can write the stack
//! pointer, `v` holds what the stack pointer holds, the restores are
//! `mov sp, sp` and both go. Without it the restore is a write to the stack
//! pointer that no frame model can follow, and the function keeps a frame
//! register (`has_fp`: `OpaqueSpAdjustment`) though its frame is static.
//!
//! The pin's test is that the live range of `v` overlaps no def of a unit of
//! the reserved register (all of them dead defs: calls, `ADJCALLSTACK*`,
//! `sub rsp`). Here the range is the instructions between the save and the last
//! restore on every path, and the defs are the instructions below that can
//! write the stack pointer.
//!
//! ```text
//!   v = mov sp                    (gone)
//!   ... loops, no call ...   =>   ... loops, no call ...
//!   mov sp, v                     (gone)
//!   call report                   call report
//! ```
//!
//! DEVIATE 1: only the case the pin's `CP.isFlipped()` branch handles whose
//! uses are all restores; a `v` read any other way is left (the pin would
//! replace each read by the register, which here could place the stack
//! pointer where an encoding takes none). Exit: none seen.
//!
//! DEVIATE 2: the other branch (`sp = mov v` where `v` is defined once in the
//! block, moving the def to write `sp`) is not translated: a dynamic
//! allocation's `sub` writes the stack pointer through the same pin pass but
//! nothing here depends on it.
//!
//! DEVIATE 3: a block's phis and any other mention of `v` (`requires`,
//! `delivers`, `undefs`) end the join: the pin's `RHS.containsOneValue()` is
//! here a single `defines` and no reader but the restores.
//!
//! # Example
//! `stacksave`, a loop over a static array, `stackrestore`, a call
//! (vla_callee_constant, x86_64 -O2): no `mov rsp, rax`, no frame register.
//!
//! # Runs
//! Before the coalescer, after two-address conversion; not at -O0, where the
//! pin does not coalesce.
//!
//! # See also
//! LLVM [`RegisterCoalescer`]
//! (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2314);
//! [`crate::coalescer`], [`crate::backend::framefree`]

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::model::ir::{Loc, Operation, root};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct JoinStackPointer {
    pub stack: RegId,
}

impl LIRTransform for JoinStackPointer {
    fn class_name(&self) -> &'static str {
        "JoinStackPointer"
    }

    fn name(&self) -> &str {
        "joinsp"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(joined(&body, self.stack))
    }
}

/// `body` with every save of the stack pointer that can be joined with it and
/// its restores removed.
pub(crate) fn joined(
    body: &LirBody,
    stack: RegId,
) -> LirBody {
    let saves = candidates(body, stack);
    if saves.is_empty() {
        return body.clone();
    }
    let mut gone: Vec<Vec<bool>> = body.blocks.iter().map(|block| vec![false; block.insns.len()]).collect();
    let mut any = false;
    for value in saves {
        if let Some(places) = joinable(body, stack, value) {
            for (block, index) in places {
                gone[block][index] = true;
            }
            any = true;
        }
    }
    if !any {
        return body.clone();
    }
    let rebuilt = body
        .blocks
        .iter()
        .zip(&gone)
        .map(|(block, gone)| {
            let kept: Vec<Arc<Insn>> =
                block.insns.iter().zip(gone).filter(|(_, gone)| !**gone).map(|(one, _)| Arc::clone(one)).collect();
            block.with_insns(kept)
        })
        .collect();
    body.with_blocks(rebuilt)
}

/// `v = mov sp`: the value `v` it defines.
fn save_of(
    one: &Insn,
    stack: RegId,
) -> Option<u32> {
    let what = one.what.as_ref()?;
    match (what.op, &what.dests[..], &what.sources[..], &one.defines[..]) {
        (Operation::Move, [Loc::Held(held)], [Loc::Reg(reg)], [value])
            if root(reg.register) == root(stack)
                && held.value == *value
                && held.offset == 0
                && held.width == reg.width =>
        {
            Some(*value)
        }
        _ => None,
    }
}

/// `mov sp, v` for a `v` of `members`: the value read.
fn restore_of(
    one: &Insn,
    stack: RegId,
    members: &[u32],
) -> Option<u32> {
    let what = one.what.as_ref()?;
    match (what.op, &what.dests[..], &what.sources[..], &one.uses[..]) {
        (Operation::Move, [Loc::Reg(reg)], [Loc::Held(held)], [used])
            if root(reg.register) == root(stack)
                && members.contains(&held.value)
                && held.value == *used
                && held.offset == 0
                && held.width == reg.width
                && one.defines.is_empty() =>
        {
            Some(*used)
        }
        _ => None,
    }
}

/// `d = mov s`: the value `d` it defines and the `s` it copies.
fn copy_of(one: &Insn) -> Option<(u32, u32)> {
    let what = one.what.as_ref()?;
    match (what.op, &what.dests[..], &what.sources[..], &one.defines[..], &one.uses[..]) {
        (Operation::Move, [Loc::Held(to)], [Loc::Held(from)], [defined], [used])
            if to.value == *defined
                && from.value == *used
                && to.offset == 0
                && from.offset == 0
                && to.width == from.width =>
        {
            Some((*defined, *used))
        }
        _ => None,
    }
}

fn candidates(
    body: &LirBody,
    stack: RegId,
) -> Vec<u32> {
    body.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| save_of(one, stack)).collect()
}

/// Whether `one` can write the stack pointer: anything the walk of `has_fp`
/// does not model as a plain computation.
fn may_write_stack(
    one: &Insn,
    stack: RegId,
) -> bool {
    let Some(what) = one.what.as_ref() else { return true };
    let plain = matches!(
        what.op,
        Operation::Move
            | Operation::Exchange
            | Operation::Address
            | Operation::Binary
            | Operation::Multiply
            | Operation::Divide
            | Operation::Compare
            | Operation::Unary
            | Operation::Funnel
            | Operation::Extend
            | Operation::Jump
            | Operation::Branch
            | Operation::Select
            | Operation::Nothing
            | Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    );
    !plain
        || one.call.is_some()
        || one.frame_adjust
        || one.clobbers.iter().chain(&one.clobbers_high).any(|register| root(*register) == root(stack))
        || what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if root(reg.register) == root(stack)))
}

/// The instructions to delete (block index, index) if the save of `value` can
/// be joined: the values it is copied into are read only by restores, and no
/// instruction on a path between the save and a restore can write the stack
/// pointer.
fn joinable(
    body: &LirBody,
    stack: RegId,
    value: u32,
) -> Option<Vec<(usize, usize)>> {
    let all = || {
        body.blocks
            .iter()
            .enumerate()
            .flat_map(|(b, block)| block.insns.iter().enumerate().map(move |(i, one)| (b, i, one)))
    };
    // `%v = COPY %save`: the pin joins these first, then the join with the
    // register.
    let mut members = vec![value];
    loop {
        let before = members.len();
        for (_, _, one) in all() {
            if let Some((to, from)) = copy_of(one) {
                if members.contains(&from) && !members.contains(&to) {
                    members.push(to);
                }
            }
        }
        if members.len() == before {
            break;
        }
    }
    let mut save = None;
    let mut copies: Vec<(usize, usize)> = Vec::new();
    let mut restores: Vec<(usize, usize)> = Vec::new();
    let mut defined: Vec<u32> = Vec::new();
    for (b, block) in body.blocks.iter().enumerate() {
        if block
            .phis
            .iter()
            .any(|phi| members.contains(&phi.result) || phi.incoming.iter().any(|(_, v)| members.contains(v)))
        {
            return None;
        }
        for (i, one) in block.insns.iter().enumerate() {
            let names = |list: &[u32]| list.iter().any(|v| members.contains(v));
            if names(&one.partial_defs)
                || names(&one.early_clobbers)
                || names(&one.undefs)
                || one.requires.iter().chain(&one.delivers).any(|(held, _)| members.contains(&held.value))
            {
                return None;
            }
            if !names(&one.defines) && !names(&one.uses) {
                continue;
            }
            if save_of(one, stack) == Some(value) {
                if save.is_some() {
                    return None;
                }
                save = Some((b, i));
            } else if let Some((to, _)) =
                copy_of(one).filter(|(to, from)| members.contains(to) && members.contains(from))
            {
                if defined.contains(&to) || to == value {
                    return None;
                }
                defined.push(to);
                copies.push((b, i));
            } else if restore_of(one, stack, &members).is_some() {
                restores.push((b, i));
            } else {
                return None;
            }
        }
    }
    let (def_block, def_index) = save?;
    if restores.is_empty() || defined.len() + 1 != members.len() {
        return None;
    }
    let index_of: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(b, block)| (block.at, b)).collect();
    let succs: Vec<Vec<usize>> = body
        .blocks
        .iter()
        .map(|block| block.succ.iter().filter_map(|at| index_of.get(at).copied()).collect())
        .collect();
    let last_use = |b: usize| restores.iter().filter(|(rb, _)| *rb == b).map(|&(_, i)| i).max();
    // A restore above the save in its own block reads the previous trip's
    // value.
    if restores.iter().any(|&(b, i)| b == def_block && i < def_index) {
        return None;
    }
    // Live into a block: restored in it, or in a block it leads to, before any
    // save.
    let n = body.blocks.len();
    let mut live_in = vec![false; n];
    loop {
        let mut changed = false;
        for b in (0..n).rev() {
            if b == def_block || live_in[b] {
                continue;
            }
            if last_use(b).is_some() || succs[b].iter().any(|&s| live_in[s]) {
                live_in[b] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let live_out = |b: usize| succs[b].iter().any(|&s| live_in[s]);
    for (b, block) in body.blocks.iter().enumerate() {
        let (from, to) = if b == def_block {
            (def_index + 1, if live_out(b) { block.insns.len() } else { last_use(b).unwrap_or(def_index + 1) })
        } else if live_in[b] {
            (0, if live_out(b) { block.insns.len() } else { last_use(b).unwrap_or(0) })
        } else {
            continue;
        };
        for i in from..to.max(from).min(block.insns.len()) {
            let one = &block.insns[i];
            if restore_of(one, stack, &members).is_none() && may_write_stack(one, stack) {
                return None;
            }
        }
    }
    let mut places = restores;
    places.extend(copies);
    places.push((def_block, def_index));
    Some(places)
}

#[cfg(test)]
#[path = "joinsp_tests.rs"]
mod tests;
