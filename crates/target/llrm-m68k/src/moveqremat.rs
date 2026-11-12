//! A `moveq` constant is made again where it is read, not held in a register.
//!
//! # How it works
//! [`rematerializable_constants`] marks each `MOVI32ri` of a value `moveq`
//! takes (-128 to 127) rematerializable before allocation. The spiller and the
//! split editor then make the constant again at a use (2 bytes, 4 clocks)
//! instead of spilling it, copying it or keeping it in a register across a
//! call.
//!
//! DEVIATE M68K-MOVEQ-REMAT: the pin's `MOVI32ri` is neither
//! `isReMaterializable` nor `isAsCheapAsAMove`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kInstrData.td>),
//! so it holds every constant in a register or a frame slot. Reason: a `moveq`
//! is smaller than any copy or reload of it, and gcc makes it again. Only the
//! `moveq` range: a 6-byte `move.l #imm` made again costs more than the 4-byte
//! reload it replaces. Off for `Options::PINNED`. Exit: none.

use llrm_spike_mf::{Kind, Operand};

use crate::mfunc::MFunction;

/// The values `moveq` loads.
const MOVEQ: std::ops::RangeInclusive<i64> = -128..=127;

/// How many constants it marked.
pub fn rematerializable_constants(function: &mut MFunction) -> usize {
    let mut marked = 0;
    for block in &mut function.blocks {
        for one in &mut block.insts {
            if one.kind == Kind::Op("MOVI32ri")
                && matches!(
                    one.operands[..],
                    [Operand::Def(_), Operand::Imm(value)] if MOVEQ.contains(&value)
                )
            {
                one.rematerializable = true;
                marked += 1;
            }
        }
    }
    marked
}
