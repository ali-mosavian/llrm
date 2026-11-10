//! A scalar SSE instruction that writes only the low lane of its destination
//! waits for the last write of the whole register, unless the register was
//! last written long enough ago: `BreakFalseDeps`. Translated from
//! [BreakFalseDeps.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BreakFalseDeps.cpp)
//! (`processDefs`, the partial-register half; `shouldBreakDependence`) with the
//! clearance of
//! [ReachingDefAnalysis.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ReachingDefAnalysis.cpp)
//! (`getClearance`) and the x86 hooks
//! [`hasPartialRegUpdate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L6672),
//! `getPartialRegUpdateClearance` (L7056) and `breakPartialRegDependency`
//! (L7453).
//!
//! # How it works
//! `cvtsi2ss xmm0, eax` keeps the upper lanes of `xmm0`, so it reads `xmm0`
//! though its text does not. When the last instruction to write `xmm0` is
//! fewer than 64 instructions before it, an `xorps xmm0, xmm0` (a zeroing
//! idiom the core does not wait on) goes just before it. The distance is the
//! reaching definition's: a definition from a predecessor block is counted
//! from the end of that block and the nearest of the predecessors' is taken; a
//! register nothing in the function wrote has no definition and is clear. An
//! instruction that also reads its destination (`cvtss2sd xmm0, xmm0`) is
//! left, the dependence is a true one.
//!
//! ```text
//!   cvtsi2ss xmm0, eax            cvtsi2ss xmm0, eax
//!   addss    xmm1, xmm0           addss    xmm1, xmm0
//!   cvtsi2ss xmm0, ecx    ==>     xorps    xmm0, xmm0
//!                                 cvtsi2ss xmm0, ecx
//! ```
//!
//! # Example
//! The unit tests `a_convert_right_after_a_write_of_its_register_gets_a_xorps`
//! and `a_convert_the_register_was_last_written_far_before_is_left`.
//!
//! # Runs
//! Last, optimizing levels only: the pin runs it before emission, after block
//! placement, as the distances are counted in the final order.
//!
//! # Limits
//! Only the instructions of the SSE baseline: the pin's AVX forms
//! (`v`-prefixed, `vxorps`) and the undefined-source half
//! (`pickBestRegisterForUndef`, the shuffles) have no counterpart here, the
//! target has no AVX. A call's clobbers are not definitions, as the pin's
//! register mask is not an operand a def walks. The reaching definitions are
//! solved to a fixed point over the blocks where the pin walks a loop's blocks
//! twice; the two agree unless loops nest three deep with the definition in the
//! innermost. A function with `minsize` gets none (the pin's `hasMinSize`
//! bail), `optsize` does.

use std::sync::Arc;

use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashMap;

/// `-partial-reg-update-clearance`.
const CLEARANCE: i64 = 64;
/// `ReachingDefDefaultVal`: nothing wrote the register.
const NONE: i64 = -(1 << 21);
/// `FunctionLiveInMarker`: an argument register, as if written just before the
/// first instruction.
const LIVE_IN: i64 = -1;
/// The registers `xmm0` to `xmm31`.
const UNITS: usize = 32;

type Defs = [i64; UNITS];

/// The phase.
pub struct BreakFalseDeps;

impl LIRTransform for BreakFalseDeps {
    fn class_name(&self) -> &'static str {
        "BreakFalseDeps"
    }

    fn name(&self) -> &str {
        "breakfalsedeps"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if body.min_size || !body.blocks.iter().any(|block| block.insns.iter().any(|one| partial_update(one).is_some()))
        {
            return Ok(body);
        }
        Ok(self.broken(&body))
    }
}

impl BreakFalseDeps {
    /// `runOnMachineFunction`: every block against the definitions that reach
    /// it.
    fn broken(
        &self,
        body: &LirBody,
    ) -> LirBody {
        let entering = reaching(body);
        let mut out = body.clone();
        for (block, defs) in out.blocks.iter_mut().zip(&entering) {
            let mut at = *defs;
            let mut count = 0_i64;
            let mut insns: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len() + 1);
            for one in block.insns.iter() {
                if one.emits() {
                    if let Some(dest) = partial_update(one) {
                        // `shouldBreakDependence`: the clearance is below the
                        // want.
                        if count - at[unit(dest)] < CLEARANCE {
                            insns.push(Arc::new(zeroed(one, dest)));
                        }
                    }
                    for written in written(one) {
                        at[written] = count;
                    }
                    count += 1;
                }
                insns.push(Arc::clone(one));
            }
            *block = block.with_insns(insns);
        }
        out
    }
}

