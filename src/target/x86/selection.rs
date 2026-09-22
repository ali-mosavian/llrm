//! Exact portable-IR to x86 Machine IR selection for the supported scalar
//! integer, floating, control-flow, and direct-call surface. Unsupported IR
//! is refused at the boundary instead of being approximated or silently
//! discarded.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::codegen::machine::{
    FrameIndex, FrameObject, FrameObjectKind, InstructionFlags, MachineAddressSpace, MachineBlock,
    MachineBlockId, MachineCallingConvention, MachineDataObject, MachineDataObjectId,
    MachineDataRelocation, MachineFloatKind, MachineFunction, MachineFunctionId,
    MachineInstruction, MachineInstructionError, MachineInstructionId, MachineLinkage,
    MachineModule, MachineOperand, MachineOperandKind, MachineRegister, MachineSignature,
    MachineValueType, OperandRole, RegisterClass, RegisterConstraint, VirtualRegister,
    VirtualRegisterId,
};
use crate::ir::{
    AddressSpace, BinaryOp, Block, BlockId, Callee, CallingConvention, CastOp, ComparePredicate,
    Constant, Effects, FloatKind, FloatRounding, Function, FunctionId, Global, GlobalId,
    Instruction, InstructionKind, Linkage, MemoryEffects, Module, Operand, Terminator, TypeId,
    TypeKind, TypedConstant, UnaryOp, Value, ValueId,
};

use super::{ConditionCode, X86Opcode, X86Register, X86RegisterClass, X87MemoryFormat};

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
    UnsupportedGlobalInitializer {
        global: GlobalId,
    },
    UnknownDataRelocationTarget {
        global: GlobalId,
        target: GlobalId,
    },
    UnknownGlobal {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        global: GlobalId,
    },
    UnsupportedAddressSpace {
        type_id: TypeId,
        address_space: AddressSpace,
    },
    DuplicateFunction {
        function: FunctionId,
    },
    UnknownCallee {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        callee: FunctionId,
    },
    IndirectCallee {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedCallTarget {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        callee: FunctionId,
    },
    UnsupportedCallResult {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        callee: FunctionId,
        result: TypeId,
        values: usize,
    },
    UnsupportedExternalResult {
        function: FunctionId,
        result: TypeId,
    },
    CallArgumentCount {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        callee: FunctionId,
        expected: usize,
        actual: usize,
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
    ParameterAddressOutOfBounds {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        parameter: u32,
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
    UnsupportedFloatConstant {
        type_id: TypeId,
        value: String,
    },
    DataObjectIdExhausted,
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
    UnsupportedCast {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedInstruction {
        function: FunctionId,
        block: BlockId,
        instruction: crate::ir::InstructionId,
    },
    UnsupportedCompare {
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
            Self::UnsupportedGlobalInitializer { global } => {
                write!(formatter, "global {global} has an unsupported initializer")
            }
            Self::UnknownDataRelocationTarget { global, target } => write!(
                formatter,
                "global {global} has a relocation targeting unknown global {target}"
            ),
            Self::UnknownGlobal {
                function,
                block,
                instruction,
                global,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} references unknown global {global}"
            ),
            Self::UnsupportedAddressSpace {
                type_id,
                address_space,
            } => write!(
                formatter,
                "type {type_id} uses unsupported address space {address_space:?}"
            ),
            Self::DuplicateFunction { function } => {
                write!(formatter, "duplicate IR function {function}")
            }
            Self::UnknownCallee {
                function,
                block,
                instruction,
                callee,
            } => write!(
                formatter,
                "function {function} block {block} {instruction} calls unknown function {callee}"
            ),
            Self::IndirectCallee {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} {instruction} has an unsupported indirect callee"
            ),
            Self::UnsupportedCallTarget {
                function,
                block,
                instruction,
                callee,
            } => write!(
                formatter,
                "function {function} block {block} {instruction} calls unsupported target {callee}"
            ),
            Self::UnsupportedCallResult {
                function,
                block,
                instruction,
                callee,
                result,
                values,
            } => write!(
                formatter,
                "function {function} block {block} {instruction} call to {callee} has result type {result} and {values} result values, but only void calls without results are supported"
            ),
            Self::UnsupportedExternalResult { function, result } => write!(
                formatter,
                "external far-Pascal declaration {function} has result type {result}, but only void calls are supported"
            ),
            Self::CallArgumentCount {
                function,
                block,
                instruction,
                callee,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} block {block} {instruction} call to {callee} has {actual} arguments, expected {expected}"
            ),
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
            Self::ParameterAddressOutOfBounds {
                function,
                block,
                instruction,
                parameter,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} references missing parameter {parameter}"
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
            Self::UnsupportedFloatConstant { type_id, value } => write!(
                formatter,
                "floating constant {value:?} of type {type_id} has no exact x86 storage form"
            ),
            Self::DataObjectIdExhausted => {
                formatter.write_str("floating constant pool exhausted Machine IR data IDs")
            }
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
            Self::UnsupportedCast {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} has an unsupported cast"
            ),
            Self::UnsupportedInstruction {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} is unsupported"
            ),
            Self::UnsupportedCompare {
                function,
                block,
                instruction,
            } => write!(
                formatter,
                "function {function} block {block} comparison {instruction} is unsupported"
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

/// Selects the supported scalar x86 Machine IR subset.
pub fn select_module(module: &Module) -> Result<MachineModule, SelectionError> {
    let types = collect_types(module)?;
    let globals = collect_globals(module)?;
    let mut data_objects = module
        .globals
        .iter()
        .map(|global| select_data_object(global, &types, &globals))
        .collect::<Result<Vec<_>, _>>()?;
    let (float_data, float_constants) = select_float_constants(module, &types, &data_objects)?;
    data_objects.extend(float_data);
    let functions_by_id = collect_functions(module)?;
    for function in &module.functions {
        if function.blocks.is_empty() {
            validate_external_declaration(function, &types)?;
        }
    }
    let mut functions = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        if !function.blocks.is_empty() {
            functions.push(select_function(
                function,
                &types,
                &globals,
                &functions_by_id,
                &float_constants,
            )?);
        }
    }

    Ok(MachineModule {
        data_objects,
        functions,
    })
}

fn collect_globals(module: &Module) -> Result<BTreeMap<GlobalId, &Global>, SelectionError> {
    let mut globals = BTreeMap::new();
    for global in &module.globals {
        if globals.insert(global.id, global).is_some() {
            return Err(SelectionError::UnsupportedGlobal { global: global.id });
        }
    }
    Ok(globals)
}

fn select_data_object(
    global: &Global,
    types: &BTreeMap<TypeId, &TypeKind>,
    globals: &BTreeMap<GlobalId, &Global>,
) -> Result<MachineDataObject, SelectionError> {
    let Some(initializer) = &global.initializer else {
        return Err(SelectionError::UnsupportedGlobalInitializer { global: global.id });
    };
    let (bytes, relocations) = match initializer {
        Constant::Bytes(bytes) => (bytes, Vec::new()),
        Constant::RelocatableBytes { bytes, relocations } => (
            bytes,
            relocations
                .iter()
                .map(|relocation| select_data_relocation(global.id, relocation, globals))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => {
            return Err(SelectionError::UnsupportedGlobalInitializer { global: global.id });
        }
    };
    let TypeKind::Array { element, length } = type_kind(types, global.type_id)? else {
        return Err(SelectionError::UnsupportedGlobal { global: global.id });
    };
    if !matches!(type_kind(types, *element)?, TypeKind::Integer { bits: 8 })
        || usize::try_from(*length).ok() != Some(bytes.len())
    {
        return Err(SelectionError::UnsupportedGlobal { global: global.id });
    }
    Ok(MachineDataObject {
        id: MachineDataObjectId::new(global.id.get()),
        name: global.name.clone(),
        bytes: bytes.clone(),
        relocations,
        alignment: 1,
        constant: global.constant,
        linkage: machine_linkage(global.linkage),
        address_space: machine_data_address_space(global.address_space),
    })
}

fn select_data_relocation(
    global: GlobalId,
    relocation: &crate::ir::GlobalRelocation,
    globals: &BTreeMap<GlobalId, &Global>,
) -> Result<MachineDataRelocation, SelectionError> {
    if !globals.contains_key(&relocation.target) {
        return Err(SelectionError::UnknownDataRelocationTarget {
            global,
            target: relocation.target,
        });
    }
    Ok(MachineDataRelocation {
        offset: relocation.offset,
        target: MachineDataObjectId::new(relocation.target.get()),
        addend: relocation.addend,
        width: relocation.width,
        address_space: machine_data_address_space(relocation.address_space),
    })
}

fn select_float_constants(
    module: &Module,
    types: &BTreeMap<TypeId, &TypeKind>,
    existing: &[MachineDataObject],
) -> Result<
    (
        Vec<MachineDataObject>,
        BTreeMap<(TypeId, String), SelectedFloatConstant>,
    ),
    SelectionError,
> {
    let mut keys = BTreeSet::new();
    for function in &module.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let InstructionKind::Store { address, value, .. } = &instruction.kind {
                    if let Operand::Constant(TypedConstant {
                        type_id,
                        value: Constant::Float(_),
                    }) = value
                    {
                        if matches!(
                            type_kind(types, *type_id)?,
                            TypeKind::Float(FloatKind::Binary32)
                        ) {
                            collect_float_constant(address, &mut keys);
                            continue;
                        }
                    }
                }
                for operand in instruction_operands(&instruction.kind) {
                    collect_float_constant(operand, &mut keys);
                }
            }
            match &block.terminator {
                Terminator::Branch { condition, .. } => {
                    collect_float_constant(condition, &mut keys)
                }
                Terminator::Switch { selector, .. } => collect_float_constant(selector, &mut keys),
                Terminator::Return(Some(value)) => collect_float_constant(value, &mut keys),
                Terminator::Jump(_) | Terminator::Return(None) | Terminator::Unreachable => {}
            }
        }
    }

    let mut next_id =
        existing
            .iter()
            .map(|object| object.id.get())
            .max()
            .map_or(Ok(0), |last| {
                last.checked_add(1)
                    .ok_or(SelectionError::DataObjectIdExhausted)
            })?;
    let mut names = existing
        .iter()
        .map(|object| object.name.clone())
        .collect::<BTreeSet<_>>();
    let mut objects = Vec::with_capacity(keys.len());
    let mut selected = BTreeMap::new();
    for (ordinal, (type_id, value)) in keys.into_iter().enumerate() {
        let kind = match type_kind(types, type_id)? {
            TypeKind::Float(kind) => *kind,
            _ => {
                return Err(SelectionError::UnsupportedFloatConstant { type_id, value });
            }
        };
        if exact_x87_constant_opcode(type_id, kind, &value)?.is_some() {
            continue;
        }
        let (bytes, format) = float_constant_bytes(type_id, kind, &value)?;
        let stem = format!("__llrm_float_{ordinal}");
        let mut name = stem.clone();
        let mut suffix = 0_u32;
        while !names.insert(name.clone()) {
            suffix = suffix
                .checked_add(1)
                .ok_or(SelectionError::DataObjectIdExhausted)?;
            name = format!("{stem}_{suffix}");
        }
        objects.push(MachineDataObject {
            id: MachineDataObjectId::new(next_id),
            name: name.clone(),
            bytes,
            address_space: MachineAddressSpace::NearData,
            relocations: Vec::new(),
            alignment: 2,
            constant: true,
            linkage: MachineLinkage::Internal,
        });
        selected.insert((type_id, value), SelectedFloatConstant { name, format });
        next_id = next_id
            .checked_add(1)
            .ok_or(SelectionError::DataObjectIdExhausted)?;
    }
    Ok((objects, selected))
}

fn collect_float_constant(operand: &Operand, constants: &mut BTreeSet<(TypeId, String)>) {
    if let Operand::Constant(TypedConstant {
        type_id,
        value: Constant::Float(value),
    }) = operand
    {
        constants.insert((*type_id, value.clone()));
    }
}

fn instruction_operands(kind: &InstructionKind) -> Vec<&Operand> {
    match kind {
        InstructionKind::Phi { incoming } => incoming.iter().map(|entry| &entry.value).collect(),
        InstructionKind::StackAlloc { .. } | InstructionKind::ParameterAddress { .. } => Vec::new(),
        InstructionKind::Unary { operand, .. } | InstructionKind::Cast { operand, .. } => {
            vec![operand]
        }
        InstructionKind::Binary { left, right, .. }
        | InstructionKind::Compare { left, right, .. } => vec![left, right],
        InstructionKind::Load { address, .. } => vec![address],
        InstructionKind::Store { address, value, .. } => vec![address, value],
        InstructionKind::GetElementPointer { base, indices } => {
            std::iter::once(base).chain(indices).collect()
        }
        InstructionKind::ComposePointer { segment, offset } => vec![segment, offset],
        InstructionKind::Select {
            condition,
            then_value,
            else_value,
        } => vec![condition, then_value, else_value],
        InstructionKind::Call { callee, .. } => {
            // Direct floating call constants are transferred as their raw ABI
            // bits, so they need no x87 constant-pool cell.
            let mut operands =
                Vec::with_capacity(usize::from(matches!(callee, Callee::Indirect(_))));
            if let Callee::Indirect(operand) = callee {
                operands.push(operand);
            }
            operands
        }
        InstructionKind::Intrinsic { arguments, .. } => arguments.iter().collect(),
    }
}

fn function_uses_value(function: &Function, value: ValueId) -> bool {
    function.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            value_operands(&instruction.kind)
                .into_iter()
                .any(|operand| matches!(operand, Operand::Value(id) if *id == value))
        }) || terminator_operands(&block.terminator)
            .into_iter()
            .any(|operand| matches!(operand, Operand::Value(id) if *id == value))
    })
}

fn value_operands(kind: &InstructionKind) -> Vec<&Operand> {
    match kind {
        InstructionKind::Call {
            callee, arguments, ..
        } => {
            let mut operands = Vec::with_capacity(arguments.len() + 1);
            if let Callee::Indirect(callee) = callee {
                operands.push(callee);
            }
            operands.extend(arguments);
            operands
        }
        _ => instruction_operands(kind),
    }
}

fn terminator_operands(terminator: &Terminator) -> Vec<&Operand> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![condition],
        Terminator::Switch { selector, .. } => vec![selector],
        Terminator::Return(Some(value)) => vec![value],
        Terminator::Jump(_) | Terminator::Return(None) | Terminator::Unreachable => Vec::new(),
    }
}

fn float_constant_bytes(
    type_id: TypeId,
    kind: FloatKind,
    value: &str,
) -> Result<(Vec<u8>, X87MemoryFormat), SelectionError> {
    match kind {
        FloatKind::Binary32 => value
            .parse::<f32>()
            .map(|value| {
                (
                    value.to_bits().to_le_bytes().to_vec(),
                    X87MemoryFormat::Float32,
                )
            })
            .map_err(|_| SelectionError::UnsupportedFloatConstant {
                type_id,
                value: value.to_owned(),
            }),
        FloatKind::Binary64 => value
            .parse::<f64>()
            .map(|value| {
                (
                    value.to_bits().to_le_bytes().to_vec(),
                    X87MemoryFormat::Float64,
                )
            })
            .map_err(|_| SelectionError::UnsupportedFloatConstant {
                type_id,
                value: value.to_owned(),
            }),
        FloatKind::Extended80 => Err(SelectionError::UnsupportedFloatConstant {
            type_id,
            value: value.to_owned(),
        }),
    }
}

fn exact_x87_constant_opcode(
    type_id: TypeId,
    kind: FloatKind,
    value: &str,
) -> Result<Option<X86Opcode>, SelectionError> {
    let bits = match kind {
        FloatKind::Binary32 => u64::from(
            value
                .parse::<f32>()
                .map_err(|_| SelectionError::UnsupportedFloatConstant {
                    type_id,
                    value: value.to_owned(),
                })?
                .to_bits(),
        ),
        FloatKind::Binary64 => value
            .parse::<f64>()
            .map_err(|_| SelectionError::UnsupportedFloatConstant {
                type_id,
                value: value.to_owned(),
            })?
            .to_bits(),
        FloatKind::Extended80 => return Ok(None),
    };
    let one = match kind {
        FloatKind::Binary32 => u64::from(1.0_f32.to_bits()),
        FloatKind::Binary64 => 1.0_f64.to_bits(),
        FloatKind::Extended80 => unreachable!("extended constants returned above"),
    };
    Ok(match bits {
        0 => Some(X86Opcode::X87LoadZero),
        bits if bits == one => Some(X86Opcode::X87LoadOne),
        _ => None,
    })
}

fn machine_data_address_space(address_space: AddressSpace) -> MachineAddressSpace {
    match address_space {
        AddressSpace::Generic => MachineAddressSpace::Generic,
        AddressSpace::NearData => MachineAddressSpace::NearData,
        AddressSpace::FarData => MachineAddressSpace::FarData,
        AddressSpace::HugeData => MachineAddressSpace::HugeData,
        AddressSpace::Code => MachineAddressSpace::Code,
        AddressSpace::Segment => MachineAddressSpace::Segment,
    }
}

