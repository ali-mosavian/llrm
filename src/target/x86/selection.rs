//! Initial exact portable-IR to x86 Machine IR selection.
//!
//! This selector intentionally handles only side-effect-free integer
//! expressions and unconditional control flow.  Unsupported IR is refused at
//! the boundary instead of being approximated or silently discarded.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    InstructionFlags, MachineBlock, MachineBlockId, MachineFunction, MachineFunctionId,
    MachineInstruction, MachineInstructionError, MachineInstructionId, MachineModule,
    MachineOperand, MachineOperandKind, MachineRegister, OperandRole, RegisterClass,
    VirtualRegister, VirtualRegisterId,
};
use crate::ir::{
    BinaryOp, Block, BlockId, CallingConvention, Constant, Function, FunctionId, GlobalId,
    Instruction, InstructionKind, Linkage, Module, Operand, Terminator, TypeId, TypeKind,
    TypedConstant, UnaryOp, Value, ValueId,
};

use super::{X86Opcode, X86RegisterClass};

/// A refusal while selecting portable IR into the currently supported x86 subset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionError {
    DuplicateType {
        type_id: TypeId,
    },
    MissingType {
        type_id: TypeId,
    },
    UnsupportedType {
        type_id: TypeId,
    },
    UnsupportedIntegerWidth {
        type_id: TypeId,
        bits: u16,
    },
    UnsupportedGlobal {
        global: GlobalId,
    },
    DuplicateFunction {
        function: FunctionId,
    },
    ExternalDeclaration {
        function: FunctionId,
    },
    UnsupportedFunctionProperty {
        function: FunctionId,
        property: FunctionProperty,
    },
    DuplicateBlock {
        function: FunctionId,
        block: BlockId,
    },
    UnknownJumpTarget {
        function: FunctionId,
        block: BlockId,
        target: BlockId,
    },
    DuplicateValue {
        function: FunctionId,
        value: ValueId,
    },
    UnknownValue {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        value: ValueId,
    },
    SignatureParameterCount {
        function: FunctionId,
        signature: usize,
        values: usize,
    },
    SignatureParameterTypeMismatch {
        function: FunctionId,
        parameter: usize,
        signature: TypeId,
        value: TypeId,
    },
    InvalidResultCount {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        count: usize,
    },
    OperandTypeMismatch {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        expected: TypeId,
        actual: TypeId,
    },
    UnsupportedConstant {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedUnary {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedBinary {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedInstruction {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedTerminator {
        function: FunctionId,
        block: BlockId,
    },
    UnsupportedReturnValue {
        function: FunctionId,
        block: BlockId,
    },
    ReturnWithoutValueForNonVoid {
        function: FunctionId,
        block: BlockId,
        result: TypeId,
    },
    MachineInstruction {
        function: FunctionId,
        error: MachineInstructionError,
    },
    IdExhausted {
        function: FunctionId,
    },
}

/// A function-level contract not yet represented in Machine IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionProperty {
    Variadic,
    CallingConvention(CallingConvention),
    Linkage(Linkage),
    Attributes,
}

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateType { type_id } => write!(formatter, "duplicate IR type {type_id}"),
            Self::MissingType { type_id } => write!(formatter, "missing IR type {type_id}"),
            Self::UnsupportedType { type_id } => write!(formatter, "unsupported IR type {type_id}"),
            Self::UnsupportedIntegerWidth { type_id, bits } => {
                write!(
                    formatter,
                    "unsupported integer type {type_id} with width {bits}"
                )
            }
            Self::UnsupportedGlobal { global } => write!(formatter, "unsupported global {global}"),
            Self::DuplicateFunction { function } => {
                write!(formatter, "duplicate IR function {function}")
            }
            Self::ExternalDeclaration { function } => {
                write!(
                    formatter,
                    "external declaration {function} has no body to select"
                )
            }
            Self::UnsupportedFunctionProperty { function, property } => write!(
                formatter,
                "function {function} has unsupported property {property:?}"
            ),
            Self::DuplicateBlock { function, block } => {
                write!(formatter, "function {function} has duplicate block {block}")
            }
            Self::UnknownJumpTarget {
                function,
                block,
                target,
            } => write!(
                formatter,
                "function {function} block {block} jumps to unknown block {target}"
            ),
            Self::DuplicateValue { function, value } => {
                write!(formatter, "function {function} has duplicate value {value}")
            }
            Self::UnknownValue {
                function,
                block,
                instruction,
                value,
            } => write!(
                formatter,
                "function {function} block {block} {instruction} references unknown value {value}"
            ),
            Self::SignatureParameterCount {
                function,
                signature,
                values,
            } => write!(
                formatter,
                "function {function} signature has {signature} parameters but body has {values}"
            ),
            Self::SignatureParameterTypeMismatch {
                function,
                parameter,
                signature,
                value,
            } => write!(
                formatter,
                "function {function} parameter {parameter} has type {value}, expected {signature}"
            ),
            Self::InvalidResultCount {
                function,
                block,
                instruction,
                count,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} has {count} results, expected one"
            ),
            Self::OperandTypeMismatch {
                function,
                block,
                instruction,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} uses type {actual}, expected {expected}"
            ),
            Self::UnsupportedConstant {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} has an unsupported constant"
            ),
            Self::UnsupportedUnary {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} has an unsupported unary operation"
            ),
            Self::UnsupportedBinary {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} has an unsupported binary operation"
            ),
            Self::UnsupportedInstruction {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} is unsupported"
            ),
            Self::UnsupportedTerminator { function, block } => {
                write!(
                    formatter,
                    "function {function} block {block} has an unsupported terminator"
                )
            }
            Self::UnsupportedReturnValue { function, block } => write!(
                formatter,
                "function {function} block {block} returns a value, which is unsupported"
            ),
            Self::ReturnWithoutValueForNonVoid {
                function,
                block,
                result,
            } => write!(
                formatter,
                "function {function} block {block} returns no value for result type {result}"
            ),
            Self::MachineInstruction { function, error } => {
                write!(
                    formatter,
                    "function {function} produced invalid Machine IR: {error}"
                )
            }
            Self::IdExhausted { function } => {
                write!(formatter, "function {function} exhausted Machine IR IDs")
            }
        }
    }
}

