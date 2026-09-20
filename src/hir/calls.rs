//! Planning of exact runtime-call declarations for HIR lowering.
//!
//! This module validates only the narrow runtime ABI shape that portable IR
//! can represent today.  It does not lower operands, choose effects, or infer
//! any semantics from a routine name.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{hir, ir};

/// Call declarations and per-site lowering information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallPlan {
    /// Declarations in deterministic callee-name order.
    pub(super) declarations: Vec<ir::Function>,
    /// Target and ABI argument order for each source call site.
    pub(super) sites: BTreeMap<(hir::FunctionId, hir::InstructionId), PlannedCall>,
}

/// The target declaration and source operand order for one call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PlannedCall {
    pub(super) target: ir::FunctionId,
    /// Source operand indices in callee ABI order.
    pub(super) argument_indices: Vec<usize>,
}

/// A type signature inferred from one call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSignature {
    pub result: ir::TypeId,
    pub parameters: Vec<ir::TypeId>,
}

/// The malformed property of a call ABI order list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiOrderError {
    WrongLength,
    OutOfBounds { index: usize },
    Duplicate { index: usize },
}

/// An unsupported operand form at a call site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallOperandError {
    Place,
    Element,
    Projection,
    Indirect,
}

/// A callable parameter shape the portable ABI cannot represent exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterError {
    Array,
    Segmented,
}

/// A refusal raised while constructing a call plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallPlanError {
    MissingCalleeName {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
    },
    MissingAbi {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
    },
    DuplicateAbi {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
    },
    OrphanAbi {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
    },
    MissingCallable {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
    },
    AmbiguousCallable {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
    },
    UndefinedCallable {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
    },
    UnsupportedCallableParameter {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        parameter: usize,
        issue: CallableParameterError,
    },
    CallableResultMismatch {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        expected: ir::TypeId,
        actual: ir::TypeId,
    },
    CallableParameterCount {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        expected: usize,
        actual: usize,
    },
    CallableByValueParameterMismatch {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        parameter: usize,
        expected: ir::TypeId,
        actual: ir::TypeId,
    },
    CallableByReferenceParameterMismatch {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        parameter: usize,
        expected: hir::TypeId,
        actual: ir::TypeId,
    },
    UnsupportedDistance {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        distance: hir::CallDistance,
    },
    UnsupportedCleanup {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        cleanup: hir::StackCleanup,
    },
    ResultArity {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        count: usize,
    },
    UnknownValue {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        value: hir::ValueId,
    },
    AmbiguousValue {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        value: hir::ValueId,
    },
    UnsupportedOperand {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        index: usize,
        operand: CallOperandError,
    },
    MalformedOrder {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        issue: AbiOrderError,
    },
    MissingVoidType,
    AmbiguousVoidType,
    SignatureConflict {
        callee: String,
        existing: CallSignature,
        incoming: CallSignature,
    },
    MissingDefinedFunction {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        name: String,
    },
    AmbiguousDefinedFunction {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        name: String,
    },
    DefinedFunctionSignatureMismatch {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
        name: String,
        expected: CallSignature,
        actual: Vec<CallSignature>,
    },
    DefinedFunctionDistance {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        target: hir::FunctionId,
        distance: hir::CallDistance,
    },
    DefinedFunctionCleanup {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        target: hir::FunctionId,
        cleanup: hir::StackCleanup,
    },
    DuplicateSite {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
    },
    MissingDeclaration {
        callee: String,
    },
    FunctionIdOverflow {
        maximum: hir::FunctionId,
    },
    ParameterIdOverflow {
        callee: String,
        count: usize,
    },
}

