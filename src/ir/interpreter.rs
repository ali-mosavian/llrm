//! A small, deterministic interpreter for the integer subset of portable IR.
//!
//! This is deliberately a semantic oracle rather than a backend.  It has no
//! memory, call, or host-layout model: instructions needing one are refused.

use std::collections::BTreeMap;
use std::fmt;

use crate::support::diagnostic::Diagnostic;

use super::{
    BinaryOp, Block, BlockId, CastOp, ComparePredicate, Constant, Function, FunctionId,
    Instruction, InstructionId, InstructionKind, Module, Operand, Terminator, TypeId, TypeKind,
    TypedConstant, UnaryOp, ValueId,
};

/// The maximum number of instructions and terminators executed by default.
pub const DEFAULT_STEP_LIMIT: usize = 1_000_000;

/// An integer value represented by its exact IR width and unsigned bit pattern.
///
/// Signedness is an interpretation selected by an operation, never an
/// attribute of the value.  `raw_bits` is always normalized to `bits` bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeInteger {
    bits: u16,
    raw_bits: u128,
}

impl RuntimeInteger {
    /// Builds an integer from an unsigned bit pattern, truncating to `bits`.
    pub fn from_unsigned(bits: u16, value: u128) -> Result<Self, InterpretError> {
        let mask = mask(bits)?;
        Ok(Self {
            bits,
            raw_bits: value & mask,
        })
    }

    /// Builds an integer from a signed value, truncating to `bits`.
    pub fn from_signed(bits: u16, value: i128) -> Result<Self, InterpretError> {
        Self::from_unsigned(bits, value as u128)
    }

    /// The fixed width of this integer.
    pub const fn bits(self) -> u16 {
        self.bits
    }

    /// The canonical unsigned bit pattern of this integer.
    pub const fn raw_bits(self) -> u128 {
        self.raw_bits
    }

    /// Interprets the bit pattern as an unsigned integer.
    pub const fn as_unsigned(self) -> u128 {
        self.raw_bits
    }

    /// Interprets the bit pattern as a two's-complement signed integer.
    pub const fn as_signed(self) -> i128 {
        if self.bits == 128 {
            self.raw_bits as i128
        } else {
            let sign = 1u128 << (self.bits - 1);
            if self.raw_bits & sign == 0 {
                self.raw_bits as i128
            } else {
                (self.raw_bits | !((1u128 << self.bits) - 1)) as i128
            }
        }
    }
}

/// A deterministic, explicit refusal or trap encountered during execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterpretError {
    InvalidModule(Vec<Diagnostic>),
    UnknownFunction(FunctionId),
    Declaration(FunctionId),
    UnsupportedVariadic(FunctionId),
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    ArgumentType {
        index: usize,
        expected: TypeId,
        actual_bits: u16,
    },
    UnknownBlock(BlockId),
    UnknownValue(ValueId),
    TypeMismatch {
        context: &'static str,
        expected: TypeId,
        actual: TypeId,
    },
    ExpectedI1 {
        context: &'static str,
        actual: TypeId,
    },
    UnsupportedWidth(u16),
    UnsupportedType(TypeId),
    UnsupportedConstant,
    UnsupportedInstruction(InstructionId),
    InvalidCast {
        instruction: InstructionId,
        from: TypeId,
        to: TypeId,
    },
    MissingPhiIncoming {
        block: BlockId,
        predecessor: Option<BlockId>,
    },
    Unreachable(BlockId),
    DivideByZero(InstructionId),
    SignedDivisionOverflow(InstructionId),
    OversizedShift(InstructionId),
    StepLimit {
        limit: usize,
    },
}

