//! Typed, source-ordered model of an Open Watcom code-generator capture.
//!
//! This is deliberately a WCC frontend representation. Raw `CG*`, `DG*`,
//! `TY_*`, naming, segment, and calling-convention facts are resolved here or
//! by the later WCC raiser; they are not portable HIR, MIR, or MC concepts.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::Record;

const FE_PROC: u32 = 0x1;
const FE_CONSTANT: u32 = 0x10;
const FE_VOLATILE: u32 = 0x800;
const FE_INTERNAL: u32 = 0x1000;
const FE_GLOBAL: u32 = 0x4;
const FE_IMPORT: u32 = 0x8;
const PRIVATE_SEGMENT: u32 = 0x40;
const FAR_CALL: u32 = 0x4;

macro_rules! capture_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

capture_id!(SegmentId);
capture_id!(SymbolId);
capture_id!(BackId);
capture_id!(NodeId);
capture_id!(CallId);
capture_id!(TempId);
capture_id!(SourceFileId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolAttributes(u32);

impl SymbolAttributes {
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is_procedure(self) -> bool {
        self.0 & FE_PROC != 0
    }

    pub const fn is_constant(self) -> bool {
        self.0 & FE_CONSTANT != 0
    }

    pub const fn is_volatile(self) -> bool {
        self.0 & FE_VOLATILE != 0
    }

    pub const fn is_internal(self) -> bool {
        self.0 & FE_INTERNAL != 0
    }

    pub const fn is_imported(self) -> bool {
        self.0 & FE_IMPORT != 0
    }

    pub const fn is_exported(self) -> bool {
        self.0 & FE_GLOBAL != 0 && !self.is_imported()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallConvention {
    pub class: u32,
    pub target: u32,
    pub parameters: String,
}

impl CallConvention {
    pub fn has_register_parameters(&self) -> bool {
        self.parameters != "[]"
    }

    pub const fn is_far(&self) -> bool {
        self.target & FAR_CALL != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineFixupKind {
    Offset,
    Segment,
    RelativeOffset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineFixup {
    pub offset: u32,
    pub kind: InlineFixupKind,
    pub symbol: SymbolId,
    pub addend: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineCode {
    pub bytes: Vec<u8>,
    pub fixups: Vec<InlineFixup>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub base: String,
    pub pattern: String,
    pub attributes: SymbolAttributes,
    pub segment: i32,
    pub convention: Option<CallConvention>,
    pub code: Option<InlineCode>,
}

impl Symbol {
    pub fn object_name(&self) -> String {
        if self.pattern == "^" {
            return self.base.to_uppercase();
        }
        let base = match self.base.as_str() {
            "__inportb__" => "inportb",
            "__inportw__" => "inport",
            "__outportb__" => "outportb",
            "__outportw__" => "outport",
            _ => &self.base,
        };
        if self.pattern.is_empty() {
            base.to_owned()
        } else {
            self.pattern.replace('*', base)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackTarget {
    Literal,
    Symbol(SymbolId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub call: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingCall {
    pub id: CallId,
    pub target: NodeId,
    pub value_type: String,
    pub symbol: SymbolId,
    /// Capture order is significant: WCC records the last source argument first.
    pub parameters: Vec<(NodeId, String)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutomaticId {
    Symbol(SymbolId),
    Temporary(TempId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceLocation {
    pub file: Option<SourceFileId>,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Statement {
    pub call: String,
    pub args: Vec<String>,
    pub location: SourceLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub symbol: SymbolId,
    pub value_type: String,
    pub parameters: Vec<(SymbolId, String)>,
    pub automatics: Vec<(AutomaticId, String)>,
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataItemKind {
    Label,
    BackPointer,
    FrontendPointer,
    Integer,
    Integer64,
    Float,
    Bytes,
    RepeatedByte,
    UninitializedBytes,
    Align,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataItem {
    pub kind: DataItemKind,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub id: SegmentId,
    pub name: String,
    pub attributes: u32,
    pub alignment: Option<u32>,
    pub items: Vec<DataItem>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CaptureUnit {
    pub target: u32,
    pub segments: BTreeMap<SegmentId, Segment>,
    pub segment_order: Vec<SegmentId>,
    pub symbols: BTreeMap<SymbolId, Symbol>,
    pub symbol_order: Vec<SymbolId>,
    pub backs: BTreeMap<BackId, BackTarget>,
    pub types: BTreeMap<String, u32>,
    pub aliases: BTreeMap<String, String>,
    pub nodes: BTreeMap<NodeId, Node>,
    pub node_order: Vec<NodeId>,
    pub calls: BTreeMap<CallId, PendingCall>,
    pub call_order: Vec<CallId>,
    pub procedures: Vec<Procedure>,
    pub source_files: BTreeMap<SourceFileId, String>,
}

impl CaptureUnit {
    pub fn canonical_type(&self, name: &str) -> String {
        let mut current = name;
        let mut seen = BTreeSet::new();
        while let Some(next) = self.aliases.get(current) {
            if !seen.insert(current) {
                break;
            }
            current = next;
        }
        current.to_owned()
    }

    pub fn is_grouped(&self, symbol: &Symbol) -> bool {
        if symbol.attributes.is_imported() && symbol.segment < 0 {
            return false;
        }
        u32::try_from(symbol.segment)
            .ok()
            .and_then(|segment| self.segments.get(&SegmentId::new(segment)))
            .is_none_or(|segment| segment.attributes & PRIVATE_SEGMENT == 0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildError {
    pub line: usize,
    pub kind: BuildErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildErrorKind {
    MissingArgument { call: String, index: usize },
    MissingField { call: String, field: &'static str },
    MissingResult { call: String },
    InvalidNumber { value: String },
    InvalidHandle { expected: char, value: String },
    InvalidBytes { value: String },
    InvalidInlineFixup { value: String },
    Duplicate { entity: &'static str, id: u32 },
    UnknownSegment(SegmentId),
    UnknownSymbol(SymbolId),
    UnknownNode(NodeId),
    UnknownCall(CallId),
    UnknownSourceFile(SourceFileId),
    DataWithoutSegment,
    OutsideProcedure { call: String },
    SymbolIsNotProcedure(SymbolId),
    ShimRefused { detail: String },
    UnsupportedRecord { call: String },
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "WCC capture line {}: ", self.line)?;
        match &self.kind {
            BuildErrorKind::MissingArgument { call, index } => {
                write!(formatter, "{call} is missing argument {index}")
            }
            BuildErrorKind::MissingField { call, field } => {
                write!(formatter, "{call} is missing field {field}")
            }
            BuildErrorKind::MissingResult { call } => {
                write!(formatter, "{call} has no result handle")
            }
            BuildErrorKind::InvalidNumber { value } => {
                write!(formatter, "invalid number {value:?}")
            }
            BuildErrorKind::InvalidHandle { expected, value } => {
                write!(formatter, "expected {expected}-handle, got {value:?}")
            }
            BuildErrorKind::InvalidBytes { value } => {
                write!(formatter, "invalid byte string {value:?}")
            }
            BuildErrorKind::InvalidInlineFixup { value } => {
                write!(formatter, "invalid inline-code fixup {value:?}")
            }
            BuildErrorKind::Duplicate { entity, id } => {
                write!(formatter, "duplicate {entity} {id}")
            }
            BuildErrorKind::UnknownSegment(id) => write!(formatter, "unknown segment {id}"),
            BuildErrorKind::UnknownSymbol(id) => write!(formatter, "unknown symbol {id}"),
            BuildErrorKind::UnknownNode(id) => write!(formatter, "unknown node {id}"),
            BuildErrorKind::UnknownCall(id) => write!(formatter, "unknown call {id}"),
            BuildErrorKind::UnknownSourceFile(id) => write!(formatter, "unknown source file {id}"),
            BuildErrorKind::DataWithoutSegment => {
                formatter.write_str("data before any selected segment")
            }
            BuildErrorKind::OutsideProcedure { call } => {
                write!(formatter, "{call} outside a procedure")
            }
            BuildErrorKind::SymbolIsNotProcedure(id) => {
                write!(formatter, "symbol {id} is not a procedure")
            }
            BuildErrorKind::ShimRefused { detail } => write!(formatter, "shim refused {detail}"),
            BuildErrorKind::UnsupportedRecord { call } => {
                write!(formatter, "unsupported record {call}")
            }
        }
    }
}

impl Error for BuildError {}

/// Build the WCC capture model used by the later semantic raiser.
pub fn build(records: &[Record]) -> Result<CaptureUnit, BuildError> {
    let mut unit = CaptureUnit::default();
    let mut procedure = None;
    let mut segment = None;
    let mut location = SourceLocation::default();

    for record in records {
        match record.call.as_str() {
            "UNSUPPORTED" => {
                return Err(error(
                    record,
                    BuildErrorKind::ShimRefused {
                        detail: record.args.join(" "),
                    },
                ));
            }
            "INIT" => unit.target = hex_field(record, "target")?,
            "SEG" => define_segment(&mut unit, record)?,
            "SETSEG" => {
                let raw = decimal_arg::<u32>(record, 0)?;
                if raw == 0 {
                    segment = None;
                } else {
                    let id = SegmentId::new(raw);
                    if !unit.segments.contains_key(&id) {
                        return Err(error(record, BuildErrorKind::UnknownSegment(id)));
                    }
                    segment = Some(id);
                }
            }
            "TYPE" => {
                let name = arg(record, 0)?.to_owned();
                let size = decimal_field::<u32>(record, "size")?;
                if unit.types.insert(name, size).is_some() {
                    return Err(duplicate(record, "type", 0));
                }
            }
            "ALIAS" => {
                let name = arg(record, 0)?.to_owned();
                let target = arg(record, 1)?.to_owned();
                if unit.aliases.insert(name, target).is_some() {
                    return Err(duplicate(record, "alias", 0));
                }
            }
            "SYM" => define_symbol(&mut unit, record)?,
            "CALLCONV" => attach_calling_convention(&mut unit, record)?,
            "CODE" => attach_inline_code(&mut unit, record)?,
            "BENewBack" => define_back(&mut unit, record)?,
            "CGProcDecl" => {
                let symbol = symbol_arg(record, 0)?;
                let Some(declaration) = unit.symbols.get(&symbol) else {
                    return Err(error(record, BuildErrorKind::UnknownSymbol(symbol)));
                };
                if !declaration.attributes.is_procedure() {
                    return Err(error(record, BuildErrorKind::SymbolIsNotProcedure(symbol)));
                }
                unit.procedures.push(Procedure {
                    symbol,
                    value_type: arg(record, 1)?.to_owned(),
                    parameters: Vec::new(),
                    automatics: Vec::new(),
                    body: Vec::new(),
                });
                procedure = Some(unit.procedures.len() - 1);
            }
            "CGParmDecl" => {
                let symbol = symbol_arg(record, 0)?;
                require_symbol(&unit, record, symbol)?;
                current_procedure(&mut unit, procedure, record)?
                    .parameters
                    .push((symbol, arg(record, 1)?.to_owned()));
            }
            "CGAutoDecl" => {
                let symbol = symbol_arg(record, 0)?;
                require_symbol(&unit, record, symbol)?;
                current_procedure(&mut unit, procedure, record)?
                    .automatics
                    .push((AutomaticId::Symbol(symbol), arg(record, 1)?.to_owned()));
            }
            "CGTemp" => {
                let id = TempId::new(result_handle(record, 't')?);
                current_procedure(&mut unit, procedure, record)?
                    .automatics
                    .push((AutomaticId::Temporary(id), arg(record, 0)?.to_owned()));
            }
            "CGInitCall" => define_call(&mut unit, procedure, record)?,
            "CGAddParm" => add_call_parameter(&mut unit, procedure, record)?,
            "DBSrcFile" => {
                let id = SourceFileId::new(result_handle(record, 'f')?);
                if unit
                    .source_files
                    .insert(id, arg(record, 0)?.to_owned())
                    .is_some()
                {
                    return Err(duplicate(record, "source file", id.get()));
                }
            }
            "DBSrcCue" => {
                let file = SourceFileId::new(handle_arg(record, 0, 'f')?);
                if !unit.source_files.contains_key(&file) {
                    return Err(error(record, BuildErrorKind::UnknownSourceFile(file)));
                }
                location = SourceLocation {
                    file: Some(file),
                    line: decimal_arg(record, 1)?,
                    column: decimal_arg(record, 2)?,
                };
            }
            "CGSelInit" => {
                let result = record.result.clone().ok_or_else(|| {
                    error(
                        record,
                        BuildErrorKind::MissingResult {
                            call: record.call.clone(),
                        },
                    )
                })?;
                push_statement(&mut unit, procedure, record, vec![result], location)?;
            }
            call if is_statement(call) => {
                push_statement(&mut unit, procedure, record, record.args.clone(), location)?
            }
            call if call.starts_with("DG") => {
                let Some(id) = segment else {
                    return Err(error(record, BuildErrorKind::DataWithoutSegment));
                };
                let Some(segment) = unit.segments.get_mut(&id) else {
                    return Err(error(record, BuildErrorKind::UnknownSegment(id)));
                };
                segment.items.push(DataItem {
                    kind: data_kind(call),
                    args: record.args.clone(),
                });
            }
            call if is_ignored(call) => {}
            _ if record.result.as_deref().is_some_and(is_node_handle) => {
                let id = NodeId::new(result_handle(record, 'n')?);
                if unit.nodes.contains_key(&id) {
                    return Err(duplicate(record, "node", id.get()));
                }
                unit.node_order.push(id);
                unit.nodes.insert(
                    id,
                    Node {
                        id,
                        call: record.call.clone(),
                        args: record.args.clone(),
                    },
                );
            }
            _ => {
                return Err(error(
                    record,
                    BuildErrorKind::UnsupportedRecord {
                        call: record.call.clone(),
                    },
                ));
            }
        }
    }
    Ok(unit)
}

fn define_segment(unit: &mut CaptureUnit, record: &Record) -> Result<(), BuildError> {
    let id = SegmentId::new(decimal_arg(record, 0)?);
    if unit.segments.contains_key(&id) {
        return Err(duplicate(record, "segment", id.get()));
    }
    let segment = Segment {
        id,
        name: field(record, "name")?.to_owned(),
        attributes: hex_field(record, "attr")?,
        alignment: record
            .fields
            .get("align")
            .map(|value| decimal(record, value))
            .transpose()?,
        items: Vec::new(),
    };
    unit.segment_order.push(id);
    unit.segments.insert(id, segment);
    Ok(())
}

fn define_symbol(unit: &mut CaptureUnit, record: &Record) -> Result<(), BuildError> {
    let id = symbol_arg(record, 0)?;
    if unit.symbols.contains_key(&id) {
        return Err(duplicate(record, "symbol", id.get()));
    }
    let symbol = Symbol {
        id,
        name: field(record, "name")?.to_owned(),
        base: field(record, "base")?.to_owned(),
        pattern: field(record, "pattern")?.to_owned(),
        attributes: SymbolAttributes::from_bits(hex_field(record, "attr")?),
        segment: record
            .fields
            .get("seg")
            .map(|value| decimal(record, value))
            .transpose()?
            .unwrap_or(0),
        convention: None,
        code: None,
    };
    unit.symbol_order.push(id);
    unit.symbols.insert(id, symbol);
    Ok(())
}

fn attach_calling_convention(unit: &mut CaptureUnit, record: &Record) -> Result<(), BuildError> {
    let id = symbol_arg(record, 0)?;
    let convention = CallConvention {
        class: hex_field(record, "class")?,
        target: hex_field(record, "target")?,
        parameters: record
            .fields
            .get("parms")
            .cloned()
            .unwrap_or_else(|| "[]".to_owned()),
    };
    let Some(symbol) = unit.symbols.get_mut(&id) else {
        return Err(error(record, BuildErrorKind::UnknownSymbol(id)));
    };
    if symbol.convention.replace(convention).is_some() {
        return Err(duplicate(record, "calling convention", id.get()));
    }
    Ok(())
}

fn attach_inline_code(unit: &mut CaptureUnit, record: &Record) -> Result<(), BuildError> {
    let id = symbol_arg(record, 0)?;
    require_symbol(unit, record, id)?;
    let bytes_text = field(record, "bytes")?;
    let bytes = decode_hex(bytes_text).ok_or_else(|| {
        error(
            record,
            BuildErrorKind::InvalidBytes {
                value: bytes_text.to_owned(),
            },
        )
    })?;
    let mut fixups = Vec::new();
    let fixup_text = field(record, "fix")?;
    if fixup_text != "-" && !fixup_text.is_empty() {
        for raw in fixup_text.split(',') {
            let parts = raw.split(':').collect::<Vec<_>>();
            if parts.len() != 4 {
                return Err(invalid_fixup(record, raw));
            }
            let offset: u32 = decimal(record, parts[0])?;
            let kind = match parts[1] {
                "offset" => InlineFixupKind::Offset,
                "segment" => InlineFixupKind::Segment,
                "reloff" => InlineFixupKind::RelativeOffset,
                _ => return Err(invalid_fixup(record, raw)),
            };
            let symbol = SymbolId::new(parse_handle(record, parts[2], 'y')?);
            require_symbol(unit, record, symbol)?;
            let addend: i32 = decimal(record, parts[3])?;
            if offset
                .checked_add(2)
                .is_none_or(|end| end as usize > bytes.len())
                || fixups.iter().any(|fixup: &InlineFixup| {
                    offset < fixup.offset + 2 && fixup.offset < offset + 2
                })
            {
                return Err(invalid_fixup(record, raw));
            }
            fixups.push(InlineFixup {
                offset,
                kind,
                symbol,
                addend,
            });
        }
    }
    let Some(symbol) = unit.symbols.get_mut(&id) else {
        unreachable!("symbol checked above")
    };
    if symbol.code.replace(InlineCode { bytes, fixups }).is_some() {
        return Err(duplicate(record, "inline code", id.get()));
    }
    Ok(())
}

fn define_back(unit: &mut CaptureUnit, record: &Record) -> Result<(), BuildError> {
    let id = BackId::new(result_handle(record, 'b')?);
    if unit.backs.contains_key(&id) {
        return Err(duplicate(record, "back", id.get()));
    }
    let raw = handle_arg(record, 0, 'y')?;
    let target = if raw == 0 {
        BackTarget::Literal
    } else {
        let symbol = SymbolId::new(raw);
        require_symbol(unit, record, symbol)?;
        BackTarget::Symbol(symbol)
    };
    unit.backs.insert(id, target);
    Ok(())
}

fn define_call(
    unit: &mut CaptureUnit,
    procedure: Option<usize>,
    record: &Record,
) -> Result<(), BuildError> {
    require_procedure(procedure, record)?;
    let id = CallId::new(result_handle(record, 'c')?);
    if unit.calls.contains_key(&id) {
        return Err(duplicate(record, "call", id.get()));
    }
    let target = NodeId::new(handle_arg(record, 0, 'n')?);
    if !unit.nodes.contains_key(&target) {
        return Err(error(record, BuildErrorKind::UnknownNode(target)));
    }
    let symbol = symbol_arg(record, 2)?;
    require_symbol(unit, record, symbol)?;
    unit.call_order.push(id);
    unit.calls.insert(
        id,
        PendingCall {
            id,
            target,
            value_type: arg(record, 1)?.to_owned(),
            symbol,
            parameters: Vec::new(),
        },
    );
    Ok(())
}

fn add_call_parameter(
    unit: &mut CaptureUnit,
    procedure: Option<usize>,
    record: &Record,
) -> Result<(), BuildError> {
    require_procedure(procedure, record)?;
    let call_id = CallId::new(handle_arg(record, 0, 'c')?);
    let node = NodeId::new(handle_arg(record, 1, 'n')?);
    if !unit.nodes.contains_key(&node) {
        return Err(error(record, BuildErrorKind::UnknownNode(node)));
    }
    let value_type = arg(record, 2)?.to_owned();
    let Some(call) = unit.calls.get_mut(&call_id) else {
        return Err(error(record, BuildErrorKind::UnknownCall(call_id)));
    };
    call.parameters.push((node, value_type));
    Ok(())
}

fn push_statement(
    unit: &mut CaptureUnit,
    procedure: Option<usize>,
    record: &Record,
    args: Vec<String>,
    location: SourceLocation,
) -> Result<(), BuildError> {
    current_procedure(unit, procedure, record)?
        .body
        .push(Statement {
            call: record.call.clone(),
            args,
            location,
        });
    Ok(())
}

fn current_procedure<'a>(
    unit: &'a mut CaptureUnit,
    procedure: Option<usize>,
    record: &Record,
) -> Result<&'a mut Procedure, BuildError> {
    let index = require_procedure(procedure, record)?;
    Ok(&mut unit.procedures[index])
}

fn require_procedure(procedure: Option<usize>, record: &Record) -> Result<usize, BuildError> {
    procedure.ok_or_else(|| {
        error(
            record,
            BuildErrorKind::OutsideProcedure {
                call: record.call.clone(),
            },
        )
    })
}

fn require_symbol(unit: &CaptureUnit, record: &Record, id: SymbolId) -> Result<(), BuildError> {
    if unit.symbols.contains_key(&id) {
        Ok(())
    } else {
        Err(error(record, BuildErrorKind::UnknownSymbol(id)))
    }
}

fn is_statement(call: &str) -> bool {
    matches!(
        call,
        "CGDone"
            | "CGTrash"
            | "CGControl"
            | "CGReturn"
            | "CGSelCase"
            | "CGSelRange"
            | "CGSelOther"
            | "CGSelect"
            | "CGBigLabel"
    )
}

fn is_ignored(call: &str) -> bool {
    matches!(
        call,
        "START"
            | "STOP"
            | "FINI"
            | "ABORT"
            | "BENewLabel"
            | "BEFiniLabel"
            | "CGLastParm"
            | "BEFiniBack"
    )
}

fn data_kind(call: &str) -> DataItemKind {
    match call {
        "DGLabel" => DataItemKind::Label,
        "DGBackPtr" => DataItemKind::BackPointer,
        "DGFEPtr" => DataItemKind::FrontendPointer,
        "DGInteger" => DataItemKind::Integer,
        "DGInteger64" => DataItemKind::Integer64,
        "DGFloat" => DataItemKind::Float,
        "DGBytes" => DataItemKind::Bytes,
        "DGIBytes" => DataItemKind::RepeatedByte,
        "DGUBytes" => DataItemKind::UninitializedBytes,
        "DGAlign" => DataItemKind::Align,
        other => DataItemKind::Other(other.to_owned()),
    }
}

fn arg<'a>(record: &'a Record, index: usize) -> Result<&'a str, BuildError> {
    record.args.get(index).map(String::as_str).ok_or_else(|| {
        error(
            record,
            BuildErrorKind::MissingArgument {
                call: record.call.clone(),
                index,
            },
        )
    })
}

fn field<'a>(record: &'a Record, name: &'static str) -> Result<&'a str, BuildError> {
    record.fields.get(name).map(String::as_str).ok_or_else(|| {
        error(
            record,
            BuildErrorKind::MissingField {
                call: record.call.clone(),
                field: name,
            },
        )
    })
}

fn decimal_arg<T: std::str::FromStr>(record: &Record, index: usize) -> Result<T, BuildError> {
    decimal(record, arg(record, index)?)
}

fn decimal_field<T: std::str::FromStr>(
    record: &Record,
    name: &'static str,
) -> Result<T, BuildError> {
    decimal(record, field(record, name)?)
}

fn decimal<T: std::str::FromStr>(record: &Record, value: &str) -> Result<T, BuildError> {
    value.parse().map_err(|_| {
        error(
            record,
            BuildErrorKind::InvalidNumber {
                value: value.to_owned(),
            },
        )
    })
}

fn hex_field(record: &Record, name: &'static str) -> Result<u32, BuildError> {
    let value = field(record, name)?;
    u32::from_str_radix(value.strip_prefix("0x").unwrap_or(value), 16).map_err(|_| {
        error(
            record,
            BuildErrorKind::InvalidNumber {
                value: value.to_owned(),
            },
        )
    })
}

fn symbol_arg(record: &Record, index: usize) -> Result<SymbolId, BuildError> {
    Ok(SymbolId::new(handle_arg(record, index, 'y')?))
}

fn handle_arg(record: &Record, index: usize, prefix: char) -> Result<u32, BuildError> {
    parse_handle(record, arg(record, index)?, prefix)
}

fn result_handle(record: &Record, prefix: char) -> Result<u32, BuildError> {
    let value = record.result.as_deref().ok_or_else(|| {
        error(
            record,
            BuildErrorKind::MissingResult {
                call: record.call.clone(),
            },
        )
    })?;
    parse_handle(record, value, prefix)
}

fn parse_handle(record: &Record, value: &str, prefix: char) -> Result<u32, BuildError> {
    let Some(digits) = value.strip_prefix(prefix) else {
        return Err(error(
            record,
            BuildErrorKind::InvalidHandle {
                expected: prefix,
                value: value.to_owned(),
            },
        ));
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error(
            record,
            BuildErrorKind::InvalidHandle {
                expected: prefix,
                value: value.to_owned(),
            },
        ));
    }
    decimal(record, digits)
}

fn is_node_handle(value: &str) -> bool {
    value.strip_prefix('n').is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&value[at..at + 2], 16).ok())
        .collect()
}

fn invalid_fixup(record: &Record, value: &str) -> BuildError {
    error(
        record,
        BuildErrorKind::InvalidInlineFixup {
            value: value.to_owned(),
        },
    )
}

fn duplicate(record: &Record, entity: &'static str, id: u32) -> BuildError {
    error(record, BuildErrorKind::Duplicate { entity, id })
}

fn error(record: &Record, kind: BuildErrorKind) -> BuildError {
    BuildError {
        line: record.line,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::wcc::parse;

    fn captured(text: &str) -> Result<CaptureUnit, BuildError> {
        build(&parse(text).unwrap())
    }

    #[test]
    fn builds_the_real_choose_capture_in_source_order() {
        let unit = captured(include_str!("../../../fixtures/c/choose.cgs")).unwrap();
        assert_eq!(unit.target, 0xec);
        assert_eq!(
            unit.segment_order,
            [
                SegmentId::new(1),
                SegmentId::new(2),
                SegmentId::new(3),
                SegmentId::new(4)
            ]
        );
        assert_eq!(unit.symbols.len(), 10);
        assert_eq!(unit.backs.len(), 2);
        assert!(
            unit.backs
                .values()
                .all(|target| *target == BackTarget::Literal)
        );
        assert_eq!(unit.nodes.len(), 52);
        assert_eq!(unit.calls.len(), 2);
        assert_eq!(unit.procedures.len(), 3);
        assert_eq!(unit.calls[&CallId::new(41)].parameters.len(), 3);
    }

    #[test]
    fn builds_every_committed_wcc_capture() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/c");
        for name in [
            "anims.cgs",
            "calls.cgs",
            "choose.cgs",
            "control.cgs",
            "crosscall.cgs",
            "fardata.cgs",
            "farderef.cgs",
            "farloadloop.cgs",
            "farptr.cgs",
            "fill.cgs",
            "floats.cgs",
            "halve.cgs",
            "indexed.cgs",
            "inline.cgs",
            "iparg.cgs",
            "ipconst.cgs",
            "longret.cgs",
            "loopaddr.cgs",
            "ls.cgs",
            "mir/euclid64.cgs",
            "mir/fib64.cgs",
            "mir/int64.cgs",
            "mir/mix64.cgs",
            "pal.cgs",
            "pointers.cgs",
            "qglsurf.cgs",
            "regs.cgs",
            "rotate.cgs",
        ] {
            let text = std::fs::read_to_string(root.join(name)).unwrap();
            build(&parse(&text).unwrap()).unwrap_or_else(|error| panic!("{name}: {error}"));
        }
    }

    #[test]
    fn retains_inline_bytes_fixups_and_distinct_object_names() {
        let unit = captured(include_str!("../../../fixtures/c/inline.cgs")).unwrap();
        let symbol = &unit.symbols[&SymbolId::new(5)];
        let code = symbol.code.as_ref().unwrap();
        assert_eq!(symbol.name, "F.0");
        assert_eq!(symbol.object_name(), "F.0_");
        assert_eq!(code.bytes.len(), 16);
        assert_eq!(code.fixups.len(), 4);
        assert_eq!(code.fixups[0].kind, InlineFixupKind::Offset);
        assert_eq!(code.fixups[0].symbol, SymbolId::new(2));
    }

    #[test]
    fn keeps_wcc_naming_grouping_and_alias_cycle_rules_local() {
        let mut unit = CaptureUnit::default();
        unit.aliases.insert("a".into(), "b".into());
        unit.aliases.insert("b".into(), "a".into());
        assert_eq!(unit.canonical_type("a"), "a");
        let intrinsic = Symbol {
            id: SymbolId::new(1),
            name: "input".into(),
            base: "__inportw__".into(),
            pattern: "_*".into(),
            attributes: SymbolAttributes::from_bits(FE_IMPORT),
            segment: -1,
            convention: None,
            code: None,
        };
        assert_eq!(intrinsic.object_name(), "_inport");
        assert!(!unit.is_grouped(&intrinsic));
    }

    #[test]
    fn retains_register_parameter_convention_without_lowering_it() {
        let unit = captured(include_str!("../../../fixtures/c/regs.cgs")).unwrap();
        let convention = unit.symbols[&SymbolId::new(3)].convention.as_ref().unwrap();
        assert_eq!(convention.parameters, "[3:0]");
        assert!(convention.has_register_parameters());
        assert_eq!(convention.class, 0x80);
        assert_eq!(convention.target, 0x717);
    }

    #[test]
    fn refuses_bad_state_references_and_shim_failures() {
        assert!(matches!(
            captured("- DGBytes 1 ff\n"),
            Err(BuildError {
                line: 1,
                kind: BuildErrorKind::DataWithoutSegment
            })
        ));
        assert!(matches!(
            captured("- CGReturn n0 TY_DEFAULT\n"),
            Err(BuildError {
                line: 1,
                kind: BuildErrorKind::OutsideProcedure { .. }
            })
        ));
        assert!(matches!(
            captured("- CALLCONV y7 class=0x80 target=0x4 parms=[]\n"),
            Err(BuildError {
                line: 1,
                kind: BuildErrorKind::UnknownSymbol(SymbolId(7))
            })
        ));
        assert!(matches!(
            captured("UNSUPPORTED register convention\n"),
            Err(BuildError {
                line: 1,
                kind: BuildErrorKind::ShimRefused { .. }
            })
        ));
    }
}
