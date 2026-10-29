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
//! DEVIATE: `encodeFPSYSSelect`, `encodeScale` and `encodeIndexSuppress` (FPU
//! and 68020 index modes) are not translated. Reason: the slice does not reach
//! them. Exit: M7.
//!
//! # See also
//! `llvm/lib/Target/M68k/MCTargetDesc/M68kMCCodeEmitter.cpp`, pin d1106deb71cc.

use llrm_lir::registers::RegId;

use crate::REGISTER_INFO;
use crate::dagisel::enc::{Enc, Piece, encoding};
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
        _ => Err(format!("{}: operand {index} is not encodable as {enc:?}", inst.opcode)),
    }
}

/// `encodeInstruction`.
pub fn encode(inst: &Inst) -> Result<Encoded, String> {
    let &(_, width, pieces) = encoding(inst.opcode).ok_or_else(|| format!("no encoding for {}", inst.opcode))?;
    if width == 0 {
        return Err(format!("{} is a pseudo-instruction: it has no encoding", inst.opcode));
    }
    let mut bits: u128 = 0;
    let mut fixups = Vec::new();
    for piece in pieces {
        let (lo, width, value) = match *piece {
            Piece::Const { lo, width, value } => (lo, width, value),
            Piece::Operand { lo, width, at, from, enc } => {
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