impl fmt::Display for CallPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCalleeName {
                function,
                instruction,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has no callee name"
            ),
            Self::MissingAbi {
                function,
                instruction,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has no ABI"
            ),
            Self::DuplicateAbi {
                function,
                instruction,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has duplicate ABI metadata"
            ),
            Self::OrphanAbi {
                function,
                instruction,
            } => write!(
                formatter,
                "function {function} ABI metadata refers to non-call instruction {instruction}"
            ),
            Self::MissingCallable {
                function,
                instruction,
                callable,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} refers to unknown callable {callable}"
            ),
            Self::AmbiguousCallable {
                function,
                instruction,
                callable,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} refers to duplicate callable {callable}"
            ),
            Self::UndefinedCallable {
                function,
                instruction,
                callable,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} refers to undefined callable {callable}"
            ),
            Self::UnsupportedCallableParameter {
                function,
                instruction,
                callable,
                parameter,
                issue,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} parameter {parameter} is unsupported: {issue:?}"
            ),
            Self::CallableResultMismatch {
                function,
                instruction,
                callable,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} returns type {expected}, not {actual}"
            ),
            Self::CallableParameterCount {
                function,
                instruction,
                callable,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} has {actual} ABI parameters, expected {expected}"
            ),
            Self::CallableByValueParameterMismatch {
                function,
                instruction,
                callable,
                parameter,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} parameter {parameter} has type {expected}, not {actual}"
            ),
            Self::CallableByReferenceParameterMismatch {
                function,
                instruction,
                callable,
                parameter,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} parameter {parameter} needs a pointer to type {expected}, not type {actual}"
            ),
            Self::UnsupportedDistance {
                function,
                instruction,
                distance,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has unsupported {distance:?} distance"
            ),
            Self::UnsupportedCleanup {
                function,
                instruction,
                cleanup,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has unsupported {cleanup:?} cleanup"
            ),
            Self::ResultArity {
                function,
                instruction,
                count,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has {count} results, expected at most one"
            ),
            Self::UnknownValue {
                function,
                instruction,
                value,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} references unknown value {value}"
            ),
            Self::AmbiguousValue {
                function,
                instruction,
                value,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} references ambiguous value {value}"
            ),
            Self::UnsupportedOperand {
                function,
                instruction,
                index,
                operand,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} operand {index} is unsupported: {operand:?}"
            ),
            Self::MalformedOrder {
                function,
                instruction,
                issue,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has malformed ABI order: {issue:?}"
            ),
            Self::MissingVoidType => write!(
                formatter,
                "runtime calls without a result require one void type"
            ),
            Self::AmbiguousVoidType => write!(
                formatter,
                "runtime calls without a result require exactly one void type"
            ),
            Self::SignatureConflict { callee, .. } => write!(
                formatter,
                "runtime callee {callee:?} has incompatible inferred signatures"
            ),
            Self::MissingDefinedFunction {
                function,
                instruction,
                callable,
                name,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} has no defined target named {name:?}"
            ),
            Self::AmbiguousDefinedFunction {
                function,
                instruction,
                callable,
                name,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} has multiple matching targets named {name:?}"
            ),
            Self::DefinedFunctionSignatureMismatch {
                function,
                instruction,
                callable,
                name,
                ..
            } => write!(
                formatter,
                "function {function} call instruction {instruction} callable {callable} target {name:?} has an incompatible ABI signature"
            ),
            Self::DefinedFunctionDistance {
                function,
                instruction,
                target,
                distance,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} target {target} has unsupported {distance:?} distance"
            ),
            Self::DefinedFunctionCleanup {
                function,
                instruction,
                target,
                cleanup,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} target {target} has unsupported {cleanup:?} cleanup"
            ),
            Self::DuplicateSite {
                function,
                instruction,
            } => write!(
                formatter,
                "function {function} repeats call instruction {instruction}"
            ),
            Self::MissingDeclaration { callee } => {
                write!(formatter, "runtime callee {callee:?} has no declaration")
            }
            Self::FunctionIdOverflow { maximum } => write!(
                formatter,
                "cannot allocate a runtime declaration after function id {maximum}"
            ),
            Self::ParameterIdOverflow { callee, count } => write!(
                formatter,
                "runtime callee {callee:?} has {count} parameters, exceeding the value-id range"
            ),
        }
    }
}

impl Error for CallPlanError {}

