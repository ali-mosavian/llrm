//! A compare of a byte with zero goes where the instruction that made the byte
//! set the flags.
//!
//! Translated from the zero-compare case of
//! `X86InstrInfo::optimizeCompareInstr`
//! ([X86InstrInfo.cpp#L5473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5473)),
//! which the pin's PeepholeOptimizer runs before allocation: `test %eax, %eax`
//! after the instruction that defined `%eax` is dropped, because that
//! instruction already set the flags the branch reads (the backward scan,
//! [L5547](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5547)).
//! A `MOV32r0` between the two is moved above the instruction that set the
//! flags
//! ([L5640](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5640),
//! [L5864](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5864)),
//! because `xor` clobbers the flags the compare would use.
//!
//! # How it works
//! A select is a branch over one block (`select::expand_selects`): the false
//! value is loaded in the head, before the branch. A load sets N and Z, so on
//! the 6502 that load stands between the `and` that made the tested byte and
//! the `cmp #0` the branch reads, and the compare cannot be dropped. Every
//! literal load here (`LDI`) is what `MOV32r0` is there: it has no input, so it
//! moves above the instruction that set the flags, and the compare is then
//! redundant.
//!
//! `and`, `ora` and `eor` set N and Z from their result and leave the carry
//! alone, which is what `isDefConvertible` asks of the instruction
//! ([X86InstrInfo.cpp#L5053](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L5053)).
//! The branch must read N and Z only (`beq`, `bne`, `bmi`, `bpl`): `cmp #0`
//! also sets the carry.
//!
//! ```text
//!  and #1                ldy #9
//!  ldy #9                and #1
//!  cmp #0         =>     bne L
//!  bne L
//! ```
//!
//! Not translated: the other flag sources (`adc`/`sbc` read and write the
//! carry; the transfers and shifts), a source in another block (the pin
//! scans back into a single predecessor), and a compare of a word.
//!
//! # Runs
//! In `alloc::emit_with`, before allocation, so that `copy_point` finds the
//! source as the instruction that sets the flags and places a phi copy above
//! it.

use llrm_spike_mf::{Inst, Kind, MFunction, Operand, Reg};

use crate::alloc::Class;
use crate::reg;

/// A branch that reads N or Z and nothing else.
fn reads_zero_or_sign(inst: &Inst) -> bool {
    inst.terminator && matches!(inst.kind, Kind::Op("BEQ" | "BNE" | "BMI" | "BPL"))
}

/// The instructions that set N and Z from the byte they leave in `a` and
/// leave the carry as it was (`isDefConvertible`).
fn converts(inst: &Inst) -> bool {
    matches!(inst.kind, Kind::Op("AND" | "ORA" | "EOR"))
        && matches!(
            inst.operands.first(),
            Some(Operand::Def(Reg::Phys(a))) if *a == reg::A
        )
}

fn defines(
    inst: &Inst,
    register: Reg,
) -> bool {
    inst.operands.iter().any(|o| matches!(o, Operand::Def(r) if *r == register))
}

fn reads(
    inst: &Inst,
    register: Reg,
) -> bool {
    inst.operands.iter().any(|o| matches!(o, Operand::Use(r) if *r == register))
}

fn mentions(
    inst: &Inst,
    register: Reg,
) -> bool {
    inst.operands.iter().any(|o| matches!(o, Operand::Def(r) | Operand::Use(r) if *r == register))
}

/// The `cmp #0` at `compare`, with its flags read by branches that read N and
/// Z only: the register it tests and the flags it defines.
fn zero_compare(
    insts: &[Inst],
    compare: usize,
) -> Option<(Reg, Operand)> {
    let inst = insts.get(compare).filter(|i| i.kind == Kind::Op("CMP") && i.sets_flags())?;
    let [flags @ Operand::Def(_), Operand::Use(tested), Operand::Imm(0)] = &inst.operands[..] else { return None };
    let after = &insts[compare + 1..];
    let readers_only_zero_and_sign = after.iter().all(|i| !i.reads_flags() || reads_zero_or_sign(i));
    let none_sets_flags = after.iter().all(|i| !i.sets_flags());
    (!after.is_empty() && readers_only_zero_and_sign && none_sets_flags).then(|| (*tested, flags.clone()))
}

/// The index of the instruction that made `tested`, the literal loads between
/// it and the compare (to move above it), when all that stands between is
/// copies of the same byte and those loads.
fn source(
    insts: &[Inst],
    compare: usize,
    tested: Reg,
) -> Option<(usize, Vec<usize>)> {
    let mut need = tested;
    let mut loads = Vec::new();
    for at in (0..compare).rev() {
        let inst = &insts[at];
        if defines(inst, need) {
            if inst.kind == Kind::Copy {
                let [Operand::Def(_), Operand::Use(from)] = &inst.operands[..] else { return None };
                need = *from;
                continue;
            }
            return (need == Reg::Phys(reg::A) && converts(inst)).then_some((at, loads));
        }
        match inst.kind {
            // Only a copy of the tested byte: any other copy becomes a load
            // once allocated (`ldx`, `lda`), which sets N and Z from its own
            // byte, so the branch would read that and not the byte the setter
            // made. x86's `mov` leaves the flags alone, which is what the pin's
            // backward scan relies on.
            Kind::Copy if reads(inst, need) => {}
            Kind::Op("LDI") => loads.push(at),
            _ => return None,
        }
    }
    None
}

fn drop_one(insts: &mut Vec<Inst>) -> bool {
    for compare in 0..insts.len() {
        let Some((tested, flags)) = zero_compare(insts, compare) else { continue };
        let Some((setter, loads)) = source(insts, compare, tested) else { continue };
        // a load moves above the setter only if nothing between reads or
        // writes what it defines
        let movable = loads
            .iter()
            .all(
                |&load| {
                    let Some(Operand::Def(defined)) = insts[load].operands.first() else { return false };
                    (setter..compare).filter(|&at| at != load).all(|at| !mentions(&insts[at], *defined))
                },
            );
        if !movable {
            continue;
        }
        // the loads in their order, the setter after them, the compare gone
        let moved: Vec<Inst> = loads.iter().rev().map(|&at| insts[at].clone()).collect();
        let mut gone: Vec<usize> = loads.clone();
        gone.push(compare);
        gone.sort_unstable();
        for at in gone.into_iter().rev() {
            insts.remove(at);
        }
        if !insts[setter].operands.contains(&flags) {
            insts[setter].operands.push(flags);
        }
        insts[setter].sets_flags = true;
        for (k, load) in moved.into_iter().enumerate() {
            insts.insert(setter + k, load);
        }
        return true;
    }
    false
}

/// Drops the compares of a byte with zero that the instruction which made the
/// byte already did; whether anything changed.
pub fn drop_zero_compares(function: &mut MFunction<Class>) -> bool {
    let mut changed = false;
    for block in &mut function.blocks {
        while drop_one(&mut block.insts) {
            changed = true;
        }
    }
    changed
}
