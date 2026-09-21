//! Exact initial i386 register and BP-relative instruction encoding.
//!
//! The initial target has a 16-bit default operand size.  This module encodes
//! physical general-purpose register forms plus the BP-relative frame form
//! produced by frame-index elimination. Direct far calls retain their symbolic
//! target in one typed x86 fixup; branches, calls of other forms, segments,
//! and x87 remain explicit unsupported forms until their semantics and
//! relocation contracts are implemented.

use std::error::Error;
use std::fmt;

use crate::mc::{Fixup, MCInstruction, MCOperand};

use super::{OperandSize, X86FixupKind, X86Opcode, X86Register};

/// The bytes and relocations selected for one x86 instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedInstruction {
    pub bytes: Vec<u8>,
    pub fixups: Vec<Fixup>,
}

/// An x86 instruction which the initial register-form encoder cannot encode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeError {
    /// The MC opcode is not one of the stable x86 opcode identities.
    UnknownOpcode { raw: u32 },
    /// The MC physical-register identity is not an x86 architectural view.
    UnknownRegister { raw: u32 },
    /// The architectural view exists but is outside this register-form subset.
    UnsupportedRegister { raw: u32 },
    /// A materialized frame address does not use 16-bit BP as its base.
    UnsupportedFrameBase { opcode: X86Opcode, raw: u32 },
    /// The operand count does not match the selected instruction form.
    Arity {
        opcode: X86Opcode,
        expected: usize,
        actual: usize,
    },
    /// The byte-only API cannot represent this instruction's relocations.
    FixupsRequired { opcode: X86Opcode, count: usize },
    /// An operand has a kind the selected form cannot encode.
    OperandKind {
        opcode: X86Opcode,
        index: usize,
        expected: &'static str,
    },
    /// Two register operands do not name views with one common width.
    WidthMismatch {
        opcode: X86Opcode,
        left: OperandSize,
        right: OperandSize,
    },
    /// An immediate cannot be represented in its selected instruction width.
    ImmediateOutOfRange { value: i64, bits: u8 },
    /// The opcode is known but has no exact implementation in this subset.
    UnsupportedForm {
        opcode: X86Opcode,
        reason: &'static str,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOpcode { raw } => write!(formatter, "unknown x86 opcode {raw}"),
            Self::UnknownRegister { raw } => write!(formatter, "unknown x86 register {raw}"),
            Self::UnsupportedRegister { raw } => {
                write!(
                    formatter,
                    "x86 register {raw} is not a general-purpose register view"
                )
            }
            Self::UnsupportedFrameBase { opcode, raw } => {
                write!(
                    formatter,
                    "{opcode:?} frame address uses x86 register {raw}, not BP"
                )
            }
            Self::Arity {
                opcode,
                expected,
                actual,
            } => write!(
                formatter,
                "{opcode:?} expects {expected} operands, found {actual}"
            ),
            Self::FixupsRequired { opcode, count } => {
                write!(formatter, "{opcode:?} requires {count} fixups")
            }
            Self::OperandKind {
                opcode,
                index,
                expected,
            } => write!(formatter, "{opcode:?} operand {index} must be {expected}"),
            Self::WidthMismatch {
                opcode,
                left,
                right,
            } => write!(
                formatter,
                "{opcode:?} operands have mismatched widths {} and {}",
                left.bits(),
                right.bits()
            ),
            Self::ImmediateOutOfRange { value, bits } => {
                write!(formatter, "immediate {value} does not fit in {bits} bits")
            }
            Self::UnsupportedForm { opcode, reason } => {
                write!(formatter, "{opcode:?} is not supported: {reason}")
            }
        }
    }
}

impl Error for EncodeError {}

/// Encodes one physical-register x86 instruction using a 16-bit default mode.
///
/// This compatibility entry point returns only bytes.  Use
/// [`encode_with_fixups`] when a caller needs relocation information.
pub fn encode(instruction: &MCInstruction) -> Result<Vec<u8>, EncodeError> {
    let encoded = encode_with_fixups(instruction)?;
    if encoded.fixups.is_empty() {
        Ok(encoded.bytes)
    } else {
        Err(EncodeError::FixupsRequired {
            opcode: decode_opcode(instruction.opcode.get())?,
            count: encoded.fixups.len(),
        })
    }
}