/// Builds runtime declarations and direct-call lowering metadata.
pub(super) fn plan_calls(module: &hir::Module) -> Result<CallPlan, CallPlanError> {
    let mut void_type = None;
    let mut signatures = BTreeMap::<String, CallSignature>::new();
    let mut pending = BTreeMap::new();

    for function in &module.functions {
        let values = ValueTypes::new(function);
        let mut call_instructions = BTreeSet::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if instruction.opcode != hir::Opcode::Call {
                    continue;
                }
                call_instructions.insert(instruction.id);

                let site = (function.id, instruction.id);
                if pending.contains_key(&site) {
                    return Err(CallPlanError::DuplicateSite {
                        function: function.id,
                        instruction: instruction.id,
                    });
                }
                let abi = matching_abi(function, instruction.id)?;
                validate_abi(function.id, instruction, abi)?;
                let signature = infer_signature(
                    module,
                    function.id,
                    instruction,
                    &values,
                    abi,
                    &mut void_type,
                )?;
                let call = match abi.callee {
                    Some(callable) => PendingCall::Defined {
                        target: resolve_defined_target(
                            module,
                            function.id,
                            instruction.id,
                            callable,
                            &signature,
                        )?,
                        argument_indices: abi.order.clone(),
                    },
                    None => {
                        let callee = instruction.callee.as_ref().ok_or(
                            CallPlanError::MissingCalleeName {
                                function: function.id,
                                instruction: instruction.id,
                            },
                        )?;
                        if let Some(existing) = signatures.get(callee) {
                            if existing != &signature {
                                return Err(CallPlanError::SignatureConflict {
                                    callee: callee.clone(),
                                    existing: existing.clone(),
                                    incoming: signature,
                                });
                            }
                        } else {
                            signatures.insert(callee.clone(), signature);
                        }
                        PendingCall::Runtime {
                            callee: callee.clone(),
                            argument_indices: abi.order.clone(),
                        }
                    }
                };
                pending.insert(site, call);
            }
        }
        if let Some(abi) = function
            .calls
            .iter()
            .find(|abi| !call_instructions.contains(&abi.instruction))
        {
            return Err(CallPlanError::OrphanAbi {
                function: function.id,
                instruction: abi.instruction,
            });
        }
    }

    let mut ids = BTreeMap::new();
    let mut declarations = Vec::with_capacity(signatures.len());
    let mut next = module
        .functions
        .iter()
        .map(|function| function.id.get())
        .max();
    for (name, signature) in signatures {
        let maximum = next
            .map(hir::FunctionId::new)
            .unwrap_or(hir::FunctionId::new(0));
        let id = maximum
            .get()
            .checked_add(1)
            .map(ir::FunctionId::new)
            .ok_or(CallPlanError::FunctionIdOverflow { maximum })?;
        next = Some(id.get());
        ids.insert(name.clone(), id);
        let parameters = signature
            .parameters
            .iter()
            .enumerate()
            .map(|(index, type_id)| {
                let raw = u32::try_from(index).map_err(|_| CallPlanError::ParameterIdOverflow {
                    callee: name.clone(),
                    count: signature.parameters.len(),
                })?;
                Ok(ir::Value {
                    id: ir::ValueId::new(raw),
                    type_id: *type_id,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        declarations.push(ir::Function {
            id,
            name,
            signature: ir::Signature {
                result: signature.result,
                parameters: signature.parameters,
                variadic: false,
                calling_convention: ir::CallingConvention::Runtime,
            },
            linkage: ir::Linkage::External,
            attributes: Vec::new(),
            parameters,
            blocks: Vec::new(),
        });
    }

    let mut sites = BTreeMap::new();
    for (site, pending) in pending {
        let (target, argument_indices) = match pending {
            PendingCall::Defined {
                target,
                argument_indices,
            } => (target, argument_indices),
            PendingCall::Runtime {
                callee,
                argument_indices,
            } => (
                ids.get(&callee)
                    .copied()
                    .ok_or(CallPlanError::MissingDeclaration { callee })?,
                argument_indices,
            ),
        };
        sites.insert(
            site,
            PlannedCall {
                target,
                argument_indices,
            },
        );
    }
    Ok(CallPlan {
        declarations,
        sites,
    })
}

enum PendingCall {
    Runtime {
        callee: String,
        argument_indices: Vec<usize>,
    },
    Defined {
        target: ir::FunctionId,
        argument_indices: Vec<usize>,
    },
}

fn unique_void_type(module: &hir::Module) -> Result<ir::TypeId, CallPlanError> {
    let mut voids = module
        .types
        .iter()
        .filter(|type_| type_.kind == hir::TypeKind::Void)
        .map(|type_| ir::TypeId::new(type_.id.get()));
    let Some(void) = voids.next() else {
        return Err(CallPlanError::MissingVoidType);
    };
    if voids.next().is_some() {
        return Err(CallPlanError::AmbiguousVoidType);
    }
    Ok(void)
}

fn matching_abi<'a>(
    function: &'a hir::Function,
    instruction: hir::InstructionId,
) -> Result<&'a hir::CallAbi, CallPlanError> {
    let mut matches = function
        .calls
        .iter()
        .filter(|abi| abi.instruction == instruction);
    let Some(abi) = matches.next() else {
        return Err(CallPlanError::MissingAbi {
            function: function.id,
            instruction,
        });
    };
    if matches.next().is_some() {
        return Err(CallPlanError::DuplicateAbi {
            function: function.id,
            instruction,
        });
    }
    Ok(abi)
}

fn validate_abi(
    function: hir::FunctionId,
    instruction: &hir::Instruction,
    abi: &hir::CallAbi,
) -> Result<(), CallPlanError> {
    if abi.distance != hir::CallDistance::Far {
        return Err(CallPlanError::UnsupportedDistance {
            function,
            instruction: instruction.id,
            distance: abi.distance,
        });
    }
    if abi.cleanup != hir::StackCleanup::Callee {
        return Err(CallPlanError::UnsupportedCleanup {
            function,
            instruction: instruction.id,
            cleanup: abi.cleanup,
        });
    }
    if instruction.results.len() > 1 {
        return Err(CallPlanError::ResultArity {
            function,
            instruction: instruction.id,
            count: instruction.results.len(),
        });
    }

    if abi.order.len() != instruction.operands.len() {
        return Err(CallPlanError::MalformedOrder {
            function,
            instruction: instruction.id,
            issue: AbiOrderError::WrongLength,
        });
    }
    let mut seen = BTreeSet::new();
    for index in &abi.order {
        if *index >= instruction.operands.len() {
            return Err(CallPlanError::MalformedOrder {
                function,
                instruction: instruction.id,
                issue: AbiOrderError::OutOfBounds { index: *index },
            });
        }
        if !seen.insert(*index) {
            return Err(CallPlanError::MalformedOrder {
                function,
                instruction: instruction.id,
                issue: AbiOrderError::Duplicate { index: *index },
            });
        }
    }
    Ok(())
}

fn resolve_defined_target(
    module: &hir::Module,
    function: hir::FunctionId,
    instruction: hir::InstructionId,
    callable_id: hir::CallableId,
    signature: &CallSignature,
) -> Result<ir::FunctionId, CallPlanError> {
    let callable = unique_callable(module, function, instruction, callable_id)?;
    if !callable.defined {
        return Err(CallPlanError::UndefinedCallable {
            function,
            instruction,
            callable: callable_id,
        });
    }
    validate_callable_parameters(function, instruction, callable)?;
    validate_callable_signature(module, function, instruction, callable, signature)?;

    let name = normalized_name(&callable.name);
    let named = module
        .functions
        .iter()
        .filter(|candidate| normalized_name(&candidate.name) == name)
        .collect::<Vec<_>>();
    if named.is_empty() {
        return Err(CallPlanError::MissingDefinedFunction {
            function,
            instruction,
            callable: callable_id,
            name,
        });
    }

    let mut actual = Vec::with_capacity(named.len());
    let mut matches = Vec::new();
    for candidate in named {
        let candidate_signature = function_signature(function, instruction, candidate)?;
        if candidate_signature == *signature {
            matches.push(candidate);
        } else {
            actual.push(candidate_signature);
        }
    }
    if matches.is_empty() {
        return Err(CallPlanError::DefinedFunctionSignatureMismatch {
            function,
            instruction,
            callable: callable_id,
            name,
            expected: signature.clone(),
            actual,
        });
    }
    if matches.len() != 1 {
        return Err(CallPlanError::AmbiguousDefinedFunction {
            function,
            instruction,
            callable: callable_id,
            name,
        });
    }
    let target = matches[0];
    if target.abi.distance != hir::CallDistance::Far {
        return Err(CallPlanError::DefinedFunctionDistance {
            function,
            instruction,
            target: target.id,
            distance: target.abi.distance,
        });
    }
    if target.abi.cleanup != hir::StackCleanup::Callee {
        return Err(CallPlanError::DefinedFunctionCleanup {
            function,
            instruction,
            target: target.id,
            cleanup: target.abi.cleanup,
        });
    }
    Ok(ir::FunctionId::new(target.id.get()))
}

fn unique_callable<'a>(
    module: &'a hir::Module,
    function: hir::FunctionId,
    instruction: hir::InstructionId,
    callable_id: hir::CallableId,
) -> Result<&'a hir::Callable, CallPlanError> {
    let mut matches = module
        .callables
        .iter()
        .filter(|callable| callable.id == callable_id);
    let Some(callable) = matches.next() else {
        return Err(CallPlanError::MissingCallable {
            function,
            instruction,
            callable: callable_id,
        });
    };
    if matches.next().is_some() {
        return Err(CallPlanError::AmbiguousCallable {
            function,
            instruction,
            callable: callable_id,
        });
    }
    Ok(callable)
}

