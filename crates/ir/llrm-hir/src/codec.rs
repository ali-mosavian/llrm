//! Port of `qbopt/hir/codec.py`: deterministic, strict JSON wire format for
//! HIR producers.
//!
//! Python reflects over each dataclass's type hints; `_Hint` and `_Record`
//! are those hints and fields, written out, so `_make` and `_record` keep
//! Python's walk, order and refusal text.

use std::any::Any;

use llrm_support::hash::IndexMap;

use crate::model;
use crate::verify::{InvalidHIR, verify};
use llrm_support::pyjson::{self, Json};
use llrm_support::pyrepr;

pub type JSON = Json;

/// Python's `_TAGS`.
#[allow(non_snake_case)]
fn _TAGS(tag: &str) -> Option<&'static _Record> {
    match tag {
        "array_element" => Some(&ARRAY_ELEMENT),
        "constant" => Some(&CONSTANT),
        "descriptor" => Some(&DESCRIPTOR_PLACE),
        "indirect" => Some(&INDIRECT_PLACE),
        "place" => Some(&PLACE_REF),
        "projection" => Some(&PROJECTED_PLACE),
        "value" => Some(&VALUE_REF),
        _ => None,
    }
}

/// Python's `_plain`, one implementation per runtime type it meets.
trait _Plain {
    fn _plain(&self) -> JSON;
}

impl _Plain for i64 {
    fn _plain(&self) -> JSON {
        Json::Int(*self)
    }
}

impl _Plain for bool {
    fn _plain(&self) -> JSON {
        Json::Bool(*self)
    }
}

impl _Plain for String {
    fn _plain(&self) -> JSON {
        Json::Str(self.clone())
    }
}

impl<T: _Plain> _Plain for Option<T> {
    fn _plain(&self) -> JSON {
        self.as_ref().map_or(Json::None, _Plain::_plain)
    }
}

impl<T: _Plain> _Plain for Vec<T> {
    fn _plain(&self) -> JSON {
        Json::List(self.iter().map(_Plain::_plain).collect())
    }
}

impl _Plain for (i64, i64) {
    fn _plain(&self) -> JSON {
        Json::List(vec![self.0._plain(), self.1._plain()])
    }
}

impl _Plain for model::Number {
    fn _plain(&self) -> JSON {
        match self {
            model::Number::Int(one) => Json::Int(*one),
            model::Number::Float(one) => Json::Float(*one),
        }
    }
}

macro_rules! plain_enums {
    ($($name:ident),*) => {
        $(
            impl _Plain for model::$name {
                fn _plain(&self) -> JSON {
                    Json::Str(self.value().to_owned())
                }
            }
        )*
    };
}

plain_enums!(
    RuntimeProfile,
    Dialect,
    TargetProfile,
    ArrayOrder,
    FloatMode,
    FloatSemantics,
    Frames,
    TypeKind,
    AddressKind,
    FloatEvaluation,
    StackCleanup,
    FloatReturn,
    CallDistance,
    Storage,
    DataLinkage,
    FunctionLinkage,
    DescriptorField,
    Op,
    TerminatorKind,
    DebugKind,
    DebugReach
);

macro_rules! plain_record {
    ($name:ident, $tag:expr, $($field:ident => $key:literal),*) => {
        impl _Plain for model::$name {
            fn _plain(&self) -> JSON {
                let mut out: IndexMap<String, JSON> = IndexMap::default();
                $(out.insert($key.to_owned(), self.$field._plain());)*
                let tag: Option<&str> = $tag;
                if let Some(tag) = tag {
                    out.insert("tag".to_owned(), Json::Str(tag.to_owned()));
                }
                Json::Dict(out)
            }
        }
    };
}

plain_record!(Type, None, id => "id", name => "name", kind => "kind", width => "width", signed => "signed",
    evaluation => "evaluation", element => "element", rank => "rank", bounds => "bounds", address => "address");
plain_record!(Place, None, id => "id", name => "name", r#type => "type", storage => "storage", offset => "offset",
    symbol => "symbol", extent => "extent", address => "address", volatile => "volatile");
