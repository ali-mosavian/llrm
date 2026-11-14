//! The instructions this target has above the pin's table, by code and name:
//! the writebacks of [`super::indexed`], the short forms of
//! [`super::short`], the divide-remainders of [`super::divrem`] and the
//! `pea` push of [`super::pea`], which only [`crate::callpush`] makes, and the
//! `cmpa.l` compares of [`super::cmpa`].

use llrm_dagisel::instr::InstrDesc;

use super::asm::{AsmOperand, asm};
use super::enc::{Piece, encoding};
use super::{adda, bfffo, cmpa, divrem, indexed, pea, scaled, short, widemul, wordjump};

/// The machine code of the instruction named `name`, the pin's or a local one.
pub fn code_of(name: &str) -> Option<u16> {
    short::code_of(name)
        .or_else(|| scaled::code_of(name))
        .or_else(|| indexed::code_of(name))
        .or_else(|| divrem::code_of(name))
        .or_else(|| widemul::code_of(name))
        .or_else(|| cmpa::code_of(name))
        .or_else(|| adda::code_of(name))
        .or_else(|| bfffo::code_of(name))
        .or_else(|| wordjump::code_of(name))
}

/// The name of machine code `code`.
pub fn name_of(code: u32) -> Option<&'static str> {
    short::name_of(code)
        .or_else(|| scaled::name_of(code))
        .or_else(|| indexed::name_of(code))
        .or_else(|| divrem::name_of(code))
        .or_else(|| widemul::name_of(code))
        .or_else(|| cmpa::name_of(code))
        .or_else(|| adda::name_of(code))
        .or_else(|| bfffo::name_of(code))
        .or_else(|| wordjump::name_of(code))
}

/// The description of machine code `code`.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    short::desc(code)
        .or_else(|| scaled::desc(code))
        .or_else(|| indexed::desc(code))
        .or_else(|| divrem::desc(code))
        .or_else(|| widemul::desc(code))
        .or_else(|| cmpa::desc(code))
        .or_else(|| adda::desc(code))
        .or_else(|| bfffo::desc(code))
        .or_else(|| wordjump::desc(code))
}

/// The pin's `asm` row of `name`, or the local instruction's.
pub fn text(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    asm(name)
        .or_else(|| short::short_asm(name))
        .or_else(|| scaled::scaled_asm(name))
        .or_else(|| indexed::source_asm(name))
        .or_else(|| divrem::divrem_asm(name))
        .or_else(|| widemul::widemul_asm(name))
        .or_else(|| pea::pea_asm(name))
        .or_else(|| cmpa::cmpa_asm(name))
        .or_else(|| adda::adda_asm(name))
        .or_else(|| bfffo::bfffo_asm(name))
        .or_else(|| wordjump::wordjump_asm(name))
}

/// The pin's encoding of `name`, or the local instruction's.
pub fn bits(name: &str) -> Option<(u16, &'static [Piece])> {
    encoding(name)
        .map(|&(_, width, pieces)| (width, pieces))
        .or_else(|| short::short_encoding(name))
        .or_else(|| scaled::scaled_encoding(name))
        .or_else(|| indexed::source_encoding(name))
        .or_else(|| divrem::divrem_encoding(name))
        .or_else(|| widemul::widemul_encoding(name))
        .or_else(|| pea::pea_encoding(name))
        .or_else(|| cmpa::cmpa_encoding(name))
        .or_else(|| adda::adda_encoding(name))
        .or_else(|| bfffo::bfffo_encoding(name))
        .or_else(|| wordjump::wordjump_encoding(name))
}