/// Encodes one x86 instruction and preserves every target-owned fixup.
pub fn encode_with_fixups(instruction: &MCInstruction) -> Result<EncodedInstruction, EncodeError> {
    let opcode = decode_opcode(instruction.opcode.get())?;
    let bytes = match opcode {
        X86Opcode::Copy | X86Opcode::PhiCopy | X86Opcode::Mov => {
            encode_move(opcode, &instruction.operands)
        }
        X86Opcode::Add => encode_add_sub(opcode, &instruction.operands, 0, 0x00, 0x01),
        X86Opcode::Sub => encode_add_sub(opcode, &instruction.operands, 5, 0x28, 0x29),
        X86Opcode::And => encode_register_operands(opcode, &instruction.operands, 0x20, 0x21),
        X86Opcode::Or => encode_register_operands(opcode, &instruction.operands, 0x08, 0x09),
        X86Opcode::Xor => encode_register_operands(opcode, &instruction.operands, 0x30, 0x31),
        X86Opcode::Cmp => encode_register_operands(opcode, &instruction.operands, 0x38, 0x39),
        X86Opcode::Test => encode_register_operands(opcode, &instruction.operands, 0x84, 0x85),
        X86Opcode::Imul => encode_imul(opcode, &instruction.operands),
        X86Opcode::SignExtendWordToDword => {
            encode_sign_extend_word_to_dword(opcode, &instruction.operands)
        }
        X86Opcode::ShiftLeftDouble => encode_shift_left_double(opcode, &instruction.operands),
        X86Opcode::Neg => encode_unary(opcode, &instruction.operands, 3),
        X86Opcode::Not => encode_unary(opcode, &instruction.operands, 2),
        X86Opcode::Push => encode_push_pop(opcode, &instruction.operands, 0x50),
        X86Opcode::Pop => encode_push_pop(opcode, &instruction.operands, 0x58),
        X86Opcode::Leave => encode_return(opcode, &instruction.operands, 0xc9),
        X86Opcode::ReturnNear => encode_return(opcode, &instruction.operands, 0xc3),
        X86Opcode::ReturnFar => encode_far_return(opcode, &instruction.operands),
        X86Opcode::Lea => return encode_lea(opcode, &instruction.operands),
        X86Opcode::Load => encode_load(opcode, &instruction.operands),
        X86Opcode::Store => encode_store(opcode, &instruction.operands),
        X86Opcode::CallFar => return encode_far_call(opcode, &instruction.operands),
        X86Opcode::CallNear => return encode_near_call(opcode, &instruction.operands),
        X86Opcode::Idiv
        | X86Opcode::ShiftLeft
        | X86Opcode::ShiftRightLogical
        | X86Opcode::ShiftRightArithmetic
        | X86Opcode::Jump
        | X86Opcode::JumpConditional
        | X86Opcode::MergeWords
        | X86Opcode::LowWord
        | X86Opcode::HighWord => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "this initial encoder accepts only exact register forms",
        }),
    }?;
    Ok(EncodedInstruction {
        bytes,
        fixups: Vec::new(),
    })
}

fn encode_near_call(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    expect_arity(opcode, operands, 1)?;
    let MCOperand::Expression(expression) = &operands[0] else {
        return Err(EncodeError::OperandKind {
            opcode,
            index: 0,
            expected: "a symbolic near-call target",
        });
    };
    let kind = X86FixupKind::PcRelative16;
    Ok(EncodedInstruction {
        bytes: vec![0xe8, 0, 0],
        fixups: vec![Fixup {
            offset: 1,
            kind: kind.into(),
            expression: *expression,
            pc_relative: kind.pc_relative(),
        }],
    })
}

fn encode_far_call(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    expect_arity(opcode, operands, 1)?;
    let MCOperand::Expression(expression) = &operands[0] else {
        return Err(EncodeError::OperandKind {
            opcode,
            index: 0,
            expected: "a symbolic far-call target",
        });
    };
    let kind = X86FixupKind::FarPointer1616;
    // These zero bytes are the pre-fixup skeleton. The future fixup
    // application/object stage materializes the expression, including its
    // addend, exactly once.
    let mut bytes = vec![0x9a];
    bytes.extend(vec![0; usize::from(kind.width())]);

    Ok(EncodedInstruction {
        bytes,
        fixups: vec![Fixup {
            offset: 1,
            kind: kind.into(),
            expression: *expression,
            pc_relative: kind.pc_relative(),
        }],
    })
}

fn encode_lea(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    if let [destination, MCOperand::Expression(expression)] = operands {
        let destination = match destination {
            MCOperand::Register(_) => register_operand(opcode, operands, 0)?,
            _ => {
                return Err(EncodeError::OperandKind {
                    opcode,
                    index: 0,
                    expected: "a word address register",
                });
            }
        };
        if destination.size != OperandSize::Word {
            return Err(EncodeError::UnsupportedForm {
                opcode,
                reason: "a relocatable near address requires a word destination",
            });
        }
        let kind = X86FixupKind::Absolute16;
        let mut bytes = vec![0x8d, (destination.code << 3) | 0b110];
        let offset = bytes.len() as u32;
        bytes.extend(vec![0; usize::from(kind.width())]);
        return Ok(EncodedInstruction {
            bytes,
            fixups: vec![Fixup {
                offset,
                kind: kind.into(),
                expression: *expression,
                pc_relative: kind.pc_relative(),
            }],
        });
    }

    let bytes = encode_frame_lea(opcode, operands)?;
    Ok(EncodedInstruction {
        bytes,
        fixups: Vec::new(),
    })
}

fn encode_frame_load(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let destination = register_operand(opcode, operands, 0)?;
    let displacement = frame_displacement(opcode, operands, 1, 2)?;
    let mut bytes = prefix_for(destination.size);
    bytes.push(match destination.size {
        OperandSize::Byte => 0x8a,
        OperandSize::Word | OperandSize::Dword => 0x8b,
    });
    bytes.extend(displacement.with_register(destination.code));
    Ok(bytes)
}