plain_record!(Value, None, id => "id", r#type => "type");
plain_record!(DebugType, None, id => "id", kind => "kind", name => "name", target => "target", size => "size",
    reach => "reach", members => "members");
plain_record!(DebugMember, None, name => "name", r#type => "type", offset => "offset");
plain_record!(DebugParameter, None, argument => "argument", name => "name", r#type => "type");
plain_record!(DebugVariable, None, place => "place", name => "name", r#type => "type");
plain_record!(DebugFunction, None, function => "function", module => "module", name => "name", r#type => "type", parameters => "parameters",
    variables => "variables");
plain_record!(DebugGlobal, None, function => "function", object => "object", offset => "offset", name => "name", r#type => "type");
plain_record!(Debug, None, types => "types", functions => "functions", globals => "globals");
plain_record!(ValueRef, Some("value"), value => "value");
plain_record!(Constant, Some("constant"), r#type => "type", value => "value");
plain_record!(PlaceRef, Some("place"), place => "place");
plain_record!(ArrayElement, Some("array_element"), place => "place", indices => "indices");
plain_record!(ProjectedPlace, Some("projection"), place => "place", indices => "indices", offset => "offset",
    r#type => "type");
plain_record!(IndirectPlace, Some("indirect"), base => "base", offset => "offset", r#type => "type",
    volatile => "volatile", inbounds => "inbounds", origin => "origin",
    allocation => "allocation");
plain_record!(DescriptorPlace, Some("descriptor"), base => "base", field => "field", r#type => "type");
plain_record!(Asm, None, code => "code", inputs => "inputs", outputs => "outputs", clobbers => "clobbers",
    memory => "memory");

/// Only an inline block names `asm`, and only a promised op `nowrap`: every
/// other instruction is written as before they existed.
impl _Plain for model::Instruction {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("id".to_owned(), self.id._plain());
        out.insert("op".to_owned(), self.op._plain());
        out.insert("results".to_owned(), self.results._plain());
        out.insert("operands".to_owned(), self.operands._plain());
        out.insert("callee".to_owned(), self.callee._plain());
        out.insert("pure".to_owned(), self.pure._plain());
        if let Some(asm) = &self.asm {
            out.insert("asm".to_owned(), asm._plain());
        }
        if self.inbounds {
            out.insert("inbounds".to_owned(), self.inbounds._plain());
        }
        if let Some(line) = self.line {
            out.insert("line".to_owned(), line._plain());
        }
        Json::Dict(out)
    }
}
plain_record!(Terminator, None, kind => "kind", operands => "operands", targets => "targets", cases => "cases");
plain_record!(Block, None, id => "id", instructions => "instructions", terminator => "terminator", cold => "cold");
/// A float result's return is written only when it is not BASIC's, as
/// before any other existed.
fn float_return(out: &mut IndexMap<String, JSON>, float_return: model::FloatReturn) {
    if float_return != model::FloatReturn::Pointer {
        out.insert("float_return".to_owned(), float_return._plain());
    }
}

impl _Plain for model::CallAbi {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("instruction".to_owned(), self.instruction._plain());
        out.insert("order".to_owned(), self.order._plain());
        out.insert("cleanup".to_owned(), self.cleanup._plain());
        out.insert("distance".to_owned(), self.distance._plain());
        out.insert("callee".to_owned(), self.callee._plain());
        float_return(&mut out, self.float_return);
        Json::Dict(out)
    }
}
impl _Plain for model::Callable {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("id".to_owned(), self.id._plain());
        out.insert("name".to_owned(), self.name._plain());
        out.insert("result_type".to_owned(), self.result_type._plain());
        out.insert("parameter_types".to_owned(), self.parameter_types._plain());
        out.insert("by_value".to_owned(), self.by_value._plain());
        out.insert("segmented".to_owned(), self.segmented._plain());
        out.insert("arrays".to_owned(), self.arrays._plain());
        out.insert("defined".to_owned(), self.defined._plain());
        if self.symbol.is_some() {
            out.insert("symbol".to_owned(), self.symbol._plain());
        }
        Json::Dict(out)
    }
}
impl _Plain for model::ProcedureAbi {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("cleanup".to_owned(), self.cleanup._plain());
        out.insert("distance".to_owned(), self.distance._plain());
        out.insert("parameter_bytes".to_owned(), self.parameter_bytes._plain());
        float_return(&mut out, self.float_return);
        if self.variadic {
            out.insert("variadic".to_owned(), self.variadic._plain());
        }
        Json::Dict(out)
    }
}
// `promises` only when made, so that a function reads as it always has.
impl _Plain for model::Function {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("id".to_owned(), self.id._plain());
        out.insert("name".to_owned(), self.name._plain());
        out.insert("result_type".to_owned(), self.result_type._plain());
        out.insert("values".to_owned(), self.values._plain());
        out.insert("places".to_owned(), self.places._plain());
        out.insert("blocks".to_owned(), self.blocks._plain());
        out.insert("entry".to_owned(), self.entry._plain());
        out.insert("parameters".to_owned(), self.parameters._plain());
        out.insert("abi".to_owned(), self.abi._plain());
        out.insert("calls".to_owned(), self.calls._plain());
        out.insert("error_handler".to_owned(), self.error_handler._plain());
        out.insert("error_handler_local".to_owned(), self.error_handler_local._plain());
        out.insert("external_entries".to_owned(), self.external_entries._plain());
        out.insert("linkage".to_owned(), self.linkage._plain());
        if self.symbol.is_some() {
            out.insert("symbol".to_owned(), self.symbol._plain());
        }
        Json::Dict(out)
    }
}
// `code` only when set, so that data relocations read as they always have.
impl _Plain for model::DataRelocation {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("at".to_owned(), self.at._plain());
        out.insert("target".to_owned(), self.target._plain());
        out.insert("addend".to_owned(), self.addend._plain());
        out.insert("address".to_owned(), self.address._plain());
        if self.code {
            out.insert("code".to_owned(), self.code._plain());
        }
        Json::Dict(out)
    }
}
// `segment` only when set, so that data reads as it always has.
impl _Plain for model::DataObject {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("id".to_owned(), self.id._plain());
        out.insert("name".to_owned(), self.name._plain());
        out.insert("bytes".to_owned(), self.bytes._plain());
        out.insert("readonly".to_owned(), self.readonly._plain());
        out.insert("relocations".to_owned(), self.relocations._plain());
        out.insert("linkage".to_owned(), self.linkage._plain());
        out.insert("address".to_owned(), self.address._plain());
        out.insert("addressed".to_owned(), self.addressed._plain());
        if self.segment.is_some() {
            out.insert("segment".to_owned(), self.segment._plain());
        }
        Json::Dict(out)
    }
}
plain_record!(AliasClass, None, name => "name", parent => "parent", types => "types");
// A stated fact as flat fields: the subject's kind, its function and its id.
impl _Plain for crate::facts::Stated {
    fn _plain(&self) -> JSON {
        let (function, id, part) = self.subject.fields();
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("subject".to_owned(), Json::Str(crate::facts::Subject::kind_key(self.subject.kind()).to_owned()));
        if let Some(function) = function {
            out.insert("function".to_owned(), function._plain());
        }
        if let Some(id) = id {
            out.insert("id".to_owned(), id._plain());
        }
        if let Some(part) = part {
            out.insert("part".to_owned(), part._plain());
        }
        out.insert("fact".to_owned(), Json::Str(self.fact.key().to_owned()));
        if let Some(value) = self.fact.wire_value() {
            out.insert("value".to_owned(), value._plain());
        }
        if self.source.is_some() {
            out.insert("source".to_owned(), self.source._plain());
        }
        Json::Dict(out)
    }
}
// `alias_classes` only when made, so that a module reads as it always has.
impl _Plain for model::Module {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("id".to_owned(), self.id._plain());
        out.insert("name".to_owned(), self.name._plain());
        out.insert("types".to_owned(), self.types._plain());
        out.insert("functions".to_owned(), self.functions._plain());
        out.insert("data".to_owned(), self.data._plain());
        out.insert("callables".to_owned(), self.callables._plain());
        if !self.alias_classes.is_empty() {
            out.insert("alias_classes".to_owned(), self.alias_classes._plain());
        }
        if !self.facts.is_empty() {
            out.insert("facts".to_owned(), self.facts._plain());
        }
        if self.debug.is_some() {
            out.insert("debug".to_owned(), self.debug._plain());
        }
        if !self.line_numbers.is_empty() {
            out.insert("line_numbers".to_owned(), self.line_numbers._plain());
        }
        Json::Dict(out)
    }
}
plain_record!(CellWriters, None, cell => "cell", routines => "routines");
// `reads_arguments` only when made, so that promises read as they always have.
impl _Plain for model::RuntimePromises {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("calling_back".to_owned(), self.calling_back._plain());
        out.insert("writers".to_owned(), self.writers._plain());
        out.insert("nounwind".to_owned(), self.nounwind._plain());
        if !self.reads_arguments.is_empty() {
            out.insert("reads_arguments".to_owned(), self.reads_arguments._plain());
        }
        Json::Dict(out)
    }
}
/// Float semantics are written only when they are the machine's, as before
/// they existed.
impl _Plain for model::Program {
    fn _plain(&self) -> JSON {
        let mut out: IndexMap<String, JSON> = IndexMap::default();
        out.insert("dialect".to_owned(), self.dialect._plain());
        out.insert("runtime".to_owned(), self.runtime._plain());
        out.insert("modules".to_owned(), self.modules._plain());
        out.insert("schema".to_owned(), self.schema._plain());
        out.insert("target".to_owned(), self.target._plain());
        out.insert("array_order".to_owned(), self.array_order._plain());
        out.insert("float_mode".to_owned(), self.float_mode._plain());
        if self.float_semantics != model::FloatSemantics::Declared {
            out.insert("float_semantics".to_owned(), self.float_semantics._plain());
        }
        if !self.zeroed_locals {
            out.insert("zeroed_locals".to_owned(), self.zeroed_locals._plain());
        }
        if self.frames != model::Frames::Runtime {
            out.insert("frames".to_owned(), self.frames._plain());
        }
        if self.promises != model::RuntimePromises::default() {
            out.insert("promises".to_owned(), self.promises._plain());
        }
        if !self.entries.is_empty() {
            out.insert("entries".to_owned(), self.entries._plain());
        }
        if !self.preserved.is_empty() {
            out.insert("preserved".to_owned(), self.preserved._plain());
        }
        if self.constant_segment.is_some() {
            out.insert("constant_segment".to_owned(), self.constant_segment._plain());
        }
        Json::Dict(out)
    }
}