impl fmt::Display for InterpretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModule(diagnostics) => {
                write!(
                    formatter,
                    "cannot interpret invalid module ({})",
                    diagnostics.len()
                )
            }
            Self::UnknownFunction(id) => write!(formatter, "unknown function {id}"),
            Self::Declaration(id) => write!(formatter, "function {id} is only a declaration"),
            Self::UnsupportedVariadic(id) => {
                write!(formatter, "variadic function {id} is unsupported")
            }
            Self::ArgumentCount { expected, actual } => {
                write!(
                    formatter,
                    "function expects {expected} arguments, got {actual}"
                )
            }
            Self::ArgumentType {
                index,
                expected,
                actual_bits,
            } => write!(
                formatter,
                "argument {index} has width {actual_bits}, expected integer type {expected}"
            ),
            Self::UnknownBlock(id) => write!(formatter, "unknown block {id}"),
            Self::UnknownValue(id) => write!(formatter, "unknown or unavailable value {id}"),
            Self::TypeMismatch {
                context,
                expected,
                actual,
            } => write!(
                formatter,
                "{context} has type {actual}, expected {expected}"
            ),
            Self::ExpectedI1 { context, actual } => {
                write!(
                    formatter,
                    "{context} has type {actual}, expected an i1 integer"
                )
            }
            Self::UnsupportedWidth(bits) => write!(formatter, "unsupported integer width {bits}"),
            Self::UnsupportedType(id) => write!(formatter, "unsupported type {id}"),
            Self::UnsupportedConstant => write!(formatter, "unsupported non-integer constant"),
            Self::UnsupportedInstruction(id) => write!(formatter, "unsupported instruction {id}"),
            Self::InvalidCast {
                instruction,
                from,
                to,
            } => write!(
                formatter,
                "invalid integer cast in instruction {instruction}: {from} to {to}"
            ),
            Self::MissingPhiIncoming { block, predecessor } => match predecessor {
                Some(predecessor) => write!(
                    formatter,
                    "block {block} has no phi input for predecessor {predecessor}"
                ),
                None => write!(
                    formatter,
                    "entry block {block} has a phi without an incoming edge"
                ),
            },
            Self::Unreachable(block) => write!(
                formatter,
                "executed unreachable terminator in block {block}"
            ),
            Self::DivideByZero(instruction) => {
                write!(formatter, "division by zero in instruction {instruction}")
            }
            Self::SignedDivisionOverflow(instruction) => write!(
                formatter,
                "signed minimum divided by -1 in instruction {instruction}"
            ),
            Self::OversizedShift(instruction) => {
                write!(formatter, "oversized shift in instruction {instruction}")
            }
            Self::StepLimit { limit } => write!(formatter, "execution exceeded step limit {limit}"),
        }
    }
}

impl std::error::Error for InterpretError {}

/// Configurable executor for one function at a time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Interpreter {
    step_limit: usize,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new(DEFAULT_STEP_LIMIT)
    }
}

impl Interpreter {
    /// Creates an interpreter that allows at most `step_limit` operations.
    pub const fn new(step_limit: usize) -> Self {
        Self { step_limit }
    }

