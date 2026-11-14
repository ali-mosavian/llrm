//! `M68kMCCodeEmitter`: an instruction's bytes, from the pin's encoding
//! tables ([`crate::dagisel::enc`]), and the fixups for operands that name a
//! symbol.
//!
//! # Fixups an object writer needs
//! `size` bytes at `offset` hold `symbol + addend`, big-endian:
//! - absolute 16 or 32 bits (`R_68K_16`, `R_68K_32`): `jsr g`, `move.l #g, ..`;
//! - pc-relative 8, 16 or 32 bits (`R_68K_PC8`, `PC16`, `PC32`): `bra`/`bcc`
//!   and `(d16,%pc)` forms. The value is `symbol + addend - address of the
//!   fixed bytes`; the addend already holds the correction for a byte inside
//!   the opcode word (`encodePCRelImm`).
//!
//! `encodeScale` and `encodeIndexSuppress` are the pin's, for the full
//! extension word of the PC-relative indexed forms (`PCIBD`). DEVIATE:
//! `encodeFPSYSSelect` (FPU control registers) is not translated. Reason: the
//! slice does not reach it. Exit: M7.
//!
//! # See also
//! `llvm/lib/Target/M68k/MCTargetDesc/M68kMCCodeEmitter.cpp`, pin d1106deb71cc.

use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;
use crate::dagisel::enc::{Enc, Piece};
use crate::dagisel::local;
use crate::mc::{Inst, Operand};

/// A field of the bytes that waits for a symbol's address (`MCFixup`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub offset: usize,
    pub symbol: String,
    pub addend: i64,
    pub size: u8,
    pub pc_rel: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub fixups: Vec<Fixup>,
}

/// A jump table: one absolute 32-bit fixup per label.
pub fn table(labels: &[String]) -> Encoded {
    Encoded {
        bytes: vec![0; 4 * labels.len()],
        fixups: labels
            .iter()
            .enumerate()
            .map(|(at, label)| Fixup { offset: 4 * at, symbol: label.clone(), addend: 0, size: 4, pc_rel: false })
            .collect(),
    }
}

/// A jump table of 16-bit offsets from its start: entry `at` is `label -
/// start`, a pc-relative fixup with the addend `2 * at` (its place is `start +
/// 2 * at`).
pub fn word_table(labels: &[String]) -> Encoded {
    Encoded {
        bytes: vec![0; 2 * labels.len()],
        fixups: labels
            .iter()
            .enumerate()
            .map(|(at, label)| Fixup {
                offset: 2 * at,
                symbol: label.clone(),
                addend: 2 * at as i64,
                size: 2,
                pc_rel: true,
            })
            .collect(),
    }
}

/// `getEncodingValue` with the `D/A` bit `getMachineOpValue` adds.
fn register_value(id: RegId) -> Option<u64> {
    let name = REGISTER_INFO.name(id)?;
    let number = |digits: &str| digits.parse::<u64>().ok().filter(|n| *n < 8);
    match name {
        "sp" | "wsp" => Some(15),
        _ if name.starts_with("bd") || name.starts_with("wd") => number(&name[2..]),
        _ if name.starts_with("wa") => number(&name[2..]).map(|n| 8 + n),
        _ if name.starts_with('d') => number(&name[1..]),
        _ if name.starts_with('a') => number(&name[1..]).map(|n| 8 + n),
        _ => None,
    }
}

/// `M68k::swapWord`: the halves of a long swap, so the high word is emitted
/// first; a byte or word is cut to size.
fn swap_word(
    value: i64,
    size: u8,
) -> u64 {
    match size {
        8 => value as u8 as u64,
        16 => value as u16 as u64,
        _ => {
            let long = value as u32;
            (long.rotate_left(16)) as u64
        }
    }
}

/// `getBytePosition`: the byte of the big-endian instruction a field at bit
/// `at` starts in.
fn byte_position(
    size: u8,
    at: u16,
) -> usize {
    let byte = (at / 8) as usize;
    match (size % 16 != 0, at & 15 < 8) {
        (false, _) => byte,
        (true, true) => byte + 1,
        (true, false) => byte - 1,
    }
}