impl _Plain for model::Operand {
    fn _plain(&self) -> JSON {
        match self {
            model::Operand::ValueRef(one) => one._plain(),
            model::Operand::Constant(one) => one._plain(),
            model::Operand::PlaceRef(one) => one._plain(),
            model::Operand::ArrayElement(one) => one._plain(),
            model::Operand::ProjectedPlace(one) => one._plain(),
            model::Operand::IndirectPlace(one) => one._plain(),
            model::Operand::DescriptorPlace(one) => one._plain(),
        }
    }
}

pub fn encode(program: &model::Program, indent: Option<usize>) -> Result<String, InvalidHIR> {
    verify(program)?;
    let separators = if indent.is_some_and(|width| width > 0) { None } else { Some((",", ":")) };
    Ok(pyjson::dumps(&program._plain(), indent, separators, true) + "\n")
}

/// A type hint `_make` walks.
enum _Hint {
    Int,
    Float,
    Str,
    Bool,
    NoneType,
    Enum(&'static str, &'static [&'static str]),
    Record(&'static _Record),
    Operand,
    Tuple(&'static _Hint),
    Union(&'static [_Hint]),
}

impl _Hint {
    /// `str()` of the hint, as a union spells its members.
    fn text(&self) -> String {
        match self {
            _Hint::Int => "int".to_owned(),
            _Hint::Float => "float".to_owned(),
            _Hint::Str => "str".to_owned(),
            _Hint::Bool => "bool".to_owned(),
            _Hint::NoneType => "None".to_owned(),
            _Hint::Enum(name, _) => format!("qbopt.hir.model.{name}"),
            _Hint::Record(record) => format!("qbopt.hir.model.{}", record.name),
            _Hint::Operand => "Operand".to_owned(),
            _Hint::Tuple(item) => format!("tuple[{}, ...]", item.text()),
            _Hint::Union(choices) => choices.iter().map(_Hint::text).collect::<Vec<_>>().join(" | "),
        }
    }
}

/// One dataclass: its fields in declaration order, and its constructor.
struct _Record {
    name: &'static str,
    fields: &'static [(&'static str, _Hint, bool)],
    build: fn(&mut _Args) -> Result<_Made, InvalidHIR>,
}

/// A value `_make` produced.
enum _Made {
    None,
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Enum(String),
    Tuple(Vec<_Made>),
    Object(Box<dyn Any>),
}

type _Args = IndexMap<&'static str, _Made>;

fn _make(type_: &_Hint, value: &JSON, where_: &str) -> Result<_Made, InvalidHIR> {
    match type_ {
        _Hint::Operand => {
            let record = match value {
                Json::Dict(items) => match items.get("tag") {
                    Some(Json::Str(tag)) => _TAGS(tag),
                    _ => None,
                },
                _ => None,
            };
            let Some(record) = record else {
                return Err(InvalidHIR(format!("{where_}: unknown operand tag")));
            };
            _record(record, value, where_, true)
        }
        _Hint::Tuple(subtype) => {
            let Json::List(items) = value else {
                return Err(InvalidHIR(format!("{where_}: expected array")));
            };
            let where_ = format!("{where_}[]");
            Ok(_Made::Tuple(items.iter().map(|one| _make(subtype, one, &where_)).collect::<Result<_, _>>()?))
        }
        _Hint::Union(choices) => {
            let tagged = match value {
                Json::Dict(items) => match items.get("tag") {
                    Some(Json::Str(tag)) => _TAGS(tag),
                    _ => None,
                },
                _ => None,
            };
            if let Some(record) = tagged.filter(|record| {
                choices.iter().any(|one| matches!(one, _Hint::Record(choice) if std::ptr::eq(*choice, *record)))
            }) {
                return _record(record, value, where_, true);
            }
            for choice in *choices {
                if matches!(choice, _Hint::NoneType) && *value == Json::None {
                    return Ok(_Made::None);
                }
                if let Ok(made) = _make(choice, value, where_) {
                    return Ok(made);
                }
            }
            Err(InvalidHIR(format!("{where_}: value does not match {}", type_.text())))
        }
        _Hint::Enum(name, values) => match value {
            Json::Str(one) if values.contains(&one.as_str()) => Ok(_Made::Enum(one.clone())),
            _ => Err(InvalidHIR(format!("{where_}: unknown {name} {}", value.repr()))),
        },
        _Hint::Record(record) => _record(record, value, where_, false),
        _Hint::Int => match value {
            Json::Int(one) => Ok(_Made::Int(*one)),
            _ => Err(InvalidHIR(format!("{where_}: expected integer"))),
        },
        _Hint::Float => match value {
            Json::Int(one) => Ok(_Made::Int(*one)),
            Json::Float(one) => Ok(_Made::Float(*one)),
            _ => Err(InvalidHIR(format!("{where_}: expected number"))),
        },
        _Hint::Str => match value {
            Json::Str(one) => Ok(_Made::Str(one.clone())),
            _ => Err(InvalidHIR(format!("{where_}: expected string"))),
        },
        _Hint::Bool => match value {
            Json::Bool(one) => Ok(_Made::Bool(*one)),
            _ => Err(InvalidHIR(format!("{where_}: expected boolean"))),
        },
        // Python passes any value through here; a Rust field cannot hold it.
        _Hint::NoneType => match value {
            Json::None => Ok(_Made::None),
            _ => Err(InvalidHIR(format!("{where_}: expected None"))),
        },
    }
}

fn _record(type_: &_Record, value: &JSON, where_: &str, tagged: bool) -> Result<_Made, InvalidHIR> {
    let Json::Dict(value) = value else {
        return Err(InvalidHIR(format!("{where_}: expected object")));
    };
    let allowed = |name: &str| type_.fields.iter().any(|(field, _, _)| *field == name) || (tagged && name == "tag");
    let mut unknown: Vec<String> = value.keys().filter(|name| !allowed(name)).cloned().collect();
    if !unknown.is_empty() {
        unknown.sort();
        return Err(InvalidHIR(format!("{where_}: unknown fields {}", pyrepr::list(&unknown))));
    }
    let mut missing: Vec<String> = type_
        .fields
        .iter()
        .filter(|(name, _, required)| *required && !value.contains_key(*name))
        .map(|(name, _, _)| (*name).to_owned())
        .collect();
    if !missing.is_empty() {
        missing.sort();
        return Err(InvalidHIR(format!("{where_}: missing fields {}", pyrepr::list(&missing))));
    }
    let mut args = _Args::default();
    for (name, hint, _) in type_.fields {
        if let Some(one) = value.get(*name) {
            args.insert(name, _make(hint, one, &format!("{where_}.{name}"))?);
        }
    }
    (type_.build)(&mut args)
}

/// `facts` as the module's `facts` field holds them, for a frontend
/// that writes its module's JSON itself.
pub fn facts_json(facts: &[crate::facts::Stated]) -> String {
    pyjson::dumps(&facts.to_vec()._plain(), None, Some((",", ":")), true)
}

/// `debug` as the module's `debug` field holds it, for a frontend
/// writing HIR as text.
pub fn debug_json(debug: &model::Debug) -> String {
    pyjson::dumps(&debug._plain(), None, Some((",", ":")), true)
}

pub fn decode(text: &str) -> Result<model::Program, InvalidHIR> {
    let raw = pyjson::loads(text).map_err(|error| InvalidHIR(format!("invalid HIR JSON: {error}")))?;
    // A program of another schema is refused as such, not by the first field it has that this one does not.
    if let Json::Dict(fields) = &raw
        && let Some(Json::Int(schema)) = fields.get("schema")
        && *schema != model::SCHEMA_VERSION
    {
        return Err(InvalidHIR(format!("unsupported HIR schema {schema}")));
    }
    let _Made::Object(program) = _record(&PROGRAM, &raw, "program", false)? else {
        unreachable!("a record builds an object");
    };
    let program = *program.downcast::<model::Program>().expect("PROGRAM builds a Program");
    verify(&program)?;
    Ok(program)
}

// ---- The dataclasses' hints and constructors ----

/// Python's `type_(**args)` reading one argument back.
trait _FromMade: Sized {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR>;
}

impl _FromMade for i64 {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::Int(one) => Ok(one),
            _ => unreachable!("an int hint makes an int"),
        }
    }
}

impl _FromMade for bool {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::Bool(one) => Ok(one),
            _ => unreachable!("a bool hint makes a bool"),
        }
    }
}

