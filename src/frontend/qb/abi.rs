//! Microsoft BASIC's hidden floating-function result pointer.
//!
//! This is the HIR counterpart of `frontend/qb/abi.py::physicalize`'s
//! floating-return slice.  HIR still distinguishes a storage `SINGLE` or
//! `DOUBLE` from its extended-precision evaluation value and retains pointer
//! element types.  That information is deliberately gone after generic HIR
//! lowering, so this ABI policy belongs here rather than in portable IR.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::hir::{
    AddressKind, CallAbi, CallDistance, Callable, CallableId, FloatEvaluation, Function,
    FunctionId, Instruction, InstructionId, Module, Opcode, Operand, StackCleanup, Terminator,
    Type, TypeId, TypeKind, Value, ValueId,
};

/// A malformed HIR property that prevents exact floating-result
/// physicalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicFloatAbiError {
    DuplicateType(TypeId),
    MissingType(TypeId),
    DuplicateCallable(CallableId),
    MissingCallable {
        function: FunctionId,
        instruction: InstructionId,
        callable: CallableId,
    },
    AmbiguousDefinedFunction {
        callable: CallableId,
        name: String,
    },
    DuplicateCallAbi {
        function: FunctionId,
        instruction: InstructionId,
    },
    MissingValue {
        function: FunctionId,
        value: ValueId,
    },
    DuplicateValue {
        function: FunctionId,
        value: ValueId,
    },
    MalformedFloatingFunction {
        function: FunctionId,
        reason: &'static str,
    },
    MalformedFloatingCall {
        function: FunctionId,
        instruction: InstructionId,
        reason: &'static str,
    },
    ValueIdExhausted {
        function: FunctionId,
    },
    InstructionIdExhausted {
        function: FunctionId,
    },
}

impl fmt::Display for BasicFloatAbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateType(type_id) => write!(formatter, "duplicate HIR type {type_id}"),
            Self::MissingType(type_id) => write!(formatter, "missing HIR type {type_id}"),
            Self::DuplicateCallable(callable) => {
                write!(formatter, "duplicate HIR callable {callable}")
            }
            Self::MissingCallable {
                function,
                instruction,
                callable,
            } => write!(
                formatter,
                "function {function} call {instruction} refers to missing callable {callable}"
            ),
            Self::AmbiguousDefinedFunction { callable, name } => write!(
                formatter,
                "callable {callable} ({name:?}) resolves to multiple defined functions"
            ),
            Self::DuplicateCallAbi {
                function,
                instruction,
            } => write!(
                formatter,
                "function {function} call {instruction} has multiple ABI records"
            ),
            Self::MissingValue { function, value } => {
                write!(
                    formatter,
                    "function {function} refers to missing value {value}"
                )
            }
            Self::DuplicateValue { function, value } => {
                write!(formatter, "function {function} has duplicate value {value}")
            }
            Self::MalformedFloatingFunction { function, reason } => {
                write!(
                    formatter,
                    "function {function} has malformed floating ABI: {reason}"
                )
            }
            Self::MalformedFloatingCall {
                function,
                instruction,
                reason,
            } => write!(
                formatter,
                "function {function} call {instruction} has malformed floating ABI: {reason}"
            ),
            Self::ValueIdExhausted { function } => {
                write!(formatter, "function {function} exhausted value IDs")
            }
            Self::InstructionIdExhausted { function } => {
                write!(formatter, "function {function} exhausted instruction IDs")
            }
        }
    }
}

impl Error for BasicFloatAbiError {}