/// The register `one` writes a low lane of, and nothing else reads: the
/// instructions of `hasPartialRegUpdate` that exist without AVX.
/// `getPartialRegUpdateClearance` returns none when the destination is also a
/// source, the update is then wanted.
fn partial_update(one: &Insn) -> Option<Reg> {
    let what = one.what.as_ref()?;
    if !matches!(
        what.name.as_deref()?,
        "cvtsi2ss"
            | "cvtsi2sd"
            | "cvtss2sd"
            | "cvtsd2ss"
            | "sqrtss"
            | "sqrtsd"
            | "rcpss"
            | "rsqrtss"
            | "roundss"
            | "roundsd"
    ) {
        return None;
    }
    let [Loc::Reg(dest)] = what.dests.as_slice() else { return None };
    if !dest.register.is_xmm() {
        return None;
    }
    let read = what.sources.iter().any(|source| matches!(source, Loc::Reg(reg) if reg.register == dest.register));
    (!read).then_some(*dest)
}

/// `breakPartialRegDependency`: `xorps dest, dest` before `one`.
fn zeroed(
    one: &Insn,
    dest: Reg,
) -> Insn {
    let dest = Reg { register: dest.register, width: 16 };
    let what = Semantics {
        name: Some("xorps".into()),
        dests: vec![Loc::Reg(dest)],
        sources: vec![Loc::Reg(dest), Loc::Reg(dest)],
        ..Semantics::new(Operation::Binary)
    };
    Insn::new(one.at, one.covers, Some(what), vec![], vec![])
}

fn unit(reg: Reg) -> usize {
    (reg.register.number() as usize).min(UNITS - 1)
}

/// The `xmm` registers `one` defines: its destinations, and the results a call
/// delivers.
fn written(one: &Insn) -> Vec<usize> {
    let mut found = Vec::new();
    if let Some(what) = one.what.as_ref() {
        for dest in &what.dests {
            if let Loc::Reg(reg) = dest {
                if reg.register.is_xmm() {
                    found.push(unit(*reg));
                }
            }
        }
    }
    for (_, register) in &one.delivers {
        if register.is_xmm() {
            found.push(unit(Reg { register: *register, width: 16 }));
        }
    }
    found
}

/// What the block `block` leaves, relative to its end, given what reached its
/// start: the position of each register's last definition less the length of
/// the block (`leaveBasicBlock`).
fn leaving(
    block: &LirBlock,
    entering: &Defs,
) -> Defs {
    let mut at = *entering;
    let mut count = 0_i64;
    for one in block.insns.iter().filter(|one| one.emits()) {
        for written in written(one) {
            at[written] = count;
        }
        count += 1;
    }
    at.map(|def| if def == NONE { NONE } else { def - count })
}