    /// Executes `function` with integer arguments.
    pub fn execute(
        self,
        module: &Module,
        function: FunctionId,
        arguments: &[RuntimeInteger],
    ) -> Result<Option<RuntimeInteger>, InterpretError> {
        module.verify().map_err(InterpretError::InvalidModule)?;
        let function = module
            .functions
            .iter()
            .find(|candidate| candidate.id == function)
            .ok_or(InterpretError::UnknownFunction(function))?;
        if function.blocks.is_empty() {
            return Err(InterpretError::Declaration(function.id));
        }
        if function.signature.variadic {
            return Err(InterpretError::UnsupportedVariadic(function.id));
        }
        if function.parameters.len() != arguments.len() {
            return Err(InterpretError::ArgumentCount {
                expected: function.parameters.len(),
                actual: arguments.len(),
            });
        }

        let types = TypeTable::new(module);
        let result_bits = match types.kind(function.signature.result)? {
            TypeKind::Void => None,
            TypeKind::Integer { bits } => {
                mask(*bits)?;
                Some(*bits)
            }
            _ => return Err(InterpretError::UnsupportedType(function.signature.result)),
        };
        let value_types = value_types(function);
        let mut values = BTreeMap::new();
        for (index, (parameter, argument)) in function.parameters.iter().zip(arguments).enumerate()
        {
            let bits = types.integer_bits(parameter.type_id)?;
            if argument.bits != bits {
                return Err(InterpretError::ArgumentType {
                    index,
                    expected: parameter.type_id,
                    actual_bits: argument.bits,
                });
            }
            values.insert(parameter.id, *argument);
        }

        let blocks = function
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        let mut current = function.blocks[0].id;
        let mut predecessor = None;
        let mut steps = 0usize;

        loop {
            let block = blocks
                .get(&current)
                .copied()
                .ok_or(InterpretError::UnknownBlock(current))?;
            execute_phis(
                block,
                predecessor,
                &mut values,
                &value_types,
                &types,
                &mut steps,
                self.step_limit,
            )?;
            for instruction in block
                .instructions
                .iter()
                .skip_while(|instruction| matches!(instruction.kind, InstructionKind::Phi { .. }))
            {
                consume_step(&mut steps, self.step_limit)?;
                let value = execute_instruction(instruction, &values, &value_types, &types)?;
                if let Some(value) = value {
                    let result = single_result(instruction)?;
                    values.insert(result.id, value);
                }
            }

            consume_step(&mut steps, self.step_limit)?;
            match &block.terminator {
                Terminator::Jump(target) => {
                    predecessor = Some(block.id);
                    current = *target;
                }
                Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let (condition, type_id) = operand(condition, &values, &value_types, &types)?;
                    require_i1(type_id, &types, "branch condition")?;
                    predecessor = Some(block.id);
                    current = if condition.raw_bits == 0 {
                        *else_block
                    } else {
                        *then_block
                    };
                }
                Terminator::Switch {
                    selector,
                    cases,
                    default,
                } => {
                    let (selector, selector_type) =
                        operand(selector, &values, &value_types, &types)?;
                    let selector_bits = types.integer_bits(selector_type)?;
                    predecessor = Some(block.id);
                    current = *default;
                    for (value, target) in cases {
                        if RuntimeInteger::from_signed(selector_bits, *value)?.raw_bits
                            == selector.raw_bits
                        {
                            current = *target;
                            break;
                        }
                    }
                }
                Terminator::Return(value) => {
                    return match value {
                        None => {
                            if !matches!(types.kind(function.signature.result)?, TypeKind::Void) {
                                Err(InterpretError::UnsupportedType(function.signature.result))
                            } else {
                                Ok(None)
                            }
                        }
                        Some(value) => {
                            let (value, type_id) = operand(value, &values, &value_types, &types)?;
                            if type_id != function.signature.result {
                                return Err(InterpretError::TypeMismatch {
                                    context: "return value",
                                    expected: function.signature.result,
                                    actual: type_id,
                                });
                            }
                            let Some(result_bits) = result_bits else {
                                return Err(InterpretError::UnsupportedType(
                                    function.signature.result,
                                ));
                            };
                            if value.bits != result_bits {
                                return Err(InterpretError::ArgumentType {
                                    index: 0,
                                    expected: function.signature.result,
                                    actual_bits: value.bits,
                                });
                            }
                            Ok(Some(value))
                        }
                    };
                }
                Terminator::Unreachable => return Err(InterpretError::Unreachable(block.id)),
            }
        }
    }
}

/// Executes one function with [`DEFAULT_STEP_LIMIT`].
pub fn interpret(
    module: &Module,
    function: FunctionId,
    arguments: &[RuntimeInteger],
) -> Result<Option<RuntimeInteger>, InterpretError> {
    Interpreter::default().execute(module, function, arguments)
}

/// Executes one function with an explicit instruction and terminator limit.
pub fn interpret_with_step_limit(
    module: &Module,
    function: FunctionId,
    arguments: &[RuntimeInteger],
    step_limit: usize,
) -> Result<Option<RuntimeInteger>, InterpretError> {
    Interpreter::new(step_limit).execute(module, function, arguments)
}

struct TypeTable<'module> {
    types: BTreeMap<TypeId, &'module TypeKind>,
}

impl<'module> TypeTable<'module> {
    fn new(module: &'module Module) -> Self {
        Self {
            types: module
                .types
                .iter()
                .map(|type_| (type_.id, &type_.kind))
                .collect(),
        }
    }

