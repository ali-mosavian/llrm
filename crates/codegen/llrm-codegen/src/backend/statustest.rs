//! An x87 compare read from the status word: `fnstsw ax; test ah, mask; jcc`
//! for `fnstsw ax; sahf; jcc`. DEVIATE X87-FCMP-AH (owner x87, exit
//! 2026-11-20; no pin pass: the pin's `SAHF` is kept everywhere). gcc's
//! `ix86_expand_fp_compare` makes the same choice; its code is not translated.
//!
//! # How it works
//! After `fucom`, `fnstsw ax` puts C0 in bit 0 of `ah`, C2 in bit 2 and C3 in
//! bit 6; `sahf` copies those into CF, PF and ZF. A condition on one flag, or
//! on CF and ZF together (`ja`, `jbe`), is one `test ah, mask` and a `je` or
//! `jne`: `ja` is `test ah, 0x41; je`. A `sahf` followed by exactly one flag
//! reader, a `jcc`, `setcc` or `cmovcc` of those conditions with nothing
//! else reading a flag after it, becomes the test. A `sahf` several readers
//! share (`==` is `je` and `jnp`) stays.
//!
//! The test is one clock on a 486 (`test r8, imm`) against two for `sahf`, and
//! two bytes longer: made where the target prices `sahf` above a register-
//! immediate ALU operation, and not where bytes are held (-O1, size).
//!
//! # Example
//! `fcomp; fnstsw ax; sahf; jbe L` (`x_ld_abs` -O1, 256 executions):
//!
//! ```text
//!   fnstsw ax                fnstsw ax
//!   sahf              =>     test ah, 0x41
//!   jbe L                    jne L
//! ```
//!
//! # Runs
//! After allocation, before the peephole optimizer, on a target whose profile
//! prices `sahf`.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_ARITHMETIC_LANES, _flag_effect, _flags_live_out, _skippable_nothing};
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct StatusWordTest;

impl LIRTransform for StatusWordTest {
    fn class_name(&self) -> &'static str {
        "StatusWordTest"
    }

    fn name(&self) -> &str {
        "statustest"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(tested(&body))
    }
}

/// The flag reader a condition suffix names, split as `(family, condition)`.
fn split(name: &str) -> Option<(&'static str, &str)> {
    if let Some(cc) = name.strip_prefix("cmov") {
        Some(("cmov", cc))
    } else if let Some(cc) = name.strip_prefix("set") {
        Some(("set", cc))
    } else {
        name.strip_prefix('j').map(|cc| ("j", cc))
    }
}

/// The bits of `ah` a condition of `sahf`'s flags reads, and whether the
/// condition holds when the masked bits are all clear (`e`) or not (`ne`).
fn mask_of(condition: &str) -> Option<(i64, &'static str)> {
    // CF = C0 (bit 0), PF = C2 (bit 2), ZF = C3 (bit 6).
    Some(match condition {
        "a" | "nbe" => (0x41, "e"),
        "be" | "na" => (0x41, "ne"),
        "ae" | "nb" | "nc" => (0x01, "e"),
        "b" | "c" | "nae" => (0x01, "ne"),
        "e" | "z" => (0x40, "ne"),
        "ne" | "nz" => (0x40, "e"),
        "p" | "pe" => (0x04, "ne"),
        "np" | "po" => (0x04, "e"),
        _ => return None,
    })
}