/// Physicalizes Microsoft BASIC `SINGLE` and `DOUBLE` function results.
///
/// The input module is never changed.  For a defined far-Pascal floating
/// function whose final formal is the measured near result pointer, the
/// physical result becomes that pointer.  Each semantic return stores through
/// it, leaving generic HIR lowering to perform its established dynamic-rounding
/// storage truncation.  A direct caller receives the pointer and immediately
/// reloads the original extended-precision SSA value.
pub fn physicalize(module: &Module) -> Result<Module, BasicFloatAbiError> {
    let types = unique_types(module)?;
    let callables = unique_callables(module)?;
    let function_plans = function_plans(module, &types)?;
    let callable_plans = callable_plans(module, &types, &function_plans)?;

    // Validate every direct candidate before cloning so every refusal leaves
    // the source and the would-be result untouched.
    let caller_plans = caller_plans(module, &callables, &callable_plans)?;

    let mut physical = module.clone();
    for callable in &mut physical.callables {
        if let Some(plan) = callable_plans.get(&callable.id) {
            callable.result_type = Some(plan.pointer_type);
        }
    }
    for function in &mut physical.functions {
        if let Some(plan) = function_plans.get(&function.id) {
            physicalize_function(function, plan)?;
        }
        if let Some(plans) = caller_plans.get(&function.id) {
            physicalize_calls(function, plans)?;
        }
    }
    Ok(physical)
}

#[derive(Clone, Copy, Debug)]
struct FunctionPlan {
    result_type: TypeId,
    pointer_type: TypeId,
    hidden_parameter: ValueId,
}

#[derive(Clone, Copy, Debug)]
struct CallablePlan {
    result_type: TypeId,
    pointer_type: TypeId,
}

#[derive(Clone, Copy, Debug)]
struct CallPlan {
    instruction: InstructionId,
    result_type: TypeId,
    pointer_type: TypeId,
}

fn unique_types(module: &Module) -> Result<BTreeMap<TypeId, &Type>, BasicFloatAbiError> {
    let mut types = BTreeMap::new();
    for type_ in &module.types {
        if types.insert(type_.id, type_).is_some() {
            return Err(BasicFloatAbiError::DuplicateType(type_.id));
        }
    }
    Ok(types)
}

fn unique_callables(
    module: &Module,
) -> Result<BTreeMap<CallableId, &Callable>, BasicFloatAbiError> {
    let mut callables = BTreeMap::new();
    for callable in &module.callables {
        if callables.insert(callable.id, callable).is_some() {
            return Err(BasicFloatAbiError::DuplicateCallable(callable.id));
        }
    }
    Ok(callables)
}

fn function_plans(
    module: &Module,
    types: &BTreeMap<TypeId, &Type>,
) -> Result<BTreeMap<FunctionId, FunctionPlan>, BasicFloatAbiError> {
    let mut plans = BTreeMap::new();
    for function in &module.functions {
        let Some(result) = types.get(&function.result_type).copied() else {
            return Err(BasicFloatAbiError::MissingType(function.result_type));
        };
        if !is_split_float(result)
            || function.abi.distance != CallDistance::Far
            || function.abi.cleanup != StackCleanup::Callee
            || function.parameters.is_empty()
        {
            continue;
        }
        let values = unique_values(function)?;
        let hidden_parameter = *function.parameters.last().expect("nonempty was checked");
        let Some(pointer_type) = values.get(&hidden_parameter).copied() else {
            return Err(BasicFloatAbiError::MalformedFloatingFunction {
                function: function.id,
                reason: "final parameter does not name a value",
            });
        };
        if is_near_pointer_to(types, pointer_type, function.result_type) {
            plans.insert(
                function.id,
                FunctionPlan {
                    result_type: function.result_type,
                    pointer_type,
                    hidden_parameter,
                },
            );
        }
    }
    Ok(plans)
}