impl Error for SelectionError {}

/// Selects the initial integer/control-flow x86 Machine IR subset.
pub fn select_module(module: &Module) -> Result<MachineModule, SelectionError> {
    if let Some(global) = module.globals.first() {
        return Err(SelectionError::UnsupportedGlobal { global: global.id });
    }

    let types = collect_types(module)?;
    let mut function_ids = BTreeSet::new();
    let mut functions = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        if !function_ids.insert(function.id) {
            return Err(SelectionError::DuplicateFunction {
                function: function.id,
            });
        }
        functions.push(select_function(function, &types)?);
    }

    Ok(MachineModule { functions })
}

fn collect_types(module: &Module) -> Result<BTreeMap<TypeId, &TypeKind>, SelectionError> {
    let mut types = BTreeMap::new();
    for ty in &module.types {
        if types.insert(ty.id, &ty.kind).is_some() {
            return Err(SelectionError::DuplicateType { type_id: ty.id });
        }
    }
    Ok(types)
}

fn select_function(
    function: &Function,
    types: &BTreeMap<TypeId, &TypeKind>,
) -> Result<MachineFunction, SelectionError> {
    if function.blocks.is_empty() {
        return Err(SelectionError::ExternalDeclaration {
            function: function.id,
        });
    }
    if function.signature.variadic {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::Variadic,
        });
    }
    if function.signature.calling_convention != CallingConvention::Basic {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::CallingConvention(function.signature.calling_convention),
        });
    }
    if function.linkage != Linkage::Internal {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::Linkage(function.linkage),
        });
    }
    if !function.attributes.is_empty() {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::Attributes,
        });
    }
    if function.parameters.len() != function.signature.parameters.len() {
        return Err(SelectionError::SignatureParameterCount {
            function: function.id,
            signature: function.signature.parameters.len(),
            values: function.parameters.len(),
        });
    }

    let block_ids = collect_blocks(function)?;
    validate_value_ids(function)?;
    let mut selector = FunctionSelector::new(function, types, block_ids);
    selector.select_parameters()?;
    for block in &function.blocks {
        selector.select_block(block)?;
    }
    Ok(selector.finish())
}

