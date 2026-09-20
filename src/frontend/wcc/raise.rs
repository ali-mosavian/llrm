//! Narrow semantic raising from WCC capture facts to portable HIR.
//!
//! This first slice intentionally covers the scalar call shape recorded by
//! `fixtures/c/iparg.cgs`.  WCC node names and calling-convention bits stay
//! here; the HIR built below contains only values, functions, and generic ABI
//! facts.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::hir;

use super::capture::{
    AutomaticId, CallId, CaptureUnit, Node, NodeId, Procedure, SourceLocation, Symbol, SymbolId,
    TempId,
};

const WCC_REVERSE_PARAMETERS: u32 = 0x01;
const WCC_CALLER_CLEANUP: u32 = 0x80;

const VOID_TYPE: hir::TypeId = hir::TypeId::new(0);
const I16_TYPE: hir::TypeId = hir::TypeId::new(1);

/// A source-located refusal while raising a WCC capture unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RaiseError {
    pub location: SourceLocation,
    pub kind: RaiseErrorKind,
}

/// The unsupported or malformed WCC fact that prevented semantic raising.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RaiseErrorKind {
    MissingSymbol(SymbolId),
    MissingCallConvention(SymbolId),
    UnsupportedCallingConvention {
        symbol: SymbolId,
        detail: &'static str,
    },
    UnsupportedType {
        name: String,
    },
    MissingNode(NodeId),
    UnsupportedNode {
        node: NodeId,
        call: String,
    },
    InvalidNode {
        node: NodeId,
        detail: String,
    },
    UnsupportedStatement {
        call: String,
    },
    MissingParameterBinding(SymbolId),
    MissingTemporaryDeclaration(TempId),
    MissingTemporaryBinding(TempId),
    InvalidAssignmentTarget(NodeId),
    MissingPendingCall(CallId),
    InvalidCallTarget {
        call: CallId,
        detail: String,
    },
    MissingCallable(SymbolId),
    DuplicateTerminator,
    MissingReturn,
    IdOverflow {
        entity: &'static str,
    },
}

impl fmt::Display for RaiseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "WCC capture source {}:{}:{}: ",
            self.location.file.map_or(0, |file| file.get()),
            self.location.line,
            self.location.column
        )?;
        match &self.kind {
            RaiseErrorKind::MissingSymbol(symbol) => write!(formatter, "missing symbol {symbol}"),
            RaiseErrorKind::MissingCallConvention(symbol) => {
                write!(
                    formatter,
                    "procedure symbol {symbol} has no calling convention"
                )
            }
            RaiseErrorKind::UnsupportedCallingConvention { symbol, detail } => {
                write!(
                    formatter,
                    "procedure symbol {symbol} has unsupported convention: {detail}"
                )
            }
            RaiseErrorKind::UnsupportedType { name } => {
                write!(formatter, "unsupported WCC type {name:?}")
            }
            RaiseErrorKind::MissingNode(node) => {
                write!(formatter, "missing expression node {node}")
            }
            RaiseErrorKind::UnsupportedNode { node, call } => {
                write!(formatter, "unsupported expression node {node}: {call}")
            }
            RaiseErrorKind::InvalidNode { node, detail } => {
                write!(formatter, "invalid expression node {node}: {detail}")
            }
            RaiseErrorKind::UnsupportedStatement { call } => {
                write!(formatter, "unsupported procedure statement {call}")
            }
            RaiseErrorKind::MissingParameterBinding(symbol) => {
                write!(formatter, "parameter symbol {symbol} has no value binding")
            }
            RaiseErrorKind::MissingTemporaryDeclaration(temporary) => {
                write!(formatter, "temporary {temporary} has no declaration")
            }
            RaiseErrorKind::MissingTemporaryBinding(temporary) => {
                write!(formatter, "temporary {temporary} is read before assignment")
            }
            RaiseErrorKind::InvalidAssignmentTarget(node) => {
                write!(
                    formatter,
                    "expression node {node} is not a scalar temporary"
                )
            }
            RaiseErrorKind::MissingPendingCall(call) => {
                write!(formatter, "missing pending call {call}")
            }
            RaiseErrorKind::InvalidCallTarget { call, detail } => {
                write!(formatter, "invalid direct call {call}: {detail}")
            }
            RaiseErrorKind::MissingCallable(symbol) => {
                write!(formatter, "procedure symbol {symbol} has no callable")
            }
            RaiseErrorKind::DuplicateTerminator => formatter.write_str("statement follows return"),
            RaiseErrorKind::MissingReturn => {
                formatter.write_str("procedure has no return statement")
            }
            RaiseErrorKind::IdOverflow { entity } => {
                write!(formatter, "{entity} identifier overflow")
            }
        }
    }
}