    fn kind(&self, type_id: TypeId) -> Result<&TypeKind, InterpretError> {
        self.types
            .get(&type_id)
            .copied()
            .ok_or(InterpretError::UnsupportedType(type_id))
    }

    fn integer_bits(&self, type_id: TypeId) -> Result<u16, InterpretError> {
        match self.kind(type_id)? {
            TypeKind::Integer { bits } => {
                mask(*bits)?;
                Ok(*bits)
            }
            _ => Err(InterpretError::UnsupportedType(type_id)),
        }
    }
}

fn value_types(function: &Function) -> BTreeMap<ValueId, TypeId> {
    let mut types = BTreeMap::new();
    for value in &function.parameters {
        types.insert(value.id, value.type_id);
    }
    for block in &function.blocks {
        for instruction in &block.instructions {
            for value in &instruction.results {
                types.insert(value.id, value.type_id);
            }
        }
    }
    types
}

fn execute_phis(
    block: &Block,
    predecessor: Option<BlockId>,
    values: &mut BTreeMap<ValueId, RuntimeInteger>,
    value_types: &BTreeMap<ValueId, TypeId>,
    types: &TypeTable<'_>,
    steps: &mut usize,
    limit: usize,
) -> Result<(), InterpretError> {
    let mut pending = Vec::new();
    for instruction in &block.instructions {
        let InstructionKind::Phi { incoming } = &instruction.kind else {
            break;
        };
        consume_step(steps, limit)?;
        let source = incoming
            .iter()
            .find(|source| Some(source.predecessor) == predecessor)
            .ok_or(InterpretError::MissingPhiIncoming {
                block: block.id,
                predecessor,
            })?;
        let (value, value_type) = operand(&source.value, values, value_types, types)?;
        let result = single_result(instruction)?;
        if result.type_id != value_type {
            return Err(InterpretError::TypeMismatch {
                context: "phi input",
                expected: result.type_id,
                actual: value_type,
            });
        }
        require_integer(result.type_id, types)?;
        pending.push((result.id, value));
    }
    for (id, value) in pending {
        values.insert(id, value);
    }
    Ok(())
}

