//! The instructions this target has above the pin's table, by code and name:
//! the writebacks of [`super::indexed`] and the short forms of
//! [`super::short`].

use llrm_dagisel::instr::InstrDesc;

use super::asm::{AsmOperand, asm};
use super::enc::{Piece, encoding};
use super::{indexed, short};

/// The machine code of the instruction named `name`, the pin's or a local one.
pub fn code_of(name: &str) -> Option<u16> {
    short::code_of(name).or_else(|| indexed::code_of(name))
}

/// The name of machine code `code`.
pub fn name_of(code: u32) -> Option<&'static str> {
    short::name_of(code).or_else(|| indexed::name_of(code))
}

/// The description of machine code `code`.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    short::desc(code).or_else(|| indexed::desc(code))
}

/// The pin's `asm` row of `name`, or the local instruction's.
pub fn text(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    asm(name).or_else(|| short::short_asm(name)).or_else(|| indexed::source_asm(name))
}

/// The pin's encoding of `name`, or the local instruction's.
pub fn bits(name: &str) -> Option<(u16, &'static [Piece])> {
    encoding(name)
        .map(|&(_, width, pieces)| (width, pieces))
        .or_else(|| short::short_encoding(name))
        .or_else(|| indexed::source_encoding(name))
}
