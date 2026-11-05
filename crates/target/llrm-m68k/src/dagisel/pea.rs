//! `pea (imm).w`: the push of a constant that fits a signed word.
//!
//! `move.l #imm, -(%sp)` takes 6 bytes (20 clk on the 68000); `pea` with an
//! absolute short operand takes 4 (16 clk) and pushes the sign-extended
//! address. [`crate::callpush`] chooses it for an argument push.
//!
//! DEVIATE: the pin's M68k has no `PEA`. Reason: best of gcc and clang is the
//! goal; gcc pushes small constants with it. Exit: none.

use super::asm::AsmOperand;
use super::enc::{Enc, Piece};

/// The name of the instruction: operand 0 is the immediate.
pub const NAME: &str = "PEAaw";

/// Whether `pea` can push `value`.
pub fn fits(value: i64) -> bool {
    i16::try_from(value).is_ok()
}

static ASM: (&str, &str, &[AsmOperand]) =
    (NAME, "pea\t$imm:w", &[AsmOperand { name: "imm", at: 0, method: "printPCRelImm" }]);

/// `0100 1000 0111 1000` and the word that follows.
static PIECES: [Piece; 2] = [
    Piece::Const { lo: 0, width: 16, value: 0x4878 },
    Piece::Operand { lo: 16, width: 16, at: 0, from: 0, enc: Enc::Plain },
];

pub fn pea_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    (name == NAME).then_some(&ASM)
}

pub fn pea_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    (name == NAME).then_some((32, &PIECES[..]))
}
