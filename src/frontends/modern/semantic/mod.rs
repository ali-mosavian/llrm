use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::arguments;
use super::arguments::Formal;
use super::conversions;
use super::conversions::Rules;
use super::error::Diagnostic;
use super::hir;
use super::lexer;
use super::parser;
use super::syntax::Abi;
use super::syntax::AssignTarget;
use super::syntax::BinaryOp;
use super::syntax::Clause;
use super::syntax::Expr;
use super::syntax::FixedStorage;
use super::syntax::FixedType;
use super::syntax::Function;
use super::syntax::IterationMode;
use super::syntax::MAX_RANK;
use super::syntax::Module;
use super::syntax::Parameter;
use super::syntax::ParameterType;
use super::syntax::Span;
use super::syntax::Statement;
use super::syntax::Struct;
use super::syntax::TUPLE;
use super::syntax::FUNCTION;
use super::syntax::TypeAnnotation;
use super::syntax::TypeName;
use super::syntax::TypeSpec;
use super::syntax::UnaryOp;
use super::syntax::{FStringPart, Format};

mod statements;
mod loops;
mod places;
mod expressions;
mod indexing;
mod operators;
mod hints;
mod arrays;
mod calls;
mod printing;
mod emission;
mod bits;
mod function_values;
mod checks;
mod conditional;
mod division;
mod enums;
mod exhaustive;
mod failure;
mod drops;
mod foreign;
mod generators;
mod generics;
mod borrows;
mod dictionaries;
mod references;
mod instances;
mod iterators;
mod views;
mod lambdas;
mod matching;
mod methods;
mod moves;
mod ownership;
mod runtime;
mod strings;
mod tuples;
mod vectors;
use ownership::Owned;
mod properties;
mod results;
mod sequences;

const VOID: u32 = 1;
const BOOL: u32 = 2;
const CHAR: u32 = 3;
const I8: u32 = 4;
const U8: u32 = 5;
const I16: u32 = 6;
const U16: u32 = 7;
const I32: u32 = 8;
const U32: u32 = 9;
const F32: u32 = 10;
const F64: u32 = 11;
const STRING: u32 = 12;
const ADDR: u32 = 13;
const I64: u32 = 14;
const FIXED_START: u32 = 15;

/// A descriptor's flags byte (section 13).
const STRING_READONLY: u8 = 0x08;

/// The binding of an aggregate result's slot.
const RESULT: &str = "$result";

#[derive(Default)]
struct LiteralPool {
    floats: BTreeMap<(TypeName, u64), u32>,
    strings: BTreeMap<Vec<u8>, u32>,
    data: Vec<hir::DataObject>,
}

impl LiteralPool {
    fn float(&mut self, type_name: TypeName, bits: u64) -> u32 {
        if let Some(symbol) = self.floats.get(&(type_name, bits)) {
            return *symbol;
        }
        let id = self.data.len() as u32 + 1;
        let (name, bytes) = match type_name {
            TypeName::F32 => {
                let bits = bits as u32;
                (format!("$f32_{bits:08x}"), bits.to_le_bytes().to_vec())
            }
            TypeName::F64 => (format!("$f64_{bits:016x}"), bits.to_le_bytes().to_vec()),
            _ => unreachable!("only floats enter the constant pool"),
        };
        self.floats.insert((type_name, bits), id);
        self.data.push(hir::DataObject { id, name, bytes });
        id
    }

    fn string(&mut self, value: &[u8]) -> u32 {
        if let Some(symbol) = self.strings.get(value) {
            return *symbol;
        }
        let id = self.data.len() as u32 + 1;
        let length =
            u16::try_from(value.len()).expect("string length checked by semantic analysis");
        let mut bytes = Vec::with_capacity(value.len() + 7);
        // A literal is static and read-only: the first write copies it to the heap.
        bytes.extend([STRING_READONLY, 0]);
        bytes.extend(length.to_le_bytes());
        bytes.extend(length.to_le_bytes());
        bytes.extend(value);
        bytes.push(0);
        self.strings.insert(value.to_vec(), id);
        self.data.push(hir::DataObject {
            id,
            name: format!("$str{id}"),
            bytes,
        });
        id
    }
}

struct TypeRegistry {
    types: Vec<hir::Type>,
    arrays: BTreeMap<(u32, Shape), u32>,
    structs: BTreeMap<String, StructLayout>,
    enums: BTreeMap<String, enums::EnumLayout>,
    templates: BTreeMap<String, generics::Template>,
    /// Each instance of a generic type, by its spelling: the applied type it is.
    applied: BTreeMap<String, TypeSpec>,
    /// Each reference type, by its pointer type: what it refers to.
    referents: BTreeMap<u32, ElementType>,
    fixed_names: BTreeMap<String, TypeName>,
    pointers: BTreeMap<(u32, u32), u32>,
    /// Raw pointers, by name, and what each points to.
    raw_pointers: BTreeMap<String, u32>,
    raw_targets: BTreeMap<u32, ElementType>,
    /// Structs whose `@repr` fixes their layout for a foreign ABI.
    represented: BTreeSet<u32>,
    /// Structs with a `drop` method, and its name.
    dropped: BTreeMap<u32, String>,
    /// Each `vec[T]` type's element.
    vectors: BTreeMap<u32, ElementType>,
    /// Each dict type's key, value and entry struct.
    dictionaries: BTreeMap<u32, (ElementType, ElementType, u32)>,
    bits: BTreeMap<String, bits::BitsLayout>,
    /// Each function type, by its HIR type.
    function_types: BTreeMap<u32, function_values::FunctionType>,
    /// Each declared field not declared `mut`, by its type's name.
    fixed_fields: BTreeSet<(String, String)>,
    slice_descriptors: BTreeMap<(u32, u8), u32>,
}

#[derive(Clone, Debug)]
struct StructLayout {
    id: u32,
    name: String,
    fields: BTreeMap<String, FieldLayout>,
    /// The source fields in declaration order, for positional patterns.
    order: Vec<String>,
    /// What a copy moves, as (offset, type): a struct's scalar leaves, an
    /// enum's whole words, since its variants' fields overlap.
    copy: Vec<(u32, TypeName)>,
}

#[derive(Clone, Copy, Debug)]
struct FieldLayout {
    type_: ElementType,
    offset: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ElementType {
    Scalar(TypeName),
    Struct(u32),
}

impl ElementType {
    fn id(self) -> u32 {
        match self {
            Self::Scalar(type_name) => type_id(type_name),
            Self::Struct(id) => id,
        }
    }
}

impl TypeRegistry {
    fn new() -> Self {
        Self {
            types: vec![
                plain_type(VOID, "void", "void", 0, None, "none"),
                plain_type(BOOL, "bool", "boolean", 1, None, "none"),
                plain_type(CHAR, "char", "integer", 1, Some(false), "none"),
                plain_type(I8, "i8", "integer", 1, Some(true), "none"),
                plain_type(U8, "u8", "integer", 1, Some(false), "none"),
                plain_type(I16, "i16", "integer", 2, Some(true), "none"),
                plain_type(U16, "u16", "integer", 2, Some(false), "none"),
                plain_type(I32, "i32", "integer", 4, Some(true), "none"),
                plain_type(U32, "u32", "integer", 4, Some(false), "none"),
                plain_type(F32, "f32", "float", 4, None, "extended80"),
                plain_type(F64, "f64", "float", 8, None, "extended80"),
                hir::Type {
                    id: STRING,
                    name: "string".into(),
                    kind: "pointer",
                    width: 2,
                    signed: None,
                    evaluation: "none",
                    element: Some(CHAR),
                    rank: 0,
                    bounds: Vec::new(),
                    address: "near",
                },
                hir::Type {
                    id: ADDR,
                    name: "addr".into(),
                    kind: "pointer",
                    width: 4,
                    signed: None,
                    evaluation: "none",
                    element: Some(U8),
                    rank: 0,
                    bounds: Vec::new(),
                    address: "far",
                },
            ],
            arrays: BTreeMap::new(),
            structs: BTreeMap::new(),
            enums: BTreeMap::new(),
            templates: BTreeMap::new(),
            applied: BTreeMap::new(),
            referents: BTreeMap::new(),
            fixed_names: BTreeMap::new(),
            pointers: BTreeMap::new(),
            raw_pointers: BTreeMap::new(),
            raw_targets: BTreeMap::new(),
            represented: BTreeSet::new(),
            dropped: BTreeMap::new(),
            slice_descriptors: BTreeMap::new(),
            vectors: BTreeMap::new(),
            dictionaries: BTreeMap::new(),
            fixed_fields: BTreeSet::new(),
            function_types: BTreeMap::new(),
            bits: BTreeMap::new(),
        }
    }