impl Error for RaiseError {}

/// Raises the supported scalar WCC capture subset into one generic HIR module.
pub fn raise_module(unit: &CaptureUnit, module_name: &str) -> Result<hir::Module, RaiseError> {
    let mut callable_ids = BTreeMap::new();
    for (index, procedure) in unit.procedures.iter().enumerate() {
        let raw = u32::try_from(index)
            .map_err(|_| error_default(RaiseErrorKind::IdOverflow { entity: "callable" }))?;
        callable_ids.insert(procedure.symbol, hir::CallableId::new(raw));
    }

    let callables = unit
        .procedures
        .iter()
        .enumerate()
        .map(|(index, procedure)| callable(unit, procedure, index))
        .collect::<Result<Vec<_>, _>>()?;
    let functions = unit
        .procedures
        .iter()
        .enumerate()
        .map(|(index, procedure)| raise_function(unit, procedure, index, &callable_ids))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(hir::Module {
        id: hir::ModuleId::new(0),
        name: module_name.to_owned(),
        types: scalar_types(),
        functions,
        data: Vec::new(),
        callables,
    })
}

fn scalar_types() -> Vec<hir::Type> {
    vec![
        hir::Type {
            id: VOID_TYPE,
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
            id: I16_TYPE,
            name: "i16".into(),
            kind: hir::TypeKind::Integer,
            width: 2,
            signed: Some(true),
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        },
    ]
}

fn callable(
    unit: &CaptureUnit,
    procedure: &Procedure,
    index: usize,
) -> Result<hir::Callable, RaiseError> {
    let symbol = symbol(unit, procedure.symbol, SourceLocation::default())?;
    let id = u32::try_from(index)
        .map_err(|_| error_default(RaiseErrorKind::IdOverflow { entity: "callable" }))?;
    Ok(hir::Callable {
        id: hir::CallableId::new(id),
        name: symbol.object_name(),
        result_type: optional_result_type(unit, &procedure.value_type, SourceLocation::default())?,
        parameters: procedure
            .parameters
            .iter()
            .map(|(_, type_name)| {
                Ok(hir::Parameter {
                    type_id: value_type(unit, type_name, SourceLocation::default())?,
                    by_value: true,
                    segmented: false,
                    array: false,
                })
            })
            .collect::<Result<Vec<_>, RaiseError>>()?,
        defined: true,
    })
}

