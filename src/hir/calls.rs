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

/// The complete shape of a named external declaration inferred from a call.
///
/// Calling convention is kept separate from [`CallSignature`] because direct
/// calls first identify a target by its source-level type signature, then
/// validate the site and target ABI independently.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeSignature {
    signature: CallSignature,
    calling_convention: ir::CallingConvention,
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
    UnsupportedAbi {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        distance: hir::CallDistance,
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
    CallingConventionConflict {
        callee: String,
        existing: ir::CallingConvention,
        incoming: ir::CallingConvention,
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
    DefinedFunctionAbiMismatch {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        target: hir::FunctionId,
        call_distance: hir::CallDistance,
        call_cleanup: hir::StackCleanup,
        target_distance: hir::CallDistance,
        target_cleanup: hir::StackCleanup,
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
            Self::UnsupportedAbi {
                function,
                instruction,
                distance,
                cleanup,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} has unsupported ABI {distance:?} with {cleanup:?} cleanup"
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
            Self::CallingConventionConflict { callee, .. } => write!(
                formatter,
                "runtime callee {callee:?} has incompatible inferred calling conventions"
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
            Self::DefinedFunctionAbiMismatch {
                function,
                instruction,
                target,
                call_distance,
                call_cleanup,
                target_distance,
                target_cleanup,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} ABI {call_distance:?} with {call_cleanup:?} cleanup does not match target {target} ABI {target_distance:?} with {target_cleanup:?} cleanup"
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
    let mut signatures = BTreeMap::<String, RuntimeSignature>::new();
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
                let calling_convention = validate_abi(function.id, instruction, abi)?;
                let signature =
                    infer_signature(module, function, instruction, &values, abi, &mut void_type)?;
                let call = match abi.callee {
                    Some(callable) => PendingCall::Defined {
                        target: resolve_defined_target(
                            module,
                            function.id,
                            instruction.id,
                            callable,
                            &signature,
                            abi,
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
                        let runtime_signature = RuntimeSignature {
                            signature,
                            calling_convention,
                        };
                        if let Some(existing) = signatures.get(callee) {
                            if existing.signature != runtime_signature.signature {
                                return Err(CallPlanError::SignatureConflict {
                                    callee: callee.clone(),
                                    existing: existing.signature.clone(),
                                    incoming: runtime_signature.signature,
                                });
                            }
                            if existing.calling_convention != runtime_signature.calling_convention {
                                return Err(CallPlanError::CallingConventionConflict {
                                    callee: callee.clone(),
                                    existing: existing.calling_convention,
                                    incoming: runtime_signature.calling_convention,
                                });
                            }
                        } else {
                            signatures.insert(callee.clone(), runtime_signature);
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
    for (name, runtime_signature) in signatures {
        let signature = runtime_signature.signature;
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
                calling_convention: runtime_signature.calling_convention,
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

pub(super) fn calling_convention(
    distance: hir::CallDistance,
    cleanup: hir::StackCleanup,
) -> Option<ir::CallingConvention> {
    match (distance, cleanup) {
        (hir::CallDistance::Near, hir::StackCleanup::Caller) => Some(ir::CallingConvention::C),
        (hir::CallDistance::Far, hir::StackCleanup::Caller) => {
            Some(ir::CallingConvention::FarCdecl)
        }
        (hir::CallDistance::Far, hir::StackCleanup::Callee) => {
            Some(ir::CallingConvention::FarPascal)
        }
        (hir::CallDistance::Near, hir::StackCleanup::Callee) => None,
    }
}

fn validate_abi(
    function: hir::FunctionId,
    instruction: &hir::Instruction,
    abi: &hir::CallAbi,
) -> Result<ir::CallingConvention, CallPlanError> {
    let calling_convention =
        calling_convention(abi.distance, abi.cleanup).ok_or(CallPlanError::UnsupportedAbi {
            function,
            instruction: instruction.id,
            distance: abi.distance,
            cleanup: abi.cleanup,
        })?;
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
    Ok(calling_convention)
}

fn resolve_defined_target(
    module: &hir::Module,
    function: hir::FunctionId,
    instruction: hir::InstructionId,
    callable_id: hir::CallableId,
    signature: &CallSignature,
    call_abi: &hir::CallAbi,
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
    if calling_convention(target.abi.distance, target.abi.cleanup)
        != calling_convention(call_abi.distance, call_abi.cleanup)
    {
        return Err(CallPlanError::DefinedFunctionAbiMismatch {
            function,
            instruction,
            target: target.id,
            call_distance: call_abi.distance,
            call_cleanup: call_abi.cleanup,
            target_distance: target.abi.distance,
            target_cleanup: target.abi.cleanup,
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
    function: &hir::Function,
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
        [result] => ir::TypeId::new(values.get(function.id, instruction.id, *result)?.get()),
        _ => {
            return Err(CallPlanError::ResultArity {
                function: function.id,
                instruction: instruction.id,
                count: instruction.results.len(),
            });
        }
    };
    let mut source_types = Vec::with_capacity(instruction.operands.len());
    for (index, operand) in instruction.operands.iter().enumerate() {
        let type_id = match operand {
            hir::Operand::Value(value) => values.get(function.id, instruction.id, *value)?,
            hir::Operand::Constant { type_id, .. } => *type_id,
            // A frontend may deliberately materialize a floating BYVAL
            // argument in declared-width storage before the call.  Preserve
            // that explicit rounding boundary: the call signature sees the
            // place's storage type, and lowering emits a direct load rather
            // than extending and immediately truncating the value again.
            hir::Operand::Place(place) => {
                float_place_type(module, function, *place).ok_or_else(|| {
                    unsupported_operand(function.id, instruction.id, index, CallOperandError::Place)
                })?
            }
            hir::Operand::Element { .. } => {
                return Err(unsupported_operand(
                    function.id,
                    instruction.id,
                    index,
                    CallOperandError::Element,
                ));
            }
            hir::Operand::Projection { type_id, .. } if is_float_type(module, *type_id) => *type_id,
            hir::Operand::Projection { .. } => {
                return Err(unsupported_operand(
                    function.id,
                    instruction.id,
                    index,
                    CallOperandError::Projection,
                ));
            }
            hir::Operand::Indirect { .. } => {
                return Err(unsupported_operand(
                    function.id,
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
                function: function.id,
                instruction: instruction.id,
                issue: AbiOrderError::OutOfBounds { index: *index },
            })?;
        parameters.push(type_id);
    }
    Ok(CallSignature { result, parameters })
}

fn is_float_type(module: &hir::Module, type_id: hir::TypeId) -> bool {
    let mut matches = module.types.iter().filter(|type_| type_.id == type_id);
    matches
        .next()
        .is_some_and(|type_| type_.kind == hir::TypeKind::Float)
        && matches.next().is_none()
}

fn float_place_type(
    module: &hir::Module,
    function: &hir::Function,
    place_id: hir::PlaceId,
) -> Option<hir::TypeId> {
    let mut matches = function.places.iter().filter(|place| place.id == place_id);
    let type_id = matches.next()?.type_id;
    (matches.next().is_none() && is_float_type(module, type_id)).then_some(type_id)
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
            ir::CallingConvention::FarPascal
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
    fn plans_an_explicit_float_storage_place_as_a_by_value_argument() {
        let f32 = hir::TypeId::new(3);
        let mut module = module(
            Vec::new(),
            vec![call(
                0,
                "B$FLOAT",
                Vec::new(),
                vec![hir::Operand::Place(hir::PlaceId::new(0))],
            )],
            vec![abi(0, vec![0])],
        );
        module.types.push(hir::Type {
            id: f32,
            name: "single".into(),
            kind: hir::TypeKind::Float,
            width: 4,
            signed: None,
            evaluation: hir::FloatEvaluation::Extended80,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        });
        module.functions[0].places.push(hir::Place {
            id: hir::PlaceId::new(0),
            name: "rounded".into(),
            type_id: f32,
            storage: hir::Storage::Local,
            offset: 0,
            symbol: hir::DataId::new(0),
            extent: 4,
            address: hir::AddressKind::Near,
        });

        let plan = plan_calls(&module).unwrap();

        assert_eq!(
            plan.declarations[0].signature.parameters,
            vec![ir::TypeId::new(3)]
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
    fn rejects_conflicting_runtime_calling_conventions() {
        let mut second = abi(1, vec![0]);
        second.cleanup = hir::StackCleanup::Caller;
        let module = module(
            vec![value(0, I16)],
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
                    vec![hir::Operand::Value(hir::ValueId::new(0))],
                ),
            ],
            vec![abi(0, vec![0]), second],
        );

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::CallingConventionConflict {
                existing: ir::CallingConvention::FarPascal,
                incoming: ir::CallingConvention::FarCdecl,
                ..
            })
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
    fn plans_a_far_cdecl_abi_runtime_declaration() {
        let mut call_abi = abi(0, Vec::new());
        call_abi.cleanup = hir::StackCleanup::Caller;
        let module = module(
            Vec::new(),
            vec![call(0, "B$RT", Vec::new(), Vec::new())],
            vec![call_abi],
        );

        let plan = plan_calls(&module).expect("far caller-cleanup call is representable");

        assert_eq!(
            plan.declarations[0].signature.calling_convention,
            ir::CallingConvention::FarCdecl
        );
    }

    #[test]
    fn rejects_near_callee_cleanup_at_a_call_site() {
        let mut call_abi = abi(0, Vec::new());
        call_abi.distance = hir::CallDistance::Near;
        let module = module(
            Vec::new(),
            vec![call(0, "B$RT", Vec::new(), Vec::new())],
            vec![call_abi],
        );

        assert!(matches!(
            plan_calls(&module),
            Err(CallPlanError::UnsupportedAbi {
                distance: hir::CallDistance::Near,
                cleanup: hir::StackCleanup::Callee,
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
    fn rejects_a_defined_target_with_a_different_abi_distance() {
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
            Err(CallPlanError::DefinedFunctionAbiMismatch {
                call_distance: hir::CallDistance::Far,
                call_cleanup: hir::StackCleanup::Callee,
                target_distance: hir::CallDistance::Near,
                target_cleanup: hir::StackCleanup::Callee,
                ..
            })
        ));
    }

    #[test]
    fn rejects_a_defined_target_with_a_different_abi_cleanup() {
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
            Err(CallPlanError::DefinedFunctionAbiMismatch {
                call_distance: hir::CallDistance::Far,
                call_cleanup: hir::StackCleanup::Callee,
                target_distance: hir::CallDistance::Far,
                target_cleanup: hir::StackCleanup::Caller,
                ..
            })
        ));
    }

    #[test]
    fn resolves_a_near_caller_cleanup_abi_defined_target() {
        let mut module = module(
            Vec::new(),
            vec![call(0, "worker", Vec::new(), Vec::new())],
            vec![direct_abi(0, Vec::new(), 7)],
        );
        module.functions[0].calls[0].distance = hir::CallDistance::Near;
        module.functions[0].calls[0].cleanup = hir::StackCleanup::Caller;
        module
            .callables
            .push(callable(7, "worker", None, Vec::new()));
        let mut target = defined_function(1, "worker", VOID, Vec::new());
        target.abi.distance = hir::CallDistance::Near;
        target.abi.cleanup = hir::StackCleanup::Caller;
        module.functions.push(target);

        let plan = plan_calls(&module).expect("matching near caller-cleanup target resolves");

        assert_eq!(
            plan.sites[&(hir::FunctionId::new(0), hir::InstructionId::new(0))].target,
            ir::FunctionId::new(1)
        );
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
