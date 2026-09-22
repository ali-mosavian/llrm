//! Lowering from resolved HIR into portable SSA IR.
//!
//! This boundary lowers semantics that portable IR can represent exactly and
//! refuses the rest rather than inventing memory, call, or ABI behavior.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::old::{hir, ir};
use crate::support::diagnostic::Diagnostic;

use super::calls::{CallPlan, CallPlanError, calling_convention, plan_calls};
use super::globals::{GlobalPlan, GlobalPlanError, PlannedPlace, plan_globals};

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
    MissingCallPlan,
    MissingGlobalPlan,
    MissingPointerType,
    DuplicatePlace,
    MissingIndirectOffsetType,
    UnsupportedIndirectOffsetAddress,
    ReadOnlyStore,
    ZeroExtent,
    ExtentOverflow,
    ValueIdOverflow,
    InstructionIdOverflow,
    SynthesizedTypeIdOverflow,
    ParameterIndex,
    ParameterType,
    ParameterExtent,
    ParameterAddress,
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
    UnsupportedFunctionAbi {
        function: hir::FunctionId,
        distance: hir::CallDistance,
        cleanup: hir::StackCleanup,
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
    UnknownPlace {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: hir::InstructionId,
        place: hir::PlaceId,
    },
    AmbiguousPlace {
        function: hir::FunctionId,
        block: hir::BlockId,
        instruction: hir::InstructionId,
        place: hir::PlaceId,
    },
    InvalidPlace {
        function: hir::FunctionId,
        place: hir::PlaceId,
        property: InvalidProperty,
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
    CallPlan {
        error: CallPlanError,
    },
    GlobalPlan {
        error: GlobalPlanError,
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
            Self::UnsupportedFunctionAbi {
                function,
                distance,
                cleanup,
            } => write!(
                formatter,
                "cannot lower function {function}: unsupported ABI {distance:?} with {cleanup:?} cleanup"
            ),
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
            Self::UnknownPlace {
                function,
                block,
                instruction,
                place,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} references unknown place {place}"
            ),
            Self::AmbiguousPlace {
                function,
                block,
                instruction,
                place,
            } => write!(
                formatter,
                "function {function} block {block} instruction {instruction} references ambiguous place {place}"
            ),
            Self::InvalidPlace {
                function,
                place,
                property,
            } => write!(
                formatter,
                "function {function} place {place} has invalid property {property:?}"
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
            Self::CallPlan { error } => write!(formatter, "cannot plan runtime calls: {error}"),
            Self::GlobalPlan { error } => write!(formatter, "cannot plan static data: {error}"),
        }
    }
}

impl Error for LowerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CallPlan { error } => Some(error),
            Self::GlobalPlan { error } => Some(error),
            _ => None,
        }
    }
}

/// Lowers the representable scalar subset of one HIR module into portable IR.
///
/// IDs and declaration order are retained in their corresponding IR domains.
/// Features without an exact portable representation return [`LowerError`]
/// rather than being erased or approximated.
/// Lowers one module using the historic column-major default.
///
/// New source-frontends must call [`lower_module_with_array_order`] with their
/// resolved program setting. This entry point remains for scalar callers that
/// predate an explicit HIR program context.
pub fn lower_module(module: &hir::Module) -> Result<ir::Module, LowerError> {
    lower_module_with_array_order(module, hir::ArrayOrder::ColumnMajor)
}

/// Lowers one module with its source-level array dimension order.
pub fn lower_module_with_array_order(
    module: &hir::Module,
    array_order: hir::ArrayOrder,
) -> Result<ir::Module, LowerError> {
    let calls = plan_calls(module).map_err(|error| LowerError::CallPlan { error })?;
    let globals = plan_globals(module).map_err(|error| LowerError::GlobalPlan { error })?;
    let stack = plan_stack_places(module, &globals)?;
    // The frontend may retain a catalog of unused built-ins.  Synthesis must
    // follow the same reachability rule as ordinary type lowering so an
    // integer-only module does not acquire an irrelevant evaluation format.
    let mut required_types = required_type_ids(module);
    required_types.extend(indirect_offset_type_ids(module)?);
    required_types.extend(globals.source_types.iter().copied());
    let float_types = FloatTypes::new(module, &globals, &required_types)?;
    let lowerer = Lowerer {
        module,
        calls: &calls,
        globals: &globals,
        stack: &stack,
        float_types: &float_types,
        array_order,
    };
    // The QB frontend carries a catalog of built-in types and callables in
    // every module. Declarations that no lowered function references have no
    // portable-IR semantics, so they must not make an otherwise scalar module
    // fail merely because their representation is not implemented yet.
    let mut types = module
        .types
        .iter()
        .filter(|type_| required_types.contains(&type_.id))
        .map(|type_| lowerer.lower_type(type_))
        .collect::<Result<Vec<_>, _>>()?;
    let mut functions = module
        .functions
        .iter()
        .map(|function| lowerer.lower_function(function))
        .collect::<Result<Vec<_>, _>>()?;
    let declarations = calls
        .declarations
        .iter()
        .cloned()
        .map(|mut declaration| {
            declaration.signature.result =
                lowerer.evaluation_type(hir::TypeId::new(declaration.signature.result.get()))?;
            Ok(declaration)
        })
        .collect::<Result<Vec<_>, LowerError>>()?;
    drop(lowerer);
    types.extend(globals.extra_types);
    types.extend(float_types.declarations());
    functions.extend(declarations);
    let lowered = ir::Module {
        name: module.name.clone(),
        types,
        globals: globals.globals,
        functions,
    };
    lowered
        .verify()
        .map_err(|diagnostics| LowerError::Verification { diagnostics })?;
    Ok(lowered)
}

fn required_type_ids(module: &hir::Module) -> BTreeSet<hir::TypeId> {
    let mut required = BTreeSet::new();
    for function in &module.functions {
        required.insert(function.result_type);
        required.extend(function.values.iter().map(|value| value.type_id));
        for block in &function.blocks {
            for instruction in &block.instructions {
                for operand in &instruction.operands {
                    collect_operand_types(operand, &mut required);
                }
            }
            match &block.terminator {
                hir::Terminator::Branch { condition, .. } => {
                    collect_operand_types(condition, &mut required);
                }
                hir::Terminator::Switch { selector, .. } => {
                    collect_operand_types(selector, &mut required);
                }
                hir::Terminator::Return(Some(value)) => {
                    collect_operand_types(value, &mut required);
                }
                hir::Terminator::Jump(_)
                | hir::Terminator::Return(None)
                | hir::Terminator::Unreachable => {}
            }
        }
    }
    required
}

fn collect_operand_types(operand: &hir::Operand, required: &mut BTreeSet<hir::TypeId>) {
    match operand {
        hir::Operand::Constant { type_id, .. } | hir::Operand::Indirect { type_id, .. } => {
            required.insert(*type_id);
        }
        hir::Operand::Projection {
            type_id, indices, ..
        } => {
            required.insert(*type_id);
            for index in indices {
                collect_operand_types(index, required);
            }
        }
        hir::Operand::Element { indices, .. } => {
            for index in indices {
                collect_operand_types(index, required);
            }
        }
        hir::Operand::Value(_) | hir::Operand::Place(_) => {}
    }
}

fn used_values(function: &hir::Function) -> BTreeSet<hir::ValueId> {
    let mut used = BTreeSet::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                collect_operand_values(operand, &mut used);
            }
        }
        match &block.terminator {
            hir::Terminator::Branch { condition, .. } => {
                collect_operand_values(condition, &mut used);
            }
            hir::Terminator::Switch { selector, .. } => {
                collect_operand_values(selector, &mut used);
            }
            hir::Terminator::Return(Some(value)) => collect_operand_values(value, &mut used),
            hir::Terminator::Jump(_)
            | hir::Terminator::Return(None)
            | hir::Terminator::Unreachable => {}
        }
    }
    used
}

fn collect_operand_values(operand: &hir::Operand, used: &mut BTreeSet<hir::ValueId>) {
    match operand {
        hir::Operand::Value(value) => {
            used.insert(*value);
        }
        hir::Operand::Indirect { base, .. } => {
            used.insert(*base);
        }
        hir::Operand::Element { indices, .. } | hir::Operand::Projection { indices, .. } => {
            for index in indices {
                collect_operand_values(index, used);
            }
        }
        hir::Operand::Constant { .. } | hir::Operand::Place(_) => {}
    }
}

fn indirect_offset_type_ids(module: &hir::Module) -> Result<BTreeSet<hir::TypeId>, LowerError> {
    let mut required = BTreeSet::new();
    for function in &module.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if !matches!(instruction.opcode, hir::Opcode::Load | hir::Opcode::Store) {
                    continue;
                }
                let Some(hir::Operand::Indirect { base, offset, .. }) =
                    instruction.operands.first()
                else {
                    continue;
                };
                if *offset == 0 {
                    continue;
                }
                let Some(base) = function.values.iter().find(|value| value.id == *base) else {
                    continue;
                };
                let Some(pointer) = module.types.iter().find(|type_| type_.id == base.type_id)
                else {
                    continue;
                };
                if pointer.kind != hir::TypeKind::Pointer {
                    continue;
                }
                let offset_type = indirect_offset_type(module, pointer).map_err(|property| {
                    LowerError::InvalidInstruction {
                        function: function.id,
                        block: block.id,
                        instruction: instruction.id,
                        property,
                    }
                })?;
                required.insert(offset_type.id);
            }
        }
    }
    Ok(required)
}

fn indirect_offset_type<'module>(
    module: &'module hir::Module,
    pointer: &hir::Type,
) -> Result<&'module hir::Type, InvalidProperty> {
    let width = match pointer.address {
        hir::AddressKind::Near | hir::AddressKind::Far => 2,
        hir::AddressKind::Huge => pointer.width,
        hir::AddressKind::None | hir::AddressKind::Code | hir::AddressKind::Segment => {
            return Err(InvalidProperty::UnsupportedIndirectOffsetAddress);
        }
    };
    module
        .types
        .iter()
        .find(|type_| type_.kind == hir::TypeKind::Integer && type_.width == width)
        .ok_or(InvalidProperty::MissingIndirectOffsetType)
}

#[derive(Default)]
struct StackPlan {
    allocations: BTreeMap<hir::FunctionId, Vec<ir::Instruction>>,
    places: BTreeMap<(hir::FunctionId, hir::PlaceId), ir::Value>,
}

struct MemoryAddress {
    type_id: hir::TypeId,
    address: ir::Operand,
    pointer_type: ir::TypeId,
    address_kind: hir::AddressKind,
    readonly: bool,
    volatile: bool,
}

