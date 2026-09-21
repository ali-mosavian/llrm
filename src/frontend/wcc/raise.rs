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
const I32_TYPE: hir::TypeId = hir::TypeId::new(2);
const U16_TYPE: hir::TypeId = hir::TypeId::new(3);
const U32_TYPE: hir::TypeId = hir::TypeId::new(4);
const BOOL_TYPE: hir::TypeId = hir::TypeId::new(5);
const WCC_BIG_DATA: u32 = 0x2;

struct WccTypes {
    types: Vec<hir::Type>,
    aggregates: BTreeMap<String, hir::TypeId>,
    near_pointer: Option<hir::TypeId>,
}

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
    let types = wcc_types(unit)?;
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
        .map(|(index, procedure)| raise_function(unit, procedure, index, &callable_ids, &types))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(hir::Module {
        id: hir::ModuleId::new(0),
        name: module_name.to_owned(),
        types: types.types,
        functions,
        data: Vec::new(),
        callables,
    })
}

fn wcc_types(unit: &CaptureUnit) -> Result<WccTypes, RaiseError> {
    let mut types = scalar_types();
    // Every supported value-producing CG node carries its type in its final
    // argument. CGCall is scalar-only in this slice and resolves its type
    // from CGInitCall instead, so it cannot introduce a supported pointer.
    let has_near_pointer = unit.nodes.values().any(|node| {
        node.args
            .last()
            .is_some_and(|name| unit.canonical_type(name) == "TY_POINTER")
    });
    if has_near_pointer && unit.target & WCC_BIG_DATA != 0 {
        return Err(error_default(RaiseErrorKind::UnsupportedType {
            name: "TY_POINTER in a WCC big-data target".into(),
        }));
    }
    let near_pointer = if has_near_pointer {
        let id = hir::TypeId::new(
            u32::try_from(types.len())
                .map_err(|_| error_default(RaiseErrorKind::IdOverflow { entity: "type" }))?,
        );
        types.push(hir::Type {
            id,
            name: "near-pointer".into(),
            kind: hir::TypeKind::Pointer,
            width: 2,
            signed: None,
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::Near,
        });
        Some(id)
    } else {
        None
    };
    let mut aggregates = BTreeMap::new();
    for (name, width) in &unit.types {
        let id = hir::TypeId::new(
            u32::try_from(types.len())
                .map_err(|_| error_default(RaiseErrorKind::IdOverflow { entity: "type" }))?,
        );
        types.push(hir::Type {
            id,
            name: name.clone(),
            kind: hir::TypeKind::Opaque,
            width: usize::try_from(*width).map_err(|_| {
                error_default(RaiseErrorKind::IdOverflow {
                    entity: "aggregate extent",
                })
            })?,
            signed: None,
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        });
        aggregates.insert(name.clone(), id);
    }
    Ok(WccTypes {
        types,
        aggregates,
        near_pointer,
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
        hir::Type {
            id: I32_TYPE,
            name: "i32".into(),
            kind: hir::TypeKind::Integer,
            width: 4,
            signed: Some(true),
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        },
        hir::Type {
            id: U16_TYPE,
            name: "u16".into(),
            kind: hir::TypeKind::Integer,
            width: 2,
            signed: Some(false),
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        },
        hir::Type {
            id: U32_TYPE,
            name: "u32".into(),
            kind: hir::TypeKind::Integer,
            width: 4,
            signed: Some(false),
            evaluation: hir::FloatEvaluation::None,
            element: None,
            bounds: Vec::new(),
            address: hir::AddressKind::None,
        },
        hir::Type {
            id: BOOL_TYPE,
            name: "bool".into(),
            kind: hir::TypeKind::Boolean,
            width: 1,
            signed: None,
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
    types: &WccTypes,
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
    let mut builder = FunctionRaiser::new(unit, procedure, location, callable_ids, types)?;
    let parameters = builder.parameters()?;
    let parameter_bytes = procedure
        .parameters
        .iter()
        .try_fold(0usize, |sum, (_, type_name)| {
            let type_id = value_type(unit, type_name, location)?;
            let width = match type_width(&types.types, type_id, location)? {
                width @ (2 | 4) => width,
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
    let places = std::mem::take(&mut builder.places);
    let blocks = builder.finish_blocks()?;

    Ok(hir::Function {
        id: hir::FunctionId::new(id),
        name: symbol.object_name(),
        result_type: value_type(unit, &procedure.value_type, location)?,
        values: builder.values,
        places,
        blocks,
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
    types: &'a WccTypes,
    values: Vec<hir::Value>,
    places: Vec<hir::Place>,
    blocks: Vec<RaisedBlock>,
    current_block: usize,
    labels: BTreeMap<String, hir::BlockId>,
    calls: Vec<hir::CallAbi>,
    parameter_bindings: BTreeMap<SymbolId, hir::Operand>,
    automatic_places: BTreeMap<SymbolId, hir::PlaceId>,
    temporary_places: BTreeMap<TempId, hir::PlaceId>,
    node_bindings: BTreeMap<NodeId, hir::Operand>,
    next_value: u32,
    next_instruction: u32,
    next_block: u32,
}

struct RaisedBlock {
    id: hir::BlockId,
    instructions: Vec<hir::Instruction>,
    terminator: Option<hir::Terminator>,
}

impl<'a> FunctionRaiser<'a> {
    fn new(
        unit: &'a CaptureUnit,
        procedure: &'a Procedure,
        location: SourceLocation,
        callable_ids: &'a BTreeMap<SymbolId, hir::CallableId>,
        types: &'a WccTypes,
    ) -> Result<Self, RaiseError> {
        let mut places = Vec::new();
        let mut automatic_places = BTreeMap::new();
        let mut temporary_places = BTreeMap::new();
        for (automatic, type_name) in &procedure.automatics {
            let type_id = capture_type(unit, types, type_name, location)?;
            let id =
                hir::PlaceId::new(u32::try_from(places.len()).map_err(|_| {
                    error(location, RaiseErrorKind::IdOverflow { entity: "place" })
                })?);
            let name = match automatic {
                AutomaticId::Symbol(symbol_id) => {
                    let symbol = symbol(unit, *symbol_id, location)?;
                    automatic_places.insert(*symbol_id, id);
                    symbol.name.clone()
                }
                AutomaticId::Temporary(temporary) => {
                    temporary_places.insert(*temporary, id);
                    format!("temporary{}", temporary.get())
                }
            };
            places.push(hir::Place {
                id,
                name,
                type_id,
                storage: hir::Storage::Local,
                offset: 0,
                symbol: hir::DataId::new(0),
                extent: type_width(&types.types, type_id, location)?,
                address: hir::AddressKind::Near,
            });
        }
        Ok(Self {
            unit,
            procedure,
            location,
            callable_ids,
            types,
            values: Vec::new(),
            places,
            blocks: vec![RaisedBlock {
                id: hir::BlockId::new(0),
                instructions: Vec::new(),
                terminator: None,
            }],
            current_block: 0,
            labels: BTreeMap::new(),
            calls: Vec::new(),
            parameter_bindings: BTreeMap::new(),
            automatic_places,
            temporary_places,
            node_bindings: BTreeMap::new(),
            next_value: 0,
            next_instruction: 0,
            next_block: 1,
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
            if self.current().terminator.is_some()
                && !matches!(statement.call.as_str(), "CGControl")
            {
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
                    self.current_mut().terminator = Some(hir::Terminator::Return(Some(operand)));
                }
                "CGControl" => self.control(statement)?,
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

    fn control(&mut self, statement: &super::capture::Statement) -> Result<(), RaiseError> {
        let operation = self.require_argument(&statement.args, 0)?;
        let label = self.require_argument(&statement.args, 2)?.to_owned();
        match operation {
            "O_LABEL" => {
                let target = self.label(&label)?;
                if self.current().id != target && self.current().terminator.is_none() {
                    self.current_mut().terminator = Some(hir::Terminator::Jump(target));
                }
                self.select_block(target)?;
            }
            "O_GOTO" => {
                let target = self.label(&label)?;
                self.current_mut().terminator = Some(hir::Terminator::Jump(target));
            }
            "O_IF_TRUE" | "O_IF_FALSE" => {
                let condition = self.node(NodeId::new(parse_node_id(
                    self.require_argument(&statement.args, 1)?,
                    self.location,
                )?))?;
                require_operand_type(BOOL_TYPE, &condition, &self.values, self.location)?;
                let target = self.label(&label)?;
                let fallthrough = self.new_block()?;
                let (then_block, else_block) = if operation == "O_IF_TRUE" {
                    (target, fallthrough)
                } else {
                    (fallthrough, target)
                };
                self.current_mut().terminator = Some(hir::Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                });
                self.select_block(fallthrough)?;
            }
            _ => {
                return Err(error(
                    self.location,
                    RaiseErrorKind::UnsupportedStatement {
                        call: format!("CGControl {operation}"),
                    },
                ));
            }
        }
        Ok(())
    }

    fn current(&self) -> &RaisedBlock {
        &self.blocks[self.current_block]
    }

    fn current_mut(&mut self) -> &mut RaisedBlock {
        &mut self.blocks[self.current_block]
    }

    fn new_block(&mut self) -> Result<hir::BlockId, RaiseError> {
        let id = hir::BlockId::new(self.next_block);
        self.next_block = self.next_block.checked_add(1).ok_or_else(|| {
            error(
                self.location,
                RaiseErrorKind::IdOverflow { entity: "block" },
            )
        })?;
        self.blocks.push(RaisedBlock {
            id,
            instructions: Vec::new(),
            terminator: None,
        });
        Ok(id)
    }

    fn label(&mut self, name: &str) -> Result<hir::BlockId, RaiseError> {
        if let Some(id) = self.labels.get(name) {
            return Ok(*id);
        }
        let id = self.new_block()?;
        self.labels.insert(name.to_owned(), id);
        Ok(id)
    }

    fn select_block(&mut self, id: hir::BlockId) -> Result<(), RaiseError> {
        self.current_block = self
            .blocks
            .iter()
            .position(|block| block.id == id)
            .ok_or_else(|| {
                error(
                    self.location,
                    RaiseErrorKind::InvalidNode {
                        node: NodeId::new(0),
                        detail: format!("missing raised block {id}"),
                    },
                )
            })?;
        Ok(())
    }

    fn finish_blocks(&mut self) -> Result<Vec<hir::Block>, RaiseError> {
        if self.blocks.iter().any(|block| block.terminator.is_none()) {
            return Err(error(self.location, RaiseErrorKind::MissingReturn));
        }
        let mut finished = Vec::with_capacity(self.blocks.len());
        for block in std::mem::take(&mut self.blocks) {
            let terminator = block
                .terminator
                .ok_or_else(|| error(self.location, RaiseErrorKind::MissingReturn))?;
            finished.push(hir::Block {
                id: block.id,
                instructions: block.instructions,
                terminator,
            });
        }
        Ok(finished)
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
            "CGCompare" => self.compare(id, &node)?,
            "CGAssign" => self.assign(id, &node)?,
            "CGPreGets" => self.pre_gets(id, &node)?,
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
        if let Some(parameter) = self.parameter_bindings.get(&symbol) {
            return Ok(parameter.clone());
        }
        self.automatic_places
            .get(&symbol)
            .copied()
            .map(hir::Operand::Place)
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
        self.temporary_places
            .get(&temporary)
            .copied()
            .map(hir::Operand::Place)
            .ok_or_else(|| {
                error(
                    self.location,
                    RaiseErrorKind::MissingTemporaryDeclaration(temporary),
                )
            })
    }

    fn integer(&self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let value = self
            .node_argument(id, node, 0)?
            .parse::<i64>()
            .map_err(|_| {
                self.invalid_node(id, "integer literal is outside the supported scalar range")
            })?;
        let type_id = value_type(self.unit, self.node_argument(id, node, 1)?, self.location)?;
        if !is_integer_type(type_id) {
            return Err(self.invalid_node(id, "integer literal is not a supported integer type"));
        }
        Ok(hir::Operand::Constant {
            type_id,
            value: hir::ConstantValue::Integer(wrap_integer(value, type_id)),
        })
    }

    fn unary(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let operation = self.node_argument(id, node, 0)?;
        let operand_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?);
        let established_value = self.unit.nodes.get(&operand_id).is_some_and(|operand| {
            matches!(operand.call.as_str(), "CGFEName" | "CGTempName" | "CGCall")
        });
        let operand = self.node(operand_id)?;
        let type_id = capture_type(
            self.unit,
            self.types,
            self.node_argument(id, node, 2)?,
            self.location,
        )?;
        match operation {
            // WCC uses O_POINTS both to preserve an aggregate's address and to
            // dereference a typed pointer. Scalar frame cells remain loads.
            "O_POINTS" => match operand {
                hir::Operand::Place(place) => {
                    self.require_place_type(place, type_id)?;
                    if self.is_aggregate_type(type_id) {
                        return Ok(hir::Operand::Place(place));
                    }
                    let result = self.new_value(type_id)?;
                    self.push_instruction(
                        hir::Opcode::Load,
                        vec![result],
                        vec![hir::Operand::Place(place)],
                        None,
                    )?;
                    Ok(hir::Operand::Value(result))
                }
                hir::Operand::Value(base) if self.is_pointer_value(base)? => {
                    if !is_integer_type(type_id) {
                        return Err(
                            self.invalid_node(id, "pointer dereference is not a scalar type")
                        );
                    }
                    let result = self.new_value(type_id)?;
                    self.push_instruction(
                        hir::Opcode::Load,
                        vec![result],
                        vec![hir::Operand::Indirect {
                            base,
                            offset: 0,
                            type_id,
                            volatile: false,
                        }],
                        None,
                    )?;
                    Ok(hir::Operand::Value(result))
                }
                _ if established_value => {
                    require_operand_type(type_id, &operand, &self.values, self.location)?;
                    Ok(operand)
                }
                _ => Err(self.invalid_node(
                    id,
                    "O_POINTS is only established for scalar names, call results, and typed pointers",
                )),
            },
            "O_CONVERT" => {
                let source_type = self.node_type(operand_id)?;
                require_operand_type(source_type, &operand, &self.values, self.location)?;
                self.convert(operand, source_type, type_id)
            }
            _ => Err(self.invalid_node(id, "unsupported unary operation")),
        }
    }

    fn binary(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let capture_type = capture_type(
            self.unit,
            self.types,
            self.node_argument(id, node, 3)?,
            self.location,
        )?;
        let left_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?);
        let right_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 2)?,
            self.location,
        )?);
        if self.is_near_pointer_type(capture_type) {
            return self.near_pointer_arithmetic(id, node, left_id, right_id);
        }
        let opcode = match self.node_argument(id, node, 0)? {
            "O_TIMES" => hir::Opcode::Multiply,
            "O_PLUS" => hir::Opcode::Add,
            "O_MINUS" => hir::Opcode::Subtract,
            "O_DIV" => hir::Opcode::Divide,
            _ => return Err(self.invalid_node(id, "unsupported binary operation")),
        };
        let left = self.node(left_id)?;
        let right = self.node(right_id)?;
        let type_id = capture_type;
        let left = self.coerce(left, self.node_type(left_id)?, type_id)?;
        let right = self.coerce(right, self.node_type(right_id)?, type_id)?;
        let result = self.new_value(type_id)?;
        self.push_instruction(opcode, vec![result], vec![left, right], None)?;
        Ok(hir::Operand::Value(result))
    }

    fn near_pointer_arithmetic(
        &mut self,
        id: NodeId,
        node: &Node,
        left_id: NodeId,
        right_id: NodeId,
    ) -> Result<hir::Operand, RaiseError> {
        let operation = self.node_argument(id, node, 0)?;
        if !matches!(operation, "O_PLUS" | "O_MINUS") {
            return Err(self.invalid_node(id, "unsupported near-pointer operation"));
        }
        // CGBinary evaluates its left expression before its right expression,
        // as Python _Raise.binary does.  O_PLUS may select the right address
        // only after both expressions have run.
        let left = self.node(left_id)?;
        let right = self.node(right_id)?;
        let left_type = self.node_type(left_id)?;
        let right_type = self.node_type(right_id)?;
        let left_is_address = self.is_near_pointer_base(&left)?;
        let right_is_address = self.is_near_pointer_base(&right)?;
        let (base, offset, offset_type) = match operation {
            "O_PLUS" if left_is_address => (left, right, right_type),
            "O_PLUS" if right_is_address => (right, left, left_type),
            "O_MINUS" if left_is_address => (left, right, right_type),
            "O_MINUS" if right_is_address => {
                return Err(self.invalid_node(
                    id,
                    "near-pointer subtraction requires the address on the left",
                ));
            }
            _ => {
                return Err(self.invalid_node(
                    id,
                    "near-pointer arithmetic requires an aggregate place or supported pointer value",
                ));
            }
        };
        require_operand_type(offset_type, &offset, &self.values, self.location)?;
        if !is_integer_type(offset_type) {
            return Err(self.invalid_node(id, "near-pointer byte offset is not an integer"));
        }
        let pointer_type = self.near_pointer_type()?;
        let base = self.materialize_near_pointer_base(id, base, pointer_type)?;
        let offset = if operation == "O_MINUS" {
            let negated = self.new_value(offset_type)?;
            self.push_instruction(hir::Opcode::Negate, vec![negated], vec![offset], None)?;
            hir::Operand::Value(negated)
        } else {
            offset
        };
        let result = self.new_value(pointer_type)?;
        self.push_instruction(
            hir::Opcode::OffsetPointer,
            vec![result],
            vec![hir::Operand::Value(base), offset],
            None,
        )?;
        Ok(hir::Operand::Value(result))
    }

    fn is_near_pointer_base(&self, operand: &hir::Operand) -> Result<bool, RaiseError> {
        match operand {
            hir::Operand::Place(place) => Ok(self.is_aggregate_type(self.place_type(*place)?)),
            hir::Operand::Value(value) => self.is_pointer_value(*value),
            _ => Ok(false),
        }
    }

    fn materialize_near_pointer_base(
        &mut self,
        id: NodeId,
        base: hir::Operand,
        pointer_type: hir::TypeId,
    ) -> Result<hir::ValueId, RaiseError> {
        match base {
            hir::Operand::Place(place) => {
                let aggregate_type = self.place_type(place)?;
                if !self.is_aggregate_type(aggregate_type) {
                    return Err(
                        self.invalid_node(id, "near-pointer place base is not an aggregate")
                    );
                }
                let address = self.new_value(pointer_type)?;
                self.push_instruction(
                    hir::Opcode::Address,
                    vec![address],
                    vec![hir::Operand::Place(place)],
                    None,
                )?;
                Ok(address)
            }
            hir::Operand::Value(value) => {
                if !self.is_pointer_value(value)? {
                    return Err(
                        self.invalid_node(id, "near-pointer value base is not a supported pointer")
                    );
                }
                Ok(value)
            }
            _ => {
                return Err(self.invalid_node(
                    id,
                    "near-pointer base is neither an aggregate place nor a supported pointer value",
                ));
            }
        }
    }

    fn compare(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let opcode = match self.node_argument(id, node, 0)? {
            "O_LT" => hir::Opcode::LessThan,
            _ => return Err(self.invalid_node(id, "unsupported comparison operation")),
        };
        let left_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?);
        let right_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 2)?,
            self.location,
        )?);
        let type_id = value_type(self.unit, self.node_argument(id, node, 3)?, self.location)?;
        let left_type = self.node_type(left_id)?;
        let left = self.node(left_id)?;
        let left = self.coerce(left, left_type, type_id)?;
        let right_type = self.node_type(right_id)?;
        let right = self.node(right_id)?;
        let right = self.coerce(right, right_type, type_id)?;
        let result = self.new_value(BOOL_TYPE)?;
        self.push_instruction(opcode, vec![result], vec![left, right], None)?;
        Ok(hir::Operand::Value(result))
    }

    fn assign(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let destination = NodeId::new(parse_node_id(
            self.node_argument(id, node, 0)?,
            self.location,
        )?);
        let target = self.node(destination)?;
        let source = NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?);
        let source_type = self.node_type(source)?;
        let value = self.node(source)?;
        let type_id = value_type(self.unit, self.node_argument(id, node, 2)?, self.location)?;
        let value = self.coerce(value, source_type, type_id)?;
        let address = match target {
            hir::Operand::Place(place) => {
                self.require_place_type(place, type_id)?;
                hir::Operand::Place(place)
            }
            hir::Operand::Value(base) if self.is_pointer_value(base)? => hir::Operand::Indirect {
                base,
                offset: 0,
                type_id,
                volatile: false,
            },
            _ => {
                return Err(error(
                    self.location,
                    RaiseErrorKind::InvalidAssignmentTarget(destination),
                ));
            }
        };
        self.push_instruction(
            hir::Opcode::Store,
            Vec::new(),
            vec![address, value.clone()],
            None,
        )?;
        Ok(value)
    }

    fn pre_gets(&mut self, id: NodeId, node: &Node) -> Result<hir::Operand, RaiseError> {
        let opcode = match self.node_argument(id, node, 0)? {
            "O_PLUS" => hir::Opcode::Add,
            "O_MINUS" => hir::Opcode::Subtract,
            _ => return Err(self.invalid_node(id, "unsupported pre-get operation")),
        };
        let target_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 1)?,
            self.location,
        )?);
        let target = self.node(target_id)?;
        let hir::Operand::Place(place) = target else {
            return Err(error(
                self.location,
                RaiseErrorKind::InvalidAssignmentTarget(target_id),
            ));
        };
        let type_id = value_type(self.unit, self.node_argument(id, node, 3)?, self.location)?;
        self.require_place_type(place, type_id)?;
        let old = self.new_value(type_id)?;
        self.push_instruction(
            hir::Opcode::Load,
            vec![old],
            vec![hir::Operand::Place(place)],
            None,
        )?;
        let source_id = NodeId::new(parse_node_id(
            self.node_argument(id, node, 2)?,
            self.location,
        )?);
        let source_type = self.node_type(source_id)?;
        let source = self.node(source_id)?;
        let source = self.coerce(source, source_type, type_id)?;
        let result = self.new_value(type_id)?;
        self.push_instruction(
            opcode,
            vec![result],
            vec![hir::Operand::Value(old), source],
            None,
        )?;
        self.push_instruction(
            hir::Opcode::Store,
            Vec::new(),
            vec![hir::Operand::Place(place), hir::Operand::Value(result)],
            None,
        )?;
        Ok(hir::Operand::Value(result))
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

    fn require_place_type(
        &self,
        place: hir::PlaceId,
        expected: hir::TypeId,
    ) -> Result<(), RaiseError> {
        let actual = self
            .places
            .iter()
            .find(|candidate| candidate.id == place)
            .map(|candidate| candidate.type_id)
            .ok_or_else(|| {
                self.invalid_node(NodeId::new(0), "operand refers to an unknown place")
            })?;
        if actual == expected {
            Ok(())
        } else {
            Err(self.invalid_node(NodeId::new(0), "place type disagrees with capture type"))
        }
    }

    fn place_type(&self, place: hir::PlaceId) -> Result<hir::TypeId, RaiseError> {
        self.places
            .iter()
            .find(|candidate| candidate.id == place)
            .map(|candidate| candidate.type_id)
            .ok_or_else(|| self.invalid_node(NodeId::new(0), "operand refers to an unknown place"))
    }

    fn is_aggregate_type(&self, type_id: hir::TypeId) -> bool {
        self.types
            .aggregates
            .values()
            .any(|candidate| *candidate == type_id)
    }

    fn near_pointer_type(&self) -> Result<hir::TypeId, RaiseError> {
        self.types.near_pointer.ok_or_else(|| {
            error(
                self.location,
                RaiseErrorKind::UnsupportedType {
                    name: "TY_POINTER without a pointer capture node".into(),
                },
            )
        })
    }

    fn is_near_pointer_type(&self, type_id: hir::TypeId) -> bool {
        self.types.near_pointer == Some(type_id)
    }

    fn is_pointer_value(&self, value: hir::ValueId) -> Result<bool, RaiseError> {
        let type_id = self
            .values
            .iter()
            .find(|candidate| candidate.id == value)
            .ok_or_else(|| {
                self.invalid_node(NodeId::new(0), "expression refers to an unknown value")
            })?
            .type_id;
        Ok(self.is_near_pointer_type(type_id))
    }

    fn node_type(&self, id: NodeId) -> Result<hir::TypeId, RaiseError> {
        let node = self
            .unit
            .nodes
            .get(&id)
            .ok_or_else(|| error(self.location, RaiseErrorKind::MissingNode(id)))?;
        if node.call == "CGCompare" {
            return Ok(BOOL_TYPE);
        }
        let type_index = match node.call.as_str() {
            "CGFEName" | "CGTempName" | "CGInteger" => 1,
            "CGUnary" | "CGBinary" | "CGAssign" | "CGPreGets" => node.args.len() - 1,
            "CGCall" => {
                let call = CallId::new(parse_call_id(
                    node.args
                        .first()
                        .ok_or_else(|| self.invalid_node(id, "CGCall has no call handle"))?,
                    self.location,
                )?);
                let pending = self.unit.calls.get(&call).ok_or_else(|| {
                    error(self.location, RaiseErrorKind::MissingPendingCall(call))
                })?;
                return value_type(self.unit, &pending.value_type, self.location);
            }
            _ => return Err(self.invalid_node(id, "expression has no scalar type")),
        };
        capture_type(
            self.unit,
            self.types,
            node.args
                .get(type_index)
                .ok_or_else(|| self.invalid_node(id, "expression is missing its type"))?,
            self.location,
        )
    }

    fn coerce(
        &mut self,
        operand: hir::Operand,
        source: hir::TypeId,
        target: hir::TypeId,
    ) -> Result<hir::Operand, RaiseError> {
        require_operand_type(source, &operand, &self.values, self.location)?;
        if source == target {
            return Ok(operand);
        }
        if is_integer_type(source) && is_integer_type(target) {
            if let hir::Operand::Constant {
                value: hir::ConstantValue::Integer(value),
                ..
            } = operand
            {
                return Ok(hir::Operand::Constant {
                    type_id: target,
                    value: hir::ConstantValue::Integer(wrap_integer(value, target)),
                });
            }
        }
        let result = self.new_value(target)?;
        self.push_instruction(hir::Opcode::Convert, vec![result], vec![operand], None)?;
        Ok(hir::Operand::Value(result))
    }

    fn convert(
        &mut self,
        operand: hir::Operand,
        source: hir::TypeId,
        target: hir::TypeId,
    ) -> Result<hir::Operand, RaiseError> {
        self.coerce(operand, source, target)
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
        self.current_mut().instructions.push(hir::Instruction {
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
        "TY_INT_4" => Ok(I32_TYPE),
        "TY_UINT_2" | "TY_UNSIGNED" => Ok(U16_TYPE),
        "TY_UINT_4" => Ok(U32_TYPE),
        _ => Err(error(
            location,
            RaiseErrorKind::UnsupportedType {
                name: name.to_owned(),
            },
        )),
    }
}

fn capture_type(
    unit: &CaptureUnit,
    types: &WccTypes,
    name: &str,
    location: SourceLocation,
) -> Result<hir::TypeId, RaiseError> {
    if unit.canonical_type(name) == "TY_POINTER" {
        return types.near_pointer.ok_or_else(|| {
            error(
                location,
                RaiseErrorKind::UnsupportedType {
                    name: name.to_owned(),
                },
            )
        });
    }
    value_type(unit, name, location).or_else(|_| {
        types
            .aggregates
            .get(unit.canonical_type(name).as_str())
            .copied()
            .ok_or_else(|| {
                error(
                    location,
                    RaiseErrorKind::UnsupportedType {
                        name: name.to_owned(),
                    },
                )
            })
    })
}

fn type_width(
    types: &[hir::Type],
    type_id: hir::TypeId,
    location: SourceLocation,
) -> Result<usize, RaiseError> {
    types
        .iter()
        .find(|type_| type_.id == type_id)
        .map(|type_| type_.width)
        .ok_or_else(|| {
            error(
                location,
                RaiseErrorKind::UnsupportedType {
                    name: format!("missing HIR type {type_id}"),
                },
            )
        })
}

fn is_integer_type(type_id: hir::TypeId) -> bool {
    matches!(type_id, I16_TYPE | I32_TYPE | U16_TYPE | U32_TYPE)
}

/// A constant conversion is a value fact. Nonconstants remain explicit HIR
/// conversions so lowering chooses sign or zero extension from the source type.
fn wrap_integer(value: i64, target: hir::TypeId) -> i64 {
    let bits = match target {
        I16_TYPE | U16_TYPE => 16,
        I32_TYPE | U32_TYPE => 32,
        _ => unreachable!("constant folding is restricted to integer scalar targets"),
    };
    let modulus = 1_i64 << bits;
    let wrapped = value & (modulus - 1);
    if matches!(target, I16_TYPE | I32_TYPE) && wrapped & (modulus >> 1) != 0 {
        wrapped - modulus
    } else {
        wrapped
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
    use std::collections::BTreeSet;

    use super::{RaiseError, RaiseErrorKind, U16_TYPE, U32_TYPE, WCC_BIG_DATA, raise_module};
    use crate::frontend::wcc::{capture, parse};
    use crate::hir;
    use crate::ir;

    fn iparg() -> capture::CaptureUnit {
        capture::build(&parse(include_str!("../../../fixtures/c/iparg.cgs")).unwrap()).unwrap()
    }

    fn parity_scalar() -> capture::CaptureUnit {
        capture::build(&parse(include_str!("../../../fixtures/c/parity/scalar.cgs")).unwrap())
            .unwrap()
    }

    fn unsigned() -> capture::CaptureUnit {
        capture::build(&parse(include_str!("../../../fixtures/c/unsigned.cgs")).unwrap()).unwrap()
    }

    fn cells() -> capture::CaptureUnit {
        capture::build(&parse(include_str!("../../../fixtures/c/cells.cgs")).unwrap()).unwrap()
    }

    fn parity() -> capture::CaptureUnit {
        capture::build(&parse(include_str!("../../../fixtures/c/parity/parity.cgs")).unwrap())
            .unwrap()
    }

    fn near_pointer_arithmetic(first_operation: &str) -> capture::CaptureUnit {
        // Minimal WCC records for `index + cells` followed by
        // `(cells + 6) - index`.  They isolate the pointer forms WCC emits
        // while retaining the same CGBinary/CGUnary capture representation as
        // the recorded C fixtures.
        let capture = format!(
            r#"INIT sw=0x808000 target=0xec size=50 rev=0x23
SEG 1 attr=0x7 name="_TEXT" align=1
SEG 2 attr=0x1c name="CONST" align=2
SEG 3 attr=0xc name="CONST2" align=2
SEG 4 attr=0x6 name="_DATA" align=2
START
f1 DBSrcFile "near_pointer_arithmetic.c"
SYM y1 name="near_pointer_arithmetic" base="near_pointer_arithmetic" pattern="_*" attr=0x7 seg=1
CALLCONV y1 class=0x80 target=0x716 parms=[] ret=0:0
- CGProcDecl y1 TY_INTEGER
l2 CGLastParm
TYPE T25 size=8 align=2
SYM y3 name="cells" base="cells" pattern="_*" attr=0x0 seg=2
- CGAutoDecl y3 T25
SYM y4 name="index" base="index" pattern="_*" attr=0x0 seg=2
- CGAutoDecl y4 TY_INTEGER
n6 CGFEName y3 T25
n7 CGFEName y4 TY_INTEGER
n8 CGUnary O_POINTS n7 TY_INTEGER
n9 CGBinary {first_operation} n8 n6 TY_POINTER
- CGDone n9
n10 CGFEName y3 T25
n11 CGInteger 6 TY_INTEGER
n12 CGBinary O_PLUS n10 n11 TY_POINTER
n13 CGFEName y4 TY_INTEGER
n14 CGUnary O_POINTS n13 TY_INTEGER
n15 CGBinary O_MINUS n12 n14 TY_POINTER
- CGDone n15
n16 CGInteger 0 TY_INTEGER
- CGReturn n16 TY_INTEGER
STOP
FINI
"#
        );
        capture::build(&parse(&capture).unwrap()).unwrap()
    }

    #[test]
    fn raises_real_short_cells_by_extent_and_typed_dereference() {
        // Python qbopt/cfront/raise_hir.py::_Raise.points and ::assign use
        // cells.cgs's WCC T-record extent, O_PLUS, CGAssign, and O_POINTS.
        let module = raise_module(&cells(), "cells").unwrap();
        let function = &module.functions[0];
        let cells = function
            .places
            .iter()
            .find(|place| place.name == "cells")
            .expect("captured automatic cells place");

        assert!(matches!(
            &module.types[cells.type_id.get() as usize],
            hir::Type {
                kind: hir::TypeKind::Opaque,
                width: 8,
                element: None,
                bounds,
                ..
            } if bounds.is_empty()
        ));
        assert_eq!(cells.extent, 8);
        let dynamic_offset = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match instruction {
                hir::Instruction {
                    opcode: hir::Opcode::OffsetPointer,
                    operands,
                    ..
                } => match operands.as_slice() {
                    [hir::Operand::Value(_), hir::Operand::Value(offset)] => Some(*offset),
                    _ => None,
                },
                _ => None,
            })
            .expect("dynamic cells access has a byte offset");
        assert!(function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction,
                hir::Instruction {
                    opcode: hir::Opcode::Multiply,
                    results,
                    operands,
                    ..
                } if results == &vec![dynamic_offset]
                    && matches!(operands.as_slice(), [hir::Operand::Value(_), hir::Operand::Constant {
                        type_id,
                        value: hir::ConstantValue::Integer(2),
                    }] if *type_id == hir::TypeId::new(1))
            )));
        assert!(function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction,
                hir::Instruction {
                    opcode: hir::Opcode::OffsetPointer,
                    operands,
                    ..
                } if matches!(operands.as_slice(), [hir::Operand::Value(_), hir::Operand::Constant {
                    type_id,
                    value: hir::ConstantValue::Integer(6),
                }] if *type_id == hir::TypeId::new(1))
            )));
        assert!(function.blocks.iter().flat_map(|block| &block.instructions).any(
            |instruction| matches!(
                instruction,
                hir::Instruction {
                    opcode: hir::Opcode::Store,
                    operands,
                    ..
                } if matches!(operands.first(), Some(hir::Operand::Indirect { type_id, offset: 0, .. }) if *type_id == hir::TypeId::new(1))
            )
        ));
        assert!(function.blocks.iter().flat_map(|block| &block.instructions).any(
            |instruction| matches!(
                instruction,
                hir::Instruction {
                    opcode: hir::Opcode::Load,
                    operands,
                    ..
                } if matches!(operands.as_slice(), [hir::Operand::Indirect { type_id, offset: 0, .. }] if *type_id == hir::TypeId::new(1))
            )
        ));
        assert!(module.verify().is_ok());

        let lowered = hir::lower_to_ir(&module).unwrap();
        assert!(
            lowered.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::StackAlloc {
                        size: 8,
                        alignment: 1,
                        ..
                    }
                ))
        );
        assert!(
            lowered.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::GetElementPointer { .. }
                ))
        );
        assert!(
            lowered.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    &instruction.kind,
                    ir::InstructionKind::GetElementPointer { indices, .. }
                        if matches!(indices.as_slice(), [ir::Operand::Constant(ir::TypedConstant {
                            type_id,
                            value: ir::Constant::Integer(6),
                        })] if *type_id == ir::TypeId::new(1))
                ))
        );
        assert!(
            lowered.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(instruction.kind, ir::InstructionKind::Store { .. }))
        );
        assert!(
            lowered.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction,
                    ir::Instruction {
                        results,
                        kind: ir::InstructionKind::Load { .. },
                        ..
                    } if matches!(results.as_slice(), [ir::Value { type_id, .. }] if *type_id == ir::TypeId::new(1))
                ))
        );
    }

    #[test]
    fn raises_real_pair_array_fields_through_chained_near_offsets() {
        // Python _Raise.binary/_Raise.offset treat the second O_PLUS in
        // points[index].x and .y as another byte offset of the first pointer.
        let module = raise_module(&parity(), "parity").unwrap();
        let function = &module.functions[0];
        let points = function
            .places
            .iter()
            .find(|place| place.name == "points")
            .expect("captured points place");
        assert!(matches!(
            &module.types[points.type_id.get() as usize],
            hir::Type {
                kind: hir::TypeKind::Opaque,
                width: 32,
                element: None,
                bounds,
                ..
            } if bounds.is_empty()
        ));
        assert_eq!(points.extent, 32);

        let instructions = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let indexed = instructions
            .iter()
            .filter_map(|instruction| match instruction {
                hir::Instruction {
                    opcode: hir::Opcode::OffsetPointer,
                    results,
                    operands,
                    ..
                } if matches!(operands.as_slice(), [hir::Operand::Value(_), hir::Operand::Value(offset)] if instructions.iter().any(|candidate| matches!(
                    candidate,
                    hir::Instruction {
                        opcode: hir::Opcode::Multiply,
                        results,
                        operands,
                        ..
                    } if results == &vec![*offset]
                        && matches!(operands.as_slice(), [hir::Operand::Value(_), hir::Operand::Constant {
                            type_id,
                            value: hir::ConstantValue::Integer(4),
                        }] if *type_id == hir::TypeId::new(1))
                ))) => results.first().copied(),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert!(
            !indexed.is_empty(),
            "index * 4 must feed an aggregate byte offset"
        );
        for field_offset in [0, 2] {
            assert!(instructions.iter().any(|instruction| matches!(
                instruction,
                hir::Instruction {
                    opcode: hir::Opcode::OffsetPointer,
                    operands,
                    ..
                } if matches!(operands.as_slice(), [hir::Operand::Value(base), hir::Operand::Constant {
                    value: hir::ConstantValue::Integer(offset),
                    ..
                }] if *offset == field_offset && indexed.contains(base))
            )));
        }
        assert!(instructions.iter().any(|instruction| matches!(
            instruction,
            hir::Instruction {
                opcode: hir::Opcode::Store,
                operands,
                ..
            } if matches!(operands.first(), Some(hir::Operand::Indirect { type_id, offset: 0, .. }) if *type_id == hir::TypeId::new(1))
        )));
        assert!(instructions.iter().any(|instruction| matches!(
            instruction,
            hir::Instruction {
                opcode: hir::Opcode::Load,
                operands,
                ..
            } if matches!(operands.as_slice(), [hir::Operand::Indirect { type_id, offset: 0, .. }] if *type_id == hir::TypeId::new(1))
        )));
        assert!(module.verify().is_ok());

        let lowered = hir::lower_to_ir(&module).unwrap();
        let ir_instructions = lowered.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let indexed = ir_instructions
            .iter()
            .filter_map(|instruction| match &instruction.kind {
                ir::InstructionKind::GetElementPointer { indices, .. }
                    if matches!(indices.as_slice(), [ir::Operand::Value(_)]) =>
                {
                    instruction.results.first().map(|result| result.id)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        for field_offset in [0, 2] {
            assert!(ir_instructions.iter().any(|instruction| matches!(
                &instruction.kind,
                ir::InstructionKind::GetElementPointer {
                    base: ir::Operand::Value(base),
                    indices,
                } if indexed.contains(base)
                    && matches!(indices.as_slice(), [ir::Operand::Constant(ir::TypedConstant {
                        value: ir::Constant::Integer(offset),
                        ..
                    })] if *offset == field_offset)
            )));
        }
    }

    #[test]
    fn raises_commuted_near_add_and_left_near_subtract_in_source_order() {
        // Python _Raise.binary first evaluates both operands, then swaps only
        // O_PLUS with an address on the right; _Raise.offset negates a dynamic
        // byte offset for address-left O_MINUS.
        let module = raise_module(&near_pointer_arithmetic("O_PLUS"), "near-arithmetic").unwrap();
        let instructions = &module.functions[0].blocks[0].instructions;
        let opcodes = instructions
            .iter()
            .map(|instruction| instruction.opcode)
            .collect::<Vec<_>>();
        assert_eq!(
            opcodes,
            vec![
                hir::Opcode::Load,
                hir::Opcode::Address,
                hir::Opcode::OffsetPointer,
                hir::Opcode::Address,
                hir::Opcode::OffsetPointer,
                hir::Opcode::Load,
                hir::Opcode::Negate,
                hir::Opcode::OffsetPointer,
            ]
        );
        assert!(module.verify().is_ok());

        let lowered = hir::lower_to_ir(&module).unwrap();
        assert_eq!(
            lowered.functions[0].blocks[0]
                .instructions
                .iter()
                .filter(|instruction| {
                    matches!(
                        instruction.kind,
                        ir::InstructionKind::GetElementPointer { .. }
                    )
                })
                .count(),
            3
        );

        let error =
            raise_module(&near_pointer_arithmetic("O_MINUS"), "near-arithmetic").unwrap_err();
        assert!(matches!(
            error.kind,
            RaiseErrorKind::InvalidNode { node, .. } if node == capture::NodeId::new(9)
        ));
    }

    #[test]
    fn refuses_to_narrow_a_big_data_pointer_to_near() {
        // Python _Raise.width/far_pointer make TY_POINTER four bytes and far
        // when WCC's BIG_DATA target bit is present. Until far pointers are
        // ported, the Rust frontend must refuse instead of changing its ABI.
        let mut unit = cells();
        unit.target |= WCC_BIG_DATA;

        assert!(matches!(
            raise_module(&unit, "cells"),
            Err(RaiseError {
                kind: RaiseErrorKind::UnsupportedType { name },
                ..
            }) if name.contains("big-data")
        ));
    }

    #[test]
    fn raises_real_unsigned_scalars_with_unsigned_lowering() {
        let module = raise_module(&unsigned(), "unsigned").unwrap();

        assert!(matches!(
            module.types[U16_TYPE.get() as usize],
            hir::Type {
                kind: hir::TypeKind::Integer,
                width: 2,
                signed: Some(false),
                ..
            }
        ));
        assert!(matches!(
            module.types[U32_TYPE.get() as usize],
            hir::Type {
                kind: hir::TypeKind::Integer,
                width: 4,
                signed: Some(false),
                ..
            }
        ));
        assert!(module.verify().is_ok());

        let lowered = hir::lower_to_ir(&module).unwrap();
        let instructions = lowered
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .map(|instruction| &instruction.kind)
            .collect::<Vec<_>>();
        assert!(instructions.iter().any(|kind| matches!(
            kind,
            ir::InstructionKind::Cast {
                op: ir::CastOp::ZeroExtend,
                ..
            }
        )));
        assert!(instructions.iter().any(|kind| matches!(
            kind,
            ir::InstructionKind::Binary {
                op: ir::BinaryOp::UnsignedDivide,
                ..
            }
        )));
        assert!(instructions.iter().any(|kind| matches!(
            kind,
            ir::InstructionKind::Compare {
                predicate: ir::ComparePredicate::UnsignedLessThan,
                ..
            }
        )));
    }

    #[test]
    fn raises_the_real_scalar_capture_with_typed_cells_and_loop_control() {
        let module = raise_module(&parity_scalar(), "parity_scalar").unwrap();
        let function = &module.functions[0];

        assert_eq!(module.types.len(), 6);
        assert_eq!(function.name, "_parity_scalar");
        assert_eq!(function.result_type, hir::TypeId::new(2));
        assert_eq!(
            function
                .places
                .iter()
                .map(|place| (place.name.as_str(), place.type_id, place.extent))
                .collect::<Vec<_>>(),
            vec![
                ("temporary3", hir::TypeId::new(2), 4),
                ("total", hir::TypeId::new(2), 4),
                ("index", hir::TypeId::new(1), 2),
            ]
        );
        assert_eq!(function.blocks.len(), 4);
        assert_eq!(
            function.blocks[0].terminator,
            hir::Terminator::Jump(hir::BlockId::new(1))
        );
        assert!(
            matches!(
                function.blocks[0].instructions.first(),
                Some(hir::Instruction {
                    opcode: hir::Opcode::Store,
                    operands,
                    ..
                }) if matches!(
                    operands.as_slice(),
                    [
                        hir::Operand::Place(place),
                        hir::Operand::Constant {
                            type_id,
                            value: hir::ConstantValue::Integer(17),
                        },
                    ] if *place == hir::PlaceId::new(1) && *type_id == hir::TypeId::new(2)
                )
            ),
            "Python FunctionRaiser.convert folds the widening of integer constants"
        );
        let comparison = function.blocks[1]
            .instructions
            .iter()
            .find(|instruction| instruction.opcode == hir::Opcode::LessThan)
            .and_then(|instruction| instruction.results.first())
            .copied()
            .expect("loop header has a comparison result");
        assert_eq!(
            function.blocks[1].terminator,
            hir::Terminator::Branch {
                condition: hir::Operand::Value(comparison),
                then_block: hir::BlockId::new(3),
                else_block: hir::BlockId::new(2),
            },
            "Python FunctionRaiser.branch sends O_IF_FALSE to l5 and falls through to the loop body"
        );
        assert_eq!(
            function.blocks[3].terminator,
            hir::Terminator::Jump(hir::BlockId::new(1))
        );
        let opcodes = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .map(|instruction| instruction.opcode)
            .collect::<Vec<_>>();
        assert!(opcodes.contains(&hir::Opcode::Load));
        assert!(opcodes.contains(&hir::Opcode::Store));
        assert!(opcodes.contains(&hir::Opcode::Convert));
        assert!(opcodes.contains(&hir::Opcode::Add));
        assert!(opcodes.contains(&hir::Opcode::Subtract));
        assert!(opcodes.contains(&hir::Opcode::Multiply));
        assert!(opcodes.contains(&hir::Opcode::LessThan));
        assert!(
            function
                .blocks
                .iter()
                .any(|block| matches!(block.terminator, hir::Terminator::Jump(_)))
        );
        assert!(function.blocks.iter().any(|block| matches!(
            block.terminator,
            hir::Terminator::Return(Some(hir::Operand::Value(_)))
        )));
        assert!(module.verify().is_ok());

        let lowered = hir::lower_to_ir(&module).unwrap();
        let lowered_function = &lowered.functions[0];
        assert_eq!(lowered_function.blocks.len(), 4);
        assert!(
            lowered_function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::StackAlloc { size: 4, .. }
                ))
        );
        assert!(
            lowered_function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::Compare {
                        predicate: ir::ComparePredicate::SignedLessThan,
                        ..
                    }
                ))
        );
        assert!(
            lowered_function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    ir::InstructionKind::Cast {
                        op: ir::CastOp::SignExtend,
                        ..
                    }
                ))
        );
    }

    #[test]
    fn raises_the_real_iparg_capture_to_generic_hir() {
        let module = raise_module(&iparg(), "iparg").unwrap();

        assert_eq!(module.types.len(), 6);
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "_twice");
        assert_eq!(module.functions[0].linkage, hir::Linkage::Internal);
        assert_eq!(module.functions[0].abi.cleanup, hir::StackCleanup::Caller);
        assert_eq!(module.functions[0].abi.distance, hir::CallDistance::Near);
        assert_eq!(module.functions[0].parameters, [hir::ValueId::new(0)]);
        assert!(matches!(
            module.functions[0].blocks[0].instructions.iter().find(|instruction| matches!(instruction.opcode, hir::Opcode::Multiply)),
            Some(hir::Instruction {
                opcode: hir::Opcode::Multiply,
                results,
                operands,
                ..
            }) if results == &vec![hir::ValueId::new(1)]
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
        unit.nodes.get_mut(&capture::NodeId::new(7)).unwrap().args[0] = "O_UNKNOWN".into();

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
