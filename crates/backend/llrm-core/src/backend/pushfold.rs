//! A pushed value that a load made and nothing else reads is pushed from
//! memory: `mov r, [m]; push r` is `push [m]`. Translated from
//! [X86CallFrameOptimization.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp)
//! (`canFoldIntoRegPush`, and the `PUSH32rmm` branch of `adjustCallSequence`).
//!
//! # How it works
//! A `push` whose one source is a value qualifies when:
//!
//! - the value is read once in the body (`hasOneNonDBGUse`);
//! - its definition is the nearest earlier one in the block, a plain `mov` of a
//!   memory cell of the value's width (a 2- or 4-byte cell: `push word ptr`,
//!   `push dword ptr`), not volatile, with no clobbers, requirements or group,
//!   and not a spill reload;
//! - nothing between the load and the push is volatile, a call, a barrier or an
//!   escape, or may write the cell (`isLoadFoldBarrier`: the pin's check is the
//!   same walk, from the load to the call frame setup);
//! - the cell is not addressed through the stack pointer, whose value the
//!   earlier pushes of the sequence move.
//!
//! The push then reads the cell and the load is deleted. The pin does this when
//! it turns the stores of a reserved call frame into pushes; ours pushes at
//! selection already, so the fold is all that is left of the pass (DEVIATE
//! CF1). The pin skips it where `slowTwoMemOps` holds; the i486 and the 8086 do
//! not set it.
//!
//! DEVIATE CF1 (no number): the pin's input is `mov [esp+k], r` stores into a
//! reserved frame, converted to pushes and folded in one pass; ours has pushes
//! from selection, so the conversion has no input and `isProfitable` (its
//! reserved-frame accounting) no question. Exit: none, the form is not made.
//!
//! # Example
//! `f(p->a, p->b, p->c)`, before (after `FloatFold`):
//!
//! ```lir
//! g: 9 instructions
//!   block 0x0000
//!     0x0001  v2 := mov [v1 + 8]
//!     0x0002  push v2
//!     0x0003  v3 := mov [v1 + 4]
//!     0x0004  push v3
//!     0x0005  v4 := mov [v1]
//!     0x0006  push v4
//! ```
//!
//! After:
//!
//! ```lir
//! g: 6 instructions
//!   block 0x0000
//!     0x0002  push [v1 + 8]
//!     0x0004  push [v1 + 4]
//!     0x0006  push [v1]
//! ```
//!
//! # Runs
//! Machine phase after `floatfold`, before the register classes are decided,
//! once per function, at every -O level but -O0 (the pin's pass is
//! `-no-x86-call-frame-opt`-switchable only, and runs at -O0 too; ours does not
//! at -O0, where nothing improves after selection).
//!
//! # Needs
//! [`crate::backend::storedhomes`] for the may-write test.
//!
//! # See also
//! [`crate::backend::floatfold`], the same fold for x87 arithmetic.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::storedhomes::may_write;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashMap;

pub struct PushFold;

impl LIRTransform for PushFold {
    fn class_name(&self) -> &'static str {
        "PushFold"
    }

    fn name(&self) -> &str {
        "pushfold"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(folded(&body))
    }
}

/// `body` with each push of a once-read load pushed from the load's cell.
pub fn folded(body: &LirBody) -> LirBody {
    // Reads of each value: instructions' register uses (an address's registers
    // included) and phi operands.
    let mut reads: HashMap<u32, usize> = HashMap::default();
    for block in &body.blocks {
        for one in block.insns.iter() {
            let mut seen: Vec<u32> = one.uses.clone();
            seen.sort_unstable();
            seen.dedup();
            for used in seen {
                *reads.entry(used).or_default() += 1;
            }
        }
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                *reads.entry(value).or_default() += 1;
            }
        }
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut at = 0;
        while at < insns.len() {
            let Some((home, value)) = candidate(&insns, at, &reads) else {
                at += 1;
                continue;
            };
            let load = Arc::clone(&insns[home]);
            let push = Arc::clone(&insns[at]);
            let Some(cell) = load.what.as_ref().and_then(|what| match what.sources.as_slice() {
                [Loc::Mem(cell)] => Some(cell.clone()),
                _ => None,
            }) else {
                at += 1;
                continue;
            };
            let mut what = push.what.clone().expect("a push");
            what.sources = vec![Loc::Mem(cell)];
            let mut fused = (*push).clone();
            fused.what = Some(what);
            fused.uses =
                push.uses.iter().copied().filter(|one| *one != value).chain(load.uses.iter().copied()).collect();
            fused.uses.dedup();
            fused.widths = push
                .widths
                .iter()
                .chain(&load.widths)
                .copied()
                .filter(|(one, _)| *one != value)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            insns[at] = Arc::new(fused);
            insns.remove(home);
            changed = true;
            // `at` now names the instruction after the fused push.
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The load `insns[at]`, a push of one value, may take its cell from: its
/// position and the value.
fn candidate(
    insns: &[Arc<Insn>],
    at: usize,
    reads: &HashMap<u32, usize>,
) -> Option<(usize, u32)> {
    let push = &insns[at];
    let what = push.what.as_ref().filter(|what| what.op == Operation::Push && what.dests.is_empty())?;
    let [Loc::Held(held)] = what.sources.as_slice() else { return None };
    let plain = push.clobbers.is_empty()
        && push.requires.is_empty()
        && push.delivers.is_empty()
        && push.group.is_none()
        && !push.volatile
        && push.call.is_none();
    if !plain || !matches!(held.width, 2 | 4) || reads.get(&held.value) != Some(&1) {
        return None;
    }
    let home = (0..at)
        .rev()
        .find(
            |&back| insns[back]
                .what
                .as_ref()
                .is_some_and(
                    |made| made.dests.iter().any(|dest| matches!(dest, Loc::Held(other) if other.value == held.value)),
                ),
        )?;
    let load = &insns[home];
    let made = load.what.as_ref()?;
    let [Loc::Held(defined)] = made.dests.as_slice() else { return None };
    let [Loc::Mem(cell)] = made.sources.as_slice() else { return None };
    if made.op != Operation::Move
        || defined.width != held.width
        || cell.width != held.width
        || cell.through == RegId::SP
        || cell.index_through == RegId::SP
        || load.volatile
        || !load.clobbers.is_empty()
        || !load.requires.is_empty()
        || !load.delivers.is_empty()
        || load.group.is_some()
        || load.spill_reload
        || load.spill_store
        || load.frame_adjust
        || load.call.is_some()
    {
        return None;
    }
    let blocked = insns[home + 1..at]
        .iter()
        .any(
            |between| between.volatile
                || between.call.is_some()
                || may_write(between, cell, false)
                || between
                    .what
                    .as_ref()
                    .is_none_or(|w| matches!(w.op, Operation::Call | Operation::Barrier | Operation::Escape)),
        );
    (!blocked).then_some((home, held.value))
}

#[cfg(test)]
#[path = "pushfold_tests.rs"]
mod tests;