/// `ReachingDefInfo`: for each block, in `body.blocks` order, the definitions
/// that reach its start, as positions relative to it (negative: an earlier
/// block). The nearest of the predecessors' wins (`enterBasicBlock`); the entry
/// block has the argument registers as written just before it.
fn reaching(body: &LirBody) -> Vec<Defs> {
    let place: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(place, block)| (block.at, place)).collect();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); body.blocks.len()];
    for (from, block) in body.blocks.iter().enumerate() {
        for to in crate::model::lir::distinct(&block.succ) {
            if let Some(&to) = place.get(&to) {
                preds[to].push(from);
            }
        }
    }
    let entry = place.get(&body.entry).copied().unwrap_or(0);
    let mut start = [NONE; UNITS];
    for one in body.blocks.get(entry).iter().flat_map(|block| block.insns.iter()).filter(|one| one.arrival()) {
        for (_, register) in &one.delivers {
            if register.is_xmm() {
                start[unit(Reg { register: *register, width: 16 })] = LIVE_IN;
            }
        }
    }
    let mut entering: Vec<Defs> = vec![[NONE; UNITS]; body.blocks.len()];
    entering[entry] = start;
    let mut out: Vec<Defs> = body.blocks.iter().zip(&entering).map(|(block, defs)| leaving(block, defs)).collect();
    // Positions only grow and are bounded above by 0: this stops.
    loop {
        let mut changed = false;
        for (place, block) in body.blocks.iter().enumerate() {
            if place != entry {
                let mut defs = [NONE; UNITS];
                for &from in &preds[place] {
                    for (def, other) in defs.iter_mut().zip(&out[from]) {
                        *def = (*def).max(*other);
                    }
                }
                if defs != entering[place] {
                    entering[place] = defs;
                    changed = true;
                }
            }
            let now = leaving(block, &entering[place]);
            if now != out[place] {
                out[place] = now;
                changed = true;
            }
        }
        if !changed {
            return entering;
        }
    }
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::RegId;

    use super::*;
    use crate::model::lir::{Block, LirBlock};

    fn xmm(register: RegId) -> Loc {
        Loc::Reg(Reg { register, width: 16 })
    }

    fn gpr() -> Loc {
        Loc::Reg(Reg { register: RegId::EAX, width: 4 })
    }

    fn insn(
        at: i64,
        name: &str,
        op: Operation,
        dest: Loc,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests: vec![dest], sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    fn body(blocks: Vec<(i64, Vec<Arc<Insn>>, Vec<i64>)>) -> LirBody {
        let blocks: Vec<LirBlock> = blocks
            .into_iter()
            .map(|(at, insns, succ)| {
                let mut block = LirBlock::new(at, insns);
                block.succ = succ.into();
                block
            })
            .collect();
        let entry = blocks[0].at;
        let blocks = crate::model::lir::shared(blocks);
        let mut body = LirBody::new("f", entry, blocks, Default::default(), Default::default());
        body.bits = 64;
        body
    }

    fn names(body: &LirBody) -> Vec<String> {
        body.blocks
            .iter()
            .flat_map(|block: &Block| block.insns.iter())
            .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            .collect()
    }

    fn run(body: LirBody) -> LirBody {
        BreakFalseDeps.transform(body).expect("runs")
    }

    /// `sfm16_float_i2f`, m64 -O2: `cvtsi2ss xmm0, eax` right after another
    /// write of `xmm0` took the old value's latency (the pin: one `xorps` in
    /// front of each of the 9 converts, ours none).
    #[test]
    fn a_convert_right_after_a_write_of_its_register_gets_a_xorps() {
        let first = insn(0, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()]);
        let again = insn(1, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()]);
        let after = run(body(vec![(0, vec![first, again], vec![])]));
        assert_eq!(names(&after), ["cvtsi2ss", "xorps", "cvtsi2ss"]);
    }

    /// A register nothing wrote has no dependence to break.
    #[test]
    fn a_convert_into_a_register_nothing_wrote_is_left() {
        let one = insn(0, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM1), vec![gpr()]);
        assert_eq!(names(&run(body(vec![(0, vec![one], vec![])]))), ["cvtsi2ss"]);
    }

    /// 64 instructions or more since the last write is clear.
    #[test]
    fn a_convert_the_register_was_last_written_far_before_is_left() {
        let mut insns = vec![insn(0, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()])];
        for at in 1..=64 {
            insns.push(insn(at, "mov", Operation::Move, gpr(), vec![Loc::Reg(Reg { register: RegId::ECX, width: 4 })]));
        }
        insns.push(insn(65, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()]));
        assert_eq!(names(&run(body(vec![(0, insns, vec![])]))).iter().filter(|n| *n == "xorps").count(), 0);
    }

    /// Reading the destination is a true dependence: `cvtss2sd xmm0, xmm0`.
    #[test]
    fn a_convert_that_reads_its_destination_is_left() {
        let first = insn(0, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()]);
        let same = insn(1, "cvtss2sd", Operation::Extend, xmm(RegId::XMM0), vec![xmm(RegId::XMM0)]);
        assert_eq!(names(&run(body(vec![(0, vec![first, same], vec![])]))), ["cvtsi2ss", "cvtss2sd"]);
    }

    /// The write that reaches a block through its predecessor counts.
    #[test]
    fn a_write_in_the_predecessor_block_reaches_the_convert() {
        let first = insn(0, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()]);
        let again = insn(1, "cvtsi2ss", Operation::Extend, xmm(RegId::XMM0), vec![gpr()]);
        let after = run(body(vec![(0, vec![first], vec![1]), (1, vec![again], vec![])]));
        assert_eq!(names(&after), ["cvtsi2ss", "xorps", "cvtsi2ss"]);
    }
}
