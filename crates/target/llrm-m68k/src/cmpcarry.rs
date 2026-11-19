//! `add.w %d0, %d0; tst.w <old %d0>; bpl` is `add.w %d0, %d0; bcc`: an `add` of
//! a register to itself leaves in C the bit it shifted out, which is the sign
//! of the old value at that size, so the `tst` of the old value and its copy
//! (the `add` writes the register the `tst` reads) both go.
//!
//! # How it works
//! [`reuse_shift_out_flags`] runs after the select is a branch and the
//! instructions of its arms are sunk (`machine_sink`), before allocation: while
//! the select is a `CMOV`, the arm's `xor` sits between the `add` and the
//! `tst` and clobbers C. For a `tst` of a register `v` at size `s` it finds the
//! `add` of `v` to itself at size `s` earlier in the block with nothing between
//! that sets or reads the codes or calls, and the readers of the codes after
//! the `tst`, up to the next setter. When every reader tests the sign (`bpl`,
//! `bge`: clear; `bmi`, `blt`: set), the `tst` goes and they become `bcc` and
//! `bcs`: `tst` clears V, so those four read N alone, and N of `tst v` is the
//! C the `add` left.
//!
//! DEVIATE ([`Options::reuse_shift_out_flags`], exit 2026-11-17): the pin has
//! no m68k `optimizeCompareInstr`, and the x86 one reuses the codes of the
//! instruction that made the compared value, not of an instruction that
//! consumed it. Reason: best of gcc and clang is the goal; crc16's bit loop
//! tests the bit it then shifts away. Off for `Options::PINNED`.
//!
//! # See also
//! `X86InstrInfo::optimizeCompareInstr` (X86InstrInfo.cpp L5473, pin
//! d1106deb71cc). [`crate::cmpshift`], the same step for `lsr` and a bound.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::cmpelim::tested;
use crate::mfunc::MFunction;

pub fn reuse_shift_out_flags(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            if let Some(readers) = reusable(&block.insts, at) {
                for (reader, renamed) in readers {
                    block.insts[reader].kind = Kind::Op(renamed);
                }
                block.insts.remove(at);
            } else {
                at += 1;
            }
        }
    }
}

/// The `add` row that doubles a register at `size` (`l`, `w` or `b`).
fn doubles(
    name: &str,
    size: char,
) -> bool {
    matches!(
        (name, size),
        ("ADD32dd", 'l') | ("ADD16dd", 'w') | ("ADD8dd", 'b')
    )
}

/// The readers to rename, with their new rows, when the `tst` at `at` is
/// redundant.
fn reusable(
    insts: &[Inst],
    at: usize,
) -> Option<Vec<(usize, &'static str)>> {
    let (v, size) = tested(&insts[at])?;
    let size = size?;
    let mut found = false;
    for before in insts[..at].iter().rev() {
        if let Kind::Op(name) = before.kind
            && doubles(name, size)
            && matches!(
                before.operands[..],
                [Operand::Def(_), Operand::Use(Reg::Virt(a)), Operand::Use(Reg::Virt(b))] if a == v && b == v
            )
        {
            found = true;
            break;
        }
        if before.sets_flags() || before.reads_flags() || before.call {
            return None;
        }
    }
    if !found {
        return None;
    }
    let mut readers = Vec::new();
    for (index, after) in insts.iter().enumerate().skip(at + 1) {
        if after.reads_flags() && !after.call {
            match after.kind {
                Kind::Op("Bpl8" | "Bge8") => readers.push((index, "Bcc8")),
                Kind::Op("Bmi8" | "Blt8") => readers.push((index, "Bcs8")),
                _ => return None,
            }
        }
        if after.sets_flags() || after.call {
            break;
        }
    }
    if readers.is_empty() { None } else { Some(readers) }
}