    fn register_fixed_types(&mut self, declarations: &[FixedType]) -> Result<(), Diagnostic> {
        if declarations.is_empty() {
            return Ok(());
        }
        self.types
            .push(plain_type(I64, "$i64", "integer", 8, Some(true), "none"));
        for declaration in declarations {
            if self.fixed_names.contains_key(&declaration.name) {
                return Err(Diagnostic::new(
                    declaration.span,
                    format!("type {:?} is declared more than once", declaration.name),
                ));
            }
            let TypeName::Fixed {
                storage,
                declaration: ordinal,
                ..
            } = declaration.type_name
            else {
                unreachable!("only fixed types are registered here")
            };
            let id = FIXED_START + u32::from(ordinal);
            if id != self.types.len() as u32 + 1 {
                return Err(Diagnostic::new(
                    declaration.span,
                    "fixed-point declarations are out of order",
                ));
            }
            self.types.push(plain_type(
                id,
                &declaration.name,
                "integer",
                match storage {
                    FixedStorage::I16 => 2,
                    FixedStorage::I32 => 4,
                },
                Some(true),
                "none",
            ));
            self.fixed_names
                .insert(declaration.name.clone(), declaration.type_name);
        }
        Ok(())
    }

    pub(super) fn register_struct(&mut self, declaration: &Struct) -> Result<(), Diagnostic> {
        if self.declared(&declaration.name) {
            return Err(Diagnostic::new(
                declaration.span,
                format!("type {:?} is declared more than once", declaration.name),
            ));
        }
        if let Some(backing) = declaration.bits {
            return self.register_bits(declaration, backing);
        }
        let mut fields = BTreeMap::new();
        let mut copy = Vec::new();
        let mut offset = 0;
        let mut alignment = 1;
        for field in &declaration.fields {
            if fields.contains_key(&field.name) {
                return Err(Diagnostic::new(
                    field.span,
                    format!("field {:?} is declared more than once", field.name),
                ));
            }
            let field_type = self.resolve_element(&field.type_spec, field.span)?;
            let field_width = self.width(field_type.id());
            let field_alignment = field_width.clamp(1, declaration.pack.unwrap_or(2));
            offset = align_up(offset, field_alignment);
            fields.insert(
                field.name.clone(),
                FieldLayout {
                    type_: field_type,
                    offset,
                },
            );
            copy.extend(
                self.copy_units(field_type)
                    .into_iter()
                    .map(|(at, one)| (offset + at, one)),
            );
            offset += field_width;
            alignment = alignment.max(field_alignment);
        }
        let width = align_up(offset, alignment);
        let order = declaration
            .fields
            .iter()
            .map(|one| one.name.clone())
            .collect();
        let id = self.aggregate(&declaration.name, width, fields, order, copy);
        if declaration.pack.is_some() {
            self.represented.insert(id);
        }
        Ok(())
    }

    /// Registers a struct-like layout and returns its type.
    fn aggregate(
        &mut self,
        name: &str,
        width: u32,
        fields: BTreeMap<String, FieldLayout>,
        order: Vec<String>,
        copy: Vec<(u32, TypeName)>,
    ) -> u32 {
        let id = self.types.len() as u32 + 1;
        self.types.push(hir::Type {
            id,
            name: name.into(),
            kind: "opaque",
            width,
            signed: None,
            evaluation: "none",
            element: None,
            rank: 0,
            bounds: Vec::new(),
            address: "none",
        });
        self.structs.insert(
            name.into(),
            StructLayout {
                id,
                name: name.into(),
                fields,
                order,
                copy,
            },
        );
        id
    }

    fn copy_units(&self, element: ElementType) -> Vec<(u32, TypeName)> {
        match element {
            ElementType::Scalar(type_name) => vec![(0, type_name)],
            ElementType::Struct(id) => self.structure(id).expect("registered layout").copy.clone(),
        }
    }

    fn declared(&self, name: &str) -> bool {
        self.structs.contains_key(name)
            || self.enums.contains_key(name)
            || self.bits.contains_key(name)
            || self.fixed_names.contains_key(name)
    }

