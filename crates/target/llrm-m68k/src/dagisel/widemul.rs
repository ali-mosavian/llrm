//! `mulu.l <ea>, Dh:Dl` and `muls.l`: the 64-bit product of two 32-bit values.
//!
//! # How it works
//! Each is the pin's `mulu.l` / `muls.l` row (a data register source) with a
//! second result, the high half `Dh`, and the extension word's size bit set.
//! [`super::lowering`] makes `UMUL_LOHI` and `SMUL_LOHI` of an `i32` one of
//! them, so a widening multiply and the wide product built from it
//! (`expandMUL`) need no library call and no 16-bit pieces.
//!
//! DEVIATE M68K-WIDEMUL: the pin's M68k has no 64-bit-result multiply; it
//! marks `UMUL_LOHI` and `SMUL_LOHI` of `i32` `LibCall` on every CPU
//! (`M68kISelLowering.cpp` lines 102-107) and its only long multiplies are
//! `SMULd32d32` and `UMULd32d32` (`M68kInstrArithmetic.td` lines 735-736),
//! whose extension word has the size bit clear and no `Dh`. Reason: gcc emits
//! one `mulu.l ea, Dh:Dl`; ours was about 30 instructions. Selected by
//! [`super::hooks::Cpu::has_wide_multiply`]: the 68020, 68030 and 68040 (the
//! 68000 and 68010 have no long multiply, the 68060 traps on the 64-bit
//! result). Owner m68k. Exit: 2026-11-27.

use std::sync::LazyLock;

use llrm_dagisel::instr::{InstrDesc, OperandDesc};

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the widening multiplies, above the divide-remainders.
pub const FIRST: u16 = 0xF300;

/// `(name, the pin's row it derives from, mnemonic, signed)`.
const FORMS: [(&str, &str, &str, u64); 2] =
    [("UMUL64", "UMULd32d32", "mulu.l", 0), ("SMUL64", "SMULd32d32", "muls.l", 1)];

/// The operands: the low half, the high half (the results, in the order of
/// `UMUL_LOHI`'s), the multiplicand (tied to the low half) and the source.
pub const LOW: usize = 0;
pub const HIGH: usize = 1;
pub const MULTIPLICAND: usize = 2;
pub const SOURCE: usize = 3;

fn pin_code(name: &str) -> Option<u16> {
    MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code)
}

pub fn names() -> impl Iterator<Item = &'static str> {
    FORMS.iter().map(|one| one.0)
}

pub fn code_of(name: &str) -> Option<u16> {
    FORMS.iter().position(|one| one.0 == name).map(|at| FIRST + at as u16)
}

pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    FORMS.get(usize::from(at)).map(|one| one.0)
}

static DESCS: LazyLock<Vec<&'static InstrDesc>> = LazyLock::new(|| {
    FORMS
        .iter()
        .map(|&(_, base, _, _)| {
            let base = instrinfo::desc(pin_code(base).expect("a pin row")).expect("a described row");
            // the pin's [dst, src (tied to dst), opd] with the high half second
            let dst = base.operands[0];
            let mut operands = vec![dst, OperandDesc { tied_to: -1, ..dst }];
            operands.push(OperandDesc { tied_to: LOW as i8, ..base.operands[1] });
            operands.push(base.operands[2]);
            // `Dh:Dl` sets the codes from the 64-bit result, which no reader of
            // the pin's 32-bit row expects: the row says it sets none
            let implicit_defs: Vec<_> =
                base.implicit_defs.iter().copied().filter(|reg| *reg != super::generated::ids::reg::CCR).collect();
            &*Box::leak(Box::new(InstrDesc {
                num_operands: 4,
                num_defs: 2,
                operands: Box::leak(operands.into_boxed_slice()),
                implicit_defs: Box::leak(implicit_defs.into_boxed_slice()),
                ..*base
            }))
        })
        .collect()
});

pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    DESCS.get(usize::from(code.checked_sub(FIRST)?)).copied()
}

static ASM: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    FORMS
        .iter()
        .map(|&(name, _, mnemonic, _)| {
            let text: &'static str = Box::leak(format!("{mnemonic}\t$opd, $hi:$lo").into_boxed_str());
            let operands: &'static [AsmOperand] = Box::leak(Box::new([
                AsmOperand { name: "opd", at: SOURCE as u8, method: "printOperand" },
                AsmOperand { name: "hi", at: HIGH as u8, method: "printOperand" },
                AsmOperand { name: "lo", at: LOW as u8, method: "printOperand" },
            ]));
            (name, text, operands)
        })
        .collect()
});

pub fn widemul_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    ASM.iter().find(|row| row.0 == name)
}

const fn field(
    lo: u16,
    width: u8,
    at: u8,
) -> Piece {
    Piece::Operand { lo, width, at, from: 0, enc: Enc::Plain }
}

const fn bits(
    lo: u16,
    width: u8,
    value: u64,
) -> Piece {
    Piece::Const { lo, width, value }
}

/// `0100 1100 00 mode reg` and the extension word `0 Dl S 1 0000000 0 Dh`:
/// `muls.l` / `mulu.l` with the size bit set and `Dh` the high register.
const fn pieces(signed: u64) -> [Piece; 9] {
    [
        field(0, 3, SOURCE as u8),
        bits(3, 3, 0),
        bits(6, 10, 304),
        field(16, 3, HIGH as u8),
        bits(19, 7, 0),
        bits(26, 1, 1),
        bits(27, 1, signed),
        field(28, 3, LOW as u8),
        bits(31, 1, 0),
    ]
}

static PIECES: [[Piece; 9]; 2] = [pieces(0), pieces(1)];

pub fn widemul_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    FORMS.iter().position(|one| one.0 == name).map(|at| (32, &PIECES[at][..]))
}
