//! `adda.w` and `suba.w`: add or subtract a data register's low word,
//! sign-extended, to an address register, leaving the codes alone.
//!
//! # How it works
//! Two rows, `ADDA16r` and `SUBA16r`, with the operands of the pin's
//! `ADD32dd` (destination, tied source, source) but an address register for
//! the first two, a word register for the third and no codes written. The
//! encoding is `1101 an 011 000 dn` (`adda.w`) and `1001 an 011 000 dn`
//! (`suba.w`). [`crate::sextacc`] makes them before allocation.
//!
//! DEVIATE C-ADDA (owner m68k, exit 2026-12-12: the pin gains the def, or the
//! rows go): the pin's M68k has no `ADDA`/`SUBA` (`M68kInstrArithmetic.td`:
//! "ADDA [~]"), so a sign-extended word added to a sum is `ext.l` and `add.l`.
//! Reason: best of gcc and clang is the goal; gcc keeps such a sum in an
//! address register and adds the word with `adda.w`.

use std::sync::LazyLock;

use llrm_dagisel::instr::InstrDesc;

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::ids::reg as pin;
use super::generated::instrinfo::{self, flag};
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the rows, above the compares'.
pub const FIRST: u16 = 0xF600;

/// The rows: name, mnemonic and the opcode's top nibble.
const ROWS: [(&str, &str, u64); 2] =
    [("ADDA16r", "adda.w\t$src, $dst", 0b1101), ("SUBA16r", "suba.w\t$src, $dst", 0b1001)];

pub fn code_of(name: &str) -> Option<u16> {
    ROWS.iter().position(|row| row.0 == name).map(|at| FIRST + at as u16)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    ROWS.get(usize::from(at)).map(|row| row.0)
}

static DESCS: LazyLock<Vec<&'static InstrDesc>> = LazyLock::new(|| {
    let code = MACHINE_OPCODES.iter().find(|(_, name)| *name == "ADD32dd").map(|(code, _)| *code).expect("a pin row");
    let base = instrinfo::desc(code).expect("a described row");
    ROWS.iter()
        .map(|_| {
            let mut operands = base.operands.to_vec();
            operands[0].reg_class = pin::AR32RegClassID as i16;
            operands[1].reg_class = pin::AR32RegClassID as i16;
            operands[2].reg_class = pin::DR16RegClassID as i16;
            let flags = base.flags & !(flag::COMMUTABLE | flag::CONVERTIBLE_TO3_ADDR);
            &*Box::leak(Box::new(InstrDesc {
                operands: Box::leak(operands.into_boxed_slice()),
                implicit_defs: &[],
                flags,
                ..*base
            }))
        })
        .collect()
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    DESCS.get(usize::from(code.checked_sub(FIRST)?)).copied()
}

static ASM: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    static OPERANDS: [AsmOperand; 2] = [
        AsmOperand { name: "src", at: 2, method: "printOperand" },
        AsmOperand { name: "dst", at: 0, method: "printOperand" },
    ];
    ROWS.iter().map(|row| (row.0, row.1, &OPERANDS[..])).collect()
});

pub fn adda_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    ASM.iter().find(|row| row.0 == name)
}

static ENCODINGS: LazyLock<Vec<(&'static str, &'static [Piece])>> = LazyLock::new(|| {
    ROWS.iter()
        .map(|row| {
            let pieces = vec![
                Piece::Operand { lo: 0, width: 3, at: 2, from: 0, enc: Enc::Plain },
                Piece::Const { lo: 3, width: 3, value: 0 },
                Piece::Const { lo: 6, width: 3, value: 3 },
                Piece::Operand { lo: 9, width: 3, at: 0, from: 0, enc: Enc::Plain },
                Piece::Const { lo: 12, width: 4, value: row.2 },
            ];
            (row.0, &*Box::leak(pieces.into_boxed_slice()))
        })
        .collect()
});

pub fn adda_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    ENCODINGS.iter().find(|row| row.0 == name).map(|row| (16, row.1))
}