    fn resolve_element(&mut self, spec: &TypeSpec, span: Span) -> Result<ElementType, Diagnostic> {
        match spec {
            TypeSpec::Primitive(type_name) => Ok(ElementType::Scalar(*type_name)),
            TypeSpec::Applied { name, args } if name == "vec" => match args.as_slice() {
                [TypeAnnotation::Value(element)] => {
                    let element = self.resolve_element(element, span)?;
                    Ok(ElementType::Scalar(self.vector(element)))
                }
                _ => Err(Diagnostic::new(span, "vec takes one element type")),
            },
            TypeSpec::Applied { name, args } if name == "dict" => match args.as_slice() {
                [TypeAnnotation::Value(key), TypeAnnotation::Value(value)] => {
                    let (key, value) = (self.resolve_element(key, span)?, self.resolve_element(value, span)?);
                    Ok(ElementType::Scalar(self.dictionary(key, value, span)?))
                }
                _ => Err(Diagnostic::new(span, "dict takes a key type and a value type")),
            },
            TypeSpec::Applied { name, args } if name == TUPLE => {
                let elements = args
                    .iter()
                    .map(|arg| match arg {
                        TypeAnnotation::Value(element) => self.resolve_element(element, span),
                        _ => Err(Diagnostic::new(
                            span,
                            "a tuple element cannot be an array yet",
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ElementType::Struct(self.tuple(&elements, span)?))
            }
            TypeSpec::Applied { name, args } if name == FUNCTION => Ok(ElementType::Scalar(self.function_type_spelled(args, span)?)),
            TypeSpec::Applied { name, args } if name.starts_with('&') => match args.as_slice() {
                [TypeAnnotation::Value(target)] => {
                    let target = self.resolve_element(target, span)?;
                    Ok(ElementType::Scalar(self.reference(target, name == "&mut")))
                }
                _ => Err(Diagnostic::new(span, "a reference takes one target type")),
            },
            TypeSpec::Applied { name, args } if name.starts_with('*') => match args.as_slice() {
                [TypeAnnotation::Value(target)] => {
                    let target = self.resolve_element(target, span)?;
                    let distance = name[1..].split(' ').next().expect("a distance");
                    Ok(ElementType::Scalar(self.raw_pointer(target, distance, name.ends_with(" mut"))))
                }
                _ => Err(Diagnostic::new(span, "a raw pointer takes one target type")),
            },
            TypeSpec::Applied { .. } => self.instantiate(spec, span),
            TypeSpec::Named(name) => self
                .fixed_names
                .get(name)
                .copied()
                .map(ElementType::Scalar)
                .or_else(|| {
                    self.structs
                        .get(name)
                        .map(|one| ElementType::Struct(one.id))
                })
                .or_else(|| self.enums.get(name).map(|one| one.element))
                .or_else(|| {
                    self.bits
                        .get(name)
                        .map(|one| ElementType::Scalar(one.type_name))
                })
                .ok_or_else(|| Diagnostic::new(span, format!("unknown type {name:?}"))),
        }
    }

    fn array(&mut self, element: ElementType, shape: Shape) -> u32 {
        let element_id = element.id();
        if let Some(id) = self.arrays.get(&(element_id, shape)) {
            return *id;
        }
        let element_type = &self.types[(element_id - 1) as usize];
        let element_name = element_type.name.clone();
        let element_width = element_type.width;
        let id = self.types.len() as u32 + 1;
        let dims = shape
            .dims()
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        self.types.push(hir::Type {
            id,
            name: format!("[{element_name}; {dims}]"),
            kind: "array",
            width: element_width * shape.len(),
            signed: None,
            evaluation: "none",
            element: Some(element_id),
            rank: u32::from(shape.rank),
            bounds: shape
                .dims()
                .iter()
                .map(|one| (0, i32::try_from(one - 1).expect("array length checked")))
                .collect(),
            address: "near",
        });
        self.arrays.insert((element_id, shape), id);
        id
    }

    fn width(&self, id: u32) -> u32 {
        self.types[(id - 1) as usize].width
    }

    fn pointer(&mut self, target: u32, rank: u32) -> u32 {
        if let Some(id) = self.pointers.get(&(target, rank)) {
            return *id;
        }
        let name = format!("&{}", self.types[(target - 1) as usize].name);
        let id = self.pointer_type(name, target, rank, true);
        self.pointers.insert((target, rank), id);
        id
    }

    /// A new pointer type to `target`, far or near.
    fn pointer_type(&mut self, name: String, target: u32, rank: u32, far: bool) -> u32 {
        let id = self.types.len() as u32 + 1;
        self.types.push(hir::Type {
            id,
            name,
            kind: "pointer",
            width: if far { 4 } else { 2 },
            signed: None,
            evaluation: "none",
            element: Some(target),
            rank,
            bounds: Vec::new(),
            address: if far { "far" } else { "near" },
        });
        id
    }

    fn slice_descriptor(&mut self, element: ElementType, rank: u8) -> u32 {
        let element_id = element.id();
        if let Some(id) = self.slice_descriptors.get(&(element_id, rank)) {
            return *id;
        }
        let id = self.types.len() as u32 + 1;
        let element_name = self.types[(element_id - 1) as usize].name.clone();
        let name = if rank == 1 {
            format!("$slice[{element_name}]")
        } else {
            format!("$slice[{element_name}, {rank}]")
        };
        self.types.push(hir::Type {
            id,
            name,
            kind: "opaque",
            width: descriptor::size(rank) + 4,
            signed: None,
            evaluation: "none",
            element: Some(element_id),
            rank: 0,
            bounds: Vec::new(),
            address: "none",
        });
        self.slice_descriptors.insert((element_id, rank), id);
        id
    }

    fn slice_pointer(&mut self, element: ElementType, rank: u8) -> u32 {
        let descriptor = self.slice_descriptor(element, rank);
        self.pointer(descriptor, 1)
    }

    fn parameter_target(
        &mut self,
        annotation: &TypeAnnotation,
        span: Span,
    ) -> Result<(BindingType, u32), Diagnostic> {
        match annotation {
            TypeAnnotation::Value(spec) => {
                let element = self.resolve_element(spec, span)?;
                Ok((
                    match element {
                        ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                        ElementType::Struct(id) => BindingType::Struct(id),
                    },
                    element.id(),
                ))
            }
            TypeAnnotation::Slice { element, rank } => {
                let element = self.resolve_element(element, span)?;
                Ok((
                    BindingType::Slice {
                        element,
                        rank: *rank,
                    },
                    element.id(),
                ))
            }
            TypeAnnotation::Array { element, dims } => {
                let element = self.resolve_element(element, span)?;
                let shape = Shape::new(dims);
                let id = self.array(element, shape);
                Ok((BindingType::Array { element, shape }, id))
            }
        }
    }

    fn structure(&self, id: u32) -> Option<&StructLayout> {
        self.structs.values().find(|one| one.id == id)
    }
}

fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn plain_type(
    id: u32,
    name: &str,
    kind: &'static str,
    width: u32,
    signed: Option<bool>,
    evaluation: &'static str,
) -> hir::Type {
    hir::Type {
        id,
        name: name.into(),
        kind,
        width,
        signed,
        evaluation,
        element: None,
        rank: 0,
        bounds: Vec::new(),
        address: "none",
    }
}

impl Signature {
    /// The bytes its arguments take on the stack, each at least a word.
    fn argument_bytes(&self, types: &TypeRegistry) -> u32 {
        self.parameters.iter().map(|one| types.width(one.hir_type()).max(2)).sum()
    }

    fn callable(&self, types: &mut TypeRegistry) -> hir::Callable {
        hir::Callable {
            id: self.id,
            name: self.name.clone(),
            result_type: Some(self.returned(types)).filter(|one| *one != TypeName::Void).map(type_id),
            parameter_types: self
                .slot_pointer(types)
                .into_iter()
                .chain(self.parameters.iter().map(|one| one.hir_type()))
                .collect(),
            defined: !self.foreign,
        }
    }
}

#[derive(Clone, Debug)]
struct Signature {
    id: u32,
    name: String,
    parameters: Vec<SignatureParameter>,
    /// Each parameter's name and default, for binding named arguments.
    formals: Vec<(String, Option<Expr>)>,
    /// `void` when the result goes to the slot.
    result: TypeName,
    /// An aggregate result's layout, written through a hidden first parameter
    /// that points at storage the caller provides.
    slot: Option<u32>,
    /// Defined by another object, called through a foreign ABI.
    foreign: bool,
    /// Defined here and callable from other objects.
    exported: bool,
    abi: Abi,
    /// A view result's element and rank: its descriptor goes to the slot.
    view: Option<(ElementType, u8)>,
    method: bool,
}

#[derive(Clone, Copy, Debug)]
enum SignatureParameter {
    Scalar(TypeName),
    /// An owned aggregate, passed as a far pointer to the caller's copy.
    Owned {
        struct_id: u32,
        pointer: u32,
    },
    Borrowed {
        mutable: bool,
        target: BindingType,
        pointer: u32,
    },
}

impl SignatureParameter {
    fn hir_type(self) -> u32 {
        match self {
            Self::Scalar(type_name) => type_id(type_name),
            Self::Owned { pointer, .. } | Self::Borrowed { pointer, .. } => pointer,
        }
    }
}

#[derive(Clone, Debug)]
enum Storage {
    Parameter(u32),
    Reference(u32),
    Slice(u32),
    Place(u32),
    ArrayView {
        place: u32,
        index: hir::Operand,
    },
    /// A lambda, compiled where it is called: its index in `lambdas`.
    Lambda(u32),
}

/// Where `break` and `continue` go, and the scopes each leaves.
#[derive(Clone, Copy, Debug)]
struct Loop {
    exit: u32,
    exit_depth: usize,
    next: u32,
    next_depth: usize,
}

impl Loop {
    fn new(exit: u32, next: u32, depth: usize) -> Self {
        Self {
            exit,
            exit_depth: depth,
            next,
            next_depth: depth,
        }
    }
}

/// Where a struct binding's bytes are; a by-value parameter has none.
fn binding_view(
    struct_id: u32,
    storage: &Storage,
    mutable: bool,
    owner: &str,
) -> Option<StructView> {
    let (place, pointer, indices) = match storage {
        Storage::Place(place) => (*place, None, Vec::new()),
        Storage::ArrayView { place, index } => (*place, None, vec![index.clone()]),
        Storage::Reference(pointer) => (0, Some(*pointer), Vec::new()),
        Storage::Parameter(_) => return None,
        Storage::Slice(_) | Storage::Lambda(_) => {
            unreachable!("a struct binding is a place or a reference")
        }
    };
    Some(StructView {
        struct_id,
        place,
        pointer,
        indices,
        offset: 0,
        mutable,
        owner: owner.into(),
    })
}

#[derive(Clone, Debug)]
struct Binding {
    type_: BindingType,
    mutable: bool,
    storage: Storage,
}

#[derive(Clone, Debug)]
struct StructView {
    struct_id: u32,
    place: u32,
    pointer: Option<u32>,
    indices: Vec<hir::Operand>,
    offset: u32,
    mutable: bool,
    owner: String,
}

#[derive(Clone, Debug)]
enum AssignmentPlace {
    Scalar(hir::Operand, TypeName),
    Struct(StructView),
    /// A field of the bits value of type `packed` kept at `place`.
    Bits {
        place: hir::Operand,
        packed: TypeName,
        field: bits::BitField,
    },
}

/// Where an indexed element lives.
enum ElementAt {
    Element(u32, Vec<hir::Operand>),
    Pointer(u32),
}

impl ElementAt {
    fn operand(self, type_id: u32) -> hir::Operand {
        match self {
            Self::Element(place, indices) => hir::Operand::ArrayElement(place, indices),
            Self::Pointer(base) => hir::Operand::IndirectPlace {
                base,
                offset: 0,
                type_id,
                inbounds: true,
            },
        }
    }

    /// A struct view's place, pointer, and indices.
    fn parts(self) -> (u32, Option<u32>, Vec<hir::Operand>) {
        match self {
            Self::Element(place, indices) => (place, None, indices),
            Self::Pointer(pointer) => (0, Some(pointer), Vec::new()),
        }
    }
}

/// A fixed array's dimensions, row-major: the last index is contiguous.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Shape {
    rank: u8,
    dims: [u32; MAX_RANK],
}

impl Shape {
    fn new(dims: &[u32]) -> Self {
        let mut all = [1; MAX_RANK];
        all[..dims.len()].copy_from_slice(dims);
        Self {
            rank: dims.len() as u8,
            dims: all,
        }
    }

    fn dims(&self) -> &[u32] {
        &self.dims[..self.rank as usize]
    }

    fn len(&self) -> u32 {
        self.dims().iter().product()
    }

    /// Each dimension's stride in elements; the last one is 1.
    fn strides(&self) -> Vec<u32> {
        let dims = self.dims();
        (0..dims.len())
            .map(|axis| dims[axis + 1..].iter().product())
            .collect()
    }

    /// The descriptor's words, in order, with their names.
    fn descriptor(&self) -> Vec<(String, u32)> {
        let mut words = Vec::new();
        for (axis, length) in self.dims().iter().enumerate() {
            let name = if self.rank == 1 {
                "length".into()
            } else {
                format!("dim{axis}")
            };
            words.push((name, *length));
        }
        words.push(("capacity".into(), self.len()));
        words
    }
}

/// The descriptor before an array's data, or in a view before its data
/// pointer: the dimensions, then the capacity, each a u16 word. Storage is
/// row-major and contiguous, so the strides follow from the dimensions.
mod descriptor {
    pub fn dim(axis: u8) -> u32 {
        2 * u32::from(axis)
    }

    pub fn capacity(rank: u8) -> u32 {
        dim(rank)
    }

    /// The descriptor's size, and so a view's data pointer offset.
    pub fn size(rank: u8) -> u32 {
        capacity(rank) + 2
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingType {
    Scalar(TypeName),
    Slice {
        element: ElementType,
        rank: u8,
    },
    Array {
        element: ElementType,
        shape: Shape,
    },
    Struct(u32),
}

impl BindingType {
    /// A one-dimensional sequence's element and, when fixed, its length.
    fn array(self) -> Option<(ElementType, Option<u32>)> {
        match self {
            Self::Slice { element, rank: 1 } => Some((element, None)),
            Self::Array { element, shape } if shape.rank == 1 => Some((element, Some(shape.len()))),
            _ => None,
        }
    }

    /// Any array's element, rank, and shape when fixed.
    fn ranked(self) -> Option<(ElementType, u8, Option<Shape>)> {
        match self {
            Self::Slice { element, rank } => Some((element, rank, None)),
            Self::Array { element, shape } => Some((element, shape.rank, Some(shape))),
            Self::Scalar(_) | Self::Struct(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
struct TypedOperand {
    operand: Option<hir::Operand>,
    type_name: TypeName,
}

#[derive(Clone, Debug)]
struct BlockBuilder {
    id: u32,
    instructions: Vec<hir::Instruction>,
    terminator: Option<hir::Terminator>,
}

/// A concrete function's parameters and result, as its callers see them.
fn signature(
    types: &mut TypeRegistry,
    function: &Function,
    id: u32,
) -> Result<Signature, Diagnostic> {
    let mut parameters = BTreeMap::new();
    for parameter in &function.parameters {
        if parameters.insert(&parameter.name, parameter.span).is_some() {
            return Err(Diagnostic::new(
                parameter.span,
                format!("parameter {:?} is declared more than once", parameter.name),
            ));
        }
        // A default is evaluated at each call site, in the caller's scope.
        if let Some(default) = parameter.default.as_ref().filter(|one| !is_literal(one)) {
            return Err(Diagnostic::new(
                default.span(),
                "a default must be a literal",
            ));
        }
    }
    let resolved_parameters = function
        .parameters
        .iter()
        .map(|parameter| parameter_kind(types, parameter))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let mut view = None;
    let (result, slot) = match types.parameter_target(&function.result, function.span)? {
        (BindingType::Scalar(type_name), _) => (type_name, None),
        (BindingType::Struct(struct_id), _) => (TypeName::Void, Some(struct_id)),
        (BindingType::Slice { element, rank }, _) => {
            view = Some((element, rank));
            (TypeName::Void, None)
        }
        _ => {
            return Err(Diagnostic::new(
                function.span,
                "a function cannot return an array",
            ));
        }
    };
    Ok(Signature {
        id,
        name: function.name.clone(),
        parameters: resolved_parameters,
        formals: function
            .parameters
            .iter()
            .map(|one| (one.name.clone(), one.default.clone()))
            .collect(),
        result,
        slot,
        foreign: false,
        exported: false,
        abi: Abi::Cdecl16,
        view,
        method: function.is_method(),
    })
}

fn parameter_kind(
    types: &mut TypeRegistry,
    parameter: &Parameter,
) -> Result<SignatureParameter, Diagnostic> {
    match &parameter.type_ {
        ParameterType::Owned(annotation) => {
            match types.parameter_target(annotation, parameter.span)? {
                (BindingType::Scalar(type_name), _) => Ok(SignatureParameter::Scalar(type_name)),
                (BindingType::Struct(struct_id), _) => Ok(SignatureParameter::Owned {
                    struct_id,
                    pointer: types.pointer(struct_id, 0),
                }),
                _ => Err(Diagnostic::new(
                    parameter.span,
                    "an array parameter is borrowed: '&T[N]' or '&[T]'",
                )),
            }
        }
        ParameterType::Borrowed { mutable, target } => {
            let (mut target, target_id) = types.parameter_target(target, parameter.span)?;
            // `&string` is a view, the descriptor `&[char]` is; `&mut string`
            // borrows the owned string, which may grow.
            if target == BindingType::Scalar(TypeName::String) && !mutable {
                target = BindingType::Slice {
                    element: ElementType::Scalar(TypeName::Char),
                    rank: 1,
                };
            }
            let pointer = match target {
                BindingType::Slice { element, rank } => types.slice_pointer(element, rank),
                _ => types.pointer(target_id, 0),
            };
            Ok(SignatureParameter::Borrowed {
                mutable: *mutable,
                target,
                pointer,
            })
        }
    }
}

pub fn compile(module: &Module, module_name: &str) -> Result<String, Diagnostic> {
    let mut types = TypeRegistry::new();
    types.register_fixed_types(&module.fixed_types)?;
    types.register_aggregates(&module.structs, &module.enums)?;
    types.register_drops(&module.functions.iter().collect::<Vec<_>>())?;
    let declared: Vec<Function> = module.functions.iter().map(|one| types.with_owner_generics(one)).collect();
    let (generators, functions): (Vec<&Function>, Vec<&Function>) = declared
        .iter()
        .partition(|one| generators::is_generator(one));
    let (templates, concrete): (Vec<&Function>, Vec<&Function>) = functions
        .into_iter()
        .partition(|one| !one.generics.is_empty());
    for function in &module.functions {
        let owner = function.name.split_once('.').map(|(owner, _)| owner);
        if owner.and_then(lexer::keyword).as_ref().and_then(parser::primitive).is_some() {
            return Err(Diagnostic::new(
                function.span,
                format!("only the language defines {}'s methods", owner.expect("a method")),
            ));
        }
    }
    let mut signatures = BTreeMap::new();
    for (index, function) in concrete.iter().enumerate() {
        if signatures.contains_key(&function.name) {
            return Err(Diagnostic::new(
                function.span,
                format!("function {:?} is declared more than once", function.name),
            ));
        }
        let mut signature = signature(&mut types, function, index as u32 + 1)?;
        if let Some(abi) = module.exports.get(&function.name) {
            foreign::check_foreign(&types, &signature, &function.name, function.span)?;
            signature.exported = true;
            signature.abi = *abi;
            signature.name = abi.symbol(&function.name);
        }
        signatures.insert(function.name.clone(), signature);
    }
    for declared in &module.externs {
        let id = signatures.len() as u32 + 1;
        let name = declared.function.name.clone();
        if signatures.contains_key(&name) {
            return Err(Diagnostic::new(
                declared.function.span,
                format!("function {name:?} is declared more than once"),
            ));
        }
        signatures.insert(name, foreign::foreign_signature(&mut types, declared, id)?);
    }

    let mut callables: Vec<_> = signatures
        .values()
        .map(|signature| signature.callable(&mut types))
        .collect();
    let mut builtin_ids = BTreeMap::new();
    let routines = print_builtins()
        .into_iter()
        .map(|(name, parameters)| (name, parameters, TypeName::Void))
        .chain(runtime::routines());
    for (name, parameters, result) in routines {
        let id = callables.len() as u32 + 1;
        builtin_ids.insert(name, (id, result));
        callables.push(hir::Callable {
            id,
            name: name.into(),
            result_type: (result != TypeName::Void).then(|| type_id(result)),
            parameter_types: parameters.into_iter().map(type_id).collect(),
            defined: false,
        });
    }
    let templates = RefCell::new(instances::Templates::new(
        templates,
        generators,
        &module.protocols,
        &module.library,
        callables.len() as u32 + 1,
    ));
    let mut functions = Vec::new();
    let mut literals = LiteralPool::default();
    for function in concrete {
        let signature = signatures.get(&function.name).expect("collected function");
        functions.push(
            FunctionCompiler::new(
                function,
                signature,
                &signatures,
                &templates,
                &builtin_ids,
                &mut literals,
                &mut types,
            )?
            .compile(function)?,
        );
    }
    // Each instance a call asked for, and those its own calls ask for; then
    // each dispatcher a call through a function value needs, once every
    // function converted to its type is known.
    let mut dispatchers = Vec::new();
    loop {
        let pending = templates.borrow_mut().next_pending();
        if pending.is_none() && dispatchers.is_empty() {
            dispatchers = types.dispatcher_bodies(Span { line: 0, column: 0, end_column: 0 });
        }
        let Some((function, signature)) = pending.or_else(|| {
            let dispatcher = dispatchers.pop()?;
            let signature = templates.borrow().instance(&dispatcher.name).expect("declared");
            Some((dispatcher, signature))
        }) else {
            break;
        };
        functions.push(
            FunctionCompiler::new(
                &function,
                &signature,
                &signatures,
                &templates,
                &builtin_ids,
                &mut literals,
                &mut types,
            )?
            .compile(&function)?,
        );
    }
    callables.extend(templates.borrow().callables(&mut types));
    let program = hir::Program {
        module_name: module_name.into(),
        types: types.types,
        functions,
        callables,
        data: literals.data,
    };
    Ok(program.json())
}

fn print_builtins() -> Vec<(&'static str, Vec<TypeName>)> {
    let mut out = vec![
        ("_pn", Vec::new()),
        (print_name(TypeName::String), vec![TypeName::String]),
    ];
    for type_name in [
        TypeName::Bool,
        TypeName::Char,
        TypeName::I8,
        TypeName::U8,
        TypeName::I16,
        TypeName::U16,
        TypeName::I32,
        TypeName::U32,
        TypeName::F32,
        TypeName::F64,
    ] {
        out.push((print_name(type_name), vec![type_name]));
    }
    // These are formatting boundaries, not arithmetic helpers. They receive
    // the signed raw storage value followed by its fractional-bit count and
    // write canonical base-10 integer.fraction text. The formatter keeps one
    // digit after the point and trims any further trailing zeroes.
    out.push(("_pf2", vec![TypeName::I16, TypeName::U8]));
    out.push(("_pf4", vec![TypeName::I32, TypeName::U8]));
    // A `&string` view: its far data and length.
    out.push(("_pv", vec![TypeName::Addr, TypeName::U16]));
    out
}

fn print_name(type_name: TypeName) -> &'static str {
    match type_name {
        TypeName::String => "_pt",
        TypeName::Addr => unreachable!("addresses have no default formatter"),
        TypeName::Enum { .. }
        | TypeName::Vector { .. }
        | TypeName::Dictionary { .. }
        | TypeName::Function { .. }
        | TypeName::Bits { .. }
        | TypeName::Pointer { .. } => {
            unreachable!("no default formatter")
        }
        TypeName::Bool => "_pb",
        TypeName::Char => "_pc",
        TypeName::I8 => "_pi1",
        TypeName::U8 => "_pu1",
        TypeName::I16 => "_pi2",
        TypeName::U16 => "_pu2",
        TypeName::I32 => "_pi4",
        TypeName::U32 => "_pu4",
        TypeName::F32 => "_pr4",
        TypeName::F64 => "_pr8",
        TypeName::Fixed {
            storage: FixedStorage::I16,
            ..
        } => "_pf2",
        TypeName::Fixed {
            storage: FixedStorage::I32,
            ..
        } => "_pf4",
        TypeName::Void | TypeName::I64 => unreachable!(),
    }
}

struct FunctionCompiler<'a> {
    signature: &'a Signature,
    signatures: &'a BTreeMap<String, Signature>,
    templates: &'a RefCell<instances::Templates>,
    /// Each runtime routine's callable and result.
    builtin_ids: &'a BTreeMap<&'static str, (u32, TypeName)>,
    values: Vec<hir::Value>,
    places: Vec<hir::Place>,
    blocks: Vec<BlockBuilder>,
    current: u32,
    parameters: Vec<u32>,
    calls: Vec<hir::CallSite>,
    scopes: Vec<BTreeMap<String, Binding>>,
    loops: Vec<Loop>,
    /// The generators being inlined, innermost last, whose `yield`s run a loop body.
    consumers: Vec<generators::Consumer>,
    /// Scopes a name cannot be found in: a generator's, from the loop body it runs.
    hidden: Vec<std::ops::Range<usize>>,
    /// How many `unsafe:` blocks enclose the statement compiled.
    unsafe_depth: u32,
    next_value: u32,
    next_place: u32,
    /// Numbers the hidden names the compiler binds.
    next_hidden: u32,
    next_instruction: u32,
    next_frame_offset: i32,
    literals: &'a mut LiteralPool,
    types: &'a mut TypeRegistry,
    constant_places: BTreeMap<u32, u32>,
    rules: &'static Rules,
    /// Each entry-zeroed array binding's descriptor offset.
    zeroed: Vec<(Span, i32)>,
    /// Places whose bindings own and drop what they hold.
    owned_places: BTreeSet<u32>,
    /// Owned aggregate parameters, by their pointer value.
    owned_references: BTreeSet<u32>,
    moves: moves::Moves,
    /// Each owning local's live flag, when its type holds a `drop`.
    drop_flags: BTreeMap<moves::Owner, u32>,
    /// What each owning value read by name came from.
    origins: BTreeMap<u32, ownership::Origin>,
    /// Owned values the current statement made and has not moved.
    temporaries: Vec<(hir::Operand, TypeName)>,
    /// Call results read in place, whose owned fields drop with the statement.
    aggregate_temporaries: Vec<StructView>,
    lambdas: Vec<lambdas::Lambda>,
    /// The owners each reference or view binding borrows, by its value.
    borrowed_from: BTreeMap<u32, BTreeSet<String>>,
    /// Views bound with `let mut`, which an assignment reseats.
    reseatable: BTreeSet<u32>,
    /// The named sequences `for` loops are walking, outermost first.
    iterated: Vec<String>,
}

impl<'a> FunctionCompiler<'a> {
    fn new(
        function: &Function,
        signature: &'a Signature,
        signatures: &'a BTreeMap<String, Signature>,
        templates: &'a RefCell<instances::Templates>,
        builtin_ids: &'a BTreeMap<&'static str, (u32, TypeName)>,
        literals: &'a mut LiteralPool,
        types: &'a mut TypeRegistry,
    ) -> Result<Self, Diagnostic> {
        let mut compiler = Self {
            signature,
            signatures,
            templates,
            builtin_ids,
            values: Vec::new(),
            places: Vec::new(),
            blocks: vec![BlockBuilder {
                id: 1,
                instructions: Vec::new(),
                terminator: None,
            }],
            current: 1,
            parameters: Vec::new(),
            calls: Vec::new(),
            scopes: vec![BTreeMap::new()],
            loops: Vec::new(),
            consumers: Vec::new(),
            hidden: Vec::new(),
            unsafe_depth: 0,
            next_value: 1,
            next_place: 1,
            next_hidden: 1,
            next_instruction: 1,
            next_frame_offset: 0,
            literals,
            types,
            constant_places: BTreeMap::new(),
            rules: &conversions::I386_REAL_MODE,
            zeroed: Vec::new(),
            owned_places: BTreeSet::new(),
            owned_references: BTreeSet::new(),
            moves: moves::Moves::default(),
            drop_flags: BTreeMap::new(),
            origins: BTreeMap::new(),
            temporaries: Vec::new(),
            aggregate_temporaries: Vec::new(),
            lambdas: Vec::new(),
            borrowed_from: BTreeMap::new(),
            reseatable: BTreeSet::new(),
            iterated: Vec::new(),
        };
        if let (Some(struct_id), Some(_)) = (signature.slot, signature.in_registers(compiler.types)) {
            let width = compiler.types.width(struct_id);
            let place = compiler.local_place(RESULT, struct_id, width, true);
            let binding = Binding { type_: BindingType::Struct(struct_id), mutable: true, storage: Storage::Place(place) };
            compiler.scopes[0].insert(RESULT.into(), binding);
        } else if let Some(pointer) = signature.slot_pointer(compiler.types) {
            let value = compiler.value_type(pointer);
            compiler.parameters.push(value);
            let (type_, storage) = match (signature.slot, signature.view) {
                (Some(struct_id), _) => (BindingType::Struct(struct_id), Storage::Reference(value)),
                (None, Some((element, rank))) => (BindingType::Slice { element, rank }, Storage::Slice(value)),
                (None, None) => unreachable!("a slot holds an aggregate or a view"),
            };
            compiler.scopes[0].insert(RESULT.into(), Binding { type_, mutable: true, storage });
        }
        for (parameter, resolved) in function.parameters.iter().zip(&signature.parameters) {
            let value = compiler.value_type(resolved.hir_type());
            compiler.parameters.push(value);
            let binding = compiler.parameter_binding(&parameter.name, resolved, value);
            compiler.scopes[0].insert(parameter.name.clone(), binding);
        }
        Ok(compiler)
    }

    /// How the callee sees parameter `name`, passed as `value`.
    fn parameter_binding(
        &mut self,
        name: &str,
        resolved: &SignatureParameter,
        value: u32,
    ) -> Binding {
        let (type_, mutable, storage) = match resolved {
            SignatureParameter::Scalar(type_name) => (
                BindingType::Scalar(*type_name),
                false,
                Storage::Parameter(value),
            ),
            SignatureParameter::Owned { struct_id, .. } => (
                BindingType::Struct(*struct_id),
                false,
                Storage::Reference(value),
            ),
            SignatureParameter::Borrowed {
                mutable, target, ..
            } => (
                *target,
                *mutable,
                if matches!(target, BindingType::Slice { .. }) {
                    Storage::Slice(value)
                } else {
                    Storage::Reference(value)
                },
            ),
        };
        // An owned string parameter is the callee's to drop: it gets a place to null on a move.
        let storage = match (type_, storage) {
            (BindingType::Scalar(type_name), Storage::Parameter(value))
                if ownership::needs_drop(type_name) =>
            {
                let place = self.place(name, type_name, false);
                self.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(place), hir::Operand::Value(value)],
                    None,
                );
                self.own(place);
                Storage::Place(place)
            }
            (BindingType::Struct(struct_id), Storage::Reference(pointer))
                if matches!(resolved, SignatureParameter::Owned { .. })
                    && self.element_needs_drop(ElementType::Struct(struct_id)) =>
            {
                self.own_aggregate(&Storage::Reference(pointer), struct_id);
                Storage::Reference(pointer)
            }
            (_, storage) => storage,
        };
        Binding {
            type_,
            mutable,
            storage,
        }
    }

    fn compile(mut self, function: &Function) -> Result<hir::Function, Diagnostic> {
        self.zero_fill(&function.body, function.span)?;
        self.statements(&function.body)?;
        if self.open() {
            self.drop_scopes(0);
            if self.signature.result == TypeName::Void && self.signature.slot.is_none() {
                self.terminate(hir::Terminator {
                    kind: "return",
                    operands: Vec::new(),
                    targets: Vec::new(),
                });
            } else {
                return Err(Diagnostic::new(
                    function.span,
                    format!(
                        "function {:?} can reach its end without returning",
                        function.name
                    ),
                ));
            }
        }
        self.prune_unreachable();
        let blocks = self
            .blocks
            .into_iter()
            .map(|block| hir::Block {
                id: block.id,
                instructions: block.instructions,
                terminator: block
                    .terminator
                    .expect("every semantic block is terminated"),
            })
            .collect();
        Ok(hir::Function {
            id: self.signature.id,
            name: self.signature.name.clone(),
            result_type: type_id(self.signature.returned(self.types)),
            values: self.values,
            places: self.places,
            blocks,
            entry: 1,
            parameters: self.parameters,
            calls: self.calls,
            exported: self.signature.exported,
            cleans: self.signature.abi.callee_cleans().then(|| self.signature.argument_bytes(&self.types)),
        })
    }

    /// Lays the zero-filled arrays a call binds at most once side by side, descriptors
    /// included, and zeroes them with one fill on entry. Each binding then stores only its
    /// descriptor.
    fn zero_fill(&mut self, body: &[Statement], span: Span) -> Result<(), Diagnostic> {
        let mut extent = 0;
        for (bind, element, dims, value) in repeated_bindings(body) {
            let Ok(ElementType::Scalar(type_name)) = self.types.resolve_element(element, bind)
            else {
                continue;
            };
            if !is_zero(&value, type_name) {
                continue;
            }
            let shape = Shape::new(dims);
            self.zeroed.push((bind, extent as i32));
            extent += width(type_name) * shape.len() + descriptor::size(shape.rank);
        }
        if extent == 0 {
            return Ok(());
        }
        let shape = Shape::new(&[extent.div_ceil(2)]);
        let element = ElementType::Scalar(TypeName::U16);
        let type_id = self.types.array(element, shape);
        let region = self.local_place("$zero", type_id, 2 * shape.len(), true);
        let base = self.next_frame_offset;
        for (_, offset) in &mut self.zeroed {
            *offset += base;
        }
        self.statement(&Statement::Bind {
            mutable: false,
            name: "$$zero_fill".into(),
            annotation: Some(TypeAnnotation::Value(TypeSpec::Primitive(TypeName::U16))),
            value: Expr::Integer(0, span),
            span,
        })?;
        self.scopes.last_mut().expect("scope").insert(
            "$zero".into(),
            Binding {
                type_: BindingType::Array { element, shape },
                mutable: true,
                storage: Storage::Place(region),
            },
        );
        self.fill("$zero", shape, span)
    }

    fn prune_unreachable(&mut self) {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![1_u32];
        while let Some(id) = pending.pop() {
            if !reachable.insert(id) {
                continue;
            }
            let block = &self.blocks[(id - 1) as usize];
            if let Some(terminator) = &block.terminator {
                pending.extend(terminator.targets.iter().copied());
            }
        }

        self.blocks.retain(|block| reachable.contains(&block.id));
        let instructions = self
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter().map(|instruction| instruction.id))
            .collect::<BTreeSet<_>>();
        self.calls
            .retain(|call| instructions.contains(&call.instruction));

        let mut defined = self.parameters.iter().copied().collect::<BTreeSet<_>>();
        defined.extend(
            self.blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .flat_map(|instruction| instruction.results.iter().copied()),
        );
        self.values.retain(|value| defined.contains(&value.id));
    }
}

fn required(value: TypedOperand, span: Span) -> Result<hir::Operand, Diagnostic> {
    value
        .operand
        .ok_or_else(|| Diagnostic::new(span, "void expression has no value"))
}

fn jump(target: u32) -> hir::Terminator {
    hir::Terminator {
        kind: "jump",
        operands: Vec::new(),
        targets: vec![target],
    }
}

fn scaled_decimal(spelling: &str, fraction: u8, span: Span) -> Result<i128, Diagnostic> {
    let (mantissa, exponent) = spelling
        .find(['e', 'E'])
        .map(|at| (&spelling[..at], &spelling[at + 1..]))
        .unwrap_or((spelling, "0"));
    let exponent = exponent
        .parse::<i32>()
        .map_err(|_| Diagnostic::new(span, "invalid fixed-point literal exponent"))?;
    let (whole, fractional) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |(whole, fractional)| (whole, fractional));
    let digits = format!("{whole}{fractional}");
    let significand = digits
        .parse::<i128>()
        .map_err(|_| Diagnostic::new(span, "fixed-point literal is too large"))?;
    let fractional_digits = i32::try_from(fractional.len())
        .map_err(|_| Diagnostic::new(span, "fixed-point literal is too long"))?;
    let decimal_power = exponent
        .checked_sub(fractional_digits)
        .ok_or_else(|| Diagnostic::new(span, "fixed-point literal exponent is too large"))?;
    let binary_scale = 1_i128
        .checked_shl(u32::from(fraction))
        .ok_or_else(|| Diagnostic::new(span, "fixed-point fraction is too large"))?;
    let mut numerator = significand
        .checked_mul(binary_scale)
        .ok_or_else(|| Diagnostic::new(span, "fixed-point literal is too large"))?;
    let denominator = if decimal_power >= 0 {
        let decimal_scale = checked_power_of_ten(decimal_power as u32, span)?;
        numerator = numerator
            .checked_mul(decimal_scale)
            .ok_or_else(|| Diagnostic::new(span, "fixed-point literal is too large"))?;
        1
    } else {
        checked_power_of_ten(decimal_power.unsigned_abs(), span)?
    };
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    Ok(if remainder != 0 && remainder >= denominator / 2 {
        quotient + 1
    } else {
        quotient
    })
}

fn checked_power_of_ten(power: u32, span: Span) -> Result<i128, Diagnostic> {
    (0..power).try_fold(1_i128, |value, _| {
        value
            .checked_mul(10)
            .ok_or_else(|| Diagnostic::new(span, "fixed-point literal exponent is too large"))
    })
}

fn fixed_storage_value(value: i128, type_name: TypeName, span: Span) -> Result<i64, Diagnostic> {
    let TypeName::Fixed { storage, .. } = type_name else {
        unreachable!("fixed storage check requires a fixed type")
    };
    let value = match storage {
        FixedStorage::I16 => i16::try_from(value).map(i64::from),
        FixedStorage::I32 => i32::try_from(value).map(i64::from),
    }
    .map_err(|_| {
        Diagnostic::new(
            span,
            format!("literal does not fit {}", type_name_text(type_name)),
        )
    })?;
    Ok(value)
}

pub(crate) fn is_integer(type_name: TypeName) -> bool {
    matches!(
        type_name,
        TypeName::I8 | TypeName::U8 | TypeName::I16 | TypeName::U16 | TypeName::I32 | TypeName::U32
    )
}

pub(crate) fn is_signed(type_name: TypeName) -> bool {
    matches!(
        type_name,
        TypeName::I8 | TypeName::I16 | TypeName::I32 | TypeName::I64 | TypeName::Fixed { .. }
    )
}

fn is_unsigned(type_name: TypeName) -> bool {
    matches!(
        type_name,
        TypeName::Char | TypeName::U8 | TypeName::U16 | TypeName::U32
    )
}

pub(crate) fn is_float(type_name: TypeName) -> bool {
    matches!(type_name, TypeName::F32 | TypeName::F64)
}

fn is_numeric(type_name: TypeName) -> bool {
    is_integer(type_name) || is_float(type_name) || is_fixed(type_name)
}

fn is_ordered(type_name: TypeName) -> bool {
    is_numeric(type_name) || type_name == TypeName::Char
}

/// A repeat literal's counts, which are compile-time integers.
fn repeat_counts(counts: &[Expr]) -> Result<Vec<u32>, Diagnostic> {
    counts
        .iter()
        .map(|count| match super::consts::integer(count) {
            Some(value) => u32::try_from(value)
                .map_err(|_| Diagnostic::new(count.span(), "a repeat count must be non-negative")),
            None => Err(Diagnostic::new(
                count.span(),
                "a repeat count must be a compile-time integer",
            )),
        })
        .collect()
}

/// A nested array literal's elements with their indices, checked against `dims`.
fn literal_elements<'e>(
    literal: &'e Expr,
    dims: &[u32],
    span: Span,
) -> Result<Vec<(Vec<u32>, &'e Expr)>, Diagnostic> {
    let Some((&length, inner)) = dims.split_first() else {
        return Ok(vec![(Vec::new(), literal)]);
    };
    let Expr::Array(items, at) = literal else {
        return Err(Diagnostic::new(
            literal.span(),
            format!("expected a nested array literal of {length} elements"),
        ));
    };
    if items.len() != length as usize {
        return Err(Diagnostic::new(
            if inner.len() + 1 == dims.len() {
                span
            } else {
                *at
            },
            format!("array expects {length} elements, got {}", items.len()),
        ));
    }
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        for (mut at, element) in literal_elements(item, inner, span)? {
            at.insert(0, index as u32);
            out.push((at, element));
        }
    }
    Ok(out)
}

/// The fixed-array bindings of a repeated literal that run at most once per call, with
/// their element, dimensions and repeated value.
fn repeated_bindings(statements: &[Statement]) -> Vec<(Span, &TypeSpec, &[u32], Expr)> {
    let mut found = Vec::new();
    for statement in statements {
        match statement {
            Statement::Bind {
                annotation: Some(TypeAnnotation::Array { element, dims }),
                value,
                span,
                ..
            } => {
                let repeated = repeated_literal(value, dims);
                if let Expr::Repeat { value, counts, .. } = repeated.as_ref().unwrap_or(value) {
                    if repeat_counts(counts).is_ok_and(|counts| counts == *dims) {
                        found.push((*span, element, dims.as_slice(), value.as_ref().clone()));
                    }
                }
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                found.extend(repeated_bindings(then_branch));
                found.extend(repeated_bindings(else_branch));
            }
            _ => {}
        }
    }
    found
}

/// Whether `literal`, as a `type_name`, is all zero bytes.
fn is_zero(literal: &Expr, type_name: TypeName) -> bool {
    use TypeName::*;
    match (literal, type_name) {
        (Expr::Integer(0, _), I8 | U8 | I16 | U16 | I32 | U32 | I64) => true,
        (Expr::Float(text, _), F32 | F64) => text
            .parse::<f64>()
            .is_ok_and(|value| value == 0.0 && value.is_sign_positive()),
        (Expr::Character(0, _), Char) => true,
        (Expr::Boolean(false, _), Bool) => true,
        _ => false,
    }
}

/// `[v, v, ...]` of one scalar literal as the `[v; dims]` it means, so it fills rather than storing each element.
fn repeated_literal(literal: &Expr, dims: &[u32]) -> Option<Expr> {
    let items = literal_elements(literal, dims, literal.span()).ok()?;
    let (_, first) = items.first()?;
    let key = scalar_literal(first)?;
    if items.len() < 2
        || items
            .iter()
            .any(|(_, item)| scalar_literal(item) != Some(key.clone()))
    {
        return None;
    }
    let span = literal.span();
    Some(Expr::Repeat {
        value: Box::new((*first).clone()),
        counts: dims
            .iter()
            .map(|count| Expr::Integer(i64::from(*count), span))
            .collect(),
        span,
    })
}

/// A number, character or boolean literal's value, whatever its spelling's position.
fn scalar_literal(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Integer(value, _) => Some(format!("i{value}")),
        Expr::Float(text, _) => Some(format!("f{text}")),
        Expr::Character(value, _) => Some(format!("c{value}")),
        Expr::Boolean(value, _) => Some(format!("b{value}")),
        Expr::Unary {
            op: UnaryOp::Negative,
            operand,
            ..
        } => scalar_literal(operand)
            .filter(|inner| !inner.starts_with(['c', 'b']))
            .map(|inner| format!("-{inner}")),
        _ => None,
    }
}

