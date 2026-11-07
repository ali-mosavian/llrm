//! `cmpa.l`: the compare whose destination is an address register.
//!
//! # How it works
//! Each row is a pin `CMP32d*` row with an address register as the
//! destination: its operands, flags and text are the pin's with `cmpa.l` for
//! `cmp.l` (`cmpi.l` for `cmpa.l #imm`), and its encoding is the pin's with
//! the opmode `111` (long, address destination) for `010`. They are named
//! `CMPA32<form>` for the pin's `CMP32d<form>`. [`crate::addrcmp`] makes them
//! after allocation.
//!
//! DEVIATE C-CMPA (owner m68k, exit 2026-11-28: the pin gains a `CMPA` def, or
//! the rows go): the pin's M68k has no `CMPA`
//! (`M68kInstrArithmetic.td`, "CMPA [~]": no def), so a pointer compared where
//! it lives in an address register is copied to a data register first. Reason:
//! best of gcc and clang is the goal; both use `cmpa.l`.
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kInstrArithmetic.td#L16>

use std::sync::LazyLock;

use llrm_dagisel::instr::InstrDesc;

use super::asm::{AsmOperand, asm};
use super::enc::{Enc, Piece, encoding};
use super::generated::ids::reg as pin;
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the rows, above the scaled rows'.
pub const FIRST: u16 = 0xF500;

/// A row: the pin's `CMP32d<form>` it derives from, and the operand that is
/// the destination.
struct Row {
    name: &'static str,
    base: &'static str,
    dest: usize,
}

/// The pin's compares of a long to a data register, by their source: a
/// register, an immediate (`cmpi`), and the memory forms.
static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    [("r", 1), ("i", 1), ("j", 0), ("p", 0), ("f", 0), ("k", 0), ("q", 0)]
        .into_iter()
        .map(|(form, dest)| Row { name: leak(format!("CMPA32{form}")), base: leak(format!("CMP32d{form}")), dest })
        .collect()
});

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// The destination operand of the pin's row `base`.
pub fn dest_of(base: &str) -> Option<usize> {
    ROWS.iter().find(|row| row.base == base).map(|row| row.dest)
}

/// The address-destination row of the pin's compare `base`, if it has one.
pub fn sibling(base: &str) -> Option<&'static str> {
    ROWS.iter().find(|row| row.base == base).map(|row| row.name)
}

/// The machine code of the row `name`.
pub fn code_of(name: &str) -> Option<u16> {
    ROWS.iter().position(|row| row.name == name).map(|at| FIRST + at as u16)
}

/// The name of machine code `code`, when it is a row's.
pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    ROWS.get(usize::from(at)).map(|row| row.name)
}

fn instrinfo_code(name: &str) -> Option<u16> {
    MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code)
}

static DESCS: LazyLock<Vec<&'static InstrDesc>> = LazyLock::new(|| {
    ROWS.iter()
        .map(|row| {
            let base = instrinfo::desc(instrinfo_code(row.base).expect("a pin row")).expect("a described row");
            let mut operands = base.operands.to_vec();
            operands[row.dest].reg_class = pin::AR32RegClassID as i16;
            &*Box::leak(Box::new(InstrDesc { operands: Box::leak(operands.into_boxed_slice()), ..*base }))
        })
        .collect()
});

/// The description of a row: the pin's, with an address-register destination.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    DESCS.get(usize::from(code.checked_sub(FIRST)?)).copied()
}

static ASM: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    let mut rows: Vec<_> = ROWS
        .iter()
        .map(|row| {
            let &(_, text, operands) = asm(row.base).expect("the pin row prints");
            let (_, rest) = text.split_once('.').expect("a sized mnemonic");
            (row.name, leak(format!("cmpa.{rest}")), operands)
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The text of a row, as the pin's `asm` rows.
pub fn cmpa_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    ASM.binary_search_by_key(&name, |row| row.0).ok().map(|at| &ASM[at])
}

static ENCODINGS: LazyLock<Vec<(&'static str, u16, &'static [Piece])>> = LazyLock::new(|| {
    let mut rows: Vec<_> = ROWS
        .iter()
        .map(|row| {
            let &(_, width, base) = encoding(row.base).expect("the pin row encodes");
            let pieces: Vec<Piece> = if row.base == "CMP32di" {
                // `cmpi.l #imm, %dn` is `0000 1100 10 ea` with the register in
                // the ea field; `cmpa.l #imm, %an` is `1011 an 111 111 100`.
                let (immediate, register) = match (base.last(), row.dest) {
                    (Some(&last @ Piece::Operand { .. }), dest) => (last, dest as u8),
                    _ => panic!("cmpi.l ends with its immediate"),
                };
                vec![
                    Piece::Const { lo: 0, width: 3, value: 4 },
                    Piece::Const { lo: 3, width: 3, value: 7 },
                    Piece::Const { lo: 6, width: 3, value: 7 },
                    Piece::Operand { lo: 9, width: 3, at: register, from: 0, enc: Enc::Plain },
                    Piece::Const { lo: 12, width: 4, value: 11 },
                    immediate,
                ]
            } else {
                // the opmode field: long to a data register is 010, to an
                // address register 111
                base.iter()
                    .map(|piece| match *piece {
                        Piece::Const { lo: 6, width: 3, value: 2 } => Piece::Const { lo: 6, width: 3, value: 7 },
                        other => other,
                    })
                    .collect()
            };
            (row.name, width, &*Box::leak(pieces.into_boxed_slice()))
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The encoding of a row.
pub fn cmpa_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    ENCODINGS.binary_search_by_key(&name, |row| row.0).ok().map(|at| (ENCODINGS[at].1, ENCODINGS[at].2))
}