impl _FromMade for String {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::Str(one) => Ok(one),
            _ => unreachable!("a str hint makes a str"),
        }
    }
}

impl _FromMade for model::Number {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::Int(one) => Ok(model::Number::Int(one)),
            _Made::Float(one) => Ok(model::Number::Float(one)),
            _ => unreachable!("int | float makes a number"),
        }
    }
}

impl<T: _FromMade> _FromMade for Option<T> {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::None => Ok(None),
            made => Ok(Some(T::from_made(made)?)),
        }
    }
}

impl<T: _FromMade> _FromMade for Vec<T> {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::Tuple(items) => items.into_iter().map(T::from_made).collect(),
            _ => unreachable!("a tuple hint makes a tuple"),
        }
    }
}

/// `tuple[int, int]`, which `_make` checks only as a tuple of ints; Python's
/// first unpacking of a wrong length raises this `ValueError`.
impl _FromMade for (i64, i64) {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        let items = Vec::<i64>::from_made(made)?;
        match items[..] {
            [one, other] => Ok((one, other)),
            [] | [_] => Err(InvalidHIR(format!("not enough values to unpack (expected 2, got {})", items.len()))),
            _ => Err(InvalidHIR("too many values to unpack (expected 2)".to_owned())),
        }
    }
}

macro_rules! made_enums {
    ($($name:ident),*) => {
        $(
            impl _FromMade for model::$name {
                fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
                    match made {
                        _Made::Enum(one) => Ok(model::$name::from_value(&one).expect("a checked member")),
                        _ => unreachable!("an enum hint makes a member"),
                    }
                }
            }
        )*
    };
}

made_enums!(
    RuntimeProfile,
    Dialect,
    TargetProfile,
    ArrayOrder,
    FloatMode,
    FloatSemantics,
    Frames,
    TypeKind,
    AddressKind,
    FloatEvaluation,
    StackCleanup,
    FloatReturn,
    CallDistance,
    Storage,
    DataLinkage,
    FunctionLinkage,
    DescriptorField,
    Op,
    TerminatorKind,
    DebugKind,
    DebugReach
);

macro_rules! made_records {
    ($($name:ident),*) => {
        $(
            impl _FromMade for model::$name {
                fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
                    match made {
                        _Made::Object(one) => Ok(*one.downcast::<model::$name>().expect("the hinted record")),
                        _ => unreachable!("a record hint makes an object"),
                    }
                }
            }
        )*
    };
}

impl _FromMade for crate::facts::Stated {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        match made {
            _Made::Object(one) => Ok(*one.downcast::<crate::facts::Stated>().expect("the hinted record")),
            _ => unreachable!("a record hint makes an object"),
        }
    }
}

made_records!(
    CellWriters,
    RuntimePromises,
    Type,
    Place,
    Value,
    Asm,
    Instruction,
    Terminator,
    Block,
    CallAbi,
    Callable,
    ProcedureAbi,
    Function,
    DataRelocation,
    DataObject,
    AliasClass,
    Module,
    DebugType,
    DebugMember,
    DebugParameter,
    DebugVariable,
    DebugFunction,
    DebugGlobal,
    Debug
);