fn execute_instruction(
    instruction: &Instruction,
    values: &BTreeMap<ValueId, RuntimeInteger>,
    value_types: &BTreeMap<ValueId, TypeId>,
    types: &TypeTable<'_>,
) -> Result<Option<RuntimeInteger>, InterpretError> {
    match &instruction.kind {
        InstructionKind::Phi { .. } => Err(InterpretError::UnsupportedInstruction(instruction.id)),
        InstructionKind::Unary {
            op,
            operand: source,
        } => {
            let result = single_result(instruction)?;
            let (source, source_type) = operand(source, values, value_types, types)?;
            require_same_type("unary operand", result.type_id, source_type)?;
            require_integer(result.type_id, types)?;
            let raw = match op {
                UnaryOp::Negate => 0u128.wrapping_sub(source.raw_bits),
                UnaryOp::Not => !source.raw_bits,
                UnaryOp::FloatNegate | UnaryOp::FloatAbsolute => {
                    return Err(InterpretError::UnsupportedInstruction(instruction.id));
                }
            };
            Ok(Some(RuntimeInteger::from_unsigned(
                result_type_bits(result, types)?,
                raw,
            )?))
        }
        InstructionKind::Binary { op, left, right } => {
            let result = single_result(instruction)?;
            let (left, left_type) = operand(left, values, value_types, types)?;
            let (right, right_type) = operand(right, values, value_types, types)?;
            require_same_type("binary left operand", result.type_id, left_type)?;
            require_same_type("binary right operand", result.type_id, right_type)?;
            let bits = result_type_bits(result, types)?;
            let value = execute_binary(*op, left, right, bits, instruction.id)?;
            Ok(Some(value))
        }
        InstructionKind::Compare {
            predicate,
            left,
            right,
        } => {
            let result = single_result(instruction)?;
            let (left, left_type) = operand(left, values, value_types, types)?;
            let (right, right_type) = operand(right, values, value_types, types)?;
            require_same_type("compare operands", left_type, right_type)?;
            require_i1(result.type_id, types, "compare result")?;
            let result = match predicate {
                ComparePredicate::Equal => left.raw_bits == right.raw_bits,
                ComparePredicate::NotEqual => left.raw_bits != right.raw_bits,
                ComparePredicate::SignedLessThan => left.as_signed() < right.as_signed(),
                ComparePredicate::SignedLessEqual => left.as_signed() <= right.as_signed(),
                ComparePredicate::SignedGreaterThan => left.as_signed() > right.as_signed(),
                ComparePredicate::SignedGreaterEqual => left.as_signed() >= right.as_signed(),
                ComparePredicate::UnsignedLessThan => left.raw_bits < right.raw_bits,
                ComparePredicate::UnsignedLessEqual => left.raw_bits <= right.raw_bits,
                ComparePredicate::UnsignedGreaterThan => left.raw_bits > right.raw_bits,
                ComparePredicate::UnsignedGreaterEqual => left.raw_bits >= right.raw_bits,
                ComparePredicate::OrderedEqual
                | ComparePredicate::OrderedNotEqual
                | ComparePredicate::OrderedLessThan
                | ComparePredicate::OrderedLessEqual
                | ComparePredicate::OrderedGreaterThan
                | ComparePredicate::OrderedGreaterEqual => {
                    return Err(InterpretError::UnsupportedInstruction(instruction.id));
                }
            };
            Ok(Some(RuntimeInteger::from_unsigned(1, u128::from(result))?))
        }
        InstructionKind::Cast {
            op,
            operand: source,
            to,
        } => {
            let result = single_result(instruction)?;
            if result.type_id != *to {
                return Err(InterpretError::TypeMismatch {
                    context: "cast result",
                    expected: *to,
                    actual: result.type_id,
                });
            }
            let (source, source_type) = operand(source, values, value_types, types)?;
            let source_bits = types.integer_bits(source_type)?;
            let target_bits = types.integer_bits(*to)?;
            let raw = match op {
                CastOp::Truncate if target_bits < source_bits => source.raw_bits,
                CastOp::SignExtend if target_bits > source_bits => source.as_signed() as u128,
                CastOp::ZeroExtend if target_bits > source_bits => source.raw_bits,
                CastOp::Bitcast if target_bits == source_bits => source.raw_bits,
                CastOp::Truncate | CastOp::SignExtend | CastOp::ZeroExtend | CastOp::Bitcast => {
                    return Err(InterpretError::InvalidCast {
                        instruction: instruction.id,
                        from: source_type,
                        to: *to,
                    });
                }
                CastOp::IntegerToFloat
                | CastOp::FloatToInteger { .. }
                | CastOp::FloatExtend
                | CastOp::FloatTruncate
                | CastOp::PointerToInteger
                | CastOp::IntegerToPointer => {
                    return Err(InterpretError::UnsupportedInstruction(instruction.id));
                }
            };
            Ok(Some(RuntimeInteger::from_unsigned(target_bits, raw)?))
        }
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            let result = single_result(instruction)?;
            let (condition, condition_type) = operand(condition, values, value_types, types)?;
            require_i1(condition_type, types, "select condition")?;
            let (then_value, then_type) = operand(then_value, values, value_types, types)?;
            let (else_value, else_type) = operand(else_value, values, value_types, types)?;
            require_same_type("select then value", result.type_id, then_type)?;
            require_same_type("select else value", result.type_id, else_type)?;
            require_integer(result.type_id, types)?;
            Ok(Some(if condition.raw_bits == 0 {
                else_value
            } else {
                then_value
            }))
        }
        InstructionKind::StackAlloc { .. }
        | InstructionKind::ParameterAddress { .. }
        | InstructionKind::Load { .. }
        | InstructionKind::Store { .. }
        | InstructionKind::GetElementPointer { .. }
        | InstructionKind::ComposePointer { .. }
        | InstructionKind::Call { .. }
        | InstructionKind::Intrinsic { .. } => {
            Err(InterpretError::UnsupportedInstruction(instruction.id))
        }
    }
}