fn callable_plans(
    module: &Module,
    types: &BTreeMap<TypeId, &Type>,
    function_plans: &BTreeMap<FunctionId, FunctionPlan>,
) -> Result<BTreeMap<CallableId, CallablePlan>, BasicFloatAbiError> {
    let mut plans = BTreeMap::new();
    for callable in &module.callables {
        if !callable.defined {
            continue;
        }
        let Some(result_type) = callable.result_type else {
            continue;
        };
        let Some(result) = types.get(&result_type).copied() else {
            return Err(BasicFloatAbiError::MissingType(result_type));
        };
        let Some(hidden) = callable.parameters.last() else {
            continue;
        };
        if is_split_float(result)
            && hidden.by_value
            && !hidden.array
            && !hidden.segmented
            && is_near_pointer_to(types, hidden.type_id, result_type)
        {
            // Calls resolve defined targets through this normalized name rule
            // in `hir::calls`; use the same rule before changing a callable's
            // physical result declaration.
            let name = normalized_name(&callable.name);
            let matches = module
                .functions
                .iter()
                .filter(|function| normalized_name(&function.name) == name)
                .filter_map(|function| function_plans.get(&function.id))
                .filter(|plan| {
                    plan.result_type == result_type && plan.pointer_type == hidden.type_id
                })
                .count();
            if matches == 0 {
                continue;
            }
            if matches > 1 {
                return Err(BasicFloatAbiError::AmbiguousDefinedFunction {
                    callable: callable.id,
                    name,
                });
            }
            plans.insert(
                callable.id,
                CallablePlan {
                    result_type,
                    pointer_type: hidden.type_id,
                },
            );
        }
    }
    Ok(plans)
}

fn caller_plans(
    module: &Module,
    callables: &BTreeMap<CallableId, &Callable>,
    callable_plans: &BTreeMap<CallableId, CallablePlan>,
) -> Result<BTreeMap<FunctionId, Vec<CallPlan>>, BasicFloatAbiError> {
    let mut all = BTreeMap::new();
    for function in &module.functions {
        let values = unique_values(function)?;
        let mut plans = Vec::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if instruction.opcode != Opcode::Call {
                    continue;
                }
                let Some(abi) = matching_call_abi(function, instruction.id)? else {
                    continue;
                };
                if abi.distance != CallDistance::Far || abi.cleanup != StackCleanup::Callee {
                    continue;
                }
                let Some(callable_id) = abi.callee else {
                    continue;
                };
                if !callables.contains_key(&callable_id) {
                    return Err(BasicFloatAbiError::MissingCallable {
                        function: function.id,
                        instruction: instruction.id,
                        callable: callable_id,
                    });
                }
                let Some(plan) = callable_plans.get(&callable_id).copied() else {
                    continue;
                };
                let [result] = instruction.results.as_slice() else {
                    return Err(BasicFloatAbiError::MalformedFloatingCall {
                        function: function.id,
                        instruction: instruction.id,
                        reason: "call does not have exactly one result",
                    });
                };
                let Some(result_type) = values.get(result).copied() else {
                    return Err(BasicFloatAbiError::MissingValue {
                        function: function.id,
                        value: *result,
                    });
                };
                if result_type != plan.result_type {
                    return Err(BasicFloatAbiError::MalformedFloatingCall {
                        function: function.id,
                        instruction: instruction.id,
                        reason: "call result type differs from its defined callable",
                    });
                }
                plans.push(CallPlan {
                    instruction: instruction.id,
                    result_type,
                    pointer_type: plan.pointer_type,
                });
            }
        }
        if !plans.is_empty() {
            all.insert(function.id, plans);
        }
    }
    Ok(all)
}

fn physicalize_function(
    function: &mut Function,
    plan: &FunctionPlan,
) -> Result<(), BasicFloatAbiError> {
    let mut next_instruction = fresh_instruction_id(function);
    let function_id = function.id;
    function.result_type = plan.pointer_type;
    for block in &mut function.blocks {
        let Terminator::Return(Some(returned)) = &block.terminator else {
            continue;
        };
        block.instructions.push(Instruction {
            id: next_instruction_id(&mut next_instruction, function_id)?,
            opcode: Opcode::Store,
            results: Vec::new(),
            operands: vec![
                Operand::Indirect {
                    base: plan.hidden_parameter,
                    offset: 0,
                    type_id: plan.result_type,
                    volatile: false,
                },
                returned.clone(),
            ],
            callee: None,
        });
        block.terminator = Terminator::Return(Some(Operand::Value(plan.hidden_parameter)));
    }
    Ok(())
}