fn validate_value_ids(function: &Function) -> Result<(), SelectionError> {
    let mut values = BTreeSet::new();
    for parameter in &function.parameters {
        if !values.insert(parameter.id) {
            return Err(SelectionError::DuplicateValue {
                function: function.id,
                value: parameter.id,
            });
        }
    }
    for block in &function.blocks {
        for instruction in &block.instructions {
            for result in &instruction.results {
                if !values.insert(result.id) {
                    return Err(SelectionError::DuplicateValue {
                        function: function.id,
                        value: result.id,
                    });
                }
            }
        }
    }
    Ok(())
}

fn collect_blocks(function: &Function) -> Result<BTreeSet<BlockId>, SelectionError> {
    let mut block_ids = BTreeSet::new();
    for block in &function.blocks {
        if !block_ids.insert(block.id) {
            return Err(SelectionError::DuplicateBlock {
                function: function.id,
                block: block.id,
            });
        }
    }
    Ok(block_ids)
}

#[derive(Clone, Copy)]
struct SelectedValue {
    register: VirtualRegisterId,
    type_id: TypeId,
}

struct FunctionSelector<'types> {
    function: &'types Function,
    types: &'types BTreeMap<TypeId, &'types TypeKind>,
    block_ids: BTreeSet<BlockId>,
    values: BTreeMap<ValueId, SelectedValue>,
    virtual_registers: Vec<VirtualRegister>,
    blocks: Vec<MachineBlock>,
    next_virtual_register: u32,
    next_instruction: u32,
}

impl<'types> FunctionSelector<'types> {
    fn new(
        function: &'types Function,
        types: &'types BTreeMap<TypeId, &'types TypeKind>,
        block_ids: BTreeSet<BlockId>,
    ) -> Self {
        Self {
            function,
            types,
            block_ids,
            values: BTreeMap::new(),
            virtual_registers: Vec::new(),
            blocks: Vec::with_capacity(function.blocks.len()),
            next_virtual_register: 0,
            next_instruction: 0,
        }
    }

    fn select_parameters(&mut self) -> Result<(), SelectionError> {
        for (index, parameter) in self.function.parameters.iter().enumerate() {
            let signature_type = self.function.signature.parameters[index];
            if parameter.type_id != signature_type {
                return Err(SelectionError::SignatureParameterTypeMismatch {
                    function: self.function.id,
                    parameter: index,
                    signature: signature_type,
                    value: parameter.type_id,
                });
            }
            self.define_value(parameter)?;
        }
        Ok(())
    }

    fn select_block(&mut self, block: &Block) -> Result<(), SelectionError> {
        let mut instructions = Vec::new();
        for instruction in &block.instructions {
            self.select_instruction(block.id, instruction, &mut instructions)?;
        }
        let (terminator, successors) = self.select_terminator(block)?;
        instructions.push(terminator);
        self.blocks.push(MachineBlock {
            id: MachineBlockId::new(block.id.get()),
            instructions,
            successors,
        });
        Ok(())
    }