/// An expression whose type comes from its context.
fn is_literal(expression: &Expr) -> bool {
    is_integer_literal(expression)
        || is_float_literal(expression)
        || matches!(
            expression,
            Expr::Character(..) | Expr::Boolean(..) | Expr::String(..)
        )
}

fn storage_type(storage: FixedStorage) -> TypeName {
    match storage {
        FixedStorage::I16 => TypeName::I16,
        FixedStorage::I32 => TypeName::I32,
    }
}

fn is_integer_literal(expression: &Expr) -> bool {
    match expression {
        Expr::Integer(..) => true,
        Expr::Unary {
            op: UnaryOp::Negative,
            operand,
            ..
        } => matches!(operand.as_ref(), Expr::Integer(..)),
        _ => false,
    }
}

fn is_float_literal(expression: &Expr) -> bool {
    match expression {
        Expr::Float(..) => true,
        Expr::Unary {
            op: UnaryOp::Negative,
            operand,
            ..
        } => matches!(operand.as_ref(), Expr::Float(..)),
        _ => false,
    }
}

/// `value`'s low bits as `target` reads them.
fn wrapped(value: i64, target: TypeName) -> i64 {
    let bits = 8 * width(target);
    let low = value & ((1_i64 << bits) - 1);
    if is_signed(target) && low >> (bits - 1) != 0 {
        low - (1_i64 << bits)
    } else {
        low
    }
}