impl _FromMade for model::Operand {
    fn from_made(made: _Made) -> Result<Self, InvalidHIR> {
        let _Made::Object(one) = made else {
            unreachable!("an operand hint makes an object");
        };
        let one = match one.downcast::<model::ValueRef>() {
            Ok(one) => return Ok(model::Operand::ValueRef(*one)),
            Err(one) => one,
        };
        let one = match one.downcast::<model::Constant>() {
            Ok(one) => return Ok(model::Operand::Constant(*one)),
            Err(one) => one,
        };
        let one = match one.downcast::<model::PlaceRef>() {
            Ok(one) => return Ok(model::Operand::PlaceRef(*one)),
            Err(one) => one,
        };
        let one = match one.downcast::<model::ArrayElement>() {
            Ok(one) => return Ok(model::Operand::ArrayElement(*one)),
            Err(one) => one,
        };
        let one = match one.downcast::<model::ProjectedPlace>() {
            Ok(one) => return Ok(model::Operand::ProjectedPlace(*one)),
            Err(one) => one,
        };
        let one = match one.downcast::<model::IndirectPlace>() {
            Ok(one) => return Ok(model::Operand::IndirectPlace(*one)),
            Err(one) => one,
        };
        Ok(model::Operand::DescriptorPlace(*one.downcast::<model::DescriptorPlace>().expect("an operand record")))
    }
}

/// A required argument.
fn _required<T: _FromMade>(args: &mut _Args, name: &str) -> Result<T, InvalidHIR> {
    T::from_made(args.shift_remove(name).expect("required fields were checked"))
}

/// An argument with its dataclass default.
fn _default<T: _FromMade>(args: &mut _Args, name: &str, default: T) -> Result<T, InvalidHIR> {
    match args.shift_remove(name) {
        Some(one) => T::from_made(one),
        None => Ok(default),
    }
}

fn _object<T: Any>(value: T) -> Result<_Made, InvalidHIR> {
    Ok(_Made::Object(Box::new(value)))
}

/// `tuple[ValueRef | Constant, ...]`; a const cannot borrow a static.
macro_rules! indices {
    () => {
        _Hint::Tuple(&_Hint::Union(&[_Hint::Record(&VALUE_REF), _Hint::Record(&CONSTANT)]))
    };
}
const OPTIONAL_INT: _Hint = _Hint::Union(&[_Hint::Int, _Hint::NoneType]);
const INTS: _Hint = _Hint::Tuple(&_Hint::Int);
const BOOLS: _Hint = _Hint::Tuple(&_Hint::Bool);
const OPERANDS: _Hint = _Hint::Tuple(&_Hint::Operand);

macro_rules! enum_hint {
    ($name:ident) => {
        _Hint::Enum(stringify!($name), model::$name::VALUES)
    };
}

static TYPE: _Record = _Record {
    name: "Type",
    fields: &[
        ("id", _Hint::Int, true),
        ("name", _Hint::Str, true),
        ("kind", enum_hint!(TypeKind), true),
        ("width", _Hint::Int, true),
        ("signed", _Hint::Union(&[_Hint::Bool, _Hint::NoneType]), false),
        ("evaluation", enum_hint!(FloatEvaluation), false),
        ("element", OPTIONAL_INT, false),
        ("rank", _Hint::Int, false),
        ("bounds", _Hint::Tuple(&_Hint::Tuple(&_Hint::Int)), false),
        ("address", enum_hint!(AddressKind), false),
    ],
    build: |args| {
        _object(model::Type {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            kind: _required(args, "kind")?,
            width: _required(args, "width")?,
            signed: _default(args, "signed", None)?,
            evaluation: _default(args, "evaluation", model::FloatEvaluation::None)?,
            element: _default(args, "element", None)?,
            rank: _default(args, "rank", 0)?,
            bounds: _default(args, "bounds", Vec::new())?,
            address: _default(args, "address", model::AddressKind::None)?,
        })
    },
};

static PLACE: _Record = _Record {
    name: "Place",
    fields: &[
        ("id", _Hint::Int, true),
        ("name", _Hint::Str, true),
        ("type", _Hint::Int, true),
        ("storage", enum_hint!(Storage), true),
        ("offset", _Hint::Int, true),
        ("symbol", _Hint::Int, false),
        ("extent", OPTIONAL_INT, false),
        ("address", enum_hint!(AddressKind), false),
        ("volatile", _Hint::Bool, false),
    ],
    build: |args| {
        _object(model::Place {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            r#type: _required(args, "type")?,
            storage: _required(args, "storage")?,
            offset: _required(args, "offset")?,
            symbol: _default(args, "symbol", 0)?,
            extent: _default(args, "extent", None)?,
            address: _default(args, "address", model::AddressKind::Near)?,
            volatile: _default(args, "volatile", false)?,
        })
    },
};