/// The operand's value for the field at bit `at`, or a fixup when it names a
/// symbol (then the field stays 0).
fn operand_value(
    inst: &Inst,
    index: usize,
    enc: Enc,
    at: u16,
    fixups: &mut Vec<Fixup>,
) -> Result<u64, String> {
    let operand = inst.operands.get(index).ok_or_else(|| format!("{}: no operand {index}", inst.opcode))?;
    match (enc, operand) {
        (Enc::Plain, Operand::Reg(id)) => {
            register_value(*id).ok_or_else(|| format!("{}: register {id:?}", inst.opcode))
        }
        (Enc::Plain, Operand::Imm(value)) => Ok(*value as u64),
        (Enc::Plain, Operand::Symbol(..)) => {
            Err(format!("{}: a symbol where only an absolute value fits", inst.opcode))
        }
        (Enc::RelocImm(size), Operand::Imm(value)) | (Enc::PcRelImm(size), Operand::Imm(value)) => {
            Ok(swap_word(*value, size))
        }
        (Enc::RelocImm(size), Operand::Symbol(symbol, addend)) => {
            fixups.push(Fixup {
                offset: byte_position(size, at),
                symbol: symbol.clone(),
                addend: *addend,
                size: size / 8,
                pc_rel: false,
            });
            Ok(0)
        }
        (Enc::PcRelImm(size), Operand::Symbol(symbol, addend)) => {
            let offset = byte_position(size, at);
            // below a word the field sits inside the opcode word: PC is the
            // next word for the first one, else the word's first byte
            let correction = match () {
                _ if size >= 16 => 0,
                _ if at < 16 => offset as i64 - 2,
                _ if offset % 2 == 1 => 1,
                _ => 0,
            };
            fixups.push(Fixup {
                offset,
                symbol: symbol.clone(),
                addend: addend + correction,
                size: size / 8,
                pc_rel: true,
            });
            Ok(0)
        }
        (Enc::InverseMoveMask, Operand::Imm(mask)) => Ok((*mask as u16).reverse_bits() as u64),
        // `encodeScale`: log2 of a scale of 1, 2, 4 or 8.
        (Enc::Scale, Operand::Imm(scale)) if matches!(scale, 1 | 2 | 4 | 8) => Ok(u64::from(scale.trailing_zeros())),
        (Enc::Scale, Operand::Imm(scale)) => Err(format!("{}: scale {scale} is not 1, 2, 4 or 8", inst.opcode)),
        // `encodeIndexSuppress`: bit 4 is set when there is no index register;
        // every operand here names one.
        (Enc::IndexSuppress, Operand::Reg(_)) => Ok(0),
        _ => Err(format!("{}: operand {index} is not encodable as {enc:?}", inst.opcode)),
    }
}

/// `encodeInstruction`.
pub fn encode(inst: &Inst) -> Result<Encoded, String> {
    let (width, pieces) = match crate::divide::encoding(inst.opcode) {
        Some(row) => row,
        None => local::bits(inst.opcode).ok_or_else(|| format!("no encoding for {}", inst.opcode))?,
    };
    if width == 0 {
        return Err(format!("{} is a pseudo-instruction: it has no encoding", inst.opcode));
    }
    let mut bits: u128 = 0;
    let mut fixups = Vec::new();
    // an operand's field may be split over several pieces (a long over two
    // words): the widest extent is the field a value must fit
    let field_bits = |index: u8| {
        pieces
            .iter()
            .filter_map(|piece| match *piece {
                Piece::Operand { at, from, width, enc: Enc::Plain, .. } if at == index => {
                    Some(u16::from(from) + u16::from(width))
                }
                _ => None,
            })
            .max()
    };
    for piece in pieces {
        let (lo, width, value) = match *piece {
            Piece::Const { lo, width, value } => (lo, width, value),
            Piece::Operand { lo, width, at, from, enc } => {
                // DEVIATE from the pin, whose `MOVM*em` (`movem` to `-(An)`)
                // takes the mask in the instruction's own
                // reversed order while the printer reads it as
                // `d0` first: ours is the printed order.
                let enc =
                    if at == 1 && matches!(inst.opcode, "MOVM32em" | "MOVM16em") { Enc::InverseMoveMask } else { enc };
                // a displacement is signed (128 is not a byte there); an
                // immediate may be written as either (`andi.b
                // #255`, `#-1`). The small `Plain` fields (registers, a quick
                // immediate where 8 is written 0, a bit number) are masked on
                // purpose and stay so.
                let displacement = crate::dagisel::local::text(inst.opcode).is_some_and(|&(_, _, shape)| {
                    shape
                        .iter()
                        .any(
                            |one| usize::from(one.at) == at as usize
                                && ["ARII", "ARID", "PCI", "PCD"].iter().any(|m| one.method.contains(m)),
                        )
                });
                let extent = match enc {
                    Enc::Plain if displacement => field_bits(at),
                    Enc::RelocImm(size) | Enc::PcRelImm(size) => Some(u16::from(size)),
                    _ => None,
                };
                if let (Some(Operand::Imm(value)), Some(bits)) = (inst.operands.get(at as usize), extent) {
                    // the pin masks silently; binutils refuses, and a cut
                    // displacement is another stack slot:
                    // `(260,%sp,%a5)` in an 8-bit index field read
                    // `(4,%sp,%a5)`
                    let top = if displacement { 1i64 << (bits - 1) } else { 1i64 << bits };
                    if bits < 64 && !(-(1i64 << (bits - 1))..top).contains(value) {
                        return Err(format!(
                            "{}: operand {at} = {value} does not fit its {bits}-bit field",
                            inst.opcode
                        ));
                    }
                }
                let whole = operand_value(inst, at as usize, enc, lo, &mut fixups)?;
                (lo, width, whole >> from)
            }
        };
        let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
        bits |= ((value & mask) as u128) << lo;
    }
    let mut bytes = Vec::new();
    for word in 0..width / 16 {
        bytes.extend_from_slice(&((bits >> (16 * word)) as u16).to_be_bytes());
    }
    Ok(Encoded { bytes, fixups })
}