fn is_comparison(operation: BinaryOp) -> bool {
    matches!(
        operation,
        BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
    )
}

fn is_shift(operation: BinaryOp) -> bool {
    matches!(operation, BinaryOp::ShiftLeft | BinaryOp::ShiftRight)
}

fn is_bitwise(operation: BinaryOp) -> bool {
    is_shift(operation)
        || matches!(
            operation,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
        )
}

/// What the left operand of a numeric binary operator may be.
fn operand_rule(operation: BinaryOp, type_name: TypeName, span: Span) -> Result<(), Diagnostic> {
    if is_comparison(operation) {
        let equality = matches!(operation, BinaryOp::Equal | BinaryOp::NotEqual);
        if (!equality && !is_ordered(type_name)) || type_name == TypeName::Void {
            return Err(Diagnostic::new(
                span,
                if equality {
                    "equality requires scalar operands"
                } else {
                    "ordering requires numeric or char operands"
                },
            ));
        }
    } else if is_bitwise(operation) {
        if !is_integer(type_name) {
            return Err(Diagnostic::new(
                span,
                "bitwise operators require integer operands",
            ));
        }
    } else if !is_numeric(type_name) {
        return Err(Diagnostic::new(
            span,
            "arithmetic requires numeric operands",
        ));
    }
    Ok(())
}