static VALUE: _Record = _Record {
    name: "Value",
    fields: &[("id", _Hint::Int, true), ("type", _Hint::Int, true)],
    build: |args| _object(model::Value { id: _required(args, "id")?, r#type: _required(args, "type")? }),
};

static VALUE_REF: _Record = _Record {
    name: "ValueRef",
    fields: &[("value", _Hint::Int, true)],
    build: |args| _object(model::ValueRef { value: _required(args, "value")? }),
};

static CONSTANT: _Record = _Record {
    name: "Constant",
    fields: &[("type", _Hint::Int, true), ("value", _Hint::Union(&[_Hint::Int, _Hint::Float]), true)],
    build: |args| _object(model::Constant { r#type: _required(args, "type")?, value: _required(args, "value")? }),
};

static PLACE_REF: _Record = _Record {
    name: "PlaceRef",
    fields: &[("place", _Hint::Int, true)],
    build: |args| _object(model::PlaceRef { place: _required(args, "place")? }),
};

static ARRAY_ELEMENT: _Record = _Record {
    name: "ArrayElement",
    fields: &[("place", _Hint::Int, true), ("indices", indices!(), true)],
    build: |args| {
        _object(model::ArrayElement { place: _required(args, "place")?, indices: _required(args, "indices")? })
    },
};

static PROJECTED_PLACE: _Record = _Record {
    name: "ProjectedPlace",
    fields: &[
        ("place", _Hint::Int, true),
        ("indices", indices!(), true),
        ("offset", _Hint::Int, true),
        ("type", _Hint::Int, true),
    ],
    build: |args| {
        _object(model::ProjectedPlace {
            place: _required(args, "place")?,
            indices: _required(args, "indices")?,
            offset: _required(args, "offset")?,
            r#type: _required(args, "type")?,
        })
    },
};

static INDIRECT_PLACE: _Record = _Record {
    name: "IndirectPlace",
    fields: &[
        ("base", _Hint::Int, true),
        ("offset", _Hint::Int, true),
        ("type", _Hint::Int, true),
        ("volatile", _Hint::Bool, false),
        ("inbounds", _Hint::Bool, false),
        ("origin", OPTIONAL_INT, false),
        ("allocation", OPTIONAL_INT, false),
    ],
    build: |args| {
        _object(model::IndirectPlace {
            base: _required(args, "base")?,
            offset: _required(args, "offset")?,
            r#type: _required(args, "type")?,
            volatile: _default(args, "volatile", false)?,
            inbounds: _default(args, "inbounds", false)?,
            origin: _default(args, "origin", None)?,
            allocation: _default(args, "allocation", None)?,
        })
    },
};

static DESCRIPTOR_PLACE: _Record = _Record {
    name: "DescriptorPlace",
    fields: &[("base", _Hint::Int, true), ("field", enum_hint!(DescriptorField), true), ("type", _Hint::Int, true)],
    build: |args| {
        _object(model::DescriptorPlace {
            base: _required(args, "base")?,
            field: _required(args, "field")?,
            r#type: _required(args, "type")?,
        })
    },
};

static INSTRUCTION: _Record = _Record {
    name: "Instruction",
    fields: &[
        ("id", _Hint::Int, true),
        ("op", enum_hint!(Op), true),
        ("results", INTS, false),
        ("operands", OPERANDS, false),
        ("callee", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), false),
        ("pure", _Hint::Bool, false),
        ("asm", _Hint::Union(&[_Hint::Record(&ASM), _Hint::NoneType]), false),
        ("inbounds", _Hint::Bool, false),
        ("line", _Hint::Union(&[_Hint::Int, _Hint::NoneType]), false),
    ],
    build: |args| {
        _object(model::Instruction {
            id: _required(args, "id")?,
            op: _required(args, "op")?,
            results: _default(args, "results", Vec::new())?,
            operands: _default(args, "operands", Vec::new())?,
            callee: _default(args, "callee", None)?,
            pure: _default(args, "pure", false)?,
            asm: _default(args, "asm", None)?,
            inbounds: _default(args, "inbounds", false)?,
            line: _default(args, "line", None)?,
        })
    },
};

static ASM: _Record = _Record {
    name: "Asm",
    fields: &[
        ("code", INTS, true),
        ("inputs", _Hint::Tuple(&_Hint::Str), true),
        ("outputs", _Hint::Tuple(&_Hint::Str), true),
        ("clobbers", _Hint::Tuple(&_Hint::Str), true),
        ("memory", _Hint::Bool, true),
    ],
    build: |args| {
        _object(model::Asm {
            code: _required(args, "code")?,
            inputs: _required(args, "inputs")?,
            outputs: _required(args, "outputs")?,
            clobbers: _required(args, "clobbers")?,
            memory: _required(args, "memory")?,
        })
    },
};

static TERMINATOR: _Record = _Record {
    name: "Terminator",
    fields: &[
        ("kind", enum_hint!(TerminatorKind), true),
        ("operands", OPERANDS, false),
        ("targets", INTS, false),
        ("cases", _Hint::Tuple(&_Hint::Tuple(&_Hint::Int)), false),
    ],
    build: |args| {
        _object(model::Terminator {
            kind: _required(args, "kind")?,
            operands: _default(args, "operands", Vec::new())?,
            targets: _default(args, "targets", Vec::new())?,
            cases: _default(args, "cases", Vec::new())?,
        })
    },
};

static BLOCK: _Record = _Record {
    name: "Block",
    fields: &[
        ("id", _Hint::Int, true),
        ("instructions", _Hint::Tuple(&_Hint::Record(&INSTRUCTION)), true),
        ("terminator", _Hint::Record(&TERMINATOR), true),
        ("cold", _Hint::Bool, false),
    ],
    build: |args| {
        _object(model::Block {
            id: _required(args, "id")?,
            instructions: _required(args, "instructions")?,
            terminator: _required(args, "terminator")?,
            cold: _default(args, "cold", false)?,
        })
    },
};

static CALL_ABI: _Record = _Record {
    name: "CallAbi",
    fields: &[
        ("instruction", _Hint::Int, true),
        ("order", INTS, true),
        ("cleanup", enum_hint!(StackCleanup), true),
        ("distance", enum_hint!(CallDistance), true),
        ("callee", OPTIONAL_INT, false),
        ("float_return", enum_hint!(FloatReturn), false),
    ],
    build: |args| {
        _object(model::CallAbi {
            instruction: _required(args, "instruction")?,
            order: _required(args, "order")?,
            cleanup: _required(args, "cleanup")?,
            distance: _required(args, "distance")?,
            callee: _default(args, "callee", None)?,
            float_return: _default(args, "float_return", model::FloatReturn::Pointer)?,
        })
    },
};

static CALLABLE: _Record = _Record {
    name: "Callable",
    fields: &[
        ("id", _Hint::Int, true),
        ("name", _Hint::Str, true),
        ("result_type", OPTIONAL_INT, true),
        ("parameter_types", INTS, true),
        ("by_value", BOOLS, true),
        ("segmented", BOOLS, true),
        ("arrays", BOOLS, true),
        ("defined", _Hint::Bool, true),
        ("symbol", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), false),
    ],
    build: |args| {
        _object(model::Callable {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            result_type: _required(args, "result_type")?,
            parameter_types: _required(args, "parameter_types")?,
            by_value: _required(args, "by_value")?,
            segmented: _required(args, "segmented")?,
            arrays: _required(args, "arrays")?,
            defined: _required(args, "defined")?,
            symbol: _default(args, "symbol", None)?,
        })
    },
};

static PROCEDURE_ABI: _Record = _Record {
    name: "ProcedureAbi",
    fields: &[
        ("cleanup", enum_hint!(StackCleanup), true),
        ("distance", enum_hint!(CallDistance), true),
        ("parameter_bytes", _Hint::Int, true),
        ("float_return", enum_hint!(FloatReturn), false),
        ("variadic", _Hint::Bool, false),
    ],
    build: |args| {
        _object(model::ProcedureAbi {
            cleanup: _required(args, "cleanup")?,
            distance: _required(args, "distance")?,
            parameter_bytes: _required(args, "parameter_bytes")?,
            float_return: _default(args, "float_return", model::FloatReturn::Pointer)?,
            variadic: _default(args, "variadic", false)?,
        })
    },
};

static FUNCTION: _Record = _Record {
    name: "Function",
    fields: &[
        ("id", _Hint::Int, true),
        ("name", _Hint::Str, true),
        ("result_type", _Hint::Int, true),
        ("values", _Hint::Tuple(&_Hint::Record(&VALUE)), true),
        ("places", _Hint::Tuple(&_Hint::Record(&PLACE)), true),
        ("blocks", _Hint::Tuple(&_Hint::Record(&BLOCK)), true),
        ("entry", _Hint::Int, true),
        ("parameters", INTS, false),
        ("abi", _Hint::Union(&[_Hint::Record(&PROCEDURE_ABI), _Hint::NoneType]), false),
        ("calls", _Hint::Tuple(&_Hint::Record(&CALL_ABI)), false),
        ("error_handler", OPTIONAL_INT, false),
        ("error_handler_local", _Hint::Bool, false),
        ("external_entries", INTS, false),
        ("linkage", enum_hint!(FunctionLinkage), false),
        ("symbol", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), false),
    ],
    build: |args| {
        _object(model::Function {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            result_type: _required(args, "result_type")?,
            values: _required(args, "values")?,
            places: _required(args, "places")?,
            blocks: _required(args, "blocks")?,
            entry: _required(args, "entry")?,
            parameters: _default(args, "parameters", Vec::new())?,
            abi: _default(args, "abi", None)?,
            calls: _default(args, "calls", Vec::new())?,
            error_handler: _default(args, "error_handler", None)?,
            error_handler_local: _default(args, "error_handler_local", false)?,
            external_entries: _default(args, "external_entries", Vec::new())?,
            linkage: _default(args, "linkage", model::FunctionLinkage::External)?,
            symbol: _default(args, "symbol", None)?,
        })
    },
};

static DATA_RELOCATION: _Record = _Record {
    name: "DataRelocation",
    fields: &[
        ("at", _Hint::Int, true),
        ("target", _Hint::Int, true),
        ("addend", _Hint::Int, true),
        ("address", enum_hint!(AddressKind), true),
        ("code", _Hint::Bool, false),
    ],
    build: |args| {
        _object(model::DataRelocation {
            at: _required(args, "at")?,
            target: _required(args, "target")?,
            addend: _required(args, "addend")?,
            address: _required(args, "address")?,
            code: _default(args, "code", false)?,
        })
    },
};

static DATA_OBJECT: _Record = _Record {
    name: "DataObject",
    fields: &[
        ("id", _Hint::Int, true),
        ("name", _Hint::Str, true),
        ("bytes", INTS, true),
        ("readonly", _Hint::Bool, false),
        ("relocations", _Hint::Tuple(&_Hint::Record(&DATA_RELOCATION)), false),
        ("linkage", enum_hint!(DataLinkage), false),
        ("address", enum_hint!(AddressKind), false),
        ("addressed", _Hint::Bool, false),
        ("segment", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), false),
    ],
    build: |args| {
        _object(model::DataObject {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            bytes: _required(args, "bytes")?,
            readonly: _default(args, "readonly", false)?,
            relocations: _default(args, "relocations", Vec::new())?,
            linkage: _default(args, "linkage", model::DataLinkage::Internal)?,
            address: _default(args, "address", model::AddressKind::Near)?,
            addressed: _default(args, "addressed", true)?,
            segment: _default(args, "segment", None)?,
        })
    },
};

