//! `X86FixupLEAs`. Translated from
//! [X86FixupLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L232)
//! (`runOnMachineFunction`), function by function, one PR each (plan:
//! `docs/working/lea-fixup-plan.md`).
//!
//! # How it works
//! The pass visits every LEA of every block in order. On i486 none of
//! `slowLEA`, `slow3OpsLEA` or `leaUsesAG` is set, so only `optTwoAddrLEA`
//! (L564) runs on each; until it moves here (row 100, PR 4) the rules in
//! `peephole.peep` (`lea_fixed`, `lea_alus`) do its work and this pass changes
//! nothing.
//!
//! Done here: [`is_lea`] (L227), the block loop, [`previous_instr`] (L295),
//! [`uses_register`] (L270) and [`search_backwards`] (L308), which finds the
//! write of a register within five cycles before an instruction.
//!
//! # Runs
//! After `FixupBW`, optimizing levels only.

use llrm_lir::registers::{RegId, Regs};

use crate::backend::peephole::_lanes;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

/// `INSTR_DISTANCE_THRESHOLD`: how far back a write still matters, in cycles.
const INSTR_DISTANCE_THRESHOLD: u32 = 5;

pub struct FixupLeas;

impl LIRTransform for FixupLeas {
    fn class_name(&self) -> &'static str {
        "FixupLEAs"
    }

    fn name(&self) -> &str {
        "fixupleas"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        // `runOnMachineFunction`: every LEA of every block gets
        // `optTwoAddrLEA`, which has not moved here yet.
        for block in &body.blocks {
            for one in block.insns.iter().filter(|one| is_lea(one)) {
                let _ = one;
            }
        }
        Ok(body)
    }
}

/// `isLEA`: `LEA32r`.
pub fn is_lea(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Address && what.name.as_deref() == Some("lea"))
}

/// `RegUsageState`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegUsage {
    NotUsed,
    Read,
    Write,
}

/// `usesRegister`: whether `one` writes, else reads, any byte of `register`.
/// A write wins over a read, as the pin's operand loop returns at the first
/// def.
pub fn uses_register(
    regs: Regs,
    register: RegId,
    one: &Insn,
) -> RegUsage {
    let Some(what) = one.what.as_ref() else { return RegUsage::NotUsed };
    let mine = _lanes(regs, register);
    let touches = |loc: &Loc| match loc {
        Loc::Reg(reg) => !_lanes(regs, reg.register).is_disjoint(&mine),
        Loc::Mem(mem) => [mem.through, mem.index_through]
            .into_iter()
            .any(|r| r != RegId::None && !_lanes(regs, r).is_disjoint(&mine)),
        _ => false,
    };
    if what.dests.iter().any(|dest| matches!(dest, Loc::Reg(_)) && touches(dest)) {
        return RegUsage::Write;
    }
    if what.dests.iter().chain(&what.sources).any(touches) { RegUsage::Read } else { RegUsage::NotUsed }
}

/// `getPreviousInstr`: the index before `at`, wrapping to the last instruction
/// when the block branches to itself.
pub fn previous_instr(
    block: &LirBlock,
    at: usize,
) -> Option<usize> {
    match at {
        0 if block.succ.contains(&block.at) && !block.insns.is_empty() => Some(block.insns.len() - 1),
        0 => None,
        _ => Some(at - 1),
    }
}

/// `TSM.computeInstrLatency` on the default model: a load takes 4, a
/// bookkeeping instruction 0, anything else 1.
fn instr_latency(one: &Insn) -> u32 {
    let Some(what) = one.what.as_ref() else { return 0 };
    if matches!(
        what.op,
        Operation::Nothing | Operation::Restore | Operation::Data
    ) {
        0
    } else if what.sources.iter().any(|source| matches!(source, Loc::Mem(_))) {
        4
    } else {
        1
    }
}