fn encode_load(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    if operands.len() == 3 && matches!(operands.get(2), Some(MCOperand::Immediate(_))) {
        return encode_frame_load(opcode, operands);
    }
    if operands.len() == 3 {
        return encode_segmented_load(opcode, operands);
    }
    expect_arity(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    let address = address16_operand(opcode, operands, 1)?;
    let mut bytes = prefix_for(destination.size);
    bytes.push(match destination.size {
        OperandSize::Byte => 0x8a,
        OperandSize::Word | OperandSize::Dword => 0x8b,
    });
    bytes.extend(address.with_register(destination.code));
    Ok(bytes)
}

fn encode_segmented_load(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    require_es_override(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    let address = address16_operand(opcode, operands, 1)?;
    let mut bytes = vec![0x26];
    bytes.extend(prefix_for(destination.size));
    bytes.push(match destination.size {
        OperandSize::Byte => 0x8a,
        OperandSize::Word | OperandSize::Dword => 0x8b,
    });
    bytes.extend(address.with_register(destination.code));
    Ok(bytes)
}

fn encode_frame_store(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let displacement = frame_displacement(opcode, operands, 0, 1)?;
    let source = register_operand(opcode, operands, 2)?;
    let mut bytes = prefix_for(source.size);
    bytes.push(match source.size {
        OperandSize::Byte => 0x88,
        OperandSize::Word | OperandSize::Dword => 0x89,
    });
    bytes.extend(displacement.with_register(source.code));
    Ok(bytes)
}

fn encode_store(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    if operands.len() == 3 && matches!(operands.get(1), Some(MCOperand::Immediate(_))) {
        return encode_frame_store(opcode, operands);
    }
    if operands.len() == 3 {
        return encode_segmented_store(opcode, operands);
    }
    expect_arity(opcode, operands, 2)?;
    let address = address16_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    let mut bytes = prefix_for(source.size);
    bytes.push(match source.size {
        OperandSize::Byte => 0x88,
        OperandSize::Word | OperandSize::Dword => 0x89,
    });
    bytes.extend(address.with_register(source.code));
    Ok(bytes)
}

fn encode_segmented_store(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    require_es_override(opcode, operands, 2)?;
    let address = address16_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    let mut bytes = vec![0x26];
    bytes.extend(prefix_for(source.size));
    bytes.push(match source.size {
        OperandSize::Byte => 0x88,
        OperandSize::Word | OperandSize::Dword => 0x89,
    });
    bytes.extend(address.with_register(source.code));
    Ok(bytes)
}

fn require_es_override(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<(), EncodeError> {
    let Some(MCOperand::Register(register)) = operands.get(index) else {
        return Err(EncodeError::OperandKind {
            opcode,
            index,
            expected: "the ES segment register",
        });
    };
    if decode_register(register.get())? != X86Register::Es {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "the selected segment override must be ES",
        });
    }
    Ok(())
}

fn address16_operand(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<Address16Encoding, EncodeError> {
    register_operand(opcode, operands, index)?;
    let MCOperand::Register(register) = &operands[index] else {
        unreachable!("register_operand accepted the address base");
    };
    let register = decode_register(register.get())?;
    match register {
        X86Register::Bx => Ok(Address16Encoding {
            mode: 0,
            rm: 0b111,
            displacement: None,
        }),
        X86Register::Bp => Ok(Address16Encoding {
            mode: 0b01,
            rm: 0b110,
            displacement: Some(0),
        }),
        X86Register::Si => Ok(Address16Encoding {
            mode: 0,
            rm: 0b100,
            displacement: None,
        }),
        X86Register::Di => Ok(Address16Encoding {
            mode: 0,
            rm: 0b101,
            displacement: None,
        }),
        _ => Err(EncodeError::UnsupportedRegister {
            raw: register as u32,
        }),
    }
}

fn encode_frame_lea(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let destination = register_operand(opcode, operands, 0)?;
    if destination.size == OperandSize::Byte {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "lea has no byte-register destination",
        });
    }
    let displacement = frame_displacement(opcode, operands, 1, 2)?;
    let mut bytes = prefix_for(destination.size);
    bytes.push(0x8d);
    bytes.extend(displacement.with_register(destination.code));
    Ok(bytes)
}

fn frame_displacement(
    opcode: X86Opcode,
    operands: &[MCOperand],
    base_index: usize,
    displacement_index: usize,
) -> Result<FrameDisplacement, EncodeError> {
    let base = register_operand(opcode, operands, base_index)?;
    let base_register = decode_register(match operands[base_index] {
        MCOperand::Register(register) => register.get(),
        _ => unreachable!("register_operand accepted the frame base"),
    })?;
    if base_register != X86Register::Bp || base.size != OperandSize::Word {
        return Err(EncodeError::UnsupportedFrameBase {
            opcode,
            raw: base_register as u32,
        });
    }
    let Some(MCOperand::Immediate(displacement)) = operands.get(displacement_index) else {
        return Err(EncodeError::OperandKind {
            opcode,
            index: displacement_index,
            expected: "an immediate frame displacement",
        });
    };
    Ok(FrameDisplacement::new(*displacement))
}

/// Returns the exact encoded size for one instruction.
pub fn encoded_size(instruction: &MCInstruction) -> Result<u64, EncodeError> {
    Ok(encode_with_fixups(instruction)?.bytes.len() as u64)
}

fn encode_move(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    match &operands[1] {
        MCOperand::Register(_) => {
            let source = register_operand(opcode, operands, 1)?;
            encode_register_binary(opcode, destination, source, 0x88, 0x89)
        }
        MCOperand::Immediate(value) => encode_register_immediate(destination, *value),
        MCOperand::Expression(_) => Err(EncodeError::OperandKind {
            opcode,
            index: 1,
            expected: "a register or immediate",
        }),
    }
}

fn encode_register_operands(
    opcode: X86Opcode,
    operands: &[MCOperand],
    byte_opcode: u8,
    wide_opcode: u8,
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    encode_register_binary(opcode, destination, source, byte_opcode, wide_opcode)
}

fn encode_add_sub(
    opcode: X86Opcode,
    operands: &[MCOperand],
    immediate_extension: u8,
    byte_opcode: u8,
    wide_opcode: u8,
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    if let MCOperand::Immediate(value) = &operands[1] {
        return encode_word_group_one_immediate(opcode, operands, immediate_extension, *value);
    }
    encode_register_operands(opcode, operands, byte_opcode, wide_opcode)
}