static MODULE: _Record = _Record {
    name: "Module",
    fields: &[
        ("id", _Hint::Int, true),
        ("name", _Hint::Str, true),
        ("types", _Hint::Tuple(&_Hint::Record(&TYPE)), true),
        ("functions", _Hint::Tuple(&_Hint::Record(&FUNCTION)), true),
        ("data", _Hint::Tuple(&_Hint::Record(&DATA_OBJECT)), false),
        ("callables", _Hint::Tuple(&_Hint::Record(&CALLABLE)), false),
        ("alias_classes", _Hint::Tuple(&_Hint::Record(&ALIAS_CLASS)), false),
        ("facts", _Hint::Tuple(&_Hint::Record(&STATED_FACT)), false),
        ("debug", _Hint::Union(&[_Hint::Record(&DEBUG), _Hint::NoneType]), false),
        ("line_numbers", _Hint::Tuple(&_Hint::Tuple(&_Hint::Int)), false),
    ],
    build: |args| {
        _object(model::Module {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            types: _required(args, "types")?,
            functions: _required(args, "functions")?,
            data: _default(args, "data", Vec::new())?,
            callables: _default(args, "callables", Vec::new())?,
            alias_classes: _default(args, "alias_classes", Vec::new())?,
            facts: _default(args, "facts", Vec::new())?,
            debug: _default(args, "debug", None)?,
            line_numbers: _default(args, "line_numbers", Vec::new())?,
        })
    },
};

static DEBUG_TYPE: _Record = _Record {
    name: "DebugType",
    fields: &[
        ("id", _Hint::Int, true),
        ("kind", enum_hint!(DebugKind), true),
        ("name", _Hint::Str, true),
        ("target", OPTIONAL_INT, true),
        ("size", _Hint::Int, true),
        ("reach", enum_hint!(DebugReach), true),
        ("members", _Hint::Tuple(&_Hint::Record(&DEBUG_MEMBER)), true),
    ],
    build: |args| {
        _object(model::DebugType {
            id: _required(args, "id")?,
            kind: _required(args, "kind")?,
            name: _required(args, "name")?,
            target: _required(args, "target")?,
            size: _required(args, "size")?,
            reach: _required(args, "reach")?,
            members: _required(args, "members")?,
        })
    },
};