fn raise_function(
    unit: &CaptureUnit,
    procedure: &Procedure,
    index: usize,
    callable_ids: &BTreeMap<SymbolId, hir::CallableId>,
) -> Result<hir::Function, RaiseError> {
    let location = procedure_location(procedure);
    let symbol = symbol(unit, procedure.symbol, location)?;
    let convention = symbol.convention.as_ref().ok_or_else(|| {
        error(
            location,
            RaiseErrorKind::MissingCallConvention(procedure.symbol),
        )
    })?;
    if !is_supported_c_convention(convention.class) {
        return Err(error(
            location,
            RaiseErrorKind::UnsupportedCallingConvention {
                symbol: procedure.symbol,
                detail: "only the WCC C scalar convention is supported",
            },
        ));
    }
    if convention.has_register_parameters() {
        return Err(error(
            location,
            RaiseErrorKind::UnsupportedCallingConvention {
                symbol: procedure.symbol,
                detail: "register parameters are not supported",
            },
        ));
    }

    let id = u32::try_from(index)
        .map_err(|_| error(location, RaiseErrorKind::IdOverflow { entity: "function" }))?;
    let mut builder = FunctionRaiser::new(unit, procedure, location, callable_ids)?;
    let parameters = builder.parameters()?;
    let parameter_bytes = procedure
        .parameters
        .iter()
        .try_fold(0usize, |sum, (_, type_name)| {
            let type_id = value_type(unit, type_name, location)?;
            let width = match type_id {
                I16_TYPE => 2,
                _ => {
                    return Err(error(
                        location,
                        RaiseErrorKind::UnsupportedType {
                            name: type_name.clone(),
                        },
                    ));
                }
            };
            sum.checked_add(width).ok_or_else(|| {
                error(
                    location,
                    RaiseErrorKind::IdOverflow {
                        entity: "parameter byte count",
                    },
                )
            })
        })?;
    builder.raise_statements()?;

    Ok(hir::Function {
        id: hir::FunctionId::new(id),
        name: symbol.object_name(),
        result_type: value_type(unit, &procedure.value_type, location)?,
        values: builder.values,
        places: Vec::new(),
        blocks: vec![hir::Block {
            id: hir::BlockId::new(0),
            instructions: builder.instructions,
            terminator: builder
                .terminator
                .ok_or_else(|| error(location, RaiseErrorKind::MissingReturn))?,
        }],
        entry: hir::BlockId::new(0),
        parameters,
        abi: hir::ProcedureAbi {
            cleanup: hir::StackCleanup::Caller,
            distance: if convention.is_far() {
                hir::CallDistance::Far
            } else {
                hir::CallDistance::Near
            },
            parameter_bytes,
        },
        calls: builder.calls,
        error_handler: None,
        error_handler_local: false,
        external_entries: Vec::new(),
        linkage: if symbol.attributes.is_exported() {
            hir::Linkage::External
        } else {
            hir::Linkage::Internal
        },
    })
}

struct FunctionRaiser<'a> {
    unit: &'a CaptureUnit,
    procedure: &'a Procedure,
    location: SourceLocation,
    callable_ids: &'a BTreeMap<SymbolId, hir::CallableId>,
    values: Vec<hir::Value>,
    instructions: Vec<hir::Instruction>,
    calls: Vec<hir::CallAbi>,
    parameter_bindings: BTreeMap<SymbolId, hir::Operand>,
    temporary_types: BTreeMap<TempId, hir::TypeId>,
    temporary_bindings: BTreeMap<TempId, hir::Operand>,
    node_bindings: BTreeMap<NodeId, hir::Operand>,
    terminator: Option<hir::Terminator>,
    next_value: u32,
    next_instruction: u32,
}

impl<'a> FunctionRaiser<'a> {
    fn new(
        unit: &'a CaptureUnit,
        procedure: &'a Procedure,
        location: SourceLocation,
        callable_ids: &'a BTreeMap<SymbolId, hir::CallableId>,
    ) -> Result<Self, RaiseError> {
        let mut temporary_types = BTreeMap::new();
        for (automatic, type_name) in &procedure.automatics {
            let AutomaticId::Temporary(temporary) = automatic else {
                continue;
            };
            temporary_types.insert(*temporary, value_type(unit, type_name, location)?);
        }
        Ok(Self {
            unit,
            procedure,
            location,
            callable_ids,
            values: Vec::new(),
            instructions: Vec::new(),
            calls: Vec::new(),
            parameter_bindings: BTreeMap::new(),
            temporary_types,
            temporary_bindings: BTreeMap::new(),
            node_bindings: BTreeMap::new(),
            terminator: None,
            next_value: 0,
            next_instruction: 0,
        })
    }

    fn parameters(&mut self) -> Result<Vec<hir::ValueId>, RaiseError> {
        let mut parameters = Vec::with_capacity(self.procedure.parameters.len());
        for (symbol, type_name) in &self.procedure.parameters {
            let type_id = value_type(self.unit, type_name, self.location)?;
            let value = self.new_value(type_id)?;
            self.parameter_bindings
                .insert(*symbol, hir::Operand::Value(value));
            parameters.push(value);
        }
        Ok(parameters)
    }

