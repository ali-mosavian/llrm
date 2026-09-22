use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::error::Diagnostic;
use crate::hir;
use crate::syntax::AssignTarget;
use crate::syntax::BinaryOp;
use crate::syntax::Expr;
use crate::syntax::FStringPart;
use crate::syntax::FixedStorage;
use crate::syntax::FixedType;
use crate::syntax::Function;
use crate::syntax::IterationMode;
use crate::syntax::Module;
use crate::syntax::ParameterType;
use crate::syntax::Span;
use crate::syntax::Statement;
use crate::syntax::Struct;
use crate::syntax::StructLiteralFields;
use crate::syntax::TypeAnnotation;
use crate::syntax::TypeName;
use crate::syntax::TypeSpec;
use crate::syntax::UnaryOp;

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
        let mut bytes = Vec::with_capacity(value.len() + 5);
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
    arrays: BTreeMap<(u32, u32), u32>,
    structs: BTreeMap<String, StructLayout>,
    fixed_names: BTreeMap<String, TypeName>,
    pointers: BTreeMap<(u32, u32), u32>,
    slice_descriptors: BTreeMap<u32, u32>,
}

#[derive(Clone, Debug)]
struct StructLayout {
    id: u32,
    name: String,
    fields: BTreeMap<String, FieldLayout>,
    field_order: Vec<String>,
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
                plain_type(F32, "f32", "float", 4, None, "binary32"),
                plain_type(F64, "f64", "float", 8, None, "binary64"),
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
            fixed_names: BTreeMap::new(),
            pointers: BTreeMap::new(),
            slice_descriptors: BTreeMap::new(),
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

    fn register_structs(&mut self, declarations: &[Struct]) -> Result<(), Diagnostic> {
        for declaration in declarations {
            if self.structs.contains_key(&declaration.name)
                || self.fixed_names.contains_key(&declaration.name)
            {
                return Err(Diagnostic::new(
                    declaration.span,
                    format!("struct {:?} is declared more than once", declaration.name),
                ));
            }
            let mut fields = BTreeMap::new();
            let mut field_order = Vec::new();
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
                let field_alignment = field_width.clamp(1, 2);
                offset = align_up(offset, field_alignment);
                fields.insert(
                    field.name.clone(),
                    FieldLayout {
                        type_: field_type,
                        offset,
                    },
                );
                field_order.push(field.name.clone());
                offset += field_width;
                alignment = alignment.max(field_alignment);
            }
            let width = align_up(offset, alignment);
            let id = self.types.len() as u32 + 1;
            self.types.push(hir::Type {
                id,
                name: declaration.name.clone(),
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
                declaration.name.clone(),
                StructLayout {
                    id,
                    name: declaration.name.clone(),
                    fields,
                    field_order,
                },
            );
        }
        Ok(())
    }

    fn resolve_element(&self, spec: &TypeSpec, span: Span) -> Result<ElementType, Diagnostic> {
        match spec {
            TypeSpec::Primitive(type_name) => Ok(ElementType::Scalar(*type_name)),
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
                .ok_or_else(|| Diagnostic::new(span, format!("unknown type {name:?}"))),
        }
    }

    fn array(&mut self, element: ElementType, length: u32) -> u32 {
        let element_id = element.id();
        if let Some(id) = self.arrays.get(&(element_id, length)) {
            return *id;
        }
        let element_type = &self.types[(element_id - 1) as usize];
        let element_name = element_type.name.clone();
        let element_width = element_type.width;
        let id = self.types.len() as u32 + 1;
        self.types.push(hir::Type {
            id,
            name: format!("[{element_name}; {length}]"),
            kind: "array",
            width: element_width * length,
            signed: None,
            evaluation: "none",
            element: Some(element_id),
            rank: 1,
            bounds: vec![(0, i32::try_from(length - 1).expect("array length checked"))],
            address: "near",
        });
        self.arrays.insert((element_id, length), id);
        id
    }

    fn width(&self, id: u32) -> u32 {
        self.types[(id - 1) as usize].width
    }

    fn pointer(&mut self, target: u32, rank: u32) -> u32 {
        if let Some(id) = self.pointers.get(&(target, rank)) {
            return *id;
        }
        let id = self.types.len() as u32 + 1;
        let name = format!("&{}", self.types[(target - 1) as usize].name);
        self.types.push(hir::Type {
            id,
            name,
            kind: "pointer",
            width: 4,
            signed: None,
            evaluation: "none",
            element: Some(target),
            rank,
            bounds: Vec::new(),
            address: "far",
        });
        self.pointers.insert((target, rank), id);
        id
    }

    fn slice_descriptor(&mut self, element: ElementType) -> u32 {
        let element_id = element.id();
        if let Some(id) = self.slice_descriptors.get(&element_id) {
            return *id;
        }
        let id = self.types.len() as u32 + 1;
        let element_name = self.types[(element_id - 1) as usize].name.clone();
        self.types.push(hir::Type {
            id,
            name: format!("$slice[{element_name}]"),
            kind: "opaque",
            width: 8,
            signed: None,
            evaluation: "none",
            element: Some(element_id),
            rank: 0,
            bounds: Vec::new(),
            address: "none",
        });
        self.slice_descriptors.insert(element_id, id);
        id
    }