fn collect_functions(module: &Module) -> Result<BTreeMap<FunctionId, &Function>, SelectionError> {
    let mut functions = BTreeMap::new();
    for function in &module.functions {
        if functions.insert(function.id, function).is_some() {
            return Err(SelectionError::DuplicateFunction {
                function: function.id,
            });
        }
    }
    Ok(functions)
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
    globals: &BTreeMap<GlobalId, &Global>,
    functions_by_id: &BTreeMap<FunctionId, &Function>,
    float_constants: &BTreeMap<(TypeId, String), SelectedFloatConstant>,
) -> Result<MachineFunction, SelectionError> {
    if function.signature.variadic {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::Variadic,
        });
    }
    if !matches!(
        function.signature.calling_convention,
        CallingConvention::C | CallingConvention::FarCdecl | CallingConvention::FarPascal
    ) {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::CallingConvention(function.signature.calling_convention),
        });
    }
    if !function.attributes.is_empty() {
        return Err(SelectionError::UnsupportedFunctionProperty {
            function: function.id,
            property: FunctionProperty::Attributes,
        });
    }
    if function.signature.calling_convention == CallingConvention::FarPascal
        && !matches!(type_kind(types, function.signature.result)?, TypeKind::Void)
    {
        return Err(SelectionError::UnsupportedExternalResult {
            function: function.id,
            result: function.signature.result,
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
    let mut selector = FunctionSelector::new(
        function,
        types,
        globals,
        functions_by_id,
        float_constants,
        block_ids,
    )?;
    selector.select_parameters()?;
    for block in &function.blocks {
        selector.select_block(block)?;
    }
    selector.finish()
}

fn validate_external_declaration(
    function: &Function,
    types: &BTreeMap<TypeId, &TypeKind>,
) -> Result<(), SelectionError> {
    if function.linkage != Linkage::External {
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
    for (index, parameter) in function.parameters.iter().enumerate() {
        let signature_type = function.signature.parameters[index];
        if parameter.type_id != signature_type {
            return Err(SelectionError::SignatureParameterTypeMismatch {
                function: function.id,
                parameter: index,
                signature: signature_type,
                value: parameter.type_id,
            });
        }
        if function.signature.calling_convention == CallingConvention::FarPascal {
            far_pascal_argument_type(types, parameter.type_id)?;
        }
    }
    Ok(())
}

fn far_pascal_argument_type(
    types: &BTreeMap<TypeId, &TypeKind>,
    type_id: TypeId,
) -> Result<(), SelectionError> {
    match type_kind(types, type_id)? {
        TypeKind::Integer { bits: 16 | 32 } => Ok(()),
        TypeKind::Pointer {
            address_space: AddressSpace::NearData,
        } => Ok(()),
        TypeKind::Integer { bits } => Err(SelectionError::UnsupportedIntegerWidth {
            type_id,
            bits: *bits,
        }),
        TypeKind::Void
        | TypeKind::Float(_)
        | TypeKind::Array { .. }
        | TypeKind::Structure { .. } => Err(SelectionError::UnsupportedType { type_id }),
        TypeKind::Pointer { address_space } => Err(SelectionError::UnsupportedAddressSpace {
            type_id,
            address_space: *address_space,
        }),
    }
}

fn type_kind<'types>(
    types: &'types BTreeMap<TypeId, &'types TypeKind>,
    type_id: TypeId,
) -> Result<&'types TypeKind, SelectionError> {
    types
        .get(&type_id)
        .copied()
        .ok_or(SelectionError::MissingType { type_id })
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

#[derive(Clone, Debug)]
enum SelectedLocation {
    Register(VirtualRegisterId),
    /// A pure 16-bit offset from a near pointer.  Python's
    /// `addressforms.selected` carries this in `ir.Mem.offset` until the
    /// consuming memory operation; do the same here instead of emitting an
    /// address arithmetic instruction which has no independent use.
    RegisterOffset {
        base: VirtualRegisterId,
        addend: i64,
    },
    Frame {
        index: FrameIndex,
        addend: i64,
    },
    Global {
        name: String,
        addend: i64,
    },
}

#[derive(Clone, Debug)]
struct SelectedValue {
    location: SelectedLocation,
    type_id: TypeId,
}

#[derive(Clone, Debug)]
struct SelectedFloatConstant {
    name: String,
    format: X87MemoryFormat,
}

#[derive(Clone, Copy, Debug)]
enum PendingComparison {
    Integer {
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        condition: ConditionCode,
    },
    Float {
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        condition: ConditionCode,
    },
}

struct FunctionSelector<'types> {
    function: &'types Function,
    types: &'types BTreeMap<TypeId, &'types TypeKind>,
    globals: &'types BTreeMap<GlobalId, &'types Global>,
    functions_by_id: &'types BTreeMap<FunctionId, &'types Function>,
    float_constants: &'types BTreeMap<(TypeId, String), SelectedFloatConstant>,
    block_ids: BTreeSet<BlockId>,
    values: BTreeMap<ValueId, SelectedValue>,
    comparisons: BTreeMap<ValueId, PendingComparison>,
    virtual_registers: Vec<VirtualRegister>,
    frame_objects: Vec<FrameObject>,
    parameter_frames: Vec<FrameIndex>,
    entry_prefix: Vec<MachineInstruction>,
    entry: MachineBlockId,
    blocks: Vec<MachineBlock>,
    next_virtual_register: u32,
    next_frame_index: u32,
    next_instruction: u32,
}

impl<'types> FunctionSelector<'types> {
    fn new(
        function: &'types Function,
        types: &'types BTreeMap<TypeId, &'types TypeKind>,
        globals: &'types BTreeMap<GlobalId, &'types Global>,
        functions_by_id: &'types BTreeMap<FunctionId, &'types Function>,
        float_constants: &'types BTreeMap<(TypeId, String), SelectedFloatConstant>,
        block_ids: BTreeSet<BlockId>,
    ) -> Result<Self, SelectionError> {
        // Portable IR carries no entry field: its defined-function contract is
        // that the first block is the entry.  HIR lowering verifies that its
        // explicit entry has this position before producing portable IR.
        let entry = MachineBlockId::new(
            function
                .blocks
                .first()
                .expect("selection only constructs Machine IR for a defined function")
                .id
                .get(),
        );
        let mut selector = Self {
            function,
            types,
            globals,
            functions_by_id,
            float_constants,
            block_ids,
            values: BTreeMap::new(),
            comparisons: BTreeMap::new(),
            virtual_registers: Vec::new(),
            frame_objects: Vec::new(),
            parameter_frames: Vec::with_capacity(function.parameters.len()),
            entry_prefix: Vec::new(),
            entry,
            blocks: Vec::with_capacity(function.blocks.len()),
            next_virtual_register: 0,
            next_frame_index: 0,
            next_instruction: 0,
        };
        selector.entry_prefix = Vec::with_capacity(function.parameters.len());
        Ok(selector)
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
            let (size, alignment) = self.abi_size(parameter.type_id)?;
            let frame = self.fresh_frame_object(
                size,
                alignment,
                FrameObjectKind::IncomingArgument {
                    parameter: index as u32,
                },
            )?;
            self.parameter_frames.push(frame);
            if !function_uses_value(self.function, parameter.id) {
                continue;
            }
            let register = self.fresh_virtual_register(self.value_class(parameter.type_id)?)?;
            self.values.insert(
                parameter.id,
                SelectedValue {
                    location: SelectedLocation::Register(register),
                    type_id: parameter.type_id,
                },
            );
            let load = if matches!(self.type_kind(parameter.type_id)?, TypeKind::Float(_)) {
                self.machine_instruction(
                    X86Opcode::X87Load,
                    vec![
                        virtual_operand(register, OperandRole::Def),
                        float_format_operand(self.float_format(parameter.type_id)?),
                        frame_operand(frame, 0),
                    ],
                    load_flags(false),
                )?
            } else {
                self.machine_instruction(
                    X86Opcode::Load,
                    vec![
                        virtual_operand(register, OperandRole::Def),
                        frame_operand(frame, 0),
                    ],
                    load_flags(false),
                )?
            };
            self.entry_prefix.push(load);
        }
        Ok(())
    }

    fn select_block(&mut self, block: &Block) -> Result<(), SelectionError> {
        let mut instructions = if MachineBlockId::new(block.id.get()) == self.entry {
            std::mem::take(&mut self.entry_prefix)
        } else {
            Vec::new()
        };
        for instruction in &block.instructions {
            self.select_instruction(block.id, instruction, &mut instructions)?;
        }
        let (terminator, successors) = self.select_terminator(block, &mut instructions)?;
        if let Some(terminator) = terminator {
            instructions.push(terminator);
        }
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
                if matches!(op, UnaryOp::FloatNegate | UnaryOp::FloatAbsolute) {
                    return self.select_float_unary(block, instruction, *op, operand, output);
                }
                let opcode = match op {
                    UnaryOp::Negate => X86Opcode::Neg,
                    UnaryOp::Not => X86Opcode::Not,
                    UnaryOp::FloatNegate | UnaryOp::FloatAbsolute => unreachable!("handled above"),
                };
                let result = self.result_definition(block, instruction)?;
                let source =
                    self.select_operand(block, instruction.id, operand, result.type_id, output)?;
                let source = self.materialize_register(source, output)?;
                let result = self.define_register_value(result)?;
                self.copy(result, source, output)?;
                self.push_instruction(
                    opcode,
                    vec![virtual_operand(result, OperandRole::UseDef)],
                    InstructionFlags::NONE,
                    output,
                )?;
            }
            InstructionKind::Binary { op, left, right } => {
                if matches!(
                    op,
                    BinaryOp::FloatAdd
                        | BinaryOp::FloatSubtract
                        | BinaryOp::FloatMultiply
                        | BinaryOp::FloatDivide
                ) {
                    return self.select_float_binary(block, instruction, *op, left, right, output);
                }
                if matches!(
                    op,
                    BinaryOp::SignedDivide
                        | BinaryOp::UnsignedDivide
                        | BinaryOp::SignedRemainder
                        | BinaryOp::UnsignedRemainder
                ) {
                    return self.select_integer_division(
                        block,
                        instruction,
                        *op,
                        left,
                        right,
                        output,
                    );
                }
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
                    | BinaryOp::UnsignedRemainder => unreachable!("handled above"),
                    BinaryOp::ShiftLeft
                    | BinaryOp::LogicalShiftRight
                    | BinaryOp::ArithmeticShiftRight => {
                        return Err(SelectionError::UnsupportedBinary {
                            function: self.function.id,
                            block,
                            instruction: instruction.id,
                        });
                    }
                    BinaryOp::FloatAdd
                    | BinaryOp::FloatSubtract
                    | BinaryOp::FloatMultiply
                    | BinaryOp::FloatDivide => unreachable!("handled above"),
                };
                let result = self.result_definition(block, instruction)?;
                let left =
                    self.select_operand(block, instruction.id, left, result.type_id, output)?;
                let right =
                    self.select_operand(block, instruction.id, right, result.type_id, output)?;
                let left = self.materialize_register(left, output)?;
                let right = self.materialize_register(right, output)?;
                let result = self.define_register_value(result)?;
                self.copy(result, left, output)?;
                self.push_instruction(
                    opcode,
                    vec![
                        virtual_operand(result, OperandRole::UseDef),
                        virtual_operand(right, OperandRole::Use),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
            }
            InstructionKind::StackAlloc {
                size,
                alignment,
                address_space,
            } => self.select_stack_alloc(block, instruction, *size, *alignment, *address_space)?,
            InstructionKind::ParameterAddress { parameter } => {
                self.select_parameter_address(block, instruction, *parameter)?
            }
            InstructionKind::Cast { op, operand, to } => {
                self.select_cast(block, instruction, *op, operand, *to, output)?
            }
            InstructionKind::Load {
                address,
                alignment: _,
                volatile,
            } => self.select_load(block, instruction, address, *volatile, output)?,
            InstructionKind::Store {
                address,
                value,
                alignment: _,
                volatile,
            } => self.select_store(block, instruction, address, value, *volatile, output)?,
            InstructionKind::Compare {
                predicate,
                left,
                right,
            } => self.select_branch_compare(block, instruction, *predicate, left, right, output)?,
            InstructionKind::GetElementPointer { base, indices } => {
                self.select_get_element_pointer(block, instruction, base, indices, output)?
            }
            InstructionKind::ComposePointer { segment, offset } => {
                self.select_compose_pointer(block, instruction, segment, offset, output)?
            }
            InstructionKind::Phi { .. }
            | InstructionKind::Select { .. }
            | InstructionKind::Intrinsic { .. } => {
                return Err(SelectionError::UnsupportedInstruction {
                    function: self.function.id,
                    block,
                    instruction: instruction.id,
                });
            }
            InstructionKind::Call {
                callee,
                arguments,
                effects,
            } => self.select_call(block, instruction, callee, arguments, *effects, output)?,
        }
        Ok(())
    }

    /// Selects x86's one-operand integer division family.
    ///
    /// The Machine operands make all of the architectural effects explicit:
    /// high and low dividend inputs, the flexible divisor, and both quotient
    /// and remainder definitions.  Fixed constraints are deliberately local;
    /// allocation splits them into short ranges rather than pinning source
    /// values to AX/EAX or DX/EDX for their entire lifetime.
    fn select_integer_division(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        op: BinaryOp,
        left: &Operand,
        right: &Operand,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        let bits = self.integer_bits(result.type_id)?;
        if !matches!(bits, 16 | 32) {
            return Err(SelectionError::UnsupportedBinary {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let signed = matches!(op, BinaryOp::SignedDivide | BinaryOp::SignedRemainder);
        let remainder = matches!(op, BinaryOp::SignedRemainder | BinaryOp::UnsignedRemainder);
        let class = self.integer_class(result.type_id)?;
        let left = self.select_operand(block, instruction.id, left, result.type_id, output)?;
        let right = self.select_operand(block, instruction.id, right, result.type_id, output)?;
        let left = self.materialize_register(left, output)?;
        let divisor = self.materialize_register(right, output)?;

        let low = self.fresh_virtual_register(class)?;
        let high = self.fresh_virtual_register(class)?;
        let quotient = self.fresh_virtual_register(class)?;
        let remainder_value = self.fresh_virtual_register(class)?;
        let (low_register, high_register) = if bits == 16 {
            (X86Register::Ax, X86Register::Dx)
        } else {
            (X86Register::Eax, X86Register::Edx)
        };

        self.copy(low, left, output)?;
        if signed {
            self.push_instruction(
                X86Opcode::CwdCdq,
                vec![
                    fixed_virtual_operand(high, OperandRole::Def, high_register),
                    fixed_virtual_operand(low, OperandRole::Use, low_register),
                ],
                InstructionFlags::NONE,
                output,
            )?;
        } else {
            self.push_instruction(
                X86Opcode::Mov,
                vec![
                    fixed_virtual_operand(high, OperandRole::Def, high_register),
                    immediate_operand(0),
                ],
                InstructionFlags::NONE,
                output,
            )?;
        }
        self.push_instruction(
            if signed {
                X86Opcode::Idiv
            } else {
                X86Opcode::Div
            },
            vec![
                fixed_virtual_operand(high, OperandRole::Use, high_register),
                fixed_virtual_operand(low, OperandRole::Use, low_register),
                virtual_operand(divisor, OperandRole::Use),
                fixed_virtual_operand(quotient, OperandRole::Def, low_register),
                fixed_virtual_operand(remainder_value, OperandRole::Def, high_register),
            ],
            InstructionFlags::NONE,
            output,
        )?;
        let result = self.define_register_value(result)?;
        self.copy(
            result,
            if remainder { remainder_value } else { quotient },
            output,
        )
    }

    /// Selects a byte offset from a pointer.  The portable instruction's
    /// index has already been scaled by its producer.
    fn select_get_element_pointer(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        base: &Operand,
        indices: &[Operand],
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        let base_type = self.operand_type(block, instruction.id, base)?;
        self.require_operand_type(block, instruction.id, result.type_id, base_type)?;

        let [index] = indices else {
            return Err(SelectionError::UnsupportedInstruction {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        };
        let index_type = self.operand_type(block, instruction.id, index)?;
        if self.integer_bits(index_type)? != 16 {
            return Err(SelectionError::UnsupportedInstruction {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }

        match self.pointer_address_space(result.type_id)? {
            AddressSpace::NearData => {}
            AddressSpace::FarData => {
                return self.select_far_get_element_pointer(
                    block,
                    instruction,
                    result,
                    base,
                    index,
                    index_type,
                    output,
                );
            }
            address_space => {
                return Err(SelectionError::UnsupportedAddressSpace {
                    type_id: result.type_id,
                    address_space,
                });
            }
        }

        let base = self.select_operand(block, instruction.id, base, result.type_id, output)?;
        let offset = match index {
            Operand::Constant(TypedConstant {
                type_id,
                value: Constant::Integer(value),
            }) => {
                self.require_operand_type(block, instruction.id, index_type, *type_id)?;
                integer_immediate(*value, 16)
            }
            _ => {
                let base = self.materialize_register(base, output)?;
                let result = self.define_register_value(result)?;
                self.copy(result, base, output)?;
                let offset =
                    self.select_operand(block, instruction.id, index, index_type, output)?;
                let offset = self.materialize_register(offset, output)?;
                self.push_instruction(
                    X86Opcode::Add,
                    vec![
                        virtual_operand(result, OperandRole::UseDef),
                        virtual_operand(offset, OperandRole::Use),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
                return Ok(());
            }
        };
        let location = self.offset_near_address(base, offset, output)?;
        self.insert_value(result, location)
    }

    /// Advances only the low offset word of a far 16:16 pointer.  Huge
    /// pointers require selector normalization and are deliberately refused
    /// by the caller rather than approximated as far pointers.
    fn select_far_get_element_pointer(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        result_definition: &Value,
        base: &Operand,
        index: &Operand,
        index_type: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let base = self.select_operand(
            block,
            instruction.id,
            base,
            result_definition.type_id,
            output,
        )?;
        let base = self.materialize_register(base, output)?;
        let low = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
        self.push_instruction(
            X86Opcode::LowWord,
            vec![
                virtual_operand(low, OperandRole::Def),
                virtual_operand(base, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )?;
        let offset = match index {
            Operand::Constant(TypedConstant {
                type_id,
                value: Constant::Integer(value),
            }) => {
                self.require_operand_type(block, instruction.id, index_type, *type_id)?;
                immediate_operand(integer_immediate(*value, 16))
            }
            _ => {
                let offset =
                    self.select_operand(block, instruction.id, index, index_type, output)?;
                let offset = self.materialize_register(offset, output)?;
                virtual_operand(offset, OperandRole::Use)
            }
        };
        self.push_instruction(
            X86Opcode::Add,
            vec![virtual_operand(low, OperandRole::UseDef), offset],
            InstructionFlags::NONE,
            output,
        )?;
        let high = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
        self.push_instruction(
            X86Opcode::HighWord,
            vec![
                virtual_operand(high, OperandRole::Def),
                virtual_operand(base, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )?;
        let result = self.fresh_virtual_register(X86RegisterClass::Dword.machine_class())?;
        self.insert_value(result_definition, SelectedLocation::Register(result))?;
        self.push_instruction(
            X86Opcode::MergeWords,
            vec![
                virtual_operand(result, OperandRole::Def),
                virtual_operand(low, OperandRole::Use),
                virtual_operand(high, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )
    }

    /// Forms a 16:16 pointer as a dword, with the offset in the low word.
    fn select_compose_pointer(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        segment: &Operand,
        offset: &Operand,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result_definition = self.result_definition(block, instruction)?;
        self.require_1616_pointer(result_definition.type_id)?;

        let segment_type = self.operand_type(block, instruction.id, segment)?;
        let offset_type = self.operand_type(block, instruction.id, offset)?;
        if self.integer_bits(segment_type)? != 16 || self.integer_bits(offset_type)? != 16 {
            return Err(SelectionError::UnsupportedInstruction {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let segment = self.select_operand(block, instruction.id, segment, segment_type, output)?;
        let offset = self.select_operand(block, instruction.id, offset, offset_type, output)?;
        let segment = self.materialize_register(segment, output)?;
        let offset = self.materialize_register(offset, output)?;
        let result = self.fresh_virtual_register(X86RegisterClass::Dword.machine_class())?;
        self.insert_value(result_definition, SelectedLocation::Register(result))?;
        self.push_instruction(
            X86Opcode::MergeWords,
            vec![
                virtual_operand(result, OperandRole::Def),
                virtual_operand(offset, OperandRole::Use),
                virtual_operand(segment, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )
    }

    fn select_float_unary(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        op: UnaryOp,
        operand: &Operand,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        if !matches!(self.type_kind(result.type_id)?, TypeKind::Float(_)) {
            return Err(SelectionError::UnsupportedUnary {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let operand_type = self.operand_type(block, instruction.id, operand)?;
        if operand_type != result.type_id {
            return Err(SelectionError::OperandTypeMismatch {
                function: self.function.id,
                block,
                instruction: instruction.id,
                expected: result.type_id,
                actual: operand_type,
            });
        }
        let source = self.select_operand(block, instruction.id, operand, operand_type, output)?;
        let source = self.materialize_register(source, output)?;
        let destination = self.define_register_value(result)?;
        self.push_instruction(
            match op {
                UnaryOp::FloatNegate => X86Opcode::X87ChangeSign,
                UnaryOp::FloatAbsolute => X86Opcode::X87Absolute,
                UnaryOp::Negate | UnaryOp::Not => unreachable!("integer unary selected elsewhere"),
            },
            vec![
                virtual_operand(destination, OperandRole::Def),
                virtual_operand(source, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )
    }

    fn select_float_binary(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        op: BinaryOp,
        left: &Operand,
        right: &Operand,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        if !matches!(self.type_kind(result.type_id)?, TypeKind::Float(_)) {
            return Err(SelectionError::UnsupportedBinary {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let left = self.select_operand(block, instruction.id, left, result.type_id, output)?;
        let right = self.select_operand(block, instruction.id, right, result.type_id, output)?;
        let left = self.materialize_register(left, output)?;
        let right = self.materialize_register(right, output)?;
        let destination = self.define_register_value(result)?;
        self.push_instruction(
            match op {
                BinaryOp::FloatAdd => X86Opcode::X87Add,
                BinaryOp::FloatSubtract => X86Opcode::X87Subtract,
                BinaryOp::FloatMultiply => X86Opcode::X87Multiply,
                BinaryOp::FloatDivide => X86Opcode::X87Divide,
                _ => unreachable!("integer binary selected elsewhere"),
            },
            vec![
                virtual_operand(destination, OperandRole::Def),
                virtual_operand(left, OperandRole::Use),
                virtual_operand(right, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )
    }

    /// Records a comparison only when its i1 result is later consumed as a
    /// branch condition.  Materializing that boolean would introduce a
    /// source-independent value with no x86 representation in this slice;
    /// the target selector instead emits the flag-producing compare at the
    /// branch boundary.
    fn select_branch_compare(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        predicate: ComparePredicate,
        left: &Operand,
        right: &Operand,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        if !matches!(
            self.type_kind(result.type_id)?,
            TypeKind::Integer { bits: 1 }
        ) {
            return Err(SelectionError::UnsupportedCompare {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let condition = match predicate {
            ComparePredicate::Equal => ConditionCode::Equal,
            ComparePredicate::NotEqual => ConditionCode::NotEqual,
            ComparePredicate::SignedLessThan => ConditionCode::Less,
            ComparePredicate::SignedLessEqual => ConditionCode::LessOrEqual,
            ComparePredicate::SignedGreaterThan => ConditionCode::Greater,
            ComparePredicate::SignedGreaterEqual => ConditionCode::GreaterOrEqual,
            ComparePredicate::UnsignedLessThan => ConditionCode::Below,
            ComparePredicate::UnsignedLessEqual => ConditionCode::BelowOrEqual,
            ComparePredicate::UnsignedGreaterThan => ConditionCode::Above,
            ComparePredicate::UnsignedGreaterEqual => ConditionCode::AboveOrEqual,
            ComparePredicate::OrderedEqual => ConditionCode::Equal,
            ComparePredicate::OrderedNotEqual => ConditionCode::NotEqual,
            ComparePredicate::OrderedLessThan => ConditionCode::Below,
            ComparePredicate::OrderedLessEqual => ConditionCode::BelowOrEqual,
            ComparePredicate::OrderedGreaterThan => ConditionCode::Above,
            ComparePredicate::OrderedGreaterEqual => ConditionCode::AboveOrEqual,
        };
        let left_type = self.operand_type(block, instruction.id, left)?;
        let right_type = self.operand_type(block, instruction.id, right)?;
        if left_type != right_type {
            return Err(SelectionError::UnsupportedCompare {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let left = self.select_operand(block, instruction.id, left, left_type, output)?;
        let right = self.select_operand(block, instruction.id, right, left_type, output)?;
        let left = self.materialize_register(left, output)?;
        let right = self.materialize_register(right, output)?;
        let comparison = match self.type_kind(left_type)? {
            TypeKind::Integer { bits: 16 | 32 }
                if !matches!(
                    predicate,
                    ComparePredicate::OrderedEqual
                        | ComparePredicate::OrderedNotEqual
                        | ComparePredicate::OrderedLessThan
                        | ComparePredicate::OrderedLessEqual
                        | ComparePredicate::OrderedGreaterThan
                        | ComparePredicate::OrderedGreaterEqual
                ) =>
            {
                PendingComparison::Integer {
                    left,
                    right,
                    condition,
                }
            }
            TypeKind::Float(_)
                if matches!(
                    predicate,
                    ComparePredicate::OrderedEqual
                        | ComparePredicate::OrderedNotEqual
                        | ComparePredicate::OrderedLessThan
                        | ComparePredicate::OrderedLessEqual
                        | ComparePredicate::OrderedGreaterThan
                        | ComparePredicate::OrderedGreaterEqual
                ) =>
            {
                PendingComparison::Float {
                    left,
                    right,
                    condition,
                }
            }
            _ => {
                return Err(SelectionError::UnsupportedCompare {
                    function: self.function.id,
                    block,
                    instruction: instruction.id,
                });
            }
        };
        self.comparisons.insert(result.id, comparison);
        Ok(())
    }

    fn select_stack_alloc(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        size: u32,
        alignment: u32,
        address_space: AddressSpace,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        self.require_pointer_type(result.type_id, address_space)?;
        if address_space != AddressSpace::NearData {
            return Err(SelectionError::UnsupportedAddressSpace {
                type_id: result.type_id,
                address_space,
            });
        }
        let frame = self.fresh_frame_object(size, alignment, FrameObjectKind::Local)?;
        self.insert_value(
            result,
            SelectedLocation::Frame {
                index: frame,
                addend: 0,
            },
        )
    }

    fn select_parameter_address(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        parameter: u32,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        self.require_near_pointer(result.type_id)?;
        let frame = usize::try_from(parameter)
            .ok()
            .and_then(|index| self.parameter_frames.get(index))
            .copied()
            .ok_or(SelectionError::ParameterAddressOutOfBounds {
                function: self.function.id,
                block,
                instruction: instruction.id,
                parameter,
            })?;
        self.insert_value(
            result,
            SelectedLocation::Frame {
                index: frame,
                addend: 0,
            },
        )
    }

    fn select_cast(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        op: CastOp,
        operand: &Operand,
        to: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        if result.type_id != to {
            return Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let operand_type = self.operand_type(block, instruction.id, operand)?;
        if matches!(
            op,
            CastOp::IntegerToFloat
                | CastOp::FloatExtend
                | CastOp::FloatTruncate
                | CastOp::FloatToInteger { .. }
        ) {
            return self.select_float_cast(
                block,
                instruction,
                op,
                operand,
                operand_type,
                to,
                output,
            );
        }
        match op {
            CastOp::Truncate => {
                return self.select_truncate_dword_to_word(
                    block,
                    instruction,
                    operand,
                    operand_type,
                    to,
                    output,
                );
            }
            CastOp::SignExtend => {
                return self.select_extend_word_to_dword(
                    block,
                    instruction,
                    operand,
                    operand_type,
                    to,
                    X86Opcode::SignExtendWordToDword,
                    output,
                );
            }
            CastOp::ZeroExtend => {
                return self.select_extend_word_to_dword(
                    block,
                    instruction,
                    operand,
                    operand_type,
                    to,
                    X86Opcode::ZeroExtendWordToDword,
                    output,
                );
            }
            _ => {}
        }
        if op != CastOp::Bitcast {
            return Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        if let (TypeKind::Integer { bits: from }, TypeKind::Integer { bits: into }) =
            (self.type_kind(operand_type)?, self.type_kind(to)?)
        {
            if from != into {
                return Err(SelectionError::UnsupportedCast {
                    function: self.function.id,
                    block,
                    instruction: instruction.id,
                });
            }
            let selected =
                self.select_operand(block, instruction.id, operand, operand_type, output)?;
            return self.insert_value(result, selected.location);
        }
        let (
            TypeKind::Pointer {
                address_space: from,
            },
            TypeKind::Pointer {
                address_space: into,
            },
        ) = (self.type_kind(operand_type)?, self.type_kind(to)?)
        else {
            return Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        };
        if from != into || *into != AddressSpace::NearData {
            return Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let selected = self.select_operand(block, instruction.id, operand, operand_type, output)?;
        self.insert_value(result, selected.location)
    }

    fn select_truncate_dword_to_word(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        operand: &Operand,
        source_type: TypeId,
        destination_type: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        if self.integer_bits(source_type)? != 32 || self.integer_bits(destination_type)? != 16 {
            return Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let source = self.select_operand(block, instruction.id, operand, source_type, output)?;
        let source = self.materialize_register(source, output)?;
        let destination =
            self.define_register_value(self.result_definition(block, instruction)?)?;
        self.push_instruction(
            X86Opcode::LowWord,
            vec![
                virtual_operand(destination, OperandRole::Def),
                virtual_operand(source, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )
    }

    fn select_float_cast(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        op: CastOp,
        operand: &Operand,
        source_type: TypeId,
        destination_type: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        match op {
            CastOp::IntegerToFloat => {
                self.float_kind(destination_type)?;
                let bits = self.integer_bits(source_type)?;
                let format = match bits {
                    16 => X87MemoryFormat::Signed16,
                    32 => X87MemoryFormat::Signed32,
                    _ => {
                        return Err(SelectionError::UnsupportedCast {
                            function: self.function.id,
                            block,
                            instruction: instruction.id,
                        });
                    }
                };
                let destination =
                    self.define_register_value(self.result_definition(block, instruction)?)?;
                if let Operand::Constant(TypedConstant {
                    type_id,
                    value: Constant::Integer(value),
                }) = operand
                {
                    self.require_operand_type(block, instruction.id, source_type, *type_id)?;
                    if let Some(opcode) = match *value {
                        0 => Some(X86Opcode::X87LoadZero),
                        1 => Some(X86Opcode::X87LoadOne),
                        _ => None,
                    } {
                        return self.push_instruction(
                            opcode,
                            vec![virtual_operand(destination, OperandRole::Def)],
                            InstructionFlags::NONE,
                            output,
                        );
                    }
                    let frame = self.fresh_frame_object(
                        u32::from(bits / 8),
                        2,
                        FrameObjectKind::Temporary,
                    )?;
                    self.push_instruction(
                        X86Opcode::Store,
                        vec![
                            frame_operand(frame, 0),
                            immediate_operand(i64::from(bits)),
                            immediate_operand(integer_immediate(*value, bits)),
                        ],
                        store_flags(false),
                        output,
                    )?;
                    return self.push_instruction(
                        X86Opcode::X87IntegerLoad,
                        vec![
                            virtual_operand(destination, OperandRole::Def),
                            float_format_operand(format),
                            frame_operand(frame, 0),
                        ],
                        load_flags(false),
                        output,
                    );
                }
                let source =
                    self.select_operand(block, instruction.id, operand, source_type, output)?;
                let source = self.materialize_register(source, output)?;
                let frame =
                    self.fresh_frame_object(u32::from(bits / 8), 2, FrameObjectKind::Temporary)?;
                self.push_instruction(
                    X86Opcode::Store,
                    vec![
                        frame_operand(frame, 0),
                        virtual_operand(source, OperandRole::Use),
                    ],
                    store_flags(false),
                    output,
                )?;
                self.push_instruction(
                    X86Opcode::X87IntegerLoad,
                    vec![
                        virtual_operand(destination, OperandRole::Def),
                        float_format_operand(format),
                        frame_operand(frame, 0),
                    ],
                    load_flags(false),
                    output,
                )
            }
            CastOp::FloatExtend | CastOp::FloatTruncate => {
                let source_kind = self.float_kind(source_type)?;
                let destination_kind = self.float_kind(destination_type)?;
                let valid = match op {
                    CastOp::FloatExtend => float_bytes(source_kind) < float_bytes(destination_kind),
                    CastOp::FloatTruncate => {
                        float_bytes(source_kind) > float_bytes(destination_kind)
                    }
                    _ => unreachable!(),
                };
                if !valid {
                    return Err(SelectionError::UnsupportedCast {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                    });
                }
                let source =
                    self.select_operand(block, instruction.id, operand, source_type, output)?;
                let source = self.materialize_register(source, output)?;
                let destination =
                    self.define_register_value(self.result_definition(block, instruction)?)?;
                self.copy(destination, source, output)
            }
            CastOp::FloatToInteger { rounding } => {
                self.float_kind(source_type)?;
                let bits = self.integer_bits(destination_type)?;
                let format = match bits {
                    16 => X87MemoryFormat::Signed16,
                    32 => X87MemoryFormat::Signed32,
                    _ => {
                        return Err(SelectionError::UnsupportedCast {
                            function: self.function.id,
                            block,
                            instruction: instruction.id,
                        });
                    }
                };
                let opcode = match rounding {
                    FloatRounding::Dynamic => X86Opcode::X87IntegerStorePop,
                    FloatRounding::TowardZero => X86Opcode::X87IntegerStoreTrunc,
                    FloatRounding::NearestEven => {
                        return Err(SelectionError::UnsupportedCast {
                            function: self.function.id,
                            block,
                            instruction: instruction.id,
                        });
                    }
                };
                let source =
                    self.select_operand(block, instruction.id, operand, source_type, output)?;
                let source = self.materialize_register(source, output)?;
                let frame =
                    self.fresh_frame_object(u32::from(bits / 8), 2, FrameObjectKind::Temporary)?;
                self.push_instruction(
                    opcode,
                    vec![
                        virtual_operand(source, OperandRole::Use),
                        float_format_operand(format),
                        frame_operand(frame, 0),
                    ],
                    store_flags(false),
                    output,
                )?;
                let destination =
                    self.define_register_value(self.result_definition(block, instruction)?)?;
                self.push_instruction(
                    X86Opcode::Load,
                    vec![
                        virtual_operand(destination, OperandRole::Def),
                        frame_operand(frame, 0),
                    ],
                    load_flags(false),
                    output,
                )
            }
            _ => Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            }),
        }
    }

    fn select_extend_word_to_dword(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        operand: &Operand,
        source_type: TypeId,
        destination_type: TypeId,
        opcode: X86Opcode,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        if self.integer_bits(source_type)? != 16 || self.integer_bits(destination_type)? != 32 {
            return Err(SelectionError::UnsupportedCast {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        }
        let source = self.select_operand(block, instruction.id, operand, source_type, output)?;
        let source = self.materialize_register(source, output)?;
        let destination =
            self.define_register_value(self.result_definition(block, instruction)?)?;
        self.push_instruction(
            opcode,
            vec![
                virtual_operand(destination, OperandRole::Def),
                virtual_operand(source, OperandRole::Use),
            ],
            InstructionFlags::NONE,
            output,
        )
    }

    fn select_load(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        address: &Operand,
        volatile: bool,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let result = self.result_definition(block, instruction)?;
        let address_type = self.operand_type(block, instruction.id, address)?;
        let selected = self.select_operand(block, instruction.id, address, address_type, output)?;
        let destination = self.define_register_value(result)?;
        match self.pointer_address_space(address_type)? {
            AddressSpace::NearData => {
                let address = self.memory_address_operands(selected);
                if matches!(self.type_kind(result.type_id)?, TypeKind::Float(_)) {
                    let mut operands = vec![
                        virtual_operand(destination, OperandRole::Def),
                        float_format_operand(self.float_format(result.type_id)?),
                    ];
                    operands.extend(address);
                    return self.push_instruction(
                        X86Opcode::X87Load,
                        operands,
                        load_flags(volatile),
                        output,
                    );
                }
                let mut operands = vec![virtual_operand(destination, OperandRole::Def)];
                operands.extend(address);
                self.push_instruction(X86Opcode::Load, operands, load_flags(volatile), output)
            }
            AddressSpace::FarData => {
                let offset = self.extract_far_address(selected, output)?;
                if matches!(self.type_kind(result.type_id)?, TypeKind::Float(_)) {
                    self.push_instruction(
                        X86Opcode::X87Load,
                        vec![
                            virtual_operand(destination, OperandRole::Def),
                            float_format_operand(self.float_format(result.type_id)?),
                            virtual_operand(offset, OperandRole::Use),
                            physical_operand(X86Register::Es, OperandRole::Use),
                        ],
                        load_flags(volatile),
                        output,
                    )?;
                    return self.restore_es(output);
                }
                self.push_instruction(
                    X86Opcode::Load,
                    vec![
                        virtual_operand(destination, OperandRole::Def),
                        virtual_operand(offset, OperandRole::Use),
                        physical_operand(X86Register::Es, OperandRole::Use),
                    ],
                    load_flags(volatile),
                    output,
                )?;
                self.restore_es(output)
            }
            address_space => {
                return Err(SelectionError::UnsupportedAddressSpace {
                    type_id: address_type,
                    address_space,
                });
            }
        }
    }

    fn select_store(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        address: &Operand,
        value: &Operand,
        volatile: bool,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        if !instruction.results.is_empty() {
            return Err(SelectionError::InvalidResultCount {
                function: self.function.id,
                block,
                instruction: instruction.id,
                count: instruction.results.len(),
            });
        }
        let address_type = self.operand_type(block, instruction.id, address)?;
        let value_type = self.operand_type(block, instruction.id, value)?;
        match self.pointer_address_space(address_type)? {
            AddressSpace::NearData => {
                let address =
                    self.select_operand(block, instruction.id, address, address_type, output)?;
                if let Operand::Constant(TypedConstant {
                    type_id,
                    value: Constant::Float(value),
                }) = value
                {
                    if self.float_kind(*type_id)? == FloatKind::Binary32 {
                        self.require_operand_type(block, instruction.id, value_type, *type_id)?;
                        let value = value.parse::<f32>().map_err(|_| {
                            SelectionError::UnsupportedFloatConstant {
                                type_id: *type_id,
                                value: value.clone(),
                            }
                        })?;
                        let address = self.memory_address_operands(address);
                        let mut operands = address;
                        operands.extend([
                            immediate_operand(32),
                            immediate_operand(i64::from(value.to_bits())),
                        ]);
                        return self.push_instruction(
                            X86Opcode::Store,
                            operands,
                            store_flags(volatile),
                            output,
                        );
                    }
                }
                let value =
                    self.select_operand(block, instruction.id, value, value_type, output)?;
                let value = self.materialize_register(value, output)?;
                let address = self.memory_address_operands(address);
                if matches!(self.type_kind(value_type)?, TypeKind::Float(_)) {
                    let mut operands = vec![
                        virtual_operand(value, OperandRole::Use),
                        float_format_operand(self.float_format(value_type)?),
                    ];
                    operands.extend(address);
                    return self.push_instruction(
                        X86Opcode::X87StorePop,
                        operands,
                        store_flags(volatile),
                        output,
                    );
                }
                let mut operands = address;
                operands.push(virtual_operand(value, OperandRole::Use));
                self.push_instruction(X86Opcode::Store, operands, store_flags(volatile), output)
            }
            AddressSpace::FarData => {
                // Keep the stored value live before the far-address scratch
                // definitions, so allocation cannot coalesce it with them.
                let value =
                    self.select_operand(block, instruction.id, value, value_type, output)?;
                let value = self.materialize_register(value, output)?;
                let address =
                    self.select_operand(block, instruction.id, address, address_type, output)?;
                let offset = self.extract_far_address(address, output)?;
                if matches!(self.type_kind(value_type)?, TypeKind::Float(_)) {
                    self.push_instruction(
                        X86Opcode::X87StorePop,
                        vec![
                            virtual_operand(value, OperandRole::Use),
                            float_format_operand(self.float_format(value_type)?),
                            virtual_operand(offset, OperandRole::Use),
                            physical_operand(X86Register::Es, OperandRole::Use),
                        ],
                        store_flags(volatile),
                        output,
                    )?;
                    return self.restore_es(output);
                }
                self.push_instruction(
                    X86Opcode::Store,
                    vec![
                        virtual_operand(offset, OperandRole::Use),
                        virtual_operand(value, OperandRole::Use),
                        physical_operand(X86Register::Es, OperandRole::Use),
                    ],
                    store_flags(volatile),
                    output,
                )?;
                self.restore_es(output)
            }
            address_space => {
                return Err(SelectionError::UnsupportedAddressSpace {
                    type_id: address_type,
                    address_space,
                });
            }
        }
    }

    fn select_call(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        callee: &Callee,
        arguments: &[Operand],
        effects: Effects,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let Callee::Direct(callee) = callee else {
            return Err(SelectionError::IndirectCallee {
                function: self.function.id,
                block,
                instruction: instruction.id,
            });
        };
        let Some(target) = self.functions_by_id.get(callee).copied() else {
            return Err(SelectionError::UnknownCallee {
                function: self.function.id,
                block,
                instruction: instruction.id,
                callee: *callee,
            });
        };
        if arguments.len() != target.signature.parameters.len() {
            return Err(SelectionError::CallArgumentCount {
                function: self.function.id,
                block,
                instruction: instruction.id,
                callee: *callee,
                expected: target.signature.parameters.len(),
                actual: arguments.len(),
            });
        }
        if matches!(
            target.signature.calling_convention,
            CallingConvention::C | CallingConvention::FarCdecl
        ) {
            for index in (0..arguments.len()).rev() {
                self.push_call_argument(
                    block,
                    instruction.id,
                    &arguments[index],
                    target.signature.parameters[index],
                    output,
                )?;
            }
        } else {
            for (argument, expected_type) in arguments.iter().zip(&target.signature.parameters) {
                self.push_call_argument(block, instruction.id, argument, *expected_type, output)?;
            }
        }
        match (
            target.blocks.is_empty(),
            target.linkage,
            target.signature.calling_convention,
        ) {
            (true, Linkage::External, CallingConvention::FarPascal) => {
                if !matches!(self.type_kind(target.signature.result)?, TypeKind::Void)
                    || !instruction.results.is_empty()
                {
                    return Err(SelectionError::UnsupportedCallResult {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        callee: *callee,
                        result: target.signature.result,
                        values: instruction.results.len(),
                    });
                }
                self.push_instruction(
                    X86Opcode::CallFar,
                    vec![external_symbol_operand(target.name.clone())],
                    call_flags(effects),
                    output,
                )
            }
            (false, _, CallingConvention::FarPascal) => self.select_defined_far_pascal_call_result(
                block,
                instruction,
                target,
                effects,
                output,
            ),
            (_, _, CallingConvention::C | CallingConvention::FarCdecl) => {
                self.select_caller_cleanup_call_result(block, instruction, target, effects, output)
            }
            _ => Err(SelectionError::UnsupportedCallTarget {
                function: self.function.id,
                block,
                instruction: instruction.id,
                callee: *callee,
            }),
        }
    }

    fn push_call_argument(
        &mut self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        argument: &Operand,
        expected_type: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        if matches!(self.type_kind(expected_type)?, TypeKind::Float(_)) {
            return self.push_float_call_argument(
                block,
                instruction,
                argument,
                expected_type,
                output,
            );
        }
        let argument = self.select_operand(block, instruction, argument, expected_type, output)?;
        let argument = self.materialize_register(argument, output)?;
        self.push_instruction(
            X86Opcode::Push,
            vec![virtual_operand(argument, OperandRole::Use)],
            InstructionFlags::NONE,
            output,
        )
    }

    fn push_float_call_argument(
        &mut self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        argument: &Operand,
        expected_type: TypeId,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let kind = self.float_kind(expected_type)?;

        if let Operand::Constant(TypedConstant {
            type_id,
            value: Constant::Float(value),
        }) = argument
        {
            self.require_operand_type(block, instruction, expected_type, *type_id)?;
            match kind {
                FloatKind::Binary32 => {
                    let value = value.parse::<f32>().map_err(|_| {
                        SelectionError::UnsupportedFloatConstant {
                            type_id: *type_id,
                            value: value.clone(),
                        }
                    })?;
                    self.push_instruction(
                        X86Opcode::Push,
                        vec![
                            immediate_operand(32),
                            immediate_operand(i64::from(value.to_bits())),
                        ],
                        InstructionFlags::NONE,
                        output,
                    )
                }
                FloatKind::Binary64 => {
                    let value = value.parse::<f64>().map_err(|_| {
                        SelectionError::UnsupportedFloatConstant {
                            type_id: *type_id,
                            value: value.clone(),
                        }
                    })?;
                    let bits = value.to_bits();
                    for word in [(bits >> 32) as u32, bits as u32] {
                        self.push_instruction(
                            X86Opcode::Push,
                            vec![immediate_operand(32), immediate_operand(i64::from(word))],
                            InstructionFlags::NONE,
                            output,
                        )?;
                    }
                    Ok(())
                }
                FloatKind::Extended80 => Err(SelectionError::UnsupportedInstruction {
                    function: self.function.id,
                    block,
                    instruction,
                }),
            }
        } else {
            let value = self.select_operand(block, instruction, argument, expected_type, output)?;
            let value = self.materialize_register(value, output)?;
            let (size, format) = match kind {
                FloatKind::Binary32 => (4, X87MemoryFormat::Float32),
                FloatKind::Binary64 => (8, X87MemoryFormat::Float64),
                FloatKind::Extended80 => {
                    return Err(SelectionError::UnsupportedInstruction {
                        function: self.function.id,
                        block,
                        instruction,
                    });
                }
            };
            let frame = self.fresh_frame_object(size, 2, FrameObjectKind::Temporary)?;
            self.push_instruction(
                X86Opcode::X87StorePop,
                vec![
                    virtual_operand(value, OperandRole::Use),
                    float_format_operand(format),
                    frame_operand(frame, 0),
                ],
                store_flags(false),
                output,
            )?;
            for addend in (0..size).step_by(4).rev() {
                let raw = self.fresh_virtual_register(X86RegisterClass::Dword.machine_class())?;
                self.push_instruction(
                    X86Opcode::Load,
                    vec![
                        virtual_operand(raw, OperandRole::Def),
                        frame_operand(frame, i64::from(addend)),
                    ],
                    load_flags(false),
                    output,
                )?;
                self.push_instruction(
                    X86Opcode::Push,
                    vec![virtual_operand(raw, OperandRole::Use)],
                    InstructionFlags::NONE,
                    output,
                )?;
            }
            Ok(())
        }
    }

    fn select_defined_far_pascal_call_result(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        target: &Function,
        effects: Effects,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let mut operands = vec![function_operand(MachineFunctionId::new(target.id.get()))];
        match self.type_kind(target.signature.result)? {
            TypeKind::Void => {
                if !instruction.results.is_empty() {
                    return Err(SelectionError::UnsupportedCallResult {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        callee: target.id,
                        result: target.signature.result,
                        values: instruction.results.len(),
                    });
                }
                self.push_instruction(X86Opcode::CallFar, operands, call_flags(effects), output)
            }
            TypeKind::Integer { bits: 16 } => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let result = self.define_register_value(result)?;
                operands.push(fixed_virtual_operand(
                    result,
                    OperandRole::Def,
                    X86Register::Ax,
                ));
                self.push_instruction(X86Opcode::CallFar, operands, call_flags(effects), output)
            }
            TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            } => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let delivered =
                    self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                operands.push(fixed_virtual_operand(
                    delivered,
                    OperandRole::Def,
                    X86Register::Ax,
                ));
                self.push_instruction(X86Opcode::CallFar, operands, call_flags(effects), output)?;
                let pointer = self.define_register_value(result)?;
                self.copy(pointer, delivered, output)
            }
            TypeKind::Integer { bits: 32 } => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let low = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                let high = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                operands.push(fixed_virtual_operand(
                    low,
                    OperandRole::Def,
                    X86Register::Ax,
                ));
                operands.push(fixed_virtual_operand(
                    high,
                    OperandRole::Def,
                    X86Register::Dx,
                ));
                self.push_instruction(X86Opcode::CallFar, operands, call_flags(effects), output)?;
                let joined = self.define_register_value(result)?;
                self.push_instruction(
                    X86Opcode::MergeWords,
                    vec![
                        virtual_operand(joined, OperandRole::Def),
                        virtual_operand(low, OperandRole::Use),
                        virtual_operand(high, OperandRole::Use),
                    ],
                    InstructionFlags::NONE,
                    output,
                )
            }
            _ => Err(SelectionError::UnsupportedCallResult {
                function: self.function.id,
                block,
                instruction: instruction.id,
                callee: target.id,
                result: target.signature.result,
                values: instruction.results.len(),
            }),
        }
    }

    fn select_caller_cleanup_call_result(
        &mut self,
        block: BlockId,
        instruction: &Instruction,
        target: &Function,
        effects: Effects,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let opcode = match target.signature.calling_convention {
            CallingConvention::C => X86Opcode::CallNear,
            CallingConvention::FarCdecl => X86Opcode::CallFar,
            CallingConvention::FarPascal => {
                return Err(SelectionError::UnsupportedCallTarget {
                    function: self.function.id,
                    block,
                    instruction: instruction.id,
                    callee: target.id,
                });
            }
        };
        let mut operands = vec![if target.blocks.is_empty() {
            external_symbol_operand(target.name.clone())
        } else {
            function_operand(MachineFunctionId::new(target.id.get()))
        }];
        match self.type_kind(target.signature.result)? {
            TypeKind::Void => {
                if !instruction.results.is_empty() {
                    return Err(SelectionError::UnsupportedCallResult {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        callee: target.id,
                        result: target.signature.result,
                        values: instruction.results.len(),
                    });
                }
            }
            TypeKind::Integer { bits: 16 } => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let result = self.define_register_value(result)?;
                operands.push(fixed_virtual_operand(
                    result,
                    OperandRole::Def,
                    X86Register::Ax,
                ));
            }
            TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            } => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let delivered =
                    self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                operands.push(fixed_virtual_operand(
                    delivered,
                    OperandRole::Def,
                    X86Register::Ax,
                ));
                self.push_instruction(opcode, operands, call_flags(effects), output)?;
                let pointer = self.define_register_value(result)?;
                self.copy(pointer, delivered, output)?;
                return self.select_caller_cleanup(target, output);
            }
            TypeKind::Integer { bits: 32 } => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let low = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                let high = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                operands.push(fixed_virtual_operand(
                    low,
                    OperandRole::Def,
                    X86Register::Ax,
                ));
                operands.push(fixed_virtual_operand(
                    high,
                    OperandRole::Def,
                    X86Register::Dx,
                ));
                self.push_instruction(opcode, operands, call_flags(effects), output)?;
                let joined = self.define_register_value(result)?;
                self.push_instruction(
                    X86Opcode::MergeWords,
                    vec![
                        virtual_operand(joined, OperandRole::Def),
                        virtual_operand(low, OperandRole::Use),
                        virtual_operand(high, OperandRole::Use),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
                return self.select_caller_cleanup(target, output);
            }
            TypeKind::Float(_) => {
                let result = self.result_definition(block, instruction)?;
                if result.type_id != target.signature.result {
                    return Err(SelectionError::OperandTypeMismatch {
                        function: self.function.id,
                        block,
                        instruction: instruction.id,
                        expected: target.signature.result,
                        actual: result.type_id,
                    });
                }
                let result = self.define_register_value(result)?;
                operands.push(fixed_virtual_operand(
                    result,
                    OperandRole::Def,
                    X86Register::St0,
                ));
            }
            _ => {
                return Err(SelectionError::UnsupportedCallResult {
                    function: self.function.id,
                    block,
                    instruction: instruction.id,
                    callee: target.id,
                    result: target.signature.result,
                    values: instruction.results.len(),
                });
            }
        }
        self.push_instruction(opcode, operands, call_flags(effects), output)?;
        self.select_caller_cleanup(target, output)
    }

    fn select_caller_cleanup(
        &mut self,
        target: &Function,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(), SelectionError> {
        let cleanup = self.argument_bytes_for(&target.signature.parameters)?;
        if cleanup == 0 {
            return Ok(());
        }
        self.push_instruction(
            X86Opcode::Add,
            vec![
                physical_operand(X86Register::Sp, OperandRole::UseDef),
                immediate_operand(i64::from(cleanup)),
            ],
            InstructionFlags::NONE,
            output,
        )
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
                let Some(selected) = self.values.get(value).cloned() else {
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
                match &constant.value {
                    Constant::Integer(_) => {
                        self.materialize_integer(block, instruction, constant, output)
                    }
                    Constant::GlobalAddress { global, addend } => {
                        self.require_near_pointer(constant.type_id)?;
                        let Some(global) = self.globals.get(global) else {
                            return Err(SelectionError::UnknownGlobal {
                                function: self.function.id,
                                block,
                                instruction,
                                global: *global,
                            });
                        };
                        Ok(SelectedValue {
                            location: SelectedLocation::Global {
                                name: global.name.clone(),
                                addend: *addend,
                            },
                            type_id: constant.type_id,
                        })
                    }
                    Constant::Float(value) => self.materialize_float_constant(
                        block,
                        instruction,
                        constant.type_id,
                        value,
                        output,
                    ),
                    _ => Err(SelectionError::UnsupportedConstant {
                        function: self.function.id,
                        block,
                        instruction,
                    }),
                }
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
            location: SelectedLocation::Register(register),
            type_id: constant.type_id,
        })
    }

    fn materialize_float_constant(
        &mut self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        type_id: TypeId,
        value: &str,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<SelectedValue, SelectionError> {
        if let Some(opcode) = exact_x87_constant_opcode(type_id, self.float_kind(type_id)?, value)?
        {
            let register = self.fresh_virtual_register(X86RegisterClass::X87.machine_class())?;
            self.push_instruction(
                opcode,
                vec![virtual_operand(register, OperandRole::Def)],
                InstructionFlags::NONE,
                output,
            )?;
            return Ok(SelectedValue {
                location: SelectedLocation::Register(register),
                type_id,
            });
        }
        let Some(constant) = self
            .float_constants
            .get(&(type_id, value.to_owned()))
            .cloned()
        else {
            return Err(SelectionError::UnsupportedConstant {
                function: self.function.id,
                block,
                instruction,
            });
        };
        let register = self.fresh_virtual_register(X86RegisterClass::X87.machine_class())?;
        self.push_instruction(
            X86Opcode::X87Load,
            vec![
                virtual_operand(register, OperandRole::Def),
                float_format_operand(constant.format),
                MachineOperand {
                    kind: MachineOperandKind::Global {
                        name: constant.name,
                        addend: 0,
                    },
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
            ],
            load_flags(false),
            output,
        )?;
        Ok(SelectedValue {
            location: SelectedLocation::Register(register),
            type_id,
        })
    }

    fn select_terminator(
        &mut self,
        block: &Block,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(Option<MachineInstruction>, Vec<MachineBlockId>), SelectionError> {
        match &block.terminator {
            Terminator::Branch {
                condition: Operand::Value(condition),
                then_block,
                else_block,
            } => {
                if !self.block_ids.contains(then_block) {
                    return Err(SelectionError::UnknownJumpTarget {
                        function: self.function.id,
                        block: block.id,
                        target: *then_block,
                    });
                }
                if !self.block_ids.contains(else_block) {
                    return Err(SelectionError::UnknownJumpTarget {
                        function: self.function.id,
                        block: block.id,
                        target: *else_block,
                    });
                }
                let comparison = self.comparisons.get(condition).copied().ok_or(
                    SelectionError::UnsupportedTerminator {
                        function: self.function.id,
                        block: block.id,
                    },
                )?;
                let then_block = MachineBlockId::new(then_block.get());
                let else_block = MachineBlockId::new(else_block.get());
                let condition = match comparison {
                    PendingComparison::Integer {
                        left,
                        right,
                        condition,
                    } => {
                        self.push_instruction(
                            X86Opcode::Cmp,
                            vec![
                                virtual_operand(left, OperandRole::Use),
                                virtual_operand(right, OperandRole::Use),
                            ],
                            InstructionFlags::NONE,
                            output,
                        )?;
                        condition
                    }
                    PendingComparison::Float {
                        left,
                        right,
                        condition,
                    } => {
                        self.push_instruction(
                            X86Opcode::X87Compare,
                            vec![
                                virtual_operand(left, OperandRole::Use),
                                virtual_operand(right, OperandRole::Use),
                            ],
                            InstructionFlags::NONE,
                            output,
                        )?;
                        condition
                    }
                };
                self.push_instruction(
                    X86Opcode::JumpConditional,
                    vec![
                        immediate_operand(i64::from(condition as u8)),
                        block_operand(then_block),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
                let jump = self.machine_instruction(
                    X86Opcode::Jump,
                    vec![block_operand(else_block)],
                    InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                )?;
                Ok((Some(jump), vec![then_block, else_block]))
            }
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
                Ok((Some(instruction), vec![target]))
            }
            Terminator::Return(Some(value)) => {
                if matches!(
                    self.function.signature.calling_convention,
                    CallingConvention::C | CallingConvention::FarCdecl
                ) {
                    return self.select_c_return(block, value, output);
                }
                let result_kind = self.type_kind(self.function.signature.result)?;
                let near_pointer = matches!(
                    result_kind,
                    TypeKind::Pointer {
                        address_space: AddressSpace::NearData,
                    }
                );
                let bits = match result_kind {
                    TypeKind::Integer { bits: 16 }
                    | TypeKind::Pointer {
                        address_space: AddressSpace::NearData,
                    } => 16,
                    TypeKind::Integer { bits: 32 } => 32,
                    _ => {
                        return Err(SelectionError::UnsupportedReturnValue {
                            function: self.function.id,
                            block: block.id,
                        });
                    }
                };
                let Operand::Value(value) = value else {
                    return Err(SelectionError::UnsupportedReturnValue {
                        function: self.function.id,
                        block: block.id,
                    });
                };
                let Some(selected) = self.values.get(value).cloned() else {
                    return Err(SelectionError::UnsupportedReturnValue {
                        function: self.function.id,
                        block: block.id,
                    });
                };
                if selected.type_id != self.function.signature.result {
                    return Err(SelectionError::UnsupportedReturnValue {
                        function: self.function.id,
                        block: block.id,
                    });
                }
                let value = self.materialize_register(selected, output)?;
                let mut operands = if near_pointer {
                    let delivered =
                        self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                    self.copy(delivered, value, output)?;
                    vec![fixed_virtual_operand(
                        delivered,
                        OperandRole::Use,
                        X86Register::Ax,
                    )]
                } else if bits == 16 {
                    vec![fixed_virtual_operand(
                        value,
                        OperandRole::Use,
                        X86Register::Ax,
                    )]
                } else {
                    let low =
                        self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                    let high =
                        self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
                    self.push_instruction(
                        X86Opcode::LowWord,
                        vec![
                            fixed_virtual_operand(low, OperandRole::Def, X86Register::Ax),
                            virtual_operand(value, OperandRole::Use),
                        ],
                        InstructionFlags::NONE,
                        output,
                    )?;
                    self.push_instruction(
                        X86Opcode::HighWord,
                        vec![
                            fixed_virtual_operand(high, OperandRole::Def, X86Register::Dx),
                            virtual_operand(value, OperandRole::Use),
                        ],
                        InstructionFlags::NONE,
                        output,
                    )?;
                    vec![
                        fixed_virtual_operand(low, OperandRole::Use, X86Register::Ax),
                        fixed_virtual_operand(high, OperandRole::Use, X86Register::Dx),
                    ]
                };
                operands.push(immediate_operand(i64::from(self.argument_bytes()?)));
                let return_far = self.machine_instruction(
                    X86Opcode::ReturnFar,
                    operands,
                    InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                )?;
                Ok((Some(return_far), Vec::new()))
            }
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
                let (opcode, operands) = match self.function.signature.calling_convention {
                    CallingConvention::C => (X86Opcode::ReturnNear, Vec::new()),
                    CallingConvention::FarCdecl => {
                        (X86Opcode::ReturnFar, vec![immediate_operand(0)])
                    }
                    CallingConvention::FarPascal => {
                        let far = self.function.linkage == Linkage::External;
                        let operands = if far {
                            vec![immediate_operand(i64::from(self.argument_bytes()?))]
                        } else {
                            Vec::new()
                        };
                        (
                            if far {
                                X86Opcode::ReturnFar
                            } else {
                                X86Opcode::ReturnNear
                            },
                            operands,
                        )
                    }
                };
                let instruction = self.machine_instruction(
                    opcode,
                    operands,
                    InstructionFlags {
                        terminator: true,
                        ..InstructionFlags::NONE
                    },
                )?;
                Ok((Some(instruction), Vec::new()))
            }
            Terminator::Unreachable => Ok((None, Vec::new())),
            Terminator::Branch { .. } | Terminator::Switch { .. } => {
                Err(SelectionError::UnsupportedTerminator {
                    function: self.function.id,
                    block: block.id,
                })
            }
        }
    }

    fn select_c_return(
        &mut self,
        block: &Block,
        value: &Operand,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<(Option<MachineInstruction>, Vec<MachineBlockId>), SelectionError> {
        let result_kind = self.type_kind(self.function.signature.result)?;
        if matches!(result_kind, TypeKind::Float(_)) {
            let result_type = self.function.signature.result;
            let selected = self.select_operand(
                block.id,
                crate::ir::InstructionId::new(0),
                value,
                result_type,
                output,
            )?;
            let value = self.materialize_register(selected, output)?;
            let mut operands = vec![fixed_virtual_operand(
                value,
                OperandRole::Use,
                X86Register::St0,
            )];
            let opcode = match self.function.signature.calling_convention {
                CallingConvention::C => X86Opcode::ReturnNear,
                CallingConvention::FarCdecl => {
                    operands.push(immediate_operand(0));
                    X86Opcode::ReturnFar
                }
                CallingConvention::FarPascal => {
                    return Err(SelectionError::UnsupportedReturnValue {
                        function: self.function.id,
                        block: block.id,
                    });
                }
            };
            let instruction = self.machine_instruction(
                opcode,
                operands,
                InstructionFlags {
                    terminator: true,
                    ..InstructionFlags::NONE
                },
            )?;
            return Ok((Some(instruction), Vec::new()));
        }
        let near_pointer = matches!(
            result_kind,
            TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            }
        );
        let bits = match result_kind {
            TypeKind::Integer { bits: 16 }
            | TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            } => 16,
            TypeKind::Integer { bits: 32 } => 32,
            _ => {
                return Err(SelectionError::UnsupportedReturnValue {
                    function: self.function.id,
                    block: block.id,
                });
            }
        };
        let Operand::Value(value) = value else {
            return Err(SelectionError::UnsupportedReturnValue {
                function: self.function.id,
                block: block.id,
            });
        };
        let Some(selected) = self.values.get(value).cloned() else {
            return Err(SelectionError::UnsupportedReturnValue {
                function: self.function.id,
                block: block.id,
            });
        };
        if selected.type_id != self.function.signature.result {
            return Err(SelectionError::UnsupportedReturnValue {
                function: self.function.id,
                block: block.id,
            });
        }
        let value = self.materialize_register(selected, output)?;
        let mut operands = if near_pointer {
            let delivered = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
            self.copy(delivered, value, output)?;
            vec![fixed_virtual_operand(
                delivered,
                OperandRole::Use,
                X86Register::Ax,
            )]
        } else if bits == 16 {
            vec![fixed_virtual_operand(
                value,
                OperandRole::Use,
                X86Register::Ax,
            )]
        } else {
            // Open Watcom's 16-bit C conventions return a 32-bit integer in
            // DX:AX.  Python's cfront splits the whole value in `ret`, and
            // backend::lower places those two words through `_RETURNED` in
            // AX then DX.  Keep the portable IR value whole until this target
            // boundary, then reproduce that placement exactly.
            let low = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
            let high = self.fresh_virtual_register(X86RegisterClass::Word.machine_class())?;
            self.push_instruction(
                X86Opcode::LowWord,
                vec![
                    fixed_virtual_operand(low, OperandRole::Def, X86Register::Ax),
                    virtual_operand(value, OperandRole::Use),
                ],
                InstructionFlags::NONE,
                output,
            )?;
            self.push_instruction(
                X86Opcode::HighWord,
                vec![
                    fixed_virtual_operand(high, OperandRole::Def, X86Register::Dx),
                    virtual_operand(value, OperandRole::Use),
                ],
                InstructionFlags::NONE,
                output,
            )?;
            vec![
                fixed_virtual_operand(low, OperandRole::Use, X86Register::Ax),
                fixed_virtual_operand(high, OperandRole::Use, X86Register::Dx),
            ]
        };
        let opcode = match self.function.signature.calling_convention {
            CallingConvention::C => X86Opcode::ReturnNear,
            CallingConvention::FarCdecl => {
                operands.push(immediate_operand(0));
                X86Opcode::ReturnFar
            }
            CallingConvention::FarPascal => {
                return Err(SelectionError::UnsupportedReturnValue {
                    function: self.function.id,
                    block: block.id,
                });
            }
        };
        let instruction = self.machine_instruction(
            opcode,
            operands,
            InstructionFlags {
                terminator: true,
                ..InstructionFlags::NONE
            },
        )?;
        Ok((Some(instruction), Vec::new()))
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

    fn insert_value(
        &mut self,
        value: &Value,
        location: SelectedLocation,
    ) -> Result<(), SelectionError> {
        if self.values.contains_key(&value.id) {
            return Err(SelectionError::DuplicateValue {
                function: self.function.id,
                value: value.id,
            });
        }
        self.values.insert(
            value.id,
            SelectedValue {
                location,
                type_id: value.type_id,
            },
        );
        Ok(())
    }

    fn define_register_value(
        &mut self,
        value: &Value,
    ) -> Result<VirtualRegisterId, SelectionError> {
        let register = self.fresh_virtual_register(self.value_class(value.type_id)?)?;
        self.insert_value(value, SelectedLocation::Register(register))?;
        Ok(register)
    }

    fn materialize_register(
        &mut self,
        value: SelectedValue,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<VirtualRegisterId, SelectionError> {
        match value.location {
            SelectedLocation::Register(register) => Ok(register),
            SelectedLocation::RegisterOffset { base, addend } => {
                if addend == 0 {
                    return Ok(base);
                }
                self.require_near_pointer(value.type_id)?;
                let register =
                    self.fresh_virtual_register(X86RegisterClass::Address16.machine_class())?;
                self.copy(register, base, output)?;
                self.push_instruction(
                    X86Opcode::Add,
                    vec![
                        virtual_operand(register, OperandRole::UseDef),
                        immediate_operand(addend),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
                Ok(register)
            }
            location @ (SelectedLocation::Frame { .. } | SelectedLocation::Global { .. }) => {
                self.require_near_pointer(value.type_id)?;
                let register =
                    self.fresh_virtual_register(X86RegisterClass::Address16.machine_class())?;
                self.push_instruction(
                    X86Opcode::Lea,
                    vec![
                        virtual_operand(register, OperandRole::Def),
                        selected_address_operand(location),
                    ],
                    InstructionFlags::NONE,
                    output,
                )?;
                Ok(register)
            }
        }
    }

    fn memory_address_operands(&self, value: SelectedValue) -> Vec<MachineOperand> {
        match value.location {
            SelectedLocation::RegisterOffset { base, addend } if addend != 0 => vec![
                virtual_operand(base, OperandRole::Use),
                immediate_operand(addend),
            ],
            SelectedLocation::RegisterOffset { base, .. } => {
                vec![virtual_operand(base, OperandRole::Use)]
            }
            location => vec![selected_address_operand(location)],
        }
    }

    fn offset_near_address(
        &mut self,
        value: SelectedValue,
        offset: i64,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<SelectedLocation, SelectionError> {
        let offset = signed_i16(offset);
        match value.location {
            SelectedLocation::Register(base) => Ok(SelectedLocation::RegisterOffset {
                base,
                addend: offset,
            }),
            SelectedLocation::RegisterOffset { base, addend } => {
                Ok(SelectedLocation::RegisterOffset {
                    base,
                    addend: signed_i16(addend + offset),
                })
            }
            // A local field remains an abstract frame reference so frame
            // planning can combine its slot and field offsets exactly once.
            SelectedLocation::Frame { index, addend } => Ok(SelectedLocation::Frame {
                index,
                addend: signed_i16(addend + offset),
            }),
            // `addressforms.selected` intentionally does not move an offset
            // into SEGMENT/EXTERNAL relocations.  Keep the existing symbolic
            // LEA plus arithmetic for a pointer-valued global GEP.
            location @ SelectedLocation::Global { .. } => {
                let base = self.materialize_register(
                    SelectedValue {
                        location,
                        type_id: value.type_id,
                    },
                    output,
                )?;
                Ok(SelectedLocation::RegisterOffset {
                    base,
                    addend: offset,
                })
            }
        }
    }

    /// Materializes one far 16:16 dword immediately before its ES-relative use.
    ///
    /// This is Python backend `lower._pointer_access`'s exact sequence: save ES,
    /// push the packed pointer, pop its low word into an address register, and
    /// pop its high word into ES. Keeping the packed value whole until the push
    /// lets allocation spill or rematerialize it without breaking a later
    /// pattern recognizer.
    fn extract_far_address(
        &mut self,
        pointer: SelectedValue,
        output: &mut Vec<MachineInstruction>,
    ) -> Result<VirtualRegisterId, SelectionError> {
        let pointer = self.materialize_register(pointer, output)?;
        let offset = self.fresh_virtual_register(X86RegisterClass::Address16.machine_class())?;
        self.push_instruction(
            X86Opcode::Push,
            vec![physical_operand(X86Register::Es, OperandRole::Use)],
            InstructionFlags::NONE,
            output,
        )?;
        self.push_instruction(
            X86Opcode::Push,
            vec![virtual_operand(pointer, OperandRole::Use)],
            InstructionFlags::NONE,
            output,
        )?;
        self.push_instruction(
            X86Opcode::Pop,
            vec![virtual_operand(offset, OperandRole::Def)],
            InstructionFlags::NONE,
            output,
        )?;
        self.push_instruction(
            X86Opcode::Pop,
            vec![physical_operand(X86Register::Es, OperandRole::Def)],
            InstructionFlags::NONE,
            output,
        )?;
        Ok(offset)
    }

    fn restore_es(&mut self, output: &mut Vec<MachineInstruction>) -> Result<(), SelectionError> {
        self.push_instruction(
            X86Opcode::Pop,
            vec![physical_operand(X86Register::Es, OperandRole::Def)],
            InstructionFlags::NONE,
            output,
        )
    }

    fn fresh_frame_object(
        &mut self,
        size: u32,
        alignment: u32,
        kind: FrameObjectKind,
    ) -> Result<FrameIndex, SelectionError> {
        let index = FrameIndex::new(Self::fresh_id(
            self.function.id,
            &mut self.next_frame_index,
        )?);
        self.frame_objects.push(FrameObject {
            index,
            size,
            alignment,
            kind,
        });
        Ok(index)
    }

    fn operand_type(
        &self,
        block: BlockId,
        instruction: crate::ir::InstructionId,
        operand: &Operand,
    ) -> Result<TypeId, SelectionError> {
        match operand {
            Operand::Value(value) => {
                self.values
                    .get(value)
                    .map(|one| one.type_id)
                    .ok_or(SelectionError::UnknownValue {
                        function: self.function.id,
                        block,
                        instruction,
                        value: *value,
                    })
            }
            Operand::Constant(constant) => Ok(constant.type_id),
        }
    }

    fn require_pointer_type(
        &self,
        type_id: TypeId,
        expected: AddressSpace,
    ) -> Result<(), SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Pointer { address_space } if *address_space == expected => Ok(()),
            _ => Err(SelectionError::UnsupportedType { type_id }),
        }
    }

    fn require_near_pointer(&self, type_id: TypeId) -> Result<(), SelectionError> {
        self.require_pointer_type(type_id, AddressSpace::NearData)
    }

    fn pointer_address_space(&self, type_id: TypeId) -> Result<AddressSpace, SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Pointer { address_space } => Ok(*address_space),
            _ => Err(SelectionError::UnsupportedType { type_id }),
        }
    }

    fn require_1616_pointer(&self, type_id: TypeId) -> Result<(), SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Pointer {
                address_space:
                    AddressSpace::Generic
                    | AddressSpace::FarData
                    | AddressSpace::HugeData
                    | AddressSpace::Code,
            } => Ok(()),
            TypeKind::Pointer { address_space } => Err(SelectionError::UnsupportedAddressSpace {
                type_id,
                address_space: *address_space,
            }),
            _ => Err(SelectionError::UnsupportedType { type_id }),
        }
    }

    fn value_class(&self, type_id: TypeId) -> Result<RegisterClass, SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Float(_) => Ok(X86RegisterClass::X87.machine_class()),
            TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            } => Ok(X86RegisterClass::Address16.machine_class()),
            TypeKind::Pointer { address_space } => Err(SelectionError::UnsupportedAddressSpace {
                type_id,
                address_space: *address_space,
            }),
            _ => self.integer_class(type_id),
        }
    }

    fn abi_size(&self, type_id: TypeId) -> Result<(u32, u32), SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Float(FloatKind::Binary32) => Ok((4, 2)),
            TypeKind::Float(FloatKind::Binary64) => Ok((8, 2)),
            TypeKind::Float(FloatKind::Extended80) => Ok((10, 2)),
            TypeKind::Integer { bits: 8 | 16 } => Ok((2, 2)),
            TypeKind::Integer { bits: 32 } => Ok((4, 2)),
            TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            } => Ok((2, 2)),
            TypeKind::Integer { bits } => Err(SelectionError::UnsupportedIntegerWidth {
                type_id,
                bits: *bits,
            }),
            TypeKind::Pointer { address_space } => Err(SelectionError::UnsupportedAddressSpace {
                type_id,
                address_space: *address_space,
            }),
            _ => Err(SelectionError::UnsupportedType { type_id }),
        }
    }

    fn argument_bytes(&self) -> Result<u32, SelectionError> {
        self.argument_bytes_for(&self.function.signature.parameters)
    }

    fn argument_bytes_for(&self, parameters: &[TypeId]) -> Result<u32, SelectionError> {
        parameters.iter().try_fold(0_u32, |total, type_id| {
            let (size, _) = self.abi_size(*type_id)?;
            total.checked_add(size).ok_or(SelectionError::IdExhausted {
                function: self.function.id,
            })
        })
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

    fn float_kind(&self, type_id: TypeId) -> Result<FloatKind, SelectionError> {
        match self.type_kind(type_id)? {
            TypeKind::Float(kind) => Ok(*kind),
            _ => Err(SelectionError::UnsupportedType { type_id }),
        }
    }

    fn float_format(&self, type_id: TypeId) -> Result<X87MemoryFormat, SelectionError> {
        Ok(match self.float_kind(type_id)? {
            FloatKind::Binary32 => X87MemoryFormat::Float32,
            FloatKind::Binary64 => X87MemoryFormat::Float64,
            FloatKind::Extended80 => X87MemoryFormat::Float80,
        })
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

    fn finish(self) -> Result<MachineFunction, SelectionError> {
        Ok(MachineFunction {
            id: MachineFunctionId::new(self.function.id.get()),
            name: self.function.name.clone(),
            linkage: machine_linkage(self.function.linkage),
            signature: machine_signature(self.function, self.types)?,
            entry: self.entry,
            virtual_registers: self.virtual_registers,
            blocks: self.blocks,
            frame_objects: self.frame_objects,
        })
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

fn fixed_virtual_operand(
    register: VirtualRegisterId,
    role: OperandRole,
    fixed: X86Register,
) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Virtual(register)),
        role,
        constraint: Some(RegisterConstraint::Fixed(fixed.physical())),
        tied_to: None,
    }
}

fn physical_operand(register: X86Register, role: OperandRole) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Register(MachineRegister::Physical(register.physical())),
        role,
        constraint: None,
        tied_to: None,
    }
}

fn selected_address_operand(location: SelectedLocation) -> MachineOperand {
    match location {
        SelectedLocation::Register(register) => virtual_operand(register, OperandRole::Use),
        // Offset locations are consumed by `memory_address_operands` or
        // materialized before any plain-address use.  This arm keeps the
        // helper total for the common base spelling.
        SelectedLocation::RegisterOffset { base, addend: 0 } => {
            virtual_operand(base, OperandRole::Use)
        }
        SelectedLocation::RegisterOffset { .. } => {
            unreachable!("a nonzero selected offset needs a memory tail or materialization")
        }
        SelectedLocation::Frame { index, addend } => frame_operand(index, addend),
        SelectedLocation::Global { name, addend } => MachineOperand {
            kind: MachineOperandKind::Global { name, addend },
            role: OperandRole::None,
            constraint: None,
            tied_to: None,
        },
    }
}

fn frame_operand(index: FrameIndex, addend: i64) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::FrameIndex { index, addend },
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

/// x86 near offsets are 16-bit quantities.  Keep the same signed spelling as
/// Python's `addressforms.selected`: a chain can cross the signed boundary,
/// but its encoded displacement is always the wrapped low word.
fn signed_i16(value: i64) -> i64 {
    (value + 32_768).rem_euclid(65_536) - 32_768
}

fn immediate_operand(value: i64) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Immediate(value),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn float_format_operand(format: X87MemoryFormat) -> MachineOperand {
    immediate_operand(i64::from(format as u8))
}

const fn float_bytes(kind: FloatKind) -> u8 {
    match kind {
        FloatKind::Binary32 => 4,
        FloatKind::Binary64 => 8,
        FloatKind::Extended80 => 10,
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

fn function_operand(function: MachineFunctionId) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::Function(function),
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn external_symbol_operand(name: String) -> MachineOperand {
    MachineOperand {
        kind: MachineOperandKind::ExternalSymbol { name, addend: 0 },
        role: OperandRole::None,
        constraint: None,
        tied_to: None,
    }
}

fn call_flags(effects: Effects) -> InstructionFlags {
    let (may_load, may_store) = match effects.memory {
        MemoryEffects::None => (false, false),
        MemoryEffects::Read => (true, false),
        MemoryEffects::Write => (false, true),
        MemoryEffects::ReadWrite | MemoryEffects::Unknown => (true, true),
    };
    InstructionFlags {
        call: true,
        side_effects: !effects.is_pure(),
        may_load,
        may_store,
        volatile: effects.observable && (may_load || may_store),
        ..InstructionFlags::NONE
    }
}

fn load_flags(volatile: bool) -> InstructionFlags {
    InstructionFlags {
        may_load: true,
        volatile,
        ..InstructionFlags::NONE
    }
}

fn store_flags(volatile: bool) -> InstructionFlags {
    InstructionFlags {
        side_effects: true,
        may_store: true,
        volatile,
        ..InstructionFlags::NONE
    }
}

fn machine_linkage(linkage: Linkage) -> MachineLinkage {
    match linkage {
        Linkage::Internal => MachineLinkage::Internal,
        Linkage::External => MachineLinkage::External,
    }
}

fn machine_signature(
    function: &Function,
    types: &BTreeMap<TypeId, &TypeKind>,
) -> Result<MachineSignature, SelectionError> {
    Ok(MachineSignature {
        result: match type_kind(types, function.signature.result)? {
            TypeKind::Void => None,
            _ => Some(machine_value_type(types, function.signature.result)?),
        },
        parameters: function
            .signature
            .parameters
            .iter()
            .map(|type_id| machine_value_type(types, *type_id))
            .collect::<Result<Vec<_>, _>>()?,
        variadic: function.signature.variadic,
        calling_convention: match function.signature.calling_convention {
            CallingConvention::C => MachineCallingConvention::C,
            CallingConvention::FarCdecl => MachineCallingConvention::FarCdecl,
            CallingConvention::FarPascal => MachineCallingConvention::FarPascal,
        },
    })
}

fn machine_value_type(
    types: &BTreeMap<TypeId, &TypeKind>,
    type_id: TypeId,
) -> Result<MachineValueType, SelectionError> {
    match type_kind(types, type_id)? {
        TypeKind::Integer { bits } if *bits != 0 => Ok(MachineValueType::Integer { bits: *bits }),
        TypeKind::Float(kind) => Ok(MachineValueType::Float {
            kind: match kind {
                FloatKind::Binary32 => MachineFloatKind::Binary32,
                FloatKind::Binary64 => MachineFloatKind::Binary64,
                FloatKind::Extended80 => MachineFloatKind::Extended80,
            },
        }),
        TypeKind::Pointer { address_space } => Ok(MachineValueType::Pointer {
            bits: match address_space {
                AddressSpace::NearData | AddressSpace::Segment => 16,
                AddressSpace::Generic
                | AddressSpace::FarData
                | AddressSpace::HugeData
                | AddressSpace::Code => 32,
            },
            address_space: match address_space {
                AddressSpace::Generic => MachineAddressSpace::Generic,
                AddressSpace::NearData => MachineAddressSpace::NearData,
                AddressSpace::FarData => MachineAddressSpace::FarData,
                AddressSpace::HugeData => MachineAddressSpace::HugeData,
                AddressSpace::Code => MachineAddressSpace::Code,
                AddressSpace::Segment => MachineAddressSpace::Segment,
            },
        }),
        _ => Err(SelectionError::UnsupportedType { type_id }),
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
    const I1: TypeId = TypeId::new(2);
    const I16: TypeId = TypeId::new(4);
    const I8: TypeId = TypeId::new(5);
    const BYTES: TypeId = TypeId::new(6);

    fn signature(result: TypeId, parameters: Vec<TypeId>) -> crate::ir::Signature {
        Signature {
            result,
            parameters,
            variadic: false,
            calling_convention: CallingConvention::FarPascal,
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

    fn module_with_globals(types: Vec<Type>, globals: Vec<Global>) -> Module {
        Module {
            name: "selection-data".to_owned(),
            types,
            globals,
            functions: Vec::new(),
        }
    }

    #[test]
    fn selects_every_word_and_dword_integer_divide_projection_with_explicit_pair_effects() {
        // Python's DIVIDE_PAIR contract is quotient in AX/EAX and remainder
        // in DX/EDX, while the dividend is read high then low.  Each source
        // projection still has to define both architectural results.
        for (type_id, bits) in [(I16, 16), (I32, 32)] {
            for (op, signed, wants_remainder) in [
                (BinaryOp::SignedDivide, true, false),
                (BinaryOp::SignedRemainder, true, true),
                (BinaryOp::UnsignedDivide, false, false),
                (BinaryOp::UnsignedRemainder, false, true),
            ] {
                let left = Value {
                    id: ValueId::new(0),
                    type_id,
                };
                let right = Value {
                    id: ValueId::new(1),
                    type_id,
                };
                let result = Value {
                    id: ValueId::new(2),
                    type_id,
                };
                let input = module(
                    basic_types(),
                    vec![function(
                        vec![Block {
                            id: BlockId::new(0),
                            instructions: vec![Instruction {
                                id: crate::ir::InstructionId::new(0),
                                results: vec![result],
                                kind: InstructionKind::Binary {
                                    op,
                                    left: Operand::Value(left.id),
                                    right: Operand::Value(right.id),
                                },
                            }],
                            terminator: Terminator::Return(None),
                        }],
                        vec![left, right],
                    )],
                );
                let selected = select_module(&input).expect("integer division selects");
                super::super::verify_machine(&selected).expect("division Machine IR verifies");
                let instructions = &selected.functions[0].blocks[0].instructions;
                let divide = instructions
                    .iter()
                    .find(|instruction| {
                        instruction.opcode
                            == if signed {
                                X86Opcode::Idiv.machine_opcode()
                            } else {
                                X86Opcode::Div.machine_opcode()
                            }
                    })
                    .expect("selected div/idiv");
                assert_eq!(divide.operands.len(), 5);
                let (low, high) = if bits == 16 {
                    (X86Register::Ax, X86Register::Dx)
                } else {
                    (X86Register::Eax, X86Register::Edx)
                };
                for (operand, role, fixed) in [
                    (&divide.operands[0], OperandRole::Use, high),
                    (&divide.operands[1], OperandRole::Use, low),
                    (&divide.operands[3], OperandRole::Def, low),
                    (&divide.operands[4], OperandRole::Def, high),
                ] {
                    assert_eq!(operand.role, role);
                    assert_eq!(
                        operand.constraint,
                        Some(RegisterConstraint::Fixed(fixed.physical()))
                    );
                }
                assert_eq!(divide.operands[2].role, OperandRole::Use);
                assert_eq!(
                    divide.operands[2].constraint, None,
                    "divisor stays flexible"
                );
                assert!(instructions.iter().any(|instruction| {
                    instruction.opcode == X86Opcode::Copy.machine_opcode()
                        && instruction.operands[1].kind
                            == divide.operands[if wants_remainder { 4 } else { 3 }].kind
                }));
                if signed {
                    assert!(instructions.iter().any(
                        |instruction| instruction.opcode == X86Opcode::CwdCdq.machine_opcode()
                    ));
                } else {
                    assert!(instructions.iter().any(|instruction| {
                        instruction.opcode == X86Opcode::Mov.machine_opcode()
                            && instruction.operands[0].constraint
                                == Some(RegisterConstraint::Fixed(high.physical()))
                            && instruction.operands[1] == immediate_operand(0)
                    }));
                }
            }
        }
    }

    #[test]
    fn selects_signed_i32_constant_division_as_materialized_divisor_cdq_and_idiv() {
        // The qlight scale kernel has this ordinary shape: a signed dword
        // product divided by a positive integer constant.  This is a target
        // contract test, not a fixture-specific selection rule.
        let dividend = Value {
            id: ValueId::new(0),
            type_id: I32,
        };
        let result = Value {
            id: ValueId::new(1),
            type_id: I32,
        };
        let input = module(
            basic_types(),
            vec![function(
                vec![Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![result],
                        kind: InstructionKind::Binary {
                            op: BinaryOp::SignedDivide,
                            left: Operand::Value(dividend.id),
                            right: Operand::Constant(TypedConstant {
                                type_id: I32,
                                value: Constant::Integer(120),
                            }),
                        },
                    }],
                    terminator: Terminator::Return(None),
                }],
                vec![dividend],
            )],
        );
        let selected = select_module(&input).expect("signed i32 division selects");
        let instructions = &selected.functions[0].blocks[0].instructions;
        let idiv = instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::Idiv.machine_opcode())
            .expect("signed dword idiv");
        assert!(instructions.iter().any(|instruction| {
            instruction.opcode == X86Opcode::Mov.machine_opcode()
                && instruction.operands[1] == immediate_operand(120)
                && instruction.operands[0].kind == idiv.operands[2].kind
        }));
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.opcode == X86Opcode::CwdCdq.machine_opcode())
        );
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
            Type {
                id: I1,
                kind: TypeKind::Integer { bits: 1 },
            },
            Type {
                id: I16,
                kind: TypeKind::Integer { bits: 16 },
            },
        ]
    }

    fn byte_array_types(length: u64) -> Vec<Type> {
        let mut types = basic_types();
        types.extend([
            Type {
                id: I8,
                kind: TypeKind::Integer { bits: 8 },
            },
            Type {
                id: BYTES,
                kind: TypeKind::Array {
                    element: I8,
                    length,
                },
            },
        ]);
        types
    }

    fn near_pointer_type(id: TypeId) -> Type {
        Type {
            id,
            kind: TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            },
        }
    }

    fn compose_pointer_module(address_space: AddressSpace) -> Module {
        let pointer = TypeId::new(7);
        let segment = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let offset = Value {
            id: ValueId::new(1),
            type_id: I16,
        };
        let result = Value {
            id: ValueId::new(2),
            type_id: pointer,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![result],
                    kind: InstructionKind::ComposePointer {
                        segment: Operand::Value(segment.id),
                        offset: Operand::Value(offset.id),
                    },
                }],
                terminator: Terminator::Return(None),
            }],
            vec![segment, offset],
        );
        let mut types = basic_types();
        types.push(Type {
            id: pointer,
            kind: TypeKind::Pointer { address_space },
        });
        module(types, vec![function])
    }

    fn data_global(id: u32, name: &str, initializer: Constant) -> Global {
        Global {
            id: GlobalId::new(id),
            name: name.to_owned(),
            type_id: BYTES,
            linkage: Linkage::Internal,
            constant: true,
            initializer: Some(initializer),
            address_space: AddressSpace::NearData,
        }
    }

    #[test]
    fn selects_compose_pointer_as_offset_low_and_segment_high() {
        let input = compose_pointer_module(AddressSpace::FarData);
        input.verify().expect("compose pointer IR verifies");

        let selected = select_module(&input).expect("far 16:16 pointer selects");
        selected.verify().expect("selected Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        let merge = instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::MergeWords.machine_opcode())
            .expect("compose pointer selects to MergeWords");
        let [destination, low, high] = merge.operands.as_slice() else {
            panic!("MergeWords has destination, low, and high operands");
        };
        let virtual_register = |operand: &MachineOperand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register)) => register,
            _ => panic!("MergeWords operands are virtual registers"),
        };
        let segment = virtual_register(&instructions[0].operands[0]);
        let offset = virtual_register(&instructions[1].operands[0]);
        let destination = virtual_register(destination);
        let low = virtual_register(low);
        let high = virtual_register(high);

        assert_eq!(destination, VirtualRegisterId::new(2));
        assert_eq!(low, offset, "offset is the low word");
        assert_eq!(high, segment, "segment is the high word");
        assert_eq!(merge.operands[0].role, OperandRole::Def);
        assert_eq!(merge.operands[1].role, OperandRole::Use);
        assert_eq!(merge.operands[2].role, OperandRole::Use);
        let class = |register| {
            function
                .virtual_registers
                .iter()
                .find(|candidate| candidate.id == register)
                .expect("MergeWords register is declared")
                .class
        };
        assert_eq!(class(destination), X86RegisterClass::Dword.machine_class());
        assert_eq!(class(low), X86RegisterClass::Word.machine_class());
        assert_eq!(class(high), X86RegisterClass::Word.machine_class());
    }

    #[test]
    fn compose_pointer_refuses_near_result() {
        let input = compose_pointer_module(AddressSpace::NearData);
        input.verify().expect("compose pointer IR verifies");

        assert!(matches!(
            select_module(&input),
            Err(SelectionError::UnsupportedAddressSpace {
                type_id,
                address_space: AddressSpace::NearData,
            }) if type_id == TypeId::new(7)
        ));
    }

    fn far_memory_module(address_space: AddressSpace, store: bool) -> Module {
        let pointer = TypeId::new(7);
        let segment = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let offset = Value {
            id: ValueId::new(1),
            type_id: I16,
        };
        let address = Value {
            id: ValueId::new(2),
            type_id: pointer,
        };
        let mut instructions = vec![Instruction {
            id: crate::ir::InstructionId::new(0),
            results: vec![address.clone()],
            kind: InstructionKind::ComposePointer {
                segment: Operand::Value(segment.id),
                offset: Operand::Value(offset.id),
            },
        }];
        if store {
            instructions.push(Instruction {
                id: crate::ir::InstructionId::new(1),
                results: Vec::new(),
                kind: InstructionKind::Store {
                    address: Operand::Value(address.id),
                    value: Operand::Constant(TypedConstant {
                        type_id: I16,
                        value: Constant::Integer(29),
                    }),
                    alignment: 2,
                    volatile: false,
                },
            });
        } else {
            instructions.push(Instruction {
                id: crate::ir::InstructionId::new(1),
                results: vec![Value {
                    id: ValueId::new(3),
                    type_id: I16,
                }],
                kind: InstructionKind::Load {
                    address: Operand::Value(address.id),
                    alignment: 2,
                    volatile: false,
                },
            });
        }
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions,
                terminator: Terminator::Return(None),
            }],
            vec![segment, offset],
        );
        let mut types = basic_types();
        types.push(Type {
            id: pointer,
            kind: TypeKind::Pointer { address_space },
        });
        module(types, vec![function])
    }

    fn far_float_memory_module(store: bool) -> Module {
        let pointer = TypeId::new(7);
        let float = TypeId::new(8);
        let segment = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let offset = Value {
            id: ValueId::new(1),
            type_id: I16,
        };
        let address = Value {
            id: ValueId::new(2),
            type_id: pointer,
        };
        let value = Value {
            id: ValueId::new(3),
            type_id: float,
        };
        let mut instructions = vec![Instruction {
            id: crate::ir::InstructionId::new(0),
            results: vec![address.clone()],
            kind: InstructionKind::ComposePointer {
                segment: Operand::Value(segment.id),
                offset: Operand::Value(offset.id),
            },
        }];
        instructions.push(if store {
            Instruction {
                id: crate::ir::InstructionId::new(1),
                results: Vec::new(),
                kind: InstructionKind::Store {
                    address: Operand::Value(address.id),
                    value: Operand::Value(value.id),
                    alignment: 2,
                    volatile: false,
                },
            }
        } else {
            Instruction {
                id: crate::ir::InstructionId::new(1),
                results: vec![value.clone()],
                kind: InstructionKind::Load {
                    address: Operand::Value(address.id),
                    alignment: 2,
                    volatile: false,
                },
            }
        });
        let parameters = if store {
            vec![segment, offset, value]
        } else {
            vec![segment, offset]
        };
        let mut types = basic_types();
        types.extend([
            Type {
                id: pointer,
                kind: TypeKind::Pointer {
                    address_space: AddressSpace::FarData,
                },
            },
            Type {
                id: float,
                kind: TypeKind::Float(FloatKind::Binary32),
            },
        ]);
        module(
            types,
            vec![function(
                vec![Block {
                    id: BlockId::new(0),
                    instructions,
                    terminator: Terminator::Return(None),
                }],
                parameters,
            )],
        )
    }

    fn virtual_register_id(operand: &MachineOperand) -> VirtualRegisterId {
        match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register)) => register,
            _ => panic!("operand is a virtual register"),
        }
    }

    fn python_far_access(instructions: &[MachineInstruction], opcode: X86Opcode) -> usize {
        let access = instructions
            .iter()
            .position(|instruction| {
                instruction.opcode == opcode.machine_opcode()
                    && instruction.operands.last()
                        == Some(&physical_operand(X86Register::Es, OperandRole::Use))
            })
            .expect("far access uses ES");
        assert!(access >= 4, "far access has its four-instruction setup");
        assert_eq!(
            instructions[access - 4..access]
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Push.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::Pop.machine_opcode(),
                X86Opcode::Pop.machine_opcode(),
            ]
        );
        assert_eq!(
            instructions[access - 4].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Use)]
        );
        assert_eq!(
            instructions[access - 1].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Def)]
        );
        assert_eq!(
            instructions[access + 1].operands,
            vec![physical_operand(X86Register::Es, OperandRole::Def)]
        );
        access
    }

    #[test]
    fn far_memory_selects_word_load_through_occurrence_local_es() {
        let input = far_memory_module(AddressSpace::FarData, false);
        input.verify().expect("far load IR verifies");

        let selected = select_module(&input).expect("far word load selects");
        crate::target::x86::verify_machine(&selected).expect("far word load Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        let access = python_far_access(instructions, X86Opcode::Load);
        let [pointer] = instructions[access - 3].operands.as_slice() else {
            panic!("packed pointer push has one operand");
        };
        let [low] = instructions[access - 2].operands.as_slice() else {
            panic!("offset pop has one operand");
        };
        let [destination, offset, load_es] = instructions[access].operands.as_slice() else {
            panic!("segmented Load has destination, offset, and ES operands");
        };
        assert_eq!(pointer.role, OperandRole::Use);
        assert_eq!(low.role, OperandRole::Def);
        assert_eq!(virtual_register_id(low), virtual_register_id(offset));
        assert_eq!(destination.role, OperandRole::Def);
        assert_eq!(offset.role, OperandRole::Use);
        assert_eq!(
            load_es,
            &physical_operand(X86Register::Es, OperandRole::Use)
        );
        let class = |operand: &MachineOperand| {
            let register = virtual_register_id(operand);
            function
                .virtual_registers
                .iter()
                .find(|candidate| candidate.id == register)
                .expect("virtual register is declared")
                .class
        };
        assert_eq!(class(low), X86RegisterClass::Address16.machine_class());
        assert_eq!(class(pointer), X86RegisterClass::Dword.machine_class());
        assert_eq!(class(destination), X86RegisterClass::Word.machine_class());
    }

    #[test]
    fn far_memory_selects_word_store_after_materializing_source() {
        let input = far_memory_module(AddressSpace::FarData, true);
        input.verify().expect("far store IR verifies");

        let selected = select_module(&input).expect("far word store selects");
        crate::target::x86::verify_machine(&selected).expect("far word store Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        let access = python_far_access(instructions, X86Opcode::Store);
        let [pointer] = instructions[access - 3].operands.as_slice() else {
            panic!("packed pointer push has one operand");
        };
        let [low] = instructions[access - 2].operands.as_slice() else {
            panic!("offset pop has one operand");
        };
        let [offset, source, store_es] = instructions[access].operands.as_slice() else {
            panic!("segmented Store has offset, source, and ES operands");
        };
        let [materialized, _constant] = instructions[access - 5].operands.as_slice() else {
            panic!("stored constant materializes into one register");
        };
        assert_eq!(
            instructions[access - 5].opcode,
            X86Opcode::Mov.machine_opcode()
        );
        assert_eq!(pointer.role, OperandRole::Use);
        assert_eq!(low.role, OperandRole::Def);
        assert_eq!(virtual_register_id(low), virtual_register_id(offset));
        assert_eq!(
            virtual_register_id(materialized),
            virtual_register_id(source)
        );
        assert_eq!(offset.role, OperandRole::Use);
        assert_eq!(source.role, OperandRole::Use);
        assert_eq!(
            store_es,
            &physical_operand(X86Register::Es, OperandRole::Use)
        );
        let class = |operand: &MachineOperand| {
            let register = virtual_register_id(operand);
            function
                .virtual_registers
                .iter()
                .find(|candidate| candidate.id == register)
                .expect("virtual register is declared")
                .class
        };
        assert_eq!(class(low), X86RegisterClass::Address16.machine_class());
        assert_eq!(class(pointer), X86RegisterClass::Dword.machine_class());
        assert_eq!(class(source), X86RegisterClass::Word.machine_class());
    }

    #[test]
    fn far_memory_selects_float_load_and_store_through_occurrence_local_es() {
        for (store, opcode) in [(false, X86Opcode::X87Load), (true, X86Opcode::X87StorePop)] {
            let input = far_float_memory_module(store);
            input.verify().expect("far float memory IR verifies");

            let selected = select_module(&input).expect("far float memory access selects");
            selected.verify().expect("far float Machine IR verifies");
            let instructions = &selected.functions[0].blocks[0].instructions;
            python_far_access(instructions, opcode);
        }
    }

    #[test]
    fn far_memory_refuses_huge_data() {
        let input = far_memory_module(AddressSpace::HugeData, false);
        input.verify().expect("huge load IR verifies");

        assert!(matches!(
            select_module(&input),
            Err(SelectionError::UnsupportedAddressSpace {
                type_id,
                address_space: AddressSpace::HugeData,
            }) if type_id == TypeId::new(7)
        ));
    }

    #[test]
    fn selects_constant_byte_offset_get_element_pointer() {
        let near = TypeId::new(7);
        let base = Value {
            id: ValueId::new(0),
            type_id: near,
        };
        let zero = Value {
            id: ValueId::new(1),
            type_id: near,
        };
        let four = Value {
            id: ValueId::new(2),
            type_id: near,
        };
        let zero_load = Value {
            id: ValueId::new(3),
            type_id: I16,
        };
        let four_load = Value {
            id: ValueId::new(4),
            type_id: I16,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![base.clone()],
                        kind: InstructionKind::StackAlloc {
                            size: 8,
                            alignment: 2,
                            address_space: AddressSpace::NearData,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![zero.clone()],
                        kind: InstructionKind::GetElementPointer {
                            base: Operand::Value(base.id),
                            indices: vec![Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(0),
                            })],
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(2),
                        results: vec![zero_load],
                        kind: InstructionKind::Load {
                            address: Operand::Value(zero.id),
                            alignment: 2,
                            volatile: false,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(3),
                        results: vec![four.clone()],
                        kind: InstructionKind::GetElementPointer {
                            base: Operand::Value(base.id),
                            indices: vec![Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(4),
                            })],
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(4),
                        results: vec![four_load],
                        kind: InstructionKind::Load {
                            address: Operand::Value(four.id),
                            alignment: 2,
                            volatile: false,
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
            Vec::new(),
        );
        let mut types = basic_types();
        types.push(near_pointer_type(near));

        let selected = select_module(&module(types, vec![function])).expect("byte offset selects");
        selected.verify().expect("selected Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Load.machine_opcode(),
                X86Opcode::Load.machine_opcode(),
                X86Opcode::ReturnNear.machine_opcode(),
            ]
        );
        assert!(instructions.iter().all(|instruction| {
            !matches!(
                X86Opcode::from_machine_opcode(instruction.opcode),
                Some(X86Opcode::Copy | X86Opcode::Add)
            )
        }));
        assert_eq!(
            instructions[0].operands[1].kind,
            MachineOperandKind::FrameIndex {
                index: FrameIndex::new(0),
                addend: 0,
            }
        );
        assert_eq!(
            instructions[1].operands[1].kind,
            MachineOperandKind::FrameIndex {
                index: FrameIndex::new(0),
                addend: 4,
            }
        );
    }

    #[test]
    fn selects_dynamic_byte_offset_get_element_pointer() {
        let near = TypeId::new(7);
        let offset = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let base = Value {
            id: ValueId::new(1),
            type_id: near,
        };
        let constant = Value {
            id: ValueId::new(2),
            type_id: near,
        };
        let result = Value {
            id: ValueId::new(3),
            type_id: near,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![base.clone()],
                        kind: InstructionKind::StackAlloc {
                            size: 8,
                            alignment: 2,
                            address_space: AddressSpace::NearData,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![constant.clone()],
                        kind: InstructionKind::GetElementPointer {
                            base: Operand::Value(base.id),
                            indices: vec![Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(6),
                            })],
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(2),
                        results: vec![result],
                        kind: InstructionKind::GetElementPointer {
                            base: Operand::Value(constant.id),
                            indices: vec![Operand::Value(offset.id)],
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
            vec![offset],
        );
        let mut types = basic_types();
        types.push(near_pointer_type(near));

        let selected = select_module(&module(types, vec![function])).expect("byte offset selects");
        selected.verify().expect("selected Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Load.machine_opcode(),
                X86Opcode::Lea.machine_opcode(),
                X86Opcode::Copy.machine_opcode(),
                X86Opcode::Add.machine_opcode(),
                X86Opcode::ReturnNear.machine_opcode(),
            ]
        );
        assert_eq!(
            instructions[1].operands[1].kind,
            MachineOperandKind::FrameIndex {
                index: FrameIndex::new(1),
                addend: 6,
            },
            "a pointer-valued constant GEP materializes its folded frame address",
        );
        let [destination, source] = instructions[3].operands.as_slice() else {
            panic!("address addition has two operands");
        };
        let MachineOperandKind::Register(MachineRegister::Virtual(destination_register)) =
            destination.kind
        else {
            panic!("address addition updates a virtual register");
        };
        let MachineOperandKind::Register(MachineRegister::Virtual(source_register)) = source.kind
        else {
            panic!("dynamic offset uses a virtual register");
        };
        assert_eq!(destination.role, OperandRole::UseDef);
        assert_eq!(source.role, OperandRole::Use);
        assert!(function.virtual_registers.iter().any(|register| {
            register.id == destination_register
                && register.class == X86RegisterClass::Address16.machine_class()
        }));
        assert!(function.virtual_registers.iter().any(|register| {
            register.id == source_register
                && register.class == X86RegisterClass::Word.machine_class()
        }));
    }

    #[test]
    fn selects_far_byte_offset_without_changing_the_selector() {
        let far = TypeId::new(7);
        let segment = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let offset = Value {
            id: ValueId::new(1),
            type_id: I16,
        };
        let base = Value {
            id: ValueId::new(2),
            type_id: far,
        };
        let result = Value {
            id: ValueId::new(3),
            type_id: far,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![base.clone()],
                        kind: InstructionKind::ComposePointer {
                            segment: Operand::Value(segment.id),
                            offset: Operand::Value(offset.id),
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![result],
                        kind: InstructionKind::GetElementPointer {
                            base: Operand::Value(base.id),
                            indices: vec![Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(6),
                            })],
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
            vec![segment, offset],
        );
        let mut types = basic_types();
        types.push(Type {
            id: far,
            kind: TypeKind::Pointer {
                address_space: AddressSpace::FarData,
            },
        });
        let input = module(types, vec![function]);
        input.verify().expect("far byte offset IR verifies");

        let selected = select_module(&input).expect("far byte offset selects");
        selected.verify().expect("selected Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions[2..]
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::MergeWords.machine_opcode(),
                X86Opcode::LowWord.machine_opcode(),
                X86Opcode::Add.machine_opcode(),
                X86Opcode::HighWord.machine_opcode(),
                X86Opcode::MergeWords.machine_opcode(),
                X86Opcode::ReturnNear.machine_opcode(),
            ]
        );
        let virtual_register = |operand: &MachineOperand| match operand.kind {
            MachineOperandKind::Register(MachineRegister::Virtual(register)) => register,
            _ => panic!("far byte offset uses virtual registers"),
        };
        let [low, base_low] = instructions[3].operands.as_slice() else {
            panic!("LowWord has destination and base");
        };
        let [added, immediate] = instructions[4].operands.as_slice() else {
            panic!("Add has destination and offset");
        };
        let [high, base_high] = instructions[5].operands.as_slice() else {
            panic!("HighWord has destination and base");
        };
        let [merged, merged_low, merged_high] = instructions[6].operands.as_slice() else {
            panic!("MergeWords has destination, low, and high");
        };
        let low = virtual_register(low);
        let base_low = virtual_register(base_low);
        let added = virtual_register(added);
        let high = virtual_register(high);
        let base_high = virtual_register(base_high);
        let merged = virtual_register(merged);
        assert_eq!(
            base_low, base_high,
            "both halves come from the same packed pointer"
        );
        assert_eq!(low, added, "only the low word is advanced");
        assert_eq!(virtual_register(merged_low), low);
        assert_eq!(
            virtual_register(merged_high),
            high,
            "high word is unchanged"
        );
        assert_eq!(immediate, &immediate_operand(6));
        assert_eq!(instructions[3].operands[0].role, OperandRole::Def);
        assert_eq!(instructions[4].operands[0].role, OperandRole::UseDef);
        assert_eq!(instructions[5].operands[0].role, OperandRole::Def);
        assert_eq!(instructions[6].operands[0].role, OperandRole::Def);
        let class = |register| {
            function
                .virtual_registers
                .iter()
                .find(|candidate| candidate.id == register)
                .expect("selected register is declared")
                .class
        };
        assert_eq!(class(low), X86RegisterClass::Word.machine_class());
        assert_eq!(class(high), X86RegisterClass::Word.machine_class());
        assert_eq!(class(merged), X86RegisterClass::Dword.machine_class());
    }

    #[test]
    fn refuses_malformed_get_element_pointer() {
        let near = TypeId::new(7);
        let huge = TypeId::new(8);
        let result = Value {
            id: ValueId::new(0),
            type_id: near,
        };
        let gep = |indices| Function {
            id: FunctionId::new(4),
            name: "selected".to_owned(),
            signature: signature(VOID, Vec::new()),
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![result.clone()],
                    kind: InstructionKind::GetElementPointer {
                        base: Operand::Constant(TypedConstant {
                            type_id: near,
                            value: Constant::Null,
                        }),
                        indices,
                    },
                }],
                terminator: Terminator::Return(None),
            }],
        };
        let mut types = basic_types();
        types.extend([
            near_pointer_type(near),
            Type {
                id: huge,
                kind: TypeKind::Pointer {
                    address_space: AddressSpace::HugeData,
                },
            },
        ]);

        for indices in [
            Vec::new(),
            vec![
                Operand::Constant(TypedConstant {
                    type_id: I16,
                    value: Constant::Integer(0),
                }),
                Operand::Constant(TypedConstant {
                    type_id: I16,
                    value: Constant::Integer(1),
                }),
            ],
            vec![Operand::Constant(TypedConstant {
                type_id: I32,
                value: Constant::Integer(0),
            })],
        ] {
            assert!(matches!(
                select_module(&module(types.clone(), vec![gep(indices)])),
                Err(SelectionError::UnsupportedInstruction { .. })
            ));
        }

        let huge_result = Value {
            id: ValueId::new(0),
            type_id: huge,
        };
        let huge_gep = Function {
            id: FunctionId::new(4),
            name: "selected".to_owned(),
            signature: signature(VOID, Vec::new()),
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![huge_result],
                    kind: InstructionKind::GetElementPointer {
                        base: Operand::Constant(TypedConstant {
                            type_id: huge,
                            value: Constant::Null,
                        }),
                        indices: vec![Operand::Constant(TypedConstant {
                            type_id: I16,
                            value: Constant::Integer(0),
                        })],
                    },
                }],
                terminator: Terminator::Return(None),
            }],
        };
        assert!(matches!(
            select_module(&module(types, vec![huge_gep])),
            Err(SelectionError::UnsupportedAddressSpace {
                type_id,
                address_space: AddressSpace::HugeData,
            }) if type_id == huge
        ));
    }

    #[test]
    fn selects_hir_data_bytes_and_relocations_without_losing_layout_intent() {
        // Python HIR global planning preserves source object order and emits
        // symbolic patches verbatim. Segment patches and near-data patches
        // are both 16-bit fields but retain distinct address-space intent.
        let mut source = data_global(
            7,
            "source",
            Constant::RelocatableBytes {
                bytes: vec![0, 0, 0, 0],
                relocations: vec![
                    crate::ir::GlobalRelocation {
                        offset: 0,
                        target: GlobalId::new(9),
                        addend: -3,
                        width: 2,
                        address_space: AddressSpace::Segment,
                    },
                    crate::ir::GlobalRelocation {
                        offset: 2,
                        target: GlobalId::new(3),
                        addend: 11,
                        width: 2,
                        address_space: AddressSpace::NearData,
                    },
                ],
            },
        );
        source.address_space = AddressSpace::FarData;
        let target = data_global(3, "target", Constant::Bytes(vec![0, 1, 2, 3]));
        let segment_target = data_global(9, "segment-target", Constant::Bytes(vec![4, 5, 6, 7]));

        let selected = select_module(&module_with_globals(
            byte_array_types(4),
            vec![source, target, segment_target],
        ))
        .expect("portable data selected");

        assert_eq!(
            selected.data_objects,
            vec![
                MachineDataObject {
                    id: MachineDataObjectId::new(7),
                    name: "source".to_owned(),
                    bytes: vec![0, 0, 0, 0],
                    relocations: vec![
                        MachineDataRelocation {
                            offset: 0,
                            target: MachineDataObjectId::new(9),
                            addend: -3,
                            width: 2,
                            address_space: MachineAddressSpace::Segment,
                        },
                        MachineDataRelocation {
                            offset: 2,
                            target: MachineDataObjectId::new(3),
                            addend: 11,
                            width: 2,
                            address_space: MachineAddressSpace::NearData,
                        },
                    ],
                    alignment: 1,
                    constant: true,
                    linkage: MachineLinkage::Internal,
                    address_space: MachineAddressSpace::FarData,
                },
                MachineDataObject {
                    id: MachineDataObjectId::new(3),
                    name: "target".to_owned(),
                    bytes: vec![0, 1, 2, 3],
                    relocations: Vec::new(),
                    alignment: 1,
                    constant: true,
                    linkage: MachineLinkage::Internal,
                    address_space: MachineAddressSpace::NearData,
                },
                MachineDataObject {
                    id: MachineDataObjectId::new(9),
                    name: "segment-target".to_owned(),
                    bytes: vec![4, 5, 6, 7],
                    relocations: Vec::new(),
                    alignment: 1,
                    constant: true,
                    linkage: MachineLinkage::Internal,
                    address_space: MachineAddressSpace::NearData,
                },
            ]
        );
    }

    #[test]
    fn refuses_relocatable_data_with_an_unknown_target() {
        let source = data_global(
            7,
            "source",
            Constant::RelocatableBytes {
                bytes: vec![0, 0],
                relocations: vec![crate::ir::GlobalRelocation {
                    offset: 0,
                    target: GlobalId::new(8),
                    addend: 0,
                    width: 2,
                    address_space: AddressSpace::NearData,
                }],
            },
        );

        assert_eq!(
            select_module(&module_with_globals(byte_array_types(2), vec![source])),
            Err(SelectionError::UnknownDataRelocationTarget {
                global: GlobalId::new(7),
                target: GlobalId::new(8),
            })
        );
    }

    fn runtime_declaration(id: FunctionId, name: &str, parameters: Vec<TypeId>) -> Function {
        Function {
            id,
            name: name.to_owned(),
            signature: crate::ir::Signature {
                result: VOID,
                parameters: parameters.clone(),
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::External,
            attributes: Vec::new(),
            parameters: parameters
                .into_iter()
                .enumerate()
                .map(|(index, type_id)| Value {
                    id: ValueId::new(index as u32),
                    type_id,
                })
                .collect(),
            blocks: Vec::new(),
        }
    }

    #[test]
    fn selects_signed_i16_compare_branch_as_cmp_jl_and_false_jump() {
        // Python cfront.raise_hir.FunctionRaiser.compare/branch/jump_if
        // lowers scalar.c's `index < 8` followed by O_IF_FALSE.  The true
        // successor is the loop body; the false successor is loop exit.
        let left = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let right = Value {
            id: ValueId::new(1),
            type_id: I16,
        };
        let compared = Value {
            id: ValueId::new(2),
            type_id: I1,
        };
        let function = function(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![compared.clone()],
                        kind: InstructionKind::Compare {
                            predicate: ComparePredicate::SignedLessThan,
                            left: Operand::Value(left.id),
                            right: Operand::Value(right.id),
                        },
                    }],
                    terminator: Terminator::Branch {
                        condition: Operand::Value(compared.id),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
            ],
            vec![left, right],
        );

        let selected = select_module(&module(basic_types(), vec![function])).unwrap();
        selected.verify().unwrap();
        let block = &selected.functions[0].blocks[0];
        assert_eq!(
            block.successors,
            [MachineBlockId::new(1), MachineBlockId::new(2)]
        );
        assert_eq!(
            block
                .instructions
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Load.machine_opcode(),
                X86Opcode::Load.machine_opcode(),
                X86Opcode::Cmp.machine_opcode(),
                X86Opcode::JumpConditional.machine_opcode(),
                X86Opcode::Jump.machine_opcode(),
            ]
        );
        assert_eq!(
            block.instructions[3].operands,
            vec![
                immediate_operand(i64::from(ConditionCode::Less as u8)),
                block_operand(MachineBlockId::new(1)),
            ]
        );
        assert_eq!(
            block.instructions[4].operands,
            vec![block_operand(MachineBlockId::new(2))]
        );
    }

    #[test]
    fn selects_word_and_dword_integer_branch_predicates_and_scalar_for_bounds() {
        // qbopt.backend.lower._BRANCHES is the Python target oracle.  First
        // check each integer predicate it names against x86's condition code.
        for type_id in [I16, I32] {
            for (predicate, condition) in [
                (ComparePredicate::Equal, ConditionCode::Equal),
                (ComparePredicate::NotEqual, ConditionCode::NotEqual),
                (ComparePredicate::SignedLessThan, ConditionCode::Less),
                (
                    ComparePredicate::SignedLessEqual,
                    ConditionCode::LessOrEqual,
                ),
                (ComparePredicate::SignedGreaterThan, ConditionCode::Greater),
                (
                    ComparePredicate::SignedGreaterEqual,
                    ConditionCode::GreaterOrEqual,
                ),
                (ComparePredicate::UnsignedLessThan, ConditionCode::Below),
                (
                    ComparePredicate::UnsignedLessEqual,
                    ConditionCode::BelowOrEqual,
                ),
                (ComparePredicate::UnsignedGreaterThan, ConditionCode::Above),
                (
                    ComparePredicate::UnsignedGreaterEqual,
                    ConditionCode::AboveOrEqual,
                ),
            ] {
                let left = Value {
                    id: ValueId::new(0),
                    type_id,
                };
                let right = Value {
                    id: ValueId::new(1),
                    type_id,
                };
                let compared = Value {
                    id: ValueId::new(2),
                    type_id: I1,
                };
                let function = function(
                    vec![
                        Block {
                            id: BlockId::new(0),
                            instructions: vec![Instruction {
                                id: crate::ir::InstructionId::new(0),
                                results: vec![compared.clone()],
                                kind: InstructionKind::Compare {
                                    predicate,
                                    left: Operand::Value(left.id),
                                    right: Operand::Value(right.id),
                                },
                            }],
                            terminator: Terminator::Branch {
                                condition: Operand::Value(compared.id),
                                then_block: BlockId::new(1),
                                else_block: BlockId::new(2),
                            },
                        },
                        Block {
                            id: BlockId::new(1),
                            instructions: Vec::new(),
                            terminator: Terminator::Return(None),
                        },
                        Block {
                            id: BlockId::new(2),
                            instructions: Vec::new(),
                            terminator: Terminator::Return(None),
                        },
                    ],
                    vec![left, right],
                );

                let selected = select_module(&module(basic_types(), vec![function])).unwrap();
                super::super::verify_machine(&selected).unwrap();
                let block = &selected.functions[0].blocks[0];
                let left_register = block.instructions[0].operands[0].kind.clone();
                let right_register = block.instructions[1].operands[0].kind.clone();
                assert_eq!(
                    block.instructions[2]
                        .operands
                        .iter()
                        .map(|operand| operand.kind.clone())
                        .collect::<Vec<_>>(),
                    vec![left_register, right_register]
                );
                assert_eq!(
                    block.instructions[3].operands,
                    vec![
                        immediate_operand(i64::from(condition as u8)),
                        block_operand(MachineBlockId::new(1)),
                    ]
                );
            }
        }

        // bench/parity/scalar.bas has this scalar FOR dispatch: a nonnegative
        // step takes the inclusive <= bound, and a negative step takes >=.
        let counter = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let limit = Value {
            id: ValueId::new(1),
            type_id: I16,
        };
        let step = Value {
            id: ValueId::new(2),
            type_id: I16,
        };
        let less_equal = Value {
            id: ValueId::new(3),
            type_id: I1,
        };
        let greater_equal = Value {
            id: ValueId::new(4),
            type_id: I1,
        };
        let step_nonnegative = Value {
            id: ValueId::new(5),
            type_id: I1,
        };
        let function = function(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![step_nonnegative.clone()],
                        kind: InstructionKind::Compare {
                            predicate: ComparePredicate::SignedGreaterEqual,
                            left: Operand::Value(step.id),
                            right: Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(0),
                            }),
                        },
                    }],
                    terminator: Terminator::Branch {
                        condition: Operand::Value(step_nonnegative.id),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![less_equal.clone()],
                        kind: InstructionKind::Compare {
                            predicate: ComparePredicate::SignedLessEqual,
                            left: Operand::Value(counter.id),
                            right: Operand::Value(limit.id),
                        },
                    }],
                    terminator: Terminator::Branch {
                        condition: Operand::Value(less_equal.id),
                        then_block: BlockId::new(3),
                        else_block: BlockId::new(4),
                    },
                },
                Block {
                    id: BlockId::new(2),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(2),
                        results: vec![greater_equal.clone()],
                        kind: InstructionKind::Compare {
                            predicate: ComparePredicate::SignedGreaterEqual,
                            left: Operand::Value(counter.id),
                            right: Operand::Value(limit.id),
                        },
                    }],
                    terminator: Terminator::Branch {
                        condition: Operand::Value(greater_equal.id),
                        then_block: BlockId::new(3),
                        else_block: BlockId::new(4),
                    },
                },
                Block {
                    id: BlockId::new(3),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
                Block {
                    id: BlockId::new(4),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
            ],
            vec![counter, limit, step],
        );

        let selected = select_module(&module(basic_types(), vec![function])).unwrap();
        selected.verify().unwrap();
        let step_select = &selected.functions[0].blocks[0];
        let positive = &selected.functions[0].blocks[1];
        let negative = &selected.functions[0].blocks[2];
        let counter_register = step_select.instructions[0].operands[0].kind.clone();
        let limit_register = step_select.instructions[1].operands[0].kind.clone();
        let step_register = step_select.instructions[2].operands[0].kind.clone();
        let zero_register = step_select.instructions[3].operands[0].kind.clone();
        assert_eq!(
            step_select.instructions[4]
                .operands
                .iter()
                .map(|operand| operand.kind.clone())
                .collect::<Vec<_>>(),
            vec![step_register, zero_register]
        );
        assert_eq!(
            step_select.instructions[5].operands,
            vec![
                immediate_operand(i64::from(ConditionCode::GreaterOrEqual as u8)),
                block_operand(MachineBlockId::new(1)),
            ]
        );
        assert_eq!(
            step_select.instructions[6].operands,
            vec![block_operand(MachineBlockId::new(2))]
        );
        for (block, compare_at, branch_at, jump_at, condition) in [
            (positive, 0, 1, 2, ConditionCode::LessOrEqual),
            (negative, 0, 1, 2, ConditionCode::GreaterOrEqual),
        ] {
            assert_eq!(
                block.instructions[compare_at]
                    .operands
                    .iter()
                    .map(|operand| operand.kind.clone())
                    .collect::<Vec<_>>(),
                vec![counter_register.clone(), limit_register.clone()]
            );
            assert_eq!(
                block.instructions[branch_at].operands,
                vec![
                    immediate_operand(i64::from(condition as u8)),
                    block_operand(MachineBlockId::new(3)),
                ]
            );
            assert_eq!(
                block.instructions[jump_at].operands,
                vec![block_operand(MachineBlockId::new(4))]
            );
        }
    }

    #[test]
    fn selects_ordered_float_compare_and_refuses_unsupported_integer_width() {
        let float = TypeId::new(3);
        let ordered_result = Value {
            id: ValueId::new(2),
            type_id: I1,
        };
        let ordered_float = function(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![ordered_result.clone()],
                        kind: InstructionKind::Compare {
                            predicate: ComparePredicate::OrderedLessThan,
                            left: Operand::Constant(TypedConstant {
                                type_id: float,
                                value: Constant::Float("1.0".to_owned()),
                            }),
                            right: Operand::Constant(TypedConstant {
                                type_id: float,
                                value: Constant::Float("2.0".to_owned()),
                            }),
                        },
                    }],
                    terminator: Terminator::Branch {
                        condition: Operand::Value(ordered_result.id),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
            ],
            Vec::new(),
        );
        let mut types = basic_types();
        types.push(Type {
            id: float,
            kind: TypeKind::Float(FloatKind::Binary32),
        });
        let selected = select_module(&module(types, vec![ordered_float])).unwrap();
        assert_eq!(
            selected.data_objects.len(),
            1,
            "Python selects exact positive 1.0 with fld1 rather than a pool load",
        );
        assert_eq!(
            selected.functions[0].blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::X87LoadOne.machine_opcode(),
                X86Opcode::X87Load.machine_opcode(),
                X86Opcode::X87Compare.machine_opcode(),
                X86Opcode::JumpConditional.machine_opcode(),
                X86Opcode::Jump.machine_opcode(),
            ]
        );
        assert_eq!(
            selected.functions[0].blocks[0].instructions[3].operands[0],
            immediate_operand(i64::from(ConditionCode::Below as u8))
        );

        let byte_left = Value {
            id: ValueId::new(0),
            type_id: I8,
        };
        let byte_right = Value {
            id: ValueId::new(1),
            type_id: I8,
        };
        let byte_result = Value {
            id: ValueId::new(2),
            type_id: I1,
        };
        let unsupported_width = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![byte_result],
                    kind: InstructionKind::Compare {
                        predicate: ComparePredicate::SignedLessEqual,
                        left: Operand::Value(byte_left.id),
                        right: Operand::Value(byte_right.id),
                    },
                }],
                terminator: Terminator::Return(None),
            }],
            vec![byte_left, byte_right],
        );
        let mut byte_types = basic_types();
        byte_types.push(Type {
            id: I8,
            kind: TypeKind::Integer { bits: 8 },
        });
        assert!(matches!(
            select_module(&module(byte_types, vec![unsupported_width])),
            Err(SelectionError::UnsupportedCompare { .. })
        ));
    }

    #[test]
    fn selects_word_and_dword_integer_casts() {
        // Python lower represents a truncate as a low-word view, and signed
        // and unsigned widening as movsx and movzx respectively.
        for (op, source_type, destination_type, opcode, destination_class, source_class) in [
            (
                CastOp::Truncate,
                I32,
                I16,
                X86Opcode::LowWord,
                X86RegisterClass::Word,
                X86RegisterClass::Dword,
            ),
            (
                CastOp::SignExtend,
                I16,
                I32,
                X86Opcode::SignExtendWordToDword,
                X86RegisterClass::Dword,
                X86RegisterClass::Word,
            ),
            (
                CastOp::ZeroExtend,
                I16,
                I32,
                X86Opcode::ZeroExtendWordToDword,
                X86RegisterClass::Dword,
                X86RegisterClass::Word,
            ),
        ] {
            let source = Value {
                id: ValueId::new(0),
                type_id: source_type,
            };
            let result = Value {
                id: ValueId::new(1),
                type_id: destination_type,
            };
            let function = function(
                vec![Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![result],
                        kind: InstructionKind::Cast {
                            op,
                            operand: Operand::Value(source.id),
                            to: destination_type,
                        },
                    }],
                    terminator: Terminator::Return(None),
                }],
                vec![source],
            );

            let selected = select_module(&module(basic_types(), vec![function])).unwrap();
            super::super::verify_machine(&selected).unwrap();
            let instruction = selected.functions[0].blocks[0]
                .instructions
                .iter()
                .find(|instruction| instruction.opcode == opcode.machine_opcode())
                .expect("selected cast opcode");
            assert!(matches!(
                instruction.operands.as_slice(),
                [
                    MachineOperand {
                        kind: MachineOperandKind::Register(MachineRegister::Virtual(destination)),
                        role: OperandRole::Def,
                        constraint: None,
                        tied_to: None,
                    },
                    MachineOperand {
                        kind: MachineOperandKind::Register(MachineRegister::Virtual(source)),
                        role: OperandRole::Use,
                        constraint: None,
                        tied_to: None,
                    },
                ] if selected.functions[0].virtual_registers.iter().any(|register| register.id == *destination && register.class == destination_class.machine_class())
                    && selected.functions[0].virtual_registers.iter().any(|register| register.id == *source && register.class == source_class.machine_class())
            ));
        }
    }

    #[test]
    fn selects_float_storage_casts_and_preserves_integer_rounding() {
        let f32_type = TypeId::new(7);
        let f80_type = TypeId::new(8);
        let source = Value {
            id: ValueId::new(0),
            type_id: f32_type,
        };
        let extended = Value {
            id: ValueId::new(1),
            type_id: f80_type,
        };
        let integer = Value {
            id: ValueId::new(2),
            type_id: I32,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![extended.clone()],
                        kind: InstructionKind::Cast {
                            op: CastOp::FloatExtend,
                            operand: Operand::Value(source.id),
                            to: f80_type,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![integer],
                        kind: InstructionKind::Cast {
                            op: CastOp::FloatToInteger {
                                rounding: FloatRounding::TowardZero,
                            },
                            operand: Operand::Value(extended.id),
                            to: I32,
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
            vec![source],
        );
        let mut types = basic_types();
        types.extend([
            Type {
                id: f32_type,
                kind: TypeKind::Float(FloatKind::Binary32),
            },
            Type {
                id: f80_type,
                kind: TypeKind::Float(FloatKind::Extended80),
            },
        ]);

        let selected = select_module(&module(types, vec![function])).unwrap();
        let function = &selected.functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::X87Load.machine_opcode(),
                X86Opcode::Copy.machine_opcode(),
                X86Opcode::X87IntegerStoreTrunc.machine_opcode(),
                X86Opcode::Load.machine_opcode(),
                X86Opcode::ReturnNear.machine_opcode(),
            ]
        );
        assert!(
            function
                .frame_objects
                .iter()
                .any(|frame| { frame.size == 4 && frame.kind == FrameObjectKind::Temporary })
        );
        assert_eq!(
            function.blocks[0].instructions[2].operands[1],
            float_format_operand(X87MemoryFormat::Signed32)
        );
    }

    #[test]
    fn selects_integer_to_float_through_python_fild_storage_forms() {
        // Python floatalloc._integer_loads uses FLDZ/FLD1 for exact 0/1.
        // Every other signed word/dword reaches FILD through an owned cell;
        // x87 cannot read a general-purpose register directly.
        let float = TypeId::new(7);
        let mut types = basic_types();
        types.push(Type {
            id: float,
            kind: TypeKind::Float(FloatKind::Binary32),
        });

        for (source_type, value, opcode) in [
            (I16, 0, X86Opcode::X87LoadZero),
            (I32, 1, X86Opcode::X87LoadOne),
        ] {
            let result = Value {
                id: ValueId::new(0),
                type_id: float,
            };
            let input = module(
                types.clone(),
                vec![function(
                    vec![Block {
                        id: BlockId::new(0),
                        instructions: vec![Instruction {
                            id: crate::ir::InstructionId::new(0),
                            results: vec![result],
                            kind: InstructionKind::Cast {
                                op: CastOp::IntegerToFloat,
                                operand: Operand::Constant(TypedConstant {
                                    type_id: source_type,
                                    value: Constant::Integer(value),
                                }),
                                to: float,
                            },
                        }],
                        terminator: Terminator::Return(None),
                    }],
                    Vec::new(),
                )],
            );
            let selected = select_module(&input).expect("exact integer conversion selects");
            super::super::verify_machine(&selected).expect("exact conversion Machine IR verifies");
            assert!(selected.functions[0].frame_objects.is_empty());
            assert_eq!(
                selected.functions[0].blocks[0].instructions[0].opcode,
                opcode.machine_opcode()
            );
        }

        for (source_type, value, format) in [
            (I16, -27_i128, X87MemoryFormat::Signed16),
            (I32, i128::from(0xc174_7c23_u32), X87MemoryFormat::Signed32),
        ] {
            let result = Value {
                id: ValueId::new(0),
                type_id: float,
            };
            let input = module(
                types.clone(),
                vec![function(
                    vec![Block {
                        id: BlockId::new(0),
                        instructions: vec![Instruction {
                            id: crate::ir::InstructionId::new(0),
                            results: vec![result],
                            kind: InstructionKind::Cast {
                                op: CastOp::IntegerToFloat,
                                operand: Operand::Constant(TypedConstant {
                                    type_id: source_type,
                                    value: Constant::Integer(value),
                                }),
                                to: float,
                            },
                        }],
                        terminator: Terminator::Return(None),
                    }],
                    Vec::new(),
                )],
            );
            let selected = select_module(&input).expect("integer literal conversion selects");
            super::super::verify_machine(&selected)
                .expect("literal conversion Machine IR verifies");
            let function = &selected.functions[0];
            assert_eq!(function.frame_objects.len(), 1);
            assert_eq!(
                function.frame_objects[0].size,
                u32::from(match source_type {
                    I16 => 2_u16,
                    I32 => 4_u16,
                    _ => unreachable!(),
                })
            );
            assert_eq!(
                function.blocks[0]
                    .instructions
                    .iter()
                    .map(|instruction| instruction.opcode)
                    .collect::<Vec<_>>(),
                vec![
                    X86Opcode::Store.machine_opcode(),
                    X86Opcode::X87IntegerLoad.machine_opcode(),
                    X86Opcode::ReturnNear.machine_opcode(),
                ]
            );
            assert_eq!(
                function.blocks[0].instructions[0].operands[1],
                immediate_operand(i64::from(match source_type {
                    I16 => 16_u16,
                    I32 => 32_u16,
                    _ => unreachable!(),
                }))
            );
            assert_eq!(
                function.blocks[0].instructions[1].operands[1],
                float_format_operand(format)
            );
        }

        for (source_type, format) in [
            (I16, X87MemoryFormat::Signed16),
            (I32, X87MemoryFormat::Signed32),
        ] {
            let source = Value {
                id: ValueId::new(0),
                type_id: source_type,
            };
            let result = Value {
                id: ValueId::new(1),
                type_id: float,
            };
            let input = module(
                types.clone(),
                vec![function(
                    vec![Block {
                        id: BlockId::new(0),
                        instructions: vec![Instruction {
                            id: crate::ir::InstructionId::new(0),
                            results: vec![result],
                            kind: InstructionKind::Cast {
                                op: CastOp::IntegerToFloat,
                                operand: Operand::Value(source.id),
                                to: float,
                            },
                        }],
                        terminator: Terminator::Return(None),
                    }],
                    vec![source],
                )],
            );
            let selected = select_module(&input).expect("integer register conversion selects");
            super::super::verify_machine(&selected)
                .expect("register conversion Machine IR verifies");
            let function = &selected.functions[0];
            assert!(function.frame_objects.iter().any(|frame| {
                frame.size
                    == match source_type {
                        I16 => 2,
                        I32 => 4,
                        _ => unreachable!(),
                    }
                    && frame.kind == FrameObjectKind::Temporary
            }));
            let fild = function.blocks[0]
                .instructions
                .iter()
                .find(|instruction| {
                    instruction.opcode == X86Opcode::X87IntegerLoad.machine_opcode()
                })
                .expect("selected FILD");
            assert_eq!(fild.operands[1], float_format_operand(format));
            assert!(
                function.blocks[0]
                    .instructions
                    .iter()
                    .any(|instruction| instruction.opcode == X86Opcode::Store.machine_opcode())
            );
        }

        let byte = Value {
            id: ValueId::new(0),
            type_id: I8,
        };
        let result = Value {
            id: ValueId::new(1),
            type_id: float,
        };
        let mut byte_types = types;
        byte_types.push(Type {
            id: I8,
            kind: TypeKind::Integer { bits: 8 },
        });
        assert!(matches!(
            select_module(&module(
                byte_types,
                vec![function(
                    vec![Block {
                        id: BlockId::new(0),
                        instructions: vec![Instruction {
                            id: crate::ir::InstructionId::new(0),
                            results: vec![result],
                            kind: InstructionKind::Cast {
                                op: CastOp::IntegerToFloat,
                                operand: Operand::Value(byte.id),
                                to: float,
                            },
                        }],
                        terminator: Terminator::Return(None),
                    }],
                    vec![byte],
                )],
            )),
            Err(SelectionError::UnsupportedCast { .. })
        ));
    }

    #[test]
    fn selects_same_width_integer_bitcast_as_python_value_view() {
        // qbopt/cfront/raise_hir.py:_Raise.convert keeps a nonconstant
        // same-width integer conversion as the held word.  hir/lower.py:
        // Lowerer.convert_op represents that exact view as CastOp::Bitcast;
        // selection must therefore reuse the selected word rather than
        // materializing a conversion or refusing signedness-only changes.
        let unsigned_word = TypeId::new(3);
        let source = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let viewed = Value {
            id: ValueId::new(1),
            type_id: unsigned_word,
        };
        let masked = Value {
            id: ValueId::new(2),
            type_id: unsigned_word,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![viewed.clone()],
                        kind: InstructionKind::Cast {
                            op: CastOp::Bitcast,
                            operand: Operand::Value(source.id),
                            to: unsigned_word,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![masked],
                        kind: InstructionKind::Binary {
                            op: BinaryOp::And,
                            left: Operand::Value(viewed.id),
                            right: Operand::Constant(TypedConstant {
                                type_id: unsigned_word,
                                value: Constant::Integer(0x8000),
                            }),
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
            vec![source],
        );
        let mut types = basic_types();
        types.push(Type {
            id: unsigned_word,
            kind: TypeKind::Integer { bits: 16 },
        });

        let selected = select_module(&module(types, vec![function]))
            .expect("same-width integer bitcast selects");
        selected.verify().expect("selected Machine IR verifies");
        let instructions = &selected.functions[0].blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| X86Opcode::from_raw(instruction.opcode.get()).unwrap())
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Load,
                X86Opcode::Mov,
                X86Opcode::Copy,
                X86Opcode::And,
                X86Opcode::ReturnNear,
            ],
            "the cast is a typed view: selection emits only the parameter setup and its consuming operation",
        );
    }

    #[test]
    fn stores_a_binary32_literal_as_its_python_oracle_bits() {
        let pointer = TypeId::new(7);
        let float = TypeId::new(8);
        let address = Value {
            id: ValueId::new(0),
            type_id: pointer,
        };
        let function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: Vec::new(),
                    kind: InstructionKind::Store {
                        address: Operand::Value(address.id),
                        value: Operand::Constant(TypedConstant {
                            type_id: float,
                            value: Constant::Float("3.0".to_owned()),
                        }),
                        alignment: 2,
                        volatile: false,
                    },
                }],
                terminator: Terminator::Return(None),
            }],
            vec![address],
        );
        let mut types = basic_types();
        types.extend([
            Type {
                id: pointer,
                kind: TypeKind::Pointer {
                    address_space: AddressSpace::NearData,
                },
            },
            Type {
                id: float,
                kind: TypeKind::Float(FloatKind::Binary32),
            },
        ]);

        let selected =
            select_module(&module(types, vec![function])).expect("binary32 literal store selects");
        selected.verify().expect("selected Machine IR verifies");
        assert!(
            selected.data_objects.is_empty(),
            "a direct literal store must not leave a constant-pool object",
        );
        let stores = selected.functions[0].blocks[0]
            .instructions
            .iter()
            .filter(|instruction| instruction.opcode == X86Opcode::Store.machine_opcode())
            .collect::<Vec<_>>();
        assert_eq!(stores.len(), 1);
        assert!(matches!(
            stores[0].operands.as_slice(),
            [
                MachineOperand {
                    kind: MachineOperandKind::Register(MachineRegister::Virtual(_)),
                    role: OperandRole::Use,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::Immediate(32),
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
                MachineOperand {
                    kind: MachineOperandKind::Immediate(bits),
                    role: OperandRole::None,
                    constraint: None,
                    tied_to: None,
                },
            ] if *bits == i64::from(3.0_f32.to_bits())
        ));
        assert_eq!(stores[0].flags, store_flags(false));
        assert!(
            selected.functions[0].blocks[0]
                .instructions
                .iter()
                .all(
                    |instruction| instruction.opcode != X86Opcode::X87Load.machine_opcode()
                        && instruction.opcode != X86Opcode::X87StorePop.machine_opcode()
                )
        );
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
        assert_eq!(function.entry, MachineBlockId::new(2));
        assert_eq!(function.virtual_registers.len(), 4);
        assert!(matches!(
            function.frame_objects.as_slice(),
            [FrameObject {
                size: 4,
                kind: FrameObjectKind::IncomingArgument { parameter: 0 },
                ..
            }]
        ));
        assert_eq!(function.blocks[0].successors, vec![MachineBlockId::new(3)]);
        assert_eq!(function.blocks[0].instructions.len(), 7);
        assert_eq!(
            function.blocks[0].instructions[0].opcode,
            X86Opcode::Load.machine_opcode()
        );
        assert!(matches!(
            function.blocks[0].instructions[0].operands[0].kind,
            MachineOperandKind::Register(MachineRegister::Virtual(register))
                if register == VirtualRegisterId::new(0)
        ));
        assert_eq!(
            function.blocks[0].instructions[1].opcode,
            X86Opcode::Mov.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[2].opcode,
            X86Opcode::Copy.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[3].opcode,
            X86Opcode::Add.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[4].opcode,
            X86Opcode::Copy.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[5].opcode,
            X86Opcode::Not.machine_opcode()
        );
        assert_eq!(
            function.blocks[0].instructions[6].opcode,
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
    fn refuses_a_used_i1_but_selects_a_used_float_parameter_into_x87() {
        let i1 = TypeId::new(2);
        let float = TypeId::new(3);
        let mut types = basic_types();
        types.push(Type {
            id: float,
            kind: TypeKind::Float(FloatKind::Binary32),
        });

        let i1_function = function(
            vec![
                Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Branch {
                        condition: Operand::Value(ValueId::new(0)),
                        then_block: BlockId::new(1),
                        else_block: BlockId::new(2),
                    },
                },
                Block {
                    id: BlockId::new(1),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
                Block {
                    id: BlockId::new(2),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(None),
                },
            ],
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
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(1),
                        type_id: float,
                    }],
                    kind: InstructionKind::Unary {
                        op: UnaryOp::FloatNegate,
                        operand: Operand::Value(ValueId::new(0)),
                    },
                }],
                terminator: Terminator::Return(None),
            }],
            vec![Value {
                id: ValueId::new(0),
                type_id: float,
            }],
        );
        let selected = select_module(&module(types, vec![float_function])).unwrap();
        let function = &selected.functions[0];
        assert_eq!(
            function.signature.parameters,
            vec![MachineValueType::Float {
                kind: MachineFloatKind::Binary32,
            }]
        );
        assert_eq!(
            function.blocks[0].instructions[0].opcode,
            X86Opcode::X87Load.machine_opcode()
        );
        assert_eq!(
            function.virtual_registers[0].class,
            X86RegisterClass::X87.machine_class()
        );
    }

    #[test]
    fn loads_an_addressed_float_parameter_only_in_its_source_block() {
        // Port of cfront.raise_hir._Raise.points/floating: an unused formal
        // SSA value creates no entry x87 load. The source load reads the
        // existing incoming frame cell in the block where it is consumed.
        let float = TypeId::new(3);
        let pointer = TypeId::new(7);
        let mut types = basic_types();
        types.extend([
            Type {
                id: float,
                kind: TypeKind::Float(FloatKind::Binary32),
            },
            Type {
                id: pointer,
                kind: TypeKind::Pointer {
                    address_space: AddressSpace::NearData,
                },
            },
        ]);
        let addressed = function(
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
                            id: crate::ir::InstructionId::new(0),
                            results: vec![Value {
                                id: ValueId::new(1),
                                type_id: pointer,
                            }],
                            kind: InstructionKind::ParameterAddress { parameter: 0 },
                        },
                        Instruction {
                            id: crate::ir::InstructionId::new(1),
                            results: vec![Value {
                                id: ValueId::new(2),
                                type_id: float,
                            }],
                            kind: InstructionKind::Load {
                                address: Operand::Value(ValueId::new(1)),
                                alignment: 2,
                                volatile: false,
                            },
                        },
                    ],
                    terminator: Terminator::Return(None),
                },
            ],
            vec![Value {
                id: ValueId::new(0),
                type_id: float,
            }],
        );

        let selected = select_module(&module(types, vec![addressed])).unwrap();
        let function = &selected.functions[0];
        assert!(
            function.blocks[0]
                .instructions
                .iter()
                .all(|instruction| instruction.opcode != X86Opcode::X87Load.machine_opcode())
        );
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .filter(|instruction| instruction.opcode == X86Opcode::X87Load.machine_opcode())
                .count(),
            1
        );
        assert!(matches!(
            function.frame_objects.as_slice(),
            [FrameObject {
                kind: FrameObjectKind::IncomingArgument { parameter: 0 },
                ..
            }]
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

    #[test]
    fn selects_runtime_arguments_before_a_far_external_call() {
        let runtime = runtime_declaration(FunctionId::new(5), "B$RT", vec![I16, I32]);
        let caller = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: Vec::new(),
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(runtime.id),
                        arguments: vec![
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(7),
                            }),
                            Operand::Constant(TypedConstant {
                                type_id: I32,
                                value: Constant::Integer(9),
                            }),
                        ],
                        effects: Effects {
                            memory: MemoryEffects::Unknown,
                            may_trap: true,
                            observable: true,
                        },
                    },
                }],
                terminator: Terminator::Unreachable,
            }],
            Vec::new(),
        );

        let selected = select_module(&module(basic_types(), vec![caller, runtime]))
            .expect("runtime declaration and void call select exactly");
        selected.verify().expect("selected Machine IR verifies");
        assert_eq!(
            selected.functions.len(),
            1,
            "declarations have no Machine IR body"
        );
        let instructions = &selected.functions[0].blocks[0].instructions;
        assert_eq!(instructions[0].opcode, X86Opcode::Mov.machine_opcode());
        assert_eq!(instructions[1].opcode, X86Opcode::Push.machine_opcode());
        assert_eq!(instructions[2].opcode, X86Opcode::Mov.machine_opcode());
        assert_eq!(instructions[3].opcode, X86Opcode::Push.machine_opcode());
        assert_eq!(instructions[4].opcode, X86Opcode::CallFar.machine_opcode());
        assert_eq!(
            instructions.len(),
            5,
            "unreachable emits no machine operation"
        );
        assert!(selected.functions[0].blocks[0].successors.is_empty());
        assert!(matches!(
            instructions[4].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::ExternalSymbol { name, addend: 0 },
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            }] if name == "B$RT"
        ));
        assert_eq!(
            instructions[1].operands[0].kind, instructions[0].operands[0].kind,
            "the first ABI argument is pushed before the call"
        );
        assert_eq!(
            instructions[3].operands[0].kind, instructions[2].operands[0].kind,
            "the second ABI argument is pushed before the call"
        );
        assert_eq!(
            instructions[4].flags,
            InstructionFlags {
                call: true,
                side_effects: true,
                may_load: true,
                may_store: true,
                volatile: true,
                ..InstructionFlags::NONE
            }
        );
    }

    #[test]
    fn selects_float_call_arguments_as_the_python_lowering_pushes_them() {
        // Python cfront.raise_hir emits the literal's IEEE-754 bits directly
        // as a dword argument. A computed value follows Python's ordinary
        // fallback exactly: fstp to its temporary, mov the raw dword to a
        // general register, then push that register.
        let float = TypeId::new(7);
        let parameter = Value {
            id: ValueId::new(0),
            type_id: float,
        };
        let callee = Function {
            id: FunctionId::new(5),
            name: "consume".to_owned(),
            signature: Signature {
                result: VOID,
                parameters: vec![float, float],
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![
                Value {
                    id: ValueId::new(0),
                    type_id: float,
                },
                Value {
                    id: ValueId::new(1),
                    type_id: float,
                },
            ],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "caller".to_owned(),
            signature: Signature {
                result: VOID,
                parameters: vec![float],
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![parameter.clone()],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: Vec::new(),
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(callee.id),
                        arguments: vec![
                            Operand::Value(parameter.id),
                            Operand::Constant(TypedConstant {
                                type_id: float,
                                value: Constant::Float("1.5".to_owned()),
                            }),
                        ],
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(None),
            }],
        };
        let mut types = basic_types();
        types.push(Type {
            id: float,
            kind: TypeKind::Float(FloatKind::Binary32),
        });

        let selected = select_module(&module(types, vec![caller, callee]))
            .expect("f32 caller-cleanup arguments select");
        selected.verify().expect("selected Machine IR verifies");
        let caller = &selected.functions[0];
        assert!(
            selected.data_objects.is_empty(),
            "direct literal arguments do not need a constant-pool object",
        );
        let instructions = &caller.blocks[0].instructions;
        let pushes = instructions
            .iter()
            .filter(|instruction| instruction.opcode == X86Opcode::Push.machine_opcode())
            .collect::<Vec<_>>();
        assert_eq!(pushes.len(), 2);
        assert_eq!(
            pushes[0].operands,
            vec![
                immediate_operand(32),
                immediate_operand(i64::from(1.5_f32.to_bits())),
            ],
            "right-to-left C argument order pushes the literal bits directly",
        );
        let store = instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::X87StorePop.machine_opcode())
            .expect("computed f32 is rounded into a temporary");
        let load = instructions
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::Load.machine_opcode())
            .expect("Python fallback loads the temporary bits into a GPR");
        assert_eq!(store.operands[2].kind, load.operands[1].kind);
        assert!(matches!(
            load.operands[1],
            MachineOperand {
                kind: MachineOperandKind::FrameIndex { index, addend: 0 },
                role: OperandRole::None,
                constraint: None,
                tied_to: None,
            } if caller.frame_objects.iter().any(|frame| frame.index == index
                && frame.size == 4
                && frame.kind == FrameObjectKind::Temporary)
        ));
        assert_eq!(load.flags, load_flags(false));
        assert_eq!(pushes[1].operands.len(), 1);
        assert_eq!(pushes[1].operands[0].kind, load.operands[0].kind);
        assert_eq!(pushes[1].flags, InstructionFlags::NONE);
    }

    #[test]
    fn selects_external_c_float_calls_as_python_invoke_and_push() {
        // Python cfront.raise_hir._Raise.library/_Raise.invoke create an
        // imported far cdecl `_sqrt` returning one x87 value.  `_Raise.push`
        // places a DOUBLE's high dword first so its low dword occupies the
        // lower stack address.  An ignored floating result remains an x87
        // value until float allocation emits Python's `fstp st(0)` discard.
        let single = TypeId::new(7);
        let double = TypeId::new(8);
        let sqrt = Function {
            id: FunctionId::new(5),
            name: "_sqrt".to_owned(),
            signature: Signature {
                result: double,
                parameters: vec![double],
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::External,
            attributes: Vec::new(),
            parameters: vec![Value {
                id: ValueId::new(0),
                type_id: double,
            }],
            blocks: Vec::new(),
        };
        let sample = Function {
            id: FunctionId::new(6),
            name: "sample".to_owned(),
            signature: Signature {
                result: single,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::External,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: Vec::new(),
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "caller".to_owned(),
            signature: Signature {
                result: VOID,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![Value {
                            id: ValueId::new(0),
                            type_id: double,
                        }],
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(sqrt.id),
                            arguments: vec![Operand::Constant(TypedConstant {
                                type_id: double,
                                value: Constant::Float("3.5".to_owned()),
                            })],
                            effects: Effects::NONE,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: vec![Value {
                            id: ValueId::new(1),
                            type_id: single,
                        }],
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(sample.id),
                            arguments: Vec::new(),
                            effects: Effects::NONE,
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
        };
        let mut types = basic_types();
        types.push(Type {
            id: single,
            kind: TypeKind::Float(FloatKind::Binary32),
        });
        types.push(Type {
            id: double,
            kind: TypeKind::Float(FloatKind::Binary64),
        });

        let selected = select_module(&module(types, vec![caller, sqrt, sample]))
            .expect("external C floating calls select as Python does");
        selected.verify().expect("selected Machine IR verifies");
        let instructions = &selected.functions[0].blocks[0].instructions;
        let bits = 3.5_f64.to_bits();
        assert_eq!(
            instructions[0].operands,
            vec![
                immediate_operand(32),
                immediate_operand(i64::from((bits >> 32) as u32)),
            ],
            "Python pushes the high DOUBLE dword first",
        );
        assert_eq!(
            instructions[1].operands,
            vec![
                immediate_operand(32),
                immediate_operand(i64::from(bits as u32))
            ],
            "Python pushes the low DOUBLE dword second",
        );
        let sqrt_call = &instructions[2];
        assert_eq!(sqrt_call.opcode, X86Opcode::CallFar.machine_opcode());
        assert!(matches!(
            sqrt_call.operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::ExternalSymbol { name, addend: 0 },
                role: OperandRole::None,
                ..
            }, MachineOperand {
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }] if name == "_sqrt" && *register == X86Register::St0.physical()
        ));
        assert_eq!(instructions[3].opcode, X86Opcode::Add.machine_opcode());
        assert_eq!(instructions[3].operands[1], immediate_operand(8));
        let sample_call = &instructions[4];
        assert_eq!(sample_call.opcode, X86Opcode::CallNear.machine_opcode());
        assert!(matches!(
            sample_call.operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::ExternalSymbol { name, addend: 0 },
                role: OperandRole::None,
                ..
            }, MachineOperand {
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }] if name == "sample" && *register == X86Register::St0.physical()
        ));

        let stackified = crate::target::x86::allocate_x87_stack(&selected.functions[0])
            .expect("the ignored Python float results stackify");
        let pop_positions = stackified.blocks[0]
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| {
                (instruction.opcode == X86Opcode::X87StackStorePop.machine_opcode())
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pop_positions.len(),
            2,
            "both unused call results become fstp st(0)"
        );
        let sample_call_position = stackified.blocks[0]
            .instructions
            .iter()
            .position(|instruction| {
                instruction.opcode == X86Opcode::CallNear.machine_opcode()
                    && matches!(
                        instruction.operands.first(),
                        Some(MachineOperand {
                            kind: MachineOperandKind::ExternalSymbol { name, .. },
                            ..
                        }) if name == "sample"
                    )
            })
            .expect("stackified body retains sample's call");
        assert!(
            pop_positions[0] < sample_call_position,
            "the unused sqrt result is popped before sample receives ST0",
        );
    }

    #[test]
    fn pushes_a_computed_external_double_as_python_push_does() {
        // Python cfront.raise_hir._Raise.push first stores a computed DOUBLE
        // to an 8-byte temporary, then emits ARG for offset +4 and offset +0.
        // The resulting stack layout is high dword first, low dword second.
        let double = TypeId::new(7);
        let consume = Function {
            id: FunctionId::new(5),
            name: "consume_double".to_owned(),
            signature: Signature {
                result: VOID,
                parameters: vec![double],
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::External,
            attributes: Vec::new(),
            parameters: vec![Value {
                id: ValueId::new(0),
                type_id: double,
            }],
            blocks: Vec::new(),
        };
        let computed = Value {
            id: ValueId::new(0),
            type_id: double,
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "caller".to_owned(),
            signature: Signature {
                result: VOID,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![computed.clone()],
                        kind: InstructionKind::Unary {
                            op: UnaryOp::FloatNegate,
                            operand: Operand::Constant(TypedConstant {
                                type_id: double,
                                value: Constant::Float("1.0".to_owned()),
                            }),
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: Vec::new(),
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(consume.id),
                            arguments: vec![Operand::Value(computed.id)],
                            effects: Effects::NONE,
                        },
                    },
                ],
                terminator: Terminator::Return(None),
            }],
        };
        let mut types = basic_types();
        types.push(Type {
            id: double,
            kind: TypeKind::Float(FloatKind::Binary64),
        });

        let selected = select_module(&module(types, vec![caller, consume]))
            .expect("computed external DOUBLE argument selects");
        selected.verify().expect("selected Machine IR verifies");
        let function = &selected.functions[0];
        let instructions = &function.blocks[0].instructions;
        let store = instructions
            .iter()
            .position(|instruction| instruction.opcode == X86Opcode::X87StorePop.machine_opcode())
            .expect("computed DOUBLE stores to Python's temporary");
        let MachineOperandKind::FrameIndex { index, addend } = instructions[store].operands[2].kind
        else {
            panic!("computed DOUBLE store lacks a frame temporary");
        };
        assert_eq!(addend, 0);
        assert!(function.frame_objects.iter().any(|frame| {
            frame.index == index && frame.size == 8 && frame.kind == FrameObjectKind::Temporary
        }));
        assert!(matches!(
            instructions[store + 1].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(_)),
                role: OperandRole::Def,
                ..
            }, MachineOperand {
                kind: MachineOperandKind::FrameIndex {
                    index: loaded,
                    addend: 4,
                },
                role: OperandRole::None,
                ..
            }] if *loaded == index
        ));
        assert_eq!(
            instructions[store + 1].opcode,
            X86Opcode::Load.machine_opcode()
        );
        assert_eq!(
            instructions[store + 2].opcode,
            X86Opcode::Push.machine_opcode()
        );
        assert_eq!(
            instructions[store + 3].opcode,
            X86Opcode::Load.machine_opcode()
        );
        assert!(matches!(
            instructions[store + 3].operands.get(1),
            Some(MachineOperand {
                kind: MachineOperandKind::FrameIndex {
                    index: loaded,
                    addend: 0,
                },
                role: OperandRole::None,
                ..
            }) if *loaded == index
        ));
        assert_eq!(
            instructions[store + 4].opcode,
            X86Opcode::Push.machine_opcode()
        );
        assert_eq!(
            instructions[store + 5].opcode,
            X86Opcode::CallFar.machine_opcode()
        );
        assert_eq!(
            instructions[store + 6].opcode,
            X86Opcode::Add.machine_opcode()
        );
        assert_eq!(instructions[store + 6].operands[1], immediate_operand(8));
    }

    #[test]
    fn selects_near_descriptor_argument_before_a_far_external_call() {
        let pointer = TypeId::new(7);
        let mut types = byte_array_types(4);
        types.push(Type {
            id: pointer,
            kind: TypeKind::Pointer {
                address_space: AddressSpace::NearData,
            },
        });
        let descriptor = data_global(3, "descriptor", Constant::Bytes(vec![0; 4]));
        let runtime = runtime_declaration(FunctionId::new(5), "B$PSSD", vec![pointer]);
        let address = Value {
            id: ValueId::new(0),
            type_id: pointer,
        };
        let caller = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![
                    Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![address.clone()],
                        kind: InstructionKind::Cast {
                            op: CastOp::Bitcast,
                            operand: Operand::Constant(TypedConstant {
                                type_id: pointer,
                                value: Constant::GlobalAddress {
                                    global: descriptor.id,
                                    addend: 0,
                                },
                            }),
                            to: pointer,
                        },
                    },
                    Instruction {
                        id: crate::ir::InstructionId::new(1),
                        results: Vec::new(),
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(runtime.id),
                            arguments: vec![Operand::Value(address.id)],
                            effects: Effects {
                                memory: MemoryEffects::Unknown,
                                may_trap: true,
                                observable: true,
                            },
                        },
                    },
                ],
                terminator: Terminator::Unreachable,
            }],
            Vec::new(),
        );
        let input = Module {
            name: "runtime-descriptor".to_owned(),
            types,
            globals: vec![descriptor],
            functions: vec![caller, runtime],
        };

        let selected = select_module(&input).expect("near descriptor ABI is supported");
        selected.verify().expect("selected Machine IR verifies");
        let instructions = &selected.functions[0].blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| X86Opcode::from_raw(instruction.opcode.get()).unwrap())
                .collect::<Vec<_>>(),
            vec![X86Opcode::Lea, X86Opcode::Push, X86Opcode::CallFar]
        );
        assert!(matches!(
            &instructions[0].operands[1].kind,
            MachineOperandKind::Global { name, addend: 0 } if name == "descriptor"
        ));
    }

    #[test]
    fn refuses_runtime_call_results() {
        let runtime = runtime_declaration(FunctionId::new(5), "B$RT", Vec::new());
        let caller = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(0),
                        type_id: I32,
                    }],
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(runtime.id),
                        arguments: Vec::new(),
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(None),
            }],
            Vec::new(),
        );

        assert!(matches!(
            select_module(&module(basic_types(), vec![caller, runtime])),
            Err(SelectionError::UnsupportedCallResult {
                callee,
                result,
                values: 1,
                ..
            }) if callee == FunctionId::new(5) && result == VOID
        ));
    }

    #[test]
    fn selects_far_pascal_i16_call_result_in_ax_without_caller_cleanup() {
        let callee = Function {
            id: FunctionId::new(5),
            name: "sum".into(),
            signature: Signature {
                result: I16,
                parameters: vec![I16, I16],
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![
                Value {
                    id: ValueId::new(0),
                    type_id: I16,
                },
                Value {
                    id: ValueId::new(1),
                    type_id: I16,
                },
            ],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "main".into(),
            signature: Signature {
                result: I16,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(0),
                        type_id: I16,
                    }],
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(callee.id),
                        arguments: vec![
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(11),
                            }),
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(22),
                            }),
                        ],
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };

        let selected = select_module(&module(basic_types(), vec![caller, callee]))
            .expect("far Pascal i16 call selects");
        super::super::verify_machine(&selected).expect("selected Machine IR verifies");

        let caller = &selected.functions[0].blocks[0].instructions;
        assert_eq!(
            caller
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::CallFar.machine_opcode(),
                X86Opcode::ReturnFar.machine_opcode(),
            ]
        );
        assert_eq!(caller[0].operands[1], immediate_operand(11));
        assert_eq!(caller[2].operands[1], immediate_operand(22));
        assert!(matches!(
            caller[4].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::Function(target),
                ..
            }, MachineOperand {
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }] if *target == MachineFunctionId::new(5) && *register == X86Register::Ax.physical()
        ));
        assert!(matches!(
            caller[5].operands.as_slice(),
            [MachineOperand {
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(register)),
                kind,
                ..
            }, MachineOperand {
                kind: MachineOperandKind::Immediate(0),
                role: OperandRole::None,
                ..
            }] if *register == X86Register::Ax.physical() && *kind == caller[4].operands[1].kind
        ));

        let returned = selected.functions[1].blocks[0]
            .instructions
            .last()
            .expect("callee has return");
        assert!(matches!(
            returned.operands.as_slice(),
            [MachineOperand {
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }, MachineOperand {
                kind: MachineOperandKind::Immediate(4),
                role: OperandRole::None,
                ..
            }] if *register == X86Register::Ax.physical()
        ));
    }

    #[test]
    fn selects_far_pascal_near_pointer_call_and_return_in_ax() {
        // Microsoft BASIC floating functions physically return their hidden
        // near result pointer in AX.  At this target boundary that pointer is
        // the same one-word ABI value as an i16; pointee semantics stay in
        // the QB frontend physicalizer.
        let pointer = TypeId::new(7);
        let parameter = |id| Value {
            id: ValueId::new(id),
            type_id: pointer,
        };
        let callee = Function {
            id: FunctionId::new(5),
            name: "return_pointer".into(),
            signature: Signature {
                result: pointer,
                parameters: vec![pointer],
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::External,
            attributes: Vec::new(),
            parameters: vec![parameter(0)],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "caller".into(),
            signature: Signature {
                result: pointer,
                parameters: vec![pointer],
                variadic: false,
                calling_convention: CallingConvention::FarPascal,
            },
            linkage: Linkage::External,
            attributes: Vec::new(),
            parameters: vec![parameter(0)],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![parameter(1)],
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(callee.id),
                        arguments: vec![Operand::Value(ValueId::new(0))],
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(1)))),
            }],
        };
        let mut types = basic_types();
        types.push(near_pointer_type(pointer));

        let selected = select_module(&module(types, vec![caller, callee]))
            .expect("far Pascal near-pointer result uses the one-word ABI");
        super::super::verify_machine(&selected).expect("selected Machine IR verifies");

        let caller = &selected.functions[0].blocks[0].instructions;
        let call = caller
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::CallFar.machine_opcode())
            .expect("caller contains the far call");
        assert!(matches!(
            call.operands.last(),
            Some(MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Virtual(_)),
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(register)),
                tied_to: None,
            }) if *register == X86Register::Ax.physical()
        ));
        let returned = caller
            .iter()
            .find(|instruction| instruction.opcode == X86Opcode::ReturnFar.machine_opcode())
            .expect("caller returns the pointer");
        assert!(matches!(
            returned.operands.first(),
            Some(MachineOperand {
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }) if *register == X86Register::Ax.physical()
        ));
    }

    #[test]
    fn selects_far_cdecl_i16_call_return_and_caller_cleanup() {
        let callee = Function {
            id: FunctionId::new(5),
            name: "sum".into(),
            signature: Signature {
                result: I16,
                parameters: vec![I16, I16],
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![
                Value {
                    id: ValueId::new(0),
                    type_id: I16,
                },
                Value {
                    id: ValueId::new(1),
                    type_id: I16,
                },
            ],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "main".into(),
            signature: Signature {
                result: I16,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(0),
                        type_id: I16,
                    }],
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(callee.id),
                        arguments: vec![
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(11),
                            }),
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(22),
                            }),
                        ],
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };

        let selected = select_module(&module(basic_types(), vec![caller, callee]))
            .expect("far caller-cleanup i16 pair selects");
        selected.verify().expect("selected Machine IR verifies");

        let caller = &selected.functions[0].blocks[0].instructions;
        assert_eq!(
            caller
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::CallFar.machine_opcode(),
                X86Opcode::Add.machine_opcode(),
                X86Opcode::ReturnFar.machine_opcode(),
            ]
        );
        assert_eq!(caller[0].operands[1], immediate_operand(22));
        assert_eq!(caller[2].operands[1], immediate_operand(11));
        assert!(matches!(
            caller[4].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::Function(target),
                ..
            }, MachineOperand {
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }] if *target == MachineFunctionId::new(5) && *register == X86Register::Ax.physical()
        ));
        assert!(matches!(
            caller[5].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::UseDef,
                constraint: None,
                tied_to: None,
            }, MachineOperand {
                kind: MachineOperandKind::Immediate(4),
                ..
            }] if *register == X86Register::Sp.physical()
        ));
        assert!(matches!(
            caller[6].operands.as_slice(),
            [MachineOperand {
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }, MachineOperand {
                kind: MachineOperandKind::Immediate(0),
                ..
            }] if *register == X86Register::Ax.physical()
        ));

        let callee = &selected.functions[1].blocks[0].instructions;
        assert_eq!(
            callee.last().expect("callee has return").opcode,
            X86Opcode::ReturnFar.machine_opcode()
        );
        assert!(matches!(
            callee.last().expect("callee has return").operands.as_slice(),
            [MachineOperand {
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }, MachineOperand {
                kind: MachineOperandKind::Immediate(0),
                ..
            }] if *register == X86Register::Ax.physical()
        ));
    }

    #[test]
    fn selects_far_cdecl_i32_call_result_in_dx_ax_and_cleans_arguments() {
        let callee = Function {
            id: FunctionId::new(5),
            name: "sum".into(),
            signature: Signature {
                result: I32,
                parameters: vec![I16, I16],
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![
                Value {
                    id: ValueId::new(0),
                    type_id: I16,
                },
                Value {
                    id: ValueId::new(1),
                    type_id: I16,
                },
            ],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Unreachable,
            }],
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "main".into(),
            signature: Signature {
                result: I32,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(0),
                        type_id: I32,
                    }],
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(callee.id),
                        arguments: vec![
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(11),
                            }),
                            Operand::Constant(TypedConstant {
                                type_id: I16,
                                value: Constant::Integer(22),
                            }),
                        ],
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };

        let selected = select_module(&module(basic_types(), vec![caller, callee]))
            .expect("far caller-cleanup i32 call selects");
        selected.verify().expect("selected Machine IR verifies");

        let caller = &selected.functions[0].blocks[0].instructions;
        assert_eq!(
            caller
                .iter()
                .map(|instruction| instruction.opcode)
                .collect::<Vec<_>>(),
            vec![
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::Mov.machine_opcode(),
                X86Opcode::Push.machine_opcode(),
                X86Opcode::CallFar.machine_opcode(),
                X86Opcode::MergeWords.machine_opcode(),
                X86Opcode::Add.machine_opcode(),
                X86Opcode::LowWord.machine_opcode(),
                X86Opcode::HighWord.machine_opcode(),
                X86Opcode::ReturnFar.machine_opcode(),
            ]
        );
        assert_eq!(caller[0].operands[1], immediate_operand(22));
        assert_eq!(caller[2].operands[1], immediate_operand(11));
        assert!(matches!(
            caller[4].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::Function(target),
                ..
            }, MachineOperand {
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(low)),
                ..
            }, MachineOperand {
                role: OperandRole::Def,
                constraint: Some(RegisterConstraint::Fixed(high)),
                ..
            }] if *target == MachineFunctionId::new(5)
                && *low == X86Register::Ax.physical()
                && *high == X86Register::Dx.physical()
        ));
        assert_eq!(caller[5].operands[0].role, OperandRole::Def);
        assert_eq!(caller[5].operands[1].role, OperandRole::Use);
        assert_eq!(caller[5].operands[2].role, OperandRole::Use);
        assert_eq!(caller[5].operands[1].kind, caller[4].operands[1].kind);
        assert_eq!(caller[5].operands[2].kind, caller[4].operands[2].kind);
        assert!(matches!(
            caller[6].operands.as_slice(),
            [MachineOperand {
                kind: MachineOperandKind::Register(MachineRegister::Physical(register)),
                role: OperandRole::UseDef,
                ..
            }, MachineOperand {
                kind: MachineOperandKind::Immediate(4),
                ..
            }] if *register == X86Register::Sp.physical()
        ));
    }

    #[test]
    fn selects_near_c_i16_call_and_return() {
        let callee = Function {
            id: FunctionId::new(5),
            name: "id".into(),
            signature: Signature {
                result: I16,
                parameters: vec![I16],
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![Value {
                id: ValueId::new(0),
                type_id: I16,
            }],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };
        let caller = Function {
            id: FunctionId::new(4),
            name: "main".into(),
            signature: Signature {
                result: I16,
                parameters: Vec::new(),
                variadic: false,
                calling_convention: CallingConvention::C,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: vec![Instruction {
                    id: crate::ir::InstructionId::new(0),
                    results: vec![Value {
                        id: ValueId::new(0),
                        type_id: I16,
                    }],
                    kind: InstructionKind::Call {
                        callee: Callee::Direct(callee.id),
                        arguments: vec![Operand::Constant(TypedConstant {
                            type_id: I16,
                            value: Constant::Integer(7),
                        })],
                        effects: Effects::NONE,
                    },
                }],
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };

        let selected = select_module(&module(basic_types(), vec![caller, callee]))
            .expect("near caller-cleanup i16 pair selects");
        selected.verify().expect("selected Machine IR verifies");

        let caller = &selected.functions[0].blocks[0].instructions;
        assert_eq!(caller[2].opcode, X86Opcode::CallNear.machine_opcode());
        assert_eq!(caller[3].opcode, X86Opcode::Add.machine_opcode());
        assert_eq!(caller[4].opcode, X86Opcode::ReturnNear.machine_opcode());
        assert!(matches!(
            caller[4].operands.as_slice(),
            [MachineOperand {
                role: OperandRole::Use,
                constraint: Some(RegisterConstraint::Fixed(register)),
                ..
            }] if *register == X86Register::Ax.physical()
        ));
        assert_eq!(
            selected.functions[1].blocks[0]
                .instructions
                .last()
                .expect("callee has return")
                .opcode,
            X86Opcode::ReturnNear.machine_opcode()
        );
    }

    #[test]
    fn selects_far_pascal_i16_return_in_ax() {
        // Python's return lowering keeps an ordinary word result as one
        // value; AX is an ABI boundary occurrence, not the value's lifetime.
        let value = Value {
            id: ValueId::new(0),
            type_id: I16,
        };
        let mut function = function(
            vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(value.id))),
            }],
            vec![value],
        );
        function.signature.result = I16;

        let selected = select_module(&module(basic_types(), vec![function]))
            .expect("far Pascal i16 return selects");
        super::super::verify_machine(&selected).expect("selected Machine IR verifies");

        let returned = selected.functions[0].blocks[0]
            .instructions
            .last()
            .expect("function has a return");
        assert_eq!(returned.opcode, X86Opcode::ReturnFar.machine_opcode());
        assert!(matches!(
            returned.operands.as_slice(),
            [
                MachineOperand {
                    role: OperandRole::Use,
                    constraint: Some(RegisterConstraint::Fixed(register)),
                    ..
                },
                MachineOperand {
                    kind: MachineOperandKind::Immediate(2),
                    role: OperandRole::None,
                    ..
                }
            ] if *register == X86Register::Ax.physical()
        ));
    }

    #[test]
    fn selects_far_cdecl_i32_return_in_dx_ax() {
        // Python cfront.raise_hir.FunctionRaiser.ret splits a 32-bit C
        // result into two words, and backend.lower._RETURNED places them in
        // AX then DX.  parity/scalar returns an i32 through this exact ABI.
        let function = Function {
            id: FunctionId::new(0),
            name: "parity_scalar".into(),
            signature: Signature {
                result: I32,
                parameters: vec![I32],
                variadic: false,
                calling_convention: CallingConvention::FarCdecl,
            },
            linkage: Linkage::Internal,
            attributes: Vec::new(),
            parameters: vec![Value {
                id: ValueId::new(0),
                type_id: I32,
            }],
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
            }],
        };

        let selected = select_module(&module(basic_types(), vec![function]))
            .expect("far cdecl i32 return selects");
        selected.verify().expect("selected Machine IR verifies");

        let instructions = &selected.functions[0].blocks[0].instructions;
        let returned = instructions.last().expect("function has a return");
        assert_eq!(returned.opcode, X86Opcode::ReturnFar.machine_opcode());
        assert!(matches!(
            returned.operands.as_slice(),
            [
                MachineOperand {
                    role: OperandRole::Use,
                    constraint: Some(RegisterConstraint::Fixed(low)),
                    ..
                },
                MachineOperand {
                    role: OperandRole::Use,
                    constraint: Some(RegisterConstraint::Fixed(high)),
                    ..
                },
                MachineOperand {
                    kind: MachineOperandKind::Immediate(0),
                    ..
                }
            ] if *low == X86Register::Ax.physical() && *high == X86Register::Dx.physical()
        ));
        assert_eq!(
            instructions[instructions.len() - 3].opcode,
            X86Opcode::LowWord.machine_opcode()
        );
        assert_eq!(
            instructions[instructions.len() - 2].opcode,
            X86Opcode::HighWord.machine_opcode()
        );
        assert_eq!(
            instructions[instructions.len() - 3].operands[0].constraint,
            Some(RegisterConstraint::Fixed(X86Register::Ax.physical())),
            "the low return word is born in AX rather than copied there after frame teardown"
        );
        assert_eq!(
            instructions[instructions.len() - 2].operands[0].constraint,
            Some(RegisterConstraint::Fixed(X86Register::Dx.physical())),
            "the high return word is born in DX rather than copied there after frame teardown"
        );
    }

    #[test]
    fn selects_c_float_call_and_return_through_st0() {
        // Python cfront.raise_hir._Raise.invoke (1121-1152) materializes a
        // floating call result as one x87 value, and ret (515-520) returns
        // one such value.  Both sides of the C ABI therefore use ST0 rather
        // than the integer AX/DX result protocol.
        for float_kind in [FloatKind::Binary32, FloatKind::Binary64] {
            let float = TypeId::new(7);
            let mut types = basic_types();
            types.push(Type {
                id: float,
                kind: TypeKind::Float(float_kind),
            });
            let callee = Function {
                id: FunctionId::new(5),
                name: "identity_float".into(),
                signature: Signature {
                    result: float,
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::FarCdecl,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![Value {
                            id: ValueId::new(0),
                            type_id: float,
                        }],
                        kind: InstructionKind::Unary {
                            op: UnaryOp::FloatNegate,
                            operand: Operand::Constant(TypedConstant {
                                type_id: float,
                                value: Constant::Float("1.0".into()),
                            }),
                        },
                    }],
                    terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
                }],
            };
            let caller = Function {
                id: FunctionId::new(4),
                name: "main".into(),
                signature: Signature {
                    result: float,
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::FarCdecl,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: vec![Instruction {
                        id: crate::ir::InstructionId::new(0),
                        results: vec![Value {
                            id: ValueId::new(0),
                            type_id: float,
                        }],
                        kind: InstructionKind::Call {
                            callee: Callee::Direct(callee.id),
                            arguments: Vec::new(),
                            effects: Effects::NONE,
                        },
                    }],
                    terminator: Terminator::Return(Some(Operand::Value(ValueId::new(0)))),
                }],
            };
            let literal_callee = Function {
                id: FunctionId::new(6),
                name: "literal_float".into(),
                signature: Signature {
                    result: float,
                    parameters: Vec::new(),
                    variadic: false,
                    calling_convention: CallingConvention::FarCdecl,
                },
                linkage: Linkage::Internal,
                attributes: Vec::new(),
                parameters: Vec::new(),
                blocks: vec![Block {
                    id: BlockId::new(0),
                    instructions: Vec::new(),
                    terminator: Terminator::Return(Some(Operand::Constant(TypedConstant {
                        type_id: float,
                        value: Constant::Float("1.0".into()),
                    }))),
                }],
            };

            for (calling_convention, call_opcode, return_opcode) in [
                (
                    CallingConvention::C,
                    X86Opcode::CallNear,
                    X86Opcode::ReturnNear,
                ),
                (
                    CallingConvention::FarCdecl,
                    X86Opcode::CallFar,
                    X86Opcode::ReturnFar,
                ),
            ] {
                let mut caller = caller.clone();
                let mut callee = callee.clone();
                let mut literal_callee = literal_callee.clone();
                caller.signature.calling_convention = calling_convention;
                callee.signature.calling_convention = calling_convention;
                literal_callee.signature.calling_convention = calling_convention;
                let selected =
                    select_module(&module(types.clone(), vec![caller, callee, literal_callee]))
                        .expect("C floating calls and returns select through ST0");
                selected.verify().expect("selected Machine IR verifies");

                for function in &selected.functions {
                    let instructions = &function.blocks[0].instructions;
                    let returned = instructions.last().expect("function returns");
                    assert_eq!(returned.opcode, return_opcode.machine_opcode());
                    assert!(matches!(
                        returned.operands.first(),
                        Some(MachineOperand {
                            role: OperandRole::Use,
                            constraint: Some(RegisterConstraint::Fixed(register)),
                            ..
                        }) if *register == X86Register::St0.physical()
                    ));
                }
                let call = selected.functions[0].blocks[0]
                    .instructions
                    .iter()
                    .find(|instruction| instruction.opcode == call_opcode.machine_opcode())
                    .expect("caller has ABI call");
                assert!(matches!(
                    call.operands.last(),
                    Some(MachineOperand {
                        role: OperandRole::Def,
                        constraint: Some(RegisterConstraint::Fixed(register)),
                        ..
                    }) if *register == X86Register::St0.physical()
                ));
                assert_eq!(
                    selected.functions[2].blocks[0]
                        .instructions
                        .iter()
                        .map(|instruction| instruction.opcode)
                        .collect::<Vec<_>>(),
                    vec![
                        X86Opcode::X87LoadOne.machine_opcode(),
                        return_opcode.machine_opcode(),
                    ],
                    "a direct float literal is materialized on x87 before its ST0 return",
                );
            }
        }
    }
}
