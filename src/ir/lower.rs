//! Lowering from resolved HIR into portable SSA IR.
//!
//! This is deliberately a scalar-only boundary.  It does not invent memory
//! operations, call semantics, or ABI details that portable IR cannot yet
//! represent exactly.

use std::error::Error;
use std::fmt;

use crate::support::diagnostic::Diagnostic;
use crate::{hir, ir};

/// A HIR feature that the portable scalar lowering cannot represent exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedFeature {
    ArrayType,
    OpaqueType,
    DataObject,
    Callable,
    Place,
    CallAbi,
    ErrorHandler,
    ExternalEntry,
    Memory,
    PointerExtraction,
    Concatenation,
    DivideRemainder,
    StringOperation,
    Call,
    UnsupportedCast,
}

/// A malformed scalar property that cannot be lowered without guessing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidProperty {
    ZeroWidth,
    WidthOverflow,
    MissingFloatEvaluation,
    EntryBlockIsNotFirst,
    ResultArity,
    OperandArity,
    OperandTypes,
    CopyTypes,
    CastTypes,
    ReturnType,
    BranchCondition,
    SwitchSelector,
    ConstantType,
}

/// The kind of an unsupported HIR operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedOperand {
    Place,
    Element,
    Projection,
    Indirect,
}

/// A structured failure from [`lower_module`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LowerError {
    UnsupportedType {
        type_id: hir::TypeId,
        feature: UnsupportedFeature,
    },
    InvalidType {
        type_id: hir::TypeId,
        property: InvalidProperty,
    },
    UnknownType {
        type_id: hir::TypeId,
    },
    UnsupportedModule {
        feature: UnsupportedFeature,
    },
    UnsupportedFunction {
        function: hir::FunctionId,
        feature: UnsupportedFeature,
    },
    InvalidFunction {
        function: hir::FunctionId,
        property: InvalidProperty,
    },
    UnsupportedInstruction {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: hir::InstructionId,
        opcode: hir::Opcode,
        feature: UnsupportedFeature,
    },
    InvalidInstruction {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: hir::InstructionId,
        property: InvalidProperty,
    },
    UnsupportedOperand {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        index: usize,
        operand: UnsupportedOperand,
    },
    UnknownValue {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        value: hir::ValueId,
    },
    InvalidBlock {
        function: hir::FunctionId,
        block: hir::BlockId,
        property: InvalidProperty,
    },
    InvalidOperand {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        index: usize,
        property: InvalidProperty,
    },
    Verification {
        diagnostics: Vec<Diagnostic>,
    },
}

impl fmt::Display for LowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType { type_id, feature } => {
                write!(formatter, "cannot lower type {type_id}: {feature:?}")
            }
            Self::InvalidType { type_id, property } => {
                write!(formatter, "invalid type {type_id}: {property:?}")
            }
            Self::UnknownType { type_id } => write!(formatter, "unknown type {type_id}"),
            Self::UnsupportedModule { feature } => {
                write!(formatter, "cannot lower module: {feature:?}")
            }
            Self::UnsupportedFunction { function, feature } => {
                write!(formatter, "cannot lower function {function}: {feature:?}")
            }
            Self::InvalidFunction { function, property } => {
                write!(formatter, "invalid function {function}: {property:?}")
            }
            Self::UnsupportedInstruction {
                function,
                block,
                instruction,
                feature,
                ..
            } => write!(
                formatter,
                "cannot lower function {function} block {block} instruction {instruction}: {feature:?}"
            ),
            Self::InvalidInstruction {
                function,
                block,
                instruction,
                property,
            } => write!(
                formatter,
                "invalid function {function} block {block} instruction {instruction}: {property:?}"
            ),
            Self::UnsupportedOperand {
                function,
                block,
                instruction,
                index,
                operand,
            } => write!(
                formatter,
                "cannot lower function {function} block {block} {instruction:?} operand {index}: {operand:?}"
            ),
            Self::UnknownValue {
                function,
                block,
                instruction,
                value,
            } => write!(
                formatter,
                "function {function} block {block} {instruction:?} references unknown value {value}"
            ),
            Self::InvalidBlock {
                function,
                block,
                property,
            } => write!(
                formatter,
                "invalid function {function} block {block}: {property:?}"
            ),
            Self::InvalidOperand {
                function,
                block,
                instruction,
                index,
                property,
            } => write!(
                formatter,
                "invalid function {function} block {block} {instruction:?} operand {index}: {property:?}"
            ),
            Self::Verification { diagnostics } => write!(
                formatter,
                "lowered portable IR failed verification with {} diagnostic(s)",
                diagnostics.len()
            ),
        }
    }
}

impl Error for LowerError {}

/// Lowers the representable scalar subset of one HIR module into portable IR.
///
/// IDs and declaration order are retained in their corresponding IR domains.
/// Features without an exact portable representation return [`LowerError`]
/// rather than being erased or approximated.
pub fn lower_module(module: &hir::Module) -> Result<ir::Module, LowerError> {
    let lowerer = Lowerer { module };
    let types = module
        .types
        .iter()
        .map(|type_| lowerer.lower_type(type_))
        .collect::<Result<Vec<_>, _>>()?;

    if !module.data.is_empty() {
        return Err(LowerError::UnsupportedModule {
            feature: UnsupportedFeature::DataObject,
        });
    }
    if !module.callables.is_empty() {
        return Err(LowerError::UnsupportedModule {
            feature: UnsupportedFeature::Callable,
        });
    }

    let functions = module
        .functions
        .iter()
        .map(|function| lowerer.lower_function(function))
        .collect::<Result<Vec<_>, _>>()?;
    let lowered = ir::Module {
        name: module.name.clone(),
        types,
        globals: Vec::new(),
        functions,
    };
    lowered
        .verify()
        .map_err(|diagnostics| LowerError::Verification { diagnostics })?;
    Ok(lowered)
}