fn encode_word_group_one_immediate(
    opcode: X86Opcode,
    operands: &[MCOperand],
    extension: u8,
    value: i64,
) -> Result<Vec<u8>, EncodeError> {
    let destination = register_operand(opcode, operands, 0)?;
    if destination.size != OperandSize::Word {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "group-1 immediate form is implemented only for 16-bit registers",
        });
    }

    // First validate the source spelling under the same signed-or-unsigned
    // 16-bit convention as register moves.  Once narrowed, the low word is
    // the operation's exact modulo-16-bit immediate.  `83 /n ib` is valid
    // precisely when sign-extending that byte reproduces the word.
    let word = encode_immediate(value, OperandSize::Word)?;
    let immediate = u16::from_le_bytes([word[0], word[1]]);
    let signed = immediate as i16;
    if (-128..=127).contains(&signed) {
        Ok(vec![
            0x83,
            modrm(extension, destination.code),
            signed as i8 as u8,
        ])
    } else {
        Ok(vec![
            0x81,
            modrm(extension, destination.code),
            word[0],
            word[1],
        ])
    }
}

fn encode_register_binary(
    opcode: X86Opcode,
    destination: RegisterEncoding,
    source: RegisterEncoding,
    byte_opcode: u8,
    wide_opcode: u8,
) -> Result<Vec<u8>, EncodeError> {
    require_matching_width(opcode, destination, source)?;
    let mut bytes = prefix_for(destination.size);
    bytes.push(match destination.size {
        OperandSize::Byte => byte_opcode,
        OperandSize::Word | OperandSize::Dword => wide_opcode,
    });
    bytes.push(modrm(source.code, destination.code));
    Ok(bytes)
}

fn encode_register_immediate(
    destination: RegisterEncoding,
    value: i64,
) -> Result<Vec<u8>, EncodeError> {
    let mut bytes = prefix_for(destination.size);
    bytes.push(match destination.size {
        OperandSize::Byte => 0xb0 + destination.code,
        OperandSize::Word | OperandSize::Dword => 0xb8 + destination.code,
    });
    bytes.extend(encode_immediate(value, destination.size)?);
    Ok(bytes)
}

fn encode_imul(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    require_matching_width(opcode, destination, source)?;
    if destination.size == OperandSize::Byte || source.size == OperandSize::Byte {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "two-operand imul has no byte-register form",
        });
    }
    let mut bytes = prefix_for(destination.size);
    bytes.extend([0x0f, 0xaf, modrm(destination.code, source.code)]);
    Ok(bytes)
}

fn encode_sign_extend_word_to_dword(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    if destination.size != OperandSize::Dword || source.size != OperandSize::Word {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "movsx r32, r16 requires a dword destination and word source",
        });
    }
    Ok(vec![0x66, 0x0f, 0xbf, modrm(destination.code, source.code)])
}

fn encode_shift_left_double(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let destination = register_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    let Some(MCOperand::Immediate(16)) = operands.get(2) else {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "shift-left-double requires immediate count 16",
        });
    };
    if destination.size != OperandSize::Dword || source.size != OperandSize::Dword {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "shift-left-double requires dword destination and source",
        });
    }
    // SHLD's ModR/M reg field is the source, unlike the destination-first
    // arithmetic forms above: `shld edx,ecx,16` is 66 0f a4 ca 10.
    Ok(vec![
        0x66,
        0x0f,
        0xa4,
        modrm(source.code, destination.code),
        16,
    ])
}

fn encode_unary(
    opcode: X86Opcode,
    operands: &[MCOperand],
    extension: u8,
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 1)?;
    let register = register_operand(opcode, operands, 0)?;
    let mut bytes = prefix_for(register.size);
    bytes.push(match register.size {
        OperandSize::Byte => 0xf6,
        OperandSize::Word | OperandSize::Dword => 0xf7,
    });
    bytes.push(modrm(extension, register.code));
    Ok(bytes)
}

fn encode_push_pop(
    opcode: X86Opcode,
    operands: &[MCOperand],
    base_opcode: u8,
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 1)?;
    if matches!(operands.first(), Some(MCOperand::Register(register)) if decode_register(register.get())? == X86Register::Es)
    {
        return Ok(vec![if base_opcode == 0x50 { 0x06 } else { 0x07 }]);
    }
    let register = register_operand(opcode, operands, 0)?;
    if register.size == OperandSize::Byte {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "push and pop have no byte-register form",
        });
    }
    let mut bytes = prefix_for(register.size);
    bytes.push(base_opcode + register.code);
    Ok(bytes)
}

fn encode_return(
    opcode: X86Opcode,
    operands: &[MCOperand],
    encoding: u8,
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 0)?;
    Ok(vec![encoding])
}

fn encode_far_return(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    match operands {
        [] | [MCOperand::Immediate(0)] => Ok(vec![0xcb]),
        [MCOperand::Immediate(cleanup)] if (1..=i64::from(u16::MAX)).contains(cleanup) => {
            let mut bytes = vec![0xca];
            bytes.extend((*cleanup as u16).to_le_bytes());
            Ok(bytes)
        }
        [MCOperand::Immediate(cleanup)] => Err(EncodeError::ImmediateOutOfRange {
            value: *cleanup,
            bits: 16,
        }),
        [_] => Err(EncodeError::OperandKind {
            opcode,
            index: 0,
            expected: "a u16 stack-cleanup immediate",
        }),
        _ => Err(EncodeError::Arity {
            opcode,
            expected: 1,
            actual: operands.len(),
        }),
    }
}

fn expect_arity(
    opcode: X86Opcode,
    operands: &[MCOperand],
    expected: usize,
) -> Result<(), EncodeError> {
    if operands.len() == expected {
        Ok(())
    } else {
        Err(EncodeError::Arity {
            opcode,
            expected,
            actual: operands.len(),
        })
    }
}