fn plan_stack_places(module: &hir::Module, globals: &GlobalPlan) -> Result<StackPlan, LowerError> {
    let mut plan = StackPlan::default();
    for function in &module.functions {
        let mut seen = BTreeSet::new();
        let mut parameter_places = BTreeSet::new();
        let mut next_value = function
            .values
            .iter()
            .map(|value| value.id.get())
            .max()
            .map_or(Some(0), |maximum| maximum.checked_add(1));
        let mut next_instruction = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .map(|instruction| instruction.id.get())
            .max()
            .map_or(Some(0), |maximum| maximum.checked_add(1));

        for place in &function.places {
            if !seen.insert(place.id) {
                return Err(LowerError::InvalidPlace {
                    function: function.id,
                    place: place.id,
                    property: InvalidProperty::DuplicatePlace,
                });
            }
            let parameter = match place.storage {
                hir::Storage::Local => None,
                hir::Storage::Parameter { index } => Some(index),
                hir::Storage::Static
                | hir::Storage::Module
                | hir::Storage::Common
                | hir::Storage::External => continue,
            };
            if place.extent == 0 {
                return Err(LowerError::InvalidPlace {
                    function: function.id,
                    place: place.id,
                    property: InvalidProperty::ZeroExtent,
                });
            }
            let size = u32::try_from(place.extent).map_err(|_| LowerError::InvalidPlace {
                function: function.id,
                place: place.id,
                property: InvalidProperty::ExtentOverflow,
            })?;
            let pointer_type = globals.pointer_types.get(&place.address).copied().ok_or(
                LowerError::InvalidPlace {
                    function: function.id,
                    place: place.id,
                    property: InvalidProperty::MissingPointerType,
                },
            )?;
            if let Some(index) = parameter {
                let Some(parameter_id) = usize::try_from(index)
                    .ok()
                    .and_then(|index| function.parameters.get(index))
                else {
                    return Err(LowerError::InvalidPlace {
                        function: function.id,
                        place: place.id,
                        property: InvalidProperty::ParameterIndex,
                    });
                };
                if !parameter_places.insert(index) {
                    return Err(LowerError::InvalidPlace {
                        function: function.id,
                        place: place.id,
                        property: InvalidProperty::DuplicatePlace,
                    });
                }
                let parameter_type = function
                    .values
                    .iter()
                    .find(|value| value.id == *parameter_id)
                    .map(|value| value.type_id);
                if parameter_type != Some(place.type_id) {
                    return Err(LowerError::InvalidPlace {
                        function: function.id,
                        place: place.id,
                        property: InvalidProperty::ParameterType,
                    });
                }
                let type_width = module
                    .types
                    .iter()
                    .find(|type_| type_.id == place.type_id)
                    .map(|type_| type_.width);
                if type_width != Some(place.extent) {
                    return Err(LowerError::InvalidPlace {
                        function: function.id,
                        place: place.id,
                        property: InvalidProperty::ParameterExtent,
                    });
                }
                if place.address != hir::AddressKind::Near || place.offset != 0 {
                    return Err(LowerError::InvalidPlace {
                        function: function.id,
                        place: place.id,
                        property: InvalidProperty::ParameterAddress,
                    });
                }
            }
            let value_id = next_value.ok_or(LowerError::InvalidPlace {
                function: function.id,
                place: place.id,
                property: InvalidProperty::ValueIdOverflow,
            })?;
            next_value = value_id.checked_add(1);
            let instruction_id = next_instruction.ok_or(LowerError::InvalidPlace {
                function: function.id,
                place: place.id,
                property: InvalidProperty::InstructionIdOverflow,
            })?;
            next_instruction = instruction_id.checked_add(1);

            let value = ir::Value {
                id: ir::ValueId::new(value_id),
                type_id: pointer_type,
            };
            if plan
                .places
                .insert((function.id, place.id), value.clone())
                .is_some()
            {
                return Err(LowerError::InvalidPlace {
                    function: function.id,
                    place: place.id,
                    property: InvalidProperty::DuplicatePlace,
                });
            }
            plan.allocations
                .entry(function.id)
                .or_default()
                .push(ir::Instruction {
                    id: ir::InstructionId::new(instruction_id),
                    results: vec![value],
                    kind: parameter.map_or(
                        ir::InstructionKind::StackAlloc {
                            size,
                            alignment: 1,
                            address_space: lower_address_kind(place.address),
                        },
                        |parameter| ir::InstructionKind::ParameterAddress { parameter },
                    ),
                });
        }
    }
    Ok(plan)
}

struct Lowerer<'module> {
    module: &'module hir::Module,
    calls: &'module CallPlan,
    globals: &'module GlobalPlan,
    stack: &'module StackPlan,
    float_types: &'module FloatTypes,
    array_order: hir::ArrayOrder,
}

/// The source type id always names storage.  When evaluation needs a wider
/// format, this table gives that format one fresh, module-wide IR type id.
/// Keeping it separate from HIR ids makes the ABI/storage boundary explicit.
struct FloatTypes {
    binary32: Option<ir::TypeId>,
    binary64: Option<ir::TypeId>,
    extended80: Option<ir::TypeId>,
}

impl FloatTypes {
    fn new(
        module: &hir::Module,
        globals: &GlobalPlan,
        required: &BTreeSet<hir::TypeId>,
    ) -> Result<Self, LowerError> {
        let mut needed = [false; 3];
        for type_ in module
            .types
            .iter()
            .filter(|type_| required.contains(&type_.id))
        {
            if type_.kind != hir::TypeKind::Float {
                continue;
            }
            let storage = storage_float_kind(type_)?;
            let evaluation = evaluation_float_kind(type_)?;
            if storage != evaluation {
                needed[float_kind_index(evaluation)] = true;
            }
        }
        let maximum = module
            .types
            .iter()
            .map(|type_| type_.id.get())
            .chain(globals.extra_types.iter().map(|type_| type_.id.get()))
            .max();
        let mut next = maximum.map_or(Some(0), |id| id.checked_add(1));
        let mut ids = [None; 3];
        for index in 0..ids.len() {
            if needed[index] {
                let id = next.ok_or(LowerError::InvalidType {
                    type_id: hir::TypeId::new(u32::MAX),
                    property: InvalidProperty::SynthesizedTypeIdOverflow,
                })?;
                next = id.checked_add(1);
                ids[index] = Some(ir::TypeId::new(id));
            }
        }
        Ok(Self {
            binary32: ids[0],
            binary64: ids[1],
            extended80: ids[2],
        })
    }

    fn id(&self, kind: ir::FloatKind) -> Option<ir::TypeId> {
        match kind {
            ir::FloatKind::Binary32 => self.binary32,
            ir::FloatKind::Binary64 => self.binary64,
            ir::FloatKind::Extended80 => self.extended80,
        }
    }

    fn declarations(&self) -> Vec<ir::Type> {
        [
            (self.binary32, ir::FloatKind::Binary32),
            (self.binary64, ir::FloatKind::Binary64),
            (self.extended80, ir::FloatKind::Extended80),
        ]
        .into_iter()
        .filter_map(|(id, kind)| {
            id.map(|id| ir::Type {
                id,
                kind: ir::TypeKind::Float(kind),
            })
        })
        .collect()
    }
}

fn float_kind_index(kind: ir::FloatKind) -> usize {
    match kind {
        ir::FloatKind::Binary32 => 0,
        ir::FloatKind::Binary64 => 1,
        ir::FloatKind::Extended80 => 2,
    }
}

fn storage_float_kind(type_: &hir::Type) -> Result<ir::FloatKind, LowerError> {
    match type_.width {
        4 => Ok(ir::FloatKind::Binary32),
        8 => Ok(ir::FloatKind::Binary64),
        10 => Ok(ir::FloatKind::Extended80),
        0 => Err(LowerError::InvalidType {
            type_id: type_.id,
            property: InvalidProperty::ZeroWidth,
        }),
        _ => Err(LowerError::InvalidType {
            type_id: type_.id,
            property: InvalidProperty::WidthOverflow,
        }),
    }
}