    fn raise_statements(&mut self) -> Result<(), RaiseError> {
        for statement in &self.procedure.body {
            if self.terminator.is_some() {
                return Err(error(
                    statement.location,
                    RaiseErrorKind::DuplicateTerminator,
                ));
            }
            self.location = statement.location;
            match statement.call.as_str() {
                "CGDone" => {
                    self.require_argument(&statement.args, 0)?;
                    self.node(NodeId::new(parse_node_id(
                        &statement.args[0],
                        self.location,
                    )?))?;
                }
                "CGReturn" => {
                    let node = NodeId::new(parse_node_id(
                        self.require_argument(&statement.args, 0)?,
                        self.location,
                    )?);
                    let expected = value_type(
                        self.unit,
                        self.require_argument(&statement.args, 1)?,
                        self.location,
                    )?;
                    let operand = self.node(node)?;
                    require_operand_type(expected, &operand, &self.values, self.location)?;
                    self.terminator = Some(hir::Terminator::Return(Some(operand)));
                }
                call => {
                    return Err(error(
                        statement.location,
                        RaiseErrorKind::UnsupportedStatement {
                            call: call.to_owned(),
                        },
                    ));
                }
            }
        }
        Ok(())
    }

    fn node(&mut self, id: NodeId) -> Result<hir::Operand, RaiseError> {
        if let Some(binding) = self.node_bindings.get(&id) {
            return Ok(binding.clone());
        }
        let node = self
            .unit
            .nodes
            .get(&id)
            .cloned()
            .ok_or_else(|| error(self.location, RaiseErrorKind::MissingNode(id)))?;
        let result = match node.call.as_str() {
            "CGFEName" => self.frontend_name(id, &node)?,
            "CGTempName" => self.temporary_name(id, &node)?,
            "CGInteger" => self.integer(id, &node)?,
            "CGUnary" => self.unary(id, &node)?,
            "CGBinary" => self.binary(id, &node)?,
            "CGAssign" => self.assign(id, &node)?,
            "CGCall" => self.call(id, &node)?,
            _ => {
                return Err(error(
                    self.location,
                    RaiseErrorKind::UnsupportedNode {
                        node: id,
                        call: node.call,
                    },
                ));
            }
        };
        self.node_bindings.insert(id, result.clone());
        Ok(result)
    }

    fn frontend_name(&self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let symbol = SymbolId::new(parse_symbol_id(
            self.node_argument(id, node, 0)?,
            self.location,
        )?);
        self.parameter_bindings
            .get(&symbol)
            .cloned()
            .ok_or_else(|| {
                error(
                    self.location,
                    RaiseErrorKind::MissingParameterBinding(symbol),
                )
            })
    }

    fn temporary_name(&self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let temporary = TempId::new(parse_temp_id(
            self.node_argument(id, node, 0)?,
            self.location,
        )?);
        if !self.temporary_types.contains_key(&temporary) {
            return Err(error(
                self.location,
                RaiseErrorKind::MissingTemporaryDeclaration(temporary),
            ));
        }
        self.temporary_bindings
            .get(&temporary)
            .cloned()
            .ok_or_else(|| {
                error(
                    self.location,
                    RaiseErrorKind::MissingTemporaryBinding(temporary),
                )
            })
    }

    fn integer(&self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let value = self
            .node_argument(id, node, 0)?
            .parse::<i16>()
            .map_err(|_| self.invalid_node(id, "integer literal is outside signed i16"))?;
        let type_id = value_type(self.unit, self.node_argument(id, node, 1)?, self.location)?;
        if type_id != I16_TYPE {
            return Err(self.invalid_node(id, "integer literal is not TY_INT_2 or TY_INTEGER"));
        }
        Ok(hir::Operand::Constant {
            type_id,
            value: hir::ConstantValue::Integer(i64::from(value)),
        })
    }

