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

use crate::old::mc::{Fixup, MCInstruction, MCOperand};

use super::instructions::X87MemoryFormat;
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
        X86Opcode::Or => encode_group_one_binary(opcode, &instruction.operands, 1, 0x08, 0x09),
        X86Opcode::Xor => encode_register_operands(opcode, &instruction.operands, 0x30, 0x31),
        X86Opcode::Cmp => encode_register_operands(opcode, &instruction.operands, 0x38, 0x39),
        X86Opcode::Test => encode_register_operands(opcode, &instruction.operands, 0x84, 0x85),
        X86Opcode::Imul => encode_imul(opcode, &instruction.operands),
        X86Opcode::SignExtendWordToDword => {
            encode_sign_extend_word_to_dword(opcode, &instruction.operands)
        }
        X86Opcode::ZeroExtendWordToDword => {
            encode_zero_extend_word_to_dword(opcode, &instruction.operands)
        }
        X86Opcode::CwdCdq => encode_cwd_cdq(opcode, &instruction.operands),
        X86Opcode::Div | X86Opcode::Idiv => encode_divide(opcode, &instruction.operands),
        X86Opcode::ShiftLeftDouble => encode_shift_left_double(opcode, &instruction.operands),
        X86Opcode::Neg => encode_unary(opcode, &instruction.operands, 3),
        X86Opcode::Not => encode_unary(opcode, &instruction.operands, 2),
        X86Opcode::Push => return encode_push(opcode, &instruction.operands),
        X86Opcode::Pop => encode_push_pop(opcode, &instruction.operands, 0x58),
        X86Opcode::Leave => encode_return(opcode, &instruction.operands, 0xc9),
        X86Opcode::ReturnNear => encode_return(opcode, &instruction.operands, 0xc3),
        X86Opcode::ReturnFar => encode_far_return(opcode, &instruction.operands),
        X86Opcode::Lea => return encode_lea(opcode, &instruction.operands),
        X86Opcode::Load => return encode_load_with_fixups(opcode, &instruction.operands),
        X86Opcode::Store => return encode_store(opcode, &instruction.operands),
        X86Opcode::CallFar => return encode_far_call(opcode, &instruction.operands),
        X86Opcode::CallNear => return encode_near_call(opcode, &instruction.operands),
        X86Opcode::X87Load
        | X86Opcode::X87Store
        | X86Opcode::X87StorePop
        | X86Opcode::X87IntegerLoad
        | X86Opcode::X87IntegerStore
        | X86Opcode::X87IntegerStorePop
        | X86Opcode::X87Add
        | X86Opcode::X87Subtract
        | X86Opcode::X87SubtractReverse
        | X86Opcode::X87Multiply
        | X86Opcode::X87Divide
        | X86Opcode::X87DivideReverse
        | X86Opcode::X87Compare
        | X86Opcode::X87ComparePop
        | X86Opcode::X87StoreControlWord
        | X86Opcode::X87LoadControlWord => {
            return encode_x87_memory_or_stack(opcode, &instruction.operands);
        }
        X86Opcode::X87IntegerStoreTrunc => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "x87 truncating integer stores must be expanded to a control-word sequence before MC",
        }),
        X86Opcode::X87AddPop
        | X86Opcode::X87SubtractPop
        | X86Opcode::X87SubtractReversePop
        | X86Opcode::X87MultiplyPop
        | X86Opcode::X87DividePop
        | X86Opcode::X87DivideReversePop => {
            encode_x87_pop_arithmetic(opcode, &instruction.operands)
        }
        X86Opcode::X87ComparePop2 => encode_x87_compare_pop2(opcode, &instruction.operands),
        X86Opcode::X87StackLoad => encode_x87_stack_load(opcode, &instruction.operands),
        X86Opcode::X87StackStorePop => encode_x87_stack_store_pop(opcode, &instruction.operands),
        X86Opcode::X87Exchange => encode_x87_exchange(opcode, &instruction.operands),
        X86Opcode::X87LoadZero => encode_x87_unary(opcode, &instruction.operands, 0xd9, 0xee),
        X86Opcode::X87LoadOne => encode_x87_unary(opcode, &instruction.operands, 0xd9, 0xe8),
        X86Opcode::X87ChangeSign => encode_x87_unary(opcode, &instruction.operands, 0xd9, 0xe0),
        X86Opcode::X87Absolute => encode_x87_unary(opcode, &instruction.operands, 0xd9, 0xe1),
        X86Opcode::X87SquareRoot => encode_x87_unary(opcode, &instruction.operands, 0xd9, 0xfa),
        X86Opcode::X87StoreStatusWord => {
            encode_x87_store_status_word(opcode, &instruction.operands)
        }
        X86Opcode::Wait => encode_return(opcode, &instruction.operands, 0x9b),
        X86Opcode::Sahf => encode_return(opcode, &instruction.operands, 0x9e),
        X86Opcode::ShiftLeft
        | X86Opcode::ShiftRightLogical
        | X86Opcode::ShiftRightArithmetic
        | X86Opcode::Jump
        | X86Opcode::JumpConditional
        | X86Opcode::MergeWords
        | X86Opcode::LowWord
        | X86Opcode::HighWord
        | X86Opcode::Nothing => Err(EncodeError::UnsupportedForm {
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
        // A symbolic LEA materializes a word offset in the x86 near-data
        // address space.  It is not an ordinary target-segment offset: OMF
        // needs that distinction to select the DGROUP frame.
        let kind = X86FixupKind::NearData16;
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

    let bytes = encode_displacement_lea(opcode, operands)?;
    Ok(EncodedInstruction {
        bytes,
        fixups: Vec::new(),
    })
}

fn encode_displacement_load(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let destination = register_operand(opcode, operands, 0)?;
    let displacement = address16_displacement(opcode, operands, 1, 2)?;
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
        return encode_displacement_load(opcode, operands);
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

fn encode_load_with_fixups(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    if let [MCOperand::Register(_), MCOperand::Expression(expression)] = operands {
        let destination = register_operand(opcode, operands, 0)?;
        let mut bytes = prefix_for(destination.size);
        bytes.push(match destination.size {
            OperandSize::Byte => 0x8a,
            OperandSize::Word | OperandSize::Dword => 0x8b,
        });
        bytes.push((destination.code << 3) | 0b110);
        let offset = bytes.len() as u32;
        bytes.extend([0, 0]);
        return Ok(EncodedInstruction {
            bytes,
            fixups: vec![Fixup {
                offset,
                kind: X86FixupKind::NearData16.into(),
                expression: *expression,
                pc_relative: false,
            }],
        });
    }

    Ok(EncodedInstruction {
        bytes: encode_load(opcode, operands)?,
        fixups: Vec::new(),
    })
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

fn encode_displacement_store(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let displacement = address16_displacement(opcode, operands, 0, 1)?;
    let source = register_operand(opcode, operands, 2)?;
    let mut bytes = prefix_for(source.size);
    bytes.push(match source.size {
        OperandSize::Byte => 0x88,
        OperandSize::Word | OperandSize::Dword => 0x89,
    });
    bytes.extend(displacement.with_register(source.code));
    Ok(bytes)
}

fn encode_store(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    if matches!(
        operands.get(operands.len().saturating_sub(2)),
        Some(MCOperand::Immediate(_))
    ) && matches!(operands.last(), Some(MCOperand::Immediate(_)))
    {
        return encode_immediate_store(opcode, operands);
    }
    if let [MCOperand::Expression(expression), MCOperand::Register(_)] = operands {
        let source = register_operand(opcode, operands, 1)?;
        let mut bytes = prefix_for(source.size);
        bytes.push(match source.size {
            OperandSize::Byte => 0x88,
            OperandSize::Word | OperandSize::Dword => 0x89,
        });
        bytes.push((source.code << 3) | 0b110);
        let offset = bytes.len() as u32;
        bytes.extend([0, 0]);
        return Ok(EncodedInstruction {
            bytes,
            fixups: vec![Fixup {
                offset,
                kind: X86FixupKind::NearData16.into(),
                expression: *expression,
                pc_relative: false,
            }],
        });
    }
    let bytes = if operands.len() == 3 && matches!(operands.get(1), Some(MCOperand::Immediate(_))) {
        encode_displacement_store(opcode, operands)?
    } else if operands.len() == 3 {
        encode_segmented_store(opcode, operands)?
    } else {
        expect_arity(opcode, operands, 2)?;
        let address = address16_operand(opcode, operands, 0)?;
        let source = register_operand(opcode, operands, 1)?;
        let mut bytes = prefix_for(source.size);
        bytes.push(match source.size {
            OperandSize::Byte => 0x88,
            OperandSize::Word | OperandSize::Dword => 0x89,
        });
        bytes.extend(address.with_register(source.code));
        bytes
    };
    Ok(EncodedInstruction {
        bytes,
        fixups: Vec::new(),
    })
}

fn encode_immediate_store(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    let address_end = operands.len().checked_sub(2).ok_or(EncodeError::Arity {
        opcode,
        expected: 3,
        actual: operands.len(),
    })?;
    let [MCOperand::Immediate(bits), MCOperand::Immediate(value)] = &operands[address_end..] else {
        unreachable!("the caller identified two trailing immediates")
    };
    let size = store_immediate_size(opcode, *bits)?;
    let lead = if size == OperandSize::Byte {
        0xc6
    } else {
        0xc7
    };
    let mut encoded = encode_memory(opcode, &operands[..address_end], 0, lead, 0)?;
    let prefix = prefix_for(size);
    if !prefix.is_empty() {
        encoded.bytes.splice(0..0, prefix.iter().copied());
        let adjustment = u32::try_from(prefix.len()).expect("x86 prefix count fits u32");
        for fixup in &mut encoded.fixups {
            fixup.offset += adjustment;
        }
    }
    encoded.bytes.extend(encode_immediate(*value, size)?);
    Ok(encoded)
}

fn store_immediate_size(opcode: X86Opcode, bits: i64) -> Result<OperandSize, EncodeError> {
    match bits {
        8 => Ok(OperandSize::Byte),
        16 => Ok(OperandSize::Word),
        32 => Ok(OperandSize::Dword),
        _ => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "store immediate width must be 8, 16, or 32 bits",
        }),
    }
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

/// Encodes the target-owned x87 surface without changing generic MC address
/// operands.  Memory forms are `[st(0), format, address...]`; control-word
/// forms omit the stack register and are `[format, address...]`.  A format is
/// deliberately an immediate so the physical byte representation is not
/// inferred from an SSA or Machine-IR type.
fn encode_x87_memory_or_stack(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    match opcode {
        X86Opcode::X87StoreControlWord | X86Opcode::X87LoadControlWord => {
            let format = x87_memory_format(opcode, operands, 0)?;
            let extension = if opcode == X86Opcode::X87StoreControlWord {
                7
            } else {
                5
            };
            require_x87_format(opcode, format, &[X87MemoryFormat::Control16])?;
            encode_memory(opcode, operands, 1, 0xd9, extension)
        }
        X86Opcode::X87Load
        | X86Opcode::X87Store
        | X86Opcode::X87StorePop
        | X86Opcode::X87IntegerLoad
        | X86Opcode::X87IntegerStore
        | X86Opcode::X87IntegerStorePop => {
            expect_x87_st0(opcode, operands, 0)?;
            let format = x87_memory_format(opcode, operands, 1)?;
            let (byte, extension) = match opcode {
                X86Opcode::X87Load => match format {
                    X87MemoryFormat::Float32 => (0xd9, 0),
                    X87MemoryFormat::Float64 => (0xdd, 0),
                    X87MemoryFormat::Float80 => (0xdb, 5),
                    _ => return unsupported_x87_format(opcode, format),
                },
                X86Opcode::X87Store => match format {
                    X87MemoryFormat::Float32 => (0xd9, 2),
                    X87MemoryFormat::Float64 => (0xdd, 2),
                    _ => return unsupported_x87_format(opcode, format),
                },
                X86Opcode::X87StorePop => match format {
                    X87MemoryFormat::Float32 => (0xd9, 3),
                    X87MemoryFormat::Float64 => (0xdd, 3),
                    X87MemoryFormat::Float80 => (0xdb, 7),
                    _ => return unsupported_x87_format(opcode, format),
                },
                X86Opcode::X87IntegerLoad => match format {
                    X87MemoryFormat::Signed16 => (0xdf, 0),
                    X87MemoryFormat::Signed32 => (0xdb, 0),
                    X87MemoryFormat::Signed64 => (0xdf, 5),
                    _ => return unsupported_x87_format(opcode, format),
                },
                X86Opcode::X87IntegerStore => match format {
                    X87MemoryFormat::Signed16 => (0xdf, 2),
                    X87MemoryFormat::Signed32 => (0xdb, 2),
                    _ => return unsupported_x87_format(opcode, format),
                },
                X86Opcode::X87IntegerStorePop => match format {
                    X87MemoryFormat::Signed16 => (0xdf, 3),
                    X87MemoryFormat::Signed32 => (0xdb, 3),
                    X87MemoryFormat::Signed64 => (0xdf, 7),
                    _ => return unsupported_x87_format(opcode, format),
                },
                _ => unreachable!("the outer match lists every x87 memory opcode"),
            };
            encode_memory(opcode, operands, 2, byte, extension)
        }
        X86Opcode::X87Add
        | X86Opcode::X87Subtract
        | X86Opcode::X87SubtractReverse
        | X86Opcode::X87Multiply
        | X86Opcode::X87Divide
        | X86Opcode::X87DivideReverse
        | X86Opcode::X87Compare
        | X86Opcode::X87ComparePop => {
            if matches!(operands.get(1), Some(MCOperand::Immediate(_))) {
                expect_x87_st0(opcode, operands, 0)?;
                let format = x87_memory_format(opcode, operands, 1)?;
                let extension = x87_memory_arithmetic_extension(opcode)?;
                let byte = match format {
                    X87MemoryFormat::Float32 => 0xd8,
                    X87MemoryFormat::Float64 => 0xdc,
                    X87MemoryFormat::Signed16
                        if !matches!(opcode, X86Opcode::X87Compare | X86Opcode::X87ComparePop) =>
                    {
                        0xde
                    }
                    X87MemoryFormat::Signed32
                        if !matches!(opcode, X86Opcode::X87Compare | X86Opcode::X87ComparePop) =>
                    {
                        0xda
                    }
                    _ => return unsupported_x87_format(opcode, format),
                };
                encode_memory(opcode, operands, 2, byte, extension)
            } else {
                encode_x87_stack_arithmetic(opcode, operands)
            }
        }
        _ => unreachable!("the outer match lists only x87 memory or stack opcodes"),
    }
}

fn x87_memory_arithmetic_extension(opcode: X86Opcode) -> Result<u8, EncodeError> {
    match opcode {
        X86Opcode::X87Add => Ok(0),
        X86Opcode::X87Multiply => Ok(1),
        X86Opcode::X87Compare => Ok(2),
        X86Opcode::X87ComparePop => Ok(3),
        X86Opcode::X87Subtract => Ok(4),
        X86Opcode::X87SubtractReverse => Ok(5),
        X86Opcode::X87Divide => Ok(6),
        X86Opcode::X87DivideReverse => Ok(7),
        _ => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "not an x87 memory arithmetic operation",
        }),
    }
}