fn evaluation_float_kind(type_: &hir::Type) -> Result<ir::FloatKind, LowerError> {
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

struct GeneratedIds {
    next_value: Option<u32>,
    next_instruction: Option<u32>,
}

impl GeneratedIds {
    fn for_function(function: &hir::Function, stack: &StackPlan) -> Self {
        let next_value = function
            .values
            .iter()
            .map(|value| value.id.get())
            .chain(
                stack
                    .allocations
                    .get(&function.id)
                    .into_iter()
                    .flatten()
                    .flat_map(|instruction| instruction.results.iter().map(|value| value.id.get())),
            )
            .max()
            .map_or(Some(0), |maximum| maximum.checked_add(1));
        let next_instruction = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|instruction| instruction.id.get())
            })
            .chain(
                stack
                    .allocations
                    .get(&function.id)
                    .into_iter()
                    .flatten()
                    .map(|instruction| instruction.id.get()),
            )
            .max()
            .map_or(Some(0), |maximum| maximum.checked_add(1));
        Self {
            next_value,
            next_instruction,
        }
    }

    fn value(&mut self, function: hir::FunctionId) -> Result<ir::ValueId, LowerError> {
        let id = self.next_value.ok_or(LowerError::InvalidFunction {
            function,
            property: InvalidProperty::ValueIdOverflow,
        })?;
        self.next_value = id.checked_add(1);
        Ok(ir::ValueId::new(id))
    }

    fn instruction(&mut self, function: hir::FunctionId) -> Result<ir::InstructionId, LowerError> {
        let id = self.next_instruction.ok_or(LowerError::InvalidFunction {
            function,
            property: InvalidProperty::InstructionIdOverflow,
        })?;
        self.next_instruction = id.checked_add(1);
        Ok(ir::InstructionId::new(id))
    }
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
                address_space: lower_address_kind(type_.address),
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
        storage_float_kind(type_)
    }

    fn evaluation_type(&self, type_id: hir::TypeId) -> Result<ir::TypeId, LowerError> {
        let type_ = self.type_by_id(type_id)?;
        if type_.kind == hir::TypeKind::Pointer {
            return self
                .globals
                .pointer_types
                .get(&type_.address)
                .copied()
                .ok_or(LowerError::InvalidType {
                    type_id,
                    property: InvalidProperty::MissingPointerType,
                });
        }
        if type_.kind != hir::TypeKind::Float {
            return Ok(ir::TypeId::new(type_id.get()));
        }
        let storage = storage_float_kind(type_)?;
        let evaluation = evaluation_float_kind(type_)?;
        Ok(if storage == evaluation {
            ir::TypeId::new(type_id.get())
        } else {
            self.float_types
                .id(evaluation)
                .ok_or(LowerError::InvalidType {
                    type_id,
                    property: InvalidProperty::MissingFloatEvaluation,
                })?
        })
    }

    fn is_split_float(&self, type_id: hir::TypeId) -> Result<bool, LowerError> {
        let type_ = self.type_by_id(type_id)?;
        Ok(type_.kind == hir::TypeKind::Float
            && storage_float_kind(type_)? != evaluation_float_kind(type_)?)
    }

    fn lower_function(&self, function: &hir::Function) -> Result<ir::Function, LowerError> {
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

        let mut generated = GeneratedIds::for_function(function, self.stack);
        let mut parameter_extends = Vec::new();
        let used_values = used_values(function);
        let parameters = function
            .parameters
            .iter()
            .map(|id| {
                let value = self.value_by_id(function, *id, function.entry, None)?;
                if !self.is_split_float(value.type_id)? {
                    return self.lower_value(function, *id, function.entry, None);
                }
                if !used_values.contains(id) {
                    return Ok(ir::Value {
                        id: value_id(value.id),
                        type_id: type_id(value.type_id),
                    });
                }
                let raw = ir::Value {
                    id: generated.value(function.id)?,
                    type_id: ir::TypeId::new(value.type_id.get()),
                };
                parameter_extends.push(ir::Instruction {
                    id: generated.instruction(function.id)?,
                    results: vec![ir::Value {
                        id: value_id(value.id),
                        type_id: self.evaluation_type(value.type_id)?,
                    }],
                    kind: ir::InstructionKind::Cast {
                        op: ir::CastOp::FloatExtend,
                        operand: ir::Operand::Value(raw.id),
                        to: self.evaluation_type(value.type_id)?,
                    },
                });
                Ok(raw)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let calling_convention = calling_convention(function.abi.distance, function.abi.cleanup)
            .ok_or(LowerError::UnsupportedFunctionAbi {
                function: function.id,
                distance: function.abi.distance,
                cleanup: function.abi.cleanup,
            })?;
        let signature = ir::Signature {
            result: self.evaluation_type(function.result_type)?,
            parameters: parameters
                .iter()
                .map(|parameter| parameter.type_id)
                .collect(),
            variadic: false,
            calling_convention,
        };
        let mut blocks = function
            .blocks
            .iter()
            .map(|block| self.lower_block(function, block, &mut generated))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(allocations) = self.stack.allocations.get(&function.id) {
            let entry = blocks.first_mut().expect("entry block was checked above");
            let insertion = entry
                .instructions
                .iter()
                .take_while(|instruction| {
                    matches!(instruction.kind, ir::InstructionKind::Phi { .. })
                })
                .count();
            entry
                .instructions
                .splice(insertion..insertion, allocations.iter().cloned());
        }
        if !parameter_extends.is_empty() {
            let entry = blocks.first_mut().expect("entry block was checked above");
            let insertion = entry
                .instructions
                .iter()
                .take_while(|instruction| {
                    matches!(instruction.kind, ir::InstructionKind::Phi { .. })
                })
                .count();
            entry
                .instructions
                .splice(insertion..insertion, parameter_extends);
        }

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
        generated: &mut GeneratedIds,
    ) -> Result<ir::Block, LowerError> {
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for instruction in &block.instructions {
            if self.split_memory_access(function, block.id, instruction)? {
                instructions.extend(self.lower_split_memory_access(
                    function,
                    block.id,
                    instruction,
                    generated,
                )?);
                continue;
            }
            if self.split_float_operation(function, block.id, instruction)? {
                instructions.extend(self.lower_split_float_operation(
                    function,
                    block.id,
                    instruction,
                    generated,
                )?);
                continue;
            }
            if self.split_float_call(function, block.id, instruction)? {
                instructions.extend(self.lower_split_float_call(
                    function,
                    block.id,
                    instruction,
                    generated,
                )?);
                continue;
            }
            let prefixes =
                self.memory_address_prefix(function, block.id, instruction, generated)?;
            if let Some(prefix) = prefixes.last() {
                let address = ir::Operand::Value(prefix.results[0].id);
                let mut access = self.lower_instruction(function, block.id, instruction)?;
                match &mut access.kind {
                    ir::InstructionKind::Load {
                        address: access, ..
                    }
                    | ir::InstructionKind::Store {
                        address: access, ..
                    } => *access = address,
                    _ => unreachable!("only load/store can have an indirect-offset prefix"),
                }
                instructions.extend(prefixes);
                instructions.push(access);
            } else {
                instructions.push(self.lower_instruction(function, block.id, instruction)?);
            }
        }
        Ok(ir::Block {
            id: block_id(block.id),
            instructions,
            terminator: self.lower_terminator(function, block)?,
        })
    }

    fn split_float_call(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<bool, LowerError> {
        if instruction.opcode != hir::Opcode::Call {
            return Ok(false);
        }
        if let [result] = instruction.results.as_slice() {
            if self.is_split_float(
                self.value_by_id(function, *result, block, Some(instruction.id))?
                    .type_id,
            )? {
                return Ok(true);
            }
        }
        for (index, operand) in instruction.operands.iter().enumerate() {
            match operand {
                hir::Operand::Place(place) => {
                    let place = self.find_place(function, block, instruction, *place)?;
                    if self.is_float(place.type_id)? {
                        return Ok(true);
                    }
                }
                hir::Operand::Projection { type_id, .. } => {
                    if self.is_float(*type_id)? {
                        return Ok(true);
                    }
                }
                _ => {
                    let type_id =
                        self.operand_type(function, block, Some(instruction.id), index, operand)?;
                    if self.is_split_float(type_id)? {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn lower_split_float_call(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        generated: &mut GeneratedIds,
    ) -> Result<Vec<ir::Instruction>, LowerError> {
        let planned = self.calls.sites.get(&(function.id, instruction.id)).ok_or(
            LowerError::InvalidInstruction {
                function: function.id,
                block,
                instruction: instruction.id,
                property: InvalidProperty::MissingCallPlan,
            },
        )?;
        let mut prefixes = Vec::new();
        let arguments = planned
            .argument_indices
            .iter()
            .map(|index| {
                let operand =
                    instruction
                        .operands
                        .get(*index)
                        .ok_or(LowerError::InvalidInstruction {
                            function: function.id,
                            block,
                            instruction: instruction.id,
                            property: InvalidProperty::MissingCallPlan,
                        })?;
                self.storage_call_operand(
                    function,
                    block,
                    instruction,
                    *index,
                    operand,
                    generated,
                    &mut prefixes,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        prefixes.push(ir::Instruction {
            id: instruction_id(instruction.id),
            results: instruction
                .results
                .iter()
                .map(|result| self.lower_value(function, *result, block, Some(instruction.id)))
                .collect::<Result<Vec<_>, _>>()?,
            kind: ir::InstructionKind::Call {
                callee: ir::Callee::Direct(planned.target),
                arguments,
                effects: ir::Effects {
                    memory: ir::MemoryEffects::Unknown,
                    may_trap: true,
                    observable: true,
                },
            },
        });
        Ok(prefixes)
    }

    fn storage_call_operand(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        index: usize,
        operand: &hir::Operand,
        generated: &mut GeneratedIds,
        prefixes: &mut Vec<ir::Instruction>,
    ) -> Result<ir::Operand, LowerError> {
        let type_id = match operand {
            hir::Operand::Place(place) => {
                self.find_place(function, block, instruction, *place)?
                    .type_id
            }
            hir::Operand::Projection { type_id, .. } => *type_id,
            _ => self.operand_type(function, block, Some(instruction.id), index, operand)?,
        };
        match operand {
            hir::Operand::Place(place_id) => self.load_call_place(
                function,
                block,
                instruction,
                index,
                *place_id,
                type_id,
                generated,
                prefixes,
            ),
            hir::Operand::Projection {
                place,
                indices,
                offset,
                ..
            } => self.load_call_projection(
                function,
                block,
                instruction,
                index,
                *place,
                indices,
                *offset,
                type_id,
                generated,
                prefixes,
            ),
            _ if !self.is_split_float(type_id)? => {
                self.lower_operand(function, block, Some(instruction.id), index, operand)
            }
            // Rvalue values are evaluation-format values.  Constants have
            // storage-format semantics already, so they enter the ABI direct.
            hir::Operand::Value(_) => {
                let stored = ir::Value {
                    id: generated.value(function.id)?,
                    type_id: ir::TypeId::new(type_id.get()),
                };
                prefixes.push(ir::Instruction {
                    id: generated.instruction(function.id)?,
                    results: vec![stored.clone()],
                    kind: ir::InstructionKind::Cast {
                        op: ir::CastOp::FloatTruncate,
                        operand: self.lower_operand(
                            function,
                            block,
                            Some(instruction.id),
                            index,
                            operand,
                        )?,
                        to: stored.type_id,
                    },
                });
                Ok(ir::Operand::Value(stored.id))
            }
            hir::Operand::Constant { .. } => {
                self.lower_operand(function, block, Some(instruction.id), index, operand)
            }
            _ => Err(self.unsupported_operand(
                function,
                block,
                Some(instruction.id),
                index,
                UnsupportedOperand::Place,
            )),
        }
    }

    fn load_call_place(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        _index: usize,
        place_id: hir::PlaceId,
        type_id: hir::TypeId,
        generated: &mut GeneratedIds,
        prefixes: &mut Vec<ir::Instruction>,
    ) -> Result<ir::Operand, LowerError> {
        let place = self.find_place(function, block, instruction, place_id)?;
        if place.type_id != type_id {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        let address =
            if let Some(planned) = self.globals.places.get(&(function.id, place_id)).copied() {
                global_address(planned, planned.pointer_type)
            } else if let Some(value) = self.stack.places.get(&(function.id, place_id)) {
                ir::Operand::Value(value.id)
            } else {
                return self.invalid_instruction(
                    function,
                    block,
                    instruction,
                    InvalidProperty::MissingGlobalPlan,
                );
            };
        let value = ir::Value {
            id: generated.value(function.id)?,
            type_id: ir::TypeId::new(type_id.get()),
        };
        prefixes.push(ir::Instruction {
            id: generated.instruction(function.id)?,
            results: vec![value.clone()],
            kind: ir::InstructionKind::Load {
                address,
                alignment: 1,
                volatile: false,
            },
        });
        Ok(ir::Operand::Value(value.id))
    }

    fn load_call_projection(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        _index: usize,
        place_id: hir::PlaceId,
        indices: &[hir::Operand],
        offset: usize,
        type_id: hir::TypeId,
        generated: &mut GeneratedIds,
        prefixes: &mut Vec<ir::Instruction>,
    ) -> Result<ir::Operand, LowerError> {
        let place = self.find_place(function, block, instruction, place_id)?;
        let (base, pointer_type) =
            if let Some(planned) = self.globals.places.get(&(function.id, place_id)).copied() {
                (
                    global_address(planned, planned.pointer_type),
                    planned.pointer_type,
                )
            } else if let Some(value) = self.stack.places.get(&(function.id, place_id)) {
                (ir::Operand::Value(value.id), value.type_id)
            } else {
                return self.invalid_instruction(
                    function,
                    block,
                    instruction,
                    InvalidProperty::MissingGlobalPlan,
                );
            };
        let field = self.type_by_id(type_id)?;
        let byte_offset = if indices.is_empty() {
            self.validate_projection_field(function, block, instruction, place, field, offset)?;
            if offset == 0 {
                None
            } else {
                let offset_type =
                    self.offset_index_type(function, block, instruction, place.address)?;
                Some(ir::Operand::Constant(ir::TypedConstant {
                    type_id: ir::TypeId::new(offset_type.id.get()),
                    value: ir::Constant::Integer(offset as i128),
                }))
            }
        } else {
            let (array, element) = self.array_element_type(function, block, instruction, place)?;
            self.validate_projection_field(function, block, instruction, place, field, offset)?;
            Some(self.array_byte_offset(
                function,
                block,
                instruction,
                indices,
                array,
                element,
                offset,
                generated,
                prefixes,
            )?)
        };
        let address = if let Some(byte_offset) = byte_offset {
            let address = ir::Value {
                id: generated.value(function.id)?,
                type_id: pointer_type,
            };
            prefixes.push(ir::Instruction {
                id: generated.instruction(function.id)?,
                results: vec![address.clone()],
                kind: ir::InstructionKind::GetElementPointer {
                    base,
                    indices: vec![byte_offset],
                },
            });
            ir::Operand::Value(address.id)
        } else {
            base
        };
        let value = ir::Value {
            id: generated.value(function.id)?,
            type_id: ir::TypeId::new(type_id.get()),
        };
        prefixes.push(ir::Instruction {
            id: generated.instruction(function.id)?,
            results: vec![value.clone()],
            kind: ir::InstructionKind::Load {
                address,
                alignment: 1,
                volatile: false,
            },
        });
        Ok(ir::Operand::Value(value.id))
    }

    fn find_place<'function>(
        &self,
        function: &'function hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        id: hir::PlaceId,
    ) -> Result<&'function hir::Place, LowerError> {
        let mut places = function.places.iter().filter(|place| place.id == id);
        let Some(place) = places.next() else {
            return Err(LowerError::UnknownPlace {
                function: function.id,
                block,
                instruction: instruction.id,
                place: id,
            });
        };
        if places.next().is_some() {
            return Err(LowerError::AmbiguousPlace {
                function: function.id,
                block,
                instruction: instruction.id,
                place: id,
            });
        }
        Ok(place)
    }

    fn split_float_operation(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<bool, LowerError> {
        let relevant = matches!(
            instruction.opcode,
            hir::Opcode::Copy
                | hir::Opcode::Convert
                | hir::Opcode::Equal
                | hir::Opcode::NotEqual
                | hir::Opcode::LessThan
                | hir::Opcode::LessEqual
                | hir::Opcode::GreaterThan
                | hir::Opcode::GreaterEqual
                | hir::Opcode::FloatAdd
                | hir::Opcode::FloatSubtract
                | hir::Opcode::FloatMultiply
                | hir::Opcode::FloatDivide
                | hir::Opcode::FloatNegate
                | hir::Opcode::FloatAbsolute
                | hir::Opcode::FloatSquareRoot
                | hir::Opcode::FloatSine
                | hir::Opcode::FloatCosine
                | hir::Opcode::FloatArctangent
                | hir::Opcode::FloatLog2
                | hir::Opcode::FloatExp2
                | hir::Opcode::FloatToInteger { .. }
        );
        if !relevant {
            return Ok(false);
        }
        for (index, operand) in instruction.operands.iter().enumerate() {
            let type_id =
                self.operand_type(function, block, Some(instruction.id), index, operand)?;
            if self.is_split_float(type_id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn lower_split_float_operation(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        generated: &mut GeneratedIds,
    ) -> Result<Vec<ir::Instruction>, LowerError> {
        let mut prefixes = Vec::new();
        let mut lowered = self.lower_instruction(function, block, instruction)?;
        let mut operands = Vec::with_capacity(instruction.operands.len());
        for (index, operand) in instruction.operands.iter().enumerate() {
            operands.push(self.evaluated_float_operand(
                function,
                block,
                instruction,
                index,
                operand,
                generated,
                &mut prefixes,
            )?);
        }
        match &mut lowered.kind {
            ir::InstructionKind::Binary { left, right, .. }
            | ir::InstructionKind::Compare { left, right, .. } => {
                *left = operands[0].clone();
                *right = operands[1].clone();
            }
            ir::InstructionKind::Unary { operand, .. }
            | ir::InstructionKind::Cast { operand, .. } => *operand = operands[0].clone(),
            ir::InstructionKind::Intrinsic { arguments, .. } => *arguments = operands,
            _ => unreachable!("only scalar float operations take the split path"),
        }
        prefixes.push(lowered);
        Ok(prefixes)
    }

    /// A real constant remains typed as its storage format before the extend.
    /// `Constant::Float` is textual today, so exact source bits remain a risk
    /// until constants carry an exact binary representation.
    fn evaluated_float_operand(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        index: usize,
        operand: &hir::Operand,
        generated: &mut GeneratedIds,
        prefixes: &mut Vec<ir::Instruction>,
    ) -> Result<ir::Operand, LowerError> {
        let type_id = self.operand_type(function, block, Some(instruction.id), index, operand)?;
        if !self.is_split_float(type_id)? {
            return self.lower_operand(function, block, Some(instruction.id), index, operand);
        }
        match operand {
            hir::Operand::Constant { .. } => {
                let value = ir::Value {
                    id: generated.value(function.id)?,
                    type_id: self.evaluation_type(type_id)?,
                };
                prefixes.push(ir::Instruction {
                    id: generated.instruction(function.id)?,
                    results: vec![value.clone()],
                    kind: ir::InstructionKind::Cast {
                        op: ir::CastOp::FloatExtend,
                        operand: self.lower_operand(
                            function,
                            block,
                            Some(instruction.id),
                            index,
                            operand,
                        )?,
                        to: value.type_id,
                    },
                });
                Ok(ir::Operand::Value(value.id))
            }
            _ => self.lower_operand(function, block, Some(instruction.id), index, operand),
        }
    }

    fn split_memory_access(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<bool, LowerError> {
        if !matches!(instruction.opcode, hir::Opcode::Load | hir::Opcode::Store) {
            return Ok(false);
        }
        let memory = self.memory_address(
            function,
            block,
            instruction,
            0,
            if instruction.opcode == hir::Opcode::Load {
                1
            } else {
                2
            },
        )?;
        self.is_split_float(memory.type_id)
    }

    fn lower_split_memory_access(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        generated: &mut GeneratedIds,
    ) -> Result<Vec<ir::Instruction>, LowerError> {
        let arity = if instruction.opcode == hir::Opcode::Load {
            1
        } else {
            2
        };
        let mut result = Vec::new();
        let mut memory = self.memory_address(function, block, instruction, 0, arity)?;
        let prefixes = self.memory_address_prefix(function, block, instruction, generated)?;
        if let Some(prefix) = prefixes.last() {
            memory.address = ir::Operand::Value(prefix.results[0].id);
            result.extend(prefixes);
        }
        let storage = ir::TypeId::new(memory.type_id.get());
        let evaluation = self.evaluation_type(memory.type_id)?;
        match instruction.opcode {
            hir::Opcode::Load => {
                let loaded = ir::Value {
                    id: generated.value(function.id)?,
                    type_id: storage,
                };
                result.push(ir::Instruction {
                    id: instruction_id(instruction.id),
                    results: vec![loaded.clone()],
                    kind: ir::InstructionKind::Load {
                        address: memory.address,
                        alignment: 1,
                        volatile: memory.volatile,
                    },
                });
                result.push(ir::Instruction {
                    id: generated.instruction(function.id)?,
                    results: vec![self.one_result(function, block, instruction)?],
                    kind: ir::InstructionKind::Cast {
                        op: ir::CastOp::FloatExtend,
                        operand: ir::Operand::Value(loaded.id),
                        to: evaluation,
                    },
                });
            }
            hir::Opcode::Store => {
                if memory.readonly {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::ReadOnlyStore,
                    );
                }
                if !instruction.results.is_empty() {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::ResultArity,
                    );
                }
                let value_type = self.operand_type(
                    function,
                    block,
                    Some(instruction.id),
                    1,
                    &instruction.operands[1],
                )?;
                if value_type != memory.type_id {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::OperandTypes,
                    );
                }
                let stored_value = match &instruction.operands[1] {
                    // A literal is already represented in its declared
                    // storage format; truncating storage to itself is not a
                    // valid cast and would discard that fact.
                    hir::Operand::Constant { .. } => self.lower_operand(
                        function,
                        block,
                        Some(instruction.id),
                        1,
                        &instruction.operands[1],
                    )?,
                    _ => {
                        let stored = ir::Value {
                            id: generated.value(function.id)?,
                            type_id: storage,
                        };
                        result.push(ir::Instruction {
                            id: generated.instruction(function.id)?,
                            results: vec![stored.clone()],
                            kind: ir::InstructionKind::Cast {
                                op: ir::CastOp::FloatTruncate,
                                operand: self.lower_operand(
                                    function,
                                    block,
                                    Some(instruction.id),
                                    1,
                                    &instruction.operands[1],
                                )?,
                                to: storage,
                            },
                        });
                        ir::Operand::Value(stored.id)
                    }
                };
                result.push(ir::Instruction {
                    id: instruction_id(instruction.id),
                    results: Vec::new(),
                    kind: ir::InstructionKind::Store {
                        address: memory.address,
                        value: stored_value,
                        alignment: 1,
                        volatile: memory.volatile,
                    },
                });
            }
            _ => unreachable!("split memory access was checked above"),
        }
        Ok(result)
    }

    /// Materializes the byte-addressed offset and indices of a memory operand.
    ///
    /// HIR keeps a projected place as a source-level lvalue.  Portable IR has
    /// only pointers, so every non-root lvalue becomes one explicit GEP before
    /// the eventual load or store.  Keeping this beside indirect offsets makes
    /// the same address path serve ordinary and split-float storage accesses.
    fn memory_address_prefix(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        generated: &mut GeneratedIds,
    ) -> Result<Vec<ir::Instruction>, LowerError> {
        if !matches!(instruction.opcode, hir::Opcode::Load | hir::Opcode::Store) {
            return Ok(Vec::new());
        }
        let memory = self.memory_address(
            function,
            block,
            instruction,
            0,
            if instruction.opcode == hir::Opcode::Load {
                1
            } else {
                2
            },
        )?;
        let Some(operand) = instruction.operands.first() else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        };
        let mut prefixes = Vec::new();
        let byte_offset = match operand {
            hir::Operand::Indirect { offset, .. } => {
                if *offset == 0 {
                    return Ok(prefixes);
                }
                let offset_type =
                    self.offset_index_type(function, block, instruction, memory.address_kind)?;
                ir::Operand::Constant(ir::TypedConstant {
                    type_id: type_id(offset_type.id),
                    value: ir::Constant::Integer(*offset as i128),
                })
            }
            hir::Operand::Element { place, indices } => {
                let place = self.find_place(function, block, instruction, *place)?;
                let (array, element) =
                    self.array_element_type(function, block, instruction, place)?;
                self.array_byte_offset(
                    function,
                    block,
                    instruction,
                    indices,
                    array,
                    element,
                    0,
                    generated,
                    &mut prefixes,
                )?
            }
            hir::Operand::Projection {
                place,
                indices,
                offset,
                type_id: projected_type,
            } => {
                let place = self.find_place(function, block, instruction, *place)?;
                let field = self.type_by_id(*projected_type)?;
                if indices.is_empty() {
                    self.validate_projection_field(
                        function,
                        block,
                        instruction,
                        place,
                        field,
                        *offset,
                    )?;
                    if *offset == 0 {
                        return Ok(prefixes);
                    }
                    let offset_type =
                        self.offset_index_type(function, block, instruction, memory.address_kind)?;
                    ir::Operand::Constant(ir::TypedConstant {
                        type_id: type_id(offset_type.id),
                        value: ir::Constant::Integer(*offset as i128),
                    })
                } else {
                    let (array, element) =
                        self.array_element_type(function, block, instruction, place)?;
                    self.validate_projection_field(
                        function,
                        block,
                        instruction,
                        place,
                        field,
                        *offset,
                    )?;
                    self.array_byte_offset(
                        function,
                        block,
                        instruction,
                        indices,
                        array,
                        element,
                        *offset,
                        generated,
                        &mut prefixes,
                    )?
                }
            }
            hir::Operand::Place(_) => return Ok(prefixes),
            hir::Operand::Value(_) | hir::Operand::Constant { .. } => {
                return self.invalid_instruction(
                    function,
                    block,
                    instruction,
                    InvalidProperty::OperandTypes,
                );
            }
        };
        let value = ir::Value {
            id: generated.value(function.id)?,
            type_id: memory.pointer_type,
        };
        prefixes.push(ir::Instruction {
            id: generated.instruction(function.id)?,
            results: vec![value],
            kind: ir::InstructionKind::GetElementPointer {
                base: memory.address,
                indices: vec![byte_offset],
            },
        });
        Ok(prefixes)
    }

    fn array_element_type(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        place: &hir::Place,
    ) -> Result<(&hir::Type, &hir::Type), LowerError> {
        let array = self.type_by_id(place.type_id)?;
        let Some(element_id) = (array.kind == hir::TypeKind::Array)
            .then_some(array.element)
            .flatten()
        else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        };
        let element = self.type_by_id(element_id)?;
        Ok((array, element))
    }

    fn validate_projection_field(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        place: &hir::Place,
        field: &hir::Type,
        offset: usize,
    ) -> Result<(), LowerError> {
        let root = self.type_by_id(place.type_id)?;
        let extent = if root.kind == hir::TypeKind::Array {
            root.element
                .map(|element| self.type_by_id(element))
                .transpose()?
                .map_or(root.width, |element| element.width)
        } else {
            root.width
        };
        if offset
            .checked_add(field.width)
            .is_none_or(|end| end > extent)
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(())
    }

    /// Ports Python HIR's array linearization exactly: adjust each declared
    /// lower bound, accumulate dimensions in the frontend's order, scale to
    /// bytes once, then add a projected field displacement.  IR GEP consumes
    /// that one byte index; it must not be handed source subscripts.
    fn array_byte_offset(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        indices: &[hir::Operand],
        array: &hir::Type,
        element: &hir::Type,
        field_offset: usize,
        generated: &mut GeneratedIds,
        prefixes: &mut Vec<ir::Instruction>,
    ) -> Result<ir::Operand, LowerError> {
        if indices.is_empty() || indices.len() != array.bounds.len() {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        let mut dimensions = indices
            .iter()
            .enumerate()
            .zip(array.bounds.iter())
            .collect::<Vec<_>>();
        if self.array_order == hir::ArrayOrder::ColumnMajor {
            dimensions.reverse();
        }

        let mut offset = None;
        let mut offset_type = None;
        for ((index, operand), &(lower, upper)) in dimensions {
            let index_type =
                self.operand_type(function, block, Some(instruction.id), index, operand)?;
            if self.type_by_id(index_type)?.kind != hir::TypeKind::Integer || lower > upper {
                return self.invalid_instruction(
                    function,
                    block,
                    instruction,
                    InvalidProperty::OperandTypes,
                );
            }
            if let Some(previous_type) = offset_type {
                if previous_type != index_type {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::OperandTypes,
                    );
                }
            }
            let type_id = type_id(index_type);
            let adjusted = self.generated_binary(
                function,
                generated,
                prefixes,
                ir::BinaryOp::Subtract,
                self.lower_operand(function, block, Some(instruction.id), index, operand)?,
                ir::Operand::Constant(ir::TypedConstant {
                    type_id,
                    value: ir::Constant::Integer(i128::from(lower)),
                }),
                type_id,
            )?;
            offset = Some(if let Some(previous) = offset {
                let count = upper
                    .checked_sub(lower)
                    .and_then(|range| range.checked_add(1))
                    .ok_or(LowerError::InvalidInstruction {
                        function: function.id,
                        block,
                        instruction: instruction.id,
                        property: InvalidProperty::OperandTypes,
                    })?;
                let scaled = self.generated_binary(
                    function,
                    generated,
                    prefixes,
                    ir::BinaryOp::Multiply,
                    previous,
                    ir::Operand::Constant(ir::TypedConstant {
                        type_id,
                        value: ir::Constant::Integer(i128::from(count)),
                    }),
                    type_id,
                )?;
                self.generated_binary(
                    function,
                    generated,
                    prefixes,
                    ir::BinaryOp::Add,
                    scaled,
                    adjusted,
                    type_id,
                )?
            } else {
                adjusted
            });
            offset_type = Some(index_type);
        }
        let type_id = type_id(offset_type.expect("nonempty indices have an offset type"));
        let mut offset = self.generated_binary(
            function,
            generated,
            prefixes,
            ir::BinaryOp::Multiply,
            offset.expect("nonempty indices have an offset"),
            ir::Operand::Constant(ir::TypedConstant {
                type_id,
                value: ir::Constant::Integer(element.width as i128),
            }),
            type_id,
        )?;
        if field_offset != 0 {
            offset = self.generated_binary(
                function,
                generated,
                prefixes,
                ir::BinaryOp::Add,
                offset,
                ir::Operand::Constant(ir::TypedConstant {
                    type_id,
                    value: ir::Constant::Integer(field_offset as i128),
                }),
                type_id,
            )?;
        }
        Ok(offset)
    }

    fn generated_binary(
        &self,
        function: &hir::Function,
        generated: &mut GeneratedIds,
        prefixes: &mut Vec<ir::Instruction>,
        op: ir::BinaryOp,
        left: ir::Operand,
        right: ir::Operand,
        type_id: ir::TypeId,
    ) -> Result<ir::Operand, LowerError> {
        let result = ir::Value {
            id: generated.value(function.id)?,
            type_id,
        };
        prefixes.push(ir::Instruction {
            id: generated.instruction(function.id)?,
            results: vec![result.clone()],
            kind: ir::InstructionKind::Binary { op, left, right },
        });
        Ok(ir::Operand::Value(result.id))
    }

    fn offset_index_type(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        address: hir::AddressKind,
    ) -> Result<&hir::Type, LowerError> {
        let width = match address {
            hir::AddressKind::Near | hir::AddressKind::Far => 2,
            hir::AddressKind::Huge => 4,
            hir::AddressKind::None | hir::AddressKind::Code | hir::AddressKind::Segment => {
                return self.invalid_instruction(
                    function,
                    block,
                    instruction,
                    InvalidProperty::UnsupportedIndirectOffsetAddress,
                );
            }
        };
        self.module
            .types
            .iter()
            .find(|type_| type_.kind == hir::TypeKind::Integer && type_.width == width)
            .ok_or(LowerError::InvalidInstruction {
                function: function.id,
                block,
                instruction: instruction.id,
                property: InvalidProperty::MissingIndirectOffsetType,
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
            Opcode::FloatToInteger { rounding } => {
                self.float_to_integer(function, block, instruction, rounding)?
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
            Opcode::Load => self.lower_load(function, block, instruction)?,
            Opcode::Store => self.lower_store(function, block, instruction)?,
            Opcode::Address => self.lower_address(function, block, instruction)?,
            Opcode::OffsetPointer => self.lower_offset_pointer(function, block, instruction)?,
            Opcode::PointerOffset | Opcode::PointerSegment => {
                return self.unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::PointerExtraction,
                );
            }
            Opcode::Concat => self.compose_pointer(function, block, instruction)?,
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
            Opcode::Call => self.lower_call(function, block, instruction)?,
        };
        let results = match instruction.opcode {
            Opcode::Call => instruction
                .results
                .iter()
                .map(|result| self.lower_value(function, *result, block, Some(instruction.id)))
                .collect::<Result<Vec<_>, _>>()?,
            Opcode::Store => {
                if !instruction.results.is_empty() {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::ResultArity,
                    );
                }
                Vec::new()
            }
            _ => vec![self.one_result(function, block, instruction)?],
        };
        Ok(ir::Instruction {
            id: instruction_id(instruction.id),
            results,
            kind,
        })
    }

    fn lower_load(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        let result = self.one_result(function, block, instruction)?;
        let memory = self.memory_address(function, block, instruction, 0, 1)?;
        self.require_same_type(
            function,
            block,
            instruction,
            result.type_id,
            memory.type_id,
            InvalidProperty::OperandTypes,
        )?;
        Ok(ir::InstructionKind::Load {
            address: memory.address,
            alignment: 1,
            volatile: memory.volatile,
        })
    }

    fn lower_store(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        let memory = self.memory_address(function, block, instruction, 0, 2)?;
        if memory.readonly {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::ReadOnlyStore,
            );
        }
        let value_type = self.operand_type(
            function,
            block,
            Some(instruction.id),
            1,
            &instruction.operands[1],
        )?;
        if value_type != memory.type_id {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        let value = self.lower_operand(
            function,
            block,
            Some(instruction.id),
            1,
            &instruction.operands[1],
        )?;
        Ok(ir::InstructionKind::Store {
            address: memory.address,
            value,
            alignment: 1,
            volatile: memory.volatile,
        })
    }

    fn memory_address(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        index: usize,
        arity: usize,
    ) -> Result<MemoryAddress, LowerError> {
        if instruction.operands.len() != arity {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        }
        match &instruction.operands[index] {
            hir::Operand::Place(_) => {
                let (place, address, pointer_type, readonly) =
                    self.direct_place(function, block, instruction, index, arity)?;
                Ok(MemoryAddress {
                    type_id: place.type_id,
                    address,
                    pointer_type,
                    address_kind: place.address,
                    readonly,
                    volatile: false,
                })
            }
            hir::Operand::Element { place, .. } => {
                let (place, address, pointer_type, readonly) =
                    self.place_address(function, block, instruction, index, *place)?;
                let root = self.type_by_id(place.type_id)?;
                let Some(element) = (root.kind == hir::TypeKind::Array)
                    .then_some(root.element)
                    .flatten()
                else {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::OperandTypes,
                    );
                };
                self.type_by_id(element)?;
                Ok(MemoryAddress {
                    type_id: element,
                    address,
                    pointer_type,
                    address_kind: place.address,
                    readonly,
                    volatile: false,
                })
            }
            hir::Operand::Projection { place, type_id, .. } => {
                self.type_by_id(*type_id)?;
                let (place, address, pointer_type, readonly) =
                    self.place_address(function, block, instruction, index, *place)?;
                Ok(MemoryAddress {
                    type_id: *type_id,
                    address,
                    pointer_type,
                    address_kind: place.address,
                    readonly,
                    volatile: false,
                })
            }
            hir::Operand::Indirect {
                base,
                offset: _,
                type_id,
                volatile,
            } => {
                let base = self.value_by_id(function, *base, block, Some(instruction.id))?;
                let base_type = self.type_by_id(base.type_id)?;
                if base_type.kind != hir::TypeKind::Pointer {
                    return self.invalid_instruction(
                        function,
                        block,
                        instruction,
                        InvalidProperty::OperandTypes,
                    );
                }
                self.type_by_id(*type_id)?;
                Ok(MemoryAddress {
                    type_id: *type_id,
                    address: ir::Operand::Value(value_id(base.id)),
                    pointer_type: self.evaluation_type(base.type_id)?,
                    address_kind: base_type.address,
                    readonly: false,
                    volatile: *volatile,
                })
            }
            _ => self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            ),
        }
    }

    fn lower_address(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        let result = self.one_result(function, block, instruction)?;
        let (place, address, _, _) = self.direct_place(function, block, instruction, 0, 1)?;
        let result_type = self.type_by_id(hir::TypeId::new(result.type_id.get()))?;
        if result_type.kind != hir::TypeKind::Pointer
            || lower_address_kind(result_type.address) != lower_address_kind(place.address)
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::Cast {
            op: ir::CastOp::Bitcast,
            operand: address,
            to: result.type_id,
        })
    }

    fn lower_offset_pointer(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        if instruction.operands.len() != 2 {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        }
        let result = self.one_result(function, block, instruction)?;
        let result_type = self.type_by_id(hir::TypeId::new(result.type_id.get()))?;
        let base_type = self.operand_type(
            function,
            block,
            Some(instruction.id),
            0,
            &instruction.operands[0],
        )?;
        let base_type = self.type_by_id(base_type)?;
        let offset_type = self.operand_type(
            function,
            block,
            Some(instruction.id),
            1,
            &instruction.operands[1],
        )?;
        let offset_type = self.type_by_id(offset_type)?;
        if result_type.kind != hir::TypeKind::Pointer
            || base_type.kind != hir::TypeKind::Pointer
            || !matches!(
                offset_type.kind,
                hir::TypeKind::Boolean | hir::TypeKind::Integer
            )
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::GetElementPointer {
            base: self.lower_operand(
                function,
                block,
                Some(instruction.id),
                0,
                &instruction.operands[0],
            )?,
            // OffsetPointer is deliberately byte-addressed in HIR; any
            // element scaling is already explicit in the offset operand.
            indices: vec![self.lower_operand(
                function,
                block,
                Some(instruction.id),
                1,
                &instruction.operands[1],
            )?],
        })
    }

    fn compose_pointer(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        if instruction.operands.len() != 2 {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        }
        let result = self.one_result(function, block, instruction)?;
        let result_type = self.type_by_id(hir::TypeId::new(result.type_id.get()))?;
        let segment_type = self.operand_type(
            function,
            block,
            Some(instruction.id),
            0,
            &instruction.operands[0],
        )?;
        let segment_type = self.type_by_id(segment_type)?;
        let offset_type = self.operand_type(
            function,
            block,
            Some(instruction.id),
            1,
            &instruction.operands[1],
        )?;
        let offset_type = self.type_by_id(offset_type)?;
        if result_type.kind != hir::TypeKind::Pointer
            || result_type.width != 4
            || segment_type.kind != hir::TypeKind::Integer
            || segment_type.width != 2
            || offset_type.kind != hir::TypeKind::Integer
            || offset_type.width != 2
        {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        }
        Ok(ir::InstructionKind::ComposePointer {
            segment: self.lower_operand(
                function,
                block,
                Some(instruction.id),
                0,
                &instruction.operands[0],
            )?,
            offset: self.lower_operand(
                function,
                block,
                Some(instruction.id),
                1,
                &instruction.operands[1],
            )?,
        })
    }

    fn direct_place<'function>(
        &self,
        function: &'function hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        index: usize,
        arity: usize,
    ) -> Result<(&'function hir::Place, ir::Operand, ir::TypeId, bool), LowerError> {
        if instruction.operands.len() != arity {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandArity,
            );
        }
        let Some(hir::Operand::Place(place_id)) = instruction.operands.get(index) else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::OperandTypes,
            );
        };
        self.place_address(function, block, instruction, index, *place_id)
    }

    fn place_address<'function>(
        &self,
        function: &'function hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        _index: usize,
        place_id: hir::PlaceId,
    ) -> Result<(&'function hir::Place, ir::Operand, ir::TypeId, bool), LowerError> {
        let mut matches = function.places.iter().filter(|place| place.id == place_id);
        let Some(place) = matches.next() else {
            return Err(LowerError::UnknownPlace {
                function: function.id,
                block,
                instruction: instruction.id,
                place: place_id,
            });
        };
        if matches.next().is_some() {
            return Err(LowerError::AmbiguousPlace {
                function: function.id,
                block,
                instruction: instruction.id,
                place: place_id,
            });
        }
        if let Some(planned) = self.globals.places.get(&(function.id, place_id)).copied() {
            return Ok((
                place,
                global_address(planned, planned.pointer_type),
                planned.pointer_type,
                planned.readonly,
            ));
        }
        if let Some(value) = self.stack.places.get(&(function.id, place_id)) {
            return Ok((place, ir::Operand::Value(value.id), value.type_id, false));
        }
        Err(LowerError::InvalidInstruction {
            function: function.id,
            block,
            instruction: instruction.id,
            property: InvalidProperty::MissingGlobalPlan,
        })
    }

    fn lower_call(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
    ) -> Result<ir::InstructionKind, LowerError> {
        let planned = self.calls.sites.get(&(function.id, instruction.id)).ok_or(
            LowerError::InvalidInstruction {
                function: function.id,
                block,
                instruction: instruction.id,
                property: InvalidProperty::MissingCallPlan,
            },
        )?;
        let arguments = planned
            .argument_indices
            .iter()
            .map(|index| {
                let operand =
                    instruction
                        .operands
                        .get(*index)
                        .ok_or(LowerError::InvalidInstruction {
                            function: function.id,
                            block,
                            instruction: instruction.id,
                            property: InvalidProperty::MissingCallPlan,
                        })?;
                self.lower_operand(function, block, Some(instruction.id), *index, operand)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ir::InstructionKind::Call {
            callee: ir::Callee::Direct(planned.target),
            arguments,
            effects: ir::Effects {
                memory: ir::MemoryEffects::Unknown,
                may_trap: true,
                observable: true,
            },
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
            || self.evaluation_type(left_type)? != result.type_id
            || self.evaluation_type(right_type)? != result.type_id
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
            || self.evaluation_type(source_type)? != result.type_id
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
            || self.evaluation_type(left_type)? != result.type_id
            || self.evaluation_type(right_type)? != result.type_id
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
            || self.evaluation_type(source_type)? != result.type_id
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
            || self.evaluation_type(source_type)? != result.type_id
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

    fn float_to_integer(
        &self,
        function: &hir::Function,
        block: hir::BlockId,
        instruction: &hir::Instruction,
        rounding: hir::FloatRounding,
    ) -> Result<ir::InstructionKind, LowerError> {
        let (result, operand, source_type) = self.unary_parts(function, block, instruction)?;
        if !self.is_float(source_type)? || !self.is_integer_ir(result.type_id)? {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::CastTypes,
            );
        }
        Ok(ir::InstructionKind::Cast {
            op: ir::CastOp::FloatToInteger {
                rounding: match rounding {
                    hir::FloatRounding::Dynamic => ir::FloatRounding::Dynamic,
                    hir::FloatRounding::TowardZero => ir::FloatRounding::TowardZero,
                    hir::FloatRounding::NearestEven => ir::FloatRounding::NearestEven,
                },
            },
            operand,
            to: result.type_id,
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
        let [result] = instruction.results.as_slice() else {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::ResultArity,
            );
        };
        let target_hir = self
            .value_by_id(function, *result, block, Some(instruction.id))?
            .type_id;
        if target != self.evaluation_type(target_hir)? {
            return self.invalid_instruction(
                function,
                block,
                instruction,
                InvalidProperty::CastTypes,
            );
        }
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
                if source_type.signed == Some(true) =>
            {
                Ok(ir::CastOp::IntegerToFloat)
            }
            (hir::TypeKind::Boolean | hir::TypeKind::Integer, hir::TypeKind::Float) => self
                .unsupported_instruction(
                    function,
                    block,
                    instruction,
                    UnsupportedFeature::UnsupportedCast,
                ),
            (hir::TypeKind::Float, hir::TypeKind::Boolean | hir::TypeKind::Integer) => self
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
            type_id: self.evaluation_type(value.type_id)?,
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
        if self
            .float_types
            .declarations()
            .iter()
            .any(|type_| type_.id == id)
        {
            return Ok(true);
        }
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
        if result == self.evaluation_type(source)? {
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

fn global_address(place: PlannedPlace, pointer_type: ir::TypeId) -> ir::Operand {
    ir::Operand::Constant(ir::TypedConstant {
        type_id: pointer_type,
        value: ir::Constant::GlobalAddress {
            global: place.global,
            addend: place.addend,
        },
    })
}

fn lower_address_kind(address: hir::AddressKind) -> ir::AddressSpace {
    match address {
        hir::AddressKind::None => ir::AddressSpace::Generic,
        hir::AddressKind::Near => ir::AddressSpace::NearData,
        hir::AddressKind::Far => ir::AddressSpace::FarData,
        hir::AddressKind::Huge => ir::AddressSpace::HugeData,
        hir::AddressKind::Code => ir::AddressSpace::Code,
        hir::AddressKind::Segment => ir::AddressSpace::Segment,
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
            ir::CallingConvention::FarPascal
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
    fn lowers_an_incoming_parameter_cell_without_copying_it_to_a_local() {
        // Port of cfront.raise_hir._Raise.name/points: the formal remains an
        // ABI value for the signature, while source reads address its actual
        // incoming frame cell at the use site.
        let mut module = scalar_module();
        let function = &mut module.functions[0];
        function.places = vec![hir::Place {
            id: hir::PlaceId::new(0),
            name: "value".into(),
            type_id: hir::TypeId::new(2),
            storage: hir::Storage::Parameter { index: 0 },
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        }];
        function.blocks = vec![hir::Block {
            id: hir::BlockId::new(4),
            instructions: vec![hir::Instruction {
                id: hir::InstructionId::new(10),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(1)],
                operands: vec![hir::Operand::Place(hir::PlaceId::new(0))],
                callee: None,
            }],
            terminator: hir::Terminator::Return(None),
        }];

        let lowered = lower_module(&module).expect("parameter cell lowers");
        let instructions = &lowered.functions[0].blocks[0].instructions;
        assert!(matches!(
            instructions.as_slice(),
            [
                ir::Instruction {
                    kind: ir::InstructionKind::ParameterAddress { parameter: 0 },
                    ..
                },
                ir::Instruction {
                    kind: ir::InstructionKind::Load { .. },
                    ..
                }
            ]
        ));
        assert!(
            !instructions.iter().any(|instruction| matches!(
                instruction.kind,
                ir::InstructionKind::StackAlloc { .. }
            ))
        );
        assert_eq!(lowered.functions[0].parameters[0].id, ir::ValueId::new(0));
    }

    #[test]
    fn refuses_a_parameter_cell_with_no_corresponding_formal() {
        let mut module = scalar_module();
        module.functions[0].places.push(hir::Place {
            id: hir::PlaceId::new(0),
            name: "missing".into(),
            type_id: hir::TypeId::new(2),
            storage: hir::Storage::Parameter { index: 1 },
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        });

        assert!(matches!(
            lower_module(&module),
            Err(LowerError::InvalidPlace {
                property: InvalidProperty::ParameterIndex,
                ..
            })
        ));
    }

    #[test]
    fn lowers_pointer_concat_in_segment_then_offset_order() {
        let mut module = scalar_module();
        module.types.extend([
            hir::Type {
                id: hir::TypeId::new(6),
                name: "word".into(),
                kind: hir::TypeKind::Integer,
                width: 2,
                signed: Some(false),
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(7),
                name: "far-long".into(),
                kind: hir::TypeKind::Pointer,
                width: 4,
                signed: None,
                evaluation: hir::FloatEvaluation::None,
                element: Some(hir::TypeId::new(2)),
                bounds: Vec::new(),
                address: hir::AddressKind::Far,
            },
        ]);
        module.functions[0].values.extend([
            hir::Value {
                id: hir::ValueId::new(5),
                type_id: hir::TypeId::new(6),
            },
            hir::Value {
                id: hir::ValueId::new(6),
                type_id: hir::TypeId::new(6),
            },
            hir::Value {
                id: hir::ValueId::new(7),
                type_id: hir::TypeId::new(7),
            },
        ]);
        module.functions[0].blocks[0].instructions.extend([
            hir::Instruction {
                id: hir::InstructionId::new(14),
                opcode: hir::Opcode::Copy,
                results: vec![hir::ValueId::new(5)],
                operands: vec![hir::Operand::Constant {
                    type_id: hir::TypeId::new(6),
                    value: hir::ConstantValue::Integer(0x1234),
                }],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(15),
                opcode: hir::Opcode::Copy,
                results: vec![hir::ValueId::new(6)],
                operands: vec![hir::Operand::Constant {
                    type_id: hir::TypeId::new(6),
                    value: hir::ConstantValue::Integer(0x5678),
                }],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(16),
                opcode: hir::Opcode::Concat,
                results: vec![hir::ValueId::new(7)],
                operands: vec![
                    hir::Operand::Value(hir::ValueId::new(5)),
                    hir::Operand::Value(hir::ValueId::new(6)),
                ],
                callee: None,
            },
        ]);

        let lowered = lower_module(&module).expect("pointer concat lowers");
        let instruction = &lowered.functions[0].blocks[0].instructions[4];
        assert_eq!(instruction.id, ir::InstructionId::new(16));
        assert_eq!(
            instruction.results,
            vec![ir::Value {
                id: ir::ValueId::new(7),
                type_id: ir::TypeId::new(7),
            }]
        );
        assert!(matches!(
            &instruction.kind,
            ir::InstructionKind::ComposePointer {
                segment: ir::Operand::Value(segment),
                offset: ir::Operand::Value(offset),
            } if *segment == ir::ValueId::new(5) && *offset == ir::ValueId::new(6)
        ));
        assert_eq!(
            lowered
                .types
                .iter()
                .find(|type_| type_.id == instruction.results[0].type_id)
                .expect("pointer result type is retained")
                .kind,
            ir::TypeKind::Pointer {
                address_space: ir::AddressSpace::FarData,
            }
        );
        assert!(lowered.verify().is_ok());
    }

    #[test]
    fn lowers_generic_procedure_abi_pairs_to_calling_conventions() {
        let mut near_caller = scalar_module();
        near_caller.functions[0].abi.distance = hir::CallDistance::Near;
        near_caller.functions[0].abi.cleanup = hir::StackCleanup::Caller;
        let near = lower_module(&near_caller).expect("near caller-cleanup function lowers");
        assert_eq!(
            near.functions[0].signature.calling_convention,
            ir::CallingConvention::C
        );

        let mut far_caller = scalar_module();
        far_caller.functions[0].abi.cleanup = hir::StackCleanup::Caller;
        let far = lower_module(&far_caller).expect("far caller-cleanup function lowers");
        assert_eq!(
            far.functions[0].signature.calling_convention,
            ir::CallingConvention::FarCdecl
        );
    }

    #[test]
    fn rejects_near_callee_cleanup_procedure_abi() {
        let mut module = scalar_module();
        module.functions[0].abi.distance = hir::CallDistance::Near;

        assert!(matches!(
            lower_module(&module),
            Err(LowerError::UnsupportedFunctionAbi {
                distance: hir::CallDistance::Near,
                cleanup: hir::StackCleanup::Callee,
                ..
            })
        ));
    }

    #[test]
    fn lowers_runtime_calls_with_abi_argument_order() {
        let mut module = scalar_module();
        module.functions[0].blocks[0]
            .instructions
            .push(hir::Instruction {
                id: hir::InstructionId::new(14),
                opcode: hir::Opcode::Call,
                results: Vec::new(),
                operands: vec![hir::Operand::Value(hir::ValueId::new(1))],
                callee: Some("B$WRITE".into()),
            });
        module.functions[0].calls.push(hir::CallAbi {
            instruction: hir::InstructionId::new(14),
            order: vec![0],
            cleanup: hir::StackCleanup::Callee,
            distance: hir::CallDistance::Far,
            callee: None,
        });

        let lowered = lower_module(&module).expect("runtime call shape lowers exactly");

        let declaration = &lowered.functions[1];
        assert_eq!(declaration.id, ir::FunctionId::new(12));
        assert_eq!(declaration.name, "B$WRITE");
        assert_eq!(declaration.linkage, ir::Linkage::External);
        assert_eq!(declaration.signature.parameters, vec![ir::TypeId::new(2)]);
        assert_eq!(declaration.parameters[0].type_id, ir::TypeId::new(2));
        let call = &lowered.functions[0].blocks[0].instructions[2];
        assert!(matches!(
            &call.kind,
            ir::InstructionKind::Call {
                callee: ir::Callee::Direct(target),
                arguments,
                effects: ir::Effects {
                    memory: ir::MemoryEffects::Unknown,
                    may_trap: true,
                    observable: true,
                },
            } if *target == declaration.id
                && arguments == &vec![ir::Operand::Value(ir::ValueId::new(1))]
        ));
        assert!(lowered.verify().is_ok());
    }

    #[test]
    fn lowers_local_places_through_distinct_stack_storage() {
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
        module.functions[0].values.push(hir::Value {
            id: hir::ValueId::new(5),
            type_id: hir::TypeId::new(2),
        });
        module.functions[0].blocks[0].instructions.extend([
            hir::Instruction {
                id: hir::InstructionId::new(14),
                opcode: hir::Opcode::Store,
                results: Vec::new(),
                operands: vec![
                    hir::Operand::Place(hir::PlaceId::new(0)),
                    hir::Operand::Constant {
                        type_id: hir::TypeId::new(2),
                        value: hir::ConstantValue::Integer(7),
                    },
                ],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(15),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(5)],
                operands: vec![hir::Operand::Place(hir::PlaceId::new(0))],
                callee: None,
            },
        ]);

        let lowered = lower_module(&module).expect("local storage lowers through alloca");
        let instructions = &lowered.functions[0].blocks[0].instructions;
        assert!(matches!(
            &instructions[0],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::StackAlloc {
                    size: 4,
                    alignment: 1,
                    address_space: ir::AddressSpace::NearData,
                },
            } if *id == ir::InstructionId::new(16)
                && results == &vec![ir::Value {
                    id: ir::ValueId::new(6),
                    type_id: ir::TypeId::new(4),
                }]
        ));
        assert!(matches!(
            &instructions[3].kind,
            ir::InstructionKind::Store {
                address: ir::Operand::Value(value),
                ..
            } if *value == ir::ValueId::new(6)
        ));
        assert!(matches!(
            &instructions[4].kind,
            ir::InstructionKind::Load {
                address: ir::Operand::Value(value),
                ..
            } if *value == ir::ValueId::new(6)
        ));
        assert!(lowered.verify().is_ok());
    }

    #[test]
    fn lowers_an_indirect_byte_offset_through_a_typed_gep_prefix() {
        let mut module = scalar_module();
        module.types.push(hir::Type {
            id: hir::TypeId::new(6),
            name: "offset".into(),
            kind: hir::TypeKind::Integer,
            width: 2,
            signed: Some(true),
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        });
        module.types.push(hir::Type {
            id: hir::TypeId::new(7),
            name: "near-single".into(),
            kind: hir::TypeKind::Pointer,
            width: 2,
            signed: None,
            evaluation: hir::FloatEvaluation::None,
            element: Some(hir::TypeId::new(5)),
            bounds: Vec::new(),
            address: hir::AddressKind::Near,
        });
        module.functions[0].places.push(hir::Place {
            id: hir::PlaceId::new(0),
            name: "stack".into(),
            type_id: hir::TypeId::new(2),
            storage: hir::Storage::Local,
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        });
        module.functions[0].values.extend([
            hir::Value {
                id: hir::ValueId::new(5),
                type_id: hir::TypeId::new(7),
            },
            hir::Value {
                id: hir::ValueId::new(6),
                type_id: hir::TypeId::new(2),
            },
        ]);
        module.functions[0].blocks[0].instructions.extend([
            hir::Instruction {
                id: hir::InstructionId::new(14),
                opcode: hir::Opcode::Address,
                results: vec![hir::ValueId::new(5)],
                operands: vec![hir::Operand::Place(hir::PlaceId::new(0))],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(15),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(6)],
                operands: vec![hir::Operand::Indirect {
                    base: hir::ValueId::new(5),
                    offset: 1,
                    type_id: hir::TypeId::new(2),
                    volatile: true,
                }],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(18),
                opcode: hir::Opcode::Store,
                results: Vec::new(),
                operands: vec![
                    hir::Operand::Indirect {
                        base: hir::ValueId::new(5),
                        offset: 2,
                        type_id: hir::TypeId::new(2),
                        volatile: true,
                    },
                    hir::Operand::Value(hir::ValueId::new(0)),
                ],
                callee: None,
            },
        ]);

        let lowered = lower_module(&module).expect("indirect byte offset lowers through GEP");
        let instructions = &lowered.functions[0].blocks[0].instructions;
        assert!(matches!(
            &instructions[0],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::StackAlloc { .. },
            } if *id == ir::InstructionId::new(19)
                && results == &vec![ir::Value {
                    id: ir::ValueId::new(7),
                    type_id: ir::TypeId::new(4),
                }]
        ));
        assert!(matches!(
            &instructions[4],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::GetElementPointer { base, indices },
            } if *id == ir::InstructionId::new(20)
                && results == &vec![ir::Value {
                    id: ir::ValueId::new(8),
                    type_id: ir::TypeId::new(4),
                }]
                && base == &ir::Operand::Value(ir::ValueId::new(5))
                && indices == &vec![ir::Operand::Constant(ir::TypedConstant {
                    type_id: ir::TypeId::new(6),
                    value: ir::Constant::Integer(1),
                })]
        ));
        assert!(matches!(
            &instructions[5],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::Load {
                    address,
                    volatile: true,
                    ..
                },
            } if *id == ir::InstructionId::new(15)
                && results == &vec![ir::Value {
                    id: ir::ValueId::new(6),
                    type_id: ir::TypeId::new(2),
                }]
                && address == &ir::Operand::Value(ir::ValueId::new(8))
        ));
        assert!(matches!(
            &instructions[6],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::GetElementPointer { base, indices },
            } if *id == ir::InstructionId::new(21)
                && results == &vec![ir::Value {
                    id: ir::ValueId::new(9),
                    type_id: ir::TypeId::new(4),
                }]
                && base == &ir::Operand::Value(ir::ValueId::new(5))
                && indices == &vec![ir::Operand::Constant(ir::TypedConstant {
                    type_id: ir::TypeId::new(6),
                    value: ir::Constant::Integer(2),
                })]
        ));
        assert!(matches!(
            &instructions[7],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::Store {
                    address,
                    volatile: true,
                    ..
                },
            } if *id == ir::InstructionId::new(18)
                && results.is_empty()
                && address == &ir::Operand::Value(ir::ValueId::new(9))
        ));
        assert!(lowered.verify().is_ok());
    }

    #[test]
    fn refuses_an_indirect_byte_offset_without_a_matching_integer_type() {
        let mut module = scalar_module();
        module.functions[0].values.extend([
            hir::Value {
                id: hir::ValueId::new(5),
                type_id: hir::TypeId::new(4),
            },
            hir::Value {
                id: hir::ValueId::new(6),
                type_id: hir::TypeId::new(2),
            },
        ]);
        module.functions[0].blocks[0]
            .instructions
            .push(hir::Instruction {
                id: hir::InstructionId::new(14),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(6)],
                operands: vec![hir::Operand::Indirect {
                    base: hir::ValueId::new(5),
                    offset: 1,
                    type_id: hir::TypeId::new(2),
                    volatile: false,
                }],
                callee: None,
            });

        assert!(matches!(
            lower_module(&module),
            Err(LowerError::InvalidInstruction {
                instruction,
                property: InvalidProperty::MissingIndirectOffsetType,
                ..
            }) if instruction == hir::InstructionId::new(14)
        ));
    }

    #[test]
    fn lowers_a_far_indirect_byte_offset_with_a_16_bit_index() {
        let mut module = scalar_module();
        module.types.extend([
            hir::Type {
                id: hir::TypeId::new(6),
                name: "offset".into(),
                kind: hir::TypeKind::Integer,
                width: 2,
                signed: Some(true),
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(7),
                name: "far-long".into(),
                kind: hir::TypeKind::Pointer,
                width: 4,
                signed: None,
                evaluation: hir::FloatEvaluation::None,
                element: Some(hir::TypeId::new(2)),
                bounds: Vec::new(),
                address: hir::AddressKind::Far,
            },
        ]);
        module.data.push(hir::DataObject {
            id: hir::DataId::new(77),
            name: "far-slot".into(),
            bytes: vec![0; 4],
            readonly: false,
            relocations: Vec::new(),
            linkage: hir::Linkage::Internal,
            address: hir::AddressKind::Far,
        });
        module.functions[0].places.push(hir::Place {
            id: hir::PlaceId::new(0),
            name: "far-slot".into(),
            type_id: hir::TypeId::new(2),
            storage: hir::Storage::Module,
            offset: 0,
            symbol: hir::DataId::new(77),
            extent: 4,
            address: hir::AddressKind::Far,
        });
        module.functions[0].values.extend([
            hir::Value {
                id: hir::ValueId::new(5),
                type_id: hir::TypeId::new(7),
            },
            hir::Value {
                id: hir::ValueId::new(6),
                type_id: hir::TypeId::new(2),
            },
        ]);
        module.functions[0].blocks[0].instructions.extend([
            hir::Instruction {
                id: hir::InstructionId::new(14),
                opcode: hir::Opcode::Address,
                results: vec![hir::ValueId::new(5)],
                operands: vec![hir::Operand::Place(hir::PlaceId::new(0))],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(15),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(6)],
                operands: vec![hir::Operand::Indirect {
                    base: hir::ValueId::new(5),
                    offset: 2,
                    type_id: hir::TypeId::new(2),
                    volatile: false,
                }],
                callee: None,
            },
        ]);

        let lowered = lower_module(&module).expect("far indirect byte offset lowers through GEP");
        let instructions = &lowered.functions[0].blocks[0].instructions;
        assert!(matches!(
            lowered
                .types
                .iter()
                .find(|type_| type_.id == ir::TypeId::new(7)),
            Some(ir::Type {
                kind: ir::TypeKind::Pointer {
                    address_space: ir::AddressSpace::FarData,
                },
                ..
            })
        ));
        assert!(matches!(
            &instructions[3],
            ir::Instruction {
                id,
                results,
                kind: ir::InstructionKind::GetElementPointer { indices, .. },
            } if *id == ir::InstructionId::new(16)
                && results == &vec![ir::Value {
                    id: ir::ValueId::new(7),
                    type_id: ir::TypeId::new(7),
                }]
                && indices == &vec![ir::Operand::Constant(ir::TypedConstant {
                    type_id: ir::TypeId::new(6),
                    value: ir::Constant::Integer(2),
                })]
        ));
        assert!(matches!(
            &instructions[4],
            ir::Instruction {
                id,
                kind: ir::InstructionKind::Load { address, .. },
                ..
            } if *id == ir::InstructionId::new(15)
                && address == &ir::Operand::Value(ir::ValueId::new(7))
        ));
        assert!(lowered.verify().is_ok());
    }

    #[test]
    fn lowers_static_place_load_store_and_address() {
        let mut module = scalar_module();
        module.data.push(hir::DataObject {
            id: hir::DataId::new(77),
            name: "slot-data".into(),
            bytes: vec![0; 4],
            readonly: false,
            relocations: Vec::new(),
            linkage: hir::Linkage::Internal,
            address: hir::AddressKind::Near,
        });
        module.functions[0].places.push(hir::Place {
            id: hir::PlaceId::new(0),
            name: "slot".into(),
            type_id: hir::TypeId::new(2),
            storage: hir::Storage::Module,
            offset: 0,
            symbol: hir::DataId::new(77),
            extent: 4,
            address: hir::AddressKind::Near,
        });
        module.functions[0].values.extend([
            hir::Value {
                id: hir::ValueId::new(5),
                type_id: hir::TypeId::new(2),
            },
            hir::Value {
                id: hir::ValueId::new(6),
                type_id: hir::TypeId::new(4),
            },
        ]);
        module.functions[0].blocks[0].instructions.extend([
            hir::Instruction {
                id: hir::InstructionId::new(20),
                opcode: hir::Opcode::Store,
                results: Vec::new(),
                operands: vec![
                    hir::Operand::Place(hir::PlaceId::new(0)),
                    hir::Operand::Constant {
                        type_id: hir::TypeId::new(2),
                        value: hir::ConstantValue::Integer(9),
                    },
                ],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(21),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(5)],
                operands: vec![hir::Operand::Place(hir::PlaceId::new(0))],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(22),
                opcode: hir::Opcode::Address,
                results: vec![hir::ValueId::new(6)],
                operands: vec![hir::Operand::Place(hir::PlaceId::new(0))],
                callee: None,
            },
        ]);

        let lowered = lower_module(&module).expect("direct static memory lowers exactly");

        assert_eq!(lowered.globals.len(), 1);
        assert_eq!(lowered.globals[0].id, ir::GlobalId::new(77));
        let instructions = &lowered.functions[0].blocks[0].instructions;
        assert!(matches!(
            instructions[2].kind,
            ir::InstructionKind::Store { .. }
        ));
        assert!(matches!(
            instructions[3].kind,
            ir::InstructionKind::Load { .. }
        ));
        assert!(matches!(
            instructions[4].kind,
            ir::InstructionKind::Cast {
                op: ir::CastOp::Bitcast,
                ..
            }
        ));
        assert!(lowered.verify().is_ok());

        module.data[0].readonly = true;
        assert!(matches!(
            lower_module(&module),
            Err(LowerError::InvalidInstruction {
                property: InvalidProperty::ReadOnlyStore,
                ..
            })
        ));
    }

    #[test]
    fn lowers_integer_to_float_as_the_portable_conversion() {
        let mut module = scalar_module();
        module.functions[0].blocks[2].instructions[0].operands =
            vec![hir::Operand::Value(hir::ValueId::new(1))];

        let lowered = lower_module(&module).expect("integer-to-float conversion lowers");
        assert!(matches!(
            lowered.functions[0].blocks[2].instructions[0].kind,
            ir::InstructionKind::Cast {
                op: ir::CastOp::IntegerToFloat,
                ..
            }
        ));
        lowered.verify().expect("lowered conversion verifies");
    }

    #[test]
    fn refuses_unsigned_integer_to_float_without_an_unsigned_cast_operation() {
        let mut module = scalar_module();
        module.types[2].signed = Some(false);
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

    fn split_single_module() -> hir::Module {
        let mut module = scalar_module();
        module.types[5].evaluation = hir::FloatEvaluation::Extended80;
        let function = &mut module.functions[0];
        function.values = vec![hir::Value {
            id: hir::ValueId::new(5),
            type_id: hir::TypeId::new(5),
        }];
        function.parameters = vec![hir::ValueId::new(5)];
        function.abi.parameter_bytes = 4;
        function.blocks = vec![hir::Block {
            id: hir::BlockId::new(4),
            instructions: Vec::new(),
            terminator: hir::Terminator::Return(None),
        }];
        function.entry = hir::BlockId::new(4);
        module
    }

    #[test]
    fn split_single_parameter_keeps_four_byte_signature_and_extends_in_entry() {
        let lowered = lower_module(&split_single_module()).expect("split single parameter lowers");
        let function = &lowered.functions[0];
        assert_eq!(function.signature.parameters, vec![ir::TypeId::new(5)]);
        assert_eq!(
            lowered
                .types
                .iter()
                .find(|type_| type_.id == ir::TypeId::new(5)),
            Some(&ir::Type {
                id: ir::TypeId::new(5),
                kind: ir::TypeKind::Float(ir::FloatKind::Binary32),
            })
        );
        assert_eq!(function.parameters[0].type_id, ir::TypeId::new(5));
        assert!(matches!(
            &function.blocks[0].instructions[0],
            ir::Instruction {
                results,
                kind: ir::InstructionKind::Cast {
                    op: ir::CastOp::FloatExtend,
                    to,
                    ..
                },
                ..
            } if results[0].id == ir::ValueId::new(5)
                && *to != ir::TypeId::new(5)
                && lowered.types.iter().any(|type_| type_.id == *to
                    && type_.kind == ir::TypeKind::Float(ir::FloatKind::Extended80))
        ));
    }

    #[test]
    fn split_single_load_extends_and_store_truncates() {
        // Python HIR lowering treats a field lvalue exactly like its root
        // place: store rounds extended evaluation to SINGLE storage, then a
        // load re-extends that stored value.  qmove's vector fields are this
        // zero-offset projection form.
        let mut module = split_single_module();
        let function = &mut module.functions[0];
        function.places = vec![hir::Place {
            id: hir::PlaceId::new(0),
            name: "cell".into(),
            type_id: hir::TypeId::new(5),
            storage: hir::Storage::Local,
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        }];
        function.values.push(hir::Value {
            id: hir::ValueId::new(6),
            type_id: hir::TypeId::new(5),
        });
        function.blocks[0].instructions = vec![
            hir::Instruction {
                id: hir::InstructionId::new(19),
                opcode: hir::Opcode::Store,
                results: Vec::new(),
                operands: vec![
                    hir::Operand::Projection {
                        place: hir::PlaceId::new(0),
                        indices: Vec::new(),
                        offset: 0,
                        type_id: hir::TypeId::new(5),
                    },
                    hir::Operand::Constant {
                        type_id: hir::TypeId::new(5),
                        value: hir::ConstantValue::Real("1.25".into()),
                    },
                ],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(20),
                opcode: hir::Opcode::Store,
                results: Vec::new(),
                operands: vec![
                    hir::Operand::Projection {
                        place: hir::PlaceId::new(0),
                        indices: Vec::new(),
                        offset: 0,
                        type_id: hir::TypeId::new(5),
                    },
                    hir::Operand::Value(hir::ValueId::new(5)),
                ],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(21),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(6)],
                operands: vec![hir::Operand::Projection {
                    place: hir::PlaceId::new(0),
                    indices: Vec::new(),
                    offset: 0,
                    type_id: hir::TypeId::new(5),
                }],
                callee: None,
            },
        ];
        let lowered = lower_module(&module).expect("split single memory lowers");
        let instructions = &lowered.functions[0].blocks[0].instructions;
        assert!(instructions.iter().any(|instruction| matches!(
            &instruction.kind,
            ir::InstructionKind::Store {
                value: ir::Operand::Constant(ir::TypedConstant {
                    type_id,
                    value: ir::Constant::Float(value),
                }),
                ..
            } if *type_id == ir::TypeId::new(5) && value == "1.25"
        )));
        assert!(instructions.iter().any(|instruction| matches!(
            instruction.kind,
            ir::InstructionKind::Cast {
                op: ir::CastOp::FloatTruncate,
                ..
            }
        )));
        assert!(
            instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, ir::InstructionKind::Load { .. }))
        );
        assert!(instructions.iter().any(|instruction| matches!(
            instruction.kind,
            ir::InstructionKind::Cast {
                op: ir::CastOp::FloatExtend,
                ..
            }
        )));
    }

    #[test]
    fn lowers_multidimensional_byte_offsets_in_both_source_orders() {
        // This is the Python `hir.lower.operand` array calculation: bounds
        // [(10, 11), (20, 23)] at (11, 22), width four is byte 20 in
        // column-major order and byte 24 in row-major order. A byte-three
        // field projection is consequently 23 or 27, not a raw subscript.
        fn evaluate_integer(
            operand: &ir::Operand,
            definitions: &BTreeMap<ir::ValueId, (ir::BinaryOp, ir::Operand, ir::Operand)>,
        ) -> i128 {
            match operand {
                ir::Operand::Constant(ir::TypedConstant {
                    value: ir::Constant::Integer(value),
                    ..
                }) => *value,
                ir::Operand::Value(value) => {
                    let (op, left, right) = definitions
                        .get(value)
                        .expect("GEP offset is defined by the lowered byte arithmetic");
                    let left = evaluate_integer(left, definitions);
                    let right = evaluate_integer(right, definitions);
                    match op {
                        ir::BinaryOp::Add => left + right,
                        ir::BinaryOp::Subtract => left - right,
                        ir::BinaryOp::Multiply => left * right,
                        _ => panic!("array byte offset contains non-linearized operation {op:?}"),
                    }
                }
                _ => panic!("array byte offset is not an integer expression"),
            }
        }

        let mut module = split_single_module();
        module.types.extend([
            hir::Type {
                id: hir::TypeId::new(6),
                name: "index".into(),
                kind: hir::TypeKind::Integer,
                width: 2,
                signed: Some(true),
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
            hir::Type {
                id: hir::TypeId::new(7),
                name: "single-array".into(),
                kind: hir::TypeKind::Array,
                width: 32,
                signed: None,
                evaluation: hir::FloatEvaluation::None,
                element: Some(hir::TypeId::new(5)),
                bounds: vec![(10, 11), (20, 23)],
                address: hir::AddressKind::Near,
            },
            hir::Type {
                id: hir::TypeId::new(8),
                name: "byte".into(),
                kind: hir::TypeKind::Integer,
                width: 1,
                signed: Some(true),
                evaluation: hir::FloatEvaluation::None,
                element: None,
                bounds: Vec::new(),
                address: hir::AddressKind::None,
            },
        ]);
        let function = &mut module.functions[0];
        function.places = vec![hir::Place {
            id: hir::PlaceId::new(0),
            name: "cells".into(),
            type_id: hir::TypeId::new(7),
            storage: hir::Storage::Local,
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 32,
            address: hir::AddressKind::Near,
        }];
        function.values.push(hir::Value {
            id: hir::ValueId::new(6),
            type_id: hir::TypeId::new(5),
        });
        function.values.push(hir::Value {
            id: hir::ValueId::new(7),
            type_id: hir::TypeId::new(8),
        });
        let element = || hir::Operand::Element {
            place: hir::PlaceId::new(0),
            indices: vec![
                hir::Operand::Constant {
                    type_id: hir::TypeId::new(6),
                    value: hir::ConstantValue::Integer(11),
                },
                hir::Operand::Constant {
                    type_id: hir::TypeId::new(6),
                    value: hir::ConstantValue::Integer(22),
                },
            ],
        };
        function.blocks[0].instructions = vec![
            hir::Instruction {
                id: hir::InstructionId::new(19),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(6)],
                operands: vec![element()],
                callee: None,
            },
            hir::Instruction {
                id: hir::InstructionId::new(20),
                opcode: hir::Opcode::Load,
                results: vec![hir::ValueId::new(7)],
                operands: vec![hir::Operand::Projection {
                    place: hir::PlaceId::new(0),
                    indices: match element() {
                        hir::Operand::Element { indices, .. } => indices,
                        _ => unreachable!("array element constructor returns an element"),
                    },
                    offset: 3,
                    type_id: hir::TypeId::new(8),
                }],
                callee: None,
            },
        ];

        for (order, expected, byte_offsets) in [
            (
                hir::ArrayOrder::ColumnMajor,
                vec![
                    (ir::BinaryOp::Subtract, Some(20)),
                    (ir::BinaryOp::Subtract, Some(10)),
                    (ir::BinaryOp::Multiply, Some(2)),
                    (ir::BinaryOp::Add, None),
                    (ir::BinaryOp::Multiply, Some(4)),
                    (ir::BinaryOp::Subtract, Some(20)),
                    (ir::BinaryOp::Subtract, Some(10)),
                    (ir::BinaryOp::Multiply, Some(2)),
                    (ir::BinaryOp::Add, None),
                    (ir::BinaryOp::Multiply, Some(4)),
                    (ir::BinaryOp::Add, Some(3)),
                ],
                vec![20, 23],
            ),
            (
                hir::ArrayOrder::RowMajor,
                vec![
                    (ir::BinaryOp::Subtract, Some(10)),
                    (ir::BinaryOp::Subtract, Some(20)),
                    (ir::BinaryOp::Multiply, Some(4)),
                    (ir::BinaryOp::Add, None),
                    (ir::BinaryOp::Multiply, Some(4)),
                    (ir::BinaryOp::Subtract, Some(10)),
                    (ir::BinaryOp::Subtract, Some(20)),
                    (ir::BinaryOp::Multiply, Some(4)),
                    (ir::BinaryOp::Add, None),
                    (ir::BinaryOp::Multiply, Some(4)),
                    (ir::BinaryOp::Add, Some(3)),
                ],
                vec![24, 27],
            ),
        ] {
            let lowered = lower_module_with_array_order(&module, order)
                .expect("multidimensional indexed storage lowers");
            let instructions = &lowered.functions[0].blocks[0].instructions;
            let operations = instructions
                .iter()
                .filter_map(|instruction| match &instruction.kind {
                    ir::InstructionKind::Binary { op, right, .. } => Some((
                        *op,
                        match right {
                            ir::Operand::Constant(ir::TypedConstant {
                                value: ir::Constant::Integer(value),
                                ..
                            }) => Some(*value),
                            _ => None,
                        },
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(operations, expected, "{order:?} operation order");
            let geps = instructions
                .iter()
                .filter_map(|instruction| match &instruction.kind {
                    ir::InstructionKind::GetElementPointer { indices, .. } => Some(indices),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(geps.len(), 2, "{order:?} emits one GEP per lvalue");
            assert!(geps.iter().all(|indices| indices.len() == 1));
            let definitions = instructions
                .iter()
                .filter_map(|instruction| match &instruction.kind {
                    ir::InstructionKind::Binary { op, left, right } => Some((
                        instruction.results[0].id,
                        (*op, left.clone(), right.clone()),
                    )),
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                geps.iter()
                    .map(|indices| evaluate_integer(&indices[0], &definitions))
                    .collect::<Vec<_>>(),
                byte_offsets,
                "{order:?} byte offsets"
            );
            lowered.verify().expect("indexed storage verifies");
        }
    }

    #[test]
    fn refuses_a_noninteger_projected_memory_index() {
        let mut module = split_single_module();
        let function = &mut module.functions[0];
        function.places = vec![hir::Place {
            id: hir::PlaceId::new(0),
            name: "cell".into(),
            type_id: hir::TypeId::new(5),
            storage: hir::Storage::Local,
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        }];
        function.blocks[0].instructions = vec![hir::Instruction {
            id: hir::InstructionId::new(19),
            opcode: hir::Opcode::Store,
            results: Vec::new(),
            operands: vec![
                hir::Operand::Projection {
                    place: hir::PlaceId::new(0),
                    indices: vec![hir::Operand::Constant {
                        type_id: hir::TypeId::new(5),
                        value: hir::ConstantValue::Real("1.0".into()),
                    }],
                    offset: 0,
                    type_id: hir::TypeId::new(5),
                },
                hir::Operand::Value(hir::ValueId::new(5)),
            ],
            callee: None,
        }];

        assert!(matches!(
            lower_module(&module),
            Err(LowerError::InvalidInstruction {
                block,
                instruction,
                property: InvalidProperty::OperandTypes,
                ..
            }) if block == hir::BlockId::new(4) && instruction == hir::InstructionId::new(19)
        ));
    }
}