fn is_fixed(type_name: TypeName) -> bool {
    matches!(type_name, TypeName::Fixed { .. })
}

/// The integer type a primitive HIR type id names.
fn type_name_of(id: u32) -> TypeName {
    match id {
        I8 => TypeName::I8,
        U8 => TypeName::U8,
        I16 => TypeName::I16,
        U16 => TypeName::U16,
        I32 => TypeName::I32,
        U32 => TypeName::U32,
        I64 => TypeName::I64,
        _ => unreachable!("an index is a primitive integer"),
    }
}

fn type_id(type_name: TypeName) -> u32 {
    match type_name {
        TypeName::Void => VOID,
        TypeName::Bool => BOOL,
        TypeName::Char => CHAR,
        TypeName::I8 => I8,
        TypeName::U8 => U8,
        TypeName::I16 => I16,
        TypeName::U16 => U16,
        TypeName::I32 => I32,
        TypeName::U32 => U32,
        TypeName::F32 => F32,
        TypeName::F64 => F64,
        TypeName::String => STRING,
        TypeName::Addr => ADDR,
        TypeName::I64 => I64,
        TypeName::Fixed { declaration, .. } => FIXED_START + u32::from(declaration),
        TypeName::Enum { type_id, .. }
        | TypeName::Vector { type_id }
        | TypeName::Dictionary { type_id }
        | TypeName::Function { type_id }
        | TypeName::Bits { type_id, .. }
        | TypeName::Pointer { type_id, .. } => type_id,
    }
}