fn execute_binary(
    op: BinaryOp,
    left: RuntimeInteger,
    right: RuntimeInteger,
    bits: u16,
    instruction: InstructionId,
) -> Result<RuntimeInteger, InterpretError> {
    let raw = match op {
        BinaryOp::Add => left.raw_bits.wrapping_add(right.raw_bits),
        BinaryOp::Subtract => left.raw_bits.wrapping_sub(right.raw_bits),
        BinaryOp::Multiply => left.raw_bits.wrapping_mul(right.raw_bits),
        BinaryOp::And => left.raw_bits & right.raw_bits,
        BinaryOp::Or => left.raw_bits | right.raw_bits,
        BinaryOp::Xor => left.raw_bits ^ right.raw_bits,
        BinaryOp::ShiftLeft | BinaryOp::LogicalShiftRight | BinaryOp::ArithmeticShiftRight => {
            if right.raw_bits >= u128::from(bits) {
                return Err(InterpretError::OversizedShift(instruction));
            }
            let count = right.raw_bits as u32;
            match op {
                BinaryOp::ShiftLeft => left.raw_bits << count,
                BinaryOp::LogicalShiftRight => left.raw_bits >> count,
                BinaryOp::ArithmeticShiftRight => (left.as_signed() >> count) as u128,
                _ => return Err(InterpretError::UnsupportedInstruction(instruction)),
            }
        }
        BinaryOp::UnsignedDivide | BinaryOp::UnsignedRemainder => {
            if right.raw_bits == 0 {
                return Err(InterpretError::DivideByZero(instruction));
            }
            if matches!(op, BinaryOp::UnsignedDivide) {
                left.raw_bits / right.raw_bits
            } else {
                left.raw_bits % right.raw_bits
            }
        }
        BinaryOp::SignedDivide | BinaryOp::SignedRemainder => {
            let divisor = right.as_signed();
            if divisor == 0 {
                return Err(InterpretError::DivideByZero(instruction));
            }
            let dividend = left.as_signed();
            if dividend == signed_minimum(bits)? && divisor == -1 {
                return Err(InterpretError::SignedDivisionOverflow(instruction));
            }
            if matches!(op, BinaryOp::SignedDivide) {
                (dividend / divisor) as u128
            } else {
                (dividend % divisor) as u128
            }
        }
        BinaryOp::FloatAdd
        | BinaryOp::FloatSubtract
        | BinaryOp::FloatMultiply
        | BinaryOp::FloatDivide => return Err(InterpretError::UnsupportedInstruction(instruction)),
    };
    RuntimeInteger::from_unsigned(bits, raw)
}

fn operand(
    operand: &Operand,
    values: &BTreeMap<ValueId, RuntimeInteger>,
    value_types: &BTreeMap<ValueId, TypeId>,
    types: &TypeTable<'_>,
) -> Result<(RuntimeInteger, TypeId), InterpretError> {
    match operand {
        Operand::Value(id) => {
            let value = values
                .get(id)
                .copied()
                .ok_or(InterpretError::UnknownValue(*id))?;
            let type_id = value_types
                .get(id)
                .copied()
                .ok_or(InterpretError::UnknownValue(*id))?;
            let bits = types.integer_bits(type_id)?;
            if value.bits != bits {
                return Err(InterpretError::ArgumentType {
                    index: 0,
                    expected: type_id,
                    actual_bits: value.bits,
                });
            }
            Ok((value, type_id))
        }
        Operand::Constant(constant) => constant_integer(constant, types),
    }
}

fn constant_integer(
    constant: &TypedConstant,
    types: &TypeTable<'_>,
) -> Result<(RuntimeInteger, TypeId), InterpretError> {
    let bits = types.integer_bits(constant.type_id)?;
    let Constant::Integer(value) = &constant.value else {
        return Err(InterpretError::UnsupportedConstant);
    };
    Ok((RuntimeInteger::from_signed(bits, *value)?, constant.type_id))
}

fn single_result(instruction: &Instruction) -> Result<&super::Value, InterpretError> {
    match instruction.results.as_slice() {
        [result] => Ok(result),
        _ => Err(InterpretError::UnsupportedInstruction(instruction.id)),
    }
}

fn result_type_bits(result: &super::Value, types: &TypeTable<'_>) -> Result<u16, InterpretError> {
    types.integer_bits(result.type_id)
}