static DEBUG_MEMBER: _Record = _Record {
    name: "DebugMember",
    fields: &[("name", _Hint::Str, true), ("type", _Hint::Int, true), ("offset", _Hint::Int, true)],
    build: |args| _object(model::DebugMember { name: _required(args, "name")?, r#type: _required(args, "type")?, offset: _required(args, "offset")? }),
};

static DEBUG_PARAMETER: _Record = _Record {
    name: "DebugParameter",
    fields: &[("argument", _Hint::Int, true), ("name", _Hint::Str, true), ("type", _Hint::Int, true)],
    build: |args| _object(model::DebugParameter { argument: _required(args, "argument")?, name: _required(args, "name")?, r#type: _required(args, "type")? }),
};

static DEBUG_VARIABLE: _Record = _Record {
    name: "DebugVariable",
    fields: &[("place", _Hint::Int, true), ("name", _Hint::Str, true), ("type", _Hint::Int, true)],
    build: |args| _object(model::DebugVariable { place: _required(args, "place")?, name: _required(args, "name")?, r#type: _required(args, "type")? }),
};

static DEBUG_FUNCTION: _Record = _Record {
    name: "DebugFunction",
    fields: &[
        ("function", _Hint::Int, true),
        ("module", _Hint::Bool, true),
        ("name", _Hint::Str, true),
        ("type", _Hint::Int, true),
        ("parameters", _Hint::Tuple(&_Hint::Record(&DEBUG_PARAMETER)), true),
        ("variables", _Hint::Tuple(&_Hint::Record(&DEBUG_VARIABLE)), true),
    ],
    build: |args| {
        _object(model::DebugFunction {
            function: _required(args, "function")?,
            module: _required(args, "module")?,
            name: _required(args, "name")?,
            r#type: _required(args, "type")?,
            parameters: _required(args, "parameters")?,
            variables: _required(args, "variables")?,
        })
    },
};

static DEBUG_GLOBAL: _Record = _Record {
    name: "DebugGlobal",
    fields: &[("function", OPTIONAL_INT, false), ("object", _Hint::Int, true), ("offset", _Hint::Int, true), ("name", _Hint::Str, true), ("type", _Hint::Int, true)],
    build: |args| {
        _object(model::DebugGlobal {
            function: _default(args, "function", None)?,
            object: _required(args, "object")?,
            offset: _required(args, "offset")?,
            name: _required(args, "name")?,
            r#type: _required(args, "type")?,
        })
    },
};

static DEBUG: _Record = _Record {
    name: "Debug",
    fields: &[
        ("types", _Hint::Tuple(&_Hint::Record(&DEBUG_TYPE)), true),
        ("functions", _Hint::Tuple(&_Hint::Record(&DEBUG_FUNCTION)), true),
        ("globals", _Hint::Tuple(&_Hint::Record(&DEBUG_GLOBAL)), true),
    ],
    build: |args| _object(model::Debug { types: _required(args, "types")?, functions: _required(args, "functions")?, globals: _required(args, "globals")? }),
};

static STATED_FACT: _Record = _Record {
    name: "StatedFact",
    fields: &[
        ("subject", _Hint::Str, true),
        ("function", OPTIONAL_INT, false),
        ("id", OPTIONAL_INT, false),
        ("part", OPTIONAL_INT, false),
        ("fact", _Hint::Str, true),
        ("value", OPTIONAL_INT, false),
        ("source", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), false),
    ],
    build: |args| {
        let key: String = _required(args, "subject")?;
        let name: String = _required(args, "fact")?;
        let kind = crate::facts::Subject::kind_named(&key).ok_or_else(|| InvalidHIR(format!("unknown fact subject {key:?}")))?;
        let subject = crate::facts::Subject::of(kind, _default(args, "function", None)?, _default(args, "id", None)?, _default(args, "part", None)?)
            .ok_or_else(|| InvalidHIR(format!("a {key} fact needs its function and id")))?;
        let value: Option<i64> = _default(args, "value", None)?;
        let fact = llrm_mir::facts::Fact::from_wire(&name, value).ok_or_else(|| InvalidHIR(format!("{name:?} with value {value:?} is not a fact")))?;
        _object(crate::facts::Stated { subject, fact, source: _default(args, "source", None)? })
    },
};

static ALIAS_CLASS: _Record = _Record {
    name: "AliasClass",
    fields: &[("name", _Hint::Str, true), ("parent", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), true), ("types", INTS, true)],
    build: |args| {
        _object(model::AliasClass { name: _required(args, "name")?, parent: _required(args, "parent")?, types: _required(args, "types")? })
    },
};

static CELL_WRITERS: _Record = _Record {
    name: "CellWriters",
    fields: &[("cell", _Hint::Str, true), ("routines", _Hint::Tuple(&_Hint::Str), true)],
    build: |args| _object(model::CellWriters { cell: _required(args, "cell")?, routines: _required(args, "routines")? }),
};

static RUNTIME_PROMISES: _Record = _Record {
    name: "RuntimePromises",
    fields: &[
        ("calling_back", _Hint::Union(&[_Hint::Tuple(&_Hint::Str), _Hint::NoneType]), false),
        ("writers", _Hint::Tuple(&_Hint::Record(&CELL_WRITERS)), false),
        ("nounwind", _Hint::Tuple(&_Hint::Str), false),
        ("reads_arguments", _Hint::Tuple(&_Hint::Str), false),
    ],
    build: |args| {
        _object(model::RuntimePromises {
            calling_back: _default(args, "calling_back", None)?,
            writers: _default(args, "writers", Vec::new())?,
            nounwind: _default(args, "nounwind", Vec::new())?,
            reads_arguments: _default(args, "reads_arguments", Vec::new())?,
        })
    },
};

static PROGRAM: _Record = _Record {
    name: "Program",
    fields: &[
        ("dialect", enum_hint!(Dialect), true),
        ("runtime", enum_hint!(RuntimeProfile), true),
        ("modules", _Hint::Tuple(&_Hint::Record(&MODULE)), true),
        ("schema", _Hint::Int, false),
        ("target", enum_hint!(TargetProfile), false),
        ("array_order", enum_hint!(ArrayOrder), false),
        ("float_mode", enum_hint!(FloatMode), false),
        ("float_semantics", enum_hint!(FloatSemantics), false),
        ("zeroed_locals", _Hint::Bool, false),
        ("frames", enum_hint!(Frames), false),
        ("promises", _Hint::Record(&RUNTIME_PROMISES), false),
        ("entries", _Hint::Tuple(&_Hint::Str), false),
        ("preserved", _Hint::Tuple(&_Hint::Str), false),
        ("constant_segment", _Hint::Union(&[_Hint::Str, _Hint::NoneType]), false),
    ],
    build: |args| {
        _object(model::Program {
            dialect: _required(args, "dialect")?,
            runtime: _required(args, "runtime")?,
            modules: _required(args, "modules")?,
            schema: _default(args, "schema", model::SCHEMA_VERSION)?,
            target: _default(args, "target", model::TargetProfile::I386RealMode)?,
            array_order: _default(args, "array_order", model::ArrayOrder::ColumnMajor)?,
            float_mode: _default(args, "float_mode", model::FloatMode::Inline)?,
            float_semantics: _default(args, "float_semantics", model::FloatSemantics::Declared)?,
            zeroed_locals: _default(args, "zeroed_locals", true)?,
            frames: _default(args, "frames", model::Frames::Runtime)?,
            promises: _default(args, "promises", model::RuntimePromises::default())?,
            entries: _default(args, "entries", Vec::new())?,
            preserved: _default(args, "preserved", Vec::new())?,
            constant_segment: _default(args, "constant_segment", None)?,
        })
    },
};