struct Lowerer<'module> {
    module: &'module hir::Module,
}

impl<'module> Lowerer<'module> {
    fn lower_type(&self, type_: &hir::Type) -> Result<ir::Type, LowerError> {
        let kind = match type_.kind {
            hir::TypeKind::Void => ir::TypeKind::Void,
            hir::TypeKind::Boolean => ir::TypeKind::Integer { bits: 1 },
            hir::TypeKind::Integer => ir::TypeKind::Integer {
                bits: self.integer_bits(type_)?,
            },
            hir::TypeKind::Float => ir::TypeKind::Float(self.float_kind(type_)?),
            hir::TypeKind::Pointer => ir::TypeKind::Pointer {
                address_space: match type_.address {
                    hir::AddressKind::None => ir::AddressSpace::Generic,
                    hir::AddressKind::Near => ir::AddressSpace::NearData,
                    hir::AddressKind::Far => ir::AddressSpace::FarData,
                    hir::AddressKind::Huge => ir::AddressSpace::HugeData,
                    hir::AddressKind::Code => ir::AddressSpace::Code,
                    hir::AddressKind::Segment => ir::AddressSpace::Segment,
                },
            },
            hir::TypeKind::Array => {
                return Err(LowerError::UnsupportedType {
                    type_id: type_.id,
                    feature: UnsupportedFeature::ArrayType,
                });
            }
            hir::TypeKind::Opaque => {
                return Err(LowerError::UnsupportedType {
                    type_id: type_.id,
                    feature: UnsupportedFeature::OpaqueType,
                });
            }
        };
        Ok(ir::Type {
            id: type_id(type_.id),
            kind,
        })
    }

    fn integer_bits(&self, type_: &hir::Type) -> Result<u16, LowerError> {
        if matches!(type_.kind, hir::TypeKind::Boolean) {
            return Ok(1);
        }
        let bits = type_.width.checked_mul(8).ok_or(LowerError::InvalidType {
            type_id: type_.id,
            property: InvalidProperty::WidthOverflow,
        })?;
        let bits = u16::try_from(bits).map_err(|_| LowerError::InvalidType {
            type_id: type_.id,
            property: InvalidProperty::WidthOverflow,
        })?;
        if bits == 0 {
            return Err(LowerError::InvalidType {
                type_id: type_.id,
                property: InvalidProperty::ZeroWidth,
            });
        }
        Ok(bits)
    }

    fn float_kind(&self, type_: &hir::Type) -> Result<ir::FloatKind, LowerError> {
        match type_.evaluation {
            hir::FloatEvaluation::Binary32 => Ok(ir::FloatKind::Binary32),
            hir::FloatEvaluation::Binary64 => Ok(ir::FloatKind::Binary64),
            hir::FloatEvaluation::Extended80 => Ok(ir::FloatKind::Extended80),
            hir::FloatEvaluation::None => Err(LowerError::InvalidType {
                type_id: type_.id,
                property: InvalidProperty::MissingFloatEvaluation,
            }),
        }
    }