    fn unary(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let operation = self.node_argument(id, node, 0)?;
        let operand_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?);
        if operation == "O_POINTS"
            && !self.unit.nodes.get(&operand_id).is_some_and(|operand| {
                matches!(operand.call.as_str(), "CGFEName" | "CGTempName" | "CGCall")
            })
        {
            return Err(self.invalid_node(
                id,
                "O_POINTS is only established for scalar names and call results",
            ));
        }
        let operand = self.node(operand_id)?;
        let type_id = value_type(self.unit, self.node_argument(id, node, 2)?, self.location)?;
        require_operand_type(type_id, &operand, &self.values, self.location)?;
        match operation {
            // WCC uses O_POINTS around scalar parameter and temporary nodes.
            // It is its lvalue convention, not a portable pointer load.
            "O_POINTS" | "O_CONVERT" => Ok(operand),
            _ => Err(self.invalid_node(id, "unsupported unary operation")),
        }
    }

    fn binary(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        if self.node_argument(id, node, 0)? != "O_TIMES" {
            return Err(self.invalid_node(id, "unsupported binary operation"));
        }
        let left = self.node(NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?))?;
        let right = self.node(NodeId::new(parse_node_id(
            self.node_argument(id, node, 2)?,
            self.location,
        )?))?;
        let type_id = value_type(self.unit, self.node_argument(id, node, 3)?, self.location)?;
        require_operand_type(type_id, &left, &self.values, self.location)?;
        require_operand_type(type_id, &right, &self.values, self.location)?;
        let result = self.new_value(type_id)?;
        self.push_instruction(hir::Opcode::Multiply, vec![result], vec![left, right], None)?;
        Ok(hir::Operand::Value(result))
    }

    fn assign(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let destination = NodeId::new(parse_node_id(
            self.node_argument(id, node, 0)?,
            self.location,
        )?);
        let destination_node = self
            .unit
            .nodes
            .get(&destination)
            .ok_or_else(|| error(self.location, RaiseErrorKind::MissingNode(destination)))?;
        if destination_node.call != "CGTempName" {
            return Err(error(
                self.location,
                RaiseErrorKind::InvalidAssignmentTarget(destination),
            ));
        }
        let temporary = TempId::new(parse_temp_id(
            destination_node
                .args
                .first()
                .ok_or_else(|| self.invalid_node(destination, "temporary name has no handle"))?,
            self.location,
        )?);
        let temporary_type = self
            .temporary_types
            .get(&temporary)
            .copied()
            .ok_or_else(|| {
                error(
                    self.location,
                    RaiseErrorKind::MissingTemporaryDeclaration(temporary),
                )
            })?;
        let value = self.node(NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?))?;
        let type_id = value_type(self.unit, self.node_argument(id, node, 2)?, self.location)?;
        require_operand_type(type_id, &value, &self.values, self.location)?;
        if temporary_type != type_id {
            return Err(self.invalid_node(id, "temporary type disagrees with assignment type"));
        }
        self.temporary_bindings.insert(temporary, value.clone());
        Ok(value)
    }

    fn call(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let call = CallId::new(parse_call_id(
            self.node_argument(id, node, 0)?,
            self.location,
        )?);
        let pending = self
            .unit
            .calls
            .get(&call)
            .cloned()
            .ok_or_else(|| error(self.location, RaiseErrorKind::MissingPendingCall(call)))?;
        let target = self
            .unit
            .nodes
            .get(&pending.target)
            .ok_or_else(|| error(self.location, RaiseErrorKind::MissingNode(pending.target)))?;
        if target.call != "CGFEName" {
            return Err(error(
                self.location,
                RaiseErrorKind::InvalidCallTarget {
                    call,
                    detail: "callee is not a frontend name".into(),
                },
            ));
        }
        let target_symbol = SymbolId::new(parse_symbol_id(
            target
                .args
                .first()
                .ok_or_else(|| self.invalid_node(pending.target, "frontend name has no symbol"))?,
            self.location,
        )?);
        if target_symbol != pending.symbol {
            return Err(error(
                self.location,
                RaiseErrorKind::InvalidCallTarget {
                    call,
                    detail: "CGInitCall symbol disagrees with its callee node".into(),
                },
            ));
        }
        let callable = self
            .callable_ids
            .get(&target_symbol)
            .copied()
            .ok_or_else(|| {
                error(
                    self.location,
                    RaiseErrorKind::MissingCallable(target_symbol),
                )
            })?;
        let callee = symbol(self.unit, target_symbol, self.location)?;
        let mut operands = Vec::with_capacity(pending.parameters.len());
        for (argument, type_name) in pending.parameters.iter().rev() {
            let operand = self.node(*argument)?;
            let type_id = value_type(self.unit, type_name, self.location)?;
            require_operand_type(type_id, &operand, &self.values, self.location)?;
            operands.push(operand);
        }
        let result_type = value_type(self.unit, &pending.value_type, self.location)?;
        let results = if result_type == VOID_TYPE {
            Vec::new()
        } else {
            vec![self.new_value(result_type)?]
        };
        let instruction = self.push_instruction(
            hir::Opcode::Call,
            results.clone(),
            operands,
            Some(callee.object_name()),
        )?;
        self.calls.push(hir::CallAbi {
            instruction,
            order: (0..pending.parameters.len()).rev().collect(),
            cleanup: hir::StackCleanup::Caller,
            distance: call_distance(self.unit, target_symbol, self.location)?,
            callee: Some(callable),
        });
        match results.as_slice() {
            [] => Err(self.invalid_node(id, "void CGCall cannot be used as a scalar expression")),
            [result] => Ok(hir::Operand::Value(*result)),
            _ => Err(self.invalid_node(id, "scalar CGCall has multiple results")),
        }
    }

    fn new_value(&mut self, type_id: hir::TypeId) -> Result<hir::ValueId, RaiseError> {
        let id = hir::ValueId::new(self.next_value);
        self.next_value = self.next_value.checked_add(1).ok_or_else(|| {
            error(
                self.location,
                RaiseErrorKind::IdOverflow { entity: "value" },
            )
        })?;
        self.values.push(hir::Value { id, type_id });
        Ok(id)
    }

    fn push_instruction(
        &mut self,
        opcode: hir::Opcode,
        results: Vec<hir::ValueId>,
        operands: Vec<hir::Operand>,
        callee: Option<String>,
    ) -> Result<hir::InstructionId, RaiseError> {
        let id = hir::InstructionId::new(self.next_instruction);
        self.next_instruction = self.next_instruction.checked_add(1).ok_or_else(|| {
            error(
                self.location,
                RaiseErrorKind::IdOverflow {
                    entity: "instruction",
                },
            )
        })?;
        self.instructions.push(hir::Instruction {
            id,
            opcode,
            results,
            operands,
            callee,
        });
        Ok(id)
    }

    fn node_argument<'b>(
        &self,
        id: NodeId,
        node: &'b Node,
        index: usize,
    ) -> Result<&'b str, RaiseError> {
        node.args
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| self.invalid_node(id, "missing argument"))
    }

    fn require_argument<'b>(
        &self,
        args: &'b [String],
        index: usize,
    ) -> Result<&'b str, RaiseError> {
        args.get(index).map(String::as_str).ok_or_else(|| {
            error(
                self.location,
                RaiseErrorKind::InvalidNode {
                    node: NodeId::new(0),
                    detail: "statement is missing an argument".into(),
                },
            )
        })
    }

    fn invalid_node(&self, node: NodeId, detail: impl Into<String>) -> RaiseError {
        error(
            self.location,
            RaiseErrorKind::InvalidNode {
                node,
                detail: detail.into(),
            },
        )
    }
}