    fn slice_pointer(&mut self, element: ElementType) -> u32 {
        let descriptor = self.slice_descriptor(element);
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
            TypeAnnotation::Slice { element } => {
                let element = self.resolve_element(element, span)?;
                Ok((BindingType::Slice { element }, element.id()))
            }
            TypeAnnotation::Array { element, length } => {
                let element = self.resolve_element(element, span)?;
                let id = self.array(element, *length);
                Ok((
                    BindingType::Array {
                        element,
                        length: *length,
                    },
                    id,
                ))
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

#[derive(Clone, Debug)]
struct Signature {
    id: u32,
    name: String,
    parameters: Vec<SignatureParameter>,
    result: TypeName,
}

#[derive(Clone, Copy, Debug)]
enum SignatureParameter {
    Scalar(TypeName),
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
            Self::Borrowed { pointer, .. } => pointer,
        }
    }
}

#[derive(Clone, Debug)]
enum Storage {
    Parameter(u32),
    Reference(u32),
    Slice(u32),
    Place(u32),
    ArrayView { place: u32, index: hir::Operand },
    Dictionary { keys: u32, values: u32, length: u32 },
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingType {
    Scalar(TypeName),
    Slice {
        element: ElementType,
    },
    Array {
        element: ElementType,
        length: u32,
    },
    Struct(u32),
    Dictionary {
        key: TypeName,
        value: TypeName,
        capacity: u32,
    },
}

impl BindingType {
    fn array(self) -> Option<(ElementType, Option<u32>)> {
        match self {
            Self::Slice { element } => Some((element, None)),
            Self::Array { element, length } => Some((element, Some(length))),
            Self::Scalar(_) | Self::Struct(_) | Self::Dictionary { .. } => None,
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

pub fn compile(module: &Module, module_name: &str) -> Result<String, Diagnostic> {
    let mut types = TypeRegistry::new();
    types.register_fixed_types(&module.fixed_types)?;
    types.register_structs(&module.structs)?;
    let mut signatures = BTreeMap::new();
    for (index, function) in module.functions.iter().enumerate() {
        if signatures.contains_key(&function.name) {
            return Err(Diagnostic::new(
                function.span,
                format!("function {:?} is declared more than once", function.name),
            ));
        }
        let mut parameters = BTreeMap::new();
        for parameter in &function.parameters {
            if parameters.insert(&parameter.name, parameter.span).is_some() {
                return Err(Diagnostic::new(
                    parameter.span,
                    format!("parameter {:?} is declared more than once", parameter.name),
                ));
            }
        }
        let resolved_parameters = function
            .parameters
            .iter()
            .map(|parameter| match &parameter.type_ {
                ParameterType::Scalar(type_name) => Ok(SignatureParameter::Scalar(*type_name)),
                ParameterType::Borrowed { mutable, target } => {
                    let (target, target_id) = types.parameter_target(target, parameter.span)?;
                    let pointer = match target {
                        BindingType::Slice { element } => types.slice_pointer(element),
                        _ => types.pointer(target_id, 0),
                    };
                    Ok(SignatureParameter::Borrowed {
                        mutable: *mutable,
                        target,
                        pointer,
                    })
                }
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        signatures.insert(
            function.name.clone(),
            Signature {
                id: index as u32 + 1,
                name: function.name.clone(),
                parameters: resolved_parameters,
                result: function.result,
            },
        );
    }

    let mut callables: Vec<_> = signatures
        .values()
        .map(|signature| hir::Callable {
            id: signature.id,
            name: signature.name.clone(),
            result_type: (signature.result != TypeName::Void).then(|| type_id(signature.result)),
            parameter_types: signature
                .parameters
                .iter()
                .map(|one| one.hir_type())
                .collect(),
            defined: true,
        })
        .collect();
    let mut builtin_ids = BTreeMap::new();
    for (name, parameter) in print_builtins() {
        let id = callables.len() as u32 + 1;
        builtin_ids.insert(name, id);
        callables.push(hir::Callable {
            id,
            name: name.into(),
            result_type: None,
            parameter_types: parameter.into_iter().map(type_id).collect(),
            defined: false,
        });
    }
    let mut functions = Vec::new();
    let mut literals = LiteralPool::default();
    for function in &module.functions {
        let signature = signatures.get(&function.name).expect("collected function");
        functions.push(
            FunctionCompiler::new(
                function,
                signature,
                &signatures,
                &builtin_ids,
                &mut literals,
                &mut types,
            )?
            .compile(function)?,
        );
    }
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
    out
}

fn print_name(type_name: TypeName) -> &'static str {
    match type_name {
        TypeName::String => "_pt",
        TypeName::Addr => unreachable!("addresses have no default formatter"),
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
    builtin_ids: &'a BTreeMap<&'static str, u32>,
    values: Vec<hir::Value>,
    places: Vec<hir::Place>,
    blocks: Vec<BlockBuilder>,
    current: u32,
    parameters: Vec<u32>,
    calls: Vec<hir::CallSite>,
    scopes: Vec<BTreeMap<String, Binding>>,
    loops: Vec<(u32, u32)>,
    next_value: u32,
    next_place: u32,
    next_instruction: u32,
    next_frame_offset: i32,
    literals: &'a mut LiteralPool,
    types: &'a mut TypeRegistry,
    constant_places: BTreeMap<u32, u32>,
}

impl<'a> FunctionCompiler<'a> {
    fn new(
        function: &Function,
        signature: &'a Signature,
        signatures: &'a BTreeMap<String, Signature>,
        builtin_ids: &'a BTreeMap<&'static str, u32>,
        literals: &'a mut LiteralPool,
        types: &'a mut TypeRegistry,
    ) -> Result<Self, Diagnostic> {
        let mut compiler = Self {
            signature,
            signatures,
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
            next_value: 1,
            next_place: 1,
            next_instruction: 1,
            next_frame_offset: 0,
            literals,
            types,
            constant_places: BTreeMap::new(),
        };
        for (parameter, resolved) in function.parameters.iter().zip(&signature.parameters) {
            let value = compiler.value_type(resolved.hir_type());
            compiler.parameters.push(value);
            let (type_, mutable, storage) = match resolved {
                SignatureParameter::Scalar(type_name) => (
                    BindingType::Scalar(*type_name),
                    false,
                    Storage::Parameter(value),
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
            compiler.scopes[0].insert(
                parameter.name.clone(),
                Binding {
                    type_,
                    mutable,
                    storage,
                },
            );
        }
        Ok(compiler)
    }

    fn compile(mut self, function: &Function) -> Result<hir::Function, Diagnostic> {
        self.statements(&function.body)?;
        if self.open() {
            if function.result == TypeName::Void {
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
            name: function.name.clone(),
            result_type: type_id(function.result),
            values: self.values,
            places: self.places,
            blocks,
            entry: 1,
            parameters: self.parameters,
            calls: self.calls,
        })
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

    fn statements(&mut self, statements: &[Statement]) -> Result<(), Diagnostic> {
        for statement in statements {
            if !self.open() {
                return Err(Diagnostic::new(
                    statement.span(),
                    "statement is unreachable",
                ));
            }
            self.statement(statement)?;
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        match statement {
            Statement::Bind {
                mutable,
                name,
                annotation,
                value,
                span,
            } => {
                if self.scopes.last().expect("scope").contains_key(name) {
                    return Err(Diagnostic::new(
                        *span,
                        format!("binding {name:?} is already declared in this scope"),
                    ));
                }
                if let Expr::Comprehension {
                    element,
                    binding,
                    mode,
                    iterable,
                    span: comprehension_span,
                } = value
                {
                    return self.comprehension_binding(
                        *mutable,
                        name,
                        annotation.as_ref(),
                        element,
                        binding,
                        *mode,
                        iterable,
                        *comprehension_span,
                    );
                }
                if let Expr::DictComprehension {
                    key,
                    value: entry_value,
                    binding,
                    mode,
                    iterable,
                    span: comprehension_span,
                } = value
                {
                    if annotation.is_some() {
                        return Err(Diagnostic::new(
                            *span,
                            "dictionary comprehension types are inferred from key and value",
                        ));
                    }
                    return self.dictionary_binding(
                        *mutable,
                        name,
                        key,
                        entry_value,
                        binding,
                        *mode,
                        iterable,
                        *comprehension_span,
                    );
                }
                if let Some(TypeAnnotation::Array { element, length }) = annotation {
                    let count = match value {
                        Expr::Array(items, _) => items.len(),
                        Expr::Repeat { counts, .. } => repeat_count(counts)?,
                        _ => {
                            return Err(Diagnostic::new(
                                *span,
                                "fixed-array binding requires an array literal",
                            ))
                        }
                    };
                    if count != *length as usize {
                        return Err(Diagnostic::new(
                            *span,
                            format!("array expects {length} elements, got {count}"),
                        ));
                    }
                    if let Expr::Repeat { value, .. } = value {
                        // Evaluated before the new name exists, which it may shadow.
                        self.statement(&Statement::Bind {
                            mutable: false,
                            name: format!("${name}_fill"),
                            annotation: Some(TypeAnnotation::Value(element.clone())),
                            value: value.as_ref().clone(),
                            span: *span,
                        })?;
                    }
                    let element = self.types.resolve_element(element, *span)?;
                    let type_id = self.types.array(element, *length);
                    let place = self.array_place(name, type_id, element, *length, *mutable);
                    let binding = |mutable| Binding {
                        type_: BindingType::Array {
                            element,
                            length: *length,
                        },
                        mutable,
                        storage: Storage::Place(place),
                    };
                    if matches!(value, Expr::Repeat { .. }) {
                        // Filling stores through the name, which a `let` would refuse.
                        self.scopes
                            .last_mut()
                            .expect("scope")
                            .insert(name.clone(), binding(true));
                        self.fill(name, *length, *span)?;
                    }
                    let items = match value {
                        Expr::Array(items, _) => items.as_slice(),
                        _ => &[],
                    };
                    for (index, item) in items.iter().enumerate() {
                        let index = hir::Operand::Constant(U16, index as i64);
                        match element {
                            ElementType::Scalar(type_name) => {
                                let value = self.expression(item, Some(type_name))?;
                                self.emit(
                                    "store",
                                    Vec::new(),
                                    vec![
                                        hir::Operand::ArrayElement(place, vec![index]),
                                        required(value, item.span())?,
                                    ],
                                    None,
                                );
                            }
                            ElementType::Struct(struct_id) => {
                                self.initialize_struct(place, index, struct_id, item)?;
                            }
                        }
                    }
                    self.scopes
                        .last_mut()
                        .expect("scope")
                        .insert(name.clone(), binding(*mutable));
                    return Ok(());
                }
                if matches!(value, Expr::Array(..) | Expr::Repeat { .. }) {
                    return Err(Diagnostic::new(
                        *span,
                        "array literal requires a fixed-array annotation",
                    ));
                }
                let annotated = match annotation {
                    Some(TypeAnnotation::Value(spec)) => {
                        Some(self.types.resolve_element(spec, *span)?)
                    }
                    Some(TypeAnnotation::Slice { .. }) => {
                        return Err(Diagnostic::new(
                            *span,
                            "an owned array needs a fixed length: '[T; N]'",
                        ));
                    }
                    Some(TypeAnnotation::Array { .. }) => unreachable!(),
                    None => None,
                };
                let struct_id = match annotated {
                    Some(ElementType::Struct(struct_id)) => Some(struct_id),
                    Some(ElementType::Scalar(_)) => None,
                    None => self.struct_expression_type(value, *span)?,
                };
                if let Some(struct_id) = struct_id {
                    let type_id = self
                        .types
                        .structure(struct_id)
                        .expect("resolved struct type")
                        .id;
                    let extent = self.types.width(type_id);
                    let place = self.local_place(name, type_id, extent, *mutable);
                    let destination = StructView {
                        struct_id,
                        place,
                        pointer: None,
                        indices: Vec::new(),
                        offset: 0,
                        mutable: *mutable,
                        owner: name.clone(),
                    };
                    self.store_struct_expression(&destination, value)?;
                    self.scopes.last_mut().expect("scope").insert(
                        name.clone(),
                        Binding {
                            type_: BindingType::Struct(struct_id),
                            mutable: *mutable,
                            storage: Storage::Place(place),
                        },
                    );
                    return Ok(());
                }
                let expected = match annotated {
                    Some(ElementType::Scalar(type_name)) => Some(type_name),
                    Some(ElementType::Struct(_)) => unreachable!(),
                    None => None,
                };
                let value = self.expression(value, expected)?;
                if value.type_name == TypeName::Void {
                    return Err(Diagnostic::new(*span, "cannot bind a void expression"));
                }
                let binding_type = value.type_name;
                let place = self.place(name, binding_type, *mutable);
                self.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(place), required(value, *span)?],
                    None,
                );
                self.scopes.last_mut().expect("scope").insert(
                    name.clone(),
                    Binding {
                        type_: BindingType::Scalar(binding_type),
                        mutable: *mutable,
                        storage: Storage::Place(place),
                    },
                );
            }
            Statement::Assign {
                target,
                operation,
                value,
                span,
            } => match self.assignment_target(target, *span)? {
                AssignmentPlace::Scalar(destination, element) => {
                    let value = if let Some(operation) = operation {
                        let current = self.value(element);
                        self.emit("load", vec![current], vec![destination.clone()], None);
                        let right = self.right_operand(*operation, value, element)?;
                        self.binary_operands(
                            *operation,
                            TypedOperand {
                                operand: Some(hir::Operand::Value(current)),
                                type_name: element,
                            },
                            right,
                            Some(element),
                            *span,
                        )?
                    } else {
                        self.expression(value, Some(element))?
                    };
                    self.emit(
                        "store",
                        Vec::new(),
                        vec![destination, required(value, *span)?],
                        None,
                    );
                }
                AssignmentPlace::Struct(destination) => {
                    if operation.is_some() {
                        return Err(Diagnostic::new(
                            *span,
                            "compound assignment requires a numeric scalar",
                        ));
                    }
                    self.store_struct_expression(&destination, value)?;
                }
            },
            Statement::Expr(expression) => {
                if !matches!(expression, Expr::Call { .. } | Expr::MethodCall { .. }) {
                    return Err(Diagnostic::new(
                        expression.span(),
                        "only a function call may be used as an expression statement",
                    ));
                }
                self.expression(expression, None)?;
            }
            Statement::Return { value, span } => {
                let operands = match (self.signature.result, value) {
                    (TypeName::Void, None) => Vec::new(),
                    (TypeName::Void, Some(_)) => {
                        return Err(Diagnostic::new(
                            *span,
                            "void function cannot return a value",
                        ))
                    }
                    (_, None) => return Err(Diagnostic::new(*span, "return value is required")),
                    (result, Some(expression)) => {
                        let value = self.expression(expression, Some(result))?;
                        vec![required(value, *span)?]
                    }
                };
                self.terminate(hir::Terminator {
                    kind: "return",
                    operands,
                    targets: Vec::new(),
                });
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => self.if_statement(condition, then_branch, else_branch, *span)?,
            Statement::While {
                condition,
                body,
                span,
            } => self.while_statement(condition, body, *span)?,
            Statement::For {
                mode,
                name,
                iterable,
                body,
                span,
            } => self.for_statement(*mode, name, iterable, body, *span)?,
            Statement::ForRange {
                name,
                start,
                end,
                body,
                span,
            } => self.range_statement(name, start, end, body, *span)?,
            Statement::Break(span) => {
                let Some((target, _)) = self.loops.last().copied() else {
                    return Err(Diagnostic::new(*span, "break is only valid inside a loop"));
                };
                self.terminate(jump(target));
            }
            Statement::Continue(span) => {
                let Some((_, target)) = self.loops.last().copied() else {
                    return Err(Diagnostic::new(
                        *span,
                        "continue is only valid inside a loop",
                    ));
                };
                self.terminate(jump(target));
            }
        }
        Ok(())
    }

    /// Stores `$name_fill`, bound beforehand, to each of `name`'s `length` elements.
    fn fill(&mut self, name: &str, length: u32, span: Span) -> Result<(), Diagnostic> {
        let fill_name = format!("${name}_fill");
        let at_name = format!("${name}_at");
        let at = Expr::Name(at_name.clone(), span);
        self.scoped(&[
            Statement::Bind {
                mutable: true,
                name: at_name.clone(),
                annotation: Some(TypeAnnotation::Value(TypeSpec::Primitive(TypeName::U16))),
                value: Expr::Integer(0, span),
                span,
            },
            Statement::While {
                condition: Expr::Binary {
                    op: BinaryOp::Less,
                    left: Box::new(at.clone()),
                    right: Box::new(Expr::Integer(i64::from(length), span)),
                    span,
                },
                body: vec![
                    Statement::Assign {
                        target: AssignTarget::Index {
                            base: name.into(),
                            index: at,
                        },
                        operation: None,
                        value: Expr::Name(fill_name, span),
                        span,
                    },
                    Statement::Assign {
                        target: AssignTarget::Name(at_name),
                        operation: Some(BinaryOp::Add),
                        value: Expr::Integer(1, span),
                        span,
                    },
                ],
                span,
            },
        ])
    }

    fn if_statement(
        &mut self,
        condition: &Expr,
        then_branch: &[Statement],
        else_branch: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let condition = self.expression(condition, Some(TypeName::Bool))?;
        let then_block = self.block();
        let else_block = self.block();
        let join_block = self.block();
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![required(condition, span)?],
            targets: vec![then_block, else_block],
        });

        self.current = then_block;
        self.scoped(then_branch)?;
        let then_falls = self.open();
        if then_falls {
            self.terminate(jump(join_block));
        }

        self.current = else_block;
        self.scoped(else_branch)?;
        let else_falls = self.open();
        if else_falls {
            self.terminate(jump(join_block));
        }

        self.current = join_block;
        if !then_falls && !else_falls {
            self.terminate(hir::Terminator {
                kind: "unreachable",
                operands: Vec::new(),
                targets: Vec::new(),
            });
        }
        Ok(())
    }

    fn while_statement(
        &mut self,
        condition: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let condition_block = self.block();
        let body_block = self.block();
        let exit_block = self.block();
        self.terminate(jump(condition_block));

        self.current = condition_block;
        let condition = self.expression(condition, Some(TypeName::Bool))?;
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![required(condition, span)?],
            targets: vec![body_block, exit_block],
        });

        self.current = body_block;
        self.loops.push((exit_block, condition_block));
        self.scoped(body)?;
        self.loops.pop();
        if self.open() {
            self.terminate(jump(condition_block));
        }
        self.current = exit_block;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn comprehension_binding(
        &mut self,
        mutable: bool,
        name: &str,
        annotation: Option<&TypeAnnotation>,
        expression: &Expr,
        item_name: &str,
        mode: IterationMode,
        iterable: &Expr,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Expr::Name(source_name, _) = iterable else {
            return Err(Diagnostic::new(
                iterable.span(),
                "a bounded comprehension currently requires a named fixed array",
            ));
        };
        if source_name == name {
            return Err(Diagnostic::new(
                span,
                "a comprehension cannot replace its own source",
            ));
        }
        let source = self.binding(source_name, iterable.span())?.clone();
        let BindingType::Array {
            element: source_element,
            length,
        } = source.type_
        else {
            return Err(Diagnostic::new(
                iterable.span(),
                "a materialized comprehension needs a statically bounded source",
            ));
        };
        let source_scalar = match source_element {
            ElementType::Scalar(type_name) => type_name,
            ElementType::Struct(_) => {
                return Err(Diagnostic::new(
                    span,
                    "struct comprehension elements must select a scalar field",
                ))
            }
        };

        self.scopes.push(BTreeMap::from([(
            item_name.into(),
            Binding {
                type_: BindingType::Scalar(source_scalar),
                mutable: mode == IterationMode::Mutable,
                storage: Storage::Parameter(0),
            },
        )]));
        let inferred = self
            .expression_type_hint(expression)
            .or_else(|| match expression {
                Expr::Integer(value, _) if i16::try_from(*value).is_ok() => Some(TypeName::I16),
                Expr::Integer(value, _) if i32::try_from(*value).is_ok() => Some(TypeName::I32),
                _ => None,
            });
        self.scopes.pop();

        let annotated = match annotation {
            None => None,
            Some(TypeAnnotation::Array {
                element,
                length: annotated_length,
            }) => {
                if *annotated_length != length {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "comprehension has {length} elements, annotation expects {annotated_length}"
                        ),
                    ));
                }
                Some(self.types.resolve_element(element, span)?)
            }
            Some(_) => {
                return Err(Diagnostic::new(
                    span,
                    "a comprehension binding needs an array annotation or inference",
                ))
            }
        };
        let result_type = match (annotated, inferred) {
            (Some(ElementType::Scalar(expected)), Some(actual)) if expected != actual => {
                return Err(type_mismatch(span, expected, actual))
            }
            (Some(ElementType::Scalar(type_name)), _) | (None, Some(type_name)) => type_name,
            (Some(ElementType::Struct(_)), _) => {
                return Err(Diagnostic::new(
                    span,
                    "struct-valued comprehensions are not in this slice",
                ))
            }
            (None, None) => {
                return Err(Diagnostic::new(
                    expression.span(),
                    "cannot infer comprehension element type; add an array annotation",
                ))
            }
        };
        let result_element = ElementType::Scalar(result_type);
        let type_id = self.types.array(result_element, length);
        let place = self.array_place(name, type_id, result_element, length, mutable);
        self.scopes.last_mut().expect("scope").insert(
            name.into(),
            Binding {
                type_: BindingType::Array {
                    element: result_element,
                    length,
                },
                mutable: true,
                storage: Storage::Place(place),
            },
        );
        let counter_name = format!("$comprehension_{name}");
        let counter = self.place(&counter_name, TypeName::U16, true);
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(counter), hir::Operand::Constant(U16, 0)],
            None,
        );
        self.scopes.last_mut().expect("scope").insert(
            counter_name.clone(),
            Binding {
                type_: BindingType::Scalar(TypeName::U16),
                mutable: true,
                storage: Storage::Place(counter),
            },
        );
        let body = vec![
            Statement::Assign {
                target: AssignTarget::Index {
                    base: name.into(),
                    index: Expr::Name(counter_name.clone(), span),
                },
                operation: None,
                value: expression.clone(),
                span,
            },
            Statement::Assign {
                target: AssignTarget::Name(counter_name),
                operation: Some(BinaryOp::Add),
                value: Expr::Integer(1, span),
                span,
            },
        ];
        self.for_statement(mode, item_name, iterable, &body, span)?;
        self.scopes
            .last_mut()
            .expect("scope")
            .get_mut(name)
            .expect("comprehension result")
            .mutable = mutable;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn dictionary_binding(
        &mut self,
        mutable: bool,
        name: &str,
        key_expression: &Expr,
        value_expression: &Expr,
        item_name: &str,
        mode: IterationMode,
        iterable: &Expr,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Expr::Name(source_name, _) = iterable else {
            return Err(Diagnostic::new(
                iterable.span(),
                "a bounded dictionary comprehension requires a named fixed array",
            ));
        };
        let source = self.binding(source_name, iterable.span())?.clone();
        let BindingType::Array {
            element: ElementType::Scalar(source_type),
            length: capacity,
        } = source.type_
        else {
            return Err(Diagnostic::new(
                iterable.span(),
                "a dictionary comprehension needs a scalar fixed-array source",
            ));
        };
        self.scopes.push(BTreeMap::from([(
            item_name.into(),
            Binding {
                type_: BindingType::Scalar(source_type),
                mutable: mode == IterationMode::Mutable,
                storage: Storage::Parameter(0),
            },
        )]));
        let infer = |expression: &Expr, this: &Self| {
            this.expression_type_hint(expression)
                .or_else(|| match expression {
                    Expr::Integer(value, _) if i16::try_from(*value).is_ok() => Some(TypeName::I16),
                    Expr::Integer(value, _) if i32::try_from(*value).is_ok() => Some(TypeName::I32),
                    _ => None,
                })
        };
        let key_type = infer(key_expression, self);
        let value_type = infer(value_expression, self);
        self.scopes.pop();
        let key_type = key_type.ok_or_else(|| {
            Diagnostic::new(key_expression.span(), "cannot infer dictionary key type")
        })?;
        let value_type = value_type.ok_or_else(|| {
            Diagnostic::new(
                value_expression.span(),
                "cannot infer dictionary value type",
            )
        })?;
        if !matches!(
            key_type,
            TypeName::Char
                | TypeName::I8
                | TypeName::U8
                | TypeName::I16
                | TypeName::U16
                | TypeName::I32
                | TypeName::U32
                | TypeName::Bool
        ) {
            return Err(Diagnostic::new(
                key_expression.span(),
                "dictionary keys must have an exact scalar equality type",
            ));
        }

        let keys_name = format!("$dict_{name}_keys");
        let values_name = format!("$dict_{name}_values");
        let length_name = format!("$dict_{name}_length");
        let key_element = ElementType::Scalar(key_type);
        let value_element = ElementType::Scalar(value_type);
        let keys_type = self.types.array(key_element, capacity);
        let values_type = self.types.array(value_element, capacity);
        let keys = self.array_place(&keys_name, keys_type, key_element, capacity, true);
        let values = self.array_place(&values_name, values_type, value_element, capacity, true);
        let length = self.place(&length_name, TypeName::U16, true);
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(length), hir::Operand::Constant(U16, 0)],
            None,
        );
        let scope = self.scopes.last_mut().expect("scope");
        scope.insert(
            keys_name.clone(),
            Binding {
                type_: BindingType::Array {
                    element: key_element,
                    length: capacity,
                },
                mutable: true,
                storage: Storage::Place(keys),
            },
        );
        scope.insert(
            values_name.clone(),
            Binding {
                type_: BindingType::Array {
                    element: value_element,
                    length: capacity,
                },
                mutable: true,
                storage: Storage::Place(values),
            },
        );
        scope.insert(
            length_name.clone(),
            Binding {
                type_: BindingType::Scalar(TypeName::U16),
                mutable: true,
                storage: Storage::Place(length),
            },
        );

