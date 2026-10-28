//! An instruction nothing reads is gone: `DeadMachineInstructionElim` after
//! selection. Translated from
//! [DeadMachineInstructionElim.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/DeadMachineInstructionElim.cpp)
//! (`isDead`, `runOnMachineFunction`).
//!
//! # How it works
//! Backwards over each block, an instruction goes when it only computes: every
//! value it defines has no reader, it touches no register but its own results,
//! its memory access is not volatile, and the flags it sets are read by no
//! one. Its operands lose a reader, so a chain goes in one walk; the blocks are
//! walked again while anything went (the pin does one walk and has the
//! liveness of a loop's back edge).
//!
//! Selection leaves these where it builds a value its user then takes in
//! halves: a far pointer joined to a dword (`offset | selector << 16`) for the
//! string registers, whose offset and selector the emitter reads from the
//! halves. Left in, the join reads the selector at four bytes, and the
//! allocator holds the word in a dword register: `mov esi, ecx`, a dword spill.
//!
//! # Example
//! `memcpy` of a far struct at `-m16 -Os`: `v34 := v33 | v26 << 16`, read by no
//! one, is gone, and `v26` is a word again.

use std::sync::Arc;

use crate::backend::lanes::Lanes;
use crate::backend::liveness;
use crate::backend::precse::sets_flags;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct DeadMachineInstructionElim;

impl LIRTransform for DeadMachineInstructionElim {
    fn class_name(&self) -> &'static str {
        "DeadMachineInstructionElim"
    }

    fn name(&self) -> &str {
        "deadmi"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(eliminated(&body))
    }
}

/// `body` without the instructions nothing reads.
pub(crate) fn eliminated(body: &LirBody) -> LirBody {
    let (regs, bits) = (body.regs(), body.bits);
    let mut readers: IndexMap<u32, usize> = IndexMap::default();
    for block in &body.blocks {
        for one in block.insns.iter() {
            for &used in &one.uses {
                *readers.entry(used).or_default() += 1;
            }
        }
        for phi in &block.phis {
            for &(_, value) in &phi.incoming {
                *readers.entry(value).or_default() += 1;
            }
        }
    }
    let mut blocks: Vec<Vec<Arc<Insn>>> = body.blocks.iter().map(|block| block.insns.to_vec()).collect();
    let mut any = false;
    loop {
        let mut gone = false;
        for insns in &mut blocks {
            for index in (0..insns.len()).rev() {
                let one = &insns[index];
                if !is_dead(one, &readers)
                    || (sets_flags(regs, bits, one) && flags_read_after(regs, bits, insns, index))
                {
                    continue;
                }
                for &used in &one.uses {
                    if let Some(count) = readers.get_mut(&used) {
                        *count -= 1;
                    }
                }
                insns.remove(index);
                gone = true;
            }
        }
        if !gone {
            break;
        }
        any = true;
    }
    if !any {
        return body.clone();
    }
    let rebuilt = body.blocks.iter().zip(blocks).map(|(block, insns)| block.with_insns(insns)).collect();
    body.with_blocks(rebuilt)
}

/// `MachineInstr::isDead` and `isSafeToMove`: only values of its own defined,
/// none of them read.
fn is_dead(
    one: &Insn,
    readers: &IndexMap<u32, usize>,
) -> bool {
    let Some(what) = one.what.as_ref() else { return false };
    if !matches!(
        what.op,
        Operation::Move | Operation::Address | Operation::Binary | Operation::Unary | Operation::Extend
    ) {
        return false;
    }
    !one.defines.is_empty()
        && what.dests.iter().all(|dest| matches!(dest, Loc::Held(_)))
        && one.defines.iter().all(|value| readers.get(value).is_none_or(|count| *count == 0))
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile
        && !one.spill_reload
        && !one.spill_store
        && !one.frame_adjust
}

/// Whether an arithmetic flag `insns[index]` sets is read before the next
/// instruction that sets it. Only the six flags count (`Flag::READ`): a string
/// move reads the direction flag and no result of arithmetic. An instruction
/// with no effect table is taken to read them.
fn flags_read_after(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    insns: &[Arc<Insn>],
    index: usize,
) -> bool {
    let six = llrm_lir::flag::Flag::READ.bits();
    let mut pending = match liveness::effect(regs, bits, &insns[index]) {
        Some(effect) => Lanes::flags(effect.writes.mask() as u32 & six),
        None => Lanes::flags(six),
    };
    for later in &insns[index + 1..] {
        let Some(effect) = liveness::effect(regs, bits, later) else { return true };
        if !effect.all_reads().is_disjoint(&pending) {
            return true;
        }
        pending = pending.minus(&effect.writes);
        if pending.is_empty() {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Semantics};
    use crate::model::lir::LirBlock;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 2 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    fn body(insns: Vec<Arc<Insn>>) -> LirBody {
        let mut body =
            LirBody::new("dead", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        body.inputs = [1].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    fn names(body: &LirBody) -> Vec<String> {
        body.blocks[0].insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    /// A join nothing reads (`v3 = mov v1; v4 = shl v3`) left a dword read of a
    /// word value in the body: both go, the chain in one call.
    #[test]
    fn test_an_unread_chain_goes() {
        let one = body(vec![
            insn(0, Operation::Move, "mov", vec![held(3)], vec![held(1)], vec![3], vec![1]),
            insn(1, Operation::Binary, "shl", vec![held(4)], vec![held(3), held(3)], vec![4], vec![3]),
            insn(2, Operation::Compare, "cmp", vec![], vec![held(1), held(1)], vec![], vec![1]),
        ]);
        assert_eq!(names(&eliminated(&one)), ["cmp"]);
    }

    /// An `or` nothing reads whose flags the branch after it takes stays: the
    /// branch would read the flags of something else.
    #[test]
    fn test_an_unread_or_whose_flags_a_branch_reads_stays() {
        let one = body(vec![
            insn(0, Operation::Binary, "or", vec![held(4)], vec![held(1), held(1)], vec![4], vec![1]),
            insn(1, Operation::Branch, "je", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&eliminated(&one)), ["or", "je"]);
    }
}
