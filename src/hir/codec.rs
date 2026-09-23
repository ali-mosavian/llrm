//! Port of `qbopt/hir/codec.py`: deterministic, strict JSON wire format for
//! HIR producers.
//!
//! Python reflects over each dataclass's type hints; `_Hint` and `_Record`
//! are those hints and fields, written out, so `_make` and `_record` keep
//! Python's walk, order and refusal text.

use std::any::Any;

use crate::support::hash::IndexMap;

use crate::hir::model;
use crate::hir::verify::{InvalidHIR, verify};
use crate::support::pyjson::{self, Json};
use crate::support::pyrepr;

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
    TypeKind,
    AddressKind,
    FloatEvaluation,
    StackCleanup,
    CallDistance,
    Storage,
    DataLinkage,
    FunctionLinkage,
    DescriptorField,
    Op,
    TerminatorKind
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
plain_record!(ValueRef, Some("value"), value => "value");
plain_record!(Constant, Some("constant"), r#type => "type", value => "value");
plain_record!(PlaceRef, Some("place"), place => "place");
plain_record!(ArrayElement, Some("array_element"), place => "place", indices => "indices");
plain_record!(ProjectedPlace, Some("projection"), place => "place", indices => "indices", offset => "offset",
    r#type => "type");
plain_record!(IndirectPlace, Some("indirect"), base => "base", offset => "offset", r#type => "type",
    volatile => "volatile", inbounds => "inbounds");
plain_record!(DescriptorPlace, Some("descriptor"), base => "base", field => "field", r#type => "type");
plain_record!(Instruction, None, id => "id", op => "op", results => "results", operands => "operands",
    callee => "callee", pure => "pure");
plain_record!(Terminator, None, kind => "kind", operands => "operands", targets => "targets", cases => "cases");
plain_record!(Block, None, id => "id", instructions => "instructions", terminator => "terminator", cold => "cold");
plain_record!(CallAbi, None, instruction => "instruction", order => "order", cleanup => "cleanup",
    distance => "distance", callee => "callee");
plain_record!(Callable, None, id => "id", name => "name", result_type => "result_type",
    parameter_types => "parameter_types", by_value => "by_value", segmented => "segmented", arrays => "arrays",
    defined => "defined");
plain_record!(ProcedureAbi, None, cleanup => "cleanup", distance => "distance",
    parameter_bytes => "parameter_bytes");
plain_record!(Function, None, id => "id", name => "name", result_type => "result_type", values => "values",
    places => "places", blocks => "blocks", entry => "entry", parameters => "parameters", abi => "abi",
    calls => "calls", error_handler => "error_handler", error_handler_local => "error_handler_local",
    external_entries => "external_entries", linkage => "linkage");
plain_record!(DataRelocation, None, at => "at", target => "target", addend => "addend", address => "address");
plain_record!(DataObject, None, id => "id", name => "name", bytes => "bytes", readonly => "readonly",
    relocations => "relocations", linkage => "linkage", address => "address", addressed => "addressed");
plain_record!(Module, None, id => "id", name => "name", types => "types", functions => "functions",
    data => "data", callables => "callables");
plain_record!(Program, None, dialect => "dialect", runtime => "runtime", modules => "modules",
    schema => "schema", target => "target", array_order => "array_order", float_mode => "float_mode");

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

pub fn decode(text: &str) -> Result<model::Program, InvalidHIR> {
    let raw = pyjson::loads(text).map_err(|error| InvalidHIR(format!("invalid HIR JSON: {error}")))?;
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
    TypeKind,
    AddressKind,
    FloatEvaluation,
    StackCleanup,
    CallDistance,
    Storage,
    DataLinkage,
    FunctionLinkage,
    DescriptorField,
    Op,
    TerminatorKind
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

made_records!(
    Type,
    Place,
    Value,
    Instruction,
    Terminator,
    Block,
    CallAbi,
    Callable,
    ProcedureAbi,
    Function,
    DataRelocation,
    DataObject,
    Module
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
    ],
    build: |args| {
        _object(model::IndirectPlace {
            base: _required(args, "base")?,
            offset: _required(args, "offset")?,
            r#type: _required(args, "type")?,
            volatile: _default(args, "volatile", false)?,
            inbounds: _default(args, "inbounds", false)?,
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
    ],
    build: |args| {
        _object(model::Instruction {
            id: _required(args, "id")?,
            op: _required(args, "op")?,
            results: _default(args, "results", Vec::new())?,
            operands: _default(args, "operands", Vec::new())?,
            callee: _default(args, "callee", None)?,
            pure: _default(args, "pure", false)?,
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
    ],
    build: |args| {
        _object(model::CallAbi {
            instruction: _required(args, "instruction")?,
            order: _required(args, "order")?,
            cleanup: _required(args, "cleanup")?,
            distance: _required(args, "distance")?,
            callee: _default(args, "callee", None)?,
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
        })
    },
};

static PROCEDURE_ABI: _Record = _Record {
    name: "ProcedureAbi",
    fields: &[
        ("cleanup", enum_hint!(StackCleanup), true),
        ("distance", enum_hint!(CallDistance), true),
        ("parameter_bytes", _Hint::Int, true),
    ],
    build: |args| {
        _object(model::ProcedureAbi {
            cleanup: _required(args, "cleanup")?,
            distance: _required(args, "distance")?,
            parameter_bytes: _required(args, "parameter_bytes")?,
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
    ],
    build: |args| {
        _object(model::DataRelocation {
            at: _required(args, "at")?,
            target: _required(args, "target")?,
            addend: _required(args, "addend")?,
            address: _required(args, "address")?,
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
    ],
    build: |args| {
        _object(model::Module {
            id: _required(args, "id")?,
            name: _required(args, "name")?,
            types: _required(args, "types")?,
            functions: _required(args, "functions")?,
            data: _default(args, "data", Vec::new())?,
            callables: _default(args, "callables", Vec::new())?,
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
        })
    },
};