fn require_same_type(
    context: &'static str,
    expected: TypeId,
    actual: TypeId,
) -> Result<(), InterpretError> {
    if expected == actual {
        Ok(())
    } else {
        Err(InterpretError::TypeMismatch {
            context,
            expected,
            actual,
        })
    }
}

fn require_integer(type_id: TypeId, types: &TypeTable<'_>) -> Result<(), InterpretError> {
    types.integer_bits(type_id).map(|_| ())
}

fn require_i1(
    type_id: TypeId,
    types: &TypeTable<'_>,
    context: &'static str,
) -> Result<(), InterpretError> {
    if types.integer_bits(type_id)? == 1 {
        Ok(())
    } else {
        Err(InterpretError::ExpectedI1 {
            context,
            actual: type_id,
        })
    }
}

fn consume_step(steps: &mut usize, limit: usize) -> Result<(), InterpretError> {
    if *steps >= limit {
        return Err(InterpretError::StepLimit { limit });
    }
    *steps += 1;
    Ok(())
}

fn mask(bits: u16) -> Result<u128, InterpretError> {
    match bits {
        1..=127 => Ok((1u128 << bits) - 1),
        128 => Ok(u128::MAX),
        _ => Err(InterpretError::UnsupportedWidth(bits)),
    }
}