fn register_operand(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<RegisterEncoding, EncodeError> {
    let Some(operand) = operands.get(index) else {
        return Err(EncodeError::Arity {
            opcode,
            expected: index + 1,
            actual: operands.len(),
        });
    };
    let MCOperand::Register(register) = operand else {
        return Err(EncodeError::OperandKind {
            opcode,
            index,
            expected: "a register",
        });
    };
    register_encoding(register.get())
}

fn register_encoding(raw: u32) -> Result<RegisterEncoding, EncodeError> {
    let register = decode_register(raw)?;
    let (code, size) = match register {
        X86Register::Al => (0, OperandSize::Byte),
        X86Register::Cl => (1, OperandSize::Byte),
        X86Register::Dl => (2, OperandSize::Byte),
        X86Register::Bl => (3, OperandSize::Byte),
        X86Register::Ah => (4, OperandSize::Byte),
        X86Register::Ch => (5, OperandSize::Byte),
        X86Register::Dh => (6, OperandSize::Byte),
        X86Register::Bh => (7, OperandSize::Byte),
        X86Register::Ax => (0, OperandSize::Word),
        X86Register::Cx => (1, OperandSize::Word),
        X86Register::Dx => (2, OperandSize::Word),
        X86Register::Bx => (3, OperandSize::Word),
        X86Register::Sp => (4, OperandSize::Word),
        X86Register::Bp => (5, OperandSize::Word),
        X86Register::Si => (6, OperandSize::Word),
        X86Register::Di => (7, OperandSize::Word),
        X86Register::Eax => (0, OperandSize::Dword),
        X86Register::Ecx => (1, OperandSize::Dword),
        X86Register::Edx => (2, OperandSize::Dword),
        X86Register::Ebx => (3, OperandSize::Dword),
        X86Register::Esp => (4, OperandSize::Dword),
        X86Register::Ebp => (5, OperandSize::Dword),
        X86Register::Esi => (6, OperandSize::Dword),
        X86Register::Edi => (7, OperandSize::Dword),
        X86Register::Es
        | X86Register::Cs
        | X86Register::Ss
        | X86Register::Ds
        | X86Register::Fs
        | X86Register::Gs
        | X86Register::St0
        | X86Register::St1
        | X86Register::St2
        | X86Register::St3
        | X86Register::St4
        | X86Register::St5
        | X86Register::St6
        | X86Register::St7 => return Err(EncodeError::UnsupportedRegister { raw }),
    };
    Ok(RegisterEncoding { code, size })
}

fn require_matching_width(
    opcode: X86Opcode,
    left: RegisterEncoding,
    right: RegisterEncoding,
) -> Result<(), EncodeError> {
    if left.size == right.size {
        Ok(())
    } else {
        Err(EncodeError::WidthMismatch {
            opcode,
            left: left.size,
            right: right.size,
        })
    }
}

fn prefix_for(size: OperandSize) -> Vec<u8> {
    match size {
        OperandSize::Dword => vec![0x66],
        OperandSize::Byte | OperandSize::Word => Vec::new(),
    }
}

fn encode_immediate(value: i64, size: OperandSize) -> Result<Vec<u8>, EncodeError> {
    let bits = size.bits();
    let minimum = -(1i64 << (bits - 1));
    let maximum = (1i64 << bits) - 1;
    if !(minimum..=maximum).contains(&value) {
        return Err(EncodeError::ImmediateOutOfRange { value, bits });
    }
    let bytes = (value as u64).to_le_bytes();
    Ok(bytes[..usize::from(bits / 8)].to_vec())
}

fn modrm(reg: u8, rm: u8) -> u8 {
    0xc0 | (reg << 3) | rm
}

/// The 16-bit ModR/M spelling of `[bp+displacement]`.
///
/// Effective offsets wrap at 16 bits.  The conceptual frame depth may be just
/// below -32768 because the measured runtime header sits below a legal
/// 0x7ffe-byte reservation, so the word form deliberately retains the low
/// sixteen bits instead of imposing a signed-i16 source restriction.
struct FrameDisplacement {
    mode: u8,
    bytes: Vec<u8>,
}

struct Address16Encoding {
    mode: u8,
    rm: u8,
    displacement: Option<u8>,
}

impl Address16Encoding {
    fn with_register(self, register: u8) -> Vec<u8> {
        let mut bytes = vec![(self.mode << 6) | (register << 3) | self.rm];
        if let Some(displacement) = self.displacement {
            bytes.push(displacement);
        }
        bytes
    }
}

impl FrameDisplacement {
    fn new(value: i64) -> Self {
        if (-128..=127).contains(&value) {
            Self {
                mode: 0b01,
                bytes: vec![value as i8 as u8],
            }
        } else {
            Self {
                mode: 0b10,
                bytes: (value as u16).to_le_bytes().to_vec(),
            }
        }
    }

    fn with_register(self, register: u8) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(1 + self.bytes.len());
        // In 16-bit addressing r/m=110 denotes BP when mod is nonzero.  The
        // mod=00 spelling is an absolute disp16, so even `[bp]` uses disp8=0.
        encoded.push((self.mode << 6) | (register << 3) | 0b110);
        encoded.extend(self.bytes);
        encoded
    }
}

fn decode_opcode(raw: u32) -> Result<X86Opcode, EncodeError> {
    X86Opcode::from_raw(raw).ok_or(EncodeError::UnknownOpcode { raw })
}

fn decode_register(raw: u32) -> Result<X86Register, EncodeError> {
    X86Register::from_raw(raw).ok_or(EncodeError::UnknownRegister { raw })
}

#[derive(Clone, Copy)]
struct RegisterEncoding {
    code: u8,
    size: OperandSize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::{
        InstructionFlags, MachineBlock, MachineBlockId, MachineCallingConvention, MachineFunction,
        MachineFunctionId, MachineInstruction, MachineInstructionId, MachineLinkage, MachineModule,
        MachineOperand, MachineOperandKind, MachineSignature, OperandRole,
    };
    use crate::mc::{MCExpression, MCOperand, PhysicalRegister, SymbolId, TargetOpcode};
    use crate::target::x86::lower_allocated_module;

