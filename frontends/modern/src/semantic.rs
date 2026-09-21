use std::collections::BTreeMap;

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
use crate::syntax::Span;
use crate::syntax::Statement;
use crate::syntax::Struct;
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
const I64: u32 = 13;
const FIXED_START: u32 = 14;

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
}

#[derive(Clone, Debug)]
struct StructLayout {
    id: u32,
    name: String,
    fields: BTreeMap<String, FieldLayout>,
}

#[derive(Clone, Copy, Debug)]
struct FieldLayout {
    type_: ElementType,
    offset: u32,
}

#[derive(Clone, Copy, Debug)]
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
            ],
            arrays: BTreeMap::new(),
            structs: BTreeMap::new(),
            fixed_names: BTreeMap::new(),
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
    parameters: Vec<TypeName>,
    result: TypeName,
}

#[derive(Clone, Debug)]
enum Storage {
    Parameter(u32),
    Place(u32),
    ArrayView { place: u32, index: hir::Operand },
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

#[derive(Clone, Copy, Debug)]
enum BindingType {
    Scalar(TypeName),
    Array { element: ElementType, length: u32 },
    Struct(u32),
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
        signatures.insert(
            function.name.clone(),
            Signature {
                id: index as u32 + 1,
                name: function.name.clone(),
                parameters: function
                    .parameters
                    .iter()
                    .map(|one| one.type_name)
                    .collect(),
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
                .map(|one| type_id(*one))
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
    let mut types = TypeRegistry::new();
    types.register_fixed_types(&module.fixed_types)?;
    types.register_structs(&module.structs)?;
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
        ("__print_newline", Vec::new()),
        ("__print_text", vec![TypeName::String]),
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
        let name = match type_name {
            TypeName::Bool => "__print_bool",
            TypeName::Char => "__print_char",
            TypeName::I8 => "__print_i8",
            TypeName::U8 => "__print_u8",
            TypeName::I16 => "__print_i16",
            TypeName::U16 => "__print_u16",
            TypeName::I32 => "__print_i32",
            TypeName::U32 => "__print_u32",
            TypeName::F32 => "__print_f32",
            TypeName::F64 => "__print_f64",
            _ => unreachable!(),
        };
        out.push((name, vec![type_name]));
    }
    // These are formatting boundaries, not arithmetic helpers. They receive
    // the signed raw storage value followed by its fractional-bit count and
    // write canonical base-10 integer.fraction text. The formatter keeps one
    // digit after the point and trims any further trailing zeroes.
    out.push(("__print_fixed_i16", vec![TypeName::I16, TypeName::U8]));
    out.push(("__print_fixed_i32", vec![TypeName::I32, TypeName::U8]));
    out
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
        for parameter in &function.parameters {
            let value = compiler.value(parameter.type_name);
            compiler.parameters.push(value);
            compiler.scopes[0].insert(
                parameter.name.clone(),
                Binding {
                    type_: BindingType::Scalar(parameter.type_name),
                    mutable: false,
                    storage: Storage::Parameter(value),
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
                if let Some(TypeAnnotation::Array { element, length }) = annotation {
                    let Expr::Array(items, _) = value else {
                        return Err(Diagnostic::new(
                            *span,
                            "fixed-array binding requires an array literal",
                        ));
                    };
                    if items.len() != *length as usize {
                        return Err(Diagnostic::new(
                            *span,
                            format!("array expects {length} elements, got {}", items.len()),
                        ));
                    }
                    let element = self.types.resolve_element(element, *span)?;
                    let type_id = self.types.array(element, *length);
                    let place = self.array_place(name, type_id, element, *length, *mutable);
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
                    self.scopes.last_mut().expect("scope").insert(
                        name.clone(),
                        Binding {
                            type_: BindingType::Array {
                                element,
                                length: *length,
                            },
                            mutable: *mutable,
                            storage: Storage::Place(place),
                        },
                    );
                    return Ok(());
                }
                if matches!(value, Expr::Array(..)) {
                    return Err(Diagnostic::new(
                        *span,
                        "array literal requires a fixed-array annotation",
                    ));
                }
                let annotated = match annotation {
                    Some(TypeAnnotation::Value(spec)) => {
                        Some(self.types.resolve_element(spec, *span)?)
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
                        let right = self.expression(value, Some(element))?;
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
                if !matches!(expression, Expr::Call { .. }) {
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

    fn for_statement(
        &mut self,
        mode: IterationMode,
        name: &str,
        iterable: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Expr::Name(array_name, _) = iterable else {
            return Err(Diagnostic::new(
                iterable.span(),
                "for currently iterates a named fixed array",
            ));
        };
        let array = self.binding(array_name, iterable.span())?.clone();
        let BindingType::Array { element, length } = array.type_ else {
            return Err(Diagnostic::new(
                iterable.span(),
                "for requires a fixed array",
            ));
        };
        let Storage::Place(array_place) = array.storage else {
            return Err(Diagnostic::new(iterable.span(), "array has no storage"));
        };
        if mode == IterationMode::Value {
            return Err(Diagnostic::new(
                span,
                "by-value array iteration awaits aggregate move semantics; use '&' or '&mut'",
            ));
        }
        if mode == IterationMode::Mutable && !array.mutable {
            return Err(Diagnostic::new(
                span,
                format!("cannot take a mutable view of immutable array {array_name:?}"),
            ));
        }
        let length = u16::try_from(length).map_err(|_| {
            Diagnostic::new(
                iterable.span(),
                "for array length exceeds the 16-bit target",
            )
        })?;

        let index_place = self.place(&format!("$for{array_place}"), TypeName::U16, true);
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
            vec![
                hir::Operand::Value(index),
                hir::Operand::Constant(U16, i64::from(length)),
            ],
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
                type_: match element {
                    ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                    ElementType::Struct(id) => BindingType::Struct(id),
                },
                mutable: mode == IterationMode::Mutable,
                storage: Storage::ArrayView {
                    place: array_place,
                    index: hir::Operand::Value(index),
                },
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
        if name != &layout.name {
            return Err(Diagnostic::new(
                *span,
                format!("expected {} literal, found {name}", layout.name),
            ));
        }
        let mut seen = BTreeMap::new();
        for (name, value, field_span) in fields {
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
                        indices: destination.indices.clone(),
                        offset: destination.offset + field.offset,
                        mutable: destination.mutable,
                        owner: destination.owner.clone(),
                    };
                    let source = StructView {
                        struct_id,
                        place: source.place,
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
        hir::Operand::ProjectedPlace {
            place: view.place,
            indices: view.indices.clone(),
            offset: view.offset + field_offset,
            type_id: type_id(type_name),
        }
    }

    fn struct_expression_type(
        &self,
        expression: &Expr,
        span: Span,
    ) -> Result<Option<u32>, Diagnostic> {
        match expression {
            Expr::StructLiteral { name, .. } => self
                .types
                .structs
                .get(name)
                .map(|one| Some(one.id))
                .ok_or_else(|| Diagnostic::new(span, format!("unknown struct {name:?}"))),
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
                        };
                        Ok(AssignmentPlace::Scalar(destination, type_name))
                    }
                    BindingType::Struct(struct_id) => {
                        let (place, indices) = match binding.storage {
                            Storage::Place(place) => (place, Vec::new()),
                            Storage::ArrayView { place, index } => (place, vec![index]),
                            Storage::Parameter(_) => {
                                return Err(Diagnostic::new(span, "parameters are immutable"))
                            }
                        };
                        Ok(AssignmentPlace::Struct(StructView {
                            struct_id,
                            place,
                            indices,
                            offset: 0,
                            mutable: true,
                            owner: name.clone(),
                        }))
                    }
                    BindingType::Array { .. } => Err(Diagnostic::new(
                        span,
                        "whole array assignment is not supported",
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
                let BindingType::Array { element, length } = binding.type_ else {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {base:?} is not an array"),
                    ));
                };
                let Storage::Place(place) = binding.storage else {
                    return Err(Diagnostic::new(span, "array has no storage"));
                };
                let index = self.array_index(index, length)?;
                Ok(match element {
                    ElementType::Scalar(type_name) => AssignmentPlace::Scalar(
                        hir::Operand::ArrayElement(place, vec![index]),
                        type_name,
                    ),
                    ElementType::Struct(struct_id) => AssignmentPlace::Struct(StructView {
                        struct_id,
                        place,
                        indices: vec![index],
                        offset: 0,
                        mutable: true,
                        owner: base.clone(),
                    }),
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
                let (place, indices) = match binding.storage {
                    Storage::Place(place) => (place, Vec::new()),
                    Storage::ArrayView { place, index } => (place, vec![index]),
                    Storage::Parameter(_) => {
                        return Err(Diagnostic::new(span, "struct has no addressable storage"))
                    }
                };
                Ok(StructView {
                    struct_id,
                    place,
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
                let BindingType::Array { element, length } = binding.type_ else {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {name:?} is not an array"),
                    ));
                };
                let ElementType::Struct(struct_id) = element else {
                    return Err(Diagnostic::new(span, "array element is not a struct"));
                };
                let Storage::Place(place) = binding.storage else {
                    return Err(Diagnostic::new(span, "array has no storage"));
                };
                let index = self.array_index(index, length)?;
                Ok(StructView {
                    struct_id,
                    place,
                    indices: vec![index],
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
            Expr::Array(_, span) => Err(Diagnostic::new(
                *span,
                "an array literal is valid only as a fixed-array initializer",
            )),
            Expr::StructLiteral { span, .. } => Err(Diagnostic::new(
                *span,
                "a struct literal is currently valid only inside a fixed-array initializer",
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
                };
                Ok(TypedOperand {
                    operand: Some(operand),
                    type_name,
                })
            }
            Expr::Index { base, index, span } => {
                self.index_expression(base, index, expected, *span)
            }
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
                    _ => {}
                }
                let result = self.value(operand.type_name);
                self.emit(
                    match op {
                        UnaryOp::Negative if is_float(operand.type_name) => "fneg",
                        UnaryOp::Negative => "neg",
                        UnaryOp::Not => "not",
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

    fn array_index(&mut self, expression: &Expr, length: u32) -> Result<hir::Operand, Diagnostic> {
        if let Expr::Integer(value, span) = expression {
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
        let BindingType::Array {
            element, length, ..
        } = binding.type_
        else {
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
        let Storage::Place(place) = binding.storage else {
            return Err(Diagnostic::new(
                span,
                "array parameter lowering is not implemented",
            ));
        };
        let index = self.array_index(index, length)?;
        let result = self.value(element);
        self.emit(
            "load",
            vec![result],
            vec![hir::Operand::ArrayElement(place, vec![index])],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: element,
        })
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
        let comparison = matches!(
            operation,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        );
        if comparison && expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let mut left_expected = (!comparison).then_some(expected).flatten();
        if left_expected.is_none() {
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
        if comparison {
            let equality = matches!(operation, BinaryOp::Equal | BinaryOp::NotEqual);
            if (!equality && !is_ordered(left.type_name)) || left.type_name == TypeName::Void {
                return Err(Diagnostic::new(
                    span,
                    if equality {
                        "equality requires scalar operands"
                    } else {
                        "ordering requires numeric or char operands"
                    },
                ));
            }
        } else if !is_numeric(left.type_name) {
            return Err(Diagnostic::new(
                span,
                "arithmetic requires numeric operands",
            ));
        }
        let right = self.expression(right, Some(left.type_name))?;
        self.binary_operands(operation, left, right, expected, span)
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
        let comparison = matches!(
            operation,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        );
        if comparison && expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        if left.type_name != right.type_name {
            return Err(type_mismatch(span, left.type_name, right.type_name));
        }
        if comparison {
            let equality = matches!(operation, BinaryOp::Equal | BinaryOp::NotEqual);
            if (!equality && !is_ordered(left.type_name)) || left.type_name == TypeName::Void {
                return Err(Diagnostic::new(
                    span,
                    if equality {
                        "equality requires scalar operands"
                    } else {
                        "ordering requires numeric or char operands"
                    },
                ));
            }
        } else if !is_numeric(left.type_name) {
            return Err(Diagnostic::new(
                span,
                "arithmetic requires numeric operands",
            ));
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
            BinaryOp::Is | BinaryOp::IsNot => unreachable!("identity handled above"),
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
                        BindingType::Array { .. } | BindingType::Struct(_) => None,
                    })
            }
            Expr::Call { name, .. } => self.signatures.get(name).map(|one| one.result),
            Expr::Unary { operand, .. } => self.expression_type_hint(operand),
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
                        } => Some(element),
                        BindingType::Array {
                            element: ElementType::Struct(_),
                            ..
                        }
                        | BindingType::Scalar(_)
                        | BindingType::Struct(_) => None,
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
                ) {
                    return Some(TypeName::Bool);
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
            | Expr::StructLiteral { .. } => None,
        }
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
        for (argument, type_name) in arguments.iter().zip(&signature.parameters) {
            let value = self.expression(argument, Some(*type_name))?;
            operands.push(required(value, argument.span())?);
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
        self.emit_builtin("__print_newline", Vec::new());
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
                match storage {
                    FixedStorage::I16 => "__print_fixed_i16",
                    FixedStorage::I32 => "__print_fixed_i32",
                },
                vec![
                    hir::Operand::Value(raw),
                    hir::Operand::Constant(U8, i64::from(fraction)),
                ],
            );
            return;
        }
        let name = match type_name {
            TypeName::String => "__print_text",
            TypeName::Bool => "__print_bool",
            TypeName::Char => "__print_char",
            TypeName::I8 => "__print_i8",
            TypeName::U8 => "__print_u8",
            TypeName::I16 => "__print_i16",
            TypeName::U16 => "__print_u16",
            TypeName::I32 => "__print_i32",
            TypeName::U32 => "__print_u32",
            TypeName::F32 => "__print_f32",
            TypeName::F64 => "__print_f64",
            TypeName::Void | TypeName::I64 | TypeName::Fixed { .. } => unreachable!(),
        };
        self.emit_builtin(name, vec![operand]);
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
        let id = self.next_value;
        self.next_value += 1;
        self.values.push(hir::Value {
            id,
            type_id: type_id(type_name),
        });
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
        self.local_place(name, type_id, extent, mutable)
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
        assert!(json.contains("\"callee\":\"__print_text\""));
        assert!(json.contains("\"callee\":\"__print_i32\""));
        assert!(json.contains("\"callee\":\"__print_newline\""));
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
}