    fn lower_function(&self, function: &hir::Function) -> Result<ir::Function, LowerError> {
        if !function.places.is_empty() {
            return Err(LowerError::UnsupportedFunction {
                function: function.id,
                feature: UnsupportedFeature::Place,
            });
        }
        if !function.calls.is_empty() {
            return Err(LowerError::UnsupportedFunction {
                function: function.id,
                feature: UnsupportedFeature::CallAbi,
            });
        }
        if function.error_handler.is_some() || function.error_handler_local {
            return Err(LowerError::UnsupportedFunction {
                function: function.id,
                feature: UnsupportedFeature::ErrorHandler,
            });
        }
        if !function.external_entries.is_empty() {
            return Err(LowerError::UnsupportedFunction {
                function: function.id,
                feature: UnsupportedFeature::ExternalEntry,
            });
        }
        if function.blocks.first().map(|block| block.id) != Some(function.entry) {
            return Err(LowerError::InvalidFunction {
                function: function.id,
                property: InvalidProperty::EntryBlockIsNotFirst,
            });
        }

        let parameters = function
            .parameters
            .iter()
            .map(|id| self.lower_value(function, *id, function.entry, None))
            .collect::<Result<Vec<_>, _>>()?;
        let signature = ir::Signature {
            result: type_id(function.result_type),
            parameters: parameters
                .iter()
                .map(|parameter| parameter.type_id)
                .collect(),
            variadic: false,
            calling_convention: ir::CallingConvention::Basic,
        };
        let blocks = function
            .blocks
            .iter()
            .map(|block| self.lower_block(function, block))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ir::Function {
            id: function_id(function.id),
            name: function.name.clone(),
            signature,
            linkage: linkage(function.linkage),
            attributes: Vec::new(),
            parameters,
            blocks,
        })
    }

    fn lower_block(
        &self,
        function: &hir::Function,
        block: &hir::Block,
    ) -> Result<ir::Block, LowerError> {
        Ok(ir::Block {
            id: block_id(block.id),
            instructions: block
                .instructions
                .iter()
                .map(|instruction| self.lower_instruction(function, block.id, instruction))
                .collect::<Result<Vec<_>, _>>()?,
            terminator: self.lower_terminator(function, block)?,
        })
    }

    fn lower_instruction(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::Instruction, LowerError> {
        use hir::Opcode;

        let kind = match instruction.opcode {
            Opcode::Copy => {
                let (result, operand, source_type) =
                    self.unary_parts(function, block, instruction)?;
                self.require_same_type(
                    function,
                    block,
                    instruction,
                    result.type_id,
                    source_type,
                    InvalidProperty::CopyTypes,
                )?;
                ir::InstructionKind::Cast {
                    op: ir::CastOp::Bitcast,
                    operand,
                    to: result.type_id,
                }
            }
            Opcode::Convert => {
                let (result, operand, source_type) =
                    self.unary_parts(function, block, instruction)?;
                ir::InstructionKind::Cast {
                    op: self.convert_op(
                        function,
                        block,
                        instruction,
                        source_type,
                        result.type_id,
                    )?,
                    operand,
                    to: result.type_id,
                }
            }
            Opcode::SignExtend => {
                self.explicit_extend(function, block, instruction, ir::CastOp::SignExtend)?
            }
            Opcode::ZeroExtend => {
                self.explicit_extend(function, block, instruction, ir::CastOp::ZeroExtend)?
            }
            Opcode::Add => self.integer_binary(function, block, instruction, ir::BinaryOp::Add)?,
            Opcode::Subtract => {
                self.integer_binary(function, block, instruction, ir::BinaryOp::Subtract)?
            }
            Opcode::Multiply => {
                self.integer_binary(function, block, instruction, ir::BinaryOp::Multiply)?
            }
            Opcode::Divide => self.divide_or_remainder(function, block, instruction, false)?,
            Opcode::Remainder => self.divide_or_remainder(function, block, instruction, true)?,
            Opcode::And => self.integer_binary(function, block, instruction, ir::BinaryOp::And)?,
            Opcode::Or => self.integer_binary(function, block, instruction, ir::BinaryOp::Or)?,
            Opcode::Xor => self.integer_binary(function, block, instruction, ir::BinaryOp::Xor)?,
            Opcode::ShiftLeft => {
                self.integer_binary(function, block, instruction, ir::BinaryOp::ShiftLeft)?
            }
            Opcode::ShiftRight => self.integer_binary(
                function,
                block,
                instruction,
                ir::BinaryOp::LogicalShiftRight,
            )?,
            Opcode::ShiftRightArithmetic => self.integer_binary(
                function,
                block,
                instruction,
                ir::BinaryOp::ArithmeticShiftRight,
            )?,
            Opcode::Negate => {
                self.integer_unary(function, block, instruction, ir::UnaryOp::Negate)?
            }
            Opcode::Not => self.integer_unary(function, block, instruction, ir::UnaryOp::Not)?,
            Opcode::Equal
            | Opcode::NotEqual
            | Opcode::LessThan
            | Opcode::LessEqual
            | Opcode::GreaterThan
            | Opcode::GreaterEqual => self.compare(function, block, instruction)?,
            Opcode::FloatAdd => {
                self.float_binary(function, block, instruction, ir::BinaryOp::FloatAdd)?
            }
            Opcode::FloatSubtract => {
                self.float_binary(function, block, instruction, ir::BinaryOp::FloatSubtract)?
            }
            Opcode::FloatMultiply => {
                self.float_binary(function, block, instruction, ir::BinaryOp::FloatMultiply)?
            }
            Opcode::FloatDivide => {
                self.float_binary(function, block, instruction, ir::BinaryOp::FloatDivide)?
            }
            Opcode::FloatNegate => {
                self.float_unary(function, block, instruction, ir::UnaryOp::FloatNegate)?
            }
            Opcode::FloatAbsolute => {
                self.float_unary(function, block, instruction, ir::UnaryOp::FloatAbsolute)?
            }
            Opcode::FloatSquareRoot => {
                self.float_intrinsic(function, block, instruction, ir::Intrinsic::SquareRoot)?
            }
            Opcode::FloatSine => {
                self.float_intrinsic(function, block, instruction, ir::Intrinsic::Sine)?
            }
            Opcode::FloatCosine => {
                self.float_intrinsic(function, block, instruction, ir::Intrinsic::Cosine)?
            }
            Opcode::FloatArctangent => {
                self.float_intrinsic(function, block, instruction, ir::Intrinsic::Arctangent)?
            }
            Opcode::FloatLog2 => {
                self.float_intrinsic(function, block, instruction, ir::Intrinsic::Log2)?
            }
            Opcode::FloatExp2 => {
                self.float_intrinsic(function, block, instruction, ir::Intrinsic::Exp2)?
            }
            Opcode::Load | Opcode::Store | Opcode::Address => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::Memory,
                );
            }
            Opcode::OffsetPointer | Opcode::PointerOffset | Opcode::PointerSegment => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::PointerExtraction,
                );
            }
            Opcode::Concat => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::Concatenation,
                );
            }
            Opcode::DivideRemainder => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::DivideRemainder,
                );
            }
            Opcode::StringEqual
            | Opcode::StringNotEqual
            | Opcode::StringLessThan
            | Opcode::StringLessEqual
            | Opcode::StringGreaterThan
            | Opcode::StringGreaterEqual => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::StringOperation,
                );
            }
            Opcode::Call => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::Call,
                );
            }
        };
        Ok(ir::Instruction {
            id: instruction_id(instruction.id),
            results: self
                .one_result(function, block, instruction)
                .map(|result| vec![result])?,
            kind,
        })
    }

    fn explicit_extend(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        op: ir::CastOp,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, operand, source_type) = self.unary_parts(function, block, instruction)?;
        if !self.is_integer(source_type)?
            || !self.is_integer_ir(result.type_id)?
            || self.integer_bits_by_id(source_type)?
                >= self.integer_bits_by_ir_id(result.type_id)?
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::CastTypes,
            );
        }
        Ok(ir::InstructionKind::Cast {
            op,
            operand,
            to: result.type_id,
        })
    }

    fn integer_binary(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        op: ir::BinaryOp,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, left, left_type, right, right_type) =
            self.binary_parts(function, block, instruction)?;
        if !self.is_integer(left_type)?
            || !self.is_integer(right_type)?
            || !self.is_integer_ir(result.type_id)?
            || type_id(left_type) != result.type_id
            || type_id(right_type) != result.type_id
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::Binary { op, left, right })
    }

    fn divide_or_remainder(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        remainder: bool,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, left, left_type, right, right_type) =
            self.binary_parts(function, block, instruction)?;
        if !self.is_integer(left_type)?
            || !self.is_integer(right_type)?
            || !self.is_integer_ir(result.type_id)?
            || type_id(left_type) != result.type_id
            || type_id(right_type) != result.type_id
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        let signed = self
            .type_by_id(left_type)?
            .signed
            .ok_or(LowerError::InvalidType {
                type_id: left_type,
                property: InvalidProperty::OperandTypes,
            })?;
        let op = match (signed, remainder) {
            (true, false) => ir::BinaryOp::SignedDivide,
            (false, false) => ir::BinaryOp::UnsignedDivide,
            (true, true) => ir::BinaryOp::SignedRemainder,
            (false, true) => ir::BinaryOp::UnsignedRemainder,
        };
        Ok(ir::InstructionKind::Binary { op, left, right })
    }

    fn integer_unary(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        op: ir::UnaryOp,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, operand, source_type) = self.unary_parts(function, block, instruction)?;
        if !self.is_integer(source_type)?
            || !self.is_integer_ir(result.type_id)?
            || type_id(source_type) != result.type_id
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::Unary { op, operand })
    }

    fn compare(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, left, left_type, right, right_type) =
            self.binary_parts(function, block, instruction)?;
        if !self.is_boolean_ir(result.type_id)? || left_type != right_type {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        let predicate = if self.is_float(left_type)? {
            float_compare_predicate(instruction.opcode)
        } else if self.is_integer(left_type)? {
            let signed = self.type_by_id(left_type)?.signed;
            if !matches!(
                instruction.opcode,
                hir::Opcode::Equal | hir::Opcode::NotEqual
            ) && signed.is_none()
            {
                return self.invalid_instruction(
                    function,
                    block,
                    instruction,
                    InvalidProperty::OperandTypes,
                );
            }
            integer_compare_predicate(instruction.opcode, signed.unwrap_or(false))
        } else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        .ok_or(LowerError::InvalidInstruction {
            function: function.id,
            block,
            instruction: instruction.id,
            property: InvalidProperty::OperandTypes,
        })?;
        Ok(ir::InstructionKind::Compare {
            predicate,
            left,
            right,
        })
    }

    fn float_binary(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        op: ir::BinaryOp,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, left, left_type, right, right_type) =
            self.binary_parts(function, block, instruction)?;
        if !self.is_float(left_type)?
            || !self.is_float(right_type)?
            || !self.is_float_ir(result.type_id)?
            || type_id(left_type) != result.type_id
            || type_id(right_type) != result.type_id
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::Binary { op, left, right })
    }

    fn float_unary(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        op: ir::UnaryOp,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, operand, source_type) = self.unary_parts(function, block, instruction)?;
        if !self.is_float(source_type)?
            || !self.is_float_ir(result.type_id)?
            || type_id(source_type) != result.type_id
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::Unary { op, operand })
    }

    fn float_intrinsic(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        intrinsic: ir::Intrinsic,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, operand, source_type) = self.unary_parts(function, block, instruction)?;
        if !self.is_float(source_type)?
            || !self.is_float_ir(result.type_id)?
            || type_id(source_type) != result.type_id
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::Intrinsic {
            intrinsic,
            arguments: vec![operand],
        })
    }

    fn convert_op(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        source: hir::TypeId,
        target: ir::TypeId,
    ) -> Result<ir::CastOp, LowerError> {
        let target_hir = hir::TypeId::new(target.get());
        let source_type = self.type_by_id(source)?;
        let target_type = self.type_by_id(target_hir)?;
        match (source_type.kind, target_type.kind) {
            (
                hir::TypeKind::Boolean | hir::TypeKind::Integer,
                hir::TypeKind::Boolean | hir::TypeKind::Integer,
            ) => {
                let source_bits = self.integer_bits(source_type)?;
                let target_bits = self.integer_bits(target_type)?;
                if source_bits > target_bits {
                    Ok(ir::CastOp::Truncate)
                } else if source_bits < target_bits {
                    match source_type.signed {
                        Some(true) => Ok(ir::CastOp::SignExtend),
                        Some(false) => Ok(ir::CastOp::ZeroExtend),
                        None => self.invalid_instruction(
                            function,
                            block,
                            instruction,
                            InvalidProperty::CastTypes,
                        ),
                    }
                } else {
                    Ok(ir::CastOp::Bitcast)
                }
            }
            (hir::TypeKind::Boolean | hir::TypeKind::Integer, hir::TypeKind::Float)
            | (hir::TypeKind::Float, hir::TypeKind::Boolean | hir::TypeKind::Integer) => self
                .unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::UnsupportedCast,
                ),
            (hir::TypeKind::Float, hir::TypeKind::Float) => {
                match self
                    .float_rank(source_type)?
                    .cmp(&self.float_rank(target_type)?)
                {
                    std::cmp::Ordering::Less => Ok(ir::CastOp::FloatExtend),
                    std::cmp::Ordering::Greater => Ok(ir::CastOp::FloatTruncate),
                    std::cmp::Ordering::Equal => Ok(ir::CastOp::Bitcast),
                }
            }
            (hir::TypeKind::Pointer, hir::TypeKind::Boolean | hir::TypeKind::Integer) => {
                Ok(ir::CastOp::PointerToInteger)
            }
            (hir::TypeKind::Boolean | hir::TypeKind::Integer, hir::TypeKind::Pointer) => {
                Ok(ir::CastOp::IntegerToPointer)
            }
            (hir::TypeKind::Pointer, hir::TypeKind::Pointer)
                if source_type.address == target_type.address =>
            {
                Ok(ir::CastOp::Bitcast)
            }
            _ => self.unsupported_instruction(
                function,
                block,
                instruction,
                UnsupportedFeature::UnsupportedCast,
            ),
        }
    }

    fn lower_terminator(
        &self,
        function: &hir::Function,
        block: &hir::Block,
    ) -> Result<ir::Terminator, LowerError> {
        match &block.terminator {
            hir::Terminator::Jump(target) => Ok(ir::Terminator::Jump(block_id(*target))),
            hir::Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                let condition_type = self.operand_type(function, block.id, None, 0, condition)?;
                if !matches!(
                    self.type_by_id(condition_type)?.kind,
                    hir::TypeKind::Boolean
                ) {
                    return Err(LowerError::InvalidBlock {
                        function: function.id,
                        block: block.id,
                        property: InvalidProperty::BranchCondition,
                    });
                }
                Ok(ir::Terminator::Branch {
                    condition: self.lower_operand(function, block.id, None, 0, condition)?,
                    then_block: block_id(*then_block),
                    else_block: block_id(*else_block),
                })
            }
            hir::Terminator::Switch {
                selector,
                cases,
                default,
            } => {
                let selector_type = self.operand_type(function, block.id, None, 0, selector)?;
                if !self.is_integer(selector_type)? {
                    return Err(LowerError::InvalidBlock {
                        function: function.id,
                        block: block.id,
                        property: InvalidProperty::SwitchSelector,
                    });
                }
                Ok(ir::Terminator::Switch {
                    selector: self.lower_operand(function, block.id, None, 0, selector)?,
                    cases: cases
                        .iter()
                        .map(|(value, target)| (i128::from(*value), block_id(*target)))
                        .collect(),
                    default: block_id(*default),
                })
            }
            hir::Terminator::Return(value) => {
                let result_type = self.type_by_id(function.result_type)?;
                let returns_void = matches!(result_type.kind, hir::TypeKind::Void);
                if returns_void != value.is_none() {
                    return Err(LowerError::InvalidBlock {
                        function: function.id,
                        block: block.id,
                        property: InvalidProperty::ReturnType,
                    });
                }
                if let Some(value) = value {
                    let value_type = self.operand_type(function, block.id, None, 0, value)?;
                    if value_type != function.result_type {
                        return Err(LowerError::InvalidBlock {
                            function: function.id,
                            block: block.id,
                            property: InvalidProperty::ReturnType,
                        });
                    }
                }
                Ok(ir::Terminator::Return(
                    value
                        .as_ref()
                        .map(|value| self.lower_operand(function, block.id, None, 0, value))
                        .transpose()?,
                ))
            }
            hir::Terminator::Unreachable => Ok(ir::Terminator::Unreachable),
        }
    }

    fn unary_parts(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<(ir::Value, ir::Operand, hir::TypeId), LowerError> {
        let result = self.one_result(function, block, instruction)?;
        let [operand] = instruction.operands.as_slice() else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        };
        let source_type = self.operand_type(function, block, Some(instruction.id), 0, operand)?;
        let operand = self.lower_operand(function, block, Some(instruction.id), 0, operand)?;
        Ok((result, operand, source_type))
    }

    fn binary_parts(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<
        (
            ir::Value,
            ir::Operand,
            hir::TypeId,
            ir::Operand,
            hir::TypeId,
        ),
        LowerError,
    > {
        let result = self.one_result(function, block, instruction)?;
        let [left, right] = instruction.operands.as_slice() else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        };
        let left_type = self.operand_type(function, block, Some(instruction.id), 0, left)?;
        let right_type = self.operand_type(function, block, Some(instruction.id), 1, right)?;
        let left = self.lower_operand(function, block, Some(instruction.id), 0, left)?;
        let right = self.lower_operand(function, block, Some(instruction.id), 1, right)?;
        Ok((result, left, left_type, right, right_type))
    }

    fn one_result(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::Value, LowerError> {
        let [result] = instruction.results.as_slice() else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::ResultArity,
            );
        };
        self.lower_value(function, *result, block, Some(instruction.id))
    }

    fn lower_operand(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        index: usize,
        operand: &hir::Operand,
    ) -> Result<ir::Operand, LowerError> {
        match operand {
            hir::Operand::Value(value) => {
                self.value_by_id(function, *value, block, instruction)?;
                Ok(ir::Operand::Value(value_id(*value)))
            }
            hir::Operand::Constant {
                type_id: constant_type_id,
                value,
            } => {
                let type_ = self.type_by_id(*constant_type_id)?;
                let value = match (type_.kind, value) {
                    (
                        hir::TypeKind::Boolean | hir::TypeKind::Integer,
                        hir::ConstantValue::Integer(value),
                    ) => ir::Constant::Integer(i128::from(*value)),
                    (hir::TypeKind::Float, hir::ConstantValue::Real(value)) => {
                        ir::Constant::Float(value.clone())
                    }
                    _ => {
                        return Err(self.invalid_operand(
                            function,
                            block,
                            instruction,
                            index,
                            InvalidProperty::ConstantType,
                        ));
                    }
                };
                Ok(ir::Operand::Constant(ir::TypedConstant {
                    type_id: type_id(*constant_type_id),
                    value,
                }))
            }
            hir::Operand::Place(_) => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Place,
            )),
            hir::Operand::Element { .. } => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Element,
            )),
            hir::Operand::Projection { .. } => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Projection,
            )),
            hir::Operand::Indirect { .. } => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Indirect,
            )),
        }
    }

    fn operand_type(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        index: usize,
        operand: &hir::Operand,
    ) -> Result<hir::TypeId, LowerError> {
        match operand {
            hir::Operand::Value(value) => Ok(self
                .value_by_id(function, *value, block, instruction)?
                .type_id),
            hir::Operand::Constant { type_id, .. } => {
                self.type_by_id(*type_id)?;
                Ok(*type_id)
            }
            hir::Operand::Place(_) => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Place,
            )),
            hir::Operand::Element { .. } => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Element,
            )),
            hir::Operand::Projection { .. } => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Projection,
            )),
            hir::Operand::Indirect { .. } => Err(self.unsupported_operand(
                function,
                block,
                instruction,
                index,
                UnsupportedOperand::Indirect,
            )),
        }
    }

    fn lower_value(
        &self,
        function: &hir::Function,
        value: hir::ValueId,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
    ) -> Result<ir::Value, LowerError> {
        let value = self.value_by_id(function, value, block, instruction)?;
        self.type_by_id(value.type_id)?;
        Ok(ir::Value {
            id: value_id(value.id),
            type_id: type_id(value.type_id),
        })
    }

    fn value_by_id<'function>(
        &self,
        function: &'function hir::Function,
        value: hir::ValueId,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
    ) -> Result<&'function hir::Value, LowerError> {
        function
            .values
            .iter()
            .find(|candidate| candidate.id == value)
            .ok_or(LowerError::UnknownValue {
                function: function.id,
                block,
                instruction,
                value,
            })
    }

    fn type_by_id(&self, id: hir::TypeId) -> Result<&hir::Type, LowerError> {
        self.module
            .types
            .iter()
            .find(|type_| type_.id == id)
            .ok_or(LowerError::UnknownType { type_id: id })
    }

    fn is_integer(&self, id: hir::TypeId) -> Result<bool, LowerError> {
        Ok(matches!(
            self.type_by_id(id)?.kind,
            hir::TypeKind::Boolean | hir::TypeKind::Integer
        ))
    }

    fn is_float(&self, id: hir::TypeId) -> Result<bool, LowerError> {
        Ok(matches!(self.type_by_id(id)?.kind, hir::TypeKind::Float))
    }

    fn is_integer_ir(&self, id: ir::TypeId) -> Result<bool, LowerError> {
        self.is_integer(hir::TypeId::new(id.get()))
    }

    fn is_boolean_ir(&self, id: ir::TypeId) -> Result<bool, LowerError> {
        Ok(matches!(
            self.type_by_id(hir::TypeId::new(id.get()))?.kind,
            hir::TypeKind::Boolean
        ))
    }

    fn is_float_ir(&self, id: ir::TypeId) -> Result<bool, LowerError> {
        self.is_float(hir::TypeId::new(id.get()))
    }

    fn integer_bits_by_id(&self, id: hir::TypeId) -> Result<u16, LowerError> {
        self.integer_bits(self.type_by_id(id)?)
    }

    fn integer_bits_by_ir_id(&self, id: ir::TypeId) -> Result<u16, LowerError> {
        self.integer_bits_by_id(hir::TypeId::new(id.get()))
    }

    fn float_rank(&self, type_: &hir::Type) -> Result<u8, LowerError> {
        match type_.evaluation {
            hir::FloatEvaluation::Binary32 => Ok(0),
            hir::FloatEvaluation::Binary64 => Ok(1),
            hir::FloatEvaluation::Extended80 => Ok(2),
            hir::FloatEvaluation::None => Err(LowerError::InvalidType {
                type_id: type_.id,
                property: InvalidProperty::MissingFloatEvaluation,
            }),
        }
    }

    fn require_same_type(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        result: ir::TypeId,
        source: hir::TypeId,
        property: InvalidProperty,
    ) -> Result<(), LowerError> {
        if result == type_id(source) {
            Ok(())
        } else {
            self.invalid_instruction(function, block, instruction, property)
        }
    }

    fn unsupported_instruction<T>(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        feature: UnsupportedFeature,
    ) -> Result<T, LowerError> {
        Err(LowerError::UnsupportedInstruction {
            function: function.id,
            block,
            instruction: instruction.id,
            opcode: instruction.opcode,
            feature,
        })
    }

    fn invalid_instruction<T>(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        property: InvalidProperty,
    ) -> Result<T, LowerError> {
        Err(LowerError::InvalidInstruction {
            function: function.id,
            block,
            instruction: instruction.id,
            property,
        })
    }

    fn unsupported_operand(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        index: usize,
        operand: UnsupportedOperand,
    ) -> LowerError {
        LowerError::UnsupportedOperand {
            function: function.id,
            block,
            instruction,
            index,
            operand,
        }
    }

    fn invalid_operand(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: Option<hir::InstructionId>,
        index: usize,
        property: InvalidProperty,
    ) -> LowerError {
        LowerError::InvalidOperand {
            function: function.id,
            block,
            instruction,
            index,
            property,
        }
    }
}