fn encode_x87_stack_arithmetic(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = x87_stack_operand(opcode, operands, 0)?;
    let source = x87_stack_operand(opcode, operands, 1)?;
    let byte = match opcode {
        X86Opcode::X87Add => x87_binary_byte(opcode, destination, source, 0xc0, 0xc0)?,
        X86Opcode::X87Subtract => x87_binary_byte(opcode, destination, source, 0xe0, 0xe8)?,
        X86Opcode::X87SubtractReverse => x87_binary_byte(opcode, destination, source, 0xe8, 0xe0)?,
        X86Opcode::X87Multiply => x87_binary_byte(opcode, destination, source, 0xc8, 0xc8)?,
        X86Opcode::X87Divide => x87_binary_byte(opcode, destination, source, 0xf0, 0xf8)?,
        X86Opcode::X87DivideReverse => x87_binary_byte(opcode, destination, source, 0xf8, 0xf0)?,
        X86Opcode::X87Compare => {
            require_x87_stack_zero(opcode, destination)?;
            0xd0 + source
        }
        X86Opcode::X87ComparePop => {
            require_x87_stack_zero(opcode, destination)?;
            0xd8 + source
        }
        _ => unreachable!("the caller passes only x87 stack arithmetic opcodes"),
    };
    let lead = if destination == 0 { 0xd8 } else { 0xdc };
    let lead = match opcode {
        X86Opcode::X87Compare | X86Opcode::X87ComparePop => 0xd8,
        _ => lead,
    };
    Ok(EncodedInstruction {
        bytes: vec![lead, byte],
        fixups: Vec::new(),
    })
}