    fn instruction(opcode: X86Opcode, operands: Vec<MCOperand>) -> MCInstruction {
        MCInstruction {
            opcode: TargetOpcode::new(opcode as u32),
            operands,
        }
    }

    fn register(register: X86Register) -> MCOperand {
        MCOperand::Register(PhysicalRegister::new(register as u32))
    }

    #[test]
    fn encodes_byte_word_and_dword_moves() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![register(X86Register::Cl), register(X86Register::Ah)],
            ))
            .unwrap(),
            vec![0x88, 0xe1]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![register(X86Register::Bx), register(X86Register::Ax)],
            ))
            .unwrap(),
            vec![0x89, 0xc3]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![
                    register(X86Register::Eax),
                    MCOperand::Immediate(0x1234_5678)
                ],
            ))
            .unwrap(),
            vec![0x66, 0xb8, 0x78, 0x56, 0x34, 0x12]
        );
        assert_eq!(
            encoded_size(&instruction(
                X86Opcode::Mov,
                vec![
                    register(X86Register::Eax),
                    MCOperand::Immediate(0x1234_5678),
                ],
            ))
            .unwrap(),
            6
        );
    }

    #[test]
    fn encodes_destination_first_arithmetic_modrm_and_imul() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Add,
                vec![register(X86Register::Ax), register(X86Register::Bx)],
            ))
            .unwrap(),
            vec![0x01, 0xd8]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Imul,
                vec![register(X86Register::Ax), register(X86Register::Bx)],
            ))
            .unwrap(),
            vec![0x0f, 0xaf, 0xc3]
        );
    }

    #[test]
    fn encodes_sign_extend_word_to_dword_movsx_and_refuses_other_widths() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::SignExtendWordToDword,
                vec![register(X86Register::Eax), register(X86Register::Cx)],
            ))
            .unwrap(),
            vec![0x66, 0x0f, 0xbf, 0xc1]
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::SignExtendWordToDword,
                vec![register(X86Register::Ax), register(X86Register::Cx)],
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::SignExtendWordToDword,
                ..
            })
        ));
    }

    #[test]
    fn encodes_i32_high_return_extract_shld_and_refuses_other_forms() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::ShiftLeftDouble,
                vec![
                    register(X86Register::Edx),
                    register(X86Register::Ecx),
                    MCOperand::Immediate(16),
                ],
            ))
            .unwrap(),
            vec![0x66, 0x0f, 0xa4, 0xca, 0x10]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::ShiftLeftDouble,
                vec![
                    register(X86Register::Edx),
                    register(X86Register::Eax),
                    MCOperand::Immediate(16),
                ],
            ))
            .unwrap(),
            vec![0x66, 0x0f, 0xa4, 0xc2, 0x10]
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::ShiftLeftDouble,
                vec![
                    register(X86Register::Dx),
                    register(X86Register::Ecx),
                    MCOperand::Immediate(16),
                ],
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::ShiftLeftDouble,
                ..
            })
        ));
        assert!(matches!(
            encode(&instruction(
                X86Opcode::ShiftLeftDouble,
                vec![register(X86Register::Edx), register(X86Register::Ecx)],
            )),
            Err(EncodeError::Arity {
                opcode: X86Opcode::ShiftLeftDouble,
                expected: 3,
                actual: 2,
            })
        ));
    }

    #[test]
    fn encodes_word_group_one_stack_adjustments_with_the_shortest_immediate() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Add,
                vec![register(X86Register::Sp), MCOperand::Immediate(4)],
            ))
            .unwrap(),
            vec![0x83, 0xc4, 0x04]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Sub,
                vec![register(X86Register::Sp), MCOperand::Immediate(4)],
            ))
            .unwrap(),
            vec![0x83, 0xec, 0x04]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Add,
                vec![register(X86Register::Sp), MCOperand::Immediate(128)],
            ))
            .unwrap(),
            vec![0x81, 0xc4, 0x80, 0x00]
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::Add,
                vec![register(X86Register::Esp), MCOperand::Immediate(4)],
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::Add,
                ..
            })
        ));
    }

    #[test]
    fn accepts_signed_or_unsigned_immediate_bit_patterns_and_rejects_overflow() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![register(X86Register::Al), MCOperand::Immediate(-128)],
            ))
            .unwrap(),
            vec![0xb0, 0x80]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![register(X86Register::Al), MCOperand::Immediate(255)],
            ))
            .unwrap(),
            vec![0xb0, 0xff]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![register(X86Register::Al), MCOperand::Immediate(256)],
            )),
            Err(EncodeError::ImmediateOutOfRange {
                value: 256,
                bits: 8,
            })
        );
    }

    #[test]
    fn encodes_prefix_unary_push_pop_and_returns() {
        assert_eq!(
            encode(&instruction(X86Opcode::Leave, Vec::new())).unwrap(),
            vec![0xc9]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Neg,
                vec![register(X86Register::Ebx)],
            ))
            .unwrap(),
            vec![0x66, 0xf7, 0xdb]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Push,
                vec![register(X86Register::Eax)],
            ))
            .unwrap(),
            vec![0x66, 0x50]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Pop,
                vec![register(X86Register::Bx)],
            ))
            .unwrap(),
            vec![0x5b]
        );
        assert_eq!(
            encode(&instruction(X86Opcode::ReturnNear, Vec::new())).unwrap(),
            vec![0xc3]
        );
        assert_eq!(
            encode(&instruction(X86Opcode::ReturnFar, Vec::new())).unwrap(),
            vec![0xcb]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::ReturnFar,
                vec![MCOperand::Immediate(0)],
            ))
            .unwrap(),
            vec![0xcb]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::ReturnFar,
                vec![MCOperand::Immediate(4)],
            ))
            .unwrap(),
            vec![0xca, 0x04, 0x00]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::ReturnFar,
                vec![MCOperand::Immediate(-1)],
            )),
            Err(EncodeError::ImmediateOutOfRange {
                value: -1,
                bits: 16,
            })
        );
    }

    #[test]
    fn encodes_materialized_basic_frame_load_store_and_address() {
        // These are the exact post-layout forms behind the Python source
        // regressions for a far-Pascal argument at BP+6 and a VBDOS local
        // below the twenty-byte runtime header.
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Bp),
                    MCOperand::Immediate(6),
                ],
            ))
            .unwrap(),
            vec![0x8b, 0x46, 0x06]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Store,
                vec![
                    register(X86Register::Bp),
                    MCOperand::Immediate(-24),
                    register(X86Register::Eax),
                ],
            ))
            .unwrap(),
            vec![0x66, 0x89, 0x46, 0xe8]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Lea,
                vec![
                    register(X86Register::Bx),
                    register(X86Register::Bp),
                    MCOperand::Immediate(-32_786),
                ],
            ))
            .unwrap(),
            vec![0x8d, 0x9e, 0xee, 0x7f]
        );
    }

    #[test]
    fn encodes_legal_sixteen_bit_register_indirect_loads_and_stores() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![register(X86Register::Ax), register(X86Register::Bx)],
            ))
            .unwrap(),
            vec![0x8b, 0x07]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Store,
                vec![register(X86Register::Si), register(X86Register::Eax)],
            ))
            .unwrap(),
            vec![0x66, 0x89, 0x04]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![register(X86Register::Ax), register(X86Register::Bp)],
            ))
            .unwrap(),
            vec![0x8b, 0x46, 0x00]
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::Load,
                vec![register(X86Register::Ax), register(X86Register::Cx)],
            )),
            Err(EncodeError::UnsupportedRegister { .. })
        ));
    }

    #[test]
    fn encodes_the_exact_balanced_es_pointer_access_sequences() {
        let load = [
            instruction(X86Opcode::Push, vec![register(X86Register::Es)]),
            instruction(X86Opcode::Push, vec![register(X86Register::Eax)]),
            instruction(X86Opcode::Pop, vec![register(X86Register::Bx)]),
            instruction(X86Opcode::Pop, vec![register(X86Register::Es)]),
            instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Cx),
                    register(X86Register::Bx),
                    register(X86Register::Es),
                ],
            ),
            instruction(X86Opcode::Pop, vec![register(X86Register::Es)]),
        ];
        let store = [
            instruction(X86Opcode::Push, vec![register(X86Register::Es)]),
            instruction(X86Opcode::Push, vec![register(X86Register::Eax)]),
            instruction(X86Opcode::Pop, vec![register(X86Register::Bx)]),
            instruction(X86Opcode::Pop, vec![register(X86Register::Es)]),
            instruction(
                X86Opcode::Store,
                vec![
                    register(X86Register::Bx),
                    register(X86Register::Cx),
                    register(X86Register::Es),
                ],
            ),
            instruction(X86Opcode::Pop, vec![register(X86Register::Es)]),
        ];

        let bytes = |sequence: &[MCInstruction]| {
            sequence
                .iter()
                .flat_map(|instruction| encode(instruction).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            bytes(&load),
            vec![0x06, 0x66, 0x50, 0x5b, 0x07, 0x26, 0x8b, 0x0f, 0x07]
        );
        assert_eq!(
            bytes(&store),
            vec![0x06, 0x66, 0x50, 0x5b, 0x07, 0x26, 0x89, 0x0f, 0x07]
        );

        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Eax),
                    register(X86Register::Bx),
                    register(X86Register::Es),
                ],
            ))
            .unwrap(),
            vec![0x26, 0x66, 0x8b, 0x07]
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Bx),
                    register(X86Register::Ds),
                ],
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::Load,
                ..
            })
        ));
    }

    #[test]
    fn bp_zero_uses_a_displacement_and_non_bp_frames_are_refused() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Al),
                    register(X86Register::Bp),
                    MCOperand::Immediate(0),
                ],
            ))
            .unwrap(),
            vec![0x8a, 0x46, 0x00]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Bx),
                    MCOperand::Immediate(6),
                ],
            )),
            Err(EncodeError::UnsupportedFrameBase {
                opcode: X86Opcode::Load,
                raw: X86Register::Bx as u32,
            })
        );
    }

    #[test]
    fn refuses_relocations_and_unknown_target_ids() {
        assert!(matches!(
            encode(&instruction(X86Opcode::Jump, vec![MCOperand::Immediate(0)],)),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::Jump,
                ..
            })
        ));
        assert_eq!(
            encode(&MCInstruction {
                opcode: TargetOpcode::new(99),
                operands: Vec::new(),
            }),
            Err(EncodeError::UnknownOpcode { raw: 99 })
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::Mov,
                vec![
                    register(X86Register::Ax),
                    MCOperand::Expression(MCExpression {
                        symbol: SymbolId::new(0),
                        addend: 0,
                    }),
                ],
            )),
            Err(EncodeError::OperandKind {
                opcode: X86Opcode::Mov,
                index: 1,
                ..
            })
        ));
    }

    #[test]
    fn encodes_far_call_with_one_symbolic_far_pointer_fixup() {
        // The external far target's addend belongs to the
        // expression exactly once, not to the four zero bytes of `call far`.
        let expression = MCExpression {
            symbol: SymbolId::new(7),
            addend: -12,
        };
        let encoded = encode_with_fixups(&instruction(
            X86Opcode::CallFar,
            vec![MCOperand::Expression(expression)],
        ))
        .unwrap();

        assert_eq!(encoded.bytes, vec![0x9a, 0, 0, 0, 0]);
        assert_eq!(
            encoded_size(&instruction(
                X86Opcode::CallFar,
                vec![MCOperand::Expression(expression)],
            ))
            .unwrap(),
            5
        );
        assert_eq!(
            encoded.fixups,
            vec![Fixup {
                offset: 1,
                kind: X86FixupKind::FarPointer1616.into(),
                expression,
                pc_relative: X86FixupKind::FarPointer1616.pc_relative(),
            }]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::CallFar,
                vec![MCOperand::Expression(expression)],
            )),
            Err(EncodeError::FixupsRequired {
                opcode: X86Opcode::CallFar,
                count: 1,
            })
        );
    }

    #[test]
    fn encodes_near_call_with_one_pc_relative_fixup() {
        let expression = MCExpression {
            symbol: SymbolId::new(7),
            addend: -12,
        };
        let instruction = instruction(X86Opcode::CallNear, vec![MCOperand::Expression(expression)]);

        let encoded = encode_with_fixups(&instruction).unwrap();

        assert_eq!(encoded.bytes, vec![0xe8, 0, 0]);
        assert_eq!(encoded.fixups.len(), 1);
        assert_eq!(
            encoded.fixups[0],
            Fixup {
                offset: 1,
                kind: X86FixupKind::PcRelative16.into(),
                expression,
                pc_relative: true,
            }
        );
        assert_eq!(encoded_size(&instruction), Ok(3));
    }

    #[test]
    fn encodes_a_relocatable_near_address_with_one_absolute_fixup() {
        let expression = MCExpression {
            symbol: SymbolId::new(5),
            addend: 12,
        };
        let instruction = instruction(
            X86Opcode::Lea,
            vec![register(X86Register::Bx), MCOperand::Expression(expression)],
        );

        let encoded = encode_with_fixups(&instruction).unwrap();

        assert_eq!(encoded.bytes, vec![0x8d, 0x1e, 0, 0]);
        assert_eq!(
            encoded.fixups,
            vec![Fixup {
                offset: 2,
                kind: X86FixupKind::Absolute16.into(),
                expression,
                pc_relative: false,
            }]
        );
        assert_eq!(
            encode(&instruction),
            Err(EncodeError::FixupsRequired {
                opcode: X86Opcode::Lea,
                count: 1,
            })
        );
        assert_eq!(encoded_size(&instruction), Ok(4));
    }

    #[test]
    fn refuses_extra_far_call_operands() {
        let expression = MCExpression {
            symbol: SymbolId::new(3),
            addend: 0,
        };
        assert_eq!(
            encode_with_fixups(&instruction(
                X86Opcode::CallFar,
                vec![MCOperand::Expression(expression), register(X86Register::Ax)],
            )),
            Err(EncodeError::Arity {
                opcode: X86Opcode::CallFar,
                expected: 1,
                actual: 2,
            })
        );
    }

    #[test]
    fn refuses_missing_or_non_symbolic_far_call_targets() {
        assert_eq!(
            encode_with_fixups(&instruction(X86Opcode::CallFar, Vec::new())),
            Err(EncodeError::Arity {
                opcode: X86Opcode::CallFar,
                expected: 1,
                actual: 0,
            })
        );
        assert_eq!(
            encode_with_fixups(&instruction(
                X86Opcode::CallFar,
                vec![MCOperand::Immediate(0)],
            )),
            Err(EncodeError::OperandKind {
                opcode: X86Opcode::CallFar,
                index: 0,
                expected: "a symbolic far-call target",
            })
        );
    }

    #[test]
    fn lowers_an_external_far_call_then_preserves_its_target_through_encoding() {
        // The module boundary creates the external MC symbol and expression;
        // encoding must retain it through the fixup rather than interpreting
        // it as an immediate field.
        let module = MachineModule {
            data_objects: Vec::new(),
            functions: vec![MachineFunction {
                id: MachineFunctionId::new(0),
                name: "caller".to_owned(),
                linkage: MachineLinkage::External,
                signature: MachineSignature {
                    result: None,
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: MachineCallingConvention::C,
                },
                entry: MachineBlockId::new(0),
                virtual_registers: Vec::new(),
                blocks: vec![MachineBlock {
                    id: MachineBlockId::new(0),
                    instructions: vec![MachineInstruction {
                        id: MachineInstructionId::new(0),
                        opcode: X86Opcode::CallFar.machine_opcode(),
                        operands: vec![MachineOperand {
                            kind: MachineOperandKind::ExternalSymbol {
                                name: "runtime".to_owned(),
                                addend: 6,
                            },
                            role: OperandRole::None,
                            constraint: None,
                            tied_to: None,
                        }],
                        flags: InstructionFlags::NONE,
                    }],
                    successors: Vec::new(),
                }],
                frame_objects: Vec::new(),
            }],
        };

        let lowered = lower_allocated_module(&module).unwrap();
        let instruction = match &lowered.sections[0].fragments[1] {
            crate::mc::MCFragment::Instruction(fragment) => &fragment.instruction,
            _ => panic!("the external call follows the entry anchor"),
        };
        let runtime = lowered
            .symbols
            .iter()
            .find(|symbol| symbol.name == "runtime")
            .unwrap();
        let encoded = encode_with_fixups(instruction).unwrap();

        assert_eq!(encoded.bytes, vec![0x9a, 0, 0, 0, 0]);
        assert_eq!(
            encoded.fixups[0].expression,
            MCExpression {
                symbol: runtime.id,
                addend: 6,
            }
        );
    }
}