fn physicalize_calls(
    function: &mut Function,
    plans: &[CallPlan],
) -> Result<(), BasicFloatAbiError> {
    let plans = plans
        .iter()
        .map(|plan| (plan.instruction, *plan))
        .collect::<BTreeMap<_, _>>();
    let mut next_value = fresh_value_id(function);
    let mut next_instruction = fresh_instruction_id(function);
    let function_id = function.id;
    let mut introduced = Vec::new();

    for block in &mut function.blocks {
        let originals = std::mem::take(&mut block.instructions);
        let mut rewritten = Vec::with_capacity(originals.len());
        for mut instruction in originals {
            let Some(plan) = plans.get(&instruction.id) else {
                rewritten.push(instruction);
                continue;
            };
            let [semantic_result] = instruction.results.as_slice() else {
                return Err(BasicFloatAbiError::MalformedFloatingCall {
                    function: function.id,
                    instruction: instruction.id,
                    reason: "call does not have exactly one result",
                });
            };
            let semantic_result = *semantic_result;
            let pointer = next_value_id(&mut next_value, function_id)?;
            introduced.push(Value {
                id: pointer,
                type_id: plan.pointer_type,
            });
            instruction.results = vec![pointer];
            rewritten.push(instruction);
            rewritten.push(Instruction {
                id: next_instruction_id(&mut next_instruction, function_id)?,
                opcode: Opcode::Load,
                results: vec![semantic_result],
                operands: vec![Operand::Indirect {
                    base: pointer,
                    offset: 0,
                    type_id: plan.result_type,
                    volatile: false,
                }],
                callee: None,
            });
        }
        block.instructions = rewritten;
    }
    function.values.extend(introduced);
    Ok(())
}

fn unique_values(function: &Function) -> Result<BTreeMap<ValueId, TypeId>, BasicFloatAbiError> {
    let mut values = BTreeMap::new();
    for value in &function.values {
        if values.insert(value.id, value.type_id).is_some() {
            return Err(BasicFloatAbiError::DuplicateValue {
                function: function.id,
                value: value.id,
            });
        }
    }
    Ok(values)
}

fn matching_call_abi(
    function: &Function,
    instruction: InstructionId,
) -> Result<Option<&CallAbi>, BasicFloatAbiError> {
    let mut abis = function
        .calls
        .iter()
        .filter(|abi| abi.instruction == instruction);
    let Some(abi) = abis.next() else {
        return Ok(None);
    };
    if abis.next().is_some() {
        return Err(BasicFloatAbiError::DuplicateCallAbi {
            function: function.id,
            instruction,
        });
    }
    Ok(Some(abi))
}

fn fresh_value_id(function: &Function) -> Option<u32> {
    function
        .values
        .iter()
        .map(|value| value.id.get())
        .max()
        .map_or(Some(0), |id| id.checked_add(1))
}

fn fresh_instruction_id(function: &Function) -> Option<u32> {
    function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.get())
        })
        .max()
        .map_or(Some(0), |id| id.checked_add(1))
}

fn next_value_id(
    next: &mut Option<u32>,
    function: FunctionId,
) -> Result<ValueId, BasicFloatAbiError> {
    let id = next
        .take()
        .ok_or(BasicFloatAbiError::ValueIdExhausted { function })?;
    *next = id.checked_add(1);
    Ok(ValueId::new(id))
}

fn next_instruction_id(
    next: &mut Option<u32>,
    function: FunctionId,
) -> Result<InstructionId, BasicFloatAbiError> {
    let id = next
        .take()
        .ok_or(BasicFloatAbiError::InstructionIdExhausted { function })?;
    *next = id.checked_add(1);
    Ok(InstructionId::new(id))
}

fn is_split_float(type_: &Type) -> bool {
    type_.kind == TypeKind::Float
        && matches!(type_.width, 4 | 8)
        && type_.evaluation == FloatEvaluation::Extended80
}

fn is_near_pointer_to(
    types: &BTreeMap<TypeId, &Type>,
    pointer_type: TypeId,
    element: TypeId,
) -> bool {
    matches!(
        types.get(&pointer_type),
        Some(Type {
            kind: TypeKind::Pointer,
            address: AddressKind::Near,
            element: Some(actual),
            ..
        }) if *actual == element
    )
}