fn call_distance(
    unit: &CaptureUnit,
    symbol_id: SymbolId,
    location: SourceLocation,
) -> Result<hir::CallDistance, RaiseError> {
    let symbol = symbol(unit, symbol_id, location)?;
    let convention = symbol
        .convention
        .as_ref()
        .ok_or_else(|| error(location, RaiseErrorKind::MissingCallConvention(symbol_id)))?;
    if !is_supported_c_convention(convention.class) || convention.has_register_parameters() {
        return Err(error(
            location,
            RaiseErrorKind::UnsupportedCallingConvention {
                symbol: symbol_id,
                detail: "direct call uses an unsupported convention",
            },
        ));
    }
    Ok(if convention.is_far() {
        hir::CallDistance::Far
    } else {
        hir::CallDistance::Near
    })
}

fn value_type(
    unit: &CaptureUnit,
    name: &str,
    location: SourceLocation,
) -> Result<hir::TypeId, RaiseError> {
    match unit.canonical_type(name).as_str() {
        "TY_INT_2" | "TY_INTEGER" => Ok(I16_TYPE),
        _ => Err(error(
            location,
            RaiseErrorKind::UnsupportedType {
                name: name.to_owned(),
            },
        )),
    }
}

fn optional_result_type(
    unit: &CaptureUnit,
    name: &str,
    location: SourceLocation,
) -> Result<Option<hir::TypeId>, RaiseError> {
    let type_id = value_type(unit, name, location)?;
    Ok((type_id != VOID_TYPE).then_some(type_id))
}

