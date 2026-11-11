//! `lsl #1, Dn` is `add Dn, Dn` when nothing reads the overflow flag after
//! it: the same result, carry, extend, negative and zero codes, two clocks
//! fewer on the 68020 and no byte more.
//!
//! # How it works
//! [`shifts_to_adds`] rewrites each shift of a register by one to the `add` of
//! the register to itself where the readers of the codes after it test no
//! overflow ([`crate::cmpelim::flags_read_after`]); a `lsl` leaves V clear and
//! an `add` sets it by the sum. Run after [`crate::cmpelim`], which reuses the
//! shift's codes for a `tst` the `add` would not leave.
//!
//! DEVIATE: the pin selects the shift. Reason: gcc's pattern for a shift by
//! one is the add (crc16 -O2: 4162 -> 4018 clk on the 68000, 1875 -> 1731 on
//! the 68020, same bytes). Exit: 2026-11-12.

use llrm_spike_mf::{Kind, Operand};

use crate::ccr::flag_bit;
use crate::cmpelim::flags_read_after;
use crate::dagisel::local;
use crate::mfunc::{MFunction, op_inst};

/// The `add` of a register to itself for a shift by one of the same size.
fn add_for(name: &str) -> Option<&'static str> {
    match name {
        "SHL8di" => Some("ADD8dd"),
        "SHL16di" => Some("ADD16dd"),
        "SHL32di" => Some("ADD32dd"),
        _ => None,
    }
}

pub fn shifts_to_adds(function: &mut MFunction) {
    let overflow = flag_bit("V");
    for block in &mut function.blocks {
        for at in 0..block.insts.len() {
            let Kind::Op(name) = block.insts[at].kind else { continue };
            let Some(target) = add_for(name) else { continue };
            let [Operand::Def(def), Operand::Use(source), Operand::Imm(1)] = block.insts[at].operands[..] else {
                continue;
            };
            if flags_read_after(&block.insts, at).is_none_or(|needed| needed & overflow != 0) {
                continue;
            }
            let Some(desc) = local::code_of(target).and_then(local::desc) else { continue };
            block.insts[at] =
                op_inst(target, desc, vec![Operand::Def(def), Operand::Use(source), Operand::Use(source)]);
        }
    }
}