fn x87_binary_byte(
    opcode: X86Opcode,
    destination: u8,
    source: u8,
    st0_destination: u8,
    sti_destination: u8,
) -> Result<u8, EncodeError> {
    if destination == 0 {
        Ok(st0_destination + source)
    } else if source == 0 {
        Ok(sti_destination + destination)
    } else {
        Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "x87 register arithmetic requires st(0) as one operand",
        })
    }
}

fn encode_x87_pop_arithmetic(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = x87_stack_operand(opcode, operands, 0)?;
    expect_x87_st0(opcode, operands, 1)?;
    let byte = match opcode {
        X86Opcode::X87AddPop => 0xc0,
        X86Opcode::X87MultiplyPop => 0xc8,
        X86Opcode::X87SubtractReversePop => 0xe0,
        X86Opcode::X87SubtractPop => 0xe8,
        X86Opcode::X87DivideReversePop => 0xf0,
        X86Opcode::X87DividePop => 0xf8,
        _ => unreachable!("the caller passes only x87 pop arithmetic opcodes"),
    };
    Ok(vec![0xde, byte + destination])
}

fn encode_x87_compare_pop2(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    expect_x87_st0(opcode, operands, 0)?;
    if x87_stack_operand(opcode, operands, 1)? != 1 {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "fcompp requires st(0) and st(1)",
        });
    }
    Ok(vec![0xde, 0xd9])
}

fn encode_x87_stack_load(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    expect_x87_st0(opcode, operands, 0)?;
    Ok(vec![0xd9, 0xc0 + x87_stack_operand(opcode, operands, 1)?])
}

fn encode_x87_stack_store_pop(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = x87_stack_operand(opcode, operands, 0)?;
    expect_x87_st0(opcode, operands, 1)?;
    Ok(vec![0xdd, 0xd8 + destination])
}

fn encode_x87_exchange(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    expect_x87_st0(opcode, operands, 0)?;
    Ok(vec![0xd9, 0xc8 + x87_stack_operand(opcode, operands, 1)?])
}

fn encode_x87_unary(
    opcode: X86Opcode,
    operands: &[MCOperand],
    lead: u8,
    byte: u8,
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 1)?;
    expect_x87_st0(opcode, operands, 0)?;
    Ok(vec![lead, byte])
}

fn encode_x87_store_status_word(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 1)?;
    exact_x87_register(opcode, operands, 0, X86Register::Ax)?;
    Ok(vec![0xdf, 0xe0])
}