fn is_supported_c_convention(class: u32) -> bool {
    class & WCC_CALLER_CLEANUP != 0 && class & WCC_REVERSE_PARAMETERS == 0
}

fn require_operand_type(
    expected: hir::TypeId,
    operand: &hir::Operand,
    values: &[hir::Value],
    location: SourceLocation,
) -> Result<(), RaiseError> {
    let actual = match operand {
        hir::Operand::Value(value) => values
            .iter()
            .find(|candidate| candidate.id == *value)
            .map(|candidate| candidate.type_id)
            .ok_or_else(|| {
                error(
                    location,
                    RaiseErrorKind::InvalidNode {
                        node: NodeId::new(0),
                        detail: "expression refers to an unknown value".into(),
                    },
                )
            })?,
        hir::Operand::Constant { type_id, .. } => *type_id,
        _ => {
            return Err(error(
                location,
                RaiseErrorKind::InvalidNode {
                    node: NodeId::new(0),
                    detail: "scalar expression produced a non-value operand".into(),
                },
            ));
        }
    };
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            location,
            RaiseErrorKind::InvalidNode {
                node: NodeId::new(0),
                detail: "operand type disagrees with the capture type".into(),
            },
        ))
    }
}

fn symbol(
    unit: &CaptureUnit,
    id: SymbolId,
    location: SourceLocation,
) -> Result<&Symbol, RaiseError> {
    unit.symbols
        .get(&id)
        .ok_or_else(|| error(location, RaiseErrorKind::MissingSymbol(id)))
}

fn procedure_location(procedure: &Procedure) -> SourceLocation {
    procedure
        .body
        .first()
        .map(|statement| statement.location)
        .unwrap_or_default()
}

fn parse_handle(value: &str, prefix: char, location: SourceLocation) -> Result<u32, RaiseError> {
    let digits = value.strip_prefix(prefix).ok_or_else(|| {
        error(
            location,
            RaiseErrorKind::InvalidNode {
                node: NodeId::new(0),
                detail: format!("expected {prefix}-prefixed capture handle"),
            },
        )
    })?;
    digits.parse::<u32>().map_err(|_| {
        error(
            location,
            RaiseErrorKind::InvalidNode {
                node: NodeId::new(0),
                detail: format!("invalid {prefix}-prefixed capture handle"),
            },
        )
    })
}

fn parse_node_id(value: &str, location: SourceLocation) -> Result<u32, RaiseError> {
    parse_handle(value, 'n', location)
}

fn parse_symbol_id(value: &str, location: SourceLocation) -> Result<u32, RaiseError> {
    parse_handle(value, 'y', location)
}

fn parse_temp_id(value: &str, location: SourceLocation) -> Result<u32, RaiseError> {
    parse_handle(value, 't', location)
}

fn parse_call_id(value: &str, location: SourceLocation) -> Result<u32, RaiseError> {
    parse_handle(value, 'c', location)
}

fn error(location: SourceLocation, kind: RaiseErrorKind) -> RaiseError {
    RaiseError { location, kind }
}

fn error_default(kind: RaiseErrorKind) -> RaiseError {
    error(SourceLocation::default(), kind)
}

#[cfg(test)]
mod tests {
    use super::{RaiseErrorKind, raise_module};
    use crate::frontend::wcc::{capture, parse};
    use crate::hir;
    use crate::ir;

    fn iparg() -> capture::CaptureUnit {
        capture::build(&parse(include_str!("../../../fixtures/c/iparg.cgs")).unwrap()).unwrap()
    }