fn signed_minimum(bits: u16) -> Result<i128, InterpretError> {
    match bits {
        1..=127 => Ok(-(1i128 << (bits - 1))),
        128 => Ok(i128::MIN),
        _ => Err(InterpretError::UnsupportedWidth(bits)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Block, CallingConvention, Function, Instruction, Linkage, PhiIncoming, Signature, Type,
        Value,
    };

    const VOID: TypeId = TypeId::new(0);
    const I1: TypeId = TypeId::new(1);
    const I8: TypeId = TypeId::new(2);

    fn module(function: Function) -> Module {
        Module {
            name: "interpreter-test".into(),
            types: vec![
                Type {
                    id: VOID,
                    kind: TypeKind::Void,
                },
                Type {
                    id: I1,
                    kind: TypeKind::Integer { bits: 1 },
                },
                Type {
                    id: I8,
                    kind: TypeKind::Integer { bits: 8 },
                },
            ],
            globals: Vec::new(),
            functions: vec![function],
        }
    }

    fn function(parameters: Vec<Value>, blocks: Vec<Block>) -> Function {
        Function {
            id: FunctionId::new(0),
            name: "main".into(),
            signature: Signature {
                result: I8,
                parameters: parameters.iter().map(|value| value.type_id).collect(),
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters,
            blocks,
        }
    }

    fn integer(type_id: TypeId, value: i128) -> Operand {
        Operand::Constant(TypedConstant {
            type_id,
            value: Constant::Integer(value),
        })
    }

    #[test]
    fn loop_phi_selects_the_taken_edge_and_wraps() {
        let argument = Value {
            id: ValueId::new(0),
            type_id: I8,
        };
        let sum = Value {
            id: ValueId::new(1),
            type_id: I8,
        };
        let count = Value {
            id: ValueId::new(2),
            type_id: I8,
        };
        let next_sum = Value {
            id: ValueId::new(3),
            type_id: I8,
        };
        let next_count = Value {
            id: ValueId::new(4),
            type_id: I8,
        };
        let done = Value {
            id: ValueId::new(5),
            type_id: I1,
        };
        let test = module(function(
            vec![argument.clone()],
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Jump(BlockId::new(1)),
                },
                Block {
                    id: BlockId::new(1),
                    instructions: vec![
                        Instruction {
                            id: InstructionId::new(0),
                            results: vec![sum.clone()],
                            kind: InstructionKind::Phi {
                                incoming: vec![
                                    PhiIncoming {
                                        predecessor: BlockId::new(0),
                                        value: integer(I8, 250),
                                    },
                                    PhiIncoming {
                                        predecessor: BlockId::new(2),
                                        value: Operand::Value(next_sum.id),
                                    },
                                ],
                            },
                        },
                        Instruction {
                            id: InstructionId::new(1),
                            results: vec![count.clone()],
                            kind: InstructionKind::Phi {
                                incoming: vec![
                                    PhiIncoming {
                                        predecessor: BlockId::new(0),
                                        value: Operand::Value(argument.id),
                                    },
                                    PhiIncoming {
                                        predecessor: BlockId::new(2),
                                        value: Operand::Value(next_count.id),
                                    },
                                ],
                            },
                        },
                        Instruction {
                            id: InstructionId::new(2),
                            results: vec![done.clone()],
                            kind: InstructionKind::Compare {
                                predicate: ComparePredicate::Equal,
                                left: Operand::Value(count.id),
                                right: integer(I8, 0),
                            },
                        },
                    ],
                    terminator: Terminator::Branch {
                        condition: Operand::Value(done.id),
                        then_block: BlockId::new(3),
                        else_block: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(2),
                    instructions: vec![
                        Instruction {
                            id: InstructionId::new(3),
                            results: vec![next_sum.clone()],
                            kind: InstructionKind::Binary {
                                op: BinaryOp::Add,
                                left: Operand::Value(sum.id),
                                right: integer(I8, 10),
                            },
                        },
                        Instruction {
                            id: InstructionId::new(4),
                            results: vec![next_count.clone()],
                            kind: InstructionKind::Binary {
                                op: BinaryOp::Subtract,
                                left: Operand::Value(count.id),
                                right: integer(I8, 1),
                            },
                        },
                    ],
                    terminator: Terminator::Jump(BlockId::new(1)),
                },
                Block {
                    id: BlockId::new(3),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(Some(Operand::Value(sum.id))),
                },
            ],
        ));

        let result = interpret(
            &test,
            FunctionId::new(0),
            &[RuntimeInteger::from_unsigned(8, 2).unwrap()],
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.as_unsigned(), 14);
    }

    #[test]
    fn signed_and_unsigned_operations_observe_the_same_bits_differently() {
        let signed = Value {
            id: ValueId::new(0),
            type_id: I8,
        };
        let unsigned = Value {
            id: ValueId::new(1),
            type_id: I8,
        };
        let difference = Value {
            id: ValueId::new(2),
            type_id: I8,
        };
        let test = module(function(
            Vec::new(),
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: InstructionId::new(0),
                        results: vec![signed.clone()],
                        kind: InstructionKind::Binary {
                            op: BinaryOp::SignedDivide,
                            left: integer(I8, -2),
                            right: integer(I8, 2),
                        },
                    },
                    Instruction {
                        id: InstructionId::new(1),
                        results: vec![unsigned.clone()],
                        kind: InstructionKind::Binary {
                            op: BinaryOp::UnsignedDivide,
                            left: integer(I8, -2),
                            right: integer(I8, 2),
                        },
                    },
                    Instruction {
                        id: InstructionId::new(2),
                        results: vec![difference.clone()],
                        kind: InstructionKind::Binary {
                            op: BinaryOp::Xor,
                            left: Operand::Value(signed.id),
                            right: Operand::Value(unsigned.id),
                        },
                    },
                ],
                terminator: Terminator::Return(Some(Operand::Value(difference.id))),
            }],
        ));

        let result = interpret(&test, FunctionId::new(0), &[]).unwrap().unwrap();

        assert_eq!(result.as_unsigned(), 128);
        assert_eq!(result.as_signed(), -128);
    }

    #[test]
    fn division_by_zero_is_an_explicit_trap() {
        let quotient = Value {
            id: ValueId::new(0),
            type_id: I8,
        };
        let test = module(function(
            Vec::new(),
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: InstructionId::new(7),
                    results: vec![quotient.clone()],
                    kind: InstructionKind::Binary {
                        op: BinaryOp::SignedDivide,
                        left: integer(I8, 1),
                        right: integer(I8, 0),
                    },
                }],
                terminator: Terminator::Return(Some(Operand::Value(quotient.id))),
            }],
        ));

        assert_eq!(
            interpret(&test, FunctionId::new(0), &[]),
            Err(InterpretError::DivideByZero(InstructionId::new(7)))
        );
    }
}