fn x87_memory_format(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<X87MemoryFormat, EncodeError> {
    let Some(MCOperand::Immediate(raw)) = operands.get(index) else {
        return Err(EncodeError::OperandKind {
            opcode,
            index,
            expected: "an x87 memory-format immediate",
        });
    };
    let Ok(raw) = u8::try_from(*raw) else {
        return Err(EncodeError::OperandKind {
            opcode,
            index,
            expected: "a valid x87 memory-format immediate",
        });
    };
    X87MemoryFormat::from_raw(raw).ok_or(EncodeError::OperandKind {
        opcode,
        index,
        expected: "a valid x87 memory-format immediate",
    })
}

fn require_x87_format(
    opcode: X86Opcode,
    format: X87MemoryFormat,
    allowed: &[X87MemoryFormat],
) -> Result<(), EncodeError> {
    if allowed.contains(&format) {
        Ok(())
    } else {
        unsupported_x87_format(opcode, format)
    }
}

fn unsupported_x87_format<T>(
    opcode: X86Opcode,
    _format: X87MemoryFormat,
) -> Result<T, EncodeError> {
    Err(EncodeError::UnsupportedForm {
        opcode,
        reason: "the x87 instruction has no encoding for this memory format",
    })
}

fn encode_memory(
    opcode: X86Opcode,
    operands: &[MCOperand],
    address_index: usize,
    lead: u8,
    extension: u8,
) -> Result<EncodedInstruction, EncodeError> {
    let tail = operands.get(address_index..).unwrap_or_default();
    match tail {
        [MCOperand::Register(_)] => {
            let address = address16_operand(opcode, operands, address_index)?;
            let mut bytes = vec![lead];
            bytes.extend(address.with_register(extension));
            Ok(EncodedInstruction {
                bytes,
                fixups: Vec::new(),
            })
        }
        [MCOperand::Register(_), MCOperand::Immediate(_)] => {
            let displacement =
                address16_displacement(opcode, operands, address_index, address_index + 1)?;
            let mut bytes = vec![lead];
            bytes.extend(displacement.with_register(extension));
            Ok(EncodedInstruction {
                bytes,
                fixups: Vec::new(),
            })
        }
        [MCOperand::Register(_), MCOperand::Register(_)] => {
            require_es_override(opcode, operands, address_index + 1)?;
            let address = address16_operand(opcode, operands, address_index)?;
            let mut bytes = vec![0x26, lead];
            bytes.extend(address.with_register(extension));
            Ok(EncodedInstruction {
                bytes,
                fixups: Vec::new(),
            })
        }
        [MCOperand::Expression(expression)] => Ok(EncodedInstruction {
            bytes: vec![lead, extension << 3 | 0b110, 0, 0],
            fixups: vec![Fixup {
                offset: 2,
                kind: X86FixupKind::NearData16.into(),
                expression: *expression,
                pc_relative: false,
            }],
        }),
        _ => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "memory operand requires one address16 register, address16 plus displacement, address16 plus ES, or one symbolic near-data address",
        }),
    }
}

fn x87_stack_operand(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<u8, EncodeError> {
    let register = architectural_register(opcode, operands, index)?;
    match register {
        X86Register::St0 => Ok(0),
        X86Register::St1 => Ok(1),
        X86Register::St2 => Ok(2),
        X86Register::St3 => Ok(3),
        X86Register::St4 => Ok(4),
        X86Register::St5 => Ok(5),
        X86Register::St6 => Ok(6),
        X86Register::St7 => Ok(7),
        _ => Err(EncodeError::UnsupportedRegister {
            raw: register as u32,
        }),
    }
}

fn expect_x87_st0(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<(), EncodeError> {
    require_x87_stack_zero(opcode, x87_stack_operand(opcode, operands, index)?)
}

fn require_x87_stack_zero(opcode: X86Opcode, stack: u8) -> Result<(), EncodeError> {
    if stack == 0 {
        Ok(())
    } else {
        Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "this x87 form requires st(0)",
        })
    }
}

fn exact_x87_register(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
    expected: X86Register,
) -> Result<(), EncodeError> {
    let actual = architectural_register(opcode, operands, index)?;
    if actual == expected {
        Ok(())
    } else {
        Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "the x87 instruction's explicit register does not match its architectural role",
        })
    }
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
            displacement: Vec::new(),
        }),
        X86Register::Bp => Ok(Address16Encoding {
            mode: 0b01,
            rm: 0b110,
            displacement: vec![0],
        }),
        X86Register::Si => Ok(Address16Encoding {
            mode: 0,
            rm: 0b100,
            displacement: Vec::new(),
        }),
        X86Register::Di => Ok(Address16Encoding {
            mode: 0,
            rm: 0b101,
            displacement: Vec::new(),
        }),
        _ => Err(EncodeError::UnsupportedRegister {
            raw: register as u32,
        }),
    }
}

fn address16_displacement(
    opcode: X86Opcode,
    operands: &[MCOperand],
    base_index: usize,
    displacement_index: usize,
) -> Result<Address16Encoding, EncodeError> {
    let base = address16_operand(opcode, operands, base_index)?;
    let Some(MCOperand::Immediate(displacement)) = operands.get(displacement_index) else {
        return Err(EncodeError::OperandKind {
            opcode,
            index: displacement_index,
            expected: "an immediate address displacement",
        });
    };
    if *displacement == 0 && base.rm != 0b110 {
        return Ok(Address16Encoding {
            mode: 0,
            rm: base.rm,
            displacement: Vec::new(),
        });
    }
    let (mode, bytes) = if (-128..=127).contains(displacement) {
        (0b01, vec![*displacement as i8 as u8])
    } else {
        (0b10, (*displacement as u16).to_le_bytes().to_vec())
    };
    Ok(Address16Encoding {
        mode,
        rm: base.rm,
        displacement: bytes,
    })
}

fn encode_displacement_lea(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 3)?;
    let destination = register_operand(opcode, operands, 0)?;
    if destination.size == OperandSize::Byte {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "lea has no byte-register destination",
        });
    }
    let displacement = address16_displacement(opcode, operands, 1, 2)?;
    let mut bytes = prefix_for(destination.size);
    bytes.push(0x8d);
    bytes.extend(displacement.with_register(destination.code));
    Ok(bytes)
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
    encode_group_one_binary(
        opcode,
        operands,
        immediate_extension,
        byte_opcode,
        wide_opcode,
    )
}

fn encode_group_one_binary(
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

fn encode_zero_extend_word_to_dword(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let destination = register_operand(opcode, operands, 0)?;
    let source = register_operand(opcode, operands, 1)?;
    if destination.size != OperandSize::Dword || source.size != OperandSize::Word {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "movzx r32, r16 requires a dword destination and word source",
        });
    }
    Ok(vec![0x66, 0x0f, 0xb7, modrm(destination.code, source.code)])
}

fn encode_cwd_cdq(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 2)?;
    let high = architectural_register(opcode, operands, 0)?;
    let low = architectural_register(opcode, operands, 1)?;
    match (high, low) {
        (X86Register::Dx, X86Register::Ax) => Ok(vec![0x99]),
        (X86Register::Edx, X86Register::Eax) => Ok(vec![0x66, 0x99]),
        _ => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "cwd/cdq requires matching AX/DX or EAX/EDX operands",
        }),
    }
}