/// `searchBackwards`: the nearest earlier instruction in the block that
/// writes `register`, within [`INSTR_DISTANCE_THRESHOLD`] cycles and no
/// call or inline asm between.
pub fn search_backwards(
    regs: Regs,
    register: RegId,
    block: &LirBlock,
    at: usize,
) -> Option<usize> {
    let mut distance = 1;
    let mut cur = previous_instr(block, at);
    while let Some(index) = cur.filter(|index| *index != at) {
        let one = &block.insns[index];
        if one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Call | Operation::Escape)) {
            break;
        }
        if distance > INSTR_DISTANCE_THRESHOLD {
            break;
        }
        if uses_register(regs, register, one) == RegUsage::Write {
            return Some(index);
        }
        distance += instr_latency(one);
        cur = previous_instr(block, index);
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::{RegId, Regs};

    use super::*;
    use crate::model::ir::{Mem, Reg, Semantics};

    fn regs() -> Regs {
        Regs(&llrm_x86_m32::REGISTER_INFO)
    }

    fn reg(
        register: RegId,
        width: u32,
    ) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    fn mov(
        at: i64,
        dest: RegId,
        source: RegId,
    ) -> Arc<Insn> {
        insn(at, Operation::Move, "mov", vec![reg(dest, 4)], vec![reg(source, 4)])
    }

    fn load(
        at: i64,
        dest: RegId,
    ) -> Arc<Insn> {
        let cell = Loc::Mem(Mem { through: RegId::ESI, ..Mem::new(None, 4) });
        insn(at, Operation::Move, "mov", vec![reg(dest, 4)], vec![cell])
    }

    fn lea(at: i64) -> Arc<Insn> {
        insn(at, Operation::Address, "lea", vec![reg(RegId::EAX, 4)], vec![reg(RegId::ECX, 4)])
    }

    fn block(
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(0, insns) }
    }

    #[test]
    fn a_write_to_a_byte_of_a_register_is_a_write_of_the_register() {
        let al = insn(0, Operation::Move, "mov", vec![reg(RegId::AL, 1)], vec![reg(RegId::CL, 1)]);
        assert_eq!(uses_register(regs(), RegId::EAX, &al), RegUsage::Write);
        assert_eq!(uses_register(regs(), RegId::ECX, &al), RegUsage::Read);
        assert_eq!(uses_register(regs(), RegId::EDX, &al), RegUsage::NotUsed);
    }

    #[test]
    fn an_address_register_is_read() {
        assert_eq!(uses_register(regs(), RegId::ESI, &load(0, RegId::EAX)), RegUsage::Read);
    }

    #[test]
    fn the_nearest_write_within_the_window_is_found() {
        let b = block(vec![mov(0, RegId::ECX, RegId::EDX), mov(1, RegId::ECX, RegId::EBX), lea(2)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 2), Some(1));
        assert_eq!(search_backwards(regs(), RegId::EDX, &b, 2), None);
    }

    /// Two loads cost 8 cycles: the write before them is past the threshold.
    #[test]
    fn a_write_beyond_five_cycles_is_not_found() {
        let b = block(vec![mov(0, RegId::ECX, RegId::EDX), load(1, RegId::EBX), load(2, RegId::EDI), lea(3)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 3), None);
        let near = block(vec![mov(0, RegId::ECX, RegId::EDX), load(1, RegId::EBX), lea(2)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &near, 2), Some(0));
    }

    #[test]
    fn a_call_ends_the_search() {
        let call = insn(1, Operation::Call, "call", vec![], vec![]);
        let b = block(vec![mov(0, RegId::ECX, RegId::EDX), call, lea(2)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 2), None);
    }

    /// A block that branches to itself continues from its last instruction;
    /// the search stops on reaching the start again.
    #[test]
    fn a_self_loop_wraps_to_the_last_instruction() {
        let b = block(vec![lea(0), mov(1, RegId::ECX, RegId::EDX)], vec![0]);
        assert_eq!(previous_instr(&b, 0), Some(1));
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 0), Some(1));
        let straight = block(vec![lea(0), mov(1, RegId::ECX, RegId::EDX)], vec![]);
        assert_eq!(previous_instr(&straight, 0), None);
        assert_eq!(search_backwards(regs(), RegId::ECX, &straight, 0), None);
    }

    #[test]
    fn only_the_lea_is_a_lea() {
        assert!(is_lea(&lea(0)));
        assert!(!is_lea(&mov(0, RegId::EAX, RegId::ECX)));
    }

    #[test]
    fn the_pass_leaves_a_body_alone() {
        let b = LirBody::new("f", 0, vec![block(vec![lea(0)], vec![]).into()], Default::default(), Default::default());
        assert_eq!(FixupLeas.transform(b.clone()).unwrap(), b);
    }
}