fn type_id(id: hir::TypeId) -> ir::TypeId {
    ir::TypeId::new(id.get())
}

fn function_id(id: hir::FunctionId) -> ir::FunctionId {
    ir::FunctionId::new(id.get())
}

fn block_id(id: hir::BlockId) -> ir::BlockId {
    ir::BlockId::new(id.get())
}

fn instruction_id(id: hir::InstructionId) -> ir::InstructionId {
    ir::InstructionId::new(id.get())
}

fn value_id(id: hir::ValueId) -> ir::ValueId {
    ir::ValueId::new(id.get())
}

fn linkage(linkage: hir::Linkage) -> ir::Linkage {
    match linkage {
        hir::Linkage::Internal => ir::Linkage::Internal,
        hir::Linkage::External => ir::Linkage::External,
    }
}

fn integer_compare_predicate(opcode: hir::Opcode, signed: bool) -> Option<ir::ComparePredicate> {
    use hir::Opcode;

    Some(match opcode {
        Opcode::Equal => ir::ComparePredicate::Equal,
        Opcode::NotEqual => ir::ComparePredicate::NotEqual,
        Opcode::LessThan if signed => ir::ComparePredicate::SignedLessThan,
        Opcode::LessThan => ir::ComparePredicate::UnsignedLessThan,
        Opcode::LessEqual if signed => ir::ComparePredicate::SignedLessEqual,
        Opcode::LessEqual => ir::ComparePredicate::UnsignedLessEqual,
        Opcode::GreaterThan if signed => ir::ComparePredicate::SignedGreaterThan,
        Opcode::GreaterThan => ir::ComparePredicate::UnsignedGreaterThan,
        Opcode::GreaterEqual if signed => ir::ComparePredicate::SignedGreaterEqual,
        Opcode::GreaterEqual => ir::ComparePredicate::UnsignedGreaterEqual,
        _ => return None,
    })
}