/// `body` with each `sahf` that one flag reader follows made a `test`.
pub fn tested(body: &LirBody) -> LirBody {
    let regs = body.regs();
    let live_out = _flags_live_out(body);
    let mut changed = false;
    let blocks: Vec<_> = body
        .blocks
        .iter()
        .map(|block| {
            let insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let real: Vec<usize> = (0..insns.len()).filter(|&at| !_skippable_nothing(&insns[at])).collect();
            let mut out = insns.clone();
            let mut any = false;
            for (nth, &at) in real.iter().enumerate() {
                let Some(what) = insns[at].what.as_ref() else { continue };
                if what.op != Operation::Nothing || what.name.as_deref() != Some("sahf") {
                    continue;
                }
                let Some(&reader) = real.get(nth + 1) else { continue };
                let Some(read) = insns[reader].what.as_ref() else { continue };
                if read.indirect {
                    continue;
                }
                let Some((family, condition)) = split(read.name.as_deref().unwrap_or("")) else { continue };
                let Some((mask, holds)) = mask_of(condition) else { continue };
                // Nothing after the reader may read a flag the `test` leaves
                // other than `sahf` did.
                let mut pending = *_ARITHMETIC_LANES;
                let mut shared = false;
                for &later in &real[nth + 2..] {
                    let (reads, writes) = _flag_effect(regs, body.bits, &insns[later]);
                    if !reads.and(&_ARITHMETIC_LANES).is_empty() {
                        shared = true;
                        break;
                    }
                    pending = pending.minus(&writes);
                    if pending.is_empty() {
                        break;
                    }
                }
                if shared
                    || (!pending.is_empty()
                        && live_out.entering.get(&block.at).is_some_and(|live| !live.and(&pending).is_empty()))
                {
                    continue;
                }
                let test = Semantics {
                    sources: vec![
                        Loc::Reg(Reg { register: RegId::AH, width: 1 }),
                        Loc::Imm(Imm { value: mask, width: 1, address: None }),
                    ],
                    name: Some("test".into()),
                    ..Semantics::new(Operation::Compare)
                };
                out[at] = Arc::new(Insn { what: Some(test), ..(*insns[at]).clone() });
                let renamed = Semantics { name: Some(format!("{family}{holds}").into()), ..read.clone() };
                out[reader] = Arc::new(Insn { what: Some(renamed), ..(*insns[reader]).clone() });
                any = true;
            }
            if any {
                changed = true;
                block.with_insns(out)
            } else {
                block.clone()
            }
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `condition` holds in the flags `sahf` makes of `ah`.
    fn after_sahf(
        condition: &str,
        ah: i64,
    ) -> bool {
        let (cf, pf, zf) = (ah & 1 != 0, ah & 4 != 0, ah & 0x40 != 0);
        match condition {
            "a" | "nbe" => !cf && !zf,
            "be" | "na" => cf || zf,
            "ae" | "nb" | "nc" => !cf,
            "b" | "c" | "nae" => cf,
            "e" | "z" => zf,
            "ne" | "nz" => !zf,
            "p" | "pe" => pf,
            "np" | "po" => !pf,
            other => panic!("{other}"),
        }
    }

    /// Every condition a `sahf` reader may name holds after `test ah, mask` and
    /// the `e` or `ne` it becomes exactly where it held after `sahf`, for every
    /// status word the compare leaves (C0, C2 and C3 in bits 0, 2 and 6): a
    /// wrong mask turns an unordered compare into a branch taken.
    #[test]
    fn test_a_status_word_test_decides_each_condition_as_sahf_did() {
        for condition in
            ["a", "nbe", "be", "na", "ae", "nb", "nc", "b", "c", "nae", "e", "z", "ne", "nz", "p", "pe", "np", "po"]
        {
            let (mask, holds) = mask_of(condition).expect(condition);
            for ah in (0..8).map(|bits| (bits & 1) | ((bits >> 1 & 1) << 2) | ((bits >> 2 & 1) << 6)) {
                let zero = ah & mask == 0;
                assert_eq!(
                    zero == (holds == "e"),
                    after_sahf(condition, ah),
                    "{condition} at ah {ah:#x}, mask {mask:#x}"
                );
            }
        }
    }

    #[test]
    fn test_a_condition_no_single_test_decides_is_left_to_sahf() {
        for condition in ["l", "g", "le", "ge", "o", "no", "s", "ns", "mp", "cxz"] {
            assert_eq!(mask_of(condition), None, "{condition}");
        }
    }
}
