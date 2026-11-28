//! `lsr.l #4,%d2; cmpi.l #15,%d1; bhi` is `lsr.l #4,%d2; bne`: a compare of
//! `v` with `2^s - 1` asks whether `v >> s` is not zero, and the `lsr` that
//! made `v >> s` already set the codes for that.
//!
//! # How it works
//! [`reuse_shift_flags`] runs after selection, before allocation. For a
//! `cmpi.l #(2^s-1),v` it finds the `lsr.l #s` of `v` earlier in the block with
//! nothing between that sets the codes or calls, and the readers of the codes
//! after the compare, up to the next setter. When every reader is `bhi` or
//! `bls` (unsigned greater, lower or same), the compare goes and they become
//! `bne` and `beq`: `v - (2^s - 1)` is positive exactly when `v >> s` is not
//! zero.
//!
//! DEVIATE ([`Options::reuse_shift_flags`], exit 2026-11-20): the pin has no
//! such step; instcombine turns `(v >> s) != 0` into `v > 2^s - 1` and the
//! backend keeps the compare (hex -Os at 68020: `cmpi.l #15` is 6 B, gcc's
//! loop ends on the `lsr`). Reason: best of gcc and clang is the goal, and
//! `while (v >>= s)` digit loops are common. Off for `Options::PINNED`.
//!
//! # See also
//! `X86InstrInfo::optimizeCompareInstr` (X86InstrInfo.cpp L5473, pin
//! d1106deb71cc), which reuses the flags of an instruction that made the value.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::mfunc::MFunction;

pub fn reuse_shift_flags(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            if let Some(readers) = reusable(&block.insts, at) {
                for reader in readers {
                    let renamed = match block.insts[reader].kind {
                        Kind::Op("Bhi8") => "Bne8",
                        _ => "Beq8",
                    };
                    block.insts[reader].kind = Kind::Op(renamed);
                }
                block.insts.remove(at);
            } else {
                at += 1;
            }
        }
    }
}

/// The readers to rename, when the compare at `at` is redundant.
fn reusable(
    insts: &[Inst],
    at: usize,
) -> Option<Vec<usize>> {
    let Kind::Op("CMP32di") = insts[at].kind else { return None };
    let [Operand::Imm(bound), Operand::Use(Reg::Virt(v))] = insts[at].operands[..] else { return None };
    let shift = u32::try_from(bound.checked_add(1)?).ok().filter(|n| n.is_power_of_two())?.trailing_zeros();
    // the shift that made `v >> s`, with nothing after it that sets the codes
    let mut found = None;
    for (index, before) in insts[..at].iter().enumerate().rev() {
        if matches!(before.kind, Kind::Op("LSR32di"))
            && matches!(
                before.operands[..],
                [Operand::Def(_), Operand::Use(Reg::Virt(src)), Operand::Imm(s)] if src == v && i64::from(shift) == s
            )
        {
            found = Some(index);
            break;
        }
        if before.sets_flags() || before.call {
            return None;
        }
    }
    found?;
    // the readers of the codes after the compare: only `bhi` and `bls`
    let mut readers = Vec::new();
    for (index, after) in insts.iter().enumerate().skip(at + 1) {
        if after.reads_flags() && !after.call {
            match after.kind {
                Kind::Op("Bhi8" | "Bls8") => readers.push(index),
                _ => return None,
            }
        }
        if after.sets_flags() || after.call {
            break;
        }
    }
    if readers.is_empty() { None } else { Some(readers) }
}
