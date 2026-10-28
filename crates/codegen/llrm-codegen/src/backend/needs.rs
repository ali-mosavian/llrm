//! What a body contains that a peephole sub-pass could act on, so that
//! `peephole` skips a sub-pass whose subject is absent.
//!
//! # How it works
//! Each instruction has a set of bits, and a body has the union of its
//! instructions'. A sub-pass names the bits it needs; `gated` runs it only if
//! the body has all of them. The shape is that of the MIR passes' `Depends`,
//! for the machine passes.
//!
//! ```text
//!   COPY        register-to-register mov       machinecp
//!   UNHELD      cell read via a reg32          upperzero
//!   EXACT_CELL  a cell lowering proved exact   exactaddress
//! ```
//!
//! A bit is the sub-pass's own candidate test (
//! `upperzero::unheld`, `exactaddress::cell_of`), so "can this pass match" has
//! one definition. `Contents` keeps the bits of each block for as long as the
//! block is the same list of instructions (`Insns::same_as`), so a pass that
//! changes one block recomputes one block.
//!
//! `LLRM_CHECK_NEEDS` runs a skipped pass anyway and asserts it changed
//! nothing.
//!
//! # Example
//! No honest example: the bits steer which sub-pass of `peephole` runs and
//! change no instruction. `LLRM_CHECK_NEEDS` checks a skipped pass against its
//! bits.
//!
//! # Used by
//! `peephole`.

use llrm_lir::registers::Regs;

use crate::backend::{exactaddress, upperzero};
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, Insns, LirBody};

/// A register-to-register `mov`: what copy propagation forwards and register
/// thrashing renames.
pub const COPY: u8 = 1;
/// A cell read through a 32-bit register no value holds.
pub const UNHELD: u8 = 4;
/// A cell lowering proved exact.
pub const EXACT_CELL: u8 = 8;

/// The bits `one` has.
fn of(
    regs: Regs,
    one: &Insn,
) -> u8 {
    let mut bits = 0;
    if let Some(what) = &one.what {
        if let (Operation::Move, Some("mov"), [Loc::Reg(_)], [Loc::Reg(_)]) =
            (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
        {
            bits |= COPY;
        }
        if what.dests.iter().chain(&what.sources).any(|operand| matches!(operand, Loc::Mem(_))) {
            if upperzero::wide_roots(regs, one) != 0 {
                bits |= UNHELD;
            }
            if exactaddress::cell_of(regs, one).is_some() {
                bits |= EXACT_CELL;
            }
        }
    }
    bits
}

/// `pass` of `body`, or `body` where it lacks something `needs` names.
pub fn gated(
    seen: &mut Contents,
    needs: u8,
    body: LirBody,
    pass: impl FnOnce(LirBody) -> Result<LirBody, String>,
) -> Result<LirBody, String> {
    if seen.has(&body, needs) {
        return pass(body);
    }
    if llrm_support::env_flag!("LLRM_CHECK_NEEDS") {
        let done = pass(body.clone())?;
        assert!(done == body, "a pass skipped for lack of {needs:#x} changed the body");
    }
    Ok(body)
}

/// The bits of each block of a body, as of the instruction lists they were
/// worked out from.
#[derive(Default)]
pub struct Contents {
    blocks: Vec<(Insns, u8)>,
}

impl Contents {
    /// Whether `body` has all of `needs`, in any of its blocks.
    pub fn has(
        &mut self,
        body: &LirBody,
        needs: u8,
    ) -> bool {
        let regs = body.regs();
        let same = self.blocks.len() == body.blocks.len()
            && self.blocks.iter().zip(&body.blocks).all(|((held, _), block)| held.same_as(&block.insns));
        if !same {
            let was = std::mem::take(&mut self.blocks);
            self.blocks = body
                .blocks
                .iter()
                .enumerate()
                .map(|(at, block)| match was.get(at) {
                    Some((held, bits)) if held.same_as(&block.insns) => (held.clone(), *bits),
                    _ => (block.insns.clone(), block.insns.iter().fold(0, |bits, one| bits | of(regs, one))),
                })
                .collect();
        }
        self.blocks.iter().fold(0, |all, (_, bits)| all | bits) & needs == needs
    }
}