fn normalized_name(name: &str) -> String {
    name.trim()
        .trim_end_matches(['%', '&', '!', '#', '$'])
        .to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::physicalize;
    use crate::hir::{
        AddressKind, Block, BlockId, CallAbi, CallDistance, Callable, CallableId, FloatEvaluation,
        Function, FunctionId, Instruction, InstructionId, Linkage, Module, ModuleId, Opcode,
        Operand, Parameter, ProcedureAbi, StackCleanup, Terminator, Type, TypeId, TypeKind, Value,
        ValueId,
    };

    const VOID: TypeId = TypeId::new(0);
    const SINGLE: TypeId = TypeId::new(1);
    const DOUBLE: TypeId = TypeId::new(2);
    const SINGLE_POINTER: TypeId = TypeId::new(3);
    const DOUBLE_POINTER: TypeId = TypeId::new(4);
    const INTEGER: TypeId = TypeId::new(5);

    fn type_(id: TypeId, kind: TypeKind, width: usize) -> Type {
        Type {
            id,
            name: format!("type{}", id.get()),
            kind,
            width,
            signed: (kind == TypeKind::Integer).then_some(true),
            evaluation: FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: AddressKind::None,
        }
    }

    fn module(functions: Vec<Function>, callables: Vec<Callable>) -> Module {
        let mut single = type_(SINGLE, TypeKind::Float, 4);
        single.evaluation = FloatEvaluation::Extended80;
        let mut double = type_(DOUBLE, TypeKind::Float, 8);
        double.evaluation = FloatEvaluation::Extended80;
        let mut single_pointer = type_(SINGLE_POINTER, TypeKind::Pointer, 2);
        single_pointer.address = AddressKind::Near;
        single_pointer.element = Some(SINGLE);
        let mut double_pointer = type_(DOUBLE_POINTER, TypeKind::Pointer, 2);
        double_pointer.address = AddressKind::Near;
        double_pointer.element = Some(DOUBLE);
        Module {
            id: ModuleId::new(0),
            name: "float_abi".into(),
            types: vec![
                type_(VOID, TypeKind::Void, 0),
                single,
                double,
                single_pointer,
                double_pointer,
                type_(INTEGER, TypeKind::Integer, 2),
            ],
            functions,
            data: Vec::new(),
            callables,
        }
    }

    fn function(
        id: u32,
        name: &str,
        result_type: TypeId,
        parameters: Vec<Value>,
        instructions: Vec<Instruction>,
        terminator: Terminator,
    ) -> Function {
        let parameter_ids = parameters.iter().map(|value| value.id).collect();
        Function {
            id: FunctionId::new(id),
            name: name.into(),
            result_type,
            values: parameters,
            places: Vec::new(),
            blocks: vec![Block {
                id: BlockId::new(0),
                instructions,
                terminator,
            }],
            entry: BlockId::new(0),
            parameters: parameter_ids,
            abi: ProcedureAbi {
                cleanup: StackCleanup::Callee,
                distance: CallDistance::Far,
                parameter_bytes: 2,
            },
            calls: Vec::new(),
            error_handler: None,
            error_handler_local: false,
            external_entries: Vec::new(),
            linkage: Linkage::Internal,
        }
    }

    fn callable(id: u32, name: &str, result_type: TypeId, pointer_type: TypeId) -> Callable {
        Callable {
            id: CallableId::new(id),
            name: name.into(),
            result_type: Some(result_type),
            parameters: vec![Parameter {
                type_id: pointer_type,
                by_value: true,
                segmented: false,
                array: false,
            }],
            defined: true,
        }
    }

    fn callable_with_integer_argument(
        id: u32,
        name: &str,
        result_type: TypeId,
        pointer_type: TypeId,
    ) -> Callable {
        let mut callable = callable(id, name, result_type, pointer_type);
        callable.parameters.insert(
            0,
            Parameter {
                type_id: INTEGER,
                by_value: true,
                segmented: false,
                array: false,
            },
        );
        callable
    }

    #[test]
    fn callee_stores_storage_float_then_returns_the_hidden_pointer() {
        let hidden = Value {
            id: ValueId::new(0),
            type_id: SINGLE_POINTER,
        };
        let result = Value {
            id: ValueId::new(1),
            type_id: SINGLE,
        };
        let mut callee = function(
            1,
            "answer!",
            SINGLE,
            vec![hidden.clone(), result.clone()],
            Vec::new(),
            Terminator::Return(Some(Operand::Value(result.id))),
        );
        callee.parameters = vec![hidden.id];
        let output = physicalize(&module(
            vec![callee],
            vec![callable(7, "ANSWER", SINGLE, SINGLE_POINTER)],
        ))
        .expect("physicalizes the defined SINGLE function");
        output.verify().expect("physicalized callee HIR verifies");
        let function = &output.functions[0];
        assert_eq!(function.result_type, SINGLE_POINTER);
        assert_eq!(function.blocks[0].instructions.len(), 1);
        assert_eq!(
            function.blocks[0].instructions[0],
            Instruction {
                id: InstructionId::new(0),
                opcode: Opcode::Store,
                results: Vec::new(),
                operands: vec![
                    Operand::Indirect {
                        base: hidden.id,
                        offset: 0,
                        type_id: SINGLE,
                        volatile: false,
                    },
                    Operand::Value(result.id),
                ],
                callee: None,
            }
        );
        assert_eq!(
            function.blocks[0].terminator,
            Terminator::Return(Some(Operand::Value(hidden.id)))
        );
        assert_eq!(output.callables[0].result_type, Some(SINGLE_POINTER));
    }

    #[test]
    fn caller_receives_pointer_then_immediately_loads_original_float_without_reordering() {
        let argument = Value {
            id: ValueId::new(0),
            type_id: INTEGER,
        };
        let hidden_argument = Value {
            id: ValueId::new(1),
            type_id: SINGLE_POINTER,
        };
        let result = Value {
            id: ValueId::new(2),
            type_id: SINGLE,
        };
        let call = Instruction {
            id: InstructionId::new(4),
            opcode: Opcode::Call,
            results: vec![result.id],
            operands: vec![
                Operand::Value(argument.id),
                Operand::Value(hidden_argument.id),
            ],
            callee: Some("answer".into()),
        };
        let mut caller = function(
            0,
            "caller",
            VOID,
            vec![argument.clone(), hidden_argument.clone(), result.clone()],
            vec![call],
            Terminator::Return(None),
        );
        caller.abi.parameter_bytes = 0;
        caller.calls.push(CallAbi {
            instruction: InstructionId::new(4),
            order: vec![0, 1],
            cleanup: StackCleanup::Callee,
            distance: CallDistance::Far,
            callee: Some(CallableId::new(7)),
        });
        let target_argument = Value {
            id: ValueId::new(0),
            type_id: INTEGER,
        };
        let target_hidden = Value {
            id: ValueId::new(1),
            type_id: SINGLE_POINTER,
        };
        let target_result = Value {
            id: ValueId::new(2),
            type_id: SINGLE,
        };
        let mut target = function(
            1,
            "answer!",
            SINGLE,
            vec![
                target_argument.clone(),
                target_hidden.clone(),
                target_result.clone(),
            ],
            Vec::new(),
            Terminator::Return(Some(Operand::Value(target_result.id))),
        );
        target.parameters = vec![target_argument.id, target_hidden.id];
        let output = physicalize(&module(
            vec![caller, target],
            vec![callable_with_integer_argument(
                7,
                "answer!",
                SINGLE,
                SINGLE_POINTER,
            )],
        ))
        .expect("physicalizes defined floating call");
        output.verify().expect("physicalized caller HIR verifies");
        let instructions = &output.functions[0].blocks[0].instructions;
        let pointer = ValueId::new(3);
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].results, vec![pointer]);
        assert_eq!(
            instructions[0].operands,
            vec![
                Operand::Value(argument.id),
                Operand::Value(hidden_argument.id)
            ]
        );
        assert_eq!(output.functions[0].calls[0].order, vec![0, 1]);
        assert_eq!(
            instructions[1],
            Instruction {
                id: InstructionId::new(5),
                opcode: Opcode::Load,
                results: vec![result.id],
                operands: vec![Operand::Indirect {
                    base: pointer,
                    offset: 0,
                    type_id: SINGLE,
                    volatile: false,
                }],
                callee: None,
            }
        );
        assert!(output.functions[0].values.contains(&Value {
            id: pointer,
            type_id: SINGLE_POINTER,
        }));
    }

    #[test]
    fn single_and_double_keep_their_storage_types_in_indirect_accesses() {
        for (float, pointer) in [(SINGLE, SINGLE_POINTER), (DOUBLE, DOUBLE_POINTER)] {
            let hidden = Value {
                id: ValueId::new(0),
                type_id: pointer,
            };
            let result = Value {
                id: ValueId::new(1),
                type_id: float,
            };
            let mut callee = function(
                float.get(),
                "scalar",
                float,
                vec![hidden.clone(), result.clone()],
                Vec::new(),
                Terminator::Return(Some(Operand::Value(result.id))),
            );
            callee.parameters = vec![hidden.id];
            let output =
                physicalize(&module(vec![callee], Vec::new())).expect("physicalizes float");
            output.verify().expect("physicalized scalar HIR verifies");
            let Operand::Indirect { type_id, .. } =
                output.functions[0].blocks[0].instructions[0].operands[0]
            else {
                panic!("store uses an indirect result pointer");
            };
            assert_eq!(type_id, float);
        }
    }

    #[test]
    fn cdecl_and_runtime_calls_are_unchanged() {
        let result = Value {
            id: ValueId::new(0),
            type_id: SINGLE,
        };
        let call = Instruction {
            id: InstructionId::new(0),
            opcode: Opcode::Call,
            results: vec![result.id],
            operands: Vec::new(),
            callee: Some("runtime".into()),
        };
        let mut caller = function(
            0,
            "caller",
            VOID,
            vec![result],
            vec![call],
            Terminator::Return(None),
        );
        caller.calls = vec![CallAbi {
            instruction: InstructionId::new(0),
            order: Vec::new(),
            cleanup: StackCleanup::Caller,
            distance: CallDistance::Far,
            callee: Some(CallableId::new(7)),
        }];
        let target_hidden = Value {
            id: ValueId::new(0),
            type_id: SINGLE_POINTER,
        };
        let target_result = Value {
            id: ValueId::new(1),
            type_id: SINGLE,
        };
        let mut target = function(
            1,
            "runtime",
            SINGLE,
            vec![target_hidden.clone(), target_result.clone()],
            Vec::new(),
            Terminator::Return(Some(Operand::Value(target_result.id))),
        );
        target.parameters = vec![target_hidden.id];
        target.abi.cleanup = StackCleanup::Caller;
        let input = module(
            vec![caller, target],
            vec![callable(7, "runtime", SINGLE, SINGLE_POINTER)],
        );
        let cdecl = physicalize(&input).expect("CDECL call is left alone");
        cdecl.verify().expect("unchanged CDECL HIR verifies");
        assert_eq!(cdecl, input);

        let mut runtime = input.clone();
        runtime.functions[0].calls[0].cleanup = StackCleanup::Callee;
        runtime.functions[0].calls[0].callee = None;
        let output = physicalize(&runtime).expect("runtime call is left alone");
        output.verify().expect("unchanged runtime HIR verifies");
        assert_eq!(output, runtime);
    }

    #[test]
    fn input_module_is_not_changed() {
        let hidden = Value {
            id: ValueId::new(0),
            type_id: SINGLE_POINTER,
        };
        let result = Value {
            id: ValueId::new(1),
            type_id: SINGLE,
        };
        let mut callee = function(
            1,
            "answer",
            SINGLE,
            vec![hidden.clone(), result.clone()],
            Vec::new(),
            Terminator::Return(Some(Operand::Value(result.id))),
        );
        callee.parameters = vec![hidden.id];
        let input = module(
            vec![callee],
            vec![callable(7, "answer", SINGLE, SINGLE_POINTER)],
        );
        let before = input.clone();
        let output = physicalize(&input).expect("physicalizes copy");
        output.verify().expect("physicalized copied HIR verifies");
        assert_eq!(input, before);
        assert_ne!(output, before);
    }
}
