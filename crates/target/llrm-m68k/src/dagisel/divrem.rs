//! `divsl.l <ea>, Dr:Dq` and `divul.l`: the quotient and the remainder of one
//! 32-bit divide.
//!
//! # How it works
//! Each is the pin's `divs.l` / `divu.l` row (a data register divisor) with a
//! second result: the remainder register `Dr`, which the extension word names
//! and the pin's row leaves equal to `Dq`. [`crate::divrem`] forms them from
//! a divide and the `x - (x / y) * y` made of its quotient.
//!
//! DEVIATE: the pin's M68k has no `DIVSL` (its `SDIVREM` is a libcall), so
//! `a / b + a % b` is a divide, a multiply and a subtract; gcc divides once.
//! Reason: best of gcc and clang is the goal (muldiv 68020 42 B against 28).
//! Exit: 2026-11-12.

use std::sync::LazyLock;

use llrm_dagisel::instr::{InstrDesc, OperandDesc};

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the divide-remainder instructions, above the short forms.
pub const FIRST: u16 = 0xF200;

/// `(name, the pin's row it derives from, mnemonic, signed)`.
const FORMS: [(&str, &str, &str, u64); 2] =
    [("SDIVREM32", "SDIVd32d32", "divsl.l", 1), ("UDIVREM32", "UDIVd32d32", "divul.l", 0)];

/// The operands: the quotient, the remainder, the dividend (tied to the
/// quotient) and the divisor.
pub const QUOTIENT: usize = 0;
pub const REMAINDER: usize = 1;
pub const DIVIDEND: usize = 2;
pub const DIVISOR: usize = 3;

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
            // the pin's [dst, src (tied to dst), opd] with the remainder second
            let dst = base.operands[0];
            let mut operands = vec![dst, OperandDesc { tied_to: -1, ..dst }];
            operands.push(OperandDesc { tied_to: QUOTIENT as i8, ..base.operands[1] });
            operands.push(base.operands[2]);
            &*Box::leak(Box::new(InstrDesc {
                num_operands: 4,
                num_defs: 2,
                operands: Box::leak(operands.into_boxed_slice()),
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
            let text: &'static str = Box::leak(format!("{mnemonic}\t$opd, $rem:$dst").into_boxed_str());
            let operands: &'static [AsmOperand] = Box::leak(Box::new([
                AsmOperand { name: "opd", at: DIVISOR as u8, method: "printOperand" },
                AsmOperand { name: "rem", at: REMAINDER as u8, method: "printOperand" },
                AsmOperand { name: "dst", at: QUOTIENT as u8, method: "printOperand" },
            ]));
            (name, text, operands)
        })
        .collect()
});

pub fn divrem_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
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

/// `0100 1100 01 mode reg` and the extension word `0 Dq 1 S 0 0 0 0000000 0 Dr`:
/// `divs.l` / `divu.l` with `Dr` the remainder register.
const fn pieces(signed: u64) -> [Piece; 9] {
    [
        field(0, 3, DIVISOR as u8),
        bits(3, 3, 0),
        bits(6, 10, 305),
        field(16, 3, REMAINDER as u8),
        bits(19, 7, 0),
        bits(26, 1, 0),
        bits(27, 1, signed),
        field(28, 3, QUOTIENT as u8),
        bits(31, 1, 0),
    ]
}

static PIECES: [[Piece; 9]; 2] = [pieces(1), pieces(0)];

pub fn divrem_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    FORMS.iter().position(|one| one.0 == name).map(|at| (32, &PIECES[at][..]))
}