    fn select_instruction(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        match &instruction.kind {
            InstructionKind::Unary { op, operand } => {
                let opcode = match op {
                    UnaryOp::Negate => X86Opcode::Neg,
                    UnaryOp::Not => X86Opcode::Not,
                    UnaryOp::FloatNegate | UnaryOp::FloatAbsolute => {
                        return Err(SelectionError::UnsupportedUnary {
                            function: self.function.id,
                            block,
                            instruction: instruction.id,
                        });
                    }
                };
                let result = self.result_definition(block, instruction)?;
                let source =
                    self.select_operand(block, instruction.id, operand, result.type_id, output)?;
                let result = self.define_value(result)?;
                self.copy(result.register, source.register, output)?;
                self.push_instruction(
                    opcode,
                    vec![virtual_operand(result.register, OperandRole::UseDef)],
                    InstructionFlags::NONE,
                    output,
                )?;
            }
            InstructionKind::Binary { op, left, right } => {
                let opcode = match op {
                    BinaryOp::Add => X86Opcode::Add,
                    BinaryOp::Subtract => X86Opcode::Sub,
                    BinaryOp::Multiply => X86Opcode::Imul,
                    BinaryOp::And => X86Opcode::And,
                    BinaryOp::Or => X86Opcode::Or,
                    BinaryOp::Xor => X86Opcode::Xor,
                    BinaryOp::SignedDivide
                    | BinaryOp::UnsignedDivide
                    | BinaryOp::SignedRemainder
                    | BinaryOp::UnsignedRemainder
                    | BinaryOp::ShiftLeft
                    | BinaryOp::LogicalShiftRight
                    | BinaryOp::ArithmeticShiftRight
                    | BinaryOp::FloatAdd
                    | BinaryOp::FloatSubtract
                    | BinaryOp::FloatMultiply
                    | BinaryOp::FloatDivide => {
                        return Err(SelectionError::UnsupportedBinary {
                            function: self.function.id,
                            block,
                            instruction: instruction.id,
                        });
                    }
                };
                let result = self.result_definition(block, instruction)?;
                let left =
                    self.select_operand(block, instruction.id, left, result.type_id, output)?;
                let right =
                    self.select_operand(block, instruction.id, right, result.type_id, output)?;
                let result = self.define_value(result)?;
                self.copy(result.register, left.register, output)?;
                self.push_instruction(
                    opcode,
                    vec![
                        virtual_operand(result.register, OperandRole::UseDef),
                        virtual_operand(right.register, OperandRole::Use),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
            }
            InstructionKind::Phi { .. }
            | InstructionKind::Compare { .. }
            | InstructionKind::Cast { .. }
            | InstructionKind::Load { .. }
            | InstructionKind::Store { .. }
            | InstructionKind::GetElementPointer { .. }
            | InstructionKind::Select { .. }
            | InstructionKind::Call { .. }
            | InstructionKind::Intrinsic { .. } => {
                return Err(SelectionError::UnsupportedInstruction {
                    function: self.function.id,
                    block,
                    instruction: instruction.id,
                });
            }
        }
        Ok(())
    }

    fn result_definition<'instruction>(
        &self,
        block: BlockId,
        instruction: &'instruction Instruction,
    ) -> Result<&'instruction Value, SelectionError> {
        let [result] = instruction.results.as_slice() else {
            return Err(SelectionError::InvalidResultCount {
                function: self.function.id,
                block,
                instruction: instruction.id,
                count: instruction.results.len(),
            });
        };
        Ok(result)
    }