fn encode_divide(opcode: X86Opcode, operands: &[MCOperand]) -> Result<Vec<u8>, EncodeError> {
    expect_arity(opcode, operands, 5)?;
    let high = register_operand(opcode, operands, 0)?;
    let low = register_operand(opcode, operands, 1)?;
    let divisor = register_operand(opcode, operands, 2)?;
    let quotient = register_operand(opcode, operands, 3)?;
    let remainder = register_operand(opcode, operands, 4)?;
    let (expected_high, expected_low) = match divisor.size {
        OperandSize::Word => (X86Register::Dx, X86Register::Ax),
        OperandSize::Dword => (X86Register::Edx, X86Register::Eax),
        OperandSize::Byte => {
            return Err(EncodeError::UnsupportedForm {
                opcode,
                reason: "div/idiv has no supported byte form",
            });
        }
    };
    for (index, register) in [
        (0, expected_high),
        (1, expected_low),
        (3, expected_low),
        (4, expected_high),
    ] {
        exact_register(opcode, operands, index, register)?;
    }
    for operand in [high, low, quotient, remainder] {
        require_matching_width(opcode, divisor, operand)?;
    }
    let mut bytes = prefix_for(divisor.size);
    bytes.push(0xf7);
    bytes.push(modrm(
        if opcode == X86Opcode::Div { 6 } else { 7 },
        divisor.code,
    ));
    Ok(bytes)
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

fn encode_push(
    opcode: X86Opcode,
    operands: &[MCOperand],
) -> Result<EncodedInstruction, EncodeError> {
    match operands {
        [MCOperand::Register(_)] => Ok(EncodedInstruction {
            bytes: encode_push_pop(opcode, operands, 0x50)?,
            fixups: Vec::new(),
        }),
        [MCOperand::Immediate(bits), MCOperand::Immediate(value)] => {
            let size = push_size(opcode, *bits)?;
            let mut bytes = prefix_for(size);
            if i64::from(*value as i8) == *value {
                bytes.extend([0x6a, *value as u8]);
            } else {
                bytes.push(0x68);
                bytes.extend(encode_immediate(*value, size)?);
            }
            Ok(EncodedInstruction {
                bytes,
                fixups: Vec::new(),
            })
        }
        [MCOperand::Immediate(bits), ..] => {
            let size = push_size(opcode, *bits)?;
            let mut encoded = encode_memory(opcode, operands, 1, 0xff, 6)?;
            let prefix = prefix_for(size);
            if !prefix.is_empty() {
                encoded.bytes.splice(0..0, prefix.iter().copied());
                let adjustment = u32::try_from(prefix.len()).expect("x86 prefix count fits u32");
                for fixup in &mut encoded.fixups {
                    fixup.offset += adjustment;
                }
            }
            Ok(encoded)
        }
        _ => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "push requires a register or an explicit 16/32-bit immediate or memory source",
        }),
    }
}

fn push_size(opcode: X86Opcode, bits: i64) -> Result<OperandSize, EncodeError> {
    match bits {
        16 => Ok(OperandSize::Word),
        32 => Ok(OperandSize::Dword),
        _ => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "push source width must be 16 or 32 bits",
        }),
    }
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

fn exact_register(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
    expected: X86Register,
) -> Result<RegisterEncoding, EncodeError> {
    let register = architectural_register(opcode, operands, index)?;
    if register != expected {
        return Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "implicit division register does not match its architectural role",
        });
    }
    match operands.get(index) {
        Some(MCOperand::Register(register)) => register_encoding(register.get()),
        _ => unreachable!("architectural_register checked the operand kind"),
    }
}

