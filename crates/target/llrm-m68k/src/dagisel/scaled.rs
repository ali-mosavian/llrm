//! `(d8,An,Xn.l*2)`, `*4` and `*8`: the 68020's scaled index, as a sibling of
//! each of the pin's rows that takes `(d8,An,Xn)`.
//!
//! # How it works
//! A scaled row is the pin's indexed row with the scale in the brief
//! extension word's scale field (bits 10:9, which the pin's rows write 0): its
//! operands, description and classes are the pin's, its text prints the index
//! as `%dn.l*4`. Rows are named `<pin row>x2`, `x4`, `x8`. [`scaled_name`]
//! gives the sibling; [`scale_of`] reads a row's scale back. The extending
//! loads from `(d8,An,Xn)` (`MOVZXd32f16`, `MOVSXd32f8`..) are pseudos with no
//! text or encoding: their siblings are scaled the same way and expand to the
//! scaled sibling of the move they become.
//!
//! DEVIATE: the pin has no scaled index (its `ARII` operand has none, only
//! `PCIBD` declares one), so it prints `lsl.l #2, %d1; move.l (0,%a0,%d1), %d0`
//! for the one `move.l (%a0,%d1.l*4), %d0` gcc -m68020 uses. Reason: best of
//! gcc and clang is the goal. Exit: review 2026-11-20. Only the 68020 and later select
//! these rows; the 68000 and 68010 have no scale field.

use std::sync::LazyLock;

use llrm_dagisel::instr::InstrDesc;

use super::asm::{ASM, AsmOperand};
use super::enc::{Piece, encoding};
use super::generated::instrinfo;
use super::generated::m68k_m68000::MACHINE_OPCODES;

/// Codes of the scaled rows, above the short forms'.
pub const FIRST: u16 = 0xF400;

const SCALES: [u8; 3] = [2, 4, 8];

/// A scaled row: the pin's row it derives from, and its scale.
struct Row {
    name: &'static str,
    base: &'static str,
    scale: u8,
}

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    let mut rows = Vec::new();
    for &(base, _, operands) in ASM.iter() {
        // the pin's plain rows only: `_TC` and `_P` are pseudo variants
        if base.contains('_') {
            continue;
        }
        // one indexed operand: a move between two has a scale for each
        let hardware =
            operands.iter().filter(|one| one.method.contains("ARII")).count() == 1 && encoding(base).is_some();
        if !(hardware || is_extending_indexed_load(base)) || pin_code(base).is_none() {
            continue;
        }
        for scale in SCALES {
            rows.push(Row { name: leak(format!("{base}x{scale}")), base, scale });
        }
    }
    rows
});

/// `MOVZXd32f16`, `MOVSXd32f8`..: an extending load from `(d8,An,Xn)`.
fn is_extending_indexed_load(name: &str) -> bool {
    crate::expand::extension(name).is_some_and(|(_, _, mode, _)| mode == 'f')
}

/// The names of the scaled rows the hardware encodes (not the extending
/// pseudos, which expand to one).
pub fn names() -> impl Iterator<Item = &'static str> {
    ROWS.iter().filter(|one| !is_extending_indexed_load(one.base)).map(|one| one.name)
}

/// The machine code of the scaled row `name`.
pub fn code_of(name: &str) -> Option<u16> {
    ROWS.iter().position(|one| one.name == name).map(|at| FIRST + at as u16)
}

/// The name of machine code `code`, when it is a scaled row's.
pub fn name_of(code: u32) -> Option<&'static str> {
    let at = u16::try_from(code).ok()?.checked_sub(FIRST)?;
    ROWS.get(usize::from(at)).map(|one| one.name)
}

/// The scale of the row `name` (2, 4 or 8), when it is a scaled row's.
pub fn scale_of(name: &str) -> Option<u8> {
    ROWS.iter().find(|one| one.name == name).map(|one| one.scale)
}

/// The scaled sibling of the pin's row `base`.
pub fn scaled_name(
    base: &str,
    scale: u8,
) -> Option<&'static str> {
    ROWS.iter().find(|one| one.base == base && one.scale == scale).map(|one| one.name)
}

/// The pin's row a scaled row derives from.
pub fn base_of(name: &str) -> Option<&'static str> {
    ROWS.iter().find(|one| one.name == name).map(|one| one.base)
}

fn pin_code(name: &str) -> Option<u16> {
    MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code)
}

/// The description of a scaled row: its pin row's.
pub fn desc(code: u16) -> Option<&'static InstrDesc> {
    let row = ROWS.get(usize::from(code.checked_sub(FIRST)?))?;
    instrinfo::desc(pin_code(row.base)?)
}

static TEXT: LazyLock<Vec<(&'static str, &'static str, &'static [AsmOperand])>> = LazyLock::new(|| {
    let mut rows: Vec<_> = ROWS
        .iter()
        .map(|one| {
            let &(_, text, operands) = super::asm::asm(one.base).expect("the pin row prints");
            (one.name, text, operands)
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The text of a scaled row: its pin row's.
pub fn scaled_asm(name: &str) -> Option<&'static (&'static str, &'static str, &'static [AsmOperand])> {
    TEXT.binary_search_by_key(&name, |row| row.0).ok().map(|at| &TEXT[at])
}

static ENCODINGS: LazyLock<Vec<(&'static str, u16, &'static [Piece])>> = LazyLock::new(|| {
    let mut rows: Vec<_> = ROWS
        .iter()
        .filter(|one| !is_extending_indexed_load(one.base))
        .map(|one| {
            let &(_, width, base) = encoding(one.base).expect("the pin row encodes");
            let field = u64::from(one.scale.trailing_zeros());
            // the brief extension word is `D/A reg W/L scale 0 disp8`: its
            // index register is the 4-bit operand at bit 12 of the word, the
            // scale field three bits below it; an immediate before the word
            // moves all of it up
            let index = base
                .iter()
                .filter_map(|piece| match *piece {
                    Piece::Operand { lo, width: 4, .. } => Some(lo),
                    _ => None,
                })
                .max()
                .expect("an index register");
            let pieces: Vec<Piece> = base
                .iter()
                .map(|piece| match *piece {
                    Piece::Const { lo, width: 2, .. } if lo + 3 == index => Piece::Const { lo, width: 2, value: field },
                    other => other,
                })
                .collect();
            (one.name, width, &*Box::leak(pieces.into_boxed_slice()))
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
});

/// The encoding of a scaled row: its pin row's, with the scale field set.
pub fn scaled_encoding(name: &str) -> Option<(u16, &'static [Piece])> {
    ENCODINGS.binary_search_by_key(&name, |row| row.0).ok().map(|at| (ENCODINGS[at].1, ENCODINGS[at].2))
}