    fn select_operand(
        &mut self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        operand: &Operand,
        expected_type: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<SelectedValue, SelectionError> {
        match operand {
            Operand::Value(value) => {
                let Some(selected) = self.values.get(value).copied() else {
                    return Err(SelectionError::UnknownValue {
                        function: self.function.id,
                        block,
                        instruction,
                        value: *value,
                    });
                };
                self.require_operand_type(block, instruction, expected_type, selected.type_id)?;
                Ok(selected)
            }
            Operand::Constant(constant) => {
                self.require_operand_type(block, instruction, expected_type, constant.type_id)?;
                self.materialize_integer(block, instruction, constant, output)
            }
        }
    }

    fn materialize_integer(
        &mut self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        constant: &TypedConstant,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<SelectedValue, SelectionError> {
        let Constant::Integer(value) = &constant.value else {
            return Err(SelectionError::UnsupportedConstant {
                function: self.function.id,
                block,
                instruction,
            });
        };
        let class = self.integer_class(constant.type_id)?;
        let bits = self.integer_bits(constant.type_id)?;
        let register = self.fresh_virtual_register(class)?;
        self.push_instruction(
            X86Opcode::Mov,
            vec![
                virtual_operand(register, OperandRole::Def),
                immediate_operand(integer_immediate(*value, bits)),
            ],
            InstructionFlags::NONE,
            output,
        )?;
        Ok(SelectedValue {
            register,
            type_id: constant.type_id,
        })
    }

    fn select_terminator(
        &mut self,
        block: &Block,
    ) -> Result<(MachineInstruction, Vec<MachineBlockId>), SelectionError> {
        match &block.terminator {
            Terminator::Jump(target) => {
                if !self.block_ids.contains(target) {
                    return Err(SelectionError::UnknownJumpTarget {
                        function: self.function.id,
                        block: block.id,
                        target: *target,
                    });
                }
                let target = MachineBlockId::new(target.get());
                let instruction = self.machine_instruction(
                    X86Opcode::Jump,
                    vec![block_operand(target)],
                    InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                )?;
                Ok((instruction, vec![target]))
            }
            Terminator::Return(Some(_)) => Err(SelectionError::UnsupportedReturnValue {
                function: self.function.id,
                block: block.id,
            }),
            Terminator::Return(None) => {
                if !matches!(
                    self.type_kind(self.function.signature.result)?,
                    TypeKind::Void
                ) {
                    return Err(SelectionError::ReturnWithoutValueForNonVoid {
                        function: self.function.id,
                        block: block.id,
                        result: self.function.signature.result,
                    });
                }
                let instruction = self.machine_instruction(
                    X86Opcode::ReturnNear,
                    Vec::new(),
                    InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                )?;
                Ok((instruction, Vec::new()))
            }
            Terminator::Branch { .. } | Terminator::Switch { .. } | Terminator::Unreachable => {
                Err(SelectionError::UnsupportedTerminator {
                    function: self.function.id,
                    block: block.id,
                })
            }
        }
    }

    fn copy(
        &mut self,
        destination: VirtualRegisterId,
        source: VirtualRegisterId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        self.push_instruction(
            X86Opcode::Copy,
            vec![
                virtual_operand(destination, OperandRole::Def),
                virtual_operand(source, OperandRole::Use),
            ],
            InstructionFlags {
                copy: true,
                ..InstructionFlags::NONE
            },
            output,
        )
    }

    fn push_instruction(
        &mut self,
        opcode: X86Opcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        output.push(self.machine_instruction(opcode, operands, flags)?);
        Ok(())
    }

    fn machine_instruction(
        &mut self,
        opcode: X86Opcode,
        operands: Vec<MachineOperand>,
        flags: InstructionFlags,
    ) -> Result<MachineInstruction, SelectionError> {
        let id = MachineInstructionId::new(Self::fresh_id(
            self.function.id,
            &mut self.next_instruction,
        )?);
        MachineInstruction::new(id, opcode.machine_opcode(), operands, flags).map_err(|error| {
            SelectionError::MachineInstruction {
                function: self.function.id,
                error,
            }
        })
    }

    fn define_value(&mut self, value: &Value) -> Result<SelectedValue, SelectionError> {
        if self.values.contains_key(&value.id) {
            return Err(SelectionError::DuplicateValue {
                function: self.function.id,
                value: value.id,
            });
        }
        let class = self.integer_class(value.type_id)?;
        let selected = SelectedValue {
            register: self.fresh_virtual_register(class)?,
            type_id: value.type_id,
        };
        self.values.insert(value.id, selected);
        Ok(selected)
    }

    fn fresh_virtual_register(
        &mut self,
        class: RegisterClass,
    ) -> Result<VirtualRegisterId, SelectionError> {
        let register = VirtualRegisterId::new(Self::fresh_id(
            self.function.id,
            &mut self.next_virtual_register,
        )?);
        self.virtual_registers.push(VirtualRegister {
            id: register,
            class,
        });
        Ok(register)
    }

    fn fresh_id(function: FunctionId, next: &mut u32) -> Result<u32, SelectionError> {
        let id = *next;
        *next = next
            .checked_add(1)
            .ok_or(SelectionError::IdExhausted { function })?;
        Ok(id)
    }

    fn integer_class(&self, type_id: TypeId) -> Result<RegisterClass, SelectionError> {
        match self.integer_bits(type_id)? {
            8 => Ok(X86RegisterClass::Byte.machine_class()),
            16 => Ok(X86RegisterClass::Word.machine_class()),
            32 => Ok(X86RegisterClass::Dword.machine_class()),
            bits => Err(SelectionError::UnsupportedIntegerWidth { type_id, bits }),
        }
    }

    fn integer_bits(&self, type_id: TypeId) -> Result<u16, SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Integer {
                bits: bits @ (8 | 16 | 32),
            } => Ok(*bits),
            TypeKind::Integer { bits } => Err(SelectionError::UnsupportedIntegerWidth {
                type_id,
                bits: *bits,
            }),
            TypeKind::Void
            | TypeKind::Float(_)
            | TypeKind::Pointer { .. }
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. } => Err(SelectionError::UnsupportedType { type_id }),
        }
    }

    fn type_kind(&self, type_id: TypeId) -> Result<&TypeKind, SelectionError> {
        self.types
            .get(&type_id)
            .copied()
            .ok_or(SelectionError::MissingType { type_id })
    }

    fn require_operand_type(
        &self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        expected: TypeId,
        actual: TypeId,
    ) -> Result<(), SelectionError> {
        if expected == actual {
            Ok(())
        } else {
            Err(SelectionError::OperandTypeMismatch {
                function: self.function.id,
                block,
                instruction,
                expected,
                actual,
            })
        }
    }

    fn finish(self) -> MachineFunction {
        MachineFunction {
            id: MachineFunctionId::new(self.function.id.get()),
            name: self.function.name.clone(),
            virtual_registers: self.virtual_registers,
            blocks: self.blocks,
            frame_objects: Vec::new(),
        }
    }
}

