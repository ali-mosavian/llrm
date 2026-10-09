//! What a body contains that a peephole sub-pass could act on, so that the
//! driver skips a pass whose subject is absent: the shape of the MIR passes'
//! `Depends`, for the machine passes.
//!
//! A bit is the pass's own candidate test (`regthrash::_plain_copy`,
//! `upperzero::unheld`, `exactaddress::cell_of`), so the question "can this
//! pass match" has one definition. The bits of a block are kept for as long as
//! the block is the same list of instructions (`Insns::same_as`), as its other
//! facts are.
//! `LLRM_CHECK_NEEDS` runs a skipped pass anyway and asserts it changed
//! nothing.

use crate::backend::{exactaddress, upperzero};
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, Insns, LirBody};

/// A register-to-register `mov`: what copy propagation forwards and register
/// thrashing renames.
pub const COPY: u8 = 1;
/// An instruction the allocator inserted that defines a value: what
/// `secondary_bases` derives its candidates from.
pub const INSERTED: u8 = 2;
/// A cell read through a 32-bit register no value holds.
pub const UNHELD: u8 = 4;
/// A cell lowering proved exact.
pub const EXACT_CELL: u8 = 8;
/// A cell addressed through a value (`Mem::base`).
pub const BASED: u8 = 16;

/// The bits `one` has.
fn of(one: &Insn) -> u8 {
    let mut bits = 0;
    if one.inserted() && !one.defines.is_empty() {
        bits |= INSERTED;
    }
    if let Some(what) = &one.what {
        if let (Operation::Move, Some("mov"), [Loc::Reg(_)], [Loc::Reg(_)]) =
            (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
        {
            bits |= COPY;
        }
        if what
            .dests
            .iter()
            .chain(&what.sources)
            .any(|operand| matches!(operand, Loc::Mem(cell) if cell.base.is_some()))
        {
            bits |= BASED;
        }
        if what.dests.iter().chain(&what.sources).any(|operand| matches!(operand, Loc::Mem(_))) {
            if upperzero::unheld(one) != 0 {
                bits |= UNHELD;
            }
            if exactaddress::cell_of(one).is_some() {
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
    if llrm_support::env_set("LLRM_CHECK_NEEDS") {
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
                    _ => (block.insns.clone(), block.insns.iter().fold(0, |bits, one| bits | of(one))),
                })
                .collect();
        }
        self.blocks.iter().fold(0, |all, (_, bits)| all | bits) & needs == needs
    }
}