fn float_compare_predicate(opcode: hir::Opcode) -> Option<ir::ComparePredicate> {
    Some(match opcode {
        hir::Opcode::Equal => ir::ComparePredicate::OrderedEqual,
        hir::Opcode::NotEqual => ir::ComparePredicate::OrderedNotEqual,
        hir::Opcode::LessThan => ir::ComparePredicate::OrderedLessThan,
        hir::Opcode::LessEqual => ir::ComparePredicate::OrderedLessEqual,
        hir::Opcode::GreaterThan => ir::ComparePredicate::OrderedGreaterThan,
        hir::Opcode::GreaterEqual => ir::ComparePredicate::OrderedGreaterEqual,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_module() -> hir::Module {
        let types = vec![
            hir::Type {
                id: hir::TypeId::new(0),
                name: "void".into(),
                kind: hir::TypeKind::Void,
                width: 0,
                signed: None,
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(1),
                name: "boolean".into(),
                kind: hir::TypeKind::Boolean,
                width: 2,
                signed: Some(true),
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(2),
                name: "long".into(),
                kind: hir::TypeKind::Integer,
                width: 4,
                signed: Some(true),
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(3),
                name: "double".into(),
                kind: hir::TypeKind::Float,
                width: 8,
                signed: None,
                evaluation: hir::FloatEvaluation::Binary64,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(4),
                name: "near-long".into(),
                kind: hir::TypeKind::Pointer,
                width: 2,
                signed: None,
                evaluation: hir::FloatEvaluation::None,
                element: Some(hir::TypeId::new(2)),
                bounds: Vec::new(),
                address: hir::AddressKind::Near,
            },
            hir::Type {
                id: hir::TypeId::new(5),
                name: "single".into(),
                kind: hir::TypeKind::Float,
                width: 4,
                signed: None,
                evaluation: hir::FloatEvaluation::Binary32,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
        ];
        hir::Module {
            id: hir::ModuleId::new(7),
            name: "scalar".into(),
            types,
            functions: vec![hir::Function {
                id: hir::FunctionId::new(11),
                name: "main".into(),
                result_type: hir::TypeId::new(0),
                values: vec![
                    hir::Value {
                        id: hir::ValueId::new(0),
                        type_id: hir::TypeId::new(2),
                    },
                    hir::Value {
                        id: hir::ValueId::new(1),
                        type_id: hir::TypeId::new(2),
                    },
                    hir::Value {
                        id: hir::ValueId::new(2),
                        type_id: hir::TypeId::new(1),
                    },
                    hir::Value {
                        id: hir::ValueId::new(3),
                        type_id: hir::TypeId::new(3),
                    },
                    hir::Value {
                        id: hir::ValueId::new(4),
                        type_id: hir::TypeId::new(3),
                    },
                ],
                places: Vec::new(),
                blocks: vec![
                    hir::Block {
                        id: hir::BlockId::new(4),
                        instructions: vec![
                            hir::Instruction {
                                id: hir::InstructionId::new(10),
                                opcode: hir::Opcode::Add,
                                results: vec![hir::ValueId::new(1)],
                                operands: vec![
                                    hir::Operand::Value(hir::ValueId::new(0)),
                                    hir::Operand::Constant {
                                        type_id: hir::TypeId::new(2),
                                        value: hir::ConstantValue::Integer(1),
                                    },
                                ],
                                callee: None,
                            },
                            hir::Instruction {
                                id: hir::InstructionId::new(11),
                                opcode: hir::Opcode::LessThan,
                                results: vec![hir::ValueId::new(2)],
                                operands: vec![
                                    hir::Operand::Value(hir::ValueId::new(1)),
                                    hir::Operand::Constant {
                                        type_id: hir::TypeId::new(2),
                                        value: hir::ConstantValue::Integer(7),
                                    },
                                ],
                                callee: None,
                            },
                        ],
                        terminator: hir::Terminator::Branch {
                            condition: hir::Operand::Value(hir::ValueId::new(2)),
                            then_block: hir::BlockId::new(5),
                            else_block: hir::BlockId::new(6),
                        },
                    },
                    hir::Block {
                        id: hir::BlockId::new(5),
                        instructions: vec![hir::Instruction {
                            id: hir::InstructionId::new(12),
                            opcode: hir::Opcode::FloatSquareRoot,
                            results: vec![hir::ValueId::new(3)],
                            operands: vec![hir::Operand::Constant {
                                type_id: hir::TypeId::new(3),
                                value: hir::ConstantValue::Real("4.0".into()),
                            }],
                            callee: None,
                        }],
                        terminator: hir::Terminator::Return(None),
                    },
                    hir::Block {
                        id: hir::BlockId::new(6),
                        instructions: vec![hir::Instruction {
                            id: hir::InstructionId::new(13),
                            opcode: hir::Opcode::Convert,
                            results: vec![hir::ValueId::new(4)],
                            operands: vec![hir::Operand::Constant {
                                type_id: hir::TypeId::new(5),
                                value: hir::ConstantValue::Real("1.0".into()),
                            }],
                            callee: None,
                        }],
                        terminator: hir::Terminator::Return(None),
                    },
                ],
                entry: hir::BlockId::new(4),
                parameters: vec![hir::ValueId::new(0)],
                abi: hir::ProcedureAbi {
                    cleanup: hir::StackCleanup::Callee,
                    distance: hir::CallDistance::Far,
                    parameter_bytes: 4,
                },
                calls: Vec::new(),
                error_handler: None,
                error_handler_local: false,
                external_entries: Vec::new(),
                linkage: hir::Linkage::Internal,
            }],
            data: Vec::new(),
            callables: Vec::new(),
        }
    }

    #[test]
    fn lowers_a_complete_scalar_cfg() {
        let lowered = lower_module(&scalar_module()).expect("scalar HIR lowers");

        assert_eq!(lowered.types[1].kind, ir::TypeKind::Integer { bits: 1 });
        assert_eq!(
            lowered.types[3].kind,
            ir::TypeKind::Float(ir::FloatKind::Binary64)
        );
        assert_eq!(
            lowered.types[4].kind,
            ir::TypeKind::Pointer {
                address_space: ir::AddressSpace::NearData
            }
        );
        let function = &lowered.functions[0];
        assert_eq!(function.id, ir::FunctionId::new(11));
        assert_eq!(
            function.signature.calling_convention,
            ir::CallingConvention::Basic
        );
        assert_eq!(function.parameters[0].id, ir::ValueId::new(0));
        assert_eq!(function.blocks[0].id, ir::BlockId::new(4));
        assert!(matches!(
            &function.blocks[0].instructions[0].kind,
            ir::InstructionKind::Binary {
                op: ir::BinaryOp::Add,
                ..
            }
        ));
        assert!(matches!(
            &function.blocks[0].instructions[1].kind,
            ir::InstructionKind::Compare {
                predicate: ir::ComparePredicate::SignedLessThan,
                ..
            }
        ));
        let ir::Terminator::Branch {
            then_block,
            else_block,
            ..
        } = &function.blocks[0].terminator
        else {
            panic!("entry terminator was not a branch");
        };
        assert_eq!(*then_block, ir::BlockId::new(5));
        assert_eq!(*else_block, ir::BlockId::new(6));
        assert!(matches!(
            &function.blocks[1].instructions[0].kind,
            ir::InstructionKind::Intrinsic {
                intrinsic: ir::Intrinsic::SquareRoot,
                ..
            }
        ));
        assert!(matches!(
            &function.blocks[2].instructions[0].kind,
            ir::InstructionKind::Cast {
                op: ir::CastOp::FloatExtend,
                ..
            }
        ));
        assert!(lowered.verify().is_ok());
    }

    #[test]
    fn refuses_functions_with_places() {
        let mut module = scalar_module();
        module.functions[0].places.push(hir::Place {
            id: hir::PlaceId::new(0),
            name: "slot".into(),
            type_id: hir::TypeId::new(2),
            storage: hir::Storage::Local,
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        });

        assert_eq!(
            lower_module(&module),
            Err(LowerError::UnsupportedFunction {
                function: hir::FunctionId::new(11),
                feature: UnsupportedFeature::Place,
            })
        );
    }

    #[test]
    fn refuses_integer_to_float_without_signed_cast_semantics() {
        let mut module = scalar_module();
        module.functions[0].blocks[2].instructions[0].operands =
            vec![hir::Operand::Value(hir::ValueId::new(1))];

        assert_eq!(
            lower_module(&module),
            Err(LowerError::UnsupportedInstruction {
                function: hir::FunctionId::new(11),
                block: hir::BlockId::new(6),
                instruction: hir::InstructionId::new(13),
                opcode: hir::Opcode::Convert,
                feature: UnsupportedFeature::UnsupportedCast,
            })
        );
    }
}
