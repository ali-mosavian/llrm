//! A subtract whose difference nothing reads is a compare.
//!
//! Translated from the first switch of `X86InstrInfo::optimizeCompareInstr`
//! ([X86InstrInfo.cpp#L5477](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5477)),
//! which the pin's PeepholeOptimizer runs before allocation on every
//! `SUB` it can analyse as a compare.
//!
//! # How it works
//! `EmitCmp` builds `X86ISD::SUB` for a compare, "to enable CSE between SUB and
//! CMP". After MachineCSE and MachineSink, a `SUB` (register, immediate or
//! memory second operand) with no reader of its result writes only the flags,
//! which is what `CMP` does: the destination goes and the opcode becomes the
//! compare's. Left as a `SUB`, the dead result gets a register, and a copy
//! to keep the operand it overwrites.
//!
//! ```text
//!  d := sub a, b      (nothing reads d)         cmp a, b
//!  jl                                           jl
//! ```
//!
//! Not translated: the rest of `optimizeCompareInstr`, which removes a compare
//! whose flags an earlier instruction already sets (`SrcRegDef`, `NewCC`,
//! `InstsToUpdate`).
//!
//! # Runs
//! A machine phase on SSA after MachineSink and ahead of MachineCombiner (the
//! pin's `addMachineSSAOptimization` order), at every level but -O0.

use std::sync::Arc;

use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashSet;

pub struct PeepholeOptimizer;

impl LIRTransform for PeepholeOptimizer {
    fn class_name(&self) -> &'static str {
        "PeepholeOptimizer"
    }

    fn name(&self) -> &str {
        "peepholeopt"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(compared(&body))
    }
}

/// `body` with each dead-result subtract a compare.
pub fn compared(body: &LirBody) -> LirBody {
    let mut read: HashSet<u32> = HashSet::default();
    for block in &body.blocks {
        for phi in block.phis.iter() {
            read.extend(phi.incoming.iter().map(|&(_, value)| value));
        }
        for one in &block.insns {
            read.extend(one.uses.iter().copied());
        }
    }
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            if !block.insns.iter().any(|one| dead_subtract(one, &read).is_some()) {
                return block.clone();
            }
            changed = true;
            let insns = block
                .insns
                .iter()
                .map(|one| match dead_subtract(one, &read) {
                    Some(result) => Arc::new(compare_of(one, result)),
                    None => Arc::clone(one),
                })
                .collect::<Vec<_>>();
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The value `one` subtracts into, where that is a register of its own that
/// nothing reads and `one` is a plain `sub`.
fn dead_subtract(
    one: &Insn,
    read: &HashSet<u32>,
) -> Option<u32> {
    let what = one.what.as_ref()?;
    if what.op != Operation::Binary
        || what.name.as_deref() != Some("sub")
        || what.indirect
        || one.group.is_some()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let [Loc::Held(result)] = what.dests[..] else { return None };
    let [_, Loc::Held(_) | Loc::Imm(_) | Loc::Mem(_)] = what.sources[..] else { return None };
    (one.defines == [result.value] && !read.contains(&result.value)).then_some(result.value)
}

/// `one` as the compare of its operands.
fn compare_of(
    one: &Insn,
    result: u32,
) -> Insn {
    let what = one.what.as_ref().expect("a subtract");
    let compare = Semantics { name: Some("cmp".into()), dests: Vec::new(), ..what.clone() };
    Insn {
        what: Some(Semantics { op: Operation::Compare, ..compare }),
        defines: Vec::new(),
        widths: one.widths.iter().copied().filter(|&(value, _)| value != result).collect(),
        ..one.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Held, Imm};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
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

    fn names(
        body: &LirBody,
        kept: bool,
    ) -> Vec<String> {
        let _ = kept;
        body.blocks[0].insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
    }

    fn body_of(insns: Vec<Arc<Insn>>) -> LirBody {
        let block = LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(0, insns) };
        let mut body = LirBody::new("f", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        body
    }

    /// `v3 := sub v1, v2; jl` with nothing reading `v3` (a compare `EmitCmp`
    /// built as a subtract): the dead result took a register and a copy of the
    /// operand it overwrote (x_heapsort: 12 `sub` where the pin has 1, +16
    /// `mov`).
    #[test]
    fn test_a_subtract_nothing_reads_is_a_compare() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
            insn(1, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        let done = compared(&body);
        assert_eq!(names(&done, true), ["cmp", "jl"]);
        let cmp = &done.blocks[0].insns[0];
        assert!(cmp.defines.is_empty() && cmp.uses == [1, 2]);
        let what = cmp.what.as_ref().expect("a compare");
        assert!(what.op == Operation::Compare && what.dests.is_empty() && what.sources == [held(1), held(2)]);
    }

    /// The immediate form is a compare too.
    #[test]
    fn test_a_subtract_of_an_immediate_nothing_reads_is_a_compare() {
        let imm = Loc::Imm(Imm { value: 7, width: 4, address: None });
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), imm], vec![3], vec![1]),
            insn(1, Operation::Branch, "jae", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), true), ["cmp", "jae"]);
    }

    /// A difference somebody reads stays a subtract.
    #[test]
    fn test_a_subtract_something_reads_stays() {
        let body = body_of(vec![
            insn(0, Operation::Binary, "sub", vec![held(3)], vec![held(1), held(2)], vec![3], vec![1, 2]),
            insn(1, Operation::Move, "mov", vec![held(4)], vec![held(3)], vec![4], vec![3]),
            insn(2, Operation::Branch, "jl", vec![], vec![], vec![], vec![]),
        ]);
        assert_eq!(names(&compared(&body), false), ["sub", "mov", "jl"]);
    }
}