fn architectural_register(
    opcode: X86Opcode,
    operands: &[MCOperand],
    index: usize,
) -> Result<X86Register, EncodeError> {
    let Some(MCOperand::Register(register)) = operands.get(index) else {
        return Err(EncodeError::OperandKind {
            opcode,
            index,
            expected: "a register",
        });
    };
    decode_register(register.get())
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

struct Address16Encoding {
    mode: u8,
    rm: u8,
    displacement: Vec<u8>,
}

impl Address16Encoding {
    fn with_register(self, register: u8) -> Vec<u8> {
        let mut bytes = vec![(self.mode << 6) | (register << 3) | self.rm];
        bytes.extend(self.displacement);
        bytes
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
    use crate::old::codegen::machine::{
        InstructionFlags, MachineBlock, MachineBlockId, MachineCallingConvention, MachineFunction,
        MachineFunctionId, MachineInstruction, MachineInstructionId, MachineLinkage, MachineModule,
        MachineOperand, MachineOperandKind, MachineSignature, OperandRole,
    };
    use crate::old::mc::{MCExpression, MCOperand, PhysicalRegister, SymbolId, TargetOpcode};
    use crate::old::target::x86::lower_allocated_module;

    fn instruction(opcode: X86Opcode, operands: Vec<MCOperand>) -> MCInstruction {
        MCInstruction {
            opcode: TargetOpcode::new(opcode as u32),
            operands,
        }
    }

    fn register(register: X86Register) -> MCOperand {
        MCOperand::Register(PhysicalRegister::new(register as u32))
    }

    fn x87_memory(format: X87MemoryFormat, address: X86Register) -> Vec<MCOperand> {
        vec![
            register(X86Register::St0),
            MCOperand::Immediate(i64::from(format.raw())),
            register(address),
        ]
    }

    #[test]
    fn encodes_x87_memory_load_store_and_integer_formats() {
        for (opcode, format, bytes) in [
            (
                X86Opcode::X87Load,
                X87MemoryFormat::Float32,
                vec![0xd9, 0x07],
            ),
            (
                X86Opcode::X87Load,
                X87MemoryFormat::Float64,
                vec![0xdd, 0x07],
            ),
            (
                X86Opcode::X87Load,
                X87MemoryFormat::Float80,
                vec![0xdb, 0x2f],
            ),
            (
                X86Opcode::X87Store,
                X87MemoryFormat::Float32,
                vec![0xd9, 0x17],
            ),
            (
                X86Opcode::X87Store,
                X87MemoryFormat::Float64,
                vec![0xdd, 0x17],
            ),
            (
                X86Opcode::X87StorePop,
                X87MemoryFormat::Float32,
                vec![0xd9, 0x1f],
            ),
            (
                X86Opcode::X87StorePop,
                X87MemoryFormat::Float64,
                vec![0xdd, 0x1f],
            ),
            (
                X86Opcode::X87StorePop,
                X87MemoryFormat::Float80,
                vec![0xdb, 0x3f],
            ),
            (
                X86Opcode::X87IntegerLoad,
                X87MemoryFormat::Signed16,
                vec![0xdf, 0x07],
            ),
            (
                X86Opcode::X87IntegerLoad,
                X87MemoryFormat::Signed32,
                vec![0xdb, 0x07],
            ),
            (
                X86Opcode::X87IntegerLoad,
                X87MemoryFormat::Signed64,
                vec![0xdf, 0x2f],
            ),
            (
                X86Opcode::X87IntegerStore,
                X87MemoryFormat::Signed16,
                vec![0xdf, 0x17],
            ),
            (
                X86Opcode::X87IntegerStore,
                X87MemoryFormat::Signed32,
                vec![0xdb, 0x17],
            ),
            (
                X86Opcode::X87IntegerStorePop,
                X87MemoryFormat::Signed16,
                vec![0xdf, 0x1f],
            ),
            (
                X86Opcode::X87IntegerStorePop,
                X87MemoryFormat::Signed32,
                vec![0xdb, 0x1f],
            ),
            (
                X86Opcode::X87IntegerStorePop,
                X87MemoryFormat::Signed64,
                vec![0xdf, 0x3f],
            ),
        ] {
            assert_eq!(
                encode(&instruction(opcode, x87_memory(format, X86Register::Bx))).unwrap(),
                bytes
            );
        }
    }

    #[test]
    fn encodes_x87_memory_arithmetic_comparisons_and_bp_zero() {
        for (opcode, bytes) in [
            (X86Opcode::X87Add, vec![0xd8, 0x07]),
            (X86Opcode::X87Subtract, vec![0xd8, 0x27]),
            (X86Opcode::X87SubtractReverse, vec![0xd8, 0x2f]),
            (X86Opcode::X87Multiply, vec![0xd8, 0x0f]),
            (X86Opcode::X87Divide, vec![0xd8, 0x37]),
            (X86Opcode::X87DivideReverse, vec![0xd8, 0x3f]),
            (X86Opcode::X87Compare, vec![0xd8, 0x17]),
            (X86Opcode::X87ComparePop, vec![0xd8, 0x1f]),
        ] {
            assert_eq!(
                encode(&instruction(
                    opcode,
                    x87_memory(X87MemoryFormat::Float32, X86Register::Bx)
                ))
                .unwrap(),
                bytes
            );
        }
        assert_eq!(
            encode(&instruction(
                X86Opcode::X87Add,
                x87_memory(X87MemoryFormat::Float64, X86Register::Bx),
            ))
            .unwrap(),
            vec![0xdc, 0x07]
        );
        for (opcode, signed16, signed32) in [
            (X86Opcode::X87Add, vec![0xde, 0x07], vec![0xda, 0x07]),
            (X86Opcode::X87Subtract, vec![0xde, 0x27], vec![0xda, 0x27]),
            (
                X86Opcode::X87SubtractReverse,
                vec![0xde, 0x2f],
                vec![0xda, 0x2f],
            ),
            (X86Opcode::X87Multiply, vec![0xde, 0x0f], vec![0xda, 0x0f]),
            (X86Opcode::X87Divide, vec![0xde, 0x37], vec![0xda, 0x37]),
            (
                X86Opcode::X87DivideReverse,
                vec![0xde, 0x3f],
                vec![0xda, 0x3f],
            ),
        ] {
            assert_eq!(
                encode(&instruction(
                    opcode,
                    x87_memory(X87MemoryFormat::Signed16, X86Register::Bx),
                ))
                .unwrap(),
                signed16
            );
            assert_eq!(
                encode(&instruction(
                    opcode,
                    x87_memory(X87MemoryFormat::Signed32, X86Register::Bx),
                ))
                .unwrap(),
                signed32
            );
        }
        assert_eq!(
            encode(&instruction(
                X86Opcode::X87Load,
                vec![
                    register(X86Register::St0),
                    MCOperand::Immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    register(X86Register::Bp),
                    MCOperand::Immediate(0),
                ],
            ))
            .unwrap(),
            vec![0xd9, 0x46, 0x00]
        );
    }

    #[test]
    fn encodes_x87_stack_directions_pop_and_status_forms() {
        for (opcode, operands, bytes) in [
            (
                X86Opcode::X87Add,
                vec![register(X86Register::St0), register(X86Register::St3)],
                vec![0xd8, 0xc3],
            ),
            (
                X86Opcode::X87Add,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdc, 0xc3],
            ),
            (
                X86Opcode::X87Subtract,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdc, 0xeb],
            ),
            (
                X86Opcode::X87SubtractReverse,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdc, 0xe3],
            ),
            (
                X86Opcode::X87Multiply,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdc, 0xcb],
            ),
            (
                X86Opcode::X87Divide,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdc, 0xfb],
            ),
            (
                X86Opcode::X87DivideReverse,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdc, 0xf3],
            ),
            (
                X86Opcode::X87AddPop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xde, 0xc3],
            ),
            (
                X86Opcode::X87SubtractPop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xde, 0xeb],
            ),
            (
                X86Opcode::X87SubtractReversePop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xde, 0xe3],
            ),
            (
                X86Opcode::X87MultiplyPop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xde, 0xcb],
            ),
            (
                X86Opcode::X87DividePop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xde, 0xfb],
            ),
            (
                X86Opcode::X87DivideReversePop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xde, 0xf3],
            ),
            (
                X86Opcode::X87Compare,
                vec![register(X86Register::St0), register(X86Register::St3)],
                vec![0xd8, 0xd3],
            ),
            (
                X86Opcode::X87ComparePop,
                vec![register(X86Register::St0), register(X86Register::St3)],
                vec![0xd8, 0xdb],
            ),
            (
                X86Opcode::X87ComparePop2,
                vec![register(X86Register::St0), register(X86Register::St1)],
                vec![0xde, 0xd9],
            ),
            (
                X86Opcode::X87StackLoad,
                vec![register(X86Register::St0), register(X86Register::St3)],
                vec![0xd9, 0xc3],
            ),
            (
                X86Opcode::X87StackStorePop,
                vec![register(X86Register::St3), register(X86Register::St0)],
                vec![0xdd, 0xdb],
            ),
            (
                X86Opcode::X87Exchange,
                vec![register(X86Register::St0), register(X86Register::St3)],
                vec![0xd9, 0xcb],
            ),
        ] {
            assert_eq!(encode(&instruction(opcode, operands)).unwrap(), bytes);
        }
        for (opcode, bytes) in [
            (X86Opcode::X87LoadZero, vec![0xd9, 0xee]),
            (X86Opcode::X87LoadOne, vec![0xd9, 0xe8]),
            (X86Opcode::X87ChangeSign, vec![0xd9, 0xe0]),
            (X86Opcode::X87Absolute, vec![0xd9, 0xe1]),
            (X86Opcode::X87SquareRoot, vec![0xd9, 0xfa]),
        ] {
            assert_eq!(
                encode(&instruction(opcode, vec![register(X86Register::St0)])).unwrap(),
                bytes
            );
        }
        assert_eq!(
            encode(&instruction(
                X86Opcode::X87StoreStatusWord,
                vec![register(X86Register::Ax)]
            ))
            .unwrap(),
            vec![0xdf, 0xe0]
        );
        assert_eq!(
            encode(&instruction(X86Opcode::Wait, vec![])).unwrap(),
            vec![0x9b]
        );
        assert_eq!(
            encode(&instruction(X86Opcode::Sahf, vec![])).unwrap(),
            vec![0x9e]
        );
    }

    #[test]
    fn encodes_x87_control_words_and_symbolic_memory_fixups() {
        for (opcode, bytes) in [
            (X86Opcode::X87StoreControlWord, vec![0xd9, 0x7e, 0x00]),
            (X86Opcode::X87LoadControlWord, vec![0xd9, 0x6e, 0x00]),
        ] {
            assert_eq!(
                encode(&instruction(
                    opcode,
                    vec![
                        MCOperand::Immediate(i64::from(X87MemoryFormat::Control16.raw())),
                        register(X86Register::Bp),
                        MCOperand::Immediate(0),
                    ],
                ))
                .unwrap(),
                bytes
            );
        }
        let expression = MCExpression {
            symbol: SymbolId::new(9),
            addend: 4,
        };
        let encoded = encode_with_fixups(&instruction(
            X86Opcode::X87Load,
            vec![
                register(X86Register::St0),
                MCOperand::Immediate(i64::from(X87MemoryFormat::Float32.raw())),
                MCOperand::Expression(expression),
            ],
        ))
        .unwrap();
        assert_eq!(encoded.bytes, vec![0xd9, 0x06, 0, 0]);
        assert_eq!(
            encoded.fixups,
            vec![Fixup {
                offset: 2,
                kind: X86FixupKind::NearData16.into(),
                expression,
                pc_relative: false
            }]
        );
    }

    #[test]
    fn refuses_invalid_x87_formats_operands_and_truncating_pseudo() {
        assert!(matches!(
            encode(&instruction(
                X86Opcode::X87Add,
                x87_memory(X87MemoryFormat::Float80, X86Register::Bx)
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::X87Add,
                ..
            })
        ));
        assert!(matches!(
            encode(&instruction(
                X86Opcode::X87Add,
                x87_memory(X87MemoryFormat::Signed64, X86Register::Bx)
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::X87Add,
                ..
            })
        ));
        assert!(matches!(
            encode(&instruction(
                X86Opcode::X87Compare,
                x87_memory(X87MemoryFormat::Signed16, X86Register::Bx)
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::X87Compare,
                ..
            })
        ));
        assert!(matches!(
            encode(&instruction(
                X86Opcode::X87IntegerStore,
                x87_memory(X87MemoryFormat::Signed64, X86Register::Bx)
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::X87IntegerStore,
                ..
            })
        ));
        assert!(matches!(
            encode(&instruction(
                X86Opcode::X87IntegerStoreTrunc,
                x87_memory(X87MemoryFormat::Signed32, X86Register::Bx)
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::X87IntegerStoreTrunc,
                ..
            })
        ));
        assert!(matches!(
            encode(&instruction(
                X86Opcode::X87StackLoad,
                vec![register(X86Register::St2), register(X86Register::St3)]
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::X87StackLoad,
                ..
            })
        ));
    }

    #[test]
    fn encodes_explicit_immediate_and_memory_push_forms() {
        for (operands, bytes) in [
            (
                vec![MCOperand::Immediate(16), MCOperand::Immediate(-128)],
                vec![0x6a, 0x80],
            ),
            (
                vec![MCOperand::Immediate(16), MCOperand::Immediate(128)],
                vec![0x68, 0x80, 0x00],
            ),
            (
                vec![MCOperand::Immediate(32), MCOperand::Immediate(-128)],
                vec![0x66, 0x6a, 0x80],
            ),
            (
                vec![MCOperand::Immediate(32), MCOperand::Immediate(0x1234_5678)],
                vec![0x66, 0x68, 0x78, 0x56, 0x34, 0x12],
            ),
            (
                vec![MCOperand::Immediate(16), register(X86Register::Bx)],
                vec![0xff, 0x37],
            ),
            (
                vec![
                    MCOperand::Immediate(32),
                    register(X86Register::Bp),
                    MCOperand::Immediate(-4),
                ],
                vec![0x66, 0xff, 0x76, 0xfc],
            ),
        ] {
            assert_eq!(
                encode(&instruction(X86Opcode::Push, operands)).unwrap(),
                bytes
            );
        }

        let expression = MCExpression {
            symbol: SymbolId::new(11),
            addend: -6,
        };
        let encoded = encode_with_fixups(&instruction(
            X86Opcode::Push,
            vec![MCOperand::Immediate(32), MCOperand::Expression(expression)],
        ))
        .unwrap();
        assert_eq!(encoded.bytes, vec![0x66, 0xff, 0x36, 0, 0]);
        assert_eq!(
            encoded.fixups,
            vec![Fixup {
                offset: 3,
                kind: X86FixupKind::NearData16.into(),
                expression,
                pc_relative: false,
            }]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Push,
                vec![MCOperand::Immediate(32), MCOperand::Expression(expression)],
            )),
            Err(EncodeError::FixupsRequired {
                opcode: X86Opcode::Push,
                count: 1,
            })
        );
    }

    #[test]
    fn encodes_explicit_immediate_store_widths_frames_and_symbolic_addresses() {
        for (operands, bytes) in [
            (
                vec![
                    register(X86Register::Bx),
                    MCOperand::Immediate(8),
                    MCOperand::Immediate(0x7f),
                ],
                vec![0xc6, 0x07, 0x7f],
            ),
            (
                vec![
                    register(X86Register::Bx),
                    MCOperand::Immediate(16),
                    MCOperand::Immediate(0x1234),
                ],
                vec![0xc7, 0x07, 0x34, 0x12],
            ),
            (
                vec![
                    register(X86Register::Bx),
                    MCOperand::Immediate(32),
                    MCOperand::Immediate(0x4040_0000),
                ],
                vec![0x66, 0xc7, 0x07, 0x00, 0x00, 0x40, 0x40],
            ),
            (
                vec![
                    register(X86Register::Bp),
                    MCOperand::Immediate(-4),
                    MCOperand::Immediate(8),
                    MCOperand::Immediate(1),
                ],
                vec![0xc6, 0x46, 0xfc, 0x01],
            ),
            (
                vec![
                    register(X86Register::Bp),
                    MCOperand::Immediate(-4),
                    MCOperand::Immediate(16),
                    MCOperand::Immediate(0x1234),
                ],
                vec![0xc7, 0x46, 0xfc, 0x34, 0x12],
            ),
            (
                vec![
                    register(X86Register::Bp),
                    MCOperand::Immediate(-4),
                    MCOperand::Immediate(32),
                    MCOperand::Immediate(0x4040_0000),
                ],
                vec![0x66, 0xc7, 0x46, 0xfc, 0x00, 0x00, 0x40, 0x40],
            ),
            // Python's CHAIN regression: the raw dword is a bit pattern,
            // even when it lies above signed i32::MAX.
            (
                vec![
                    register(X86Register::Bp),
                    MCOperand::Immediate(-4),
                    MCOperand::Immediate(32),
                    MCOperand::Immediate(0xc174_7c23),
                ],
                vec![0x66, 0xc7, 0x46, 0xfc, 0x23, 0x7c, 0x74, 0xc1],
            ),
        ] {
            assert_eq!(
                encode(&instruction(X86Opcode::Store, operands)).unwrap(),
                bytes
            );
        }

        let expression = MCExpression {
            symbol: SymbolId::new(12),
            addend: 4,
        };
        let encoded = encode_with_fixups(&instruction(
            X86Opcode::Store,
            vec![
                MCOperand::Expression(expression),
                MCOperand::Immediate(32),
                MCOperand::Immediate(0x4040_0000),
            ],
        ))
        .unwrap();
        assert_eq!(
            encoded.bytes,
            vec![0x66, 0xc7, 0x06, 0, 0, 0, 0, 0x40, 0x40]
        );
        assert_eq!(
            encoded.fixups,
            vec![Fixup {
                offset: 3,
                kind: X86FixupKind::NearData16.into(),
                expression,
                pc_relative: false,
            }]
        );
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
    fn encodes_exact_word_and_dword_division_family() {
        for (opcode, high, low, divisor, bytes) in [
            (
                X86Opcode::Div,
                X86Register::Dx,
                X86Register::Ax,
                X86Register::Cx,
                vec![0xf7, 0xf1],
            ),
            (
                X86Opcode::Idiv,
                X86Register::Dx,
                X86Register::Ax,
                X86Register::Cx,
                vec![0xf7, 0xf9],
            ),
            (
                X86Opcode::Div,
                X86Register::Edx,
                X86Register::Eax,
                X86Register::Ecx,
                vec![0x66, 0xf7, 0xf1],
            ),
            (
                X86Opcode::Idiv,
                X86Register::Edx,
                X86Register::Eax,
                X86Register::Ecx,
                vec![0x66, 0xf7, 0xf9],
            ),
        ] {
            assert_eq!(
                encode(&instruction(
                    opcode,
                    vec![
                        register(high),
                        register(low),
                        register(divisor),
                        register(low),
                        register(high),
                    ],
                ))
                .unwrap(),
                bytes
            );
        }
        assert_eq!(
            encode(&instruction(
                X86Opcode::CwdCdq,
                vec![register(X86Register::Dx), register(X86Register::Ax)],
            ))
            .unwrap(),
            vec![0x99]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::CwdCdq,
                vec![register(X86Register::Edx), register(X86Register::Eax)],
            ))
            .unwrap(),
            vec![0x66, 0x99]
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
    fn encodes_zero_extend_word_to_dword_movzx_and_refuses_other_widths() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::ZeroExtendWordToDword,
                vec![register(X86Register::Eax), register(X86Register::Cx)],
            ))
            .unwrap(),
            vec![0x66, 0x0f, 0xb7, 0xc1]
        );
        assert!(matches!(
            encode(&instruction(
                X86Opcode::ZeroExtendWordToDword,
                vec![register(X86Register::Ax), register(X86Register::Cx)],
            )),
            Err(EncodeError::UnsupportedForm {
                opcode: X86Opcode::ZeroExtendWordToDword,
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
    fn encodes_python_truncation_control_word_mask() {
        assert_eq!(
            encode(&instruction(
                X86Opcode::Or,
                vec![register(X86Register::Ax), MCOperand::Immediate(0x0c00)],
            ))
            .unwrap(),
            vec![0x81, 0xc8, 0x00, 0x0c]
        );
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
    fn encodes_general_address16_displacements() {
        // `addressforms.selected` leaves a folded base plus signed offset at
        // its memory consumer.  The zero spelling is canonical except that
        // BP needs an explicit zero displacement in 16-bit addressing.
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Bx),
                    MCOperand::Immediate(0)
                ],
            ))
            .unwrap(),
            vec![0x8b, 0x07],
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Bx),
                    MCOperand::Immediate(4)
                ],
            ))
            .unwrap(),
            vec![0x8b, 0x47, 0x04],
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Bp),
                    MCOperand::Immediate(0)
                ],
            ))
            .unwrap(),
            vec![0x8b, 0x46, 0x00],
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::Load,
                vec![
                    register(X86Register::Ax),
                    register(X86Register::Si),
                    MCOperand::Immediate(128)
                ],
            ))
            .unwrap(),
            vec![0x8b, 0x84, 0x80, 0x00],
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
        assert_eq!(
            encode(&instruction(
                X86Opcode::X87Load,
                vec![
                    register(X86Register::St0),
                    MCOperand::Immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    register(X86Register::Bx),
                    register(X86Register::Es),
                ],
            ))
            .unwrap(),
            vec![0x26, 0xd9, 0x07]
        );
        assert_eq!(
            encode(&instruction(
                X86Opcode::X87StorePop,
                vec![
                    register(X86Register::St0),
                    MCOperand::Immediate(i64::from(X87MemoryFormat::Float32.raw())),
                    register(X86Register::Bx),
                    register(X86Register::Es),
                ],
            ))
            .unwrap(),
            vec![0x26, 0xd9, 0x1f]
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
    fn bp_zero_uses_a_displacement_and_bx_accepts_one() {
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
            ))
            .unwrap(),
            vec![0x8b, 0x47, 0x06]
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
    fn encodes_a_relocatable_near_address_with_one_near_data_fixup() {
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
                kind: X86FixupKind::NearData16.into(),
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
    fn encodes_direct_symbolic_loads_and_stores() {
        let expression = MCExpression {
            symbol: SymbolId::new(5),
            addend: 4,
        };
        let load = encode_with_fixups(&instruction(
            X86Opcode::Load,
            vec![
                register(X86Register::Eax),
                MCOperand::Expression(expression),
            ],
        ))
        .unwrap();
        let store = encode_with_fixups(&instruction(
            X86Opcode::Store,
            vec![
                MCOperand::Expression(expression),
                register(X86Register::Eax),
            ],
        ))
        .unwrap();

        assert_eq!(load.bytes, vec![0x66, 0x8b, 0x06, 0, 0]);
        assert_eq!(store.bytes, vec![0x66, 0x89, 0x06, 0, 0]);
        for encoded in [load, store] {
            assert_eq!(
                encoded.fixups,
                vec![Fixup {
                    offset: 3,
                    kind: X86FixupKind::NearData16.into(),
                    expression,
                    pc_relative: false,
                }]
            );
        }
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
            crate::old::mc::MCFragment::Instruction(fragment) => &fragment.instruction,
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
