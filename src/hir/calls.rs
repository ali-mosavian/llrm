//! Planning of exact runtime-call declarations for HIR lowering.
//!
//! This module validates only the narrow runtime ABI shape that portable IR
//! can represent today.  It does not lower operands, choose effects, or infer
//! any semantics from a routine name.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{hir, ir};

/// Runtime-call declarations and per-site lowering information.
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

/// A type signature inferred from one runtime call site.
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

/// An unsupported operand form at a runtime call site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallOperandError {
    Place,
    Element,
    Projection,
    Indirect,
}

/// A refusal raised while constructing a runtime-call plan.
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
    UnsupportedCallable {
        function: hir::FunctionId,
        instruction: hir::InstructionId,
        callable: hir::CallableId,
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
            Self::UnsupportedCallable {
                function,
                instruction,
                callable,
            } => write!(
                formatter,
                "function {function} call instruction {instruction} targets defined callable {callable}"
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

/// Builds external runtime declarations and source-call lowering metadata.
pub(super) fn plan_runtime_calls(module: &hir::Module) -> Result<CallPlan, CallPlanError> {
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
                let callee =
                    instruction
                        .callee
                        .as_ref()
                        .ok_or(CallPlanError::MissingCalleeName {
                            function: function.id,
                            instruction: instruction.id,
                        })?;
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
                pending.insert(
                    site,
                    PendingCall {
                        callee: callee.clone(),
                        argument_indices: abi.order.clone(),
                    },
                );
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
        let target =
            ids.get(&pending.callee)
                .copied()
                .ok_or_else(|| CallPlanError::MissingDeclaration {
                    callee: pending.callee.clone(),
                })?;
        sites.insert(
            site,
            PlannedCall {
                target,
                argument_indices: pending.argument_indices,
            },
        );
    }
    Ok(CallPlan {
        declarations,
        sites,
    })
}

struct PendingCall {
    callee: String,
    argument_indices: Vec<usize>,
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
    if let Some(callable) = abi.callee {
        return Err(CallPlanError::UnsupportedCallable {
            function,
            instruction: instruction.id,
            callable,
        });
    }
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

        let plan = plan_runtime_calls(&module).unwrap();

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

        let plan = plan_runtime_calls(&module).unwrap();

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
            plan_runtime_calls(&module),
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
            plan_runtime_calls(&module),
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
            plan_runtime_calls(&module),
            Err(CallPlanError::UnsupportedCleanup {
                cleanup: hir::StackCleanup::Caller,
                ..
            })
        ));
    }

    #[test]
    fn rejects_a_defined_callable() {
        let mut call_abi = abi(0, Vec::new());
        call_abi.callee = Some(hir::CallableId::new(0));
        let module = module(
            Vec::new(),
            vec![call(0, "B$RT", Vec::new(), Vec::new())],
            vec![call_abi],
        );

        assert!(matches!(
            plan_runtime_calls(&module),
            Err(CallPlanError::UnsupportedCallable { callable: _, .. })
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
            plan_runtime_calls(&module),
            Err(CallPlanError::OrphanAbi {
                function: hir::FunctionId::new(0),
                instruction: hir::InstructionId::new(0),
            })
        );
    }
}