pub(crate) fn width(type_name: TypeName) -> u32 {
    match type_name {
        TypeName::Void => 0,
        TypeName::Bool | TypeName::Char | TypeName::I8 | TypeName::U8 => 1,
        TypeName::I16 | TypeName::U16 => 2,
        TypeName::I32 | TypeName::U32 | TypeName::F32 => 4,
        TypeName::F64 => 8,
        TypeName::String | TypeName::Vector { .. } | TypeName::Dictionary { .. } | TypeName::Function { .. } => 2,
        TypeName::Addr => 4,
        TypeName::I64 => 8,
        TypeName::Fixed { storage, .. } => match storage {
            FixedStorage::I16 => 2,
            FixedStorage::I32 => 4,
        },
        TypeName::Enum { width, .. }
        | TypeName::Bits { width, .. }
        | TypeName::Pointer { width, .. } => u32::from(width),
    }
}

fn type_name_text(type_name: TypeName) -> String {
    match type_name {
        TypeName::Void => "void".into(),
        TypeName::Bool => "bool".into(),
        TypeName::Char => "char".into(),
        TypeName::I8 => "i8".into(),
        TypeName::U8 => "u8".into(),
        TypeName::I16 => "i16".into(),
        TypeName::U16 => "u16".into(),
        TypeName::I32 => "i32".into(),
        TypeName::U32 => "u32".into(),
        TypeName::F32 => "f32".into(),
        TypeName::F64 => "f64".into(),
        TypeName::String => "string".into(),
        TypeName::Addr => "addr".into(),
        TypeName::I64 => "$i64".into(),
        TypeName::Fixed {
            storage, fraction, ..
        } => format!(
            "fixed {}, fraction={fraction}",
            match storage {
                FixedStorage::I16 => "i16",
                FixedStorage::I32 => "i32",
            }
        ),
        TypeName::Enum { .. } => "enum".into(),
        TypeName::Vector { .. } => "vec".into(),
        TypeName::Dictionary { .. } => "dict".into(),
        TypeName::Function { .. } => "function".into(),
        TypeName::Bits { .. } => "bits struct".into(),
        TypeName::Pointer { width: 4, .. } => "*far pointer".into(),
        TypeName::Pointer { .. } => "*near pointer".into(),
    }
}

fn type_mismatch(span: Span, expected: TypeName, found: TypeName) -> Diagnostic {
    if matches!(expected, TypeName::Fixed { .. })
        && matches!(found, TypeName::Fixed { .. })
        && expected != found
    {
        return Diagnostic::new(span, "distinct fixed-point types do not match");
    }
    Diagnostic::new(
        span,
        format!(
            "expected {}, found {}",
            type_name_text(expected),
            type_name_text(found)
        ),
    )
}

#[cfg(test)]
mod tests;
