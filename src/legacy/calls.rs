//! Port of `qbopt/legacy/calls.py`: so far the fixed registers an absorbed
//! long divide uses, and the encoder `select.divides` builds it with.

use iced_x86::{
    BlockEncoder, BlockEncoderOptions, Code, Decoder, DecoderOptions, Instruction, InstructionBlock, Register,
};

use crate::frontend::declen::BITNESS;
use crate::legacy::lift::Emitted;
pub use crate::legacy::lift::relocated_memory;

/// The routine that answers a long remainder.
pub const REMAINDER: &str = "B$RMI4";
/// What the runtime returns a long in, as ax:dx.
pub const RESULT: Register = Register::EAX;
pub const OTHER: Register = Register::EBX;
pub const DIVISOR: Register = Register::ECX;

/// Encode a block whose branches name one another by instruction index.
///
/// `relocated` is Python's dict in insertion order. `Err` is Python's
/// ValueError.
pub fn assemble(steps: &mut [Instruction], relocated: &[(usize, usize)]) -> Result<Emitted, String> {
    for (index, insn) in steps.iter_mut().enumerate() {
        insn.set_ip(index as u64);
    }
    let code = BlockEncoder::encode(BITNESS, InstructionBlock::new(steps, 0), BlockEncoderOptions::NONE)
        .map_err(|error| error.to_string())?
        .code_buffer;

    let mut decoder = Decoder::with_ip(BITNESS, &code, 0, DecoderOptions::NONE);
    let mut placed = Vec::new();
    while decoder.can_decode() {
        let insn = decoder.decode();
        placed.push((insn.ip() as usize, decoder.get_constant_offsets(&insn)));
    }
    if placed.len() != steps.len() {
        return Err(format!("encoded {} instructions from {}", placed.len(), steps.len()));
    }
    let relocations = relocated
        .iter()
        .map(|&(index, field)| (placed[index].0 + placed[index].1.displacement_offset(), field))
        .collect();
    Ok(Emitted { code, relocations })
}

/// Put the high half back where BC reads it, through the stack.
pub fn restoring() -> Vec<Instruction> {
    let one =
        |code: Code, register: Register| Instruction::with1(code, register).unwrap_or_else(|error| panic!("{error}"));
    vec![
        one(Code::Push_r32, RESULT),
        one(Code::Pop_r16, Register::AX),
        one(Code::Pop_r16, Register::DX),
    ]
}