    #[test]
    fn raises_the_real_iparg_capture_to_generic_hir() {
        let module = raise_module(&iparg(), "iparg").unwrap();

        assert_eq!(module.types.len(), 2);
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "_twice");
        assert_eq!(module.functions[0].linkage, hir::Linkage::Internal);
        assert_eq!(module.functions[0].abi.cleanup, hir::StackCleanup::Caller);
        assert_eq!(module.functions[0].abi.distance, hir::CallDistance::Near);
        assert_eq!(module.functions[0].parameters, [hir::ValueId::new(0)]);
        assert!(matches!(
            module.functions[0].blocks[0].instructions.as_slice(),
            [hir::Instruction {
                opcode: hir::Opcode::Multiply,
                results,
                operands,
                ..
            }] if results == &vec![hir::ValueId::new(1)]
                && matches!(operands.as_slice(), [hir::Operand::Value(value), hir::Operand::Constant { value: hir::ConstantValue::Integer(2), .. }] if *value == hir::ValueId::new(0))
        ));

        let answer = &module.functions[1];
        assert_eq!(answer.name, "_answer_from_argument");
        assert_eq!(answer.linkage, hir::Linkage::External);
        assert_eq!(answer.abi.cleanup, hir::StackCleanup::Caller);
        assert_eq!(answer.abi.distance, hir::CallDistance::Far);
        assert_eq!(module.callables[0].name, "_twice");
        let call = &answer.blocks[0].instructions[0];
        assert!(matches!(
            call,
            hir::Instruction {
                opcode: hir::Opcode::Call,
                results,
                operands,
                callee: Some(name),
                ..
            } if results == &vec![hir::ValueId::new(0)]
                && name == "_twice"
                && matches!(operands.as_slice(), [hir::Operand::Constant { value: hir::ConstantValue::Integer(21), .. }])
        ));
        assert_eq!(answer.calls[0].instruction, call.id);
        assert_eq!(answer.calls[0].order, [0]);
        assert_eq!(answer.calls[0].cleanup, hir::StackCleanup::Caller);
        assert_eq!(answer.calls[0].distance, hir::CallDistance::Near);
        assert_eq!(answer.calls[0].callee, Some(hir::CallableId::new(0)));
        assert!(module.verify().is_ok());
        let lowered = hir::lower_to_ir(&module).unwrap();
        assert_eq!(
            lowered.functions[0].signature.calling_convention,
            ir::CallingConvention::C
        );
        assert_eq!(
            lowered.functions[1].signature.calling_convention,
            ir::CallingConvention::FarCdecl
        );
        assert!(
            lowered.functions[0].blocks[0]
                .instructions
                .iter()
                .any(|instruction| matches!(
                    &instruction.kind,
                    ir::InstructionKind::Binary {
                        op: ir::BinaryOp::Multiply,
                        ..
                    }
                ))
        );
        assert!(
            lowered.functions[1].blocks[0]
                .instructions
                .iter()
                .any(|instruction| matches!(&instruction.kind, ir::InstructionKind::Call { .. }))
        );
    }

    #[test]
    fn non_exported_procedure_linkage_does_not_depend_on_private_segment_bits() {
        let mut unit = iparg();
        unit.symbols
            .get_mut(&capture::SymbolId::new(1))
            .unwrap()
            .attributes = capture::SymbolAttributes::from_bits(0x03);

        let module = raise_module(&unit, "iparg").unwrap();

        assert_eq!(module.functions[0].linkage, hir::Linkage::Internal);
    }

    #[test]
    fn refuses_to_treat_points_of_an_arbitrary_scalar_as_identity() {
        let mut unit = iparg();
        unit.nodes.get_mut(&capture::NodeId::new(5)).unwrap().args[1] = "n6".into();

        let error = raise_module(&unit, "iparg").unwrap_err();

        assert!(matches!(
            error.kind,
            RaiseErrorKind::InvalidNode { node, .. }
                if node == capture::NodeId::new(5)
        ));
    }

    #[test]
    fn refuses_an_unrecognised_expression_at_its_source_cue() {
        let mut unit = iparg();
        unit.nodes.get_mut(&capture::NodeId::new(7)).unwrap().args[0] = "O_PLUS".into();

        let error = raise_module(&unit, "iparg").unwrap_err();
        assert_eq!(error.location.file, Some(capture::SourceFileId::new(1)));
        assert_eq!(error.location.line, 4);
        assert!(matches!(
            error.kind,
            RaiseErrorKind::InvalidNode {
                node,
                ..
            } if node == capture::NodeId::new(7)
        ));
    }
}
