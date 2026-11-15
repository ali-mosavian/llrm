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

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::classsplit::takes_data;
use crate::mfunc::{Class, Inst, MFunction};

/// The values `moveq` loads.
const MOVEQ: std::ops::RangeInclusive<i64> = -128..=127;

/// How many constants it marked.
pub fn rematerializable_constants(function: &mut MFunction) -> usize {
    let mut marked = 0;
    for block in &mut function.blocks {
        for one in &mut block.insts {
            if is_moveq(one) {
                one.rematerializable = true;
                marked += 1;
            }
        }
    }
    marked += hold_in_data_registers(function);
    marked
}

fn is_moveq(inst: &Inst) -> bool {
    inst.kind == Kind::Op("MOVI32ri")
        && matches!(inst.operands[..], [Operand::Def(_), Operand::Imm(value)] if MOVEQ.contains(&value))
}

/// A value only `moveq` makes, read only where a data register does, is held
/// in one: `move.w #11, %a1` is 4 bytes where `moveq #11, %d1` is 2, and the
/// allocator tries the address registers first for a value of either kind
/// (DEVIATE M68K-FLEXORDER, `regalloc.rs`). The pin tries the data registers
/// first, so its `MOVI32ri` (`xr32`, no `MOVQ` at selection) lands in one. A
/// narrowed constant is made again where it is read, so it holds no register
/// the loop needs.
fn hold_in_data_registers(function: &mut MFunction) -> usize {
    let count = function.classes.len();
    let mut only_moveq = vec![true; count];
    let mut defined = vec![false; count];
    let mut fits = vec![true; count];
    for block in &function.blocks {
        if block.phis.iter().any(|phi| matches!(phi.result, Reg::Virt(_))) {
            for phi in &block.phis {
                if let Reg::Virt(v) = phi.result {
                    only_moveq[v as usize] = false;
                }
                for (_, incoming) in &phi.incoming {
                    if let Reg::Virt(v) = incoming {
                        fits[*v as usize] = false;
                    }
                }
            }
        }
        for inst in &block.insts {
            for (position, operand) in inst.operands.iter().enumerate() {
                match operand {
                    Operand::Def(Reg::Virt(v)) => {
                        defined[*v as usize] = true;
                        only_moveq[*v as usize] &= is_moveq(inst);
                    }
                    Operand::Use(Reg::Virt(v)) => {
                        fits[*v as usize] &= takes_data(inst.kind, position);
                    }
                    _ => {}
                }
            }
        }
    }
    let mut held = 0;
    for v in 0..count {
        if defined[v] && only_moveq[v] && fits[v] && function.classes[v] == Class::Xr32 {
            function.classes[v] = Class::Dr32;
            held += 1;
        }
    }
    held
}
