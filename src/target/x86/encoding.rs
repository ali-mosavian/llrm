//! Exact initial i386 register-form instruction encoding.
//!
//! The initial target has a 16-bit default operand size.  This module encodes
//! only physical general-purpose register forms; memory, expressions, fixups,
//! branches, calls, segments, and x87 remain explicit unsupported forms until
//! their semantics and relocation contracts are implemented.

use std::error::Error;
use std::fmt;

use crate::mc::{MCInstruction, MCOperand};

use super::{OperandSize, X86Opcode, X86Register};

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
pub fn encode(instruction: &MCInstruction) -> Result<Vec<u8>, EncodeError> {
    let opcode = decode_opcode(instruction.opcode.get())?;
    match opcode {
        X86Opcode::Copy | X86Opcode::PhiCopy | X86Opcode::Mov => {
            encode_move(opcode, &instruction.operands)
        }
        X86Opcode::Add => encode_register_operands(opcode, &instruction.operands, 0x00, 0x01),
        X86Opcode::Sub => encode_register_operands(opcode, &instruction.operands, 0x28, 0x29),
        X86Opcode::And => encode_register_operands(opcode, &instruction.operands, 0x20, 0x21),
        X86Opcode::Or => encode_register_operands(opcode, &instruction.operands, 0x08, 0x09),
        X86Opcode::Xor => encode_register_operands(opcode, &instruction.operands, 0x30, 0x31),
        X86Opcode::Cmp => encode_register_operands(opcode, &instruction.operands, 0x38, 0x39),
        X86Opcode::Test => encode_register_operands(opcode, &instruction.operands, 0x84, 0x85),
        X86Opcode::Imul => encode_imul(opcode, &instruction.operands),
        X86Opcode::Neg => encode_unary(opcode, &instruction.operands, 3),
        X86Opcode::Not => encode_unary(opcode, &instruction.operands, 2),
        X86Opcode::Push => encode_push_pop(opcode, &instruction.operands, 0x50),
        X86Opcode::Pop => encode_push_pop(opcode, &instruction.operands, 0x58),
        X86Opcode::ReturnNear => encode_return(opcode, &instruction.operands, 0xc3),
        X86Opcode::ReturnFar => encode_return(opcode, &instruction.operands, 0xcb),
        X86Opcode::Lea
        | X86Opcode::Idiv
        | X86Opcode::ShiftLeft
        | X86Opcode::ShiftRightLogical
        | X86Opcode::ShiftRightArithmetic
        | X86Opcode::CallNear
        | X86Opcode::CallFar
        | X86Opcode::Jump
        | X86Opcode::JumpConditional
        | X86Opcode::Load
        | X86Opcode::Store
        | X86Opcode::MergeWords
        | X86Opcode::LowWord
        | X86Opcode::HighWord => Err(EncodeError::UnsupportedForm {
            opcode,
            reason: "this initial encoder accepts only exact register forms",
        }),
    }
}

/// Returns the exact encoded size for one instruction.
pub fn encoded_size(instruction: &MCInstruction) -> Result<u64, EncodeError> {
    Ok(encode(instruction)?.len() as u64)
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
    use crate::mc::{MCExpression, MCOperand, PhysicalRegister, SymbolId, TargetOpcode};

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
}
