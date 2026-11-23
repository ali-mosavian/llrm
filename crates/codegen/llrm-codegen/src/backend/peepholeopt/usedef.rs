//! A compare of a value with zero that the flags of an instruction reading
//! the value already hold.
//!
//! Translated from the `isUseDefConvertible` half of
//! `X86InstrInfo::optimizeCompareInstr`
//! ([X86InstrInfo.cpp#L5473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5473)):
//! [`use_def_convertible`] is `isUseDefConvertible`
//! ([L5307](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5307)),
//! [`compares_of_uses_reused`] the backward scan that takes the nearest flag
//! writer
//! ([L5596](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5596))
//! and the forward scan that rewrites the readers
//! ([L5760](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5760)).
//!
//! # How it works
//! `neg x` sets CF to "x is not zero" and ZF to "x is zero", so `test x, x`
//! after it, with no flag writer between them, is the same question: a reader
//! of ZF reads CF instead (`je` is `jae`, `jne` is `jb`, as `NewCC` and its
//! opposite). `bsf` and `bsr` set ZF to "the source is zero". The instruction
//! counts as a flag source only for the value it reads, not the one it makes.
//! Every reader before the flags are written again must be a condition on
//! equality (`COND_E`, `COND_NE`): the other flags mean something else after
//! `neg`. A reader of anything else, or flags live into a successor, keeps the
//! compare.
//!
//! Not translated: `lzcnt`, `tzcnt`, `popcnt` and the BMI `blsi`, `blsr` and
//! `blsmsk` rows of the table (no x86 target here has them; `rep bsf` is
//! `tzcnt`, whose flags differ, and is left alone), the backward scan into a
//! single predecessor and the NF variants (APX).
//!
//! # Example
//! ```text
//!  n := neg x ; lea ...        n := neg x ; lea ...
//!  test x, x                   jae L
//!  je L
//! ```
//!
//! From `test_a_test_after_a_neg_of_the_value_is_the_negs_carry`.
//!
//! # Runs
//! In `PeepholeOptimizer`, after the compare-with-zero reuse of the
//! instruction that made the value.

use std::sync::Arc;

use super::redundant::split;
use super::{compared_with_zero, flag_effects, successor_overwrites_flags};
use crate::backend::lanes::Lanes;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};

/// `isUseDefConvertible`: the value `one` reads, its width, and the
/// condition (`NewCC`) that reads "the value is zero" in the flags `one` sets.
fn use_def_convertible(one: &Insn) -> Option<(u32, u32, &'static str)> {
    let what = one.what.as_ref()?;
    if what.indirect || one.group.is_some() || what.repeat {
        return None;
    }
    match (what.op, what.name.as_deref()?, &what.sources[..]) {
        (Operation::Unary, "neg", [Loc::Held(source)]) => Some((source.value, source.width, "ae")),
        (Operation::Binary, "bsf" | "bsr", [_, Loc::Held(source)]) => Some((source.value, source.width, "e")),
        _ => None,
    }
}

/// `GetOppositeBranchCondition` of the conditions `use_def_convertible` gives.
fn opposite(condition: &str) -> &'static str {
    match condition {
        "ae" => "b",
        "e" => "ne",
        _ => unreachable!("a condition of the use-def table"),
    }
}

/// `body` with each compare of a value with zero that the flags of an
/// instruction reading it hold gone, its readers' conditions rewritten.
pub(super) fn compares_of_uses_reused(body: &LirBody) -> LirBody {
    let flags = flag_effects(body);
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let mut at = 0;
            while at < insns.len() {
                if let Some(updates) = redundant(body, block, &insns, at, &flags) {
                    for (reader, name) in updates {
                        let mut new = (*insns[reader]).clone();
                        new.what.as_mut().expect("a conditional reader has semantics").name = Some(name.into());
                        insns[reader] = Arc::new(new);
                    }
                    insns.remove(at);
                    changed = true;
                } else {
                    at += 1;
                }
            }
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The readers (position, new name) of the flags of the compare at `at`, if
/// the nearest flag writer before it reads the value the compare tests.
fn redundant(
    body: &LirBody,
    block: &crate::model::lir::Block,
    insns: &[Arc<Insn>],
    at: usize,
    flags: &dyn Fn(&Insn) -> Option<(Lanes, Lanes)>,
) -> Option<Vec<(usize, String)>> {
    let value = compared_with_zero(&insns[at])?;
    let Some(Loc::Held(compared)) = insns[at].what.as_ref()?.sources.first() else { return None };
    let mut new_condition = None;
    for back in (0..at).rev() {
        let one = &insns[back];
        if one.is_meta() {
            continue;
        }
        let (_, writes) = flags(one)?;
        if writes.is_empty() {
            continue;
        }
        let (source, width, condition) = use_def_convertible(one)?;
        if source != value || width != compared.width {
            return None;
        }
        new_condition = Some(condition);
        break;
    }
    let new_condition = new_condition?;
    let mut pending = Lanes::read_flags();
    let mut updates = Vec::new();
    for (reader, one) in insns.iter().enumerate().skip(at + 1) {
        if one.is_meta() {
            continue;
        }
        let (reads, writes) = flags(one)?;
        if !reads.is_disjoint(&pending) {
            let name = one.what.as_ref()?.name.as_deref()?;
            let (prefix, condition) = split(name)?;
            let new = match condition {
                "e" => new_condition,
                "ne" => opposite(new_condition),
                _ => return None,
            };
            if new != condition {
                updates.push((reader, format!("{prefix}{new}")));
            }
        }
        pending = pending.minus(&writes);
        if pending.is_empty() {
            return Some(updates);
        }
    }
    block.succ.iter().all(|next| successor_overwrites_flags(body, *next, flags, pending)).then_some(updates)
}