fn validate_callable_parameters(
    function: hir::FunctionId,
    instruction: hir::InstructionId,
    callable: &hir::Callable,
) -> Result<(), CallPlanError> {
    for (index, parameter) in callable.parameters.iter().enumerate() {
        let issue = if parameter.array {
            Some(CallableParameterError::Array)
        } else if parameter.segmented {
            Some(CallableParameterError::Segmented)
        } else {
            None
        };
        if let Some(issue) = issue {
            return Err(CallPlanError::UnsupportedCallableParameter {
                function,
                instruction,
                callable: callable.id,
                parameter: index,
                issue,
            });
        }
    }
    Ok(())
}

fn validate_callable_signature(
    module: &hir::Module,
    function: hir::FunctionId,
    instruction: hir::InstructionId,
    callable: &hir::Callable,
    signature: &CallSignature,
) -> Result<(), CallPlanError> {
    let expected_result = callable
        .result_type
        .map(|type_id| ir::TypeId::new(type_id.get()))
        .unwrap_or(unique_void_type(module)?);
    if signature.result != expected_result {
        return Err(CallPlanError::CallableResultMismatch {
            function,
            instruction,
            callable: callable.id,
            expected: expected_result,
            actual: signature.result,
        });
    }
    if callable.parameters.len() != signature.parameters.len() {
        return Err(CallPlanError::CallableParameterCount {
            function,
            instruction,
            callable: callable.id,
            expected: callable.parameters.len(),
            actual: signature.parameters.len(),
        });
    }
    for (index, (parameter, actual)) in callable
        .parameters
        .iter()
        .zip(&signature.parameters)
        .enumerate()
    {
        if parameter.by_value {
            let expected = ir::TypeId::new(parameter.type_id.get());
            if *actual != expected {
                return Err(CallPlanError::CallableByValueParameterMismatch {
                    function,
                    instruction,
                    callable: callable.id,
                    parameter: index,
                    expected,
                    actual: *actual,
                });
            }
        } else if !is_pointer_to(module, *actual, parameter.type_id) {
            return Err(CallPlanError::CallableByReferenceParameterMismatch {
                function,
                instruction,
                callable: callable.id,
                parameter: index,
                expected: parameter.type_id,
                actual: *actual,
            });
        }
    }
    Ok(())
}