fn virtual_operand(register: VirtualRegisterId, role: OperandRole) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
        role,
        constraint: None,
        tied_to: None,
    }
}

fn immediate_operand(value: i64) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Immediate(value),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn block_operand(block: MachineBlockId) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Block(block),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn integer_immediate(value: i128, bits: u16) -> i64 {
    let modulus = 1_i128 << bits;
    let truncated = value.rem_euclid(modulus);
    let signed = if truncated >= modulus / 2 {
        truncated - modulus
    } else {
        truncated
    };
    signed as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::machine::MachineRegister;
    use crate::ir::{CallingConvention, FloatKind, Signature, Type};

    const VOID: TypeId = TypeId::new(0);
    const I32: TypeId = TypeId::new(1);

    fn signature(result: TypeId, parameters: Vec<TypeId>) -> crate::ir::Signature {
        Signature {
            result,
            parameters,
            variadic: false,
            calling_convention: CallingConvention::Basic,
        }
    }

    fn function(blocks: Vec<Block>, parameters: Vec<Value>) -> Function {
        Function {
            id: FunctionId::new(4),
            name: "selected".to_owned(),
            signature: signature(VOID, parameters.iter().map(|value| value.type_id).collect()),
            linkage: crate::ir::Linkage::Internal,
            attributes: Vec::new(),
            parameters,
            blocks,
        }
    }

    fn module(types: Vec<Type>, functions: Vec<Function>) -> Module {
        Module {
            name: "selection".to_owned(),
            types,
            globals: Vec::new(),
            functions,
        }
    }

    fn basic_types() -> Vec<Type> {
        vec![
            Type {
                id: VOID,
                kind: TypeKind::Void,
            },
            Type {
                id: I32,
                kind: TypeKind::Integer { bits: 32 },
            },
        ]
    }

    #[test]
    fn selects_integer_expressions_constants_and_a_void_jump_return() {
        let parameter = Value {
            id: ValueId::new(7),
            type_id: I32,
        };
        let selected = module(
            basic_types(),
            vec![function(
                vec![
                    Block {
                        id: BlockId::new(2),
                        instructions: vec![
                            Instruction {
                                id: crate::ir::InstructionId::new(3),
                                results: vec![Value {
                                    id: ValueId::new(8),
                                    type_id: I32,
                                }],
                                kind: InstructionKind::Binary {
                                    op: BinaryOp::Add,
                                    left: Operand::Value(ValueId::new(7)),
                                    right: Operand::Constant(TypedConstant {
                                        type_id: I32,
                                        value: Constant::Integer(5),
                                    }),
                                },
                            },
                            Instruction {
                                id: crate::ir::InstructionId::new(4),
                                results: vec![Value {
                                    id: ValueId::new(9),
                                    type_id: I32,
                                }],
                                kind: InstructionKind::Unary {
                                    op: UnaryOp::Not,
                                    operand: Operand::Value(ValueId::new(8)),
                                },
                            },
                        ],
                        terminator: Terminator::Jump(BlockId::new(3)),
                    },
                    Block {
                        id: BlockId::new(3),
                        instructions: Vec::new(),
                        terminator: Terminator::Return(None),
                    },
                ],
                vec![parameter],
            )],
        );

        let selected = select_module(&selected).expect("subset IR selects exactly");
        selected.verify().expect("selected Machine IR verifies");
        let function = &selected.functions[0];
        assert_eq!(function.id, MachineFunctionId::new(4));
        assert_eq!(function.virtual_registers.len(), 4);
        assert_eq!(function.blocks[0].successors, vec![MachineBlockId::new(3)]);
        assert_eq!(function.blocks[0].instructions.len(), 6);
        assert_eq!(
            function.blocks[0].instructions[0].opcode,
            X86Opcode::Mov.machine_opcode()
        );
        assert!(matches!(
            function.blocks[0].instructions[0].operands[0].kind,
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if register == VirtualRegisterId::new(1)
        ));
        assert_eq!(
            function.blocks[0].instructions[1].opcode,
            X86Opcode::Copy.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[2].opcode,
            X86Opcode::Add.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[3].opcode,
            X86Opcode::Copy.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[4].opcode,
            X86Opcode::Not.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[5].opcode,
            X86Opcode::Jump.machine_opcode()
        );
        assert_eq!(
            function.blocks[1].instructions[0].opcode,
            X86Opcode::ReturnNear.machine_opcode()
        );
    }

    #[test]
    fn refuses_a_nonvoid_return() {
        let mut function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Constant(TypedConstant {
                    type_id: I32,
                    value: Constant::Integer(1),
                }))),
            }],
            Vec::new(),
        );
        function.signature.result = I32;

        let error = select_module(&module(basic_types(), vec![function]))
            .expect_err("return values are outside the first selector slice");
        assert!(matches!(
            error,
            SelectionError::UnsupportedReturnValue { .. }
        ));
    }

    #[test]
    fn refuses_i1_and_float_values() {
        let i1 = TypeId::new(2);
        let float = TypeId::new(3);
        let mut types = basic_types();
        types.push(Type {
            id: i1,
            kind: TypeKind::Integer { bits: 1 },
        });
        types.push(Type {
            id: float,
            kind: TypeKind::Float(FloatKind::Binary32),
        });

        let i1_function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
            vec![Value {
                id: ValueId::new(0),
                type_id: i1,
            }],
        );
        let i1_error = select_module(&module(types.clone(), vec![i1_function]))
            .expect_err("i1 has no integer x86 register class in this slice");
        assert!(matches!(
            i1_error,
            SelectionError::UnsupportedIntegerWidth { type_id, bits: 1 } if type_id == i1
        ));

        let float_function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
            vec![Value {
                id: ValueId::new(0),
                type_id: float,
            }],
        );
        let float_error = select_module(&module(types, vec![float_function]))
            .expect_err("floating values are outside the integer selector");
        assert!(matches!(
            float_error,
            SelectionError::UnsupportedType { type_id } if type_id == float
        ));
    }

    #[test]
    fn refuses_a_self_referential_ssa_result() {
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(0),
                        type_id: I32,
                    }],
                    kind: InstructionKind::Unary {
                        op: UnaryOp::Not,
                        operand: Operand::Value(ValueId::new(0)),
                    },
                }],
                terminator: Terminator::Return(None),
            }],
            Vec::new(),
        );

        assert!(matches!(
            select_module(&module(basic_types(), vec![function])),
            Err(SelectionError::UnknownValue { value, .. }) if value == ValueId::new(0)
        ));
    }

    #[test]
    fn refuses_unrepresented_function_properties() {
        let mut function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
            Vec::new(),
        );
        function.signature.variadic = true;

        assert_eq!(
            select_module(&module(basic_types(), vec![function])),
            Err(SelectionError::UnsupportedFunctionProperty {
                function: FunctionId::new(4),
                property: FunctionProperty::Variadic,
            })
        );
    }
}