        let key_name = format!("$dict_{name}_key");
        let value_name = format!("$dict_{name}_value");
        let scan_name = format!("$dict_{name}_scan");
        let found_name = format!("$dict_{name}_found");
        let scan = Expr::Name(scan_name.clone(), span);
        let length_expr = Expr::Name(length_name.clone(), span);
        let key = Expr::Name(key_name.clone(), span);
        let value = Expr::Name(value_name.clone(), span);
        let body = vec![
            Statement::Bind {
                mutable: false,
                name: key_name.clone(),
                annotation: Some(TypeAnnotation::Value(TypeSpec::Primitive(key_type))),
                value: key_expression.clone(),
                span,
            },
            Statement::Bind {
                mutable: false,
                name: value_name.clone(),
                annotation: Some(TypeAnnotation::Value(TypeSpec::Primitive(value_type))),
                value: value_expression.clone(),
                span,
            },
            Statement::Bind {
                mutable: true,
                name: scan_name.clone(),
                annotation: Some(TypeAnnotation::Value(TypeSpec::Primitive(TypeName::U16))),
                value: Expr::Integer(0, span),
                span,
            },
            Statement::Bind {
                mutable: true,
                name: found_name.clone(),
                annotation: Some(TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Bool))),
                value: Expr::Boolean(false, span),
                span,
            },
            Statement::While {
                condition: Expr::Binary {
                    op: BinaryOp::Less,
                    left: Box::new(scan.clone()),
                    right: Box::new(length_expr.clone()),
                    span,
                },
                body: vec![
                    Statement::If {
                        condition: Expr::Binary {
                            op: BinaryOp::Equal,
                            left: Box::new(Expr::Index {
                                base: Box::new(Expr::Name(keys_name.clone(), span)),
                                index: Box::new(scan.clone()),
                                span,
                            }),
                            right: Box::new(key.clone()),
                            span,
                        },
                        then_branch: vec![
                            Statement::Assign {
                                target: AssignTarget::Index {
                                    base: values_name.clone(),
                                    index: scan.clone(),
                                },
                                operation: None,
                                value: value.clone(),
                                span,
                            },
                            Statement::Assign {
                                target: AssignTarget::Name(found_name.clone()),
                                operation: None,
                                value: Expr::Boolean(true, span),
                                span,
                            },
                            Statement::Break(span),
                        ],
                        else_branch: Vec::new(),
                        span,
                    },
                    Statement::Assign {
                        target: AssignTarget::Name(scan_name.clone()),
                        operation: Some(BinaryOp::Add),
                        value: Expr::Integer(1, span),
                        span,
                    },
                ],
                span,
            },
            Statement::If {
                condition: Expr::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(Expr::Name(found_name, span)),
                    span,
                },
                then_branch: vec![
                    Statement::Assign {
                        target: AssignTarget::Index {
                            base: keys_name.clone(),
                            index: length_expr.clone(),
                        },
                        operation: None,
                        value: key,
                        span,
                    },
                    Statement::Assign {
                        target: AssignTarget::Index {
                            base: values_name.clone(),
                            index: length_expr.clone(),
                        },
                        operation: None,
                        value,
                        span,
                    },
                    Statement::Assign {
                        target: AssignTarget::Name(length_name.clone()),
                        operation: Some(BinaryOp::Add),
                        value: Expr::Integer(1, span),
                        span,
                    },
                ],
                else_branch: Vec::new(),
                span,
            },
        ];
        self.for_statement(mode, item_name, iterable, &body, span)?;
        self.scopes.last_mut().expect("scope").insert(
            name.into(),
            Binding {
                type_: BindingType::Dictionary {
                    key: key_type,
                    value: value_type,
                    capacity,
                },
                mutable,
                storage: Storage::Dictionary {
                    keys,
                    values,
                    length,
                },
            },
        );
        Ok(())
    }

    fn for_statement(
        &mut self,
        mode: IterationMode,
        name: &str,
        iterable: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if let Expr::Generator {
            element,
            binding,
            mode: generator_mode,
            iterable,
            ..
        } = iterable
        {
            if mode != IterationMode::Value {
                return Err(Diagnostic::new(
                    span,
                    "a generator is already a borrowed view",
                ));
            }
            let mut fused = Vec::with_capacity(body.len() + 1);
            fused.push(Statement::Bind {
                mutable: false,
                name: name.into(),
                annotation: None,
                value: element.as_ref().clone(),
                span,
            });
            fused.extend_from_slice(body);
            return self.for_statement(*generator_mode, binding, iterable, &fused, span);
        }
        let (array_name, range) = match iterable {
            Expr::Name(name, _) => (name.as_str(), None),
            Expr::Slice {
                base,
                start,
                end,
                span: range_span,
            } => {
                let Expr::Name(name, _) = base.as_ref() else {
                    return Err(Diagnostic::new(
                        base.span(),
                        "slice base must be a named array",
                    ));
                };
                (
                    name.as_str(),
                    Some((start.as_deref(), end.as_deref(), *range_span)),
                )
            }
            _ => {
                return Err(Diagnostic::new(
                    iterable.span(),
                    "for currently iterates a named sequence or scoped range",
                ))
            }
        };
        let array = self.binding(array_name, iterable.span())?.clone();
        let string = array.type_ == BindingType::Scalar(TypeName::String);
        let (element, mut length) = if string {
            (ElementType::Scalar(TypeName::Char), None)
        } else {
            array.type_.array().ok_or_else(|| {
                Diagnostic::new(iterable.span(), "for requires an array or string")
            })?
        };
        if string && range.is_some() {
            return Err(Diagnostic::new(span, "string ranges are not in this slice"));
        }
        let mut range_data = None;
        if let Some(range) = range {
            let Some(owner_length) = length else {
                return Err(Diagnostic::new(
                    span,
                    "nested slice ranges are not in this slice",
                ));
            };
            let (start, end) = self.slice_bounds(Some(range), owner_length, span)?;
            let Storage::Place(place) = &array.storage else {
                return Err(Diagnostic::new(
                    span,
                    "slice range needs an owned fixed array",
                ));
            };
            let pointer_type = self.types.pointer(element.id(), 0);
            let pointer = self.value_type(pointer_type);
            self.emit(
                "address",
                vec![pointer],
                vec![hir::Operand::Place(*place)],
                None,
            );
            range_data = Some(if start == 0 {
                pointer
            } else {
                self.indexed_pointer(
                    pointer,
                    hir::Operand::Constant(U16, i64::from(start)),
                    self.types.width(element.id()),
                    span,
                )?
            });
            length = Some(end - start);
        }
        if string && mode == IterationMode::Mutable {
            return Err(Diagnostic::new(
                span,
                "strings are immutable byte sequences",
            ));
        }
        if mode == IterationMode::Value && matches!(element, ElementType::Struct(_)) {
            return Err(Diagnostic::new(
                span,
                "by-value struct iteration awaits aggregate move semantics; use '&' or '&mut'",
            ));
        }
        if mode == IterationMode::Mutable && !array.mutable {
            return Err(Diagnostic::new(
                span,
                format!("cannot take a mutable view of immutable array {array_name:?}"),
            ));
        }
        let string_pointer = if string {
            Some(self.string_pointer(&array, iterable.span())?)
        } else {
            None
        };
        let length = match length {
            Some(length) => hir::Operand::Constant(
                U16,
                i64::from(u16::try_from(length).map_err(|_| {
                    Diagnostic::new(
                        iterable.span(),
                        "for array length exceeds the 16-bit target",
                    )
                })?),
            ),
            None => {
                let pointer = if let Some(pointer) = string_pointer {
                    pointer
                } else if let Storage::Slice(pointer) = &array.storage {
                    *pointer
                } else {
                    return Err(Diagnostic::new(iterable.span(), "view has no descriptor"));
                };
                let value = self.value(TypeName::U16);
                self.emit(
                    "load",
                    vec![value],
                    vec![hir::Operand::DescriptorPlace {
                        base: pointer,
                        field: "length",
                        type_id: U16,
                    }],
                    None,
                );
                hir::Operand::Value(value)
            }
        };

        let index_place = self.place(&format!("$for_{array_name}"), TypeName::U16, true);
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(index_place),
                hir::Operand::Constant(U16, 0),
            ],
            None,
        );
        let condition_block = self.block();
        let body_block = self.block();
        let increment_block = self.block();
        let exit_block = self.block();
        self.terminate(jump(condition_block));

        self.current = condition_block;
        let index = self.value(TypeName::U16);
        self.emit(
            "load",
            vec![index],
            vec![hir::Operand::Place(index_place)],
            None,
        );
        let condition = self.value(TypeName::Bool);
        self.emit(
            "below",
            vec![condition],
            vec![hir::Operand::Value(index), length],
            None,
        );
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(condition)],
            targets: vec![body_block, exit_block],
        });

        self.current = body_block;
        let element_width = self.types.width(element.id());
        let view_storage = if let Some(pointer) = string_pointer.or(range_data) {
            Storage::Reference(self.indexed_pointer(
                pointer,
                hir::Operand::Value(index),
                element_width,
                span,
            )?)
        } else {
            match array.storage {
                Storage::Place(place) => Storage::ArrayView {
                    place,
                    index: hir::Operand::Value(index),
                },
                Storage::Slice(descriptor) => {
                    let pointer = self.slice_data_pointer(descriptor, element);
                    Storage::Reference(self.indexed_pointer(
                        pointer,
                        hir::Operand::Value(index),
                        element_width,
                        span,
                    )?)
                }
                Storage::Parameter(_) | Storage::Reference(_) | Storage::ArrayView { .. } => {
                    unreachable!("checked above")
                }
                Storage::Dictionary { .. } => unreachable!("sequence is not a dictionary"),
            }
        };
        self.scopes.push(BTreeMap::new());
        self.scopes.last_mut().expect("scope").insert(
            name.into(),
            Binding {
                type_: match element {
                    ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                    ElementType::Struct(id) => BindingType::Struct(id),
                },
                mutable: mode == IterationMode::Mutable,
                storage: view_storage,
            },
        );
        self.loops.push((exit_block, increment_block));
        let result = self.statements(body);
        self.loops.pop();
        self.scopes.pop();
        result?;
        if self.open() {
            self.terminate(jump(increment_block));
        }

        self.current = increment_block;
        let old_index = self.value(TypeName::U16);
        self.emit(
            "load",
            vec![old_index],
            vec![hir::Operand::Place(index_place)],
            None,
        );
        let next_index = self.value(TypeName::U16);
        self.emit(
            "add",
            vec![next_index],
            vec![
                hir::Operand::Value(old_index),
                hir::Operand::Constant(U16, 1),
            ],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(index_place),
                hir::Operand::Value(next_index),
            ],
            None,
        );
        self.terminate(jump(condition_block));
        self.current = exit_block;
        Ok(())
    }

    fn range_statement(
        &mut self,
        name: &str,
        start: &Expr,
        end: &Expr,
        body: &[Statement],
        _span: Span,
    ) -> Result<(), Diagnostic> {
        let hint = self.expression_type_hint(end);
        if hint.is_some_and(|one| !is_integer(one)) {
            return Err(Diagnostic::new(end.span(), "range bounds must be integers"));
        }
        let start_value = self.expression(start, hint)?;
        if !is_integer(start_value.type_name) {
            return Err(Diagnostic::new(
                start.span(),
                "range bounds must be integers",
            ));
        }
        let type_name = start_value.type_name;
        let end_value = self.expression(end, Some(type_name))?;
        let counter_place = self.place(&format!("$range_{name}"), type_name, true);
        let limit_place = self.place(&format!("$range_limit_{name}"), type_name, false);
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(counter_place),
                required(start_value, start.span())?,
            ],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(limit_place),
                required(end_value, end.span())?,
            ],
            None,
        );

        let condition_block = self.block();
        let body_block = self.block();
        let increment_block = self.block();
        let exit_block = self.block();
        self.terminate(jump(condition_block));

        self.current = condition_block;
        let current = self.value(type_name);
        self.emit(
            "load",
            vec![current],
            vec![hir::Operand::Place(counter_place)],
            None,
        );
        let limit = self.value(type_name);
        self.emit(
            "load",
            vec![limit],
            vec![hir::Operand::Place(limit_place)],
            None,
        );
        let condition = self.value(TypeName::Bool);
        self.emit(
            if is_unsigned(type_name) {
                "below"
            } else {
                "lt"
            },
            vec![condition],
            vec![hir::Operand::Value(current), hir::Operand::Value(limit)],
            None,
        );
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(condition)],
            targets: vec![body_block, exit_block],
        });

        self.current = body_block;
        self.scopes.push(BTreeMap::new());
        self.scopes.last_mut().expect("scope").insert(
            name.into(),
            Binding {
                type_: BindingType::Scalar(type_name),
                mutable: false,
                storage: Storage::Place(counter_place),
            },
        );
        self.loops.push((exit_block, increment_block));
        let result = self.statements(body);
        self.loops.pop();
        self.scopes.pop();
        result?;
        if self.open() {
            self.terminate(jump(increment_block));
        }

        self.current = increment_block;
        let old_value = self.value(type_name);
        self.emit(
            "load",
            vec![old_value],
            vec![hir::Operand::Place(counter_place)],
            None,
        );
        let next_value = self.value(type_name);
        self.emit(
            "add",
            vec![next_value],
            vec![
                hir::Operand::Value(old_value),
                hir::Operand::Constant(type_id(type_name), 1),
            ],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(counter_place),
                hir::Operand::Value(next_value),
            ],
            None,
        );
        self.terminate(jump(condition_block));
        self.current = exit_block;
        Ok(())
    }

    fn scoped(&mut self, statements: &[Statement]) -> Result<(), Diagnostic> {
        self.scopes.push(BTreeMap::new());
        let result = self.statements(statements);
        self.scopes.pop();
        result
    }

    fn initialize_struct(
        &mut self,
        place: u32,
        index: hir::Operand,
        struct_id: u32,
        expression: &Expr,
    ) -> Result<(), Diagnostic> {
        self.store_struct_expression(
            &StructView {
                struct_id,
                place,
                pointer: None,
                indices: vec![index],
                offset: 0,
                mutable: true,
                owner: "array initializer".into(),
            },
            expression,
        )
    }

    fn store_struct_expression(
        &mut self,
        destination: &StructView,
        expression: &Expr,
    ) -> Result<(), Diagnostic> {
        let mut stores = Vec::new();
        self.prepare_struct_stores(destination, expression, &mut stores)?;
        for (place, value) in stores {
            self.emit("store", Vec::new(), vec![place, value], None);
        }
        Ok(())
    }

    fn prepare_struct_stores(
        &mut self,
        destination: &StructView,
        expression: &Expr,
        stores: &mut Vec<(hir::Operand, hir::Operand)>,
    ) -> Result<(), Diagnostic> {
        let layout = self
            .types
            .structure(destination.struct_id)
            .cloned()
            .expect("resolved struct type");
        let Expr::StructLiteral { name, fields, span } = expression else {
            let source = self.struct_view(expression, expression.span())?;
            if source.struct_id != destination.struct_id {
                let found = &self
                    .types
                    .structure(source.struct_id)
                    .expect("resolved struct type")
                    .name;
                return Err(Diagnostic::new(
                    expression.span(),
                    format!("expected {}, found {found}", layout.name),
                ));
            }
            return self.prepare_struct_copy(destination, &source, stores);
        };
        if let Some(name) = name {
            if name != &layout.name {
                return Err(Diagnostic::new(
                    *span,
                    format!("expected {} literal, found {name}", layout.name),
                ));
            }
        }
        let fields = match fields {
            StructLiteralFields::Named(fields) => fields.clone(),
            StructLiteralFields::Positional(values) => {
                if values.len() != layout.field_order.len() {
                    return Err(Diagnostic::new(
                        *span,
                        format!(
                            "{} literal expects {} fields, got {}",
                            layout.name,
                            layout.field_order.len(),
                            values.len()
                        ),
                    ));
                }
                layout
                    .field_order
                    .iter()
                    .zip(values)
                    .map(|(name, value)| (name.clone(), value.clone(), value.span()))
                    .collect()
            }
        };
        let mut seen = BTreeMap::new();
        for (name, value, field_span) in &fields {
            if seen.insert(name, *field_span).is_some() {
                return Err(Diagnostic::new(
                    *field_span,
                    format!("field {name:?} is initialized more than once"),
                ));
            }
            let field = layout.fields.get(name).ok_or_else(|| {
                Diagnostic::new(
                    *field_span,
                    format!("{} has no field {name:?}", layout.name),
                )
            })?;
            match field.type_ {
                ElementType::Scalar(type_name) => {
                    let value = self.expression(value, Some(type_name))?;
                    stores.push((
                        self.projected_place(destination, field.offset, type_name),
                        required(value, *field_span)?,
                    ));
                }
                ElementType::Struct(field_struct) => {
                    let nested = StructView {
                        struct_id: field_struct,
                        place: destination.place,
                        pointer: destination.pointer,
                        indices: destination.indices.clone(),
                        offset: destination.offset + field.offset,
                        mutable: destination.mutable,
                        owner: destination.owner.clone(),
                    };
                    self.prepare_struct_stores(&nested, value, stores)?;
                }
            }
        }
        let missing: Vec<_> = layout
            .fields
            .keys()
            .filter(|name| !seen.contains_key(*name))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(Diagnostic::new(
                *span,
                format!(
                    "{} literal is missing fields: {}",
                    layout.name,
                    missing.join(", ")
                ),
            ));
        }
        Ok(())
    }

    fn prepare_struct_copy(
        &mut self,
        destination: &StructView,
        source: &StructView,
        stores: &mut Vec<(hir::Operand, hir::Operand)>,
    ) -> Result<(), Diagnostic> {
        let layout = self
            .types
            .structure(destination.struct_id)
            .cloned()
            .expect("resolved struct type");
        for field in layout.fields.values() {
            match field.type_ {
                ElementType::Scalar(type_name) => {
                    let value = self.value(type_name);
                    self.emit(
                        "load",
                        vec![value],
                        vec![self.projected_place(source, field.offset, type_name)],
                        None,
                    );
                    stores.push((
                        self.projected_place(destination, field.offset, type_name),
                        hir::Operand::Value(value),
                    ));
                }
                ElementType::Struct(struct_id) => {
                    let destination = StructView {
                        struct_id,
                        place: destination.place,
                        pointer: destination.pointer,
                        indices: destination.indices.clone(),
                        offset: destination.offset + field.offset,
                        mutable: destination.mutable,
                        owner: destination.owner.clone(),
                    };
                    let source = StructView {
                        struct_id,
                        place: source.place,
                        pointer: source.pointer,
                        indices: source.indices.clone(),
                        offset: source.offset + field.offset,
                        mutable: source.mutable,
                        owner: source.owner.clone(),
                    };
                    self.prepare_struct_copy(&destination, &source, stores)?;
                }
            }
        }
        Ok(())
    }

    fn projected_place(
        &self,
        view: &StructView,
        field_offset: u32,
        type_name: TypeName,
    ) -> hir::Operand {
        if let Some(pointer) = view.pointer {
            hir::Operand::IndirectPlace {
                base: pointer,
                offset: view.offset + field_offset,
                type_id: type_id(type_name),
                inbounds: false,
            }
        } else {
            hir::Operand::ProjectedPlace {
                place: view.place,
                indices: view.indices.clone(),
                offset: view.offset + field_offset,
                type_id: type_id(type_name),
            }
        }
    }

    fn struct_expression_type(
        &self,
        expression: &Expr,
        span: Span,
    ) -> Result<Option<u32>, Diagnostic> {
        match expression {
            Expr::StructLiteral {
                name: Some(name), ..
            } => self
                .types
                .structs
                .get(name)
                .map(|one| Some(one.id))
                .ok_or_else(|| Diagnostic::new(span, format!("unknown struct {name:?}"))),
            Expr::StructLiteral { name: None, .. } => Err(Diagnostic::new(
                span,
                "anonymous struct literal requires an expected struct type",
            )),
            Expr::Name(name, _) => Ok(match self.binding(name, span)?.type_ {
                BindingType::Struct(struct_id) => Some(struct_id),
                _ => None,
            }),
            Expr::Index { base, .. } => {
                let Expr::Name(name, _) = base.as_ref() else {
                    return Ok(None);
                };
                Ok(match self.binding(name, span)?.type_ {
                    BindingType::Array {
                        element: ElementType::Struct(struct_id),
                        ..
                    }
                    | BindingType::Slice {
                        element: ElementType::Struct(struct_id),
                    } => Some(struct_id),
                    _ => None,
                })
            }
            Expr::Member { base, field, .. } => {
                let Some(struct_id) = self.struct_expression_type(base, span)? else {
                    return Ok(None);
                };
                let layout = self
                    .types
                    .structure(struct_id)
                    .expect("resolved struct type");
                let field = layout.fields.get(field).ok_or_else(|| {
                    Diagnostic::new(span, format!("{} has no field {field:?}", layout.name))
                })?;
                Ok(match field.type_ {
                    ElementType::Struct(struct_id) => Some(struct_id),
                    ElementType::Scalar(_) => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn assignment_target(
        &mut self,
        target: &AssignTarget,
        span: Span,
    ) -> Result<AssignmentPlace, Diagnostic> {
        match target {
            AssignTarget::Member { base, field } => {
                let parent = self.struct_view(base, span)?;
                if !parent.mutable {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {:?} is immutable", parent.owner),
                    ));
                }
                let layout = self
                    .types
                    .structure(parent.struct_id)
                    .expect("resolved struct type");
                let member = layout.fields.get(field).copied().ok_or_else(|| {
                    Diagnostic::new(span, format!("{} has no field {field:?}", layout.name))
                })?;
                Ok(match member.type_ {
                    ElementType::Scalar(type_name) => AssignmentPlace::Scalar(
                        self.projected_place(&parent, member.offset, type_name),
                        type_name,
                    ),
                    ElementType::Struct(struct_id) => AssignmentPlace::Struct(StructView {
                        struct_id,
                        place: parent.place,
                        pointer: parent.pointer,
                        indices: parent.indices,
                        offset: parent.offset + member.offset,
                        mutable: true,
                        owner: parent.owner,
                    }),
                })
            }
            AssignTarget::Name(name) => {
                let binding = self.binding(name, span)?.clone();
                if !binding.mutable {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {name:?} is immutable"),
                    ));
                }
                match binding.type_ {
                    BindingType::Scalar(type_name) => {
                        let destination = match binding.storage {
                            Storage::Place(place) => hir::Operand::Place(place),
                            Storage::ArrayView { place, index } => {
                                hir::Operand::ArrayElement(place, vec![index])
                            }
                            Storage::Parameter(_) => {
                                return Err(Diagnostic::new(span, "parameters are immutable"))
                            }
                            Storage::Reference(pointer) => hir::Operand::IndirectPlace {
                                base: pointer,
                                offset: 0,
                                type_id: type_id(type_name),
                                inbounds: false,
                            },
                            Storage::Slice(_) => unreachable!("a scalar binding is not a slice"),
                            Storage::Dictionary { .. } => {
                                unreachable!("a scalar binding is not a dictionary")
                            }
                        };
                        Ok(AssignmentPlace::Scalar(destination, type_name))
                    }
                    BindingType::Struct(struct_id) => {
                        let (place, pointer, indices) = match binding.storage {
                            Storage::Place(place) => (place, None, Vec::new()),
                            Storage::ArrayView { place, index } => (place, None, vec![index]),
                            Storage::Parameter(_) => {
                                return Err(Diagnostic::new(span, "parameters are immutable"))
                            }
                            Storage::Reference(pointer) => (0, Some(pointer), Vec::new()),
                            Storage::Slice(_) => unreachable!("a struct binding is not a slice"),
                            Storage::Dictionary { .. } => {
                                unreachable!("a struct binding is not a dictionary")
                            }
                        };
                        Ok(AssignmentPlace::Struct(StructView {
                            struct_id,
                            place,
                            pointer,
                            indices,
                            offset: 0,
                            mutable: true,
                            owner: name.clone(),
                        }))
                    }
                    BindingType::Array { .. } | BindingType::Slice { .. } => Err(Diagnostic::new(
                        span,
                        "whole array assignment is not supported",
                    )),
                    BindingType::Dictionary { .. } => Err(Diagnostic::new(
                        span,
                        "whole dictionary assignment is not supported",
                    )),
                }
            }
            AssignTarget::Index { base, index } => {
                let binding = self.binding(base, span)?.clone();
                if !binding.mutable {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {base:?} is immutable"),
                    ));
                }
                let Some((element, length)) = binding.type_.array() else {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {base:?} is not an array"),
                    ));
                };
                let index = self.array_index(index, length)?;
                Ok(match element {
                    ElementType::Scalar(type_name) => AssignmentPlace::Scalar(
                        self.indexed_place(
                            &binding.storage,
                            index,
                            ElementType::Scalar(type_name),
                            width(type_name),
                            span,
                        )?,
                        type_name,
                    ),
                    ElementType::Struct(struct_id) => {
                        let (place, pointer, indices) = match binding.storage {
                            Storage::Place(place) => (place, None, vec![index]),
                            Storage::Reference(pointer) => {
                                let width = self.types.width(struct_id);
                                let pointer = self.indexed_pointer(pointer, index, width, span)?;
                                (0, Some(pointer), Vec::new())
                            }
                            Storage::Slice(descriptor) => {
                                let width = self.types.width(struct_id);
                                let pointer = self
                                    .slice_data_pointer(descriptor, ElementType::Struct(struct_id));
                                let pointer = self.indexed_pointer(pointer, index, width, span)?;
                                (0, Some(pointer), Vec::new())
                            }
                            Storage::Parameter(_) | Storage::ArrayView { .. } => {
                                return Err(Diagnostic::new(span, "array has no storage"))
                            }
                            Storage::Dictionary { .. } => unreachable!("array is not a dictionary"),
                        };
                        AssignmentPlace::Struct(StructView {
                            struct_id,
                            place,
                            pointer,
                            indices,
                            offset: 0,
                            mutable: true,
                            owner: base.clone(),
                        })
                    }
                })
            }
        }
    }

    fn member_place(
        &mut self,
        base: &Expr,
        field_name: &str,
        span: Span,
    ) -> Result<(hir::Operand, TypeName, bool, String), Diagnostic> {
        let view = self.struct_view(base, span)?;
        let layout = self
            .types
            .structure(view.struct_id)
            .expect("resolved struct type");
        let field = layout.fields.get(field_name).copied().ok_or_else(|| {
            Diagnostic::new(span, format!("{} has no field {field_name:?}", layout.name))
        })?;
        let ElementType::Scalar(type_name) = field.type_ else {
            return Err(Diagnostic::new(
                span,
                "a nested struct value must be used through one of its fields",
            ));
        };
        Ok((
            self.projected_place(&view, field.offset, type_name),
            type_name,
            view.mutable,
            view.owner,
        ))
    }

    fn struct_view(&mut self, expression: &Expr, span: Span) -> Result<StructView, Diagnostic> {
        match expression {
            Expr::Name(name, _) => {
                let binding = self.binding(name, span)?.clone();
                let BindingType::Struct(struct_id) = binding.type_ else {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {name:?} is not a struct"),
                    ));
                };
                let (place, pointer, indices) = match binding.storage {
                    Storage::Place(place) => (place, None, Vec::new()),
                    Storage::ArrayView { place, index } => (place, None, vec![index]),
                    Storage::Parameter(_) => {
                        return Err(Diagnostic::new(span, "struct has no addressable storage"))
                    }
                    Storage::Reference(pointer) => (0, Some(pointer), Vec::new()),
                    Storage::Slice(_) => unreachable!("a struct binding is not a slice"),
                    Storage::Dictionary { .. } => {
                        unreachable!("a struct binding is not a dictionary")
                    }
                };
                Ok(StructView {
                    struct_id,
                    place,
                    pointer,
                    indices,
                    offset: 0,
                    mutable: binding.mutable,
                    owner: name.clone(),
                })
            }
            Expr::Index { base, index, .. } => {
                let Expr::Name(name, _) = base.as_ref() else {
                    return Err(Diagnostic::new(span, "array base must be a named binding"));
                };
                let binding = self.binding(name, span)?.clone();
                let Some((element, length)) = binding.type_.array() else {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {name:?} is not an array"),
                    ));
                };
                let ElementType::Struct(struct_id) = element else {
                    return Err(Diagnostic::new(span, "array element is not a struct"));
                };
                let index = self.array_index(index, length)?;
                let (place, pointer, indices) = match binding.storage {
                    Storage::Place(place) => (place, None, vec![index]),
                    Storage::Reference(pointer) => {
                        let width = self.types.width(struct_id);
                        let pointer = self.indexed_pointer(pointer, index, width, span)?;
                        (0, Some(pointer), Vec::new())
                    }
                    Storage::Slice(descriptor) => {
                        let width = self.types.width(struct_id);
                        let pointer =
                            self.slice_data_pointer(descriptor, ElementType::Struct(struct_id));
                        let pointer = self.indexed_pointer(pointer, index, width, span)?;
                        (0, Some(pointer), Vec::new())
                    }
                    Storage::Parameter(_) | Storage::ArrayView { .. } => {
                        return Err(Diagnostic::new(span, "array has no storage"))
                    }
                    Storage::Dictionary { .. } => unreachable!("array is not a dictionary"),
                };
                Ok(StructView {
                    struct_id,
                    place,
                    pointer,
                    indices,
                    offset: 0,
                    mutable: binding.mutable,
                    owner: name.clone(),
                })
            }
            Expr::Member {
                base,
                field,
                span: member_span,
            } => {
                let parent = self.struct_view(base, *member_span)?;
                let layout = self
                    .types
                    .structure(parent.struct_id)
                    .expect("resolved struct type");
                let field = layout.fields.get(field).copied().ok_or_else(|| {
                    Diagnostic::new(
                        *member_span,
                        format!("{} has no field {field:?}", layout.name),
                    )
                })?;
                let ElementType::Struct(field_struct) = field.type_ else {
                    return Err(Diagnostic::new(
                        *member_span,
                        "scalar field cannot be used as a struct",
                    ));
                };
                Ok(StructView {
                    struct_id: field_struct,
                    place: parent.place,
                    pointer: parent.pointer,
                    indices: parent.indices,
                    offset: parent.offset + field.offset,
                    mutable: parent.mutable,
                    owner: parent.owner,
                })
            }
            _ => Err(Diagnostic::new(
                span,
                "expression is not an addressable struct",
            )),
        }
    }

    fn expression(
        &mut self,
        expression: &Expr,
        expected: Option<TypeName>,
    ) -> Result<TypedOperand, Diagnostic> {
        match expression {
            Expr::Integer(value, span) => self.integer(*value, expected, *span),
            Expr::Float(spelling, span) => self.float(spelling, expected, *span),
            Expr::Character(value, span) => {
                if expected.is_some_and(|one| one != TypeName::Char) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        TypeName::Char,
                    ));
                }
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Constant(CHAR, i64::from(*value))),
                    type_name: TypeName::Char,
                })
            }
            Expr::String(value, span) => self.string_literal(value, expected, *span),
            Expr::FString { span, .. } => Err(Diagnostic::new(
                *span,
                "an f-string is currently valid only as a direct print argument",
            )),
            Expr::Array(_, span) | Expr::Repeat { span, .. } => Err(Diagnostic::new(
                *span,
                "an array literal is valid only as a fixed-array initializer",
            )),
            Expr::Conversion {
                target,
                value,
                span,
            } => self.conversion(*target, value, expected, *span),
            Expr::Comprehension { span, .. } => Err(Diagnostic::new(
                *span,
                "a comprehension is valid only as an array initializer",
            )),
            Expr::Generator { span, .. } => Err(Diagnostic::new(
                *span,
                "a generator is non-escaping and must be consumed by a for loop",
            )),
            Expr::DictComprehension { span, .. } => Err(Diagnostic::new(
                *span,
                "a dictionary comprehension is valid only as a binding initializer",
            )),
            Expr::StructLiteral { span, .. } => Err(Diagnostic::new(
                *span,
                "a struct literal requires an expected struct type",
            )),
            Expr::Borrow { span, .. } => Err(Diagnostic::new(
                *span,
                "a borrow is valid only as a borrowed function argument",
            )),
            Expr::Boolean(value, span) => {
                if expected.is_some_and(|one| one != TypeName::Bool) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        TypeName::Bool,
                    ));
                }
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Constant(BOOL, if *value { -1 } else { 0 })),
                    type_name: TypeName::Bool,
                })
            }
            Expr::Name(name, span) => {
                let binding = self.binding(name, *span)?.clone();
                let BindingType::Scalar(type_name) = binding.type_ else {
                    return Err(Diagnostic::new(
                        *span,
                        format!("aggregate {name:?} requires an index or field"),
                    ));
                };
                if expected.is_some_and(|one| one != type_name) {
                    return Err(type_mismatch(*span, expected.expect("checked"), type_name));
                }
                let operand = match binding.storage {
                    Storage::Parameter(value) => hir::Operand::Value(value),
                    Storage::Place(place) => {
                        let value = self.value(type_name);
                        self.emit("load", vec![value], vec![hir::Operand::Place(place)], None);
                        hir::Operand::Value(value)
                    }
                    Storage::ArrayView { place, index } => {
                        let value = self.value(type_name);
                        self.emit(
                            "load",
                            vec![value],
                            vec![hir::Operand::ArrayElement(place, vec![index])],
                            None,
                        );
                        hir::Operand::Value(value)
                    }
                    Storage::Reference(pointer) => {
                        let value = self.value(type_name);
                        self.emit(
                            "load",
                            vec![value],
                            vec![hir::Operand::IndirectPlace {
                                base: pointer,
                                offset: 0,
                                type_id: type_id(type_name),
                                inbounds: false,
                            }],
                            None,
                        );
                        hir::Operand::Value(value)
                    }
                    Storage::Slice(_) => unreachable!("a scalar binding is not a slice"),
                    Storage::Dictionary { .. } => {
                        unreachable!("a scalar binding is not a dictionary")
                    }
                };
                Ok(TypedOperand {
                    operand: Some(operand),
                    type_name,
                })
            }
            Expr::Index { base, index, span } => {
                self.index_expression(base, index, expected, *span)
            }
            Expr::Slice { span, .. } => Err(Diagnostic::new(
                *span,
                "a slice is a scoped view and cannot be used as a scalar value",
            )),
            Expr::Member { base, field, span } => {
                let (place, type_name, _, _) = self.member_place(base, field, *span)?;
                if expected.is_some_and(|one| one != type_name) {
                    return Err(type_mismatch(*span, expected.expect("checked"), type_name));
                }
                let result = self.value(type_name);
                self.emit("load", vec![result], vec![place], None);
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Value(result)),
                    type_name,
                })
            }
            Expr::Unary { op, operand, span } => {
                if *op == UnaryOp::Negative {
                    if let Expr::Integer(value, _) = operand.as_ref() {
                        let wanted = expected.unwrap_or({
                            if *value <= 32768 {
                                TypeName::I16
                            } else {
                                TypeName::I32
                            }
                        });
                        return self.integer(-*value, Some(wanted), *span);
                    }
                }
                let wanted = match op {
                    UnaryOp::Negative => expected.filter(|one| is_signed(*one) || is_float(*one)),
                    UnaryOp::Complement => expected.filter(|one| is_integer(*one)),
                    UnaryOp::Not => Some(TypeName::Bool),
                };
                let operand = self.expression(operand, wanted)?;
                match op {
                    UnaryOp::Negative
                        if !is_signed(operand.type_name) && !is_float(operand.type_name) =>
                    {
                        return Err(Diagnostic::new(
                            *span,
                            "unary '-' requires a signed integer or float",
                        ))
                    }
                    UnaryOp::Not if operand.type_name != TypeName::Bool => {
                        return Err(Diagnostic::new(*span, "not requires bool"))
                    }
                    UnaryOp::Complement if !is_integer(operand.type_name) => {
                        return Err(Diagnostic::new(*span, "'~' requires an integer"))
                    }
                    _ => {}
                }
                let result = self.value(operand.type_name);
                self.emit(
                    match op {
                        UnaryOp::Negative if is_float(operand.type_name) => "fneg",
                        UnaryOp::Negative => "neg",
                        UnaryOp::Not | UnaryOp::Complement => "not",
                    },
                    vec![result],
                    vec![required(operand.clone(), *span)?],
                    None,
                );
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Value(result)),
                    type_name: operand.type_name,
                })
            }
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => self.binary(*op, left, right, expected, *span),
            Expr::Call {
                name,
                arguments,
                span,
            } => self.call(name, arguments, expected, *span),
            Expr::MethodCall {
                receiver,
                name,
                arguments,
                span,
            } => self.array_method(receiver, name, arguments, expected, *span),
        }
    }

    fn integer(
        &self,
        value: i64,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if let Some(type_name @ TypeName::Fixed { fraction, .. }) = expected {
            let scaled = i128::from(value) << fraction;
            let value = fixed_storage_value(scaled, type_name, span)?;
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Constant(type_id(type_name), value)),
                type_name,
            });
        }
        let type_name = match expected {
            Some(type_name) if is_integer(type_name) || type_name == TypeName::Char => type_name,
            Some(other) => return Err(type_mismatch(span, other, TypeName::I16)),
            None if i16::try_from(value).is_ok() => TypeName::I16,
            None if i32::try_from(value).is_ok() => TypeName::I32,
            None => return Err(Diagnostic::new(span, "integer literal does not fit i32")),
        };
        let fits = match type_name {
            TypeName::Char | TypeName::U8 => u8::try_from(value).is_ok(),
            TypeName::I8 => i8::try_from(value).is_ok(),
            TypeName::I16 => i16::try_from(value).is_ok(),
            TypeName::U16 => u16::try_from(value).is_ok(),
            TypeName::I32 => i32::try_from(value).is_ok(),
            TypeName::U32 => u32::try_from(value).is_ok(),
            _ => false,
        };
        if !fits {
            return Err(Diagnostic::new(
                span,
                format!(
                    "integer literal {value} does not fit {}",
                    type_name_text(type_name)
                ),
            ));
        }
        Ok(TypedOperand {
            operand: Some(hir::Operand::Constant(type_id(type_name), value)),
            type_name,
        })
    }

    fn float(
        &mut self,
        spelling: &str,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if let Some(type_name @ TypeName::Fixed { fraction, .. }) = expected {
            let scaled = scaled_decimal(spelling, fraction, span)?;
            let value = fixed_storage_value(scaled, type_name, span)?;
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Constant(type_id(type_name), value)),
                type_name,
            });
        }
        let type_name = match expected {
            Some(type_name) if is_float(type_name) => type_name,
            Some(other) => return Err(type_mismatch(span, other, TypeName::F64)),
            None => TypeName::F64,
        };
        let parsed = spelling
            .parse::<f64>()
            .map_err(|_| Diagnostic::new(span, "invalid floating literal"))?;
        let bits = match type_name {
            TypeName::F32 => {
                let rounded = parsed as f32;
                if !rounded.is_finite() {
                    return Err(Diagnostic::new(span, "floating literal does not fit f32"));
                }
                u64::from(rounded.to_bits())
            }
            TypeName::F64 => {
                if !parsed.is_finite() {
                    return Err(Diagnostic::new(span, "floating literal does not fit f64"));
                }
                parsed.to_bits()
            }
            _ => unreachable!(),
        };
        let symbol = self.literals.float(type_name, bits);
        let place = if let Some(place) = self.constant_places.get(&symbol) {
            *place
        } else {
            let place = self.static_place(symbol, type_name);
            self.constant_places.insert(symbol, place);
            place
        };
        let result = self.value(type_name);
        self.emit("load", vec![result], vec![hir::Operand::Place(place)], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name,
        })
    }

    fn string_literal(
        &mut self,
        bytes: &[u8],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::String) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::String,
            ));
        }
        if bytes.contains(&0) {
            return Err(Diagnostic::new(
                span,
                "string literals cannot contain an embedded NUL",
            ));
        }
        if bytes.len() > u16::MAX as usize {
            return Err(Diagnostic::new(
                span,
                "string literal exceeds the 16-bit descriptor",
            ));
        }
        let symbol = self.literals.string(bytes);
        let place = if let Some(place) = self.constant_places.get(&symbol) {
            *place
        } else {
            let place = self.static_string_place(symbol, bytes.len() as u32 + 1);
            self.constant_places.insert(symbol, place);
            place
        };
        let result = self.value(TypeName::String);
        self.emit(
            "address",
            vec![result],
            vec![hir::Operand::Place(place)],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: TypeName::String,
        })
    }

    fn array_index(
        &mut self,
        expression: &Expr,
        length: Option<u32>,
    ) -> Result<hir::Operand, Diagnostic> {
        if let (Some(length), Expr::Integer(value, span)) = (length, expression) {
            if *value < 0 || *value >= i64::from(length) {
                return Err(Diagnostic::new(
                    *span,
                    format!("array index {value} is outside 0..{length}"),
                ));
            }
        }
        let index = self.expression(expression, None)?;
        if !is_integer(index.type_name) {
            return Err(Diagnostic::new(
                expression.span(),
                "array index must be an integer",
            ));
        }
        required(index, expression.span())
    }

    fn index_expression(
        &mut self,
        base: &Expr,
        index: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let Expr::Name(name, _) = base else {
            return Err(Diagnostic::new(span, "array base must be a named binding"));
        };
        let binding = self.binding(name, span)?.clone();
        let Some((element, length)) = binding.type_.array() else {
            return Err(Diagnostic::new(
                span,
                format!("binding {name:?} is not an array"),
            ));
        };
        let ElementType::Scalar(element) = element else {
            return Err(Diagnostic::new(
                span,
                "a struct array element must be used through one of its fields",
            ));
        };
        if expected.is_some_and(|one| one != element) {
            return Err(type_mismatch(span, expected.expect("checked"), element));
        }
        let index = self.array_index(index, length)?;
        let place = self.indexed_place(
            &binding.storage,
            index,
            ElementType::Scalar(element),
            width(element),
            span,
        )?;
        let result = self.value(element);
        self.emit("load", vec![result], vec![place], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: element,
        })
    }

    fn indexed_place(
        &mut self,
        storage: &Storage,
        index: hir::Operand,
        element: ElementType,
        element_width: u32,
        span: Span,
    ) -> Result<hir::Operand, Diagnostic> {
        let element_type = element.id();
        match storage {
            Storage::Place(place) => Ok(hir::Operand::ArrayElement(*place, vec![index])),
            Storage::Slice(descriptor) => {
                let pointer = self.slice_data_pointer(*descriptor, element);
                let address = self.indexed_pointer(pointer, index, element_width, span)?;
                Ok(hir::Operand::IndirectPlace {
                    base: address,
                    offset: 0,
                    type_id: element_type,
                    inbounds: true,
                })
            }
            Storage::Reference(pointer) => {
                let address = self.indexed_pointer(*pointer, index, element_width, span)?;
                Ok(hir::Operand::IndirectPlace {
                    base: address,
                    offset: 0,
                    type_id: element_type,
                    inbounds: true,
                })
            }
            Storage::Parameter(_) | Storage::ArrayView { .. } => {
                Err(Diagnostic::new(span, "array has no indexable storage"))
            }
            Storage::Dictionary { .. } => unreachable!("array is not a dictionary"),
        }
    }

    fn string_pointer(&mut self, binding: &Binding, span: Span) -> Result<u32, Diagnostic> {
        match binding.storage {
            Storage::Parameter(value) => Ok(value),
            Storage::Place(place) => {
                let value = self.value(TypeName::String);
                self.emit("load", vec![value], vec![hir::Operand::Place(place)], None);
                Ok(value)
            }
            _ => Err(Diagnostic::new(span, "string has no scalar pointer value")),
        }
    }

    fn slice_data_pointer(&mut self, descriptor: u32, element: ElementType) -> u32 {
        let pointer_type = self.types.pointer(element.id(), 0);
        let pointer = self.value_type(pointer_type);
        self.emit(
            "load",
            vec![pointer],
            vec![hir::Operand::IndirectPlace {
                base: descriptor,
                offset: 4,
                type_id: pointer_type,
                inbounds: false,
            }],
            None,
        );
        pointer
    }

    fn indexed_pointer(
        &mut self,
        pointer: u32,
        index: hir::Operand,
        element_width: u32,
        span: Span,
    ) -> Result<u32, Diagnostic> {
        let byte_offset = match index {
            hir::Operand::Constant(_, value) => {
                hir::Operand::Constant(U16, value * i64::from(element_width))
            }
            hir::Operand::Value(value) if element_width == 1 => hir::Operand::Value(value),
            hir::Operand::Value(value) => {
                let index_type = self
                    .values
                    .iter()
                    .find(|one| one.id == value)
                    .map(|one| one.type_id)
                    .ok_or_else(|| Diagnostic::new(span, "array index has no type"))?;
                let scaled = self.value_type(index_type);
                self.emit(
                    "mul",
                    vec![scaled],
                    vec![
                        hir::Operand::Value(value),
                        hir::Operand::Constant(index_type, i64::from(element_width)),
                    ],
                    None,
                );
                hir::Operand::Value(scaled)
            }
            _ => return Err(Diagnostic::new(span, "invalid array index operand")),
        };
        let pointer_type = self
            .values
            .iter()
            .find(|one| one.id == pointer)
            .map(|one| one.type_id)
            .ok_or_else(|| Diagnostic::new(span, "array reference has no type"))?;
        let address = self.value_type(pointer_type);
        self.emit(
            "ptr_offset",
            vec![address],
            vec![hir::Operand::Value(pointer), byte_offset],
            None,
        );
        Ok(address)
    }

    fn binary(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if matches!(operation, BinaryOp::Is | BinaryOp::IsNot) {
            return self.identity(operation, left, right, expected, span);
        }
        if matches!(operation, BinaryOp::And | BinaryOp::Or) {
            return self.logical(operation, left, right, expected, span);
        }
        let comparison = is_comparison(operation);
        if comparison && expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let mut left_expected = (!comparison).then_some(expected).flatten();
        if left_expected.is_none() && !is_shift(operation) {
            let hint = self.expression_type_hint(right);
            if (matches!(left, Expr::Integer(..))
                && hint
                    .is_some_and(|one| is_integer(one) || one == TypeName::Char || is_fixed(one)))
                || (matches!(left, Expr::Float(..))
                    && hint.is_some_and(|one| is_float(one) || is_fixed(one)))
            {
                left_expected = hint;
            }
        }
        let left = self.expression(left, left_expected)?;
        if left.type_name == TypeName::String {
            return Err(Diagnostic::new(
                span,
                "string comparison is not in the minimal runtime slice",
            ));
        }
        operand_rule(operation, left.type_name, span)?;
        let right = self.right_operand(operation, right, left.type_name)?;
        self.binary_operands(operation, left, right, expected, span)
    }

    /// A shift's count has its own unsigned type; every other right operand has the left's.
    fn right_operand(
        &mut self,
        operation: BinaryOp,
        right: &Expr,
        left: TypeName,
    ) -> Result<TypedOperand, Diagnostic> {
        if !is_shift(operation) {
            return self.expression(right, Some(left));
        }
        let literal = matches!(right, Expr::Integer(..));
        self.expression(right, literal.then_some(TypeName::U16))
    }

    /// `and` and `or` evaluate their right operand only when it decides the result.
    fn logical(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let name = format!("$logical{}", self.next_place);
        let result = self.place(&name, TypeName::Bool, true);
        let left = self.expression(left, Some(TypeName::Bool))?;
        let left = required(left, span)?;
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(result), left.clone()],
            None,
        );
        let decide = self.block();
        let join = self.block();
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![left],
            targets: if operation == BinaryOp::And {
                vec![decide, join]
            } else {
                vec![join, decide]
            },
        });
        self.current = decide;
        let right = self.expression(right, Some(TypeName::Bool))?;
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(result), required(right, span)?],
            None,
        );
        self.terminate(jump(join));
        self.current = join;
        let value = self.value(TypeName::Bool);
        self.emit("load", vec![value], vec![hir::Operand::Place(result)], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(value)),
            type_name: TypeName::Bool,
        })
    }

    fn conversion(
        &mut self,
        target: TypeName,
        value: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != target) {
            return Err(type_mismatch(span, expected.expect("checked"), target));
        }
        let literal = match value {
            Expr::Integer(..) => true,
            Expr::Unary {
                op: UnaryOp::Negative,
                operand,
                ..
            } => matches!(operand.as_ref(), Expr::Integer(..)),
            _ => false,
        };
        // A literal takes the target type, so `u8(300)` is rejected rather than wrapped.
        let value = self.expression(value, (literal && is_integer(target)).then_some(target))?;
        let source = value.type_name;
        if source == target {
            return Ok(value);
        }
        let op = match (source, target) {
            (from, to) if is_float(from) && is_integer(to) => "truncate",
            (from, to) if (is_integer(from) || is_float(from)) && is_numeric(to) && !is_fixed(to) => {
                "convert"
            }
            (TypeName::Bool, to) if is_integer(to) => "convert",
            (TypeName::Char, TypeName::U8) | (TypeName::U8, TypeName::Char) => "convert",
            _ => {
                return Err(Diagnostic::new(
                    span,
                    format!(
                        "no conversion from {} to {}",
                        type_name_text(source),
                        type_name_text(target)
                    ),
                ))
            }
        };
        let result = self.value(target);
        self.emit(op, vec![result], vec![required(value, span)?], None);
        if source != TypeName::Bool {
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Value(result)),
                type_name: target,
            });
        }
        // `true` is all ones.
        let bit = self.value(target);
        self.emit(
            "and",
            vec![bit],
            vec![
                hir::Operand::Value(result),
                hir::Operand::Constant(type_id(target), 1),
            ],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(bit)),
            type_name: target,
        })
    }

    fn binary_operands(
        &mut self,
        operation: BinaryOp,
        left: TypedOperand,
        right: TypedOperand,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if matches!(operation, BinaryOp::Is | BinaryOp::IsNot) {
            return Err(Diagnostic::new(
                span,
                "identity is not a numeric assignment operator",
            ));
        }
        let comparison = is_comparison(operation);
        if comparison && expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        operand_rule(operation, left.type_name, span)?;
        if is_shift(operation) {
            if !is_integer(right.type_name) || !is_unsigned(right.type_name) {
                return Err(Diagnostic::new(
                    span,
                    "a shift count must be an unsigned integer",
                ));
            }
            let bits = 8 * width(left.type_name);
            if let Some(hir::Operand::Constant(_, count)) = right.operand {
                if count >= i64::from(bits) {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "shift count {count} is not less than {}'s {bits} bits",
                            type_name_text(left.type_name)
                        ),
                    ));
                }
            }
        } else if left.type_name != right.type_name {
            return Err(type_mismatch(span, left.type_name, right.type_name));
        }
        if is_fixed(left.type_name)
            && matches!(
                operation,
                BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder
            )
        {
            return self.fixed_binary(operation, left, right, span);
        }
        let result_type = if comparison {
            TypeName::Bool
        } else {
            left.type_name
        };
        let result = self.value(result_type);
        let op = match operation {
            BinaryOp::Add if is_float(left.type_name) => "fadd",
            BinaryOp::Subtract if is_float(left.type_name) => "fsub",
            BinaryOp::Multiply if is_float(left.type_name) => "fmul",
            BinaryOp::Divide if is_float(left.type_name) => "fdiv",
            BinaryOp::Remainder if is_float(left.type_name) => {
                return Err(Diagnostic::new(span, "'%' is not defined for floats"))
            }
            BinaryOp::Add => "add",
            BinaryOp::Subtract => "sub",
            BinaryOp::Multiply => "mul",
            BinaryOp::Divide if is_unsigned(left.type_name) => "udiv",
            BinaryOp::Divide => "div",
            BinaryOp::Remainder if is_unsigned(left.type_name) => "urem",
            BinaryOp::Remainder => "rem",
            BinaryOp::Equal => "eq",
            BinaryOp::NotEqual => "ne",
            BinaryOp::Less if is_unsigned(left.type_name) => "below",
            BinaryOp::LessEqual if is_unsigned(left.type_name) => "beloweq",
            BinaryOp::Greater if is_unsigned(left.type_name) => "above",
            BinaryOp::GreaterEqual if is_unsigned(left.type_name) => "aboveeq",
            BinaryOp::Less => "lt",
            BinaryOp::LessEqual => "le",
            BinaryOp::Greater => "gt",
            BinaryOp::GreaterEqual => "ge",
            BinaryOp::BitAnd => "and",
            BinaryOp::BitOr => "or",
            BinaryOp::BitXor => "xor",
            BinaryOp::ShiftLeft => "shl",
            BinaryOp::ShiftRight if is_unsigned(left.type_name) => "shr",
            BinaryOp::ShiftRight => "sar",
            BinaryOp::Is | BinaryOp::IsNot => unreachable!("identity handled above"),
            BinaryOp::And | BinaryOp::Or => {
                return Err(Diagnostic::new(
                    span,
                    "'and' and 'or' have no compound assignment",
                ))
            }
        };
        self.emit(
            op,
            vec![result],
            vec![required(left, span)?, required(right, span)?],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: result_type,
        })
    }

    fn fixed_binary(
        &mut self,
        operation: BinaryOp,
        left: TypedOperand,
        right: TypedOperand,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let fixed_type = left.type_name;
        let TypeName::Fixed {
            storage, fraction, ..
        } = fixed_type
        else {
            unreachable!("fixed arithmetic requires a fixed type")
        };
        if operation == BinaryOp::Remainder {
            return Err(Diagnostic::new(
                span,
                "'%' is not defined for fixed-point values",
            ));
        }
        // Keep fixed i32 arithmetic intact through HIR.  Its widened
        // intermediate is a machine operand pair, not a first-class i64:
        // expanding it here made the generic int64 legalizer select complete
        // 64x64 multiply and 64/64 divide helpers for a 32-bit stored value.
        // Target lowering can instead use the native 32x32->64 product and
        // EDX:EAX dividend while preserving the language's wrapping result.
        if storage == FixedStorage::I32 {
            let result = self.value(fixed_type);
            let operation = match operation {
                BinaryOp::Multiply => "fixed_mul",
                BinaryOp::Divide => "fixed_div",
                _ => unreachable!("only scaling fixed operations reach this helper"),
            };
            self.emit(
                operation,
                vec![result],
                vec![
                    required(left, span)?,
                    required(right, span)?,
                    hir::Operand::Constant(U8, i64::from(fraction)),
                ],
                None,
            );
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Value(result)),
                type_name: fixed_type,
            });
        }
        let wide_type = match storage {
            FixedStorage::I16 => TypeName::I32,
            FixedStorage::I32 => TypeName::I64,
        };
        let left = self.convert_value(left, wide_type, span)?;
        let right = self.convert_value(right, wide_type, span)?;
        let left = required(left, span)?;
        let right = required(right, span)?;
        let adjusted = self.value(wide_type);
        match operation {
            BinaryOp::Multiply => {
                let product = self.value(wide_type);
                self.emit("mul", vec![product], vec![left, right], None);
                self.emit(
                    "sar",
                    vec![adjusted],
                    vec![
                        hir::Operand::Value(product),
                        hir::Operand::Constant(U8, i64::from(fraction)),
                    ],
                    None,
                );
            }
            BinaryOp::Divide => {
                let numerator = self.value(wide_type);
                self.emit(
                    "shl",
                    vec![numerator],
                    vec![left, hir::Operand::Constant(U8, i64::from(fraction))],
                    None,
                );
                self.emit(
                    "div",
                    vec![adjusted],
                    vec![hir::Operand::Value(numerator), right],
                    None,
                );
            }
            _ => unreachable!("only scaling fixed operations reach this helper"),
        }
        self.convert_value(
            TypedOperand {
                operand: Some(hir::Operand::Value(adjusted)),
                type_name: wide_type,
            },
            fixed_type,
            span,
        )
    }

    fn convert_value(
        &mut self,
        value: TypedOperand,
        target: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let result = self.value(target);
        self.emit("convert", vec![result], vec![required(value, span)?], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: target,
        })
    }

    fn identity(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let (left_place, left_index) = self.view_identity(left)?;
        let (right_place, right_index) = self.view_identity(right)?;
        if left_place != right_place {
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Constant(
                    BOOL,
                    if operation == BinaryOp::IsNot { -1 } else { 0 },
                )),
                type_name: TypeName::Bool,
            });
        }
        let result = self.value(TypeName::Bool);
        self.emit(
            if operation == BinaryOp::Is {
                "eq"
            } else {
                "ne"
            },
            vec![result],
            vec![left_index, right_index],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: TypeName::Bool,
        })
    }

    fn view_identity(&self, expression: &Expr) -> Result<(u32, hir::Operand), Diagnostic> {
        let Expr::Name(name, span) = expression else {
            return Err(Diagnostic::new(
                expression.span(),
                "'is' compares scoped struct views",
            ));
        };
        let binding = self.binding(name, *span)?;
        if !matches!(binding.type_, BindingType::Struct(_)) {
            return Err(Diagnostic::new(
                *span,
                "'is' requires struct views, not scalar values",
            ));
        }
        let Storage::ArrayView { place, index } = &binding.storage else {
            return Err(Diagnostic::new(
                *span,
                "struct value has no reference identity",
            ));
        };
        Ok((*place, index.clone()))
    }

    fn member_type_hint(&self, base: &Expr, field: &str, span: Span) -> Option<TypeName> {
        let struct_id = self.struct_type_hint(base, span)?;
        let field = self.types.structure(struct_id)?.fields.get(field)?;
        match field.type_ {
            ElementType::Scalar(type_name) => Some(type_name),
            ElementType::Struct(_) => None,
        }
    }

    fn struct_type_hint(&self, expression: &Expr, span: Span) -> Option<u32> {
        match expression {
            Expr::Name(name, _) => match self.binding(name, span).ok()?.type_ {
                BindingType::Struct(id) => id,
                _ => return None,
            },
            Expr::Index { base, .. } => {
                let Expr::Name(name, _) = base.as_ref() else {
                    return None;
                };
                match self.binding(name, span).ok()?.type_ {
                    BindingType::Array {
                        element: ElementType::Struct(id),
                        ..
                    }
                    | BindingType::Slice {
                        element: ElementType::Struct(id),
                    } => id,
                    _ => return None,
                }
            }
            Expr::Member { base, field, .. } => {
                let parent = self.struct_type_hint(base, span)?;
                match self.types.structure(parent)?.fields.get(field)?.type_ {
                    ElementType::Struct(id) => id,
                    ElementType::Scalar(_) => return None,
                }
            }
            _ => return None,
        }
        .into()
    }

    fn expression_type_hint(&self, expression: &Expr) -> Option<TypeName> {
        match expression {
            Expr::Float(..) => Some(TypeName::F64),
            Expr::Character(..) => Some(TypeName::Char),
            Expr::String(..) => Some(TypeName::String),
            Expr::Boolean(..) => Some(TypeName::Bool),
            Expr::Name(name, _) => {
                self.binding(name, expression.span())
                    .ok()
                    .and_then(|one| match one.type_ {
                        BindingType::Scalar(type_name) => Some(type_name),
                        BindingType::Array { .. }
                        | BindingType::Slice { .. }
                        | BindingType::Struct(_)
                        | BindingType::Dictionary { .. } => None,
                    })
            }
            Expr::Call { name, .. } => self.signatures.get(name).map(|one| one.result),
            Expr::MethodCall { .. } => Some(TypeName::U16),
            Expr::Unary { operand, .. } => self.expression_type_hint(operand),
            Expr::Conversion { target, .. } => Some(*target),
            Expr::Index { base, .. } => {
                let Expr::Name(name, _) = base.as_ref() else {
                    return None;
                };
                self.binding(name, expression.span())
                    .ok()
                    .and_then(|one| match one.type_ {
                        BindingType::Array {
                            element: ElementType::Scalar(element),
                            ..
                        }
                        | BindingType::Slice {
                            element: ElementType::Scalar(element),
                        } => Some(element),
                        BindingType::Array {
                            element: ElementType::Struct(_),
                            ..
                        }
                        | BindingType::Slice {
                            element: ElementType::Struct(_),
                        }
                        | BindingType::Scalar(_)
                        | BindingType::Struct(_)
                        | BindingType::Dictionary { .. } => None,
                    })
            }
            Expr::Member { base, field, span } => self.member_type_hint(base, field, *span),
            Expr::Binary {
                op, left, right, ..
            } => {
                if matches!(
                    op,
                    BinaryOp::Equal
                        | BinaryOp::NotEqual
                        | BinaryOp::Less
                        | BinaryOp::LessEqual
                        | BinaryOp::Greater
                        | BinaryOp::GreaterEqual
                        | BinaryOp::Is
                        | BinaryOp::IsNot
                        | BinaryOp::And
                        | BinaryOp::Or
                ) {
                    return Some(TypeName::Bool);
                }
                if is_shift(*op) {
                    return self.expression_type_hint(left);
                }
                match (
                    self.expression_type_hint(left),
                    self.expression_type_hint(right),
                ) {
                    (Some(left), Some(right)) if left == right => Some(left),
                    (Some(type_name), None) | (None, Some(type_name)) => Some(type_name),
                    (Some(_), Some(_)) | (None, None) => None,
                }
            }
            Expr::Integer(..)
            | Expr::FString { .. }
            | Expr::Array(..)
            | Expr::Repeat { .. }
            | Expr::Comprehension { .. }
            | Expr::Generator { .. }
            | Expr::DictComprehension { .. }
            | Expr::Slice { .. }
            | Expr::StructLiteral { .. }
            | Expr::Borrow { .. } => None,
        }
    }

    fn array_method(
        &mut self,
        receiver: &Expr,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let Expr::Name(array_name, receiver_span) = receiver else {
            return Err(Diagnostic::new(
                receiver.span(),
                "array methods currently require a named array",
            ));
        };
        let binding = self.binding(array_name, *receiver_span)?.clone();
        if let BindingType::Dictionary {
            key,
            value,
            capacity,
        } = binding.type_
        {
            return self.dictionary_method(
                &binding, key, value, capacity, name, arguments, expected, span,
            );
        }
        let string = binding.type_ == BindingType::Scalar(TypeName::String);
        let length = if string {
            None
        } else {
            let Some((_element, length)) = binding.type_.array() else {
                return Err(Diagnostic::new(
                    receiver.span(),
                    format!("{array_name:?} is not an array or string"),
                ));
            };
            length
        };
        if name == "data" {
            if !arguments.is_empty() {
                return Err(Diagnostic::new(span, "data() takes no arguments"));
            }
            if string {
                if expected.is_some_and(|one| one != TypeName::String) {
                    return Err(type_mismatch(
                        span,
                        expected.expect("checked"),
                        TypeName::String,
                    ));
                }
                let pointer = self.string_pointer(&binding, receiver.span())?;
                return Ok(TypedOperand {
                    operand: Some(hir::Operand::Value(pointer)),
                    type_name: TypeName::String,
                });
            }
            if expected.is_some_and(|one| one != TypeName::Addr) {
                return Err(type_mismatch(
                    span,
                    expected.expect("checked"),
                    TypeName::Addr,
                ));
            }
            let pointer = match binding.storage {
                Storage::Place(place) => {
                    let result = self.value(TypeName::Addr);
                    self.emit(
                        "address",
                        vec![result],
                        vec![hir::Operand::Place(place)],
                        None,
                    );
                    result
                }
                Storage::Slice(descriptor) => {
                    let BindingType::Slice { element } = binding.type_ else {
                        unreachable!("slice storage has slice type")
                    };
                    let typed = self.slice_data_pointer(descriptor, element);
                    let result = self.value(TypeName::Addr);
                    self.emit("copy", vec![result], vec![hir::Operand::Value(typed)], None);
                    result
                }
                _ => return Err(Diagnostic::new(span, "sequence has no data pointer")),
            };
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Value(pointer)),
                type_name: TypeName::Addr,
            });
        }
        if expected.is_some_and(|one| one != TypeName::U16) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::U16,
            ));
        }
        let offset = match name {
            "len" if arguments.is_empty() => 0,
            "capacity" if arguments.is_empty() => 2,
            "dim" if arguments.len() == 1 => {
                let Expr::Integer(axis, axis_span) = arguments[0] else {
                    return Err(Diagnostic::new(
                        arguments[0].span(),
                        "dimension index must be an integer literal",
                    ));
                };
                if axis != 0 {
                    return Err(Diagnostic::new(
                        axis_span,
                        "one-dimensional array has only dimension 0",
                    ));
                }
                0
            }
            "len" | "capacity" => {
                return Err(Diagnostic::new(
                    span,
                    format!("{name}() takes no arguments"),
                ))
            }
            "dim" => return Err(Diagnostic::new(span, "dim() takes one dimension index")),
            _ => {
                return Err(Diagnostic::new(
                    span,
                    format!("array has no method {name:?}"),
                ))
            }
        };
        let operand = if let Some(length) = length {
            hir::Operand::Constant(U16, i64::from(length))
        } else {
            let pointer = if string {
                self.string_pointer(&binding, receiver.span())?
            } else if let Storage::Slice(pointer) = binding.storage {
                pointer
            } else {
                return Err(Diagnostic::new(receiver.span(), "slice has no descriptor"));
            };
            let value = self.value(TypeName::U16);
            self.emit(
                "load",
                vec![value],
                vec![hir::Operand::DescriptorPlace {
                    base: pointer,
                    field: if offset == 0 { "length" } else { "capacity" },
                    type_id: U16,
                }],
                None,
            );
            hir::Operand::Value(value)
        };
        Ok(TypedOperand {
            operand: Some(operand),
            type_name: TypeName::U16,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn dictionary_method(
        &mut self,
        binding: &Binding,
        key_type: TypeName,
        value_type: TypeName,
        capacity: u32,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let Storage::Dictionary {
            keys,
            values,
            length,
        } = binding.storage
        else {
            unreachable!("dictionary type has dictionary storage")
        };
        if matches!(name, "len" | "capacity") {
            if !arguments.is_empty() {
                return Err(Diagnostic::new(
                    span,
                    format!("{name}() takes no arguments"),
                ));
            }
            if expected.is_some_and(|one| one != TypeName::U16) {
                return Err(type_mismatch(
                    span,
                    expected.expect("checked"),
                    TypeName::U16,
                ));
            }
            let operand = if name == "capacity" {
                hir::Operand::Constant(U16, i64::from(capacity))
            } else {
                let result = self.value(TypeName::U16);
                self.emit(
                    "load",
                    vec![result],
                    vec![hir::Operand::Place(length)],
                    None,
                );
                hir::Operand::Value(result)
            };
            return Ok(TypedOperand {
                operand: Some(operand),
                type_name: TypeName::U16,
            });
        }
        if name != "get" {
            return Err(Diagnostic::new(
                span,
                format!("dictionary has no method {name:?}"),
            ));
        }
        if arguments.len() != 2 {
            return Err(Diagnostic::new(span, "get() takes a key and default value"));
        }
        if expected.is_some_and(|one| one != value_type) {
            return Err(type_mismatch(span, expected.expect("checked"), value_type));
        }
        let wanted = required(
            self.expression(&arguments[0], Some(key_type))?,
            arguments[0].span(),
        )?;
        let fallback = required(
            self.expression(&arguments[1], Some(value_type))?,
            arguments[1].span(),
        )?;
        let result_place = self.place("$dict_get_result", value_type, true);
        let index_place = self.place("$dict_get_index", TypeName::U16, true);
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(result_place), fallback],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(index_place),
                hir::Operand::Constant(U16, 0),
            ],
            None,
        );
        let condition = self.block();
        let body = self.block();
        let found = self.block();
        let increment = self.block();
        let exit = self.block();
        self.terminate(jump(condition));

        self.current = condition;
        let index = self.value(TypeName::U16);
        let length_value = self.value(TypeName::U16);
        self.emit(
            "load",
            vec![index],
            vec![hir::Operand::Place(index_place)],
            None,
        );
        self.emit(
            "load",
            vec![length_value],
            vec![hir::Operand::Place(length)],
            None,
        );
        let in_range = self.value(TypeName::Bool);
        self.emit(
            "below",
            vec![in_range],
            vec![
                hir::Operand::Value(index),
                hir::Operand::Value(length_value),
            ],
            None,
        );
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(in_range)],
            targets: vec![body, exit],
        });

        self.current = body;
        let actual = self.value(key_type);
        self.emit(
            "load",
            vec![actual],
            vec![hir::Operand::ArrayElement(
                keys,
                vec![hir::Operand::Value(index)],
            )],
            None,
        );
        let equal = self.value(TypeName::Bool);
        self.emit(
            "eq",
            vec![equal],
            vec![hir::Operand::Value(actual), wanted],
            None,
        );
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(equal)],
            targets: vec![found, increment],
        });

        self.current = found;
        let selected = self.value(value_type);
        self.emit(
            "load",
            vec![selected],
            vec![hir::Operand::ArrayElement(
                values,
                vec![hir::Operand::Value(index)],
            )],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(result_place),
                hir::Operand::Value(selected),
            ],
            None,
        );
        self.terminate(jump(exit));

        self.current = increment;
        let next = self.value(TypeName::U16);
        self.emit(
            "add",
            vec![next],
            vec![hir::Operand::Value(index), hir::Operand::Constant(U16, 1)],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(index_place), hir::Operand::Value(next)],
            None,
        );
        self.terminate(jump(condition));

        self.current = exit;
        let result = self.value(value_type);
        self.emit(
            "load",
            vec![result],
            vec![hir::Operand::Place(result_place)],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: value_type,
        })
    }

    fn call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if name == "print" {
            return self.print(arguments, expected, span);
        }
        let signature = self
            .signatures
            .get(name)
            .cloned()
            .ok_or_else(|| Diagnostic::new(span, format!("unknown function {name:?}")))?;
        if signature.parameters.len() != arguments.len() {
            return Err(Diagnostic::new(
                span,
                format!(
                    "function {name:?} expects {} arguments, got {}",
                    signature.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        if expected.is_some_and(|one| one != signature.result) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                signature.result,
            ));
        }
        let mut operands = Vec::new();
        let mut borrowed = BTreeMap::new();
        for (argument, parameter) in arguments.iter().zip(&signature.parameters) {
            match parameter {
                SignatureParameter::Scalar(type_name) => {
                    let value = self.expression(argument, Some(*type_name))?;
                    operands.push(required(value, argument.span())?);
                }
                SignatureParameter::Borrowed {
                    mutable,
                    target,
                    pointer,
                } => {
                    let (operand, owner) =
                        self.borrow_argument(argument, *mutable, *target, *pointer)?;
                    if let Some(previously_mutable) = borrowed.insert(owner.clone(), *mutable) {
                        if *mutable || previously_mutable {
                            return Err(Diagnostic::new(
                                argument.span(),
                                format!("borrow of {owner:?} aliases a mutable argument"),
                            ));
                        }
                    }
                    operands.push(operand);
                }
            }
        }
        let results = if signature.result == TypeName::Void {
            Vec::new()
        } else {
            vec![self.value(signature.result)]
        };
        let instruction = self.emit(
            "call",
            results.clone(),
            operands,
            Some(signature.name.clone()),
        );
        self.calls.push(hir::CallSite {
            instruction,
            order: (0..arguments.len() as u32).rev().collect(),
            callee: signature.id,
        });
        Ok(TypedOperand {
            operand: results.first().copied().map(hir::Operand::Value),
            type_name: signature.result,
        })
    }

    fn borrow_argument(
        &mut self,
        argument: &Expr,
        required_mutable: bool,
        target: BindingType,
        pointer_type: u32,
    ) -> Result<(hir::Operand, String), Diagnostic> {
        let Expr::Borrow {
            mutable,
            operand,
            span,
        } = argument
        else {
            return Err(Diagnostic::new(
                argument.span(),
                "borrowed parameter requires an explicit '&' argument",
            ));
        };
        if required_mutable && !mutable {
            return Err(Diagnostic::new(*span, "mutable parameter requires '&mut'"));
        }
        let (name, name_span, range) = match operand.as_ref() {
            Expr::Name(name, name_span) => (name, *name_span, None),
            Expr::Slice {
                base,
                start,
                end,
                span: range_span,
            } => {
                let Expr::Name(name, name_span) = base.as_ref() else {
                    return Err(Diagnostic::new(
                        base.span(),
                        "slice base must be a named array",
                    ));
                };
                (
                    name,
                    *name_span,
                    Some((start.as_deref(), end.as_deref(), *range_span)),
                )
            }
            _ => {
                return Err(Diagnostic::new(
                    operand.span(),
                    "borrow currently requires a named binding or array range",
                ))
            }
        };
        let binding = self.binding(name, name_span)?.clone();
        let compatible = binding.type_ == target
            || matches!(
                (binding.type_, target),
                (
                    BindingType::Array { element: actual, .. },
                    BindingType::Slice { element: expected }
                ) if actual == expected
            );
        if !compatible {
            return Err(Diagnostic::new(
                *span,
                format!("borrow of {name:?} has the wrong type"),
            ));
        }
        if *mutable && !binding.mutable {
            return Err(Diagnostic::new(
                *span,
                format!("cannot mutably borrow immutable binding {name:?}"),
            ));
        }
        if let BindingType::Slice { element } = target {
            if range.is_none() {
                if let Storage::Slice(pointer) = binding.storage {
                    return Ok((hir::Operand::Value(pointer), name.clone()));
                }
            }
            let BindingType::Array {
                element: actual,
                length,
            } = binding.type_
            else {
                return Err(Diagnostic::new(
                    operand.span(),
                    "a ranged borrow currently requires a fixed array",
                ));
            };
            debug_assert_eq!(actual, element);
            let (start, end) = self.slice_bounds(range, length, operand.span())?;
            let Storage::Place(place) = binding.storage else {
                return Err(Diagnostic::new(
                    operand.span(),
                    "array has no owned payload",
                ));
            };
            let data_type = self.types.pointer(element.id(), 0);
            let data = self.value_type(data_type);
            self.emit(
                "address",
                vec![data],
                vec![hir::Operand::Place(place)],
                None,
            );
            let data = if start == 0 {
                data
            } else {
                self.indexed_pointer(
                    data,
                    hir::Operand::Constant(U16, i64::from(start)),
                    self.types.width(element.id()),
                    operand.span(),
                )?
            };
            let descriptor_type = self
                .types
                .types
                .iter()
                .find(|one| one.id == pointer_type)
                .and_then(|one| one.element)
                .expect("slice pointer has a descriptor pointee");
            let descriptor = self.local_place(&format!("$slice_{name}"), descriptor_type, 8, false);
            let view_length = end - start;
            for (offset, value) in [(0, view_length), (2, view_length)] {
                self.emit(
                    "store",
                    Vec::new(),
                    vec![
                        hir::Operand::ProjectedPlace {
                            place: descriptor,
                            indices: Vec::new(),
                            offset,
                            type_id: U16,
                        },
                        hir::Operand::Constant(U16, i64::from(value)),
                    ],
                    None,
                );
            }
            self.emit(
                "store",
                Vec::new(),
                vec![
                    hir::Operand::ProjectedPlace {
                        place: descriptor,
                        indices: Vec::new(),
                        offset: 4,
                        type_id: data_type,
                    },
                    hir::Operand::Value(data),
                ],
                None,
            );
            let result = self.value_type(pointer_type);
            self.emit(
                "address",
                vec![result],
                vec![hir::Operand::Place(descriptor)],
                None,
            );
            return Ok((hir::Operand::Value(result), name.clone()));
        }
        if range.is_some() {
            return Err(Diagnostic::new(
                operand.span(),
                "a range can only be borrowed as a slice",
            ));
        }
        let place = match binding.storage {
            Storage::Place(place) => hir::Operand::Place(place),
            Storage::ArrayView { place, index } => hir::Operand::ArrayElement(place, vec![index]),
            Storage::Reference(pointer) => return Ok((hir::Operand::Value(pointer), name.clone())),
            Storage::Slice(_) => unreachable!("slice target handled above"),
            Storage::Dictionary { .. } => unreachable!("borrow target is not a dictionary"),
            Storage::Parameter(_) => {
                return Err(Diagnostic::new(
                    *span,
                    "a by-value parameter has no borrowable storage",
                ))
            }
        };
        let result = self.value_type(pointer_type);
        self.emit("address", vec![result], vec![place], None);
        Ok((hir::Operand::Value(result), name.clone()))
    }

    fn slice_bounds(
        &self,
        range: Option<(Option<&Expr>, Option<&Expr>, Span)>,
        length: u32,
        _span: Span,
    ) -> Result<(u32, u32), Diagnostic> {
        let Some((start, end, range_span)) = range else {
            return Ok((0, length));
        };
        let endpoint = |value: Option<&Expr>, default: u32| -> Result<u32, Diagnostic> {
            let Some(value) = value else {
                return Ok(default);
            };
            match value {
                Expr::Integer(value, at) => u32::try_from(*value)
                    .map_err(|_| Diagnostic::new(*at, "slice bounds must be non-negative")),
                _ => Err(Diagnostic::new(
                    value.span(),
                    "this slice requires compile-time integer bounds",
                )),
            }
        };
        let start = endpoint(start, 0)?;
        let end = endpoint(end, length)?;
        if start > end || end > length {
            return Err(Diagnostic::new(
                range_span,
                format!("slice {start}..{end} is outside 0..{length}"),
            ));
        }
        Ok((start, end))
    }

    fn print(
        &mut self,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Void) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Void,
            ));
        }
        for argument in arguments {
            if let Expr::FString { parts, .. } = argument {
                for part in parts {
                    match part {
                        FStringPart::Text(bytes) if !bytes.is_empty() => {
                            let text = self.string_literal(
                                bytes,
                                Some(TypeName::String),
                                argument.span(),
                            )?;
                            self.emit_print(TypeName::String, required(text, argument.span())?);
                        }
                        FStringPart::Text(_) => {}
                        FStringPart::Value(expression) => {
                            let value = self.expression(expression, None)?;
                            if value.type_name == TypeName::Void {
                                return Err(Diagnostic::new(
                                    expression.span(),
                                    "cannot format void",
                                ));
                            }
                            let type_name = value.type_name;
                            self.emit_print(type_name, required(value, expression.span())?);
                        }
                    }
                }
            } else {
                let value = self.expression(argument, None)?;
                if value.type_name == TypeName::Void {
                    return Err(Diagnostic::new(argument.span(), "cannot print void"));
                }
                let type_name = value.type_name;
                self.emit_print(type_name, required(value, argument.span())?);
            }
        }
        self.emit_builtin("_pn", Vec::new());
        Ok(TypedOperand {
            operand: None,
            type_name: TypeName::Void,
        })
    }

    fn emit_print(&mut self, type_name: TypeName, operand: hir::Operand) {
        if let TypeName::Fixed {
            storage, fraction, ..
        } = type_name
        {
            let storage_type = match storage {
                FixedStorage::I16 => TypeName::I16,
                FixedStorage::I32 => TypeName::I32,
            };
            let raw = self.value(storage_type);
            self.emit("convert", vec![raw], vec![operand], None);
            self.emit_builtin(
                print_name(type_name),
                vec![
                    hir::Operand::Value(raw),
                    hir::Operand::Constant(U8, i64::from(fraction)),
                ],
            );
            return;
        }
        self.emit_builtin(print_name(type_name), vec![operand]);
    }

    fn emit_builtin(&mut self, name: &'static str, operands: Vec<hir::Operand>) {
        let count = operands.len();
        let instruction = self.emit("call", Vec::new(), operands, Some(name.into()));
        self.calls.push(hir::CallSite {
            instruction,
            order: (0..count as u32).rev().collect(),
            callee: *self
                .builtin_ids
                .get(name)
                .expect("registered print builtin"),
        });
    }

    fn binding(&self, name: &str, span: Span) -> Result<&Binding, Diagnostic> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .ok_or_else(|| Diagnostic::new(span, format!("unknown name {name:?}")))
    }

    fn value(&mut self, type_name: TypeName) -> u32 {
        self.value_type(type_id(type_name))
    }

    fn value_type(&mut self, type_id: u32) -> u32 {
        let id = self.next_value;
        self.next_value += 1;
        self.values.push(hir::Value { id, type_id });
        id
    }

    fn place(&mut self, name: &str, type_name: TypeName, mutable: bool) -> u32 {
        self.local_place(name, type_id(type_name), width(type_name), mutable)
    }

    fn local_place(&mut self, name: &str, type_id: u32, extent: u32, mutable: bool) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        self.next_frame_offset -= extent as i32;
        self.places.push(hir::Place {
            id,
            name: name.into(),
            type_id,
            mutable,
            offset: self.next_frame_offset,
            extent,
            storage: "local",
            symbol: 0,
            volatile: false,
        });
        id
    }

    fn array_place(
        &mut self,
        name: &str,
        type_id: u32,
        element: ElementType,
        length: u32,
        mutable: bool,
    ) -> u32 {
        let extent = self.types.width(element.id()) * length;
        self.next_frame_offset -= extent as i32 + 4;
        let descriptor_offset = self.next_frame_offset;

        let length_place = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id: length_place,
            name: format!("${name}.length"),
            type_id: U16,
            mutable: false,
            offset: descriptor_offset,
            extent: 2,
            storage: "local",
            symbol: 0,
            volatile: true,
        });
        let capacity_place = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id: capacity_place,
            name: format!("${name}.capacity"),
            type_id: U16,
            mutable: false,
            offset: descriptor_offset + 2,
            extent: 2,
            storage: "local",
            symbol: 0,
            volatile: true,
        });
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id,
            name: name.into(),
            type_id,
            mutable,
            offset: descriptor_offset + 4,
            extent,
            storage: "local",
            symbol: 0,
            volatile: false,
        });
        for descriptor in [length_place, capacity_place] {
            self.emit(
                "store",
                Vec::new(),
                vec![
                    hir::Operand::Place(descriptor),
                    hir::Operand::Constant(U16, i64::from(length)),
                ],
                None,
            );
        }
        id
    }

    fn static_place(&mut self, symbol: u32, type_name: TypeName) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id,
            name: format!("$literal{symbol}"),
            type_id: type_id(type_name),
            mutable: false,
            offset: 0,
            extent: width(type_name),
            storage: "module",
            symbol,
            volatile: false,
        });
        id
    }

    fn static_string_place(&mut self, symbol: u32, extent: u32) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id,
            name: format!("$string{symbol}"),
            type_id: CHAR,
            mutable: false,
            // The exported string address is the byte payload. Its length and
            // capacity words occupy the four bytes immediately before it.
            offset: 4,
            extent,
            storage: "module",
            symbol,
            volatile: false,
        });
        id
    }

    fn emit(
        &mut self,
        op: &'static str,
        results: Vec<u32>,
        operands: Vec<hir::Operand>,
        callee: Option<String>,
    ) -> u32 {
        let id = self.next_instruction;
        self.next_instruction += 1;
        self.current_block_mut()
            .instructions
            .push(hir::Instruction {
                id,
                op,
                results,
                operands,
                callee,
            });
        id
    }

    fn block(&mut self) -> u32 {
        let id = self.blocks.len() as u32 + 1;
        self.blocks.push(BlockBuilder {
            id,
            instructions: Vec::new(),
            terminator: None,
        });
        id
    }

    fn terminate(&mut self, terminator: hir::Terminator) {
        let block = self.current_block_mut();
        assert!(
            block.terminator.is_none(),
            "semantic block terminated twice"
        );
        block.terminator = Some(terminator);
    }

    fn open(&self) -> bool {
        self.blocks[(self.current - 1) as usize]
            .terminator
            .is_none()
    }

    fn current_block_mut(&mut self) -> &mut BlockBuilder {
        &mut self.blocks[(self.current - 1) as usize]
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

fn is_integer(type_name: TypeName) -> bool {
    matches!(
        type_name,
        TypeName::I8 | TypeName::U8 | TypeName::I16 | TypeName::U16 | TypeName::I32 | TypeName::U32
    )
}

fn is_signed(type_name: TypeName) -> bool {
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

fn is_float(type_name: TypeName) -> bool {
    matches!(type_name, TypeName::F32 | TypeName::F64)
}

fn is_numeric(type_name: TypeName) -> bool {
    is_integer(type_name) || is_float(type_name) || is_fixed(type_name)
}

fn is_ordered(type_name: TypeName) -> bool {
    is_numeric(type_name) || type_name == TypeName::Char
}

/// A repeat literal's element count; only rank one is implemented.
fn repeat_count(counts: &[Expr]) -> Result<usize, Diagnostic> {
    match counts {
        [Expr::Integer(count, at)] => usize::try_from(*count)
            .map_err(|_| Diagnostic::new(*at, "a repeat count must be non-negative")),
        [count] => Err(Diagnostic::new(
            count.span(),
            "a repeat count must be a compile-time integer",
        )),
        [_, second, ..] => Err(Diagnostic::new(
            second.span(),
            "only rank-one repeat literals are implemented",
        )),
        [] => unreachable!("the parser requires a count"),
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
    is_shift(operation) || matches!(operation, BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor)
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
    }
}

fn width(type_name: TypeName) -> u32 {
    match type_name {
        TypeName::Void => 0,
        TypeName::Bool | TypeName::Char | TypeName::I8 | TypeName::U8 => 1,
        TypeName::I16 | TypeName::U16 => 2,
        TypeName::I32 | TypeName::U32 | TypeName::F32 => 4,
        TypeName::F64 => 8,
        TypeName::String => 2,
        TypeName::Addr => 4,
        TypeName::I64 => 8,
        TypeName::Fixed { storage, .. } => match storage {
            FixedStorage::I16 => 2,
            FixedStorage::I32 => 4,
        },
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
mod tests {
    use crate::lexer::lex;
    use crate::parser::parse;

    use super::*;

    fn compile_source(source: &str) -> Result<String, Diagnostic> {
        let module = parse(lex(source)?)?;
        compile(&module, "test")
    }

    #[test]
    fn emits_typed_cfg_for_loop_and_call() {
        let json = compile_source(
            "fn step(value: i16) -> i16:\n\
             \x20\x20\x20\x20return value + 1\n\
             fn count(limit: i16) -> i16:\n\
             \x20\x20\x20\x20var value: i16 = 0\n\
             \x20\x20\x20\x20while value < limit:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20value = step(value)\n\
             \x20\x20\x20\x20return value\n",
        )
        .unwrap();
        assert!(json.contains("\"dialect\":\"modern\""));
        assert!(json.contains("\"op\":\"call\""));
        assert!(json.contains("\"kind\":\"branch\""));
        assert!(json.contains("\"storage\":\"local\""));
    }

    #[test]
    fn rejects_assignment_to_let() {
        let error = compile_source(
            "fn bad() -> i16:\n\
             \x20\x20\x20\x20let value = 1\n\
             \x20\x20\x20\x20value = 2\n\
             \x20\x20\x20\x20return value\n",
        )
        .unwrap_err();
        assert!(error.message.contains("immutable"));
    }

    #[test]
    fn rejects_non_boolean_condition() {
        let error = compile_source(
            "fn bad(value: i16) -> i16:\n\
             \x20\x20\x20\x20if value:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20return 1\n\
             \x20\x20\x20\x20return 0\n",
        )
        .unwrap_err();
        assert!(error.message.contains("expected bool"));
    }

    #[test]
    fn accepts_the_i16_minimum_literal() {
        let json = compile_source(
            "fn minimum() -> i16:\n\
             \x20\x20\x20\x20return -32768\n",
        )
        .unwrap();
        assert!(json.contains("\"value\":-32768"));
    }

    #[test]
    fn integer_literals_are_checked_against_their_primitive_width() {
        let error = compile_source(
            "fn too_large() -> u8:\n\
             \x20\x20\x20\x20return 256\n",
        )
        .unwrap_err();
        assert!(error.message.contains("does not fit u8"));

        let error = compile_source(
            "fn negative() -> u32:\n\
             \x20\x20\x20\x20return -1\n",
        )
        .unwrap_err();
        assert!(error.message.contains("does not fit u32"));
    }

    #[test]
    fn unsigned_and_float_operators_emit_distinct_hir_operations() {
        let json = compile_source(
            "fn quotient(a: u32, b: u32) -> u32:\n\
             \x20\x20\x20\x20return a / b\n\
             fn less(a: u16, b: u16) -> bool:\n\
             \x20\x20\x20\x20return a < b\n\
             fn product(a: f32, b: f32) -> f32:\n\
             \x20\x20\x20\x20return a * b\n",
        )
        .unwrap();
        assert!(json.contains("\"op\":\"udiv\""));
        assert!(json.contains("\"op\":\"below\""));
        assert!(json.contains("\"op\":\"fmul\""));
    }

    #[test]
    fn arrays_and_f_strings_lower_to_structural_hir_and_streaming_calls() {
        let json = compile_source(
            "fn show(index: i16) -> void:\n\
             \x20\x20\x20\x20var values: [i32; 2] = [10, 20]\n\
             \x20\x20\x20\x20values[index] = values[index] + 1\n\
             \x20\x20\x20\x20print(f\"value={values[index]}\")\n",
        )
        .unwrap();
        assert!(json.contains("\"kind\":\"array\""));
        assert!(json.contains("\"tag\":\"array_element\""));
        assert!(json.contains("\"callee\":\"_pt\""));
        assert!(json.contains("\"callee\":\"_pi4\""));
        assert!(json.contains("\"callee\":\"_pn\""));
        assert!(json.contains("\"bytes\":[6,0,6,0,118,97,108,117,101,61,0]"));
    }

    #[test]
    fn literal_array_bounds_are_checked_before_hir() {
        let error = compile_source(
            "fn bad() -> i32:\n\
             \x20\x20\x20\x20let values: [i32; 2] = [10, 20]\n\
             \x20\x20\x20\x20return values[2]\n",
        )
        .unwrap_err();
        assert!(error.message.contains("outside 0..2"));
    }

    #[test]
    fn struct_array_for_loop_uses_projected_places_without_iterator_calls() {
        let json = compile_source(
            "struct pair:\n\
             \x20\x20\x20\x20left: i32\n\
             \x20\x20\x20\x20right: i32\n\
             fn bump() -> i32:\n\
             \x20\x20\x20\x20var pairs: [pair; 2] = [pair { left: 1, right: 2 }, pair { left: 3, right: 4 }]\n\
             \x20\x20\x20\x20for item in &mut pairs:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20item.left = item.left + 1\n\
             \x20\x20\x20\x20return pairs[1].left\n",
        )
        .unwrap();
        assert!(json.contains("\"kind\":\"opaque\",\"name\":\"pair\""));
        assert!(json.contains("\"name\":\"[pair; 2]\""));
        assert!(json.contains("\"tag\":\"projection\""));
        assert!(json.contains("\"op\":\"below\""));
        assert!(!json.contains("\"callee\":\"__iter"));
    }

    #[test]
    fn range_loop_keeps_the_bound_type_and_lowers_without_a_runtime_iterator() {
        let json = compile_source(
            "fn sum(step_count: u16) -> u16:\n\
             \x20\x20\x20\x20var total: u16 = 0\n\
             \x20\x20\x20\x20for step_no in 0..step_count - 1:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20total = total + step_no\n\
             \x20\x20\x20\x20return total\n",
        )
        .unwrap();
        let range_at = json.find("\"name\":\"$range_step_no\"").unwrap();
        let range_place = &json[range_at..range_at + json[range_at..].find('}').unwrap()];
        assert!(range_place.contains(&format!("\"type\":{}", type_id(TypeName::U16))));
        assert!(json.contains("\"op\":\"below\""));
        assert!(json.contains("\"op\":\"add\""));
        assert!(!json.contains("\"callee\":\"__iter"));
    }

    #[test]
    fn range_loop_requires_integer_bounds() {
        let error = compile_source(
            "fn bad(limit: f32) -> void:\n\
             \x20\x20\x20\x20for item in 0..limit:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20print(item)\n",
        )
        .unwrap_err();
        assert!(error.message.contains("range bounds must be integers"));
    }

    #[test]
    fn fixed_point_literals_and_arithmetic_are_scaled_at_compile_time() {
        let json = compile_source(
            "type fixed8 = fixed i16, fraction=8\n\
             type fixed16 = fixed i32, fraction=16\n\
             fn calculate(a: fixed8, b: fixed8) -> fixed8:\n\
             \x20\x20\x20\x20var factor: fixed8 = 1.5\n\
             \x20\x20\x20\x20factor = 2.25\n\
             \x20\x20\x20\x20return 0.5 * a * factor / b\n",
        )
        .unwrap();
        assert!(json.contains("\"kind\":\"integer\",\"name\":\"fixed8\""));
        assert!(json.contains("\"kind\":\"integer\",\"name\":\"fixed16\""));
        assert!(json.contains(&format!("\"type\":{},\"value\":384", FIXED_START)));
        assert!(json.contains(&format!("\"type\":{},\"value\":576", FIXED_START)));
        for operation in ["convert", "mul", "sar", "shl", "div"] {
            assert!(json.contains(&format!("\"op\":\"{operation}\"")));
        }
    }

    #[test]
    fn i32_fixed_arithmetic_stays_out_of_generic_i64_hir() {
        let json = compile_source(
            "type scalar = fixed i32, fraction=9\n\
             fn product(a: scalar, b: scalar) -> scalar:\n\
             \x20\x20\x20\x20return a * b\n\
             fn quotient(a: scalar, b: scalar) -> scalar:\n\
             \x20\x20\x20\x20return a / b\n",
        )
        .unwrap();

        assert!(json.contains("\"op\":\"fixed_mul\""));
        assert!(json.contains("\"op\":\"fixed_div\""));
        for operation in ["convert", "mul", "sar", "shl", "div"] {
            assert!(!json.contains(&format!("\"op\":\"{operation}\"")));
        }
    }

    #[test]
    fn fixed_point_decimal_literals_round_once_and_must_fit_storage() {
        let rounded = compile_source(
            "type fixed8 = fixed i16, fraction=8\n\
             fn tenth() -> fixed8:\n\
             \x20\x20\x20\x20return 0.1\n",
        )
        .unwrap();
        assert!(rounded.contains(&format!("\"type\":{},\"value\":26", FIXED_START)));

        let too_large = compile_source(
            "type fixed8 = fixed i16, fraction=8\n\
             fn bad() -> fixed8:\n\
             \x20\x20\x20\x20return 128\n",
        )
        .unwrap_err();
        assert!(too_large.message.contains("does not fit"));
    }

    #[test]
    fn separately_declared_fixed_point_types_do_not_mix_implicitly() {
        let error = compile_source(
            "type distance = fixed i16, fraction=8\n\
             type duration = fixed i16, fraction=8\n\
             fn bad(left: distance, right: duration) -> distance:\n\
             \x20\x20\x20\x20return left + right\n",
        )
        .unwrap_err();
        assert!(error.message.contains("distinct fixed-point types"));
    }

    #[test]
    fn plain_for_view_is_immutable() {
        let error = compile_source(
            "struct item:\n\
             \x20\x20\x20\x20value: i16\n\
             fn bad() -> void:\n\
             \x20\x20\x20\x20var items: [item; 1] = [item { value: 1 }]\n\
             \x20\x20\x20\x20for one in &items:\n\
             \x20\x20\x20\x20\x20\x20\x20\x20one.value = 2\n",
        )
        .unwrap_err();
        assert!(error.message.contains("immutable"));
    }

    #[test]
    fn local_structs_initialize_copy_and_update_through_projected_places() {
        let json = compile_source(
            "struct point:\n\
             \x20\x20\x20\x20x: i16\n\
             \x20\x20\x20\x20y: i16\n\
             struct body:\n\
             \x20\x20\x20\x20pos: point\n\
             \x20\x20\x20\x20mass: i16\n\
             fn move() -> i16:\n\
             \x20\x20\x20\x20var current: body = body { pos: point { x: 1, y: 2 }, mass: 3 }\n\
             \x20\x20\x20\x20let snapshot = current\n\
             \x20\x20\x20\x20current.pos = point { x: snapshot.pos.y, y: snapshot.pos.x }\n\
             \x20\x20\x20\x20current.mass += 4\n\
             \x20\x20\x20\x20return current.pos.x + current.pos.y + current.mass\n",
        )
        .unwrap();
        assert!(json.contains("\"name\":\"current\""));
        assert!(json.contains("\"name\":\"snapshot\""));
        assert!(json.matches("\"tag\":\"projection\"").count() >= 10);
        assert!(json.contains("\"op\":\"add\""));
    }

    #[test]
    fn compound_assignment_evaluates_an_index_once() {
        let json = compile_source(
            "struct item:\n\
             \x20\x20\x20\x20value: i16\n\
             fn next() -> i16:\n\
             \x20\x20\x20\x20return 0\n\
             fn bump() -> i16:\n\
             \x20\x20\x20\x20var items: [item; 1] = [item { value: 1 }]\n\
             \x20\x20\x20\x20items[next()].value += 2\n\
             \x20\x20\x20\x20return items[0].value\n",
        )
        .unwrap();
        assert_eq!(json.matches("\"callee\":\"next\"").count(), 1);
    }

    #[test]
    fn print_runtime_variants_use_short_byte_width_names() {
        let names: Vec<_> = print_builtins().into_iter().map(|(name, _)| name).collect();
        assert_eq!(
            names,
            [
                "_pn", "_pt", "_pb", "_pc", "_pi1", "_pu1", "_pi2", "_pu2", "_pi4", "_pu4", "_pr4",
                "_pr8", "_pf2", "_pf4",
            ]
        );
    }
}