fn is_pointer_to(module: &hir::Module, actual: ir::TypeId, expected: hir::TypeId) -> bool {
    let types = module
        .types
        .iter()
        .filter(|type_| type_.id.get() == actual.get())
        .collect::<Vec<_>>();
    matches!(
        types.as_slice(),
        [type_] if type_.kind == hir::TypeKind::Pointer && type_.element == Some(expected)
    )
}

fn function_signature(
    source_function: hir::FunctionId,
    instruction: hir::InstructionId,
    function: &hir::Function,
) -> Result<CallSignature, CallPlanError> {
    let values = ValueTypes::new(function);
    let parameters = function
        .parameters
        .iter()
        .map(|value| {
            values
                .get(source_function, instruction, *value)
                .map(|type_id| ir::TypeId::new(type_id.get()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CallSignature {
        result: ir::TypeId::new(function.result_type.get()),
        parameters,
    })
}

fn normalized_name(name: &str) -> String {
    name.trim()
        .trim_end_matches(['%', '&', '!', '#', '$'])
        .to_ascii_uppercase()
}

fn infer_signature(
    module: &hir::Module,
    function: hir::FunctionId,
    instruction: &hir::Instruction,
    values: &ValueTypes,
    abi: &hir::CallAbi,
    void_type: &mut Option<ir::TypeId>,
) -> Result<CallSignature, CallPlanError> {
    let result = match instruction.results.as_slice() {
        [] => match *void_type {
            Some(type_id) => type_id,
            None => {
                let type_id = unique_void_type(module)?;
                *void_type = Some(type_id);
                type_id
            }
        },
        [result] => ir::TypeId::new(values.get(function, instruction.id, *result)?.get()),
        _ => {
            return Err(CallPlanError::ResultArity {
                function,
                instruction: instruction.id,
                count: instruction.results.len(),
            });
        }
    };
    let mut source_types = Vec::with_capacity(instruction.operands.len());
    for (index, operand) in instruction.operands.iter().enumerate() {
        let type_id = match operand {
            hir::Operand::Value(value) => values.get(function, instruction.id, *value)?,
            hir::Operand::Constant { type_id, .. } => *type_id,
            hir::Operand::Place(_) => {
                return Err(unsupported_operand(
                    function,
                    instruction.id,
                    index,
                    CallOperandError::Place,
                ));
            }
            hir::Operand::Element { .. } => {
                return Err(unsupported_operand(
                    function,
                    instruction.id,
                    index,
                    CallOperandError::Element,
                ));
            }
            hir::Operand::Projection { .. } => {
                return Err(unsupported_operand(
                    function,
                    instruction.id,
                    index,
                    CallOperandError::Projection,
                ));
            }
            hir::Operand::Indirect { .. } => {
                return Err(unsupported_operand(
                    function,
                    instruction.id,
                    index,
                    CallOperandError::Indirect,
                ));
            }
        };
        source_types.push(ir::TypeId::new(type_id.get()));
    }
    let mut parameters = Vec::with_capacity(abi.order.len());
    for index in &abi.order {
        let type_id = source_types
            .get(*index)
            .copied()
            .ok_or(CallPlanError::MalformedOrder {
                function,
                instruction: instruction.id,
                issue: AbiOrderError::OutOfBounds { index: *index },
            })?;
        parameters.push(type_id);
    }
    Ok(CallSignature { result, parameters })
}

fn unsupported_operand(
    function: hir::FunctionId,
    instruction: hir::InstructionId,
    index: usize,
    operand: CallOperandError,
) -> CallPlanError {
    CallPlanError::UnsupportedOperand {
        function,
        instruction,
        index,
        operand,
    }
}

struct ValueTypes {
    types: BTreeMap<hir::ValueId, hir::TypeId>,
    duplicates: BTreeSet<hir::ValueId>,
}

impl ValueTypes {
    fn new(function: &hir::Function) -> Self {
        let mut types = BTreeMap::new();
        let mut duplicates = BTreeSet::new();
        for value in &function.values {
            if types.insert(value.id, value.type_id).is_some() {
                duplicates.insert(value.id);
            }
        }
        Self { types, duplicates }
    }

    fn get(
        &self,
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        value: hir::ValueId,
    ) -> Result<hir::TypeId, CallPlanError> {
        if self.duplicates.contains(&value) {
            return Err(CallPlanError::AmbiguousValue {
                function,
                instruction,
                value,
            });
        }
        self.types
            .get(&value)
            .copied()
            .ok_or(CallPlanError::UnknownValue {
                function,
                instruction,
                value,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOID: hir::TypeId = hir::TypeId::new(0);
    const I16: hir::TypeId = hir::TypeId::new(1);
    const I32: hir::TypeId = hir::TypeId::new(2);

    fn type_(id: hir::TypeId, kind: hir::TypeKind) -> hir::Type {
        hir::Type {
            id,
            name: kind.as_str().into(),
            kind,
            width: 0,
            signed: None,
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        }
    }

    fn pointer_type(id: hir::TypeId, element: hir::TypeId) -> hir::Type {
        hir::Type {
            id,
            name: format!("near*{element}"),
            kind: hir::TypeKind::Pointer,
            width: 2,
            signed: None,
            evaluation: hir::FloatEvaluation::None,
            element: Some(element),
            bounds: Vec::new(),
            address: hir::AddressKind::Near,
        }
    }

    fn call(
        id: u32,
        callee: &str,
        results: Vec<u32>,
        operands: Vec<hir::Operand>,
    ) -> hir::Instruction {
        hir::Instruction {
            id: hir::InstructionId::new(id),
            opcode: hir::Opcode::Call,
            results: results.into_iter().map(hir::ValueId::new).collect(),
            operands,
            callee: Some(callee.into()),
        }
    }

    fn abi(id: u32, order: Vec<usize>) -> hir::CallAbi {
        hir::CallAbi {
            instruction: hir::InstructionId::new(id),
            order,
            cleanup: hir::StackCleanup::Callee,
            distance: hir::CallDistance::Far,
            callee: None,
        }
    }

    fn direct_abi(id: u32, order: Vec<usize>, callable: u32) -> hir::CallAbi {
        let mut abi = abi(id, order);
        abi.callee = Some(hir::CallableId::new(callable));
        abi
    }

    fn callable(
        id: u32,
        name: &str,
        result_type: Option<hir::TypeId>,
        parameters: Vec<hir::Parameter>,
    ) -> hir::Callable {
        hir::Callable {
            id: hir::CallableId::new(id),
            name: name.into(),
            result_type,
            parameters,
            defined: true,
        }
    }

    fn parameter(type_id: hir::TypeId) -> hir::Parameter {
        hir::Parameter {
            type_id,
            by_value: true,
            segmented: false,
            array: false,
        }
    }

    fn by_reference_parameter(type_id: hir::TypeId) -> hir::Parameter {
        hir::Parameter {
            by_value: false,
            ..parameter(type_id)
        }
    }

    fn defined_function(
        id: u32,
        name: &str,
        result_type: hir::TypeId,
        parameters: Vec<hir::TypeId>,
    ) -> hir::Function {
        let values = parameters
            .iter()
            .enumerate()
            .map(|(index, type_id)| hir::Value {
                id: hir::ValueId::new(u32::try_from(index).unwrap()),
                type_id: *type_id,
            })
            .collect::<Vec<_>>();
        hir::Function {
            id: hir::FunctionId::new(id),
            name: name.into(),
            result_type,
            values,
            places: Vec::new(),
            blocks: vec![hir::Block {
                id: hir::BlockId::new(0),
                instructions: Vec::new(),
                terminator: hir::Terminator::Return(None),
            }],
            entry: hir::BlockId::new(0),
            parameters: (0..parameters.len())
                .map(|index| hir::ValueId::new(u32::try_from(index).unwrap()))
                .collect(),
            abi: hir::ProcedureAbi {
                cleanup: hir::StackCleanup::Callee,
                distance: hir::CallDistance::Far,
                parameter_bytes: 0,
            },
            calls: Vec::new(),
            error_handler: None,
            error_handler_local: false,
            external_entries: Vec::new(),
            linkage: hir::Linkage::External,
        }
    }

    fn module(
        values: Vec<hir::Value>,
        instructions: Vec<hir::Instruction>,
        calls: Vec<hir::CallAbi>,
    ) -> hir::Module {
        hir::Module {
            id: hir::ModuleId::new(0),
            name: "calls".into(),
            types: vec![
                type_(VOID, hir::TypeKind::Void),
                type_(I16, hir::TypeKind::Integer),
                type_(I32, hir::TypeKind::Integer),
            ],
            functions: vec![hir::Function {
                id: hir::FunctionId::new(0),
                name: "main".into(),
                result_type: VOID,
                values,
                places: Vec::new(),
                blocks: vec![hir::Block {
                    id: hir::BlockId::new(0),
                    instructions,
                    terminator: hir::Terminator::Return(None),
                }],
                entry: hir::BlockId::new(0),
                parameters: Vec::new(),
                abi: hir::ProcedureAbi {
                    cleanup: hir::StackCleanup::Callee,
                    distance: hir::CallDistance::Far,
                    parameter_bytes: 0,
                },
                calls,
                error_handler: None,
                error_handler_local: false,
                external_entries: Vec::new(),
                linkage: hir::Linkage::Internal,
            }],
            data: Vec::new(),
            callables: Vec::new(),
        }
    }

    fn value(id: u32, type_id: hir::TypeId) -> hir::Value {
        hir::Value {
            id: hir::ValueId::new(id),
            type_id,
        }
    }

    #[test]
    fn plans_a_no_argument_runtime_declaration() {
        let module = module(
            Vec::new(),
            vec![call(0, "B$RT", Vec::new(), Vec::new())],
            vec![abi(0, Vec::new())],
        );

        let plan = plan_calls(&module).unwrap();

        assert_eq!(plan.declarations.len(), 1);
        assert_eq!(plan.declarations[0].id, ir::FunctionId::new(1));
        assert_eq!(plan.declarations[0].name, "B$RT");
        assert_eq!(plan.declarations[0].signature.result, ir::TypeId::new(0));
        assert!(plan.declarations[0].signature.parameters.is_empty());
        assert_eq!(
            plan.declarations[0].signature.calling_convention,
            ir::CallingConvention::Runtime
        );
        assert_eq!(plan.declarations[0].linkage, ir::Linkage::External);
    }

    #[test]
    fn preserves_the_abi_argument_permutation() {
        let module = module(
            vec![value(0, I16), value(1, I32)],
            vec![call(
                0,
                "B$ORDER",
                Vec::new(),
                vec![
                    hir::Operand::Value(hir::ValueId::new(0)),
                    hir::Operand::Value(hir::ValueId::new(1)),
                ],
            )],
            vec![abi(0, vec![1, 0])],
        );

        let plan = plan_calls(&module).unwrap();

        assert_eq!(
            plan.declarations[0].signature.parameters,
            vec![ir::TypeId::new(2), ir::TypeId::new(1)]
        );
        assert_eq!(
            plan.declarations[0].parameters,
            vec![
                ir::Value {
                    id: ir::ValueId::new(0),
                    type_id: ir::TypeId::new(2),
                },
                ir::Value {
                    id: ir::ValueId::new(1),
                    type_id: ir::TypeId::new(1),
                },
            ]
        );
        assert_eq!(
            plan.sites[&(hir::FunctionId::new(0), hir::InstructionId::new(0))].argument_indices,
            vec![1, 0]
        );
    }

    #[test]
    fn rejects_conflicting_runtime_signatures() {
        let module = module(
            vec![value(0, I16), value(1, I32)],
            vec![
                call(
                    0,
                    "B$SAME",
                    Vec::new(),
                    vec![hir::Operand::Value(hir::ValueId::new(0))],
                ),
                call(
                    1,
                    "B$SAME",
                    Vec::new(),
                    vec![hir::Operand::Value(hir::ValueId::new(1))],
                ),
            ],
            vec![abi(0, vec![0]), abi(1, vec![0])],
        );

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::SignatureConflict { .. })
        ));
    }

    #[test]
    fn rejects_a_malformed_abi_order() {
        let module = module(
            Vec::new(),
            vec![call(
                0,
                "B$ORDER",
                Vec::new(),
                vec![hir::Operand::Constant {
                    type_id: I16,
                    value: hir::ConstantValue::Integer(1),
                }],
            )],
            vec![abi(0, vec![1])],
        );

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::MalformedOrder {
                issue: AbiOrderError::OutOfBounds { .. },
                ..
            })
        ));
    }

    #[test]
    fn rejects_caller_cleanup() {
        let mut call_abi = abi(0, Vec::new());
        call_abi.cleanup = hir::StackCleanup::Caller;
        let module = module(
            Vec::new(),
            vec![call(0, "B$RT", Vec::new(), Vec::new())],
            vec![call_abi],
        );

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::UnsupportedCleanup {
                cleanup: hir::StackCleanup::Caller,
                ..
            })
        ));
    }

    #[test]
    fn resolves_a_defined_callable_without_a_declaration() {
        let mut module = module(
            vec![value(0, I16), value(1, I32)],
            vec![call(
                0,
                "worker",
                Vec::new(),
                vec![
                    hir::Operand::Value(hir::ValueId::new(0)),
                    hir::Operand::Value(hir::ValueId::new(1)),
                ],
            )],
            vec![direct_abi(0, vec![1, 0], 7)],
        );
        module.callables.push(callable(
            7,
            "Worker%",
            None,
            vec![parameter(I32), parameter(I16)],
        ));
        module
            .functions
            .push(defined_function(3, "WORKER", VOID, vec![I32, I16]));

        let plan = plan_calls(&module).unwrap();

        assert!(plan.declarations.is_empty());
        assert_eq!(
            plan.sites[&(hir::FunctionId::new(0), hir::InstructionId::new(0))],
            PlannedCall {
                target: ir::FunctionId::new(3),
                argument_indices: vec![1, 0],
            }
        );
    }

    #[test]
    fn rejects_a_missing_direct_callable() {
        let module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::MissingCallable { callable, .. }) if callable == hir::CallableId::new(7)
        ));
    }

    #[test]
    fn rejects_an_ambiguous_direct_callable() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        module.callables = vec![
            callable(7, "worker", None, Vec::new()),
            callable(7, "worker", None, Vec::new()),
        ];

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::AmbiguousCallable { callable, .. }) if callable == hir::CallableId::new(7)
        ));
    }

    #[test]
    fn rejects_a_missing_defined_target() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        module
            .callables
            .push(callable(7, "worker", None, Vec::new()));

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::MissingDefinedFunction { callable, .. }) if callable == hir::CallableId::new(7)
        ));
    }

    #[test]
    fn rejects_an_ambiguous_defined_target() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        module
            .callables
            .push(callable(7, "worker", None, Vec::new()));
        module
            .functions
            .push(defined_function(1, "worker", VOID, Vec::new()));
        module
            .functions
            .push(defined_function(2, "WORKER", VOID, Vec::new()));

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::AmbiguousDefinedFunction { callable, .. }) if callable == hir::CallableId::new(7)
        ));
    }

    #[test]
    fn rejects_a_mismatched_defined_target_signature() {
        let mut module = module(
            vec![value(0, I16)],
            vec![call(
                0,
                "worker",
                Vec::new(),
                vec![hir::Operand::Value(hir::ValueId::new(0))],
            )],
            vec![direct_abi(0, vec![0], 7)],
        );
        module
            .callables
            .push(callable(7, "worker", None, vec![parameter(I16)]));
        module
            .functions
            .push(defined_function(1, "worker", VOID, vec![I32]));

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::DefinedFunctionSignatureMismatch { callable, .. }) if callable == hir::CallableId::new(7)
        ));
    }

    #[test]
    fn rejects_an_array_callable_parameter() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        let mut array = parameter(I16);
        array.array = true;
        module
            .callables
            .push(callable(7, "worker", None, vec![array]));

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::UnsupportedCallableParameter {
                issue: CallableParameterError::Array,
                ..
            })
        ));
    }

    #[test]
    fn accepts_a_qb_style_by_reference_parameter() {
        let pointer = hir::TypeId::new(3);
        let mut module = module(
            vec![value(0, pointer)],
            vec![call(
                0,
                "worker",
                Vec::new(),
                vec![hir::Operand::Value(hir::ValueId::new(0))],
            )],
            vec![direct_abi(0, vec![0], 7)],
        );
        module.types.push(pointer_type(pointer, I16));
        module.callables.push(callable(
            7,
            "worker",
            None,
            vec![by_reference_parameter(I16)],
        ));
        module
            .functions
            .push(defined_function(1, "worker", VOID, vec![pointer]));

        let plan = plan_calls(&module).unwrap();

        assert!(plan.declarations.is_empty());
        assert_eq!(
            plan.sites[&(hir::FunctionId::new(0), hir::InstructionId::new(0))].target,
            ir::FunctionId::new(1)
        );
    }

    #[test]
    fn rejects_a_callable_by_value_signature_mismatch() {
        let mut module = module(
            vec![value(0, I16)],
            vec![call(
                0,
                "worker",
                Vec::new(),
                vec![hir::Operand::Value(hir::ValueId::new(0))],
            )],
            vec![direct_abi(0, vec![0], 7)],
        );
        module
            .callables
            .push(callable(7, "worker", None, vec![parameter(I32)]));

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::CallableByValueParameterMismatch {
                callable,
                parameter: 0,
                expected,
                actual,
                ..
            }) if callable == hir::CallableId::new(7)
                && expected == ir::TypeId::new(I32.get())
                && actual == ir::TypeId::new(I16.get())
        ));
    }

    #[test]
    fn rejects_an_undefined_callable() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        let mut callable = callable(7, "worker", None, Vec::new());
        callable.defined = false;
        module.callables.push(callable);

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::UndefinedCallable { callable, .. }) if callable == hir::CallableId::new(7)
        ));
    }

    #[test]
    fn rejects_a_near_defined_target() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        module
            .callables
            .push(callable(7, "worker", None, Vec::new()));
        let mut target = defined_function(1, "worker", VOID, Vec::new());
        target.abi.distance = hir::CallDistance::Near;
        module.functions.push(target);

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::DefinedFunctionDistance {
                distance: hir::CallDistance::Near,
                ..
            })
        ));
    }

    #[test]
    fn rejects_a_caller_cleanup_defined_target() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        module
            .callables
            .push(callable(7, "worker", None, Vec::new()));
        let mut target = defined_function(1, "worker", VOID, Vec::new());
        target.abi.cleanup = hir::StackCleanup::Caller;
        module.functions.push(target);

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::DefinedFunctionCleanup {
                cleanup: hir::StackCleanup::Caller,
                ..
            })
        ));
    }

    #[test]
    fn rejects_abi_metadata_attached_to_a_non_call() {
        let mut module = module(Vec::new(), Vec::new(), vec![abi(0, Vec::new())]);
        module.functions[0].blocks[0]
            .instructions
            .push(hir::Instruction {
                id: hir::InstructionId::new(0),
                opcode: hir::Opcode::Copy,
                results: Vec::new(),
                operands: Vec::new(),
                callee: None,
            });

        assert_eq!(
            plan_calls(&module),
            Err(CallPlanError::OrphanAbi {
                function: hir::FunctionId::new(0),
                instruction: hir::InstructionId::new(0),
            })
        );
    }
}
