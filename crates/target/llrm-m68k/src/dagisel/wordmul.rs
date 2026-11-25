//! `muls.w (%an), %dm` and its three relatives: the memory-source bases of
//! the `(An)+` twins [`super::indexed`] makes of them.
//!
//! # How it works
//! The pin's word multiply and divide have a data register and an immediate
//! source only (`M68kInstrArithmetic.td`, `MxDiMuOp`). A base row here is the
//! pin's register row with the source read through `(An)`: its text and
//! encoding differ in the printer method and the mode field, its description
//! in the class of the last operand (the `(An)` row of `add.w` has it).
//! [`super::indexed`] takes the `(An)+` twin from it as from any base, and
//! [`crate::foldload`] folds a word load into it.
//!
//! DEVIATE: the pin selects no memory source for them. Reason: gcc folds the
//! post-incremented word into `muls.w (%an)+, %dm`, which is 4 clk shorter
//! than the load that fed the register (68000). Exit: none.

use std::sync::LazyLock;

use llrm_dagisel::instr::InstrDesc;

use super::asm::{AsmOperand, asm};
use super::enc::{Piece, encoding};
use super::generated::instrinfo;

/// The base row and the pin's register row it comes from.
pub const BASES: [(&str, &str); 4] = [
    ("SMULd32j16", "SMULd32d16"),
    ("UMULd32j16", "UMULd32d16"),
    ("SDIVd32j16", "SDIVd32d16"),
    ("UDIVd32j16", "UDIVd32d16"),
];

/// The bits the memory operand of every row reads.
pub const WIDTH: &str = "16";

/// The `(An)+` twin the register row `user` has.
pub fn twin_stem(user: &str) -> Option<String> {
    BASES.iter().find(|row| row.1 == user).map(|row| row.0.replace("j16", "o16"))
}

fn register_row(base: &str) -> Option<&'static str> {
    BASES.iter().find(|row| row.0 == base).map(|row| row.1)
}

pub fn base_asm(base: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    static ROWS: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
        BASES
            .iter()
            .map(|&(base, register)| {
                let &(_, text, operands) = asm(register).expect("a printed row");
                let kept: Vec<AsmOperand> = operands
                    .iter()
                    .map(|one| AsmOperand {
                        name: one.name,
                        at: one.at,
                        method: if one.name == "opd" { "printARI16Mem" } else { one.method },
                    })
                    .collect();
                (base, text, &*Box::leak(kept.into_boxed_slice()))
            })
            .collect()
    });
    register_row(base)?;
    ROWS.iter().find(|row| row.0 == base)
}

pub fn base_encoding(base: &str) -> Option<(u16, &'static [Piece])> {
    static ROWS: LazyLock<Vec<(&'static str, u16, &'static [Piece])>> = LazyLock::new(|| {
        BASES
            .iter()
            .map(|&(base, register)| {
                let &(_, width, pieces) = encoding(register).expect("an encoded row");
                let mut pieces = pieces.to_vec();
                for piece in &mut pieces {
                    if matches!(piece, Piece::Const { lo: 3, width: 3, value: 0 }) {
                        *piece = Piece::Const { lo: 3, width: 3, value: 2 };
                    }
                }
                (base, width, &*Box::leak(pieces.into_boxed_slice()))
            })
            .collect()
    });
    register_row(base)?;
    ROWS.iter().find(|row| row.0 == base).map(|row| (row.1, row.2))
}

/// The description of a base row: the register row's, with the last operand
/// of `add.w (%an), %dm`.
pub fn base_desc(base: &str) -> Option<&'static InstrDesc> {
    static ROWS: LazyLock<Vec<(&'static str, InstrDesc)>> = LazyLock::new(|| {
        let described = |name| instrinfo::desc(super::opcodes::code_of(name).expect("a pin row")).expect("described");
        let memory = described("ADD16dj");
        BASES
            .iter()
            .map(|&(base, register)| {
                let pin = described(register);
                let mut operands = pin.operands.to_vec();
                let last = operands.len() - 1;
                operands[last] = memory.operands[memory.operands.len() - 1];
                (base, InstrDesc { operands: Box::leak(operands.into_boxed_slice()), ..*pin })
            })
            .collect()
    });
    ROWS.iter().find(|row| row.0 == base).map(|row| &row.1)
}
