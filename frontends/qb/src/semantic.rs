//! Name/type/storage resolution and direct common-HIR construction.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::intrinsics::{self, Lowering, ResultClass};
use crate::dialect::Dialect;
use crate::syntax::{
    Binary, Declaration, ExitTarget, Expr, FileMode, Literal, Module, PrintSeparator,
    ProcedureKind, ResumeTarget, Statement, TypeName, Unary,
};

const VOID: u32 = 0;
const INTEGER: u32 = 1;
const LONG: u32 = 2;
const SINGLE: u32 = 3;
const DOUBLE: u32 = 4;
const BOOLEAN: u32 = 5;
const STRING: u32 = 6;
const ANY: u32 = 7;
const BYTE: u32 = 8;
// BASCOM's declarative scanner reserves eight dimension records for an
// array whose rank is not present in its declaration.  This is an ABI storage
// rule, distinct from the language's 60-index parser ceiling.
const UNSPECIFIED_ARRAY_RANK: usize = 8;
const READ_DATA_OBJECT: &str = "$qb$readData";
const STATEMENT_TABLE_OBJECT: &str = "$qb$statementTable";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticError {
    pub message: String,
}

#[derive(Clone)]
struct Type {
    id: u32,
    name: String,
    kind: &'static str,
    width: usize,
    signed: Option<bool>,
    evaluation: &'static str,
    element: Option<u32>,
    bounds: Vec<(i64, i64)>,
    address: &'static str,
}

#[derive(Clone)]
struct Variable {
    place: u32,
    type_id: u32,
    element: Option<u32>,
    bounds: Vec<(i64, i64)>,
    indirect: Option<u32>,
    descriptor: Option<u32>,
    descriptor_place: Option<u32>,
    descriptor_data: &'static str,
}

#[derive(Clone)]
enum Operand {
    Value(u32),
    Constant(u32, Number),
    Place(u32),
    Element(u32, Vec<Operand>),
    Projection {
        place: u32,
        indices: Vec<Operand>,
        offset: usize,
        type_id: u32,
    },
    Indirect {
        base: u32,
        offset: usize,
        type_id: u32,
    },
}

enum ProjectionBase {
    Place(u32, Vec<Operand>),
    Indirect(u32),
}

#[derive(Clone)]
enum Number {
    Integer(i64),
    Real(String),
}

struct Instruction {
    id: u32,
    op: &'static str,
    results: Vec<u32>,
    operands: Vec<Operand>,
    callee: Option<String>,
}

struct CallAbi {
    instruction: u32,
    order: Vec<usize>,
    caller_cleanup: bool,
    callee: Option<u32>,
}

#[derive(Clone, Eq, PartialEq)]
struct Signature {
    symbol: u32,
    parameters: Vec<(u32, bool, bool, bool)>,
    result: Option<u32>,
    callee: String,
    cdecl: bool,
}

struct Callable {
    id: u32,
    name: String,
    result_type: Option<u32>,
    parameters: Vec<(u32, bool, bool, bool)>,
    defined: bool,
}

#[derive(Clone)]
struct Field {
    type_id: u32,
    offset: usize,
}

#[derive(Clone)]
struct Udt {
    type_id: u32,
    fields: BTreeMap<String, Field>,
}

struct Terminator {
    kind: &'static str,
    operands: Vec<Operand>,
    targets: Vec<u32>,
}

struct Block {
    id: u32,
    instructions: Vec<Instruction>,
    terminator: Option<Terminator>,
}

#[derive(Clone)]
struct Place {
    id: u32,
    name: String,
    type_id: u32,
    offset: isize,
    extent: usize,
    storage: &'static str,
    symbol: u32,
}

struct DataRelocation {
    at: usize,
    target: u32,
    addend: usize,
    address: &'static str,
}

struct DataObject {
    id: u32,
    name: String,
    bytes: Vec<u8>,
    readonly: bool,
    relocations: Vec<DataRelocation>,
    linkage: &'static str,
    address: &'static str,
}

struct Function {
    id: u32,
    name: String,
    result_type: u32,
    values: Vec<(u32, u32)>,
    places: Vec<Place>,
    blocks: Vec<Block>,
    parameters: Vec<u32>,
    caller_cleanup: bool,
    parameter_bytes: usize,
    calls: Vec<CallAbi>,
    error_handler: Option<u32>,
    error_handler_local: bool,
    external_entries: Vec<u32>,
}

struct Compiler {
    dialect: Dialect,
    runtime: String,
    row_major: bool,
    huge_arrays: bool,
    checked_arrays: bool,
    mbf: bool,
    alternate_math: bool,
    module_name: String,
    types: Vec<Type>,
    variables: BTreeMap<String, Variable>,
    constants: BTreeMap<String, (u32, Number)>,
    signatures: BTreeMap<String, Signature>,
    callables: Vec<Callable>,
    udts: BTreeMap<String, Udt>,
    pointer_types: BTreeMap<(u32, &'static str), u32>,
    functions: Vec<Function>,
    data: Vec<DataObject>,
    values: Vec<(u32, u32)>,
    places: Vec<Place>,
    blocks: Vec<Block>,
    calls: Vec<CallAbi>,
    current_block: usize,
    descriptor_bases: BTreeMap<(u32, u32, &'static str), u32>,
    descriptor_fields: BTreeMap<(u32, usize, u32), u32>,
    labels: BTreeMap<String, u32>,
    exits: Vec<(ExitTarget, u32)>,
    return_block: Option<u32>,
    result_place: Option<(u32, u32)>,
    error_handler: Option<u32>,
    error_handler_local: bool,
    next_type: u32,
    next_value: u32,
    next_place: u32,
    next_instruction: u32,
    next_block: u32,
    data_offset: usize,
    implicit_storage: &'static str,
    next_data: u32,
    def_segment_symbol: Option<u32>,
    far_string_segment_symbol: Option<u32>,
    floating_literals: BTreeMap<(u32, Vec<u8>), u32>,
    default_types: [u32; 26],
    option_base: i64,
    statement_entries: Vec<(u32, u32, u16)>,
    pending_numeric_line: Option<u16>,
}

pub fn compile(
    module: &Module,
    module_name: &str,
    dialect: Dialect,
    runtime: &str,
) -> Result<String, SemanticError> {
    compile_with_array_order(module, module_name, dialect, runtime, false)
}

pub fn compile_with_array_order(
    module: &Module,
    module_name: &str,
    dialect: Dialect,
    runtime: &str,
    row_major: bool,
) -> Result<String, SemanticError> {
    compile_with_options(
        module,
        module_name,
        dialect,
        runtime,
        row_major,
        false,
        false,
        false,
        false,
    )
}

pub fn compile_with_options(
    module: &Module,
    module_name: &str,
    dialect: Dialect,
    runtime: &str,
    row_major: bool,
    huge_arrays: bool,
    checked_arrays: bool,
    mbf: bool,
    alternate_math: bool,
) -> Result<String, SemanticError> {
    let mut compiler = Compiler::new(
        module_name,
        dialect,
        runtime,
        row_major,
        huge_arrays,
        checked_arrays,
        mbf,
        alternate_math,
    );
    compiler.apply_default_types(&module.statements)?;
    compiler.apply_option_base(&module.statements)?;
    compiler.type_declarations(module)?;
    compiler.signatures(module)?;
    compiler.declarations(module)?;
    compiler.reserve_labels(&module.statements)?;
    compiler.statements(module)?;
    compiler.finish();
    compiler.save_function(1, "__main", VOID, Vec::new(), false, 0);

    let module_default_types = compiler.default_types;
    let module_variables = compiler.variables.clone();
    let module_constants = compiler.constants.clone();
    let module_places = compiler.functions[0].places.clone();
    for (index, procedure) in module
        .procedures
        .iter()
        .filter(|procedure| !procedure.declaration)
        .enumerate()
    {
        compiler.reset_function(
            module_variables.clone(),
            module_constants.clone(),
            module_places.clone(),
        );
        compiler.implicit_storage = if procedure.is_static {
            "static"
        } else {
            "local"
        };
        compiler.default_types = module_default_types;
        let mut parameters = Vec::new();
        let mut parameter_bytes = 0;
        for parameter in &procedure.parameters {
            let parameter_type = compiler.named_type(
                &parameter.declaration.name,
                parameter.declaration.type_name.as_ref(),
            )?;
            let is_array = parameter.declaration.array;
            if is_array && parameter.by_value {
                return compiler.fail("array parameters cannot be BYVAL");
            }
            let value_type = if is_array {
                let descriptor = compiler.opaque_type(
                    format!("{} descriptor", parameter.declaration.name),
                    14 + 4 * UNSPECIFIED_ARRAY_RANK,
                );
                compiler.pointer_type(descriptor)
            } else if parameter.segmented {
                compiler.far_pointer_type(parameter_type)
            } else if parameter.by_value {
                parameter_type
            } else {
                compiler.pointer_type(parameter_type)
            };
            let value = compiler.value(value_type);
            parameters.push(value);
            parameter_bytes += compiler.width(value_type).max(2);
            if is_array {
                compiler.variables.insert(
                    canonical(&parameter.declaration.name).into(),
                    Variable {
                        place: 0,
                        type_id: parameter_type,
                        element: Some(parameter_type),
                        bounds: Vec::new(),
                        indirect: None,
                        descriptor: Some(value),
                        descriptor_place: None,
                        // A dynamic STRING array's elements are near string
                        // descriptors even when the array descriptor also
                        // carries a whole data pointer.  BC loads the adjusted
                        // near base at +0Ah before SASS/FLEN/SCAT.  Numeric
                        // and fixed-string arrays retain the general huge
                        // descriptor path because their element data may live
                        // outside DGROUP. Microsoft keeps that pointer split:
                        // selector at +2 and adjusted offset at +0Ah.
                        descriptor_data: if parameter_type == STRING {
                            "near"
                        } else if compiler.huge_arrays {
                            "split_huge"
                        } else {
                            "split_far"
                        },
                    },
                );
            } else if parameter.by_value {
                let place = compiler.declare_as(&parameter.declaration, "local")?;
                compiler.emit(
                    "store",
                    Vec::new(),
                    vec![Operand::Place(place), Operand::Value(value)],
                );
            } else {
                compiler.variables.insert(
                    canonical(&parameter.declaration.name).into(),
                    Variable {
                        place: 0,
                        type_id: parameter_type,
                        element: None,
                        bounds: Vec::new(),
                        indirect: Some(value),
                        descriptor: None,
                        descriptor_place: None,
                        descriptor_data: "none",
                    },
                );
            }
        }
        let result_type = match (&procedure.kind, &procedure.result) {
            (ProcedureKind::Sub, _) => VOID,
            (ProcedureKind::Function, result) => {
                compiler.named_type(&procedure.name, result.as_ref())?
            }
        };
        if procedure.kind == ProcedureKind::Function
            && matches!(result_type, SINGLE | DOUBLE)
            && !procedure.cdecl
        {
            // Microsoft BASIC floating functions receive a hidden near
            // destination pointer after the source formals. The semantic
            // return remains a float; late QB ABI physicalization stores it
            // through this parameter and returns the pointer in AX.
            let pointer_type = compiler.pointer_type(result_type);
            parameters.push(compiler.value(pointer_type));
            parameter_bytes += 2;
        }
        if procedure.kind == ProcedureKind::Function {
            let declaration = Declaration {
                name: procedure.name.clone(),
                type_name: Some(type_name(result_type)),
                array: false,
                bounds: Vec::new(),
                fixed_length: None,
                shared: false,
                dynamic: false,
                span: procedure.span,
            };
            let place = compiler.declare_as(&declaration, "local")?;
            compiler.result_place = Some((place, result_type));
        }
        // A procedure is its own DEF-type scope in Microsoft BASIC. Its
        // directives govern all declarations in the body regardless of
        // source order, but not the already-declared procedure signature.
        compiler.apply_default_types(&procedure.body)?;
        compiler.declarations_in(&procedure.body, compiler.implicit_storage)?;
        compiler.reserve_labels(&procedure.body)?;
        compiler
            .statement_list(&procedure.body)
            .map_err(|error| SemanticError {
                message: format!("{}: {}", procedure.name, error.message),
            })?;
        compiler.finish();
        compiler.cleanup_local_arrays()?;
        // STRING expressions in HIR are near descriptor addresses.  The
        // declared result place remains an owned four-byte descriptor, but a
        // callable result must have the same value type its callers consume.
        // `finish` has already inserted B$SCPF to move the owned descriptor
        // onto the runtime temporary chain before the frame is released.
        let hir_result_type = if result_type == STRING {
            compiler.pointer_type(STRING)
        } else {
            result_type
        };
        compiler.save_function(
            index as u32 + 2,
            &procedure.name,
            hir_result_type,
            parameters,
            procedure.cdecl,
            parameter_bytes,
        );
    }
    Ok(compiler.json())
}

impl Compiler {
    fn new(
        module_name: &str,
        dialect: Dialect,
        runtime: &str,
        row_major: bool,
        huge_arrays: bool,
        checked_arrays: bool,
        mbf: bool,
        alternate_math: bool,
    ) -> Self {
        let types = vec![
            scalar(VOID, "void", "void", 0, None, "none"),
            scalar(INTEGER, "integer", "integer", 2, Some(true), "none"),
            scalar(LONG, "long", "integer", 4, Some(true), "none"),
            scalar(SINGLE, "single", "float", 4, None, "extended80"),
            scalar(DOUBLE, "double", "float", 8, None, "extended80"),
            scalar(BOOLEAN, "boolean", "boolean", 2, Some(true), "none"),
            scalar(STRING, "string", "opaque", 4, None, "none"),
            scalar(ANY, "any", "opaque", 0, None, "none"),
            scalar(BYTE, "$byte", "integer", 1, Some(false), "none"),
        ];
        Self {
            dialect,
            runtime: runtime.into(),
            row_major,
            huge_arrays,
            checked_arrays,
            mbf,
            alternate_math,
            module_name: module_name.into(),
            types,
            variables: BTreeMap::new(),
            constants: BTreeMap::new(),
            signatures: BTreeMap::new(),
            callables: Vec::new(),
            udts: BTreeMap::new(),
            pointer_types: BTreeMap::new(),
            functions: Vec::new(),
            data: vec![
                DataObject {
                    id: 1,
                    name: "$data".into(),
                    bytes: Vec::new(),
                    readonly: false,
                    relocations: Vec::new(),
                    linkage: "internal",
                    address: "near",
                },
                DataObject {
                    id: 2,
                    name: STATEMENT_TABLE_OBJECT.into(),
                    bytes: Vec::new(),
                    readonly: true,
                    relocations: Vec::new(),
                    linkage: "internal",
                    address: "near",
                },
            ],
            values: Vec::new(),
            places: Vec::new(),
            blocks: vec![Block {
                id: 1,
                instructions: Vec::new(),
                terminator: None,
            }],
            calls: Vec::new(),
            current_block: 0,
            descriptor_bases: BTreeMap::new(),
            descriptor_fields: BTreeMap::new(),
            labels: BTreeMap::new(),
            exits: Vec::new(),
            return_block: None,
            result_place: None,
            error_handler: None,
            error_handler_local: false,
            next_type: 9,
            next_value: 1,
            next_place: 1,
            next_instruction: 1,
            next_block: 2,
            data_offset: 0,
            implicit_storage: "module",
            next_data: 3,
            def_segment_symbol: None,
            far_string_segment_symbol: None,
            floating_literals: BTreeMap::new(),
            default_types: [SINGLE; 26],
            option_base: 0,
            statement_entries: Vec::new(),
            pending_numeric_line: None,
        }
    }

    fn reset_function(
        &mut self,
        variables: BTreeMap<String, Variable>,
        constants: BTreeMap<String, (u32, Number)>,
        places: Vec<Place>,
    ) {
        self.variables = variables;
        self.constants = constants;
        self.next_place = places.iter().map(|place| place.id).max().unwrap_or(0) + 1;
        self.places = places;
        self.values.clear();
        self.blocks = vec![Block {
            id: 1,
            instructions: Vec::new(),
            terminator: None,
        }];
        self.calls.clear();
        self.current_block = 0;
        self.descriptor_bases.clear();
        self.descriptor_fields.clear();
        self.labels.clear();
        self.exits.clear();
        self.return_block = None;
        self.result_place = None;
        self.error_handler = None;
        self.error_handler_local = false;
        self.statement_entries.clear();
        self.pending_numeric_line = None;
        self.next_value = 1;
        self.next_instruction = 1;
        self.next_block = 2;
        self.data_offset = 0;
        self.implicit_storage = "local";
    }

    fn save_function(
        &mut self,
        id: u32,
        name: &str,
        result_type: u32,
        parameters: Vec<u32>,
        caller_cleanup: bool,
        parameter_bytes: usize,
    ) {
        self.prune_unreachable(&parameters);
        let retained: BTreeSet<u32> = self.blocks.iter().map(|block| block.id).collect();
        let external_entries: Vec<u32> = self
            .statement_entries
            .iter()
            .map(|(block, _, _)| *block)
            .filter(|block| retained.contains(block))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let table = self
            .data
            .iter_mut()
            .find(|object| object.name == STATEMENT_TABLE_OBJECT)
            .expect("statement table metadata object");
        for (block, instruction, line) in self.statement_entries.drain(..) {
            if !retained.contains(&block) {
                continue;
            }
            table.bytes.extend_from_slice(&id.to_le_bytes());
            table.bytes.extend_from_slice(&block.to_le_bytes());
            table.bytes.extend_from_slice(&instruction.to_le_bytes());
            table.bytes.extend_from_slice(&line.to_le_bytes());
        }
        self.functions.push(Function {
            id,
            name: name.into(),
            result_type,
            values: std::mem::take(&mut self.values),
            places: std::mem::take(&mut self.places),
            blocks: std::mem::take(&mut self.blocks),
            parameters,
            caller_cleanup,
            parameter_bytes,
            calls: std::mem::take(&mut self.calls),
            error_handler: self.error_handler,
            error_handler_local: self.error_handler_local,
            external_entries,
        });
    }

    fn prune_unreachable(&mut self, parameters: &[u32]) {
        let mut reachable = BTreeSet::from([1]);
        reachable.extend(self.statement_entries.iter().map(|(block, _, _)| *block));
        if let Some(handler) = self.error_handler {
            // ON ERROR is an asynchronous side entry installed by the
            // runtime. It deliberately has no ordinary CFG predecessor.
            reachable.insert(handler);
        }
        loop {
            let before = reachable.len();
            for block in &self.blocks {
                if reachable.contains(&block.id) {
                    if let Some(terminator) = &block.terminator {
                        reachable.extend(terminator.targets.iter().copied());
                    }
                }
            }
            if reachable.len() == before {
                break;
            }
        }

        self.blocks.retain(|block| reachable.contains(&block.id));
        let instructions: BTreeSet<u32> = self
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter().map(|instruction| instruction.id))
            .collect();
        self.calls
            .retain(|call| instructions.contains(&call.instruction));
        let mut values: BTreeSet<u32> = parameters.iter().copied().collect();
        values.extend(
            self.blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .flat_map(|instruction| instruction.results.iter().copied()),
        );
        self.values.retain(|(id, _)| values.contains(id));
    }

    fn declarations(&mut self, module: &Module) -> Result<(), SemanticError> {
        self.declarations_in(&module.statements, "module")
    }

    fn apply_option_base(&mut self, statements: &[Statement]) -> Result<(), SemanticError> {
        let mut selected = None;
        for statement in statements {
            let Statement::OptionBase(value, _) = statement else {
                continue;
            };
            if selected.replace(*value).is_some() {
                return self.fail("OPTION BASE may appear only once per module");
            }
        }
        if let Some(value) = selected {
            self.option_base = value;
        }
        Ok(())
    }

    fn type_declarations(&mut self, module: &Module) -> Result<(), SemanticError> {
        for statement in &module.statements {
            let Statement::TypeDecl { name, fields, .. } = statement else {
                continue;
            };
            if self.udts.contains_key(canonical(name)) {
                return self.fail(format!("duplicate TYPE {name}"));
            }
            let mut offset = 0usize;
            let mut resolved = BTreeMap::new();
            for field in fields {
                let type_name = field.type_name.as_ref().expect("TYPE fields are typed");
                let field_type = if *type_name == TypeName::String {
                    if let Some(length) = &field.fixed_length {
                        let width = self.constant_integer(length)? as usize;
                        self.opaque_type(format!("string*{width}"), width)
                    } else {
                        STRING
                    }
                } else {
                    self.resolve_type(Some(type_name))?
                };
                let bounds = field
                    .bounds
                    .iter()
                    .map(|bound| {
                        let lower = bound
                            .lower
                            .as_ref()
                            .map(|value| self.constant_integer(value))
                            .transpose()?
                            .unwrap_or(self.option_base);
                        let upper = self.constant_integer(&bound.upper)?;
                        if upper < lower {
                            return self.fail(format!("invalid bound on {name}.{}", field.name));
                        }
                        Ok((lower, upper))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut extent = self.width(field_type);
                for (lower, upper) in &bounds {
                    if *upper < *lower {
                        return self.fail(format!("negative bound on {name}.{}", field.name));
                    }
                    extent = extent
                        .checked_mul((upper - lower + 1) as usize)
                        .ok_or_else(|| SemanticError {
                            message: format!("field {name}.{} is too large", field.name),
                        })?;
                }
                let stored_type = if bounds.is_empty() {
                    field_type
                } else {
                    self.array_type(
                        format!("{name}.{}[]", field.name),
                        field_type,
                        bounds.clone(),
                        extent,
                    )
                };
                if resolved
                    .insert(
                        canonical(&field.name).into(),
                        Field {
                            type_id: stored_type,
                            offset,
                        },
                    )
                    .is_some()
                {
                    return self.fail(format!("duplicate field {name}.{}", field.name));
                }
                offset += extent;
            }
            let type_id = self.opaque_type(name.clone(), offset);
            self.udts.insert(
                canonical(name).into(),
                Udt {
                    type_id,
                    fields: resolved,
                },
            );
        }
        Ok(())
    }

    fn opaque_type(&mut self, name: String, width: usize) -> u32 {
        let id = self.next_type;
        self.next_type += 1;
        self.types.push(Type {
            id,
            name,
            kind: "opaque",
            width,
            signed: None,
            evaluation: "none",
            element: None,
            bounds: Vec::new(),
            address: "none",
        });
        id
    }

    fn pointer_type(&mut self, element: u32) -> u32 {
        self.addressed_pointer_type(element, "near", 2)
    }

    fn whole_pointer_type(&mut self, element: u32) -> u32 {
        self.addressed_pointer_type(element, "huge", 4)
    }

    fn far_pointer_type(&mut self, element: u32) -> u32 {
        self.addressed_pointer_type(element, "far", 4)
    }

    fn addressed_pointer_type(&mut self, element: u32, address: &'static str, width: usize) -> u32 {
        if let Some(type_id) = self.pointer_types.get(&(element, address)) {
            return *type_id;
        }
        let id = self.next_type;
        self.next_type += 1;
        let name = format!("{address}*{}", self.name(element));
        self.types.push(Type {
            id,
            name,
            kind: "pointer",
            width,
            signed: None,
            evaluation: "none",
            element: Some(element),
            bounds: Vec::new(),
            address,
        });
        self.pointer_types.insert((element, address), id);
        id
    }

    fn array_type(
        &mut self,
        name: String,
        element: u32,
        bounds: Vec<(i64, i64)>,
        width: usize,
    ) -> u32 {
        let id = self.next_type;
        self.next_type += 1;
        self.types.push(Type {
            id,
            name,
            kind: "array",
            width,
            signed: None,
            evaluation: "none",
            element: Some(element),
            bounds,
            address: "near",
        });
        id
    }

    fn resolve_type(&self, type_name: Option<&TypeName>) -> Result<u32, SemanticError> {
        match type_name.unwrap_or(&TypeName::Single) {
            TypeName::Named(name) => self
                .udts
                .get(canonical(name))
                .map(|udt| udt.type_id)
                .or((*name == "ANY").then_some(ANY))
                .ok_or_else(|| SemanticError {
                    message: format!("unknown TYPE {name}"),
                }),
            other => type_id(Some(other)),
        }
    }

    fn named_type(&self, name: &str, explicit: Option<&TypeName>) -> Result<u32, SemanticError> {
        let inferred = suffix(name);
        if let Some(selected) = explicit.or(inferred.as_ref()) {
            return self.resolve_type(Some(selected));
        }
        let first = name
            .as_bytes()
            .first()
            .copied()
            .map(|one| one.to_ascii_uppercase())
            .filter(u8::is_ascii_alphabetic)
            .ok_or_else(|| SemanticError {
                message: format!("{name} has no initial letter for default typing"),
            })?;
        Ok(self.default_types[(first - b'A') as usize])
    }

    fn apply_default_types(&mut self, statements: &[Statement]) -> Result<(), SemanticError> {
        for statement in statements {
            let Statement::DefType {
                type_name, ranges, ..
            } = statement
            else {
                continue;
            };
            let selected = self.resolve_type(Some(type_name))?;
            for (first, last) in ranges {
                for letter in (*first as u8)..=(*last as u8) {
                    self.default_types[(letter - b'A') as usize] = selected;
                }
            }
        }
        Ok(())
    }

    fn signatures(&mut self, module: &Module) -> Result<(), SemanticError> {
        for procedure in &module.procedures {
            let key = canonical(&procedure.name);
            let symbol = self
                .signatures
                .get(key)
                .map(|one| one.symbol)
                .unwrap_or(self.callables.len() as u32 + 1);
            let signature = Signature {
                symbol,
                parameters: procedure
                    .parameters
                    .iter()
                    .map(|parameter| {
                        Ok((
                            self.named_type(
                                &parameter.declaration.name,
                                parameter.declaration.type_name.as_ref(),
                            )?,
                            parameter.by_value,
                            parameter.segmented,
                            parameter.declaration.array,
                        ))
                    })
                    .collect::<Result<Vec<_>, SemanticError>>()?,
                result: match procedure.kind {
                    ProcedureKind::Sub => None,
                    ProcedureKind::Function => {
                        Some(self.named_type(&procedure.name, procedure.result.as_ref())?)
                    }
                },
                callee: procedure
                    .alias
                    .clone()
                    .unwrap_or_else(|| procedure.name.clone()),
                cdecl: procedure.cdecl,
            };
            if let Some(previous) = self.signatures.get(key) {
                if previous != &signature {
                    return self.fail(format!(
                        "declaration and definition of {} do not agree",
                        procedure.name
                    ));
                }
                if !procedure.declaration {
                    self.callables[(symbol - 1) as usize].defined = true;
                }
            } else {
                self.callables.push(Callable {
                    id: symbol,
                    name: signature.callee.clone(),
                    result_type: signature.result,
                    parameters: signature.parameters.clone(),
                    defined: !procedure.declaration,
                });
                self.signatures.insert(key.into(), signature);
            }
        }
        Ok(())
    }

    fn declarations_in(
        &mut self,
        statements: &[Statement],
        storage: &'static str,
    ) -> Result<(), SemanticError> {
        for statement in statements {
            match statement {
                Statement::Dim(items) => {
                    for item in items {
                        if storage == "local" && item.array {
                            // Microsoft documents every explicitly DIMmed
                            // array in a non-STATIC procedure as dynamic,
                            // regardless of the module's $STATIC default.
                            let mut dynamic = item.clone();
                            dynamic.dynamic = true;
                            self.declare_as(&dynamic, storage)?;
                        } else {
                            self.declare_as(item, storage)?;
                        }
                    }
                }
                Statement::Static(items) => {
                    for item in items {
                        self.declare_as(item, "static")?;
                    }
                }
                Statement::DefType { .. }
                | Statement::TypeDecl { .. }
                | Statement::OptionBase(_, _) => {}
                Statement::Redim(items) => {
                    for item in items {
                        if self.variables.contains_key(canonical(&item.name)) {
                            continue;
                        }
                        // REDIM is itself a declaration in QB, including
                        // under OPTION EXPLICIT. Declare only the dynamic
                        // descriptor here; the statement later evaluates its
                        // actual bounds and calls RDIM.
                        let mut declaration = item.clone();
                        declaration.bounds.clear();
                        self.declare_as(&declaration, storage)?;
                    }
                }
                Statement::Const { name, value, .. } => {
                    let literal = self.constant(value)?;
                    if self
                        .constants
                        .insert(canonical(name).into(), literal)
                        .is_some()
                        || self.variables.contains_key(canonical(name))
                    {
                        return self.fail(format!("duplicate declaration {name}"));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn declare_as(
        &mut self,
        declaration: &Declaration,
        storage: &'static str,
    ) -> Result<u32, SemanticError> {
        if self.variables.contains_key(canonical(&declaration.name))
            || self.constants.contains_key(canonical(&declaration.name))
        {
            return self.fail(format!("duplicate declaration {}", declaration.name));
        }
        let inferred = suffix(&declaration.name);
        let selected = declaration.type_name.as_ref().or(inferred.as_ref());
        let element = if matches!(selected, Some(TypeName::String)) {
            if let Some(length) = &declaration.fixed_length {
                let width = self.constant_integer(length)?;
                if width <= 0 {
                    return self.fail("fixed-length string width must be positive");
                }
                self.opaque_type(format!("string*{width}"), width as usize)
            } else {
                STRING
            }
        } else {
            self.named_type(&declaration.name, declaration.type_name.as_ref())?
        };
        let bounds = declaration
            .bounds
            .iter()
            .map(|one| {
                let lower = one
                    .lower
                    .as_ref()
                    .map(|value| self.constant_integer(value))
                    .transpose()?
                    .unwrap_or(self.option_base);
                let upper = self.constant_integer(&one.upper)?;
                if upper < lower {
                    return self.fail("array upper bound is below its lower bound");
                }
                Ok((lower, upper))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if declaration.array
            && !bounds.is_empty()
            && storage != "static"
            && (element == STRING || declaration.dynamic)
        {
            // Variable-length STRING elements are always managed. $DYNAMIC
            // makes the same runtime-owned representation apply to every
            // bounded array. BC creates it with B$DDIM, leaving a mutable
            // descriptor that a later REDIM may legally replace.
            let descriptor_type = self.opaque_type(
                format!("{} descriptor", declaration.name),
                14 + 4 * bounds.len(),
            );
            let descriptor_place = self.next_place;
            self.next_place += 1;
            let descriptor_extent = self.width(descriptor_type);
            self.places.push(Place {
                id: descriptor_place,
                name: format!("{}$descriptor", declaration.name),
                type_id: descriptor_type,
                offset: self.place_offset(storage, descriptor_extent),
                extent: descriptor_extent,
                storage,
                symbol: if matches!(storage, "local" | "parameter") {
                    0
                } else {
                    1
                },
            });
            self.data_offset += descriptor_extent;
            self.reserve_module_data(storage);
            let pointer_type = self.pointer_type(descriptor_type);
            let descriptor = self.value(pointer_type);
            self.emit(
                "address",
                vec![descriptor],
                vec![Operand::Place(descriptor_place)],
            );
            let mut operands = Vec::new();
            for (lower, upper) in &bounds {
                operands.push(Operand::Constant(INTEGER, Number::Integer(*lower)));
                operands.push(Operand::Constant(INTEGER, Number::Integer(*upper)));
            }
            operands.extend([
                Operand::Constant(INTEGER, Number::Integer(self.width(element) as i64)),
                Operand::Constant(
                    INTEGER,
                    Number::Integer(
                        bounds.len() as i64
                            | if element == STRING {
                                0x8000
                            } else if self.huge_arrays {
                                0x0200
                            } else {
                                0x0100
                            },
                    ),
                ),
                Operand::Value(descriptor),
            ]);
            let mut order = Vec::new();
            for dimension in (0..bounds.len()).rev() {
                order.extend([2 * dimension, 2 * dimension + 1]);
            }
            order.extend(2 * bounds.len()..2 * bounds.len() + 3);
            self.emit_call("B$DDIM", Vec::new(), operands, order, false);
            self.variables.insert(
                canonical(&declaration.name).into(),
                Variable {
                    place: 0,
                    type_id: element,
                    element: Some(element),
                    bounds: Vec::new(),
                    indirect: None,
                    descriptor: None,
                    descriptor_place: Some(descriptor_place),
                    // STRING arrays are a near run of four-byte string
                    // descriptors. Numeric/UDT dynamic arrays are huge:
                    // Microsoft AD+2 supplies the allocation selector and
                    // AD+0Ah its offset. Treating these as DGROUP wrote the
                    // first MOD_TEX record through selector zero/DGROUP.
                    descriptor_data: if element == STRING {
                        "near"
                    } else if self.huge_arrays {
                        "split_huge"
                    } else {
                        "split_far"
                    },
                },
            );
            return Ok(0);
        }
        if declaration.array && bounds.is_empty() {
            let descriptor_type = self.opaque_type(
                format!("{} descriptor", declaration.name),
                14 + 4 * UNSPECIFIED_ARRAY_RANK,
            );
            let descriptor_place = self.next_place;
            self.next_place += 1;
            let descriptor_extent = self.width(descriptor_type);
            self.places.push(Place {
                id: descriptor_place,
                name: format!("{}$descriptor", declaration.name),
                type_id: descriptor_type,
                offset: self.place_offset(storage, descriptor_extent),
                extent: descriptor_extent,
                storage,
                symbol: if matches!(storage, "local" | "parameter") {
                    0
                } else {
                    1
                },
            });
            self.data_offset += descriptor_extent;
            self.reserve_module_data(storage);
            if self.data_offset > 65536 {
                return self.fail(format!(
                    "{} descriptor exceeds the 64 KiB near-data budget",
                    declaration.name
                ));
            }
            self.variables.insert(
                canonical(&declaration.name).into(),
                Variable {
                    place: 0,
                    type_id: element,
                    element: Some(element),
                    bounds,
                    indirect: None,
                    descriptor: None,
                    descriptor_place: Some(descriptor_place),
                    descriptor_data: if element == STRING {
                        "near"
                    } else if self.huge_arrays {
                        "split_huge"
                    } else {
                        "split_far"
                    },
                },
            );
            return Ok(0);
        }
        let (type_id, extent, array_element) = if bounds.is_empty() {
            (element, self.width(element), None)
        } else {
            let count = bounds.iter().try_fold(1usize, |total, (low, high)| {
                total
                    .checked_mul((high - low + 1) as usize)
                    .ok_or_else(|| SemanticError {
                        message: format!("{} is too large", declaration.name),
                    })
            })?;
            let extent = count
                .checked_mul(self.width(element))
                .ok_or_else(|| SemanticError {
                    message: format!("{} is too large", declaration.name),
                })?;
            let id = self.array_type(
                format!("{}[]", declaration.name),
                element,
                bounds.clone(),
                extent,
            );
            (id, extent, Some(element))
        };
        if storage == "static" && extent > 65536
            || storage != "static" && self.data_offset + extent > 65536
        {
            return self.fail(format!(
                "{} exceeds the 64 KiB near-data budget",
                declaration.name
            ));
        }
        let (place_offset, place_symbol) = if storage == "static" {
            let symbol = self.next_data;
            self.next_data += 1;
            self.data.push(DataObject {
                id: symbol,
                name: format!("{}$static", declaration.name),
                bytes: vec![0; extent],
                readonly: false,
                relocations: Vec::new(),
                linkage: "internal",
                address: "near",
            });
            (0, symbol)
        } else {
            (
                self.place_offset(storage, extent),
                if matches!(storage, "local" | "parameter") {
                    0
                } else {
                    1
                },
            )
        };
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: place,
            name: declaration.name.clone(),
            type_id,
            offset: place_offset,
            extent,
            storage,
            symbol: place_symbol,
        });
        if storage != "static" {
            self.data_offset += extent;
            self.reserve_module_data(storage);
        }
        let descriptor_place = if array_element.is_some() {
            let descriptor_type = self.opaque_type(
                format!("{} descriptor", declaration.name),
                14 + 4 * bounds.len(),
            );
            let descriptor_extent = self.width(descriptor_type);
            let local_descriptor = matches!(storage, "local" | "parameter");
            let (descriptor_offset, descriptor_storage, descriptor_symbol) = if local_descriptor {
                (self.place_offset(storage, descriptor_extent), storage, 0)
            } else {
                let symbol = self.static_array_descriptor(
                    &declaration.name,
                    place_symbol,
                    place_offset as usize,
                    self.width(element),
                    &bounds,
                );
                (0, "static", symbol)
            };
            let descriptor = self.next_place;
            self.next_place += 1;
            self.places.push(Place {
                id: descriptor,
                name: format!("{}$descriptor", declaration.name),
                type_id: descriptor_type,
                offset: descriptor_offset,
                extent: descriptor_extent,
                storage: descriptor_storage,
                symbol: descriptor_symbol,
            });
            if local_descriptor {
                self.data_offset += descriptor_extent;
            }
            if self.data_offset > 65536 {
                return self.fail(format!(
                    "{} descriptor exceeds the 64 KiB near-data budget",
                    declaration.name
                ));
            }
            Some(descriptor)
        } else {
            None
        };
        self.variables.insert(
            canonical(&declaration.name).into(),
            Variable {
                place,
                type_id,
                element: array_element,
                bounds,
                indirect: None,
                descriptor: None,
                descriptor_place,
                descriptor_data: "none",
            },
        );
        Ok(place)
    }

    fn reserve_module_data(&mut self, storage: &str) {
        if !matches!(storage, "local" | "parameter") {
            self.data[0].bytes.resize(self.data_offset, 0);
        }
    }

    fn static_array_descriptor(
        &mut self,
        name: &str,
        data_symbol: u32,
        data_offset: usize,
        element_width: usize,
        bounds: &[(i64, i64)],
    ) -> u32 {
        // Microsoft ARRAY.INC's AD layout is part of the public BASIC ABI.
        // BASIC startup clears BC_DATA, so BC puts the immutable descriptor
        // in BC_CN and points it at the mutable array storage in BC_DATA.
        // FADF_STATIC says that storage is not runtime-owned.
        let symbol = self.next_data;
        self.next_data += 1;
        let mut bytes = vec![0; 14 + 4 * bounds.len()];
        bytes[8] = bounds.len() as u8;
        bytes[9] = 0x40;

        bytes[12..14].copy_from_slice(&(element_width as u16).to_le_bytes());
        // Q45A05's QB 4.5 BC_CN bytes are 03 00 01 00 then
        // 02 00 01 00 for source bounds (1 TO 2, 1 TO 3). B$LBND/B$UBND
        // count backward from AD_cDims, so the ABI stores dimensions in
        // reverse source order regardless of /R element ordering.
        for (dimension, (low, high)) in bounds.iter().rev().enumerate() {
            let at = 14 + 4 * dimension;
            let count = (high - low + 1) as u16;
            bytes[at..at + 2].copy_from_slice(&count.to_le_bytes());
            bytes[at + 2..at + 4].copy_from_slice(&(*low as u16).to_le_bytes());
        }
        self.data.push(DataObject {
            id: symbol,
            name: format!("{name}$descriptor"),
            bytes,
            readonly: true,
            relocations: vec![
                DataRelocation {
                    at: 0,
                    target: data_symbol,
                    addend: data_offset,
                    address: "far",
                },
                // The same measured object carries an offset16 relocation at
                // AD_oAdjusted (+10) to BC_DATA+data_offset. It is the array
                // data address, not a host-computed lower-bound bias.
                DataRelocation {
                    at: 10,
                    target: data_symbol,
                    addend: data_offset,
                    address: "near",
                },
            ],
            linkage: "internal",
            address: "near",
        });
        symbol
    }

    fn place_offset(&self, storage: &str, extent: usize) -> isize {
        if matches!(storage, "local" | "parameter") {
            -((self.data_offset + extent) as isize)
        } else {
            self.data_offset as isize
        }
    }

    fn reserve_labels(&mut self, statements: &[Statement]) -> Result<(), SemanticError> {
        for statement in statements {
            match statement {
                Statement::Label(name, _) => {
                    let block = self.new_block();
                    if self.labels.insert(name.clone(), block).is_some() {
                        return self.fail(format!("duplicate label {name}"));
                    }
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.reserve_labels(then_branch)?;
                    self.reserve_labels(else_branch)?;
                }
                Statement::For { body, .. } => self.reserve_labels(body)?,
                Statement::While { body, .. } | Statement::Do { body, .. } => {
                    self.reserve_labels(body)?
                }
                Statement::Select {
                    arms, otherwise, ..
                } => {
                    for (_, body) in arms {
                        self.reserve_labels(body)?;
                    }
                    self.reserve_labels(otherwise)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn statements(&mut self, module: &Module) -> Result<(), SemanticError> {
        self.statement_list(&module.statements)
    }

    fn begin_resumable_statement(&mut self, line: u16) {
        if !self.blocks[self.current_block].instructions.is_empty()
            || self.blocks[self.current_block].terminator.is_some()
        {
            let block = self.new_block();
            self.jump_if_open(block);
            self.select_block(block);
        }
        let block = self.blocks[self.current_block].id;
        let instruction = self.next_instruction;
        if self.statement_entries.last() != Some(&(block, instruction, line)) {
            self.statement_entries.push((block, instruction, line));
        }
    }

    fn statement_list(&mut self, statements: &[Statement]) -> Result<(), SemanticError> {
        for statement in statements {
            let metadata_only = matches!(
                statement,
                Statement::Dim(_)
                    | Statement::Static(_)
                    | Statement::DefType { .. }
                    | Statement::TypeDecl { .. }
                    | Statement::Const { .. }
                    | Statement::Comment(_)
                    | Statement::OptionExplicit(_)
                    | Statement::OptionBase(_, _)
                    | Statement::Data { .. }
                    | Statement::Label(_, _)
                    | Statement::OnError { .. }
            );
            if self.error_handler.is_some() && !metadata_only {
                // A numbered BASIC line remains the active ERL value for
                // every following statement until another numeric label.
                // PDS 7.1 BC emits that number on every statement-table row;
                // consuming it here made ERROR 53 followed by an unnumbered
                // handler report ERL 0 through B$FERL.
                let line = self.pending_numeric_line.unwrap_or(0);
                self.begin_resumable_statement(line);
            }
            match statement {
                Statement::Dim(_)
                | Statement::Static(_)
                | Statement::DefType { .. }
                | Statement::TypeDecl { .. }
                | Statement::Const { .. }
                | Statement::Comment(_)
                | Statement::OptionExplicit(_)
                | Statement::OptionBase(_, _) => {}
                Statement::Redim(items) => {
                    for item in items {
                        self.redim(item)?;
                    }
                }
                Statement::Erase(items) => {
                    for item in items {
                        let name = match item {
                            Expr::Name(name, _) => name,
                            Expr::Apply {
                                name, arguments, ..
                            } if arguments.is_empty() => name,
                            _ => return self.fail("ERASE requires bare array names"),
                        };
                        let variable = self.variable(name)?;
                        if variable.element.is_none() {
                            return self.fail(format!("ERASE target {name} is not an array"));
                        }
                        if variable.descriptor.is_none() && variable.descriptor_place.is_none() {
                            return self.fail(format!(
                                "static-array ERASE for {name} requires native zero-fill lowering"
                            ));
                        }
                        let descriptor = self.descriptor_pointer(&variable)?;
                        self.emit_runtime_call(
                            "B$ERAS",
                            Vec::new(),
                            vec![Operand::Value(descriptor)],
                        );
                    }
                }
                Statement::Assign { target, value, .. } => {
                    if let Expr::Apply {
                        name, arguments, ..
                    } = target
                    {
                        if intrinsics::find(canonical(name), self.dialect)
                            .is_some_and(|intrinsic| intrinsic.lowering == Lowering::Mid)
                        {
                            self.mid_assignment(arguments, value)?;
                            continue;
                        }
                    }
                    let (destination, destination_type) =
                        self.destination(target).map_err(|error| SemanticError {
                            message: format!(
                                "line {} assignment: {}",
                                target.span().line,
                                error.message
                            ),
                        })?;
                    if self.string_width(destination_type).is_some() {
                        self.string_assignment(destination, destination_type, value)?;
                        continue;
                    }
                    if self
                        .udts
                        .values()
                        .any(|record| record.type_id == destination_type)
                    {
                        let (source, source_type) = self.destination(value)?;
                        if source_type != destination_type {
                            return self.fail("record assignment requires identical types");
                        }
                        self.aggregate_assignment(
                            destination,
                            source,
                            self.width(destination_type),
                        )?;
                        continue;
                    }
                    let (source, source_type) = self.expression(value)?;
                    let source = self.convert(source, source_type, destination_type)?;
                    self.emit("store", Vec::new(), vec![destination, source]);
                }
                Statement::Label(name, _) => {
                    let target = *self.labels.get(name).ok_or_else(|| SemanticError {
                        message: format!("unknown label {name}"),
                    })?;
                    self.jump_if_open(target);
                    self.select_block(target);
                    // A symbolic label changes control flow but not BASIC's
                    // active numbered line. Microsoft PDS keeps the last
                    // numeric line on statement-table rows after `missing:`;
                    // clearing it here made ERL become zero in that handler.
                    if let Ok(line) = name.parse::<u16>() {
                        self.pending_numeric_line = Some(line);
                    }
                }
                Statement::Goto(name, _) => {
                    let target = *self.labels.get(name).ok_or_else(|| SemanticError {
                        message: format!("unknown label {name}"),
                    })?;
                    self.terminate("jump", Vec::new(), vec![target])?;
                    let continuation = self.new_block();
                    self.select_block(continuation);
                }
                Statement::CallOrGoto(name, _) => {
                    if self.signatures.contains_key(canonical(name)) {
                        self.call(name, &[], false)?;
                    } else {
                        let target = *self.labels.get(name).ok_or_else(|| SemanticError {
                            message: format!("unknown label or zero-argument procedure {name}"),
                        })?;
                        self.terminate("jump", Vec::new(), vec![target])?;
                        let continuation = self.new_block();
                        self.select_block(continuation);
                    }
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => self.if_statement(condition, then_branch, else_branch)?,
                Statement::For {
                    counter,
                    start,
                    end,
                    step,
                    body,
                    ..
                } => self.for_statement(counter, start, end, step.as_ref(), body)?,
                Statement::While {
                    condition, body, ..
                } => self.while_statement(condition, body)?,
                Statement::Do {
                    pre, post, body, ..
                } => self.do_statement(pre.as_ref(), post.as_ref(), body)?,
                Statement::Select {
                    selector,
                    arms,
                    otherwise,
                    ..
                } => self.select_statement(selector, arms, otherwise)?,
                Statement::Call {
                    name, arguments, ..
                } => match name.as_str() {
                    "BLOAD" | "BSAVE" => self.binary_memory_statement(name, arguments)?,
                    "RANDOMIZE" => {
                        let [seed] = arguments.as_slice() else {
                            return self.fail(
                                "the audited RANDOMIZE form requires one seed expression",
                            );
                        };
                        let (seed, seed_type) = self.expression(seed)?;
                        let seed = self.convert(seed, seed_type, DOUBLE)?;
                        // VBDOS BC emits the seed as an eight-byte R8 value,
                        // high dword first, then calls B$RNZP. Preserve the
                        // rounding boundary in a typed temporary so ordinary
                        // runtime-call lowering materializes those stack bytes.
                        let place = self.temporary(DOUBLE)?;
                        self.emit("store", Vec::new(), vec![Operand::Place(place), seed]);
                        self.emit_runtime_call("B$RNZP", Vec::new(), vec![Operand::Place(place)]);
                    }
                    _ => {
                        self.call(name, arguments, false)?;
                    }
                },
                Statement::DefSeg { value, .. } => {
                    if let Some(value) = value {
                        let (value, type_id) = self.expression(value)?;
                        let value = self.convert(value, type_id, INTEGER)?;
                        // B$DSEG only stores this word in the runtime-owned
                        // b$seg cell.  PEEK/POKE in every procedure and module
                        // load that same external cell, so expose the store
                        // directly instead of hiding it behind a runtime call.
                        let place = self.def_segment_place();
                        self.emit("store", Vec::new(), vec![Operand::Place(place), value]);
                    } else {
                        // rt/rtinit.asm's B$DSG0 is the distinct bare-DEF-SEG
                        // operation: copy DS into b$seg. HIR has no machine
                        // segment-register value, so retain the audited runtime
                        // call rather than inventing an ordinary integer load.
                        self.emit_runtime_call("B$DSG0", Vec::new(), Vec::new());
                    }
                }
                Statement::Open {
                    path, mode, file, ..
                } => {
                    let path = self.string_descriptor(path)?;
                    let (file, file_type) = self.expression(file)?;
                    let file = self.convert(file, file_type, INTEGER)?;
                    let mode = match mode {
                        FileMode::Input => 1,
                        FileMode::Output => 2,
                        FileMode::Append => 8,
                        FileMode::Binary => 0x20,
                    };
                    self.emit_runtime_call(
                        "B$OPEN",
                        Vec::new(),
                        vec![
                            path,
                            file,
                            Operand::Constant(INTEGER, Number::Integer(-1)),
                            Operand::Constant(INTEGER, Number::Integer(mode)),
                        ],
                    );
                }
                Statement::Close { files, .. } => {
                    let mut operands = Vec::new();
                    for file in files {
                        let (file, file_type) = self.expression(file)?;
                        operands.push(self.convert(file, file_type, INTEGER)?);
                    }
                    operands.push(Operand::Constant(
                        INTEGER,
                        Number::Integer(files.len() as i64),
                    ));
                    self.emit_runtime_call("B$CLOS", Vec::new(), operands);
                }
                Statement::LineInput {
                    file, destination, ..
                } => {
                    let (file, file_type) = self.expression(file)?;
                    let file = self.convert(file, file_type, INTEGER)?;
                    self.emit_runtime_call("B$DSKI", Vec::new(), vec![file]);

                    let (destination, destination_type) = self.destination(destination)?;
                    if destination_type != STRING {
                        return self.fail("LINE INPUT destination must be a dynamic STRING");
                    }
                    let destination = self.far_address(destination, destination_type);
                    self.emit_runtime_call(
                        "B$LNIN",
                        Vec::new(),
                        vec![
                            Operand::Constant(INTEGER, Number::Integer(0)),
                            Operand::Value(destination),
                            Operand::Constant(INTEGER, Number::Integer(0)),
                            Operand::Constant(INTEGER, Number::Integer(1)),
                        ],
                    );
                }
                Statement::FileTransfer {
                    write,
                    file,
                    position,
                    target,
                    ..
                } => {
                    let (file, file_type) = self.expression(file)?;
                    let file = self.convert(file, file_type, INTEGER)?;
                    let (target, target_type) = self.destination(target)?;
                    let address = self.far_address(target, target_type);
                    let width = self
                        .string_width(target_type)
                        .unwrap_or_else(|| self.width(target_type));
                    let mut operands = vec![file];
                    if let Some(position) = position {
                        let (position, position_type) = self.expression(position)?;
                        operands.push(self.convert(position, position_type, LONG)?);
                    }
                    operands.extend([
                        Operand::Value(address),
                        Operand::Constant(INTEGER, Number::Integer(width as i64)),
                    ]);
                    self.emit_runtime_call(
                        match (*write, position.is_some()) {
                            (false, false) => "B$GET3",
                            (true, false) => "B$PUT3",
                            (false, true) => "B$GET4",
                            (true, true) => "B$PUT4",
                        },
                        Vec::new(),
                        operands,
                    );
                }
                Statement::Seek { file, position, .. } => {
                    let (file, file_type) = self.expression(file)?;
                    let file = self.convert(file, file_type, INTEGER)?;
                    let (position, position_type) = self.expression(position)?;
                    let position = self.convert(position, position_type, LONG)?;
                    self.emit_runtime_call("B$SSEK", Vec::new(), vec![file, position]);
                }
                Statement::Print { file, items, .. } => {
                    if let Some(file) = file {
                        let (file, file_type) = self.expression(file)?;
                        let file = self.convert(file, file_type, INTEGER)?;
                        self.emit_runtime_call("B$CHOU", Vec::new(), vec![file]);
                    }
                    if items.is_empty() {
                        self.emit_runtime_call(
                            "B$PESD",
                            Vec::new(),
                            vec![Operand::Constant(INTEGER, Number::Integer(0))],
                        );
                        continue;
                    }
                    for item in items {
                        let term = match item.separator {
                            PrintSeparator::Comma => 'C',
                            PrintSeparator::Semicolon => 'S',
                            PrintSeparator::End => 'E',
                        };
                        let (suffix, operand) = if self.string_syntax(&item.value) {
                            ("SD", self.string_descriptor(&item.value)?)
                        } else {
                            let (operand, type_id) = self.expression(&item.value)?;
                            match type_id {
                                INTEGER | BOOLEAN | BYTE => {
                                    ("I2", self.convert(operand, type_id, INTEGER)?)
                                }
                                LONG => ("I4", operand),
                                SINGLE | DOUBLE => {
                                    let place = self.temporary(type_id)?;
                                    self.emit(
                                        "store",
                                        Vec::new(),
                                        vec![Operand::Place(place), operand],
                                    );
                                    (
                                        if type_id == SINGLE { "R4" } else { "R8" },
                                        Operand::Place(place),
                                    )
                                }
                                _ => return self.fail("PRINT item has an unsupported type"),
                            }
                        };
                        self.emit_runtime_call(
                            &format!("B$P{term}{suffix}"),
                            Vec::new(),
                            vec![operand],
                        );
                    }
                    if items
                        .last()
                        .is_some_and(|item| item.separator != PrintSeparator::End)
                    {
                        self.emit_runtime_call("B$PEOS", Vec::new(), Vec::new());
                    }
                }
                Statement::Input {
                    file, destinations, ..
                } => {
                    let Some(file) = file else {
                        return self.fail("console INPUT requires the audited prompt protocol");
                    };
                    let (file, file_type) = self.expression(file)?;
                    let file = self.convert(file, file_type, INTEGER)?;
                    self.emit_runtime_call("B$DSKI", Vec::new(), vec![file]);
                    for destination in destinations {
                        let (place, type_id) = self.destination(destination)?;
                        let address = self.far_address(place, type_id);
                        let (callee, operands) = match type_id {
                            INTEGER | BOOLEAN | BYTE => ("B$RDI2", vec![Operand::Value(address)]),
                            LONG => ("B$RDI4", vec![Operand::Value(address)]),
                            SINGLE => ("B$RDR4", vec![Operand::Value(address)]),
                            DOUBLE => ("B$RDR8", vec![Operand::Value(address)]),
                            _ if self.string_width(type_id).is_some() => {
                                let width = self.string_width(type_id).unwrap_or(0);
                                (
                                    "B$RDSD",
                                    vec![
                                        Operand::Value(address),
                                        Operand::Constant(INTEGER, Number::Integer(width as i64)),
                                    ],
                                )
                            }
                            _ => return self.fail("INPUT destination has an unsupported type"),
                        };
                        self.emit_runtime_call(callee, Vec::new(), operands);
                    }
                    self.emit_runtime_call("B$PEOS", Vec::new(), Vec::new());
                }
                Statement::Data { values, .. } => self.append_read_data(values)?,
                Statement::Read { destinations, .. } => {
                    for destination in destinations {
                        let (place, type_id) = self.destination(destination)?;
                        let address = self.far_address(place, type_id);
                        let (callee, operands) = match type_id {
                            INTEGER | BOOLEAN | BYTE => ("B$RDI2", vec![Operand::Value(address)]),
                            LONG => ("B$RDI4", vec![Operand::Value(address)]),
                            SINGLE => ("B$RDR4", vec![Operand::Value(address)]),
                            DOUBLE => ("B$RDR8", vec![Operand::Value(address)]),
                            _ if self.string_width(type_id).is_some() => {
                                let width = self.string_width(type_id).unwrap_or(0);
                                (
                                    "B$RDSD",
                                    vec![
                                        Operand::Value(address),
                                        Operand::Constant(INTEGER, Number::Integer(width as i64)),
                                    ],
                                )
                            }
                            _ => return self.fail("READ destination has an unsupported type"),
                        };
                        self.emit_runtime_call(callee, Vec::new(), operands);
                    }
                }
                Statement::OnError { label, local, .. } => {
                    let handler = *self.labels.get(label).ok_or_else(|| SemanticError {
                        message: format!("unknown ON ERROR label {label}"),
                    })?;
                    if self.error_handler.replace(handler).is_some() {
                        return self.fail("multiple ON ERROR registrations in one procedure");
                    }
                    self.error_handler_local = *local;
                    // Registration is function/module side metadata. The OMF
                    // adapter materializes B$OEGA or the procedure-local
                    // B$OEGP with a relocated handler after code layout; no
                    // fake ordinary CFG edge enters MIR.
                }
                Statement::Resume { target, .. } => {
                    match target {
                        ResumeTarget::Next => {
                            // B$RESN validates that an error is active, clears
                            // the handler state, searches MODULE_CODE.OF_STA,
                            // unwinds the active BASIC frame, and transfers to
                            // the next statement. It never returns here.
                            self.emit_runtime_call("B$RESN", Vec::new(), Vec::new());
                        }
                        ResumeTarget::Current => {
                            return self.fail(
                                "RESUME without a target requires the audited B$RES0 statement map",
                            );
                        }
                        ResumeTarget::Label(label) => {
                            let target = *self.labels.get(label).ok_or_else(|| SemanticError {
                                message: format!("unknown RESUME label {label}"),
                            })?;
                            // B$RESA receives the relocated code offset in AX,
                            // not on the Pascal stack. Carry only the semantic
                            // block identity here; the OMF adapter inserts the
                            // physical AX move once final statement labels exist.
                            self.emit_runtime_call(
                                &format!("$QB$RESA:{target}"),
                                Vec::new(),
                                Vec::new(),
                            );
                        }
                    }
                    self.terminate("unreachable", Vec::new(), Vec::new())?;
                    let continuation = self.new_block();
                    self.select_block(continuation);
                }
                Statement::Runtime {
                    name, arguments, ..
                } => match name.as_str() {
                    "ERROR" => {
                        if arguments.len() != 1 {
                            return self.fail("ERROR expects one error number");
                        }
                        let (number, type_id) = self.expression(&arguments[0])?;
                        let number = self.convert(number, type_id, INTEGER)?;
                        self.emit_runtime_call("B$SERR", Vec::new(), vec![number]);
                        self.terminate("unreachable", Vec::new(), Vec::new())?;
                        let continuation = self.new_block();
                        self.select_block(continuation);
                    }
                    "CLS" => {
                        if arguments.len() > 1 {
                            return self.fail("CLS expects zero or one screen selector");
                        }
                        let selector = if let Some(argument) = arguments.first() {
                            let (value, type_id) = self.expression(argument)?;
                            self.convert(value, type_id, INTEGER)?
                        } else {
                            // QB45 rt/gwscr.asm increments the argument and
                            // treats -1 as the no-parameter form.
                            Operand::Constant(INTEGER, Number::Integer(-1))
                        };
                        self.emit_runtime_call("B$SCLS", Vec::new(), vec![selector]);
                    }
                    "POKE" => {
                        if arguments.len() != 2 {
                            return self.fail("POKE expects an offset and byte value");
                        }
                        let (offset, offset_type) = self.expression(&arguments[0])?;
                        let offset = self.convert(offset, offset_type, INTEGER)?;
                        let (value, value_type) = self.expression(&arguments[1])?;
                        let value = self.convert(value, value_type, BYTE)?;
                        let segment_place = self.def_segment_place();
                        let segment = self.value(INTEGER);
                        self.emit("load", vec![segment], vec![Operand::Place(segment_place)]);
                        let pointer_type = self.far_pointer_type(BYTE);
                        let pointer = self.value(pointer_type);
                        self.emit(
                            "concat",
                            vec![pointer],
                            vec![Operand::Value(segment), offset],
                        );
                        self.emit(
                            "store",
                            Vec::new(),
                            vec![
                                Operand::Indirect {
                                    base: pointer,
                                    offset: 0,
                                    type_id: BYTE,
                                },
                                value,
                            ],
                        );
                    }
                    "SWAP" => {
                        let [left, right] = arguments.as_slice() else {
                            return self.fail("SWAP expects two variables");
                        };
                        let (left, left_type) = self.destination(left)?;
                        let (right, right_type) = self.destination(right)?;
                        if left_type != right_type {
                            return self.fail("SWAP variables must have identical types");
                        }
                        let aggregate = self
                            .udts
                            .values()
                            .any(|record| record.type_id == left_type)
                            || self.string_width(left_type).is_some_and(|width| width != 0);
                        if aggregate {
                            let temporary = Operand::Place(self.temporary(left_type)?);
                            let width = self.width(left_type);
                            self.aggregate_assignment(temporary.clone(), left.clone(), width)?;
                            self.aggregate_assignment(left, right.clone(), width)?;
                            self.aggregate_assignment(right, temporary, width)?;
                        } else {
                            let left_value = self.value(left_type);
                            self.emit("load", vec![left_value], vec![left.clone()]);
                            let right_value = self.value(right_type);
                            self.emit("load", vec![right_value], vec![right.clone()]);
                            self.emit(
                                "store",
                                Vec::new(),
                                vec![left, Operand::Value(right_value)],
                            );
                            self.emit(
                                "store",
                                Vec::new(),
                                vec![right, Operand::Value(left_value)],
                            );
                        }
                    }
                    "SCREEN" => {
                        if arguments.len() != 1 {
                            return self.fail("the audited SCREEN form requires one mode");
                        }
                        let (mode, mode_type) = self.expression(&arguments[0])?;
                        let mode = self.convert(mode, mode_type, INTEGER)?;
                        // SYS.OBJ 0787..0792 shows SCREEN 0 as the VBDOS
                        // count-led block [present=1, mode=0, count=2].
                        self.emit_runtime_call(
                            "B$CSCN",
                            Vec::new(),
                            vec![
                                Operand::Constant(INTEGER, Number::Integer(1)),
                                mode,
                                Operand::Constant(INTEGER, Number::Integer(2)),
                            ],
                        );
                    }
                    "WIDTH" => {
                        if arguments.len() != 2 {
                            return self.fail("the audited WIDTH form requires columns and rows");
                        }
                        let mut operands = Vec::new();
                        for argument in arguments {
                            let (value, type_id) = self.expression(argument)?;
                            operands.push(self.convert(value, type_id, INTEGER)?);
                        }
                        self.emit_runtime_call("B$WIDT", Vec::new(), operands);
                    }
                    "SLEEP" => {
                        if arguments.len() > 1 {
                            return self.fail("SLEEP expects zero or one duration");
                        }
                        let duration = if let Some(argument) = arguments.first() {
                            let (value, type_id) = self.expression(argument)?;
                            self.convert(value, type_id, LONG)?
                        } else {
                            Operand::Constant(LONG, Number::Integer(0))
                        };
                        self.emit_runtime_call("B$SLEP", Vec::new(), vec![duration]);
                    }
                    "END" | "SYSTEM" => {
                        if !arguments.is_empty() {
                            return self.fail(format!("{name} takes no arguments"));
                        }
                        self.emit_runtime_call("B$CEND", Vec::new(), Vec::new());
                        // B$CEND does not return. Keep following labels as
                        // detached side entries (notably ON ERROR handlers)
                        // instead of inventing a fallthrough from END.
                        self.terminate("unreachable", Vec::new(), Vec::new())?;
                        let continuation = self.new_block();
                        self.select_block(continuation);
                    }
                    _ => {
                        return self.fail(format!(
                            "{name} is parsed as a runtime statement but has no audited ABI contract"
                        ));
                    }
                },
                Statement::Exit(ExitTarget::Sub | ExitTarget::Function, _) => {
                    if self.result_place.is_some() {
                        let target = self.return_block();
                        self.terminate("jump", Vec::new(), vec![target])?;
                    } else {
                        self.terminate("return", Vec::new(), Vec::new())?;
                    }
                    let continuation = self.new_block();
                    self.select_block(continuation);
                }
                Statement::Exit(target @ (ExitTarget::Do | ExitTarget::For), _) => {
                    let destination = self
                        .exits
                        .iter()
                        .rev()
                        .find_map(|(kind, block)| (*kind == *target).then_some(*block))
                        .ok_or_else(|| SemanticError {
                            message: format!("EXIT {target:?} appears outside its loop"),
                        })?;
                    self.terminate("jump", Vec::new(), vec![destination])?;
                    let continuation = self.new_block();
                    self.select_block(continuation);
                }
            }
        }
        Ok(())
    }

    fn append_read_data(&mut self, values: &[Expr]) -> Result<(), SemanticError> {
        let line = values
            .iter()
            .map(|value| self.read_data_constant(value))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let index = if let Some(index) = self
            .data
            .iter()
            .position(|object| object.name == READ_DATA_OBJECT)
        {
            index
        } else {
            let symbol = self.next_data;
            self.next_data += 1;
            self.data.push(DataObject {
                id: symbol,
                name: READ_DATA_OBJECT.into(),
                bytes: Vec::new(),
                readonly: true,
                relocations: Vec::new(),
                linkage: "internal",
                address: "near",
            });
            self.data.len() - 1
        };
        let bytes = &mut self.data[index].bytes;
        bytes.push(b' ');
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(0);
        Ok(())
    }

    fn read_data_constant(&self, value: &Expr) -> Result<String, SemanticError> {
        match value {
            Expr::Literal(Literal::Integer(value, _), _) => Ok(value.to_string()),
            Expr::Literal(Literal::Real(value, _), _) => Ok(value.clone()),
            Expr::Literal(Literal::String(value), _) => {
                if !value.is_ascii() || value.contains(['"', '\0']) {
                    return self.fail(
                        "DATA strings require ASCII without embedded quotes in this audited form",
                    );
                }
                Ok(format!("\"{value}\""))
            }
            Expr::Unary { op, operand, .. } if matches!(op, Unary::Positive | Unary::Negative) => {
                let literal = self.read_data_constant(operand)?;
                Ok(if *op == Unary::Negative {
                    format!("-{literal}")
                } else {
                    literal
                })
            }
            _ => self.fail("DATA accepts only numeric and string constants"),
        }
    }

    fn if_statement(
        &mut self,
        condition: &Expr,
        then_branch: &[Statement],
        else_branch: &[Statement],
    ) -> Result<(), SemanticError> {
        let then_block = self.new_block();
        let else_block = self.new_block();
        let join_block = self.new_block();
        self.condition(condition, then_block, else_block, true)?;

        self.select_block(then_block);
        self.statement_list(then_branch)?;
        self.jump_if_open(join_block);

        self.select_block(else_block);
        self.statement_list(else_branch)?;
        self.jump_if_open(join_block);

        self.select_block(join_block);
        Ok(())
    }

    fn select_statement(
        &mut self,
        selector: &Expr,
        arms: &[(Vec<Expr>, Vec<Statement>)],
        otherwise: &[Statement],
    ) -> Result<(), SemanticError> {
        let string_selector = self.string_syntax(selector);
        let (selector, selector_type) = if string_selector {
            (self.string_descriptor(selector)?, STRING)
        } else {
            self.expression(selector)?
        };
        if !string_selector && !matches!(selector_type, INTEGER | LONG | SINGLE | DOUBLE) {
            return self.fail("SELECT CASE selector must be numeric or string");
        }
        let done = self.new_block();
        for (matches, body) in arms {
            if matches.is_empty() {
                return self.fail("CASE arm has no values");
            }
            let body_block = self.new_block();
            let next_arm = self.new_block();
            for (index, candidate) in matches.iter().enumerate() {
                let equal = self.value(BOOLEAN);
                if string_selector {
                    if !self.string_syntax(candidate) {
                        return self.fail("string SELECT CASE requires string case values");
                    }
                    let candidate = self.string_descriptor(candidate)?;
                    self.emit_string_compare("string_eq", equal, selector.clone(), candidate);
                } else {
                    let (candidate, candidate_type) = self.expression(candidate)?;
                    let candidate = self.convert(candidate, candidate_type, selector_type)?;
                    self.emit("eq", vec![equal], vec![selector.clone(), candidate]);
                }
                let no_match = if index + 1 == matches.len() {
                    next_arm
                } else {
                    self.new_block()
                };
                self.terminate(
                    "branch",
                    vec![Operand::Value(equal)],
                    vec![body_block, no_match],
                )?;
                if no_match != next_arm {
                    self.select_block(no_match);
                }
            }
            self.select_block(body_block);
            self.statement_list(body)?;
            self.jump_if_open(done);
            self.select_block(next_arm);
        }
        self.statement_list(otherwise)?;
        self.jump_if_open(done);
        self.select_block(done);
        Ok(())
    }

    fn for_statement(
        &mut self,
        counter: &Expr,
        start: &Expr,
        end: &Expr,
        step: Option<&Expr>,
        body: &[Statement],
    ) -> Result<(), SemanticError> {
        let (destination, counter_type) = self.destination(counter)?;
        if !matches!(counter_type, INTEGER | LONG | SINGLE | DOUBLE) {
            return self.fail("FOR counter is not numeric");
        }
        let (start_value, start_type) = self.expression(start)?;
        let start_value = self.convert(start_value, start_type, counter_type)?;
        self.emit("store", Vec::new(), vec![destination.clone(), start_value]);
        let (end_value, end_type) = self.expression(end)?;
        let end_value = self.convert(end_value, end_type, counter_type)?;
        let end_place = self.compiler_temporary("$forEnd", counter_type)?;
        self.emit(
            "store",
            Vec::new(),
            vec![Operand::Place(end_place), end_value],
        );
        let (step_value, step_type) = match step {
            Some(step) => self.expression(step)?,
            None => (Operand::Constant(INTEGER, Number::Integer(1)), INTEGER),
        };
        let step_value = self.convert(step_value, step_type, counter_type)?;
        let step_place = self.compiler_temporary("$forStep", counter_type)?;
        self.emit(
            "store",
            Vec::new(),
            vec![Operand::Place(step_place), step_value],
        );

        let test_block = self.new_block();
        let positive_test = self.new_block();
        let negative_test = self.new_block();
        let body_block = self.new_block();
        let done_block = self.new_block();
        self.terminate("jump", Vec::new(), vec![test_block])?;
        self.select_block(test_block);
        let step_value = self.value(counter_type);
        self.emit(
            "load",
            vec![step_value],
            vec![Operand::Place(step_place)],
        );
        let direction = self.value(BOOLEAN);
        let zero = if matches!(counter_type, SINGLE | DOUBLE) {
            self.floating_literal("0.0", counter_type)?
        } else {
            Operand::Constant(counter_type, Number::Integer(0))
        };
        self.emit(
            "ge",
            vec![direction],
            vec![Operand::Value(step_value), zero],
        );
        self.terminate(
            "branch",
            vec![Operand::Value(direction)],
            vec![positive_test, negative_test],
        )?;

        self.select_block(positive_test);
        let counter_value = self.load_destination(counter)?;
        let end_value = self.value(counter_type);
        self.emit(
            "load",
            vec![end_value],
            vec![Operand::Place(end_place)],
        );
        let within = self.value(BOOLEAN);
        self.emit(
            "le",
            vec![within],
            vec![counter_value, Operand::Value(end_value)],
        );
        self.terminate(
            "branch",
            vec![Operand::Value(within)],
            vec![body_block, done_block],
        )?;

        self.select_block(negative_test);
        let counter_value = self.load_destination(counter)?;
        let end_value = self.value(counter_type);
        self.emit(
            "load",
            vec![end_value],
            vec![Operand::Place(end_place)],
        );
        let within = self.value(BOOLEAN);
        self.emit(
            "ge",
            vec![within],
            vec![counter_value, Operand::Value(end_value)],
        );
        self.terminate(
            "branch",
            vec![Operand::Value(within)],
            vec![body_block, done_block],
        )?;

        self.select_block(body_block);
        self.exits.push((ExitTarget::For, done_block));
        self.statement_list(body)?;
        self.exits.pop();
        if self.block_open() {
            let counter_value = self.load_destination(counter)?;
            let step_value = self.value(counter_type);
            self.emit(
                "load",
                vec![step_value],
                vec![Operand::Place(step_place)],
            );
            let advanced = self.value(counter_type);
            self.emit(
                if matches!(counter_type, SINGLE | DOUBLE) {
                    "fadd"
                } else {
                    "add"
                },
                vec![advanced],
                vec![counter_value, Operand::Value(step_value)],
            );
            self.emit(
                "store",
                Vec::new(),
                vec![destination, Operand::Value(advanced)],
            );
            self.terminate("jump", Vec::new(), vec![test_block])?;
        }
        self.select_block(done_block);
        Ok(())
    }

    fn while_statement(
        &mut self,
        condition: &Expr,
        body: &[Statement],
    ) -> Result<(), SemanticError> {
        let test_block = self.new_block();
        let body_block = self.new_block();
        let done_block = self.new_block();
        self.terminate("jump", Vec::new(), vec![test_block])?;
        self.select_block(test_block);
        self.condition(condition, body_block, done_block, true)?;
        self.select_block(body_block);
        self.statement_list(body)?;
        self.jump_if_open(test_block);
        self.select_block(done_block);
        Ok(())
    }

    fn do_statement(
        &mut self,
        pre: Option<&(bool, Expr)>,
        post: Option<&(bool, Expr)>,
        body: &[Statement],
    ) -> Result<(), SemanticError> {
        let test_block = self.new_block();
        let body_block = self.new_block();
        let done_block = self.new_block();
        if let Some((while_true, condition)) = pre {
            self.terminate("jump", Vec::new(), vec![test_block])?;
            self.select_block(test_block);
            self.condition(condition, body_block, done_block, *while_true)?;
        } else {
            self.terminate("jump", Vec::new(), vec![body_block])?;
        }
        self.select_block(body_block);
        self.exits.push((ExitTarget::Do, done_block));
        self.statement_list(body)?;
        self.exits.pop();
        if self.block_open() {
            if let Some((while_true, condition)) = post {
                self.terminate("jump", Vec::new(), vec![test_block])?;
                self.select_block(test_block);
                self.condition(condition, body_block, done_block, *while_true)?;
            } else if pre.is_some() {
                self.terminate("jump", Vec::new(), vec![test_block])?;
            } else {
                self.terminate("jump", Vec::new(), vec![body_block])?;
            }
        }
        self.select_block(done_block);
        Ok(())
    }

    fn condition(
        &mut self,
        expression: &Expr,
        true_target: u32,
        false_target: u32,
        while_true: bool,
    ) -> Result<(), SemanticError> {
        if let Expr::Unary {
            op: Unary::Not,
            operand,
            ..
        } = expression
        {
            // In a control context BC tests NOT by exchanging the operand's
            // successors. It does not materialize the integer complement:
            // NOT &h8000 is &h7fff and therefore cannot implement the common
            // `WHILE NOT (flags AND &h8000)` mask test. This also preserves
            // ordinary bitwise NOT when the expression is used as a value.
            return self.condition(operand, true_target, false_target, !while_true);
        }
        let condition = self.truth(expression)?;
        // Under VBDOS /O, a NOT buried below another operator is still
        // materialized, but the final control transfer is exchanged.  The
        // top-level case above is different: BC strips the NOT entirely.
        let branch_on_true = while_true ^ contains_not(expression);
        let targets = if branch_on_true {
            vec![true_target, false_target]
        } else {
            vec![false_target, true_target]
        };
        self.terminate("branch", vec![condition], targets)
    }

    fn truth(&mut self, expression: &Expr) -> Result<Operand, SemanticError> {
        let (operand, type_id) = self.expression(expression)?;
        if matches!(type_id, INTEGER | LONG | BOOLEAN) {
            return Ok(operand);
        }
        if matches!(type_id, SINGLE | DOUBLE) {
            // BASIC conditions accept every numeric type. Keep the source
            // truth rule explicit: a floating value is true exactly when it
            // compares unequal to zero.
            let zero = self.floating_literal("0", type_id)?;
            let result = self.value(BOOLEAN);
            self.emit("ne", vec![result], vec![operand, zero]);
            return Ok(Operand::Value(result));
        }
        self.fail("condition is not numeric")
    }

    fn load_destination(&mut self, expression: &Expr) -> Result<Operand, SemanticError> {
        let (place, type_id) = self.destination(expression)?;
        let value = self.value(type_id);
        self.emit("load", vec![value], vec![place]);
        Ok(Operand::Value(value))
    }

    fn subplace(
        &self,
        place: &Operand,
        offset: usize,
        type_id: u32,
    ) -> Result<Operand, SemanticError> {
        match place {
            Operand::Place(place) => Ok(Operand::Projection {
                place: *place,
                indices: Vec::new(),
                offset,
                type_id,
            }),
            Operand::Element(place, indices) => Ok(Operand::Projection {
                place: *place,
                indices: indices.clone(),
                offset,
                type_id,
            }),
            Operand::Projection {
                place,
                indices,
                offset: base,
                ..
            } => Ok(Operand::Projection {
                place: *place,
                indices: indices.clone(),
                offset: base + offset,
                type_id,
            }),
            Operand::Indirect {
                base, offset: at, ..
            } => Ok(Operand::Indirect {
                base: *base,
                offset: at + offset,
                type_id,
            }),
            Operand::Value(_) | Operand::Constant(_, _) => {
                self.fail("aggregate copy operand is not a place")
            }
        }
    }

    fn aggregate_assignment(
        &mut self,
        destination: Operand,
        source: Operand,
        width: usize,
    ) -> Result<(), SemanticError> {
        // Load the complete source before storing anything. BYREF arguments
        // may alias, so interleaving chunks would corrupt an overlapping
        // assignment. LONG/INTEGER/BYTE chunks preserve every bit, including
        // record padding, without inventing a machine-width aggregate value.
        let mut loaded = Vec::new();
        let mut offset = 0;
        while offset < width {
            let remaining = width - offset;
            let type_id = if remaining >= 4 {
                LONG
            } else if remaining >= 2 {
                INTEGER
            } else {
                BYTE
            };
            let chunk = self.width(type_id);
            let from = self.subplace(&source, offset, type_id)?;
            let value = self.value(type_id);
            self.emit("load", vec![value], vec![from]);
            loaded.push((offset, type_id, value));
            offset += chunk;
        }
        for (offset, type_id, value) in loaded {
            let to = self.subplace(&destination, offset, type_id)?;
            self.emit("store", Vec::new(), vec![to, Operand::Value(value)]);
        }
        Ok(())
    }

    fn destination(&mut self, expression: &Expr) -> Result<(Operand, u32), SemanticError> {
        match expression {
            Expr::Literal(Literal::String(text), _) => {
                let place = self.string_literal(text)?;
                Ok((Operand::Place(place), STRING))
            }
            Expr::Name(name, _) => {
                if self.constants.contains_key(canonical(name)) {
                    return self.fail(format!("constant {name} is not assignable"));
                }
                let variable = self.variable(name)?;
                if variable.element.is_some() {
                    return self.fail(format!("array {name} requires subscripts"));
                }
                let operand = if let Some(base) = variable.indirect {
                    Operand::Indirect {
                        base,
                        offset: 0,
                        type_id: variable.type_id,
                    }
                } else {
                    Operand::Place(variable.place)
                };
                Ok((operand, variable.type_id))
            }
            Expr::Apply {
                name, arguments, ..
            } => {
                let variable = self.variable(name)?;
                let Some(element) = variable.element else {
                    return self.fail(format!("{name} is not an array"));
                };
                let has_descriptor =
                    variable.descriptor.is_some() || variable.descriptor_place.is_some();
                if has_descriptor && (variable.bounds.is_empty() || self.checked_arrays) {
                    let descriptor = self.descriptor_pointer(&variable)?;
                    let address = if self.checked_arrays {
                        "checked"
                    } else {
                        variable.descriptor_data
                    };
                    let pointer =
                        self.descriptor_element(descriptor, element, arguments, address)?;
                    return Ok((
                        Operand::Indirect {
                            base: pointer,
                            offset: 0,
                            type_id: element,
                        },
                        element,
                    ));
                }
                if arguments.len() != variable.bounds.len() {
                    return self.fail(format!("{name} has the wrong number of subscripts"));
                }
                let mut indices = Vec::new();
                for index in arguments {
                    let (operand, type_id) = self.expression(index)?;
                    if !matches!(type_id, INTEGER | LONG) {
                        return self.fail("array subscript is not integral");
                    }
                    indices.push(operand);
                }
                Ok((Operand::Element(variable.place, indices), element))
            }
            Expr::Index { base, indices, .. } => {
                let (pointer, element) = self.indexed(base, indices)?;
                Ok((
                    Operand::Indirect {
                        base: pointer,
                        offset: 0,
                        type_id: element,
                    },
                    element,
                ))
            }
            Expr::Field { .. } => {
                let (base, offset, type_id) = self.projection(expression)?;
                let operand = match base {
                    ProjectionBase::Place(place, indices) => Operand::Projection {
                        place,
                        indices,
                        offset,
                        type_id,
                    },
                    ProjectionBase::Indirect(base) => Operand::Indirect {
                        base,
                        offset,
                        type_id,
                    },
                };
                Ok((operand, type_id))
            }
            _ => self.fail("assignment target is not a variable or array element"),
        }
    }

    fn projection(
        &mut self,
        expression: &Expr,
    ) -> Result<(ProjectionBase, usize, u32), SemanticError> {
        match expression {
            Expr::Name(name, _) => {
                let variable = self.variable(name)?;
                if variable.element.is_some() {
                    return self.fail(format!("array {name} requires subscripts"));
                }
                let base = variable
                    .indirect
                    .map(ProjectionBase::Indirect)
                    .unwrap_or_else(|| ProjectionBase::Place(variable.place, Vec::new()));
                Ok((base, 0, variable.type_id))
            }
            Expr::Apply {
                name, arguments, ..
            } => {
                let variable = self.variable(name)?;
                let Some(element) = variable.element else {
                    return self.fail(format!("{name} is not an array"));
                };
                let has_descriptor =
                    variable.descriptor.is_some() || variable.descriptor_place.is_some();
                if has_descriptor && (variable.bounds.is_empty() || self.checked_arrays) {
                    let descriptor = self.descriptor_pointer(&variable)?;
                    let address = if self.checked_arrays {
                        "checked"
                    } else {
                        variable.descriptor_data
                    };
                    let pointer =
                        self.descriptor_element(descriptor, element, arguments, address)?;
                    return Ok((ProjectionBase::Indirect(pointer), 0, element));
                }
                if arguments.len() != variable.bounds.len() {
                    return self.fail(format!("{name} has the wrong number of subscripts"));
                }
                let mut indices = Vec::new();
                for index in arguments {
                    let (operand, type_id) = self.expression(index)?;
                    if !matches!(type_id, INTEGER | LONG) {
                        return self.fail("array subscript is not integral");
                    }
                    indices.push(operand);
                }
                Ok((ProjectionBase::Place(variable.place, indices), 0, element))
            }
            Expr::Index { base, indices, .. } => {
                let (pointer, element) = self.indexed(base, indices)?;
                Ok((ProjectionBase::Indirect(pointer), 0, element))
            }
            Expr::Field { base, name, .. } => {
                let (base, offset, base_type) = self.projection(base)?;
                let udt = self
                    .udts
                    .values()
                    .find(|udt| udt.type_id == base_type)
                    .ok_or_else(|| SemanticError {
                        message: format!(
                            "field {name} is selected from non-record {}",
                            self.name(base_type)
                        ),
                    })?;
                let field =
                    udt.fields
                        .get(canonical(name))
                        .cloned()
                        .ok_or_else(|| SemanticError {
                            message: format!("{} has no field {name}", self.name(base_type)),
                        })?;
                Ok((base, offset + field.offset, field.type_id))
            }
            _ => self.fail("field base is not stored in a place"),
        }
    }

    fn indexed(&mut self, base: &Expr, indices: &[Expr]) -> Result<(u32, u32), SemanticError> {
        let (projection, offset, array_type) = self.projection(base)?;
        let array = self
            .types
            .iter()
            .find(|one| one.id == array_type)
            .cloned()
            .ok_or_else(|| SemanticError {
                message: "unknown array type".into(),
            })?;
        let Some(element) = array.element else {
            return self.fail(format!("{} is not an array", self.name(array_type)));
        };
        if indices.len() != array.bounds.len() {
            return self.fail(format!("{} has the wrong number of subscripts", array.name));
        }
        let location = match projection {
            ProjectionBase::Place(place, root_indices) => Operand::Projection {
                place,
                indices: root_indices,
                offset,
                type_id: array_type,
            },
            ProjectionBase::Indirect(base) => Operand::Indirect {
                base,
                offset,
                type_id: array_type,
            },
        };
        let array_pointer_type = self.pointer_type(array_type);
        let array_pointer = self.value(array_pointer_type);
        self.emit("address", vec![array_pointer], vec![location]);

        let dimensions: Vec<_> = if self.row_major {
            indices.iter().zip(array.bounds.iter()).collect()
        } else {
            indices.iter().zip(array.bounds.iter()).rev().collect()
        };
        let mut linear = None;
        for (index, (lower, upper)) in dimensions {
            let (index, index_type) = self.expression(index)?;
            if !matches!(index_type, INTEGER | LONG) {
                return self.fail("array subscript is not integral");
            }
            let adjusted = self.value(index_type);
            self.emit(
                "sub",
                vec![adjusted],
                vec![
                    index,
                    Operand::Constant(index_type, Number::Integer(*lower)),
                ],
            );
            linear = Some(if let Some(previous) = linear {
                let multiplied = self.value(index_type);
                self.emit(
                    "mul",
                    vec![multiplied],
                    vec![
                        previous,
                        Operand::Constant(index_type, Number::Integer(upper - lower + 1)),
                    ],
                );
                let combined = self.value(index_type);
                self.emit(
                    "add",
                    vec![combined],
                    vec![Operand::Value(multiplied), Operand::Value(adjusted)],
                );
                Operand::Value(combined)
            } else {
                Operand::Value(adjusted)
            });
        }
        let linear = linear.expect("array has at least one dimension");
        let index_type = match linear {
            Operand::Value(value) => self
                .values
                .iter()
                .find_map(|(id, type_id)| (*id == value).then_some(*type_id))
                .expect("fresh index value"),
            _ => unreachable!(),
        };
        let bytes = self.value(index_type);
        self.emit(
            "mul",
            vec![bytes],
            vec![
                linear,
                Operand::Constant(index_type, Number::Integer(self.width(element) as i64)),
            ],
        );
        let element_pointer_type = self.pointer_type(element);
        let pointer = self.value(element_pointer_type);
        self.emit(
            "ptr_offset",
            vec![pointer],
            vec![Operand::Value(array_pointer), Operand::Value(bytes)],
        );
        Ok((pointer, element))
    }

    fn descriptor_element(
        &mut self,
        descriptor: u32,
        element: u32,
        indices: &[Expr],
        address: &'static str,
    ) -> Result<u32, SemanticError> {
        if indices.is_empty() {
            return self.fail("array element requires at least one subscript");
        }
        if matches!(address, "split_huge" | "checked") {
            // PDS /Ah and /D do not inline descriptor arithmetic. BC evaluates
            // source subscripts, pushes them last-to-first followed by rank,
            // supplies the near descriptor in BX, and B$HARY returns ES:BX.
            // Keep both returned words explicit until CONCAT makes the whole
            // pointer consumed by the element load/store.
            let mut operands = Vec::new();
            for index in indices {
                let (index, index_type) = self.expression(index)?;
                if !matches!(index_type, INTEGER | LONG) {
                    return self.fail("array subscript is not integral");
                }
                operands.push(self.convert(index, index_type, INTEGER)?);
            }
            operands.push(Operand::Constant(
                INTEGER,
                Number::Integer(indices.len() as i64),
            ));
            operands.push(Operand::Value(descriptor));
            let mut order: Vec<_> = (0..indices.len()).rev().collect();
            order.extend(indices.len()..indices.len() + 2);
            let offset = self.value(INTEGER);
            let selector = self.value(INTEGER);
            self.emit_call("B$HARY", vec![offset, selector], operands, order, false);
            let pointer_type = self.whole_pointer_type(element);
            let pointer = self.value(pointer_type);
            self.emit(
                "concat",
                vec![pointer],
                vec![Operand::Value(selector), Operand::Value(offset)],
            );
            return Ok(pointer);
        }
        let pointer_type = match address {
            "near" => self.pointer_type(element),
            "split_far" => self.far_pointer_type(element),
            _ => self.whole_pointer_type(element),
        };
        let offset_type = if matches!(address, "near" | "split_far") {
            INTEGER
        } else {
            LONG
        };
        let mut linear: Option<Operand> = None;
        let ordered: Vec<_> = if self.row_major {
            indices.iter().rev().collect()
        } else {
            indices.iter().collect()
        };
        for (dimension, index) in ordered.into_iter().enumerate() {
            let (index, index_type) = self.expression(index)?;
            if !matches!(index_type, INTEGER | LONG) {
                return self.fail("array subscript is not integral");
            }
            let index = self.convert(index, index_type, offset_type)?;
            linear = Some(if let Some(previous) = linear {
                let count = self.descriptor_field(descriptor, 14 + 4 * dimension, INTEGER);
                let count = self.convert(Operand::Value(count), INTEGER, offset_type)?;
                let multiplied = self.value(offset_type);
                self.emit("mul", vec![multiplied], vec![previous, count]);
                let combined = self.value(offset_type);
                self.emit(
                    "add",
                    vec![combined],
                    vec![Operand::Value(multiplied), index],
                );
                Operand::Value(combined)
            } else {
                index
            });
        }
        let bytes = self.value(offset_type);
        self.emit(
            "mul",
            vec![bytes],
            vec![
                linear.expect("non-empty indices"),
                Operand::Constant(offset_type, Number::Integer(self.width(element) as i64)),
            ],
        );
        // Evaluate subscripts before reading descriptor fields. A subscript
        // can call a function, and that call may REDIM an aliased descriptor.
        let pointer = if address == "split_far" {
            // In the ordinary QB/PDS/VBDOS memory model AD+0Ah is already
            // adjusted for every declared lower bound. BC adds the scaled
            // subscript to that 16-bit offset and retains AD+2 as the selector;
            // it does not normalize this as a huge pointer. /AH arrays need a
            // distinct descriptor/address policy when that dialect option is
            // introduced rather than changing this default-memory-model rule.
            let selector = self.descriptor_field(descriptor, 2, INTEGER);
            let adjusted = self.descriptor_field(descriptor, 10, INTEGER);
            let offset = self.value(INTEGER);
            self.emit(
                "add",
                vec![offset],
                vec![Operand::Value(adjusted), Operand::Value(bytes)],
            );
            let pointer = self.value(pointer_type);
            self.emit(
                "concat",
                vec![pointer],
                vec![Operand::Value(selector), Operand::Value(offset)],
            );
            pointer
        } else {
            let data = self.descriptor_data_pointer(descriptor, pointer_type, address);
            let pointer = self.value(pointer_type);
            self.emit(
                "ptr_offset",
                vec![pointer],
                vec![Operand::Value(data), Operand::Value(bytes)],
            );
            pointer
        };
        Ok(pointer)
    }

    fn descriptor_data_pointer(
        &mut self,
        descriptor: u32,
        pointer_type: u32,
        address: &'static str,
    ) -> u32 {
        let key = (descriptor, pointer_type, address);
        if let Some(data) = self.descriptor_bases.get(&key) {
            return *data;
        }
        // A near STRING-array descriptor carries only its already-adjusted
        // data offset at +0Ah. Whole-pointer descriptor policies, when added,
        // use their explicit pointer at +0 instead.
        let data = self.descriptor_field(
            descriptor,
            if address == "near" { 10 } else { 0 },
            pointer_type,
        );
        self.descriptor_bases.insert(key, data);
        data
    }

    fn descriptor_field(&mut self, descriptor: u32, offset: usize, type_id: u32) -> u32 {
        let key = (descriptor, offset, type_id);
        if let Some(value) = self.descriptor_fields.get(&key) {
            return *value;
        }
        let value = self.value(type_id);
        self.emit(
            "load",
            vec![value],
            vec![Operand::Indirect {
                base: descriptor,
                offset,
                type_id,
            }],
        );
        self.descriptor_fields.insert(key, value);
        value
    }

    fn invalidate_descriptor_cache(&mut self) {
        self.descriptor_bases.clear();
        self.descriptor_fields.clear();
    }

    fn descriptor_pointer(&mut self, variable: &Variable) -> Result<u32, SemanticError> {
        if let Some(value) = variable.descriptor {
            return Ok(value);
        }
        let place = variable.descriptor_place.ok_or_else(|| SemanticError {
            message: "array has no descriptor identity".into(),
        })?;
        let descriptor_type = self
            .places
            .iter()
            .find_map(|one| (one.id == place).then_some(one.type_id))
            .ok_or_else(|| SemanticError {
                message: "array descriptor place is not visible in this procedure".into(),
            })?;
        let pointer_type = self.pointer_type(descriptor_type);
        let pointer = self.value(pointer_type);
        self.emit("address", vec![pointer], vec![Operand::Place(place)]);
        Ok(pointer)
    }

    fn string_width(&self, type_id: u32) -> Option<usize> {
        let type_ = self.types.iter().find(|one| one.id == type_id)?;
        (type_id == STRING || (type_.kind == "opaque" && type_.name.starts_with("string*")))
            .then_some(if type_id == STRING { 0 } else { type_.width })
    }

    fn far_address(&mut self, place: Operand, type_id: u32) -> u32 {
        if let Operand::Place(place_id) = &place {
            let far_static = self
                .places
                .iter()
                .find(|one| one.id == *place_id && one.storage == "static")
                .and_then(|one| self.data.iter().find(|data| data.id == one.symbol))
                .is_some_and(|data| matches!(data.address, "far" | "huge"));
            if far_static {
                // A far object's offset and selector are independent HIR
                // values. VBDOS records the shared FSL_CONST selector in a
                // relocated DGROUP word; loading that word keeps the segment
                // fact explicit without teaching MIR or the backend a BASIC
                // literal convention.
                let offset_type = self.pointer_type(type_id);
                let near = self.value(offset_type);
                self.emit("address", vec![near], vec![place.clone()]);
                let offset = self.value(INTEGER);
                self.emit("pointer_offset", vec![offset], vec![Operand::Value(near)]);
                let selector_place = self.far_string_segment_place();
                let selector = self.value(INTEGER);
                self.emit("load", vec![selector], vec![Operand::Place(selector_place)]);
                let pointer_type = self.far_pointer_type(type_id);
                let address = self.value(pointer_type);
                self.emit(
                    "concat",
                    vec![address],
                    vec![Operand::Value(selector), Operand::Value(offset)],
                );
                return address;
            }
        }
        if let Operand::Indirect { base, offset, .. } = &place {
            if self
                .values
                .iter()
                .find_map(|(id, pointer)| (*id == *base).then_some(*pointer))
                .is_some_and(|pointer| self.width(pointer) == 4)
            {
                if *offset == 0 {
                    return *base;
                }
                let pointer_type = self.far_pointer_type(type_id);
                let address = self.value(pointer_type);
                self.emit(
                    "ptr_offset",
                    vec![address],
                    vec![
                        Operand::Value(*base),
                        Operand::Constant(LONG, Number::Integer(*offset as i64)),
                    ],
                );
                return address;
            }
        }
        let pointer_type = self.far_pointer_type(type_id);
        let address = self.value(pointer_type);
        self.emit("address", vec![address], vec![place]);
        address
    }

    fn far_string_segment_place(&mut self) -> u32 {
        let symbol = self
            .far_string_segment_symbol
            .expect("far string payload has a selector word");
        if let Some(place) = self
            .places
            .iter()
            .find(|place| place.storage == "static" && place.symbol == symbol)
        {
            return place.id;
        }
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: place,
            name: "$fslSegment".into(),
            type_id: INTEGER,
            offset: 0,
            extent: 2,
            storage: "static",
            symbol,
        });
        place
    }

    fn near_string_address(&mut self, place: Operand) -> Operand {
        if let Operand::Indirect {
            base, offset: 0, ..
        } = &place
        {
            if self
                .values
                .iter()
                .find_map(|(id, pointer)| (*id == *base).then_some(*pointer))
                .is_some_and(|pointer| self.width(pointer) == 4)
            {
                // VBDOS array descriptors carry a whole data pointer, but
                // dynamic STRING descriptors are allocated in the near string
                // arena. BC deliberately passes only the computed offset to
                // SASS/FLEN/SCMP (common.bas 0066..0076 and 01e7..01f9).
                let offset = self.value(INTEGER);
                self.emit("pointer_offset", vec![offset], vec![Operand::Value(*base)]);
                return Operand::Value(offset);
            }
        }
        let pointer_type = self.pointer_type(STRING);
        let address = self.value(pointer_type);
        self.emit("address", vec![address], vec![place]);
        Operand::Value(address)
    }

    fn string_assignment(
        &mut self,
        destination: Operand,
        destination_type: u32,
        source: &Expr,
    ) -> Result<(), SemanticError> {
        let destination_width =
            self.string_width(destination_type)
                .ok_or_else(|| SemanticError {
                    message: "string assignment destination lost its type".into(),
                })?;
        if destination_width == 0 {
            // Dynamic string expressions are represented by the near address
            // of their four-byte descriptor.  This is also exactly what the
            // measured string runtimes return in AX.  Keep that expression
            // value distinct from the descriptor object itself: treating a
            // STRING value as four returned bytes would invent an AX:DX ABI.
            let destination_address = self.near_string_address(destination);
            let source_address = self.string_descriptor(source)?;
            self.emit_runtime_call(
                "B$SASS",
                Vec::new(),
                vec![source_address, destination_address],
            );
            return Ok(());
        }
        let destination = self.far_address(destination, destination_type);
        let descriptor_result = match source {
            Expr::Name(name, _) => self.bare_function_type(name) == Some(STRING),
            Expr::Apply { name, .. } => {
                intrinsics::find(canonical(name), self.dialect)
                    .is_some_and(|intrinsic| intrinsic.result == ResultClass::String)
                    || self
                        .signatures
                        .get(canonical(name))
                        .is_some_and(|signature| signature.result == Some(STRING))
            }
            Expr::Binary {
                op: Binary::Add,
                left,
                right,
                ..
            } => self.string_syntax(left) && self.string_syntax(right),
            _ => false,
        };
        if descriptor_result {
            let descriptor = self.string_descriptor(source)?;
            let source = self.far_descriptor(descriptor)?;
            self.emit_runtime_call(
                "B$ASSN",
                Vec::new(),
                vec![
                    source,
                    Operand::Constant(INTEGER, Number::Integer(0)),
                    Operand::Value(destination),
                    Operand::Constant(INTEGER, Number::Integer(destination_width as i64)),
                ],
            );
            return Ok(());
        }
        let (source_place, source_type, source_width) = match source {
            Expr::Literal(Literal::String(text), _) if !text.is_empty() => {
                let (_, payload, payload_type) = self.string_literal_places(text)?;
                (Operand::Place(payload), payload_type, text.len())
            }
            Expr::Literal(Literal::String(text), _) => {
                let descriptor = self.string_literal(text)?;
                (Operand::Place(descriptor), STRING, 0)
            }
            _ => {
                let (place, type_id) = self.destination(source)?;
                let width = self.string_width(type_id).ok_or_else(|| SemanticError {
                    message: "string assignment source is not a string place".into(),
                })?;
                (place, type_id, width)
            }
        };
        let source = self.far_address(source_place, source_type);
        self.emit_runtime_call(
            "B$ASSN",
            Vec::new(),
            vec![
                Operand::Value(source),
                Operand::Constant(INTEGER, Number::Integer(source_width as i64)),
                Operand::Value(destination),
                Operand::Constant(INTEGER, Number::Integer(destination_width as i64)),
            ],
        );
        Ok(())
    }

    fn mid_assignment(&mut self, arguments: &[Expr], source: &Expr) -> Result<(), SemanticError> {
        if !(2..=3).contains(&arguments.len()) {
            return self.fail("MID$ assignment expects a destination, start, and optional length");
        }
        let (destination, destination_type) = self.destination(&arguments[0])?;
        let destination_width =
            self.string_width(destination_type)
                .ok_or_else(|| SemanticError {
                    message: "MID$ assignment destination must be a string".into(),
                })?;
        let destination = self.far_address(destination, destination_type);
        let source = self.string_descriptor(source)?;
        let (start, start_type) = self.expression(&arguments[1])?;
        let start = self.convert(start, start_type, INTEGER)?;
        let maximum = if let Some(length) = arguments.get(2) {
            let (length, length_type) = self.expression(length)?;
            self.convert(length, length_type, INTEGER)?
        } else {
            Operand::Constant(INTEGER, Number::Integer(i16::MAX as i64))
        };
        self.emit_runtime_call(
            "B$SMID",
            Vec::new(),
            vec![
                Operand::Value(destination),
                Operand::Constant(INTEGER, Number::Integer(destination_width as i64)),
                source,
                maximum,
                start,
            ],
        );
        Ok(())
    }

    fn far_descriptor(&mut self, descriptor: Operand) -> Result<Operand, SemanticError> {
        let Operand::Value(descriptor) = descriptor else {
            return self.fail("string descriptor is not a value");
        };
        let symbol = self.next_data;
        self.next_data += 1;
        self.data.push(DataObject {
            id: symbol,
            name: format!("$ds{symbol}"),
            bytes: vec![0, 0],
            readonly: true,
            relocations: Vec::new(),
            linkage: "internal",
            address: "near",
        });
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: place,
            name: format!("$ds{symbol}"),
            type_id: INTEGER,
            offset: 0,
            extent: 2,
            storage: "static",
            symbol,
        });
        let anchor_type = self.far_pointer_type(INTEGER);
        let anchor = self.value(anchor_type);
        self.emit("address", vec![anchor], vec![Operand::Place(place)]);
        let segment = self.value(INTEGER);
        self.emit(
            "pointer_segment",
            vec![segment],
            vec![Operand::Value(anchor)],
        );
        let offset = self.value(INTEGER);
        self.emit(
            "pointer_offset",
            vec![offset],
            vec![Operand::Value(descriptor)],
        );
        let pointer_type = self.far_pointer_type(STRING);
        let far = self.value(pointer_type);
        self.emit(
            "concat",
            vec![far],
            vec![Operand::Value(segment), Operand::Value(offset)],
        );
        Ok(Operand::Value(far))
    }

    fn string_descriptor(&mut self, expression: &Expr) -> Result<Operand, SemanticError> {
        if let Expr::Name(name, _) = expression {
            if intrinsics::find(canonical(name), self.dialect)
                .is_some_and(|intrinsic| intrinsic.lowering == Lowering::CommandLine)
            {
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call("B$FCMD", vec![result], Vec::new());
                return Ok(Operand::Value(result));
            }
            if self.bare_function_type(name) == Some(STRING) {
                let Some((result, _)) = self.call(name, &[], true)? else {
                    unreachable!("STRING function has a result")
                };
                return Ok(result);
            }
        }
        if let Expr::Binary {
            op: Binary::Add,
            left,
            right,
            ..
        } = expression
        {
            if self.string_syntax(left) && self.string_syntax(right) {
                let left = self.string_descriptor(left)?;
                let right = self.string_descriptor(right)?;
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call("B$SCAT", vec![result], vec![left, right]);
                return Ok(Operand::Value(result));
            }
        }
        if let Expr::Apply {
            name, arguments, ..
        } = expression
        {
            let name = canonical(name);
            let intrinsic = intrinsics::find(name, self.dialect);
            if let Some(intrinsic) = intrinsic {
                if !intrinsic.accepts(arguments.len()) {
                    return self.fail(format!(
                        "{}$ expects {}",
                        name,
                        arity_description(intrinsic.min_arity, intrinsic.max_arity)
                    ));
                }
            }
            let runtime = intrinsic.and_then(|intrinsic| match intrinsic.lowering {
                Lowering::RuntimeString(callee) => Some(callee),
                Lowering::Character => Some("B$FCHR"),
                Lowering::Mid => Some("B$FMID"),
                Lowering::Left => Some("B$LEFT"),
                Lowering::Right => Some("B$RGHT"),
                _ => None,
            });
            if let Some(callee) = runtime {
                let mut operands = Vec::new();
                match intrinsic.unwrap().lowering {
                    Lowering::Character => {
                        let (value, type_id) = self.expression(&arguments[0])?;
                        operands.push(self.convert(value, type_id, INTEGER)?);
                    }
                    Lowering::Mid => {
                        operands.push(self.string_descriptor(&arguments[0])?);
                        let (start, start_type) = self.expression(&arguments[1])?;
                        operands.push(self.convert(start, start_type, INTEGER)?);
                        if let Some(length) = arguments.get(2) {
                            let (length, length_type) = self.expression(length)?;
                            operands.push(self.convert(length, length_type, INTEGER)?);
                        } else {
                            // BC spells the omitted count as the largest
                            // positive INTEGER; FMID clips it to the source.
                            operands
                                .push(Operand::Constant(INTEGER, Number::Integer(i16::MAX as i64)));
                        }
                    }
                    Lowering::Left | Lowering::Right => {
                        operands.push(self.string_descriptor(&arguments[0])?);
                        let (length, length_type) = self.expression(&arguments[1])?;
                        operands.push(self.convert(length, length_type, INTEGER)?);
                    }
                    Lowering::RuntimeString(_) => {
                        operands.push(self.string_descriptor(&arguments[0])?);
                    }
                    _ => unreachable!("runtime was selected from this lowering"),
                }
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call(callee, vec![result], operands);
                return Ok(Operand::Value(result));
            }
            if intrinsic.is_some_and(|one| one.lowering == Lowering::StringFill) {
                let (length, length_type) = self.expression(&arguments[0])?;
                let length = self.convert(length, length_type, INTEGER)?;
                let (callee, repeated) = if self.string_syntax(&arguments[1]) {
                    ("B$STRS", self.string_descriptor(&arguments[1])?)
                } else {
                    let (value, type_id) = self.expression(&arguments[1])?;
                    ("B$STRI", self.convert(value, type_id, INTEGER)?)
                };
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call(callee, vec![result], vec![length, repeated]);
                return Ok(Operand::Value(result));
            }
            if intrinsic.is_some_and(|one| one.lowering == Lowering::Space) {
                let (count, type_id) = self.expression(&arguments[0])?;
                let count = self.convert(count, type_id, INTEGER)?;
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call("B$SPAC", vec![result], vec![count]);
                return Ok(Operand::Value(result));
            }
            if intrinsic.is_some_and(|one| one.lowering == Lowering::Environ) {
                let (callee, argument) = if self.string_syntax(&arguments[0]) {
                    ("B$FEVS", self.string_descriptor(&arguments[0])?)
                } else {
                    let (value, type_id) = self.expression(&arguments[0])?;
                    ("B$FEVI", self.convert(value, type_id, INTEGER)?)
                };
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call(callee, vec![result], vec![argument]);
                return Ok(Operand::Value(result));
            }
            if intrinsic.is_some_and(|one| one.lowering == Lowering::Directory) {
                // VBDOS uses the same FDR1 entry for a new search and for
                // continuation: a descriptor starts a search, while the null
                // descriptor spells DIR$("").  The result is a temporary
                // near string descriptor in AX, like the other string
                // functions.  Keep the filesystem/heap effects on the call.
                let argument = match &arguments[0] {
                    Expr::Literal(Literal::String(text), _) if text.is_empty() => {
                        Operand::Constant(INTEGER, Number::Integer(0))
                    }
                    expression => self.string_descriptor(expression)?,
                };
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call("B$FDR1", vec![result], vec![argument]);
                return Ok(Operand::Value(result));
            }
            if intrinsic.is_some_and(|one| one.lowering == Lowering::StringNumber) {
                let (mut operand, type_id) = self.expression(&arguments[0])?;
                let callee = match type_id {
                    INTEGER | BOOLEAN | BYTE => "B$STI2",
                    LONG => "B$STI4",
                    SINGLE => "B$STR4",
                    DOUBLE => "B$STR8",
                    _ => return self.fail("STR$ requires a numeric argument"),
                };
                if matches!(type_id, SINGLE | DOUBLE) {
                    // Runtime STR4/STR8 consume the declared 4/8-byte value,
                    // not the frontend's extended-precision evaluation value.
                    let stored = self.temporary(type_id)?;
                    self.emit("store", Vec::new(), vec![Operand::Place(stored), operand]);
                    operand = Operand::Place(stored);
                }
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call(callee, vec![result], vec![operand]);
                return Ok(Operand::Value(result));
            }
            if intrinsic.is_some_and(|one| {
                matches!(
                    one.lowering,
                    Lowering::PackInteger
                        | Lowering::PackLong
                        | Lowering::PackSingle
                        | Lowering::PackDouble
                )
            }) {
                let lowering = intrinsic.unwrap().lowering;
                let (target, callee) = match lowering {
                    Lowering::PackInteger => (INTEGER, "B$FMKI"),
                    Lowering::PackLong => (LONG, "B$FMKL"),
                    Lowering::PackSingle => (SINGLE, if self.mbf { "B$FMSF" } else { "B$FMKS" }),
                    Lowering::PackDouble => (DOUBLE, if self.mbf { "B$FMDF" } else { "B$FMKD" }),
                    _ => unreachable!("matched binary packer"),
                };
                let (operand, type_id) = self.expression(&arguments[0])?;
                let mut operand = self.convert(operand, type_id, target)?;
                if matches!(target, SINGLE | DOUBLE) {
                    // The runtime copies the declared 4/8-byte representation,
                    // not the frontend's extended-precision evaluation value.
                    let stored = self.temporary(target)?;
                    self.emit("store", Vec::new(), vec![Operand::Place(stored), operand]);
                    operand = Operand::Place(stored);
                }
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call(callee, vec![result], vec![operand]);
                return Ok(Operand::Value(result));
            }
            if let Some(callee) = intrinsic.and_then(|one| match one.lowering {
                Lowering::RadixText(callee) => Some(callee),
                _ => None,
            }) {
                let (operand, type_id) = self.expression(&arguments[0])?;
                let operand = self.convert(operand, type_id, LONG)?;
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call(callee, vec![result], vec![operand]);
                return Ok(Operand::Value(result));
            }
            if self
                .signatures
                .get(name)
                .is_some_and(|signature| signature.result == Some(STRING))
            {
                let Some((result, _)) = self.call(name, arguments, true)? else {
                    unreachable!("STRING function has a result")
                };
                return Ok(result);
            }
        }

        let (place, type_id) = match expression {
            Expr::Literal(Literal::String(text), _) => {
                (Operand::Place(self.string_literal(text)?), STRING)
            }
            _ => self.destination(expression)?,
        };
        let width = self.string_width(type_id).ok_or_else(|| SemanticError {
            message: "string expression requires a string operand".into(),
        })?;
        if width == 0 {
            return Ok(self.near_string_address(place));
        }

        // A fixed string has bytes but no descriptor.  B$LDFS is the measured
        // runtime bridge: far byte address + length -> temporary descriptor
        // address in AX.  The temporary then composes with every descriptor-
        // accepting string routine without exposing machine registers in HIR.
        let address = self.far_address(place, type_id);
        let pointer_type = self.pointer_type(STRING);
        let result = self.value(pointer_type);
        self.emit_runtime_call(
            "B$LDFS",
            vec![result],
            vec![
                Operand::Value(address),
                Operand::Constant(INTEGER, Number::Integer(width as i64)),
            ],
        );
        Ok(Operand::Value(result))
    }

    fn byref_string_argument(&mut self, expression: &Expr) -> Result<Operand, SemanticError> {
        // A genuine dynamic STRING lvalue already owns a stable descriptor,
        // so ordinary BYREF aliasing passes that descriptor directly. A
        // literal, fixed string, concatenation, or string-function result is
        // different: its descriptor is a runtime temporary, and routines such
        // as FLEN/FASC may consume that temporary. BC first assigns such an
        // expression to a frame-owned STRING descriptor before passing it to
        // a user procedure (D_SURF LS_INIT 012f..013f and LS_ANIMATE
        // 02a2..02c8). This is lifetime materialization, not a runtime-call
        // special case.
        let lvalue = match expression {
            Expr::Name(name, _)
                if intrinsics::find(canonical(name), self.dialect).is_none()
                    && !self.signatures.contains_key(canonical(name)) =>
            {
                Some(self.destination(expression)?)
            }
            Expr::Apply { name, .. }
                if intrinsics::find(canonical(name), self.dialect).is_none()
                    && !self.signatures.contains_key(canonical(name))
                    && self.variables.contains_key(canonical(name)) =>
            {
                Some(self.destination(expression)?)
            }
            Expr::Field { .. } | Expr::Index { .. } => Some(self.destination(expression)?),
            _ => None,
        };
        if let Some((place, type_id)) = lvalue {
            if self.string_width(type_id) == Some(0) {
                return Ok(self.near_string_address(place));
            }
        }

        let source = self.string_descriptor(expression)?;
        let temporary = self.owned_string_temporary()?;
        let destination = self.near_string_address(Operand::Place(temporary));
        self.emit_runtime_call("B$SASS", Vec::new(), vec![source, destination.clone()]);
        Ok(destination)
    }

    fn redim(&mut self, declaration: &Declaration) -> Result<(), SemanticError> {
        if !declaration.array || declaration.bounds.is_empty() {
            return self.fail(format!("REDIM {} requires bounds", declaration.name));
        }
        let variable = self.variable(&declaration.name)?;
        let element = variable.element.ok_or_else(|| SemanticError {
            message: format!("REDIM target {} is not an array", declaration.name),
        })?;
        let inferred = suffix(&declaration.name);
        let selected = declaration.type_name.as_ref().or(inferred.as_ref());
        let same_element = if matches!(selected, Some(TypeName::String)) {
            match &declaration.fixed_length {
                Some(length) => {
                    let width = self.constant_integer(length)?;
                    width > 0 && self.string_width(element) == Some(width as usize)
                }
                None => element == STRING,
            }
        } else {
            self.named_type(&declaration.name, declaration.type_name.as_ref())? == element
        };
        if !same_element {
            return self.fail(format!(
                "REDIM changes the element type of {}",
                declaration.name
            ));
        }
        let mut operands = Vec::new();
        for bound in &declaration.bounds {
            let lower = match &bound.lower {
                Some(lower) => {
                    let (value, type_id) = self.expression(lower)?;
                    self.convert(value, type_id, INTEGER)?
                }
                None => Operand::Constant(INTEGER, Number::Integer(self.option_base)),
            };
            let (upper, upper_type) = self.expression(&bound.upper)?;
            let upper = self.convert(upper, upper_type, INTEGER)?;
            operands.push(lower);
            operands.push(upper);
        }
        operands.push(Operand::Constant(
            INTEGER,
            Number::Integer(self.width(element) as i64),
        ));
        operands.push(Operand::Constant(
            INTEGER,
            Number::Integer((declaration.bounds.len() | (1 << 8)) as i64),
        ));
        operands.push(Operand::Value(self.descriptor_pointer(&variable)?));
        self.emit_runtime_call("B$RDIM", Vec::new(), operands);
        Ok(())
    }

    fn expression(&mut self, expression: &Expr) -> Result<(Operand, u32), SemanticError> {
        match expression {
            Expr::Literal(Literal::Integer(value, type_name), _) => {
                let type_id = type_id(Some(type_name))?;
                Ok((Operand::Constant(type_id, Number::Integer(*value)), type_id))
            }
            Expr::Literal(Literal::Real(value, type_name), _) => {
                let type_id = type_id(Some(type_name))?;
                Ok((self.floating_literal(value, type_id)?, type_id))
            }
            Expr::Literal(Literal::String(_), _) => {
                self.fail("string expressions are runtime-only and not attached yet")
            }
            Expr::Name(name, _) => {
                if intrinsics::find(canonical(name), self.dialect)
                    .is_some_and(|intrinsic| intrinsic.accepts(0))
                {
                    return self.builtin(name, &[])?.ok_or_else(|| SemanticError {
                        message: format!("intrinsic {name} has no value"),
                    });
                }
                if let Some((type_id, value)) = self.constants.get(canonical(name)).cloned() {
                    let operand = match value {
                        Number::Integer(value) => {
                            Operand::Constant(type_id, Number::Integer(value))
                        }
                        Number::Real(value) => self.floating_literal(&value, type_id)?,
                    };
                    return Ok((operand, type_id));
                }
                if self.bare_function_type(name).is_some() {
                    return self.call(name, &[], true)?.ok_or_else(|| SemanticError {
                        message: format!("SUB {name} does not produce a value"),
                    });
                }
                let variable = self.variable(name)?;
                if variable.element.is_some() {
                    return self.fail(format!("array {name} requires subscripts"));
                }
                let result = self.value(variable.type_id);
                let source = if let Some(base) = variable.indirect {
                    Operand::Indirect {
                        base,
                        offset: 0,
                        type_id: variable.type_id,
                    }
                } else {
                    Operand::Place(variable.place)
                };
                self.emit("load", vec![result], vec![source]);
                Ok((Operand::Value(result), variable.type_id))
            }
            Expr::Apply { .. } => {
                if let Expr::Apply {
                    name, arguments, ..
                } = expression
                {
                    if let Some(result) = self.builtin(name, arguments)? {
                        return Ok(result);
                    }
                    if self.signatures.contains_key(canonical(name)) {
                        return self
                            .call(name, arguments, true)?
                            .ok_or_else(|| SemanticError {
                                message: format!("SUB {name} does not produce a value"),
                            });
                    }
                }
                let (place, type_id) = self.destination(expression)?;
                let result = self.value(type_id);
                self.emit("load", vec![result], vec![place]);
                Ok((Operand::Value(result), type_id))
            }
            Expr::Index { .. } => {
                let (place, type_id) = self.destination(expression)?;
                let result = self.value(type_id);
                self.emit("load", vec![result], vec![place]);
                Ok((Operand::Value(result), type_id))
            }
            Expr::Field { .. } => {
                if let Some(name) = dotted_name(expression) {
                    if let Some((type_id, value)) = self.constants.get(canonical(&name)) {
                        return Ok((Operand::Constant(*type_id, value.clone()), *type_id));
                    }
                }
                let (place, type_id) = self.destination(expression)?;
                let result = self.value(type_id);
                self.emit("load", vec![result], vec![place]);
                Ok((Operand::Value(result), type_id))
            }
            Expr::Unary { op, operand, .. } => {
                let (operand, type_id) = self.expression(operand)?;
                if *op == Unary::Positive {
                    return Ok((operand, type_id));
                }
                let operation = match op {
                    Unary::Negative if matches!(type_id, SINGLE | DOUBLE) => "fneg",
                    Unary::Negative => "neg",
                    Unary::Not => "not",
                    Unary::Positive => unreachable!(),
                };
                if *op == Unary::Not && !matches!(type_id, INTEGER | LONG | BOOLEAN) {
                    return self.fail("NOT requires an integral operand");
                }
                let result = self.value(type_id);
                self.emit(operation, vec![result], vec![operand]);
                Ok((Operand::Value(result), type_id))
            }
            Expr::Binary {
                op, left, right, ..
            } => self.binary(*op, left, right),
        }
    }

    fn builtin(
        &mut self,
        name: &str,
        arguments: &[Expr],
    ) -> Result<Option<(Operand, u32)>, SemanticError> {
        let name = canonical(name);
        let Some(intrinsic) = intrinsics::find(name, self.dialect) else {
            return Ok(None);
        };
        if !intrinsic.accepts(arguments.len()) {
            return self.fail(format!(
                "{name} expects {}",
                arity_description(intrinsic.min_arity, intrinsic.max_arity)
            ));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::ErrorNumber | Lowering::ErrorLine
        ) {
            let number = intrinsic.lowering == Lowering::ErrorNumber;
            let type_id = if number { INTEGER } else { LONG };
            let result = self.value(type_id);
            self.emit_runtime_call(
                if number { "B$FERR" } else { "B$FERL" },
                vec![result],
                Vec::new(),
            );
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::LowerBound | Lowering::UpperBound
        ) {
            let array_name = match &arguments[0] {
                Expr::Name(name, _) => name,
                Expr::Apply {
                    name, arguments, ..
                } if arguments.is_empty() => name,
                _ => return self.fail(format!("{name} requires a bare array name")),
            };
            let variable = self.variable(array_name)?;
            if variable.element.is_none() {
                return self.fail(format!("{array_name} is not an array"));
            }
            let descriptor = self.descriptor_pointer(&variable)?;
            let dimension = if let Some(dimension) = arguments.get(1) {
                let (dimension, type_id) = self.expression(dimension)?;
                self.convert(dimension, type_id, INTEGER)?
            } else {
                Operand::Constant(INTEGER, Number::Integer(1))
            };
            let result = self.value(INTEGER);
            self.emit_runtime_call(
                if intrinsic.lowering == Lowering::LowerBound {
                    "B$LBND"
                } else {
                    "B$UBND"
                },
                vec![result],
                vec![Operand::Value(descriptor), dimension],
            );
            return Ok(Some((Operand::Value(result), INTEGER)));
        }
        if intrinsic.lowering == Lowering::FreeFile {
            let result = self.value(INTEGER);
            self.emit_runtime_call("B$FREF", vec![result], Vec::new());
            return Ok(Some((Operand::Value(result), INTEGER)));
        }
        if intrinsic.lowering == Lowering::HeapFree {
            if self.string_syntax(&arguments[0]) {
                // FRE has two source-level overloads. VBDOS selects FRSD for
                // strings and passes the near descriptor address; its empty
                // literal has the canonical null descriptor address used by
                // BC itself. Numeric selectors use the distinct FRI2 entry.
                let selector = match &arguments[0] {
                    Expr::Literal(Literal::String(text), _) if text.is_empty() => {
                        Operand::Constant(INTEGER, Number::Integer(0))
                    }
                    expression => self.string_descriptor(expression)?,
                };
                let result = self.value(LONG);
                self.emit_runtime_call("B$FRSD", vec![result], vec![selector]);
                return Ok(Some((Operand::Value(result), LONG)));
            }
            let (selector, selector_type) = self.expression(&arguments[0])?;
            let selector = self.convert(selector, selector_type, INTEGER)?;
            let result = self.value(LONG);
            self.emit_runtime_call("B$FRI2", vec![result], vec![selector]);
            return Ok(Some((Operand::Value(result), LONG)));
        }
        if intrinsic.lowering == Lowering::FileLength {
            let (file, file_type) = self.expression(&arguments[0])?;
            let file = self.convert(file, file_type, INTEGER)?;
            let result = self.value(LONG);
            self.emit_runtime_call("B$FLOF", vec![result], vec![file]);
            return Ok(Some((Operand::Value(result), LONG)));
        }
        if intrinsic.lowering == Lowering::Timer {
            // All three Microsoft runtimes return AX = near address of a
            // runtime-owned SINGLE, not the value itself. Keep the load
            // explicit so TIMER remains an effectful clock read in HIR.
            let pointer_type = self.pointer_type(SINGLE);
            let pointer = self.value(pointer_type);
            self.emit_runtime_call("B$TIMR", vec![pointer], Vec::new());
            let result = self.value(SINGLE);
            self.emit(
                "load",
                vec![result],
                vec![Operand::Indirect {
                    base: pointer,
                    offset: 0,
                    type_id: SINGLE,
                }],
            );
            return Ok(Some((Operand::Value(result), SINGLE)));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::ToInteger | Lowering::ToLong | Lowering::ToSingle | Lowering::ToDouble
        ) {
            let target = match intrinsic.lowering {
                Lowering::ToInteger => INTEGER,
                Lowering::ToLong => LONG,
                Lowering::ToSingle => SINGLE,
                Lowering::ToDouble => DOUBLE,
                _ => unreachable!(),
            };
            let (operand, source) = self.expression(&arguments[0])?;
            if !matches!(source, INTEGER | LONG | SINGLE | DOUBLE | BOOLEAN) {
                return self.fail(format!("{name} requires a numeric argument"));
            }
            if source == target {
                return Ok(Some((operand, target)));
            }
            let converted = self.convert(operand, source, target)?;
            return Ok(Some((converted, target)));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::PointerOffset | Lowering::PointerSegment
        ) {
            let (place, type_id) = self.destination(&arguments[0])?;
            let pointer = self.far_address(place, type_id);
            let result = self.value(INTEGER);
            self.emit(
                if intrinsic.lowering == Lowering::PointerSegment {
                    "pointer_segment"
                } else {
                    "pointer_offset"
                },
                vec![result],
                vec![Operand::Value(pointer)],
            );
            return Ok(Some((Operand::Value(result), INTEGER)));
        }
        if matches!(intrinsic.lowering, Lowering::Abs | Lowering::Sqrt) {
            let (mut operand, mut type_id) = self.expression(&arguments[0])?;
            if intrinsic.lowering == Lowering::Abs && matches!(type_id, INTEGER | LONG | BOOLEAN) {
                // Signed absolute value without control flow: (x xor sign)-sign.
                // As on the target integer instructions, MIN wraps to itself.
                let sign = self.value(type_id);
                self.emit(
                    "sar",
                    vec![sign],
                    vec![
                        operand.clone(),
                        Operand::Constant(
                            type_id,
                            Number::Integer((self.width(type_id) * 8 - 1) as i64),
                        ),
                    ],
                );
                let toggled = self.value(type_id);
                self.emit("xor", vec![toggled], vec![operand, Operand::Value(sign)]);
                let result = self.value(type_id);
                self.emit(
                    "sub",
                    vec![result],
                    vec![Operand::Value(toggled), Operand::Value(sign)],
                );
                return Ok(Some((Operand::Value(result), type_id)));
            }
            if intrinsic.lowering == Lowering::Sqrt
                && matches!(type_id, INTEGER | LONG | BOOLEAN | BYTE)
            {
                operand = self.convert(operand, type_id, SINGLE)?;
                type_id = SINGLE;
            }
            if !matches!(type_id, SINGLE | DOUBLE) {
                return self.fail(format!("{name} requires a numeric argument"));
            }
            let result = self.value(type_id);
            self.emit(
                if intrinsic.lowering == Lowering::Abs {
                    "fabs"
                } else {
                    "fsqrt"
                },
                vec![result],
                vec![operand],
            );
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if intrinsic.lowering == Lowering::Sign {
            let (operand, type_id) = self.expression(&arguments[0])?;
            if !matches!(type_id, INTEGER | LONG | SINGLE | DOUBLE | BOOLEAN | BYTE) {
                return self.fail("SGN requires a numeric argument");
            }
            let zero = if matches!(type_id, SINGLE | DOUBLE) {
                self.floating_literal("0", type_id)?
            } else {
                Operand::Constant(type_id, Number::Integer(0))
            };
            let below = self.value(BOOLEAN);
            self.emit("lt", vec![below], vec![operand.clone(), zero.clone()]);
            let above = self.value(BOOLEAN);
            self.emit("gt", vec![above], vec![operand, zero]);
            let below = self.convert(Operand::Value(below), BOOLEAN, type_id)?;
            let above = self.convert(Operand::Value(above), BOOLEAN, type_id)?;
            let result = self.value(type_id);
            self.emit(
                if matches!(type_id, SINGLE | DOUBLE) {
                    "fsub"
                } else {
                    "sub"
                },
                vec![result],
                vec![below, above],
            );
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::Sin | Lowering::Cos | Lowering::Tan
        ) {
            let (mut operand, mut type_id) = self.expression(&arguments[0])?;
            if matches!(type_id, INTEGER | LONG | BOOLEAN) {
                operand = self.convert(operand, type_id, SINGLE)?;
                type_id = SINGLE;
            }
            if !matches!(type_id, SINGLE | DOUBLE) {
                return self.fail(format!("{name} requires a numeric argument"));
            }
            let result = self.value(type_id);
            if intrinsic.lowering == Lowering::Tan {
                let sine = self.value(type_id);
                let cosine = self.value(type_id);
                self.emit("fsin", vec![sine], vec![operand.clone()]);
                self.emit("fcos", vec![cosine], vec![operand]);
                self.emit(
                    "fdiv",
                    vec![result],
                    vec![Operand::Value(sine), Operand::Value(cosine)],
                );
            } else {
                self.emit(
                    if intrinsic.lowering == Lowering::Sin {
                        "fsin"
                    } else {
                        "fcos"
                    },
                    vec![result],
                    vec![operand],
                );
            }
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if intrinsic.lowering == Lowering::Atan {
            let (mut operand, mut type_id) = self.expression(&arguments[0])?;
            if matches!(type_id, INTEGER | LONG | BOOLEAN) {
                operand = self.convert(operand, type_id, SINGLE)?;
                type_id = SINGLE;
            }
            if !matches!(type_id, SINGLE | DOUBLE) {
                return self.fail("ATN requires a numeric argument");
            }
            let result = self.value(type_id);
            self.emit("fatan", vec![result], vec![operand]);
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if matches!(intrinsic.lowering, Lowering::Log | Lowering::Exp) {
            let (mut operand, mut type_id) = self.expression(&arguments[0])?;
            if matches!(type_id, INTEGER | LONG | BOOLEAN | BYTE) {
                operand = self.convert(operand, type_id, SINGLE)?;
                type_id = SINGLE;
            }
            if !matches!(type_id, SINGLE | DOUBLE) {
                return self.fail(format!("{name} requires a numeric argument"));
            }
            let result = self.value(type_id);
            if intrinsic.lowering == Lowering::Log {
                let logarithm = self.value(type_id);
                self.emit("flog2", vec![logarithm], vec![operand]);
                let ln2 = self.floating_literal("0.69314718055994530942", type_id)?;
                self.emit("fmul", vec![result], vec![Operand::Value(logarithm), ln2]);
            } else {
                let log2e = self.floating_literal("1.4426950408889634074", type_id)?;
                let scaled = self.value(type_id);
                self.emit("fmul", vec![scaled], vec![operand, log2e]);
                self.emit("fexp2", vec![result], vec![Operand::Value(scaled)]);
            }
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if intrinsic.lowering == Lowering::Floor {
            let (operand, type_id) = self.expression(&arguments[0])?;
            if matches!(type_id, INTEGER | LONG | BOOLEAN) {
                return Ok(Some((operand, type_id)));
            }
            if !matches!(type_id, SINGLE | DOUBLE) {
                return self.fail("INT requires a numeric argument");
            }
            // INT is floor, while x87's current conversion rounds to nearest.
            // For rounded integer n, floor(x) is n + (x < n ? -1 : 0).
            // Express the correction in ordinary HIR so optimization sees
            // every value. QB booleans are -1/0 and supply that correction.
            let rounded = self.value(LONG);
            self.emit("convert", vec![rounded], vec![operand.clone()]);
            let integral = self.convert(Operand::Value(rounded), LONG, type_id)?;
            let below = self.value(BOOLEAN);
            self.emit("lt", vec![below], vec![operand, integral.clone()]);
            let correction = self.convert(Operand::Value(below), BOOLEAN, type_id)?;
            let result = self.value(type_id);
            self.emit("fadd", vec![result], vec![integral, correction]);
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if intrinsic.lowering == Lowering::Truncate {
            let (operand, type_id) = self.expression(&arguments[0])?;
            if matches!(type_id, INTEGER | LONG | BOOLEAN | BYTE) {
                return Ok(Some((operand, type_id)));
            }
            if !matches!(type_id, SINGLE | DOUBLE) {
                return self.fail("FIX requires a numeric argument");
            }
            // For nearest-rounded integer n, trunc(x) is
            // n + (x < n ? -1 : 0) - (x > n ? -1 : 0).
            let rounded = self.value(LONG);
            self.emit("convert", vec![rounded], vec![operand.clone()]);
            let integral = self.convert(Operand::Value(rounded), LONG, type_id)?;
            let below = self.value(BOOLEAN);
            self.emit("lt", vec![below], vec![operand.clone(), integral.clone()]);
            let below = self.convert(Operand::Value(below), BOOLEAN, type_id)?;
            let above = self.value(BOOLEAN);
            self.emit("gt", vec![above], vec![operand, integral.clone()]);
            let above = self.convert(Operand::Value(above), BOOLEAN, type_id)?;
            let adjusted = self.value(type_id);
            self.emit("fadd", vec![adjusted], vec![integral, below]);
            let result = self.value(type_id);
            self.emit("fsub", vec![result], vec![Operand::Value(adjusted), above]);
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if intrinsic.lowering == Lowering::Peek {
            let segment_place = self.def_segment_place();
            let (offset, offset_type) = self.expression(&arguments[0])?;
            let offset = self.convert(offset, offset_type, INTEGER)?;
            let segment = self.value(INTEGER);
            self.emit("load", vec![segment], vec![Operand::Place(segment_place)]);
            let pointer_type = self.far_pointer_type(BYTE);
            let pointer = self.value(pointer_type);
            self.emit(
                "concat",
                vec![pointer],
                vec![Operand::Value(segment), offset],
            );
            let byte = self.value(BYTE);
            self.emit(
                "load",
                vec![byte],
                vec![Operand::Indirect {
                    base: pointer,
                    offset: 0,
                    type_id: BYTE,
                }],
            );
            let result = self.convert(Operand::Value(byte), BYTE, INTEGER)?;
            return Ok(Some((result, INTEGER)));
        }
        if intrinsic.lowering == Lowering::Length {
            let produced_string =
                matches!(
                    &arguments[0],
                    Expr::Apply { name, .. }
                        if intrinsics::find(canonical(name), self.dialect)
                            .is_some_and(|intrinsic| intrinsic.result == ResultClass::String)
                ) || matches!(&arguments[0], Expr::Literal(Literal::String(_), _));
            if produced_string {
                let address = self.string_descriptor(&arguments[0])?;
                let result = self.value(INTEGER);
                self.emit_runtime_call("B$FLEN", vec![result], vec![address]);
                return Ok(Some((Operand::Value(result), INTEGER)));
            }
            let (_, type_id) = self.destination(&arguments[0])?;
            if self.string_width(type_id).is_some() {
                let address = self.string_descriptor(&arguments[0])?;
                let result = self.value(INTEGER);
                self.emit_runtime_call("B$FLEN", vec![result], vec![address]);
                return Ok(Some((Operand::Value(result), INTEGER)));
            }
            let type_ = self
                .types
                .iter()
                .find(|one| one.id == type_id)
                .cloned()
                .expect("known LEN type");
            return Ok(Some((
                Operand::Constant(INTEGER, Number::Integer(type_.width as i64)),
                INTEGER,
            )));
        }
        if intrinsic.lowering == Lowering::Asc {
            let address = self.string_descriptor(&arguments[0])?;
            let result = self.value(INTEGER);
            self.emit_runtime_call("B$FASC", vec![result], vec![address]);
            return Ok(Some((Operand::Value(result), INTEGER)));
        }
        if intrinsic.lowering == Lowering::Instr {
            let (callee, operands) = if arguments.len() == 2 {
                (
                    "B$INS2",
                    vec![
                        self.string_descriptor(&arguments[0])?,
                        self.string_descriptor(&arguments[1])?,
                    ],
                )
            } else {
                let (start, start_type) = self.expression(&arguments[0])?;
                (
                    "B$INS3",
                    vec![
                        self.convert(start, start_type, INTEGER)?,
                        self.string_descriptor(&arguments[1])?,
                        self.string_descriptor(&arguments[2])?,
                    ],
                )
            };
            let result = self.value(INTEGER);
            self.emit_runtime_call(callee, vec![result], operands);
            return Ok(Some((Operand::Value(result), INTEGER)));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::UnpackInteger | Lowering::UnpackLong
        ) {
            let descriptor = self.string_descriptor(&arguments[0])?;
            let integer = intrinsic.lowering == Lowering::UnpackInteger;
            let type_id = if integer { INTEGER } else { LONG };
            let result = self.value(type_id);
            self.emit_runtime_call(
                if integer { "B$FCVI" } else { "B$FCVL" },
                vec![result],
                vec![descriptor],
            );
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if matches!(
            intrinsic.lowering,
            Lowering::UnpackSingle | Lowering::UnpackDouble
        ) {
            let descriptor = self.string_descriptor(&arguments[0])?;
            let single = intrinsic.lowering == Lowering::UnpackSingle;
            let type_id = if single { SINGLE } else { DOUBLE };
            let callee = match (self.mbf, single) {
                (false, true) => "B$FCVS",
                (false, false) => "B$FCVD",
                (true, true) => "B$MCVS",
                (true, false) => "B$MCVD",
            };
            // strnum.asm returns AX = the near address of the converted value
            // in the runtime accumulator.  Keep the following load explicit:
            // the call does not return an x87 value, and the runtime-owned
            // memory write is observable to alias/effect analysis.
            let pointer_type = self.pointer_type(type_id);
            let pointer = self.value(pointer_type);
            self.emit_runtime_call(callee, vec![pointer], vec![descriptor]);
            let result = self.value(type_id);
            self.emit(
                "load",
                vec![result],
                vec![Operand::Indirect {
                    base: pointer,
                    offset: 0,
                    type_id,
                }],
            );
            return Ok(Some((Operand::Value(result), type_id)));
        }
        if intrinsic.lowering == Lowering::Val {
            let descriptor = self.string_descriptor(&arguments[0])?;
            // FVAL returns AX = near address of the runtime's DOUBLE DAC,
            // not a floating value in x87 or an integer register. Keep that
            // indirection explicit so the ordinary load/convert pipeline sees
            // the numeric value and the call remains an effect boundary.
            let pointer_type = self.pointer_type(DOUBLE);
            let pointer = self.value(pointer_type);
            self.emit_runtime_call("B$FVAL", vec![pointer], vec![descriptor]);
            let result = self.value(DOUBLE);
            self.emit(
                "load",
                vec![result],
                vec![Operand::Indirect {
                    base: pointer,
                    offset: 0,
                    type_id: DOUBLE,
                }],
            );
            return Ok(Some((Operand::Value(result), DOUBLE)));
        }
        if intrinsic.lowering == Lowering::Eof {
            let (file, file_type) = self.expression(&arguments[0])?;
            let file = self.convert(file, file_type, INTEGER)?;
            let result = self.value(INTEGER);
            self.emit_runtime_call("B$FEOF", vec![result], vec![file]);
            return Ok(Some((Operand::Value(result), INTEGER)));
        }
        self.fail(format!(
            "intrinsic {name} has no semantic lowering for {:?}",
            intrinsic.lowering
        ))
    }

    fn binary_memory_statement(
        &mut self,
        name: &str,
        arguments: &[Expr],
    ) -> Result<(), SemanticError> {
        if name == "BSAVE" {
            let [path, offset, length] = arguments else {
                return self.fail("BSAVE expects a path, offset, and length");
            };
            let path = self.string_descriptor(path)?;
            let (offset, offset_type) = self.expression(offset)?;
            let offset = self.convert(offset, offset_type, INTEGER)?;
            let (length, length_type) = self.expression(length)?;
            let length = self.convert(length, length_type, INTEGER)?;
            self.emit_runtime_call("B$BSAV", Vec::new(), vec![path, offset, length]);
            return Ok(());
        }

        let (path, offset, supplied) = match arguments {
            [path] => (
                path,
                Operand::Constant(INTEGER, Number::Integer(0)),
                0,
            ),
            [path, offset] => {
                let (offset, offset_type) = self.expression(offset)?;
                (path, self.convert(offset, offset_type, INTEGER)?, 1)
            }
            _ => return self.fail("BLOAD expects a path and optional offset"),
        };
        let path = self.string_descriptor(path)?;
        self.emit_runtime_call(
            "B$BLOD",
            Vec::new(),
            vec![
                path,
                offset,
                Operand::Constant(INTEGER, Number::Integer(supplied)),
            ],
        );
        Ok(())
    }

    fn call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        needs_result: bool,
    ) -> Result<Option<(Operand, u32)>, SemanticError> {
        let signature = self
            .signatures
            .get(canonical(name))
            .cloned()
            .ok_or_else(|| SemanticError {
                message: format!(
                    "call to {name} has no source signature or audited runtime ABI contract"
                ),
            })?;
        if signature.parameters.len() != arguments.len() {
            return self.fail(format!(
                "{name} expects {} arguments, got {}",
                signature.parameters.len(),
                arguments.len()
            ));
        }
        let mut operands = Vec::new();
        for (argument, (parameter_type, by_value, segmented, array)) in
            arguments.iter().zip(&signature.parameters)
        {
            if *segmented {
                if *array || *by_value {
                    return self.fail(format!("SEG parameter for {name} must be scalar BYREF"));
                }
                let (place, argument_type) = self.destination(argument)?;
                if *parameter_type != ANY && argument_type != *parameter_type {
                    return self.fail(format!("SEG argument type does not match {name}"));
                }
                if let Operand::Indirect {
                    base, offset: 0, ..
                } = &place
                {
                    let base_type = self
                        .values
                        .iter()
                        .find_map(|(id, type_id)| (*id == *base).then_some(*type_id))
                        .ok_or_else(|| SemanticError {
                            message: format!("SEG argument for {name} has no pointer type"),
                        })?;
                    if self.width(base_type) == 4 {
                        operands.push(Operand::Value(*base));
                        continue;
                    }
                }
                let pointer_type = self.far_pointer_type(argument_type);
                let address = self.value(pointer_type);
                self.emit("address", vec![address], vec![place]);
                operands.push(Operand::Value(address));
                continue;
            }
            if *array {
                let (array_name, subscripts) = match argument {
                    Expr::Apply {
                        name, arguments, ..
                    } => (name, arguments.as_slice()),
                    Expr::Name(name, _) => (name, &[][..]),
                    _ => return self.fail(format!("array argument for {name} is not an array")),
                };
                if !subscripts.is_empty() {
                    return self.fail(format!(
                        "array argument for {name} must use empty parentheses"
                    ));
                }
                let variable = self.variable(array_name)?;
                if *parameter_type != ANY && variable.element != Some(*parameter_type) {
                    return self.fail(format!("array argument type does not match {name}"));
                }
                let descriptor = self.descriptor_pointer(&variable)?;
                operands.push(Operand::Value(descriptor));
            } else if *parameter_type == STRING && self.string_syntax(argument) {
                // A source STRING formal receives a near descriptor address.
                // A BYREF formal must outlive any consuming string operation
                // inside the callee, so non-lvalues are first copied to an
                // owned descriptor. BYVAL retains the expression descriptor.
                operands.push(if *by_value {
                    self.string_descriptor(argument)?
                } else {
                    self.byref_string_argument(argument)?
                });
            } else if *by_value {
                let (operand, argument_type) = self.expression(argument)?;
                let operand = self.convert(operand, argument_type, *parameter_type)?;
                if matches!(*parameter_type, SINGLE | DOUBLE) {
                    // HIR evaluates floating expressions in extended precision,
                    // while QB's procedure ABI passes the declared 4/8-byte
                    // representation by value. Make that rounding boundary
                    // explicit before late stack argument materialization.
                    let place = self.temporary(*parameter_type)?;
                    self.emit("store", Vec::new(), vec![Operand::Place(place), operand]);
                    operands.push(Operand::Place(place));
                } else {
                    operands.push(operand);
                }
            } else {
                let (place, argument_type) = match self.destination(argument) {
                    Ok(place) => place,
                    Err(_) => {
                        // BASIC materializes an addressable temporary when a
                        // BYREF actual is an expression rather than a place.
                        let (value, type_id) = self.expression(argument)?;
                        let value = self.convert(value, type_id, *parameter_type)?;
                        let place = self.temporary(*parameter_type)?;
                        self.emit("store", Vec::new(), vec![Operand::Place(place), value]);
                        (Operand::Place(place), *parameter_type)
                    }
                };
                if argument_type != *parameter_type {
                    return self.fail(format!("BYREF argument type does not match {name}"));
                }
                match place {
                    Operand::Indirect {
                        base, offset: 0, ..
                    } => operands.push(Operand::Value(base)),
                    Operand::Indirect { base, offset, .. } => {
                        // Offsetting a field preserves the address space of
                        // its containing object. In particular, a field of a
                        // dynamic-array UDT remains a far pointer; rebuilding
                        // its type from the scalar formal silently narrowed it
                        // to a near pointer before the BYREF call.
                        let pointer_type = self
                            .values
                            .iter()
                            .find_map(|(id, type_id)| (*id == base).then_some(*type_id))
                            .ok_or_else(|| SemanticError {
                                message: "BYREF indirect base has no pointer type".into(),
                            })?;
                        let offset_type = if self.width(pointer_type) == 4 {
                            LONG
                        } else {
                            INTEGER
                        };
                        let adjusted = self.value(pointer_type);
                        self.emit(
                            "ptr_offset",
                            vec![adjusted],
                            vec![
                                Operand::Value(base),
                                Operand::Constant(offset_type, Number::Integer(offset as i64)),
                            ],
                        );
                        operands.push(Operand::Value(adjusted));
                    }
                    place => {
                        let pointer_type = self.pointer_type(*parameter_type);
                        let address = self.value(pointer_type);
                        self.emit("address", vec![address], vec![place]);
                        operands.push(Operand::Value(address));
                    }
                }
            }
        }
        if signature
            .result
            .is_some_and(|type_id| matches!(type_id, SINGLE | DOUBLE))
            && !signature.cdecl
        {
            // The hidden result slot is a caller local just as in BC output
            // (`lea ax,[bp-N] / push ax`). It is an ABI operand, not a
            // language parameter, so source arity remains unchanged.
            let type_id = signature.result.expect("checked above");
            let place = self.temporary(type_id)?;
            let pointer_type = self.pointer_type(type_id);
            let address = self.value(pointer_type);
            self.emit("address", vec![address], vec![Operand::Place(place)]);
            operands.push(Operand::Value(address));
        }
        let mut results = Vec::new();
        let result = signature.result.map(|type_id| {
            // Like the string runtime functions, a source FUNCTION AS STRING
            // returns a near descriptor address in AX. The declared STRING
            // type describes the language result; it is not a four-byte
            // register result.
            let result_type = if type_id == STRING {
                self.pointer_type(STRING)
            } else {
                type_id
            };
            let value = self.value(result_type);
            results.push(value);
            (Operand::Value(value), result_type)
        });
        if needs_result && result.is_none() {
            return self.fail(format!("SUB {name} is used as an expression"));
        }
        // BASIC's ordinary convention is Pascal: evaluate and push the first
        // formal first. DECLARE ... CDECL alone uses C's right-to-left stack
        // order and caller cleanup. VBDOS SYS's raw COM_TOKENIZE site makes
        // the distinction observable with four BYREF arguments.
        let order = if signature.cdecl {
            (0..operands.len()).rev().collect()
        } else {
            (0..operands.len()).collect()
        };
        self.emit_call_to(
            &signature.callee,
            results,
            operands,
            order,
            signature.cdecl,
            Some(signature.symbol),
        );
        Ok(result)
    }

    fn binary(
        &mut self,
        op: Binary,
        left: &Expr,
        right: &Expr,
    ) -> Result<(Operand, u32), SemanticError> {
        if op == Binary::Power {
            return self.power(left, right);
        }
        let comparison = matches!(
            op,
            Binary::Eq
                | Binary::NotEqual
                | Binary::Less
                | Binary::LessEqual
                | Binary::Greater
                | Binary::GreaterEqual
        );
        if comparison && self.string_syntax(left) && self.string_syntax(right) {
            let left = self.string_descriptor(left)?;
            let right = self.string_descriptor(right)?;
            let result = self.value(BOOLEAN);
            let operation = match op {
                Binary::Eq => "string_eq",
                Binary::NotEqual => "string_ne",
                Binary::Less => "string_lt",
                Binary::LessEqual => "string_le",
                Binary::Greater => "string_gt",
                Binary::GreaterEqual => "string_ge",
                _ => unreachable!(),
            };
            self.emit_string_compare(operation, result, left, right);
            return Ok((Operand::Value(result), BOOLEAN));
        }
        let (mut left_operand, mut left_type) = self.expression(left)?;
        let (mut right_operand, mut right_type) = self.expression(right)?;
        let integral = matches!(
            op,
            Binary::And
                | Binary::Or
                | Binary::Xor
                | Binary::Eqv
                | Binary::Imp
                | Binary::Modulo
                | Binary::IntegerDivide
        );
        if integral {
            // QB rounds floating operands before every integral/logical
            // operator. Any floating operand selects LONG evaluation; BYTE is
            // promoted to INTEGER. Keep both conversions explicit in HIR.
            let target = if matches!(left_type, SINGLE | DOUBLE)
                || matches!(right_type, SINGLE | DOUBLE)
                || left_type == LONG
                || right_type == LONG
            {
                LONG
            } else {
                INTEGER
            };
            left_operand = self.convert(left_operand, left_type, target)?;
            right_operand = self.convert(right_operand, right_type, target)?;
            left_type = target;
            right_type = target;
        }
        let common = common_type(left_type, right_type, op)?;
        let left_operand = self.convert(left_operand, left_type, common)?;
        let right_operand = self.convert(right_operand, right_type, common)?;
        if op == Binary::Imp {
            let temporary = self.value(common);
            self.emit("not", vec![temporary], vec![left_operand]);
            let combined = self.value(common);
            self.emit(
                "or",
                vec![combined],
                vec![Operand::Value(temporary), right_operand],
            );
            return Ok((Operand::Value(combined), common));
        }
        if op == Binary::Eqv {
            let combined = self.value(common);
            self.emit("xor", vec![combined], vec![left_operand, right_operand]);
            let result = self.value(common);
            self.emit("not", vec![result], vec![Operand::Value(combined)]);
            return Ok((Operand::Value(result), common));
        }
        let result_type = if comparison { BOOLEAN } else { common };
        let result = self.value(result_type);
        let operation = if matches!(common, SINGLE | DOUBLE) {
            match op {
                Binary::Add => "fadd",
                Binary::Subtract => "fsub",
                Binary::Multiply => "fmul",
                Binary::Divide => "fdiv",
                _ => binary_name(op),
            }
        } else {
            binary_name(op)
        };
        self.emit(operation, vec![result], vec![left_operand, right_operand]);
        Ok((Operand::Value(result), result_type))
    }

    fn string_syntax(&self, expression: &Expr) -> bool {
        if self
            .place_syntax_type(expression)
            .is_some_and(|type_id| self.string_width(type_id).is_some())
        {
            return true;
        }
        match expression {
            Expr::Literal(Literal::String(_), _) => true,
            Expr::Name(name, _)
                if intrinsics::find(canonical(name), self.dialect)
                    .is_some_and(|intrinsic| intrinsic.result == ResultClass::String) =>
            {
                true
            }
            Expr::Name(name, _) => self.bare_function_type(name) == Some(STRING),
            Expr::Apply { name, .. }
                if intrinsics::find(canonical(name), self.dialect)
                    .is_some_and(|intrinsic| intrinsic.result == ResultClass::String) =>
            {
                true
            }
            Expr::Apply { name, .. } => self
                .signatures
                .get(canonical(name))
                .is_some_and(|signature| signature.result == Some(STRING)),
            Expr::Binary {
                op: Binary::Add,
                left,
                right,
                ..
            } => self.string_syntax(left) && self.string_syntax(right),
            _ => false,
        }
    }

    fn bare_function_type(&self, name: &str) -> Option<u32> {
        if self.variables.contains_key(canonical(name)) {
            return None;
        }
        self.signatures.get(canonical(name)).and_then(|signature| {
            signature
                .parameters
                .is_empty()
                .then_some(signature.result)
                .flatten()
        })
    }

    fn place_syntax_type(&self, expression: &Expr) -> Option<u32> {
        match expression {
            Expr::Name(name, _) => self.variables.get(canonical(name)).map(|one| one.type_id),
            Expr::Apply { name, .. } => self
                .variables
                .get(canonical(name))
                .and_then(|one| one.element),
            Expr::Index { base, .. } => {
                let base = self.place_syntax_type(base)?;
                self.types.iter().find(|one| one.id == base)?.element
            }
            Expr::Field { base, name, .. } => {
                let base = self.place_syntax_type(base)?;
                self.udts
                    .values()
                    .find(|one| one.type_id == base)?
                    .fields
                    .get(canonical(name))
                    .map(|one| one.type_id)
            }
            _ => None,
        }
    }

    fn power(&mut self, left: &Expr, right: &Expr) -> Result<(Operand, u32), SemanticError> {
        if let Ok((exponent_type, exponent_number)) = self.constant(right) {
            let exponent_real = as_real(&exponent_number)?;
            if exponent_real.fract() == 0.0
                && exponent_real >= i64::MIN as f64
                && exponent_real <= i64::MAX as f64
            {
                // An integral constant exponent is valid for a signed base.
                // Exponentiation by squaring keeps it entirely in HIR and
                // needs logarithmic multiplies; negative powers add one
                // reciprocal. This is QGL's pervasive signed `delta ^ 2`.
                let exponent = exponent_real as i64;
                let (base, base_type) = self.expression(left)?;
                let common = common_type(base_type, exponent_type, Binary::Power)?;
                let mut factor = self.convert(base, base_type, common)?;
                let mut result = None;
                let mut magnitude = exponent.unsigned_abs();
                while magnitude != 0 {
                    if magnitude & 1 != 0 {
                        result = Some(if let Some(product) = result {
                            let multiplied = self.value(common);
                            self.emit("fmul", vec![multiplied], vec![product, factor.clone()]);
                            Operand::Value(multiplied)
                        } else {
                            factor.clone()
                        });
                    }
                    magnitude >>= 1;
                    if magnitude != 0 {
                        // A self-multiply names one HIR value twice. Keep the
                        // two x87 operands distinct at the frontend boundary:
                        // the backend deliberately has no cross-block or
                        // duplicate-value float-stack repair tier.
                        let factor_place = self.temporary(common)?;
                        self.emit(
                            "store",
                            Vec::new(),
                            vec![Operand::Place(factor_place), factor],
                        );
                        let left_factor = self.value(common);
                        self.emit(
                            "load",
                            vec![left_factor],
                            vec![Operand::Place(factor_place)],
                        );
                        let right_factor = self.value(common);
                        self.emit(
                            "load",
                            vec![right_factor],
                            vec![Operand::Place(factor_place)],
                        );
                        let squared = self.value(common);
                        self.emit(
                            "fmul",
                            vec![squared],
                            vec![Operand::Value(left_factor), Operand::Value(right_factor)],
                        );
                        factor = Operand::Value(squared);
                    }
                }
                let Some(result) = result else {
                    return Ok((self.floating_literal("1.0", common)?, common));
                };
                if exponent < 0 {
                    let one = self.floating_literal("1.0", common)?;
                    let reciprocal = self.value(common);
                    self.emit("fdiv", vec![reciprocal], vec![one, result]);
                    return Ok((Operand::Value(reciprocal), common));
                }
                return Ok((result, common));
            }
        }

        // x87 has no single POW instruction. Keep the mathematical structure
        // visible as log2(base), exponent multiply, exp2. This identity is
        // valid for a positive base; reject other domains until their
        // sign/integer-exponent CFG is represented.
        let (base_type, base) = self.constant(left).map_err(|_| SemanticError {
            message: "inline power currently requires a positive constant base".into(),
        })?;
        if as_real(&base)? <= 0.0 {
            return self.fail("inline power currently requires a positive constant base");
        }
        let (exponent, exponent_type) = self.expression(right)?;
        let common = common_type(base_type, exponent_type, Binary::Power)?;
        let exponent = self.convert(exponent, exponent_type, common)?;

        // The renderer's pervasive 2^i case is exact. Strength-reduce every
        // positive integral power-of-two base generally, so no FLD/FYL2X is
        // left for a compile-time logarithm.
        let logarithm = match base {
            Number::Integer(value) if value > 0 && (value & (value - 1)) == 0 => {
                self.floating_literal(&(value.ilog2()).to_string(), common)?
            }
            base => {
                let base = match base {
                    Number::Integer(value) => Operand::Constant(base_type, Number::Integer(value)),
                    Number::Real(value) => self.floating_literal(&value, base_type)?,
                };
                let base = self.convert(base, base_type, common)?;
                let logarithm = self.value(common);
                self.emit("flog2", vec![logarithm], vec![base]);
                Operand::Value(logarithm)
            }
        };
        let product = self.value(common);
        self.emit("fmul", vec![product], vec![logarithm, exponent]);
        let result = self.value(common);
        self.emit("fexp2", vec![result], vec![Operand::Value(product)]);
        Ok((Operand::Value(result), common))
    }

    fn convert(&mut self, operand: Operand, from: u32, to: u32) -> Result<Operand, SemanticError> {
        if from == to {
            return Ok(operand);
        }
        let numeric =
            |type_id| matches!(type_id, INTEGER | LONG | SINGLE | DOUBLE | BOOLEAN | BYTE);
        let allowed = numeric(from) && numeric(to);
        if !allowed {
            return self.fail(format!(
                "unsupported implicit conversion {} to {}",
                self.name(from),
                self.name(to)
            ));
        }
        if matches!(from, INTEGER | LONG | BOOLEAN | BYTE) && matches!(to, SINGLE | DOUBLE) {
            let place = self.temporary(from)?;
            self.emit("store", Vec::new(), vec![Operand::Place(place), operand]);
            let result = self.value(to);
            self.emit("convert", vec![result], vec![Operand::Place(place)]);
            return Ok(Operand::Value(result));
        }
        let result = self.value(to);
        self.emit("convert", vec![result], vec![operand]);
        Ok(Operand::Value(result))
    }

    fn variable(&mut self, name: &str) -> Result<Variable, SemanticError> {
        if let Some(variable) = self.variables.get(canonical(name)) {
            return Ok(variable.clone());
        }
        let type_id = self.named_type(name, None)?;
        let declaration = Declaration {
            name: name.into(),
            type_name: Some(type_name(type_id)),
            array: false,
            bounds: Vec::new(),
            fixed_length: None,
            shared: false,
            dynamic: false,
            span: crate::syntax::Span {
                line: 0,
                start: 0,
                end: 0,
            },
        };
        self.declare_as(&declaration, self.implicit_storage)?;
        Ok(self.variables[canonical(name)].clone())
    }

    fn constant(&self, expression: &Expr) -> Result<(u32, Number), SemanticError> {
        match expression {
            Expr::Literal(Literal::Integer(value, type_name), _) => {
                Ok((type_id(Some(type_name))?, Number::Integer(*value)))
            }
            Expr::Literal(Literal::Real(value, type_name), _) => {
                Ok((type_id(Some(type_name))?, Number::Real(value.clone())))
            }
            Expr::Literal(Literal::String(_), _) => {
                self.fail("string constant expressions are runtime-only")
            }
            Expr::Name(name, _) => {
                self.constants
                    .get(canonical(name))
                    .cloned()
                    .ok_or_else(|| SemanticError {
                        message: format!("unknown constant {name}"),
                    })
            }
            Expr::Field { .. } => {
                let name = dotted_name(expression).expect("field chain");
                self.constants
                    .get(canonical(&name))
                    .cloned()
                    .ok_or_else(|| SemanticError {
                        message: format!("unknown constant {name}"),
                    })
            }
            Expr::Unary { op, operand, .. } => {
                let (type_id, value) = self.constant(operand)?;
                match (op, value) {
                    (Unary::Positive, value) => Ok((type_id, value)),
                    (Unary::Negative, Number::Integer(value)) => {
                        Ok((type_id, Number::Integer(narrow(-value, type_id))))
                    }
                    (Unary::Negative, Number::Real(value)) => {
                        Ok((type_id, real(-parse_real(&value)?, type_id)))
                    }
                    (Unary::Not, Number::Integer(value)) => {
                        Ok((type_id, Number::Integer(narrow(!value, type_id))))
                    }
                    (Unary::Not, Number::Real(_)) => self.fail("NOT requires an integral constant"),
                }
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let (left_type, left) = self.constant(left)?;
                let (right_type, right) = self.constant(right)?;
                constant_binary(*op, left_type, left, right_type, right)
            }
            Expr::Apply { .. } | Expr::Index { .. } => {
                self.fail("function call or subscript is not a constant expression")
            }
        }
    }

    fn constant_integer(&self, expression: &Expr) -> Result<i64, SemanticError> {
        match self.constant(expression)? {
            (_, Number::Integer(value)) => Ok(value),
            _ => self.fail("array bound is not an integer constant"),
        }
    }

    fn value(&mut self, type_id: u32) -> u32 {
        let id = self.next_value;
        self.next_value += 1;
        self.values.push((id, type_id));
        id
    }

    fn temporary(&mut self, type_id: u32) -> Result<u32, SemanticError> {
        self.compiler_temporary("$arg", type_id)
    }

    fn compiler_temporary(&mut self, prefix: &str, type_id: u32) -> Result<u32, SemanticError> {
        let extent = self.width(type_id);
        let storage = self.implicit_storage;
        if storage != "static" && self.data_offset + extent > 65536 {
            return self.fail("temporary exceeds the 64 KiB frame budget");
        }
        let (offset, symbol) = if storage == "static" {
            let symbol = self.next_data;
            self.next_data += 1;
            self.data.push(DataObject {
                id: symbol,
                name: format!("{prefix}${}", self.next_place),
                bytes: vec![0; extent],
                readonly: false,
                relocations: Vec::new(),
                linkage: "internal",
                address: "near",
            });
            (0, symbol)
        } else {
            (
                self.place_offset(storage, extent),
                if storage == "local" { 0 } else { 1 },
            )
        };
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id,
            name: format!("{prefix}{id}"),
            type_id,
            offset,
            extent,
            storage,
            symbol,
        });
        if storage != "static" {
            self.data_offset += extent;
            self.reserve_module_data(storage);
        }
        Ok(id)
    }

    fn owned_string_temporary(&mut self) -> Result<u32, SemanticError> {
        let extent = self.width(STRING);
        if self.data_offset + extent > 65536 {
            return self.fail("STRING argument temporary exceeds the 64 KiB storage budget");
        }
        let storage = self.implicit_storage;
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id,
            name: format!("$stringArg{id}"),
            type_id: STRING,
            offset: self.place_offset(storage, extent),
            extent,
            storage,
            symbol: if storage == "local" { 0 } else { 1 },
        });
        self.data_offset += extent;
        self.reserve_module_data(storage);
        Ok(id)
    }

    fn def_segment_place(&mut self) -> u32 {
        let symbol = if let Some(symbol) = self.def_segment_symbol {
            symbol
        } else {
            let symbol = self.next_data;
            self.next_data += 1;
            self.data.push(DataObject {
                id: symbol,
                name: "b$seg".into(),
                bytes: Vec::new(),
                readonly: false,
                relocations: Vec::new(),
                linkage: "external",
                address: "near",
            });
            self.def_segment_symbol = Some(symbol);
            symbol
        };
        if let Some(place) = self
            .places
            .iter()
            .find(|place| place.storage == "external" && place.symbol == symbol)
        {
            return place.id;
        }
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: place,
            name: "b$seg".into(),
            type_id: INTEGER,
            offset: 0,
            extent: 2,
            storage: "external",
            symbol,
        });
        place
    }

    fn string_literal(&mut self, text: &str) -> Result<u32, SemanticError> {
        Ok(self.string_literal_places(text)?.0)
    }

    fn floating_literal(&mut self, text: &str, type_id: u32) -> Result<Operand, SemanticError> {
        let bytes = match type_id {
            SINGLE => text
                .parse::<f32>()
                .map(f32::to_le_bytes)
                .map(Vec::from)
                .map_err(|_| SemanticError {
                    message: format!("invalid SINGLE constant {text}"),
                })?,
            DOUBLE => text
                .parse::<f64>()
                .map(f64::to_le_bytes)
                .map(Vec::from)
                .map_err(|_| SemanticError {
                    message: format!("invalid DOUBLE constant {text}"),
                })?,
            _ => return self.fail("floating literal has a non-floating type"),
        };
        // BC pools identical floating constants across a whole module. Apart
        // from wasting BC_CN, keeping one object per occurrence can consume
        // the near string heap before the first BASIC statement in a large
        // program: QGL's 19 modules crossed that boundary by 3440 bytes.
        let key = (type_id, bytes.clone());
        let symbol = if let Some(symbol) = self.floating_literals.get(&key) {
            *symbol
        } else {
            let symbol = self.next_data;
            self.next_data += 1;
            self.data.push(DataObject {
                id: symbol,
                name: format!("$float{symbol}"),
                bytes,
                readonly: true,
                relocations: Vec::new(),
                linkage: "internal",
                address: "near",
            });
            self.floating_literals.insert(key, symbol);
            symbol
        };
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: place,
            name: format!("$float{symbol}"),
            type_id,
            offset: 0,
            extent: self.width(type_id),
            storage: "static",
            symbol,
        });
        let value = self.value(type_id);
        self.emit("load", vec![value], vec![Operand::Place(place)]);
        Ok(Operand::Value(value))
    }

    fn string_literal_places(&mut self, text: &str) -> Result<(u32, u32, u32), SemanticError> {
        if !text.is_ascii() {
            return self.fail("non-ASCII string literals require an explicit source code page");
        }
        if text.len() > i16::MAX as usize {
            return self.fail("string literal exceeds the BASIC string limit");
        }
        let payload_symbol = self.next_data;
        self.next_data += 1;

        // QB 4.5 and PDS 7.1 keep one ordinary SD and its bytes together in
        // BC_CN. VBDOS instead keeps immutable bytes in FSL_CONST: BC_CN then
        // holds a near bridge to a far SD plus the shared selector-word
        // address. These are different runtime profiles, not dialect syntax.
        let (descriptor_symbol, payload_offset) = if self.runtime == "vbdos" {
            let segment_symbol = if let Some(symbol) = self.far_string_segment_symbol {
                symbol
            } else {
                let symbol = self.next_data;
                self.next_data += 1;
                self.data.push(DataObject {
                    id: symbol,
                    name: "$fslSegment".into(),
                    bytes: vec![0, 0],
                    readonly: true,
                    relocations: vec![DataRelocation {
                        at: 0,
                        target: payload_symbol,
                        addend: 0,
                        address: "segment",
                    }],
                    linkage: "internal",
                    address: "near",
                });
                self.far_string_segment_symbol = Some(symbol);
                symbol
            };

            let mut payload_bytes = vec![0, 0, 0, 0];
            payload_bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
            payload_bytes.extend_from_slice(text.as_bytes());
            if payload_bytes.len() % 2 != 0 {
                payload_bytes.push(0);
            }
            self.data.push(DataObject {
                id: payload_symbol,
                name: format!("$string{payload_symbol}$payload"),
                bytes: payload_bytes,
                readonly: true,
                relocations: vec![DataRelocation {
                    at: 2,
                    target: payload_symbol,
                    addend: 4,
                    address: "near",
                }],
                linkage: "internal",
                address: "far",
            });

            let descriptor_symbol = self.next_data;
            self.next_data += 1;
            self.data.push(DataObject {
                id: descriptor_symbol,
                name: format!("$string{payload_symbol}$descriptor"),
                bytes: vec![0, 0, 0, 0],
                readonly: true,
                relocations: vec![
                    DataRelocation {
                        at: 0,
                        target: payload_symbol,
                        addend: 2,
                        address: "near",
                    },
                    DataRelocation {
                        at: 2,
                        target: segment_symbol,
                        addend: 0,
                        address: "near",
                    },
                ],
                linkage: "internal",
                address: "near",
            });
            (descriptor_symbol, 6)
        } else {
            let mut literal = Vec::from((text.len() as u16).to_le_bytes());
            literal.extend_from_slice(&[0, 0]);
            literal.extend_from_slice(text.as_bytes());
            if literal.len() % 2 != 0 {
                literal.push(0);
            }
            self.data.push(DataObject {
                id: payload_symbol,
                name: format!("$string{payload_symbol}"),
                bytes: literal,
                readonly: true,
                relocations: vec![DataRelocation {
                    at: 2,
                    target: payload_symbol,
                    addend: 4,
                    address: "near",
                }],
                linkage: "internal",
                address: "near",
            });
            (payload_symbol, 4)
        };
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: place,
            name: format!("$string{payload_symbol}$descriptor"),
            type_id: STRING,
            offset: 0,
            extent: 4,
            storage: "static",
            symbol: descriptor_symbol,
        });
        let payload_type = self.opaque_type(format!("string*{}", text.len()), text.len());
        let payload = self.next_place;
        self.next_place += 1;
        self.places.push(Place {
            id: payload,
            name: format!("$string{payload_symbol}$payload"),
            type_id: payload_type,
            offset: payload_offset,
            extent: text.len(),
            storage: "static",
            symbol: payload_symbol,
        });
        Ok((place, payload, payload_type))
    }

    fn emit(&mut self, op: &'static str, results: Vec<u32>, operands: Vec<Operand>) {
        let id = self.next_instruction;
        self.next_instruction += 1;
        self.blocks[self.current_block]
            .instructions
            .push(Instruction {
                id,
                op,
                results,
                operands,
                callee: None,
            });
    }

    fn emit_runtime_call(&mut self, callee: &str, results: Vec<u32>, operands: Vec<Operand>) {
        let order = (0..operands.len()).collect();
        self.emit_call(callee, results, operands, order, false);
    }

    fn emit_string_compare(
        &mut self,
        op: &'static str,
        result: u32,
        left: Operand,
        right: Operand,
    ) {
        let id = self.next_instruction;
        self.next_instruction += 1;
        self.calls.push(CallAbi {
            instruction: id,
            order: vec![0, 1],
            caller_cleanup: false,
            callee: None,
        });
        self.blocks[self.current_block]
            .instructions
            .push(Instruction {
                id,
                op,
                results: vec![result],
                operands: vec![left, right],
                callee: Some("B$SCMP".into()),
            });
        self.invalidate_descriptor_cache();
    }

    fn emit_call(
        &mut self,
        callee: &str,
        results: Vec<u32>,
        operands: Vec<Operand>,
        order: Vec<usize>,
        caller_cleanup: bool,
    ) {
        self.emit_call_to(callee, results, operands, order, caller_cleanup, None);
    }

    fn emit_call_to(
        &mut self,
        callee: &str,
        results: Vec<u32>,
        operands: Vec<Operand>,
        order: Vec<usize>,
        caller_cleanup: bool,
        symbol: Option<u32>,
    ) {
        let id = self.next_instruction;
        self.next_instruction += 1;
        self.calls.push(CallAbi {
            instruction: id,
            order,
            caller_cleanup,
            callee: symbol,
        });
        self.blocks[self.current_block]
            .instructions
            .push(Instruction {
                id,
                op: "call",
                results,
                operands,
                callee: Some(callee.into()),
            });
        self.invalidate_descriptor_cache();
    }

    fn new_block(&mut self) -> u32 {
        let id = self.next_block;
        self.next_block += 1;
        self.blocks.push(Block {
            id,
            instructions: Vec::new(),
            terminator: None,
        });
        id
    }

    fn select_block(&mut self, id: u32) {
        self.current_block = self
            .blocks
            .iter()
            .position(|block| block.id == id)
            .expect("created block");
        self.invalidate_descriptor_cache();
    }

    fn block_open(&self) -> bool {
        self.blocks[self.current_block].terminator.is_none()
    }

    fn terminate(
        &mut self,
        kind: &'static str,
        operands: Vec<Operand>,
        targets: Vec<u32>,
    ) -> Result<(), SemanticError> {
        if !self.block_open() {
            return self.fail("statement follows a terminating control transfer");
        }
        self.blocks[self.current_block].terminator = Some(Terminator {
            kind,
            operands,
            targets,
        });
        Ok(())
    }

    fn jump_if_open(&mut self, target: u32) {
        if self.block_open() {
            self.blocks[self.current_block].terminator = Some(Terminator {
                kind: "jump",
                operands: Vec::new(),
                targets: vec![target],
            });
        }
    }

    fn return_block(&mut self) -> u32 {
        if let Some(block) = self.return_block {
            block
        } else {
            let block = self.new_block();
            self.return_block = Some(block);
            block
        }
    }

    fn finish(&mut self) {
        if let Some((place, type_id)) = self.result_place {
            let target = self.return_block();
            for block in &mut self.blocks {
                if block.id != target && block.terminator.is_none() {
                    block.terminator = Some(Terminator {
                        kind: "jump",
                        operands: Vec::new(),
                        targets: vec![target],
                    });
                }
            }
            self.select_block(target);
            let result = if type_id == STRING {
                // A function's local result descriptor dies with its runtime
                // frame.  Microsoft BASIC's SCPF copies it to the temporary
                // string chain and returns the surviving descriptor address
                // in AX.  Returning the descriptor's four bytes instead both
                // invents an AX:DX convention and leaves the caller pointing
                // at freed string storage.
                let descriptor = self.near_string_address(Operand::Place(place));
                let pointer_type = self.pointer_type(STRING);
                let result = self.value(pointer_type);
                self.emit_runtime_call("B$SCPF", vec![result], vec![descriptor]);
                result
            } else {
                let result = self.value(type_id);
                self.emit("load", vec![result], vec![Operand::Place(place)]);
                result
            };
            self.blocks[self.current_block].terminator = Some(Terminator {
                kind: "return",
                operands: vec![Operand::Value(result)],
                targets: Vec::new(),
            });
        }
        for block in &mut self.blocks {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator {
                    kind: "return",
                    operands: Vec::new(),
                    targets: Vec::new(),
                });
            }
        }
    }

    fn cleanup_local_arrays(&mut self) -> Result<(), SemanticError> {
        let descriptors: Vec<(u32, u32, &'static str)> = self
            .variables
            .values()
            .filter_map(|variable| {
                variable
                    .descriptor_place
                    .map(|place| (place, variable.element))
            })
            .filter_map(|(place, element)| {
                self.places
                    .iter()
                    .find(|one| one.id == place && one.storage == "local")
                    .map(|one| {
                        (
                            place,
                            one.type_id,
                            if element == Some(STRING) {
                                "B$ERS1"
                            } else {
                                "B$ERAS"
                            },
                        )
                    })
            })
            .collect();
        if descriptors.is_empty() {
            return Ok(());
        }
        let returns: Vec<usize> = self
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(index, block)| {
                block
                    .terminator
                    .as_ref()
                    .is_some_and(|one| one.kind == "return")
                    .then_some(index)
            })
            .collect();
        for block in returns {
            self.current_block = block;
            for (place, type_id, callee) in &descriptors {
                let pointer_type = self.pointer_type(*type_id);
                let pointer = self.value(pointer_type);
                self.emit("address", vec![pointer], vec![Operand::Place(*place)]);
                self.emit_runtime_call(callee, Vec::new(), vec![Operand::Value(pointer)]);
            }
        }
        Ok(())
    }

    fn width(&self, type_id: u32) -> usize {
        self.types
            .iter()
            .find(|one| one.id == type_id)
            .expect("built-in type")
            .width
    }

    fn name(&self, type_id: u32) -> &str {
        &self
            .types
            .iter()
            .find(|one| one.id == type_id)
            .expect("known type")
            .name
    }

    fn json(&self) -> String {
        let mut out = String::new();
        write!(
            out,
            "{{\"dialect\":\"{}\",\"modules\":[{{\"functions\":[",
            dialect_name(self.dialect)
        )
        .unwrap();
        for (function_index, function) in self.functions.iter().enumerate() {
            if function_index != 0 {
                out.push(',');
            }
            out.push_str("{\"blocks\":[");
            for (block_index, block) in function.blocks.iter().enumerate() {
                if block_index != 0 {
                    out.push(',');
                }
                write!(out, "{{\"id\":{},\"instructions\":[", block.id).unwrap();
                for (index, instruction) in block.instructions.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    write!(out, "{{\"callee\":",).unwrap();
                    if let Some(callee) = &instruction.callee {
                        string(&mut out, callee);
                    } else {
                        out.push_str("null");
                    }
                    write!(
                        out,
                        ",\"id\":{},\"op\":\"{}\",\"operands\":[",
                        instruction.id, instruction.op
                    )
                    .unwrap();
                    for (operand_index, operand) in instruction.operands.iter().enumerate() {
                        if operand_index != 0 {
                            out.push(',');
                        }
                        operand_json(&mut out, operand);
                    }
                    out.push_str("],\"pure\":false,\"results\":[");
                    numbers(&mut out, &instruction.results);
                    out.push_str("]}");
                }
                let terminator = block.terminator.as_ref().expect("compiler finishes blocks");
                out.push_str("],\"terminator\":{\"cases\":[],\"kind\":");
                string(&mut out, terminator.kind);
                out.push_str(",\"operands\":[");
                for (index, operand) in terminator.operands.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    operand_json(&mut out, operand);
                }
                out.push_str("],\"targets\":[");
                numbers(&mut out, &terminator.targets);
                out.push_str("]}}");
            }
            write!(
                out,
                "],\"abi\":{{\"cleanup\":\"{}\",\"distance\":\"far\",\"parameter_bytes\":{}}},\"calls\":[",
                if function.caller_cleanup { "caller" } else { "callee" },
                function.parameter_bytes
            )
            .unwrap();
            for (index, call) in function.calls.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                write!(
                    out,
                    "{{\"callee\":{},\"cleanup\":\"{}\",\"distance\":\"far\",\"instruction\":{},\"order\":[",
                    call.callee
                        .map(|one| one.to_string())
                        .unwrap_or_else(|| "null".into()),
                    if call.caller_cleanup {
                        "caller"
                    } else {
                        "callee"
                    },
                    call.instruction
                )
                .unwrap();
                for (argument, number) in call.order.iter().enumerate() {
                    if argument != 0 {
                        out.push(',');
                    }
                    write!(out, "{number}").unwrap();
                }
                out.push_str("]}");
            }
            out.push_str("],\"entry\":1,\"error_handler\":");
            match function.error_handler {
                Some(block) => write!(out, "{block}").unwrap(),
                None => out.push_str("null"),
            }
            write!(
                out,
                ",\"error_handler_local\":{}",
                if function.error_handler_local {
                    "true"
                } else {
                    "false"
                }
            )
            .unwrap();
            out.push_str(",\"external_entries\":[");
            numbers(&mut out, &function.external_entries);
            write!(out, "],\"id\":{},\"name\":", function.id).unwrap();
            string(&mut out, &function.name);
            out.push_str(",\"parameters\":[");
            numbers(&mut out, &function.parameters);
            out.push_str("],\"places\":[");
            for (index, place) in function.places.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                let address = if place.storage == "static" {
                    self.data
                        .iter()
                        .find_map(|object| (object.id == place.symbol).then_some(object.address))
                        .unwrap_or("near")
                } else {
                    "near"
                };
                write!(
                    out,
                    "{{\"address\":\"{}\",\"extent\":{},\"id\":{},\"name\":",
                    address, place.extent, place.id
                )
                .unwrap();
                string(&mut out, &place.name);
                write!(
                    out,
                    ",\"offset\":{},\"storage\":\"{}\",\"symbol\":{},\"type\":{}}}",
                    place.offset, place.storage, place.symbol, place.type_id
                )
                .unwrap();
            }
            write!(
                out,
                "],\"result_type\":{},\"values\":[",
                function.result_type
            )
            .unwrap();
            for (index, (id, type_id)) in function.values.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                write!(out, "{{\"id\":{id},\"type\":{type_id}}}").unwrap();
            }
            out.push_str("]}");
        }
        out.push_str("],\"callables\":[");
        for (index, callable) in self.callables.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            write!(out, "{{\"arrays\":[").unwrap();
            for (parameter, (_, _, _, array)) in callable.parameters.iter().enumerate() {
                if parameter != 0 {
                    out.push(',');
                }
                out.push_str(if *array { "true" } else { "false" });
            }
            out.push_str("],\"by_value\":[");
            for (parameter, (_, by_value, _, _)) in callable.parameters.iter().enumerate() {
                if parameter != 0 {
                    out.push(',');
                }
                out.push_str(if *by_value { "true" } else { "false" });
            }
            write!(
                out,
                "],\"defined\":{},\"id\":{},\"name\":",
                callable.defined, callable.id
            )
            .unwrap();
            string(&mut out, &callable.name);
            out.push_str(",\"parameter_types\":[");
            for (parameter, (type_id, _, _, _)) in callable.parameters.iter().enumerate() {
                if parameter != 0 {
                    out.push(',');
                }
                write!(out, "{type_id}").unwrap();
            }
            out.push_str("],\"result_type\":");
            match callable.result_type {
                Some(type_id) => write!(out, "{type_id}").unwrap(),
                None => out.push_str("null"),
            }
            out.push_str(",\"segmented\":[");
            for (parameter, (_, _, segmented, _)) in callable.parameters.iter().enumerate() {
                if parameter != 0 {
                    out.push(',');
                }
                out.push_str(if *segmented { "true" } else { "false" });
            }
            out.push_str("]}");
        }
        out.push_str("],\"data\":[");
        for (index, object) in self.data.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            write!(out, "{{\"bytes\":[").unwrap();
            for (byte_index, byte) in object.bytes.iter().enumerate() {
                if byte_index != 0 {
                    out.push(',');
                }
                write!(out, "{byte}").unwrap();
            }
            write!(
                out,
                "],\"address\":\"{}\",\"id\":{},\"linkage\":\"{}\",\"name\":",
                object.address, object.id, object.linkage
            )
            .unwrap();
            string(&mut out, &object.name);
            write!(out, ",\"readonly\":{},\"relocations\":[", object.readonly).unwrap();
            for (relocation_index, relocation) in object.relocations.iter().enumerate() {
                if relocation_index != 0 {
                    out.push(',');
                }
                write!(
                    out,
                    "{{\"addend\":{},\"address\":\"{}\",\"at\":{},\"target\":{}}}",
                    relocation.addend, relocation.address, relocation.at, relocation.target
                )
                .unwrap();
            }
            out.push_str("]}");
        }
        out.push_str("],\"id\":1,\"name\":");
        string(&mut out, &self.module_name);
        out.push_str(",\"types\":[");
        for (index, type_) in self.types.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            type_json(&mut out, type_);
        }
        write!(
            out,
            "]}}],\"runtime\":\"{}\",\"schema\":1,\"target\":\"i386-real-mode\",\"array_order\":\"{}\",\"float_mode\":\"{}\"}}\n",
            self.runtime,
            if self.row_major { "row-major" } else { "column-major" },
            if self.alternate_math {
                "alternate"
            } else {
                "inline"
            }
        )
        .unwrap();
        out
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T, SemanticError> {
        Err(SemanticError {
            message: message.into(),
        })
    }
}

fn scalar(
    id: u32,
    name: &str,
    kind: &'static str,
    width: usize,
    signed: Option<bool>,
    evaluation: &'static str,
) -> Type {
    Type {
        id,
        name: name.into(),
        kind,
        width,
        signed,
        evaluation,
        element: None,
        bounds: Vec::new(),
        address: "none",
    }
}

fn type_id(type_name: Option<&TypeName>) -> Result<u32, SemanticError> {
    Ok(match type_name.unwrap_or(&TypeName::Single) {
        TypeName::Integer => INTEGER,
        TypeName::Long => LONG,
        TypeName::Single => SINGLE,
        TypeName::Double => DOUBLE,
        TypeName::String => STRING,
        TypeName::Named(name) => {
            return Err(SemanticError {
                message: format!("user-defined type {name} is not attached yet"),
            })
        }
    })
}

fn type_name(type_id: u32) -> TypeName {
    match type_id {
        INTEGER => TypeName::Integer,
        LONG => TypeName::Long,
        SINGLE => TypeName::Single,
        DOUBLE => TypeName::Double,
        STRING => TypeName::String,
        _ => unreachable!(),
    }
}

fn suffix(name: &str) -> Option<TypeName> {
    match name.as_bytes().last() {
        Some(b'%') => Some(TypeName::Integer),
        Some(b'&') => Some(TypeName::Long),
        Some(b'!') => Some(TypeName::Single),
        Some(b'#') => Some(TypeName::Double),
        Some(b'$') => Some(TypeName::String),
        _ => None,
    }
}

fn canonical(name: &str) -> &str {
    name.trim_end_matches(['%', '&', '!', '#', '$'])
}

fn arity_description(minimum: usize, maximum: usize) -> String {
    if minimum == maximum {
        match minimum {
            0 => "no arguments".into(),
            1 => "one argument".into(),
            count => format!("{count} arguments"),
        }
    } else {
        format!("between {minimum} and {maximum} arguments")
    }
}

fn dotted_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name, _) => Some(name.clone()),
        Expr::Field { base, name, .. } => {
            let mut result = dotted_name(base)?;
            result.push('.');
            result.push_str(name);
            Some(result)
        }
        _ => None,
    }
}

fn narrow(value: i64, type_id: u32) -> i64 {
    match type_id {
        INTEGER => value as i16 as i64,
        LONG => value as i32 as i64,
        _ => value,
    }
}

fn parse_real(value: &str) -> Result<f64, SemanticError> {
    value.parse::<f64>().map_err(|_| SemanticError {
        message: format!("invalid floating constant {value}"),
    })
}

fn real(value: f64, type_id: u32) -> Number {
    let value = if type_id == SINGLE {
        (value as f32) as f64
    } else {
        value
    };
    Number::Real(format!("{value:.17}"))
}

fn as_real(value: &Number) -> Result<f64, SemanticError> {
    match value {
        Number::Integer(value) => Ok(*value as f64),
        Number::Real(value) => parse_real(value),
    }
}

fn constant_binary(
    op: Binary,
    left_type: u32,
    left: Number,
    right_type: u32,
    right: Number,
) -> Result<(u32, Number), SemanticError> {
    let common = common_type(left_type, right_type, op)?;
    let comparison = matches!(
        op,
        Binary::Eq
            | Binary::NotEqual
            | Binary::Less
            | Binary::LessEqual
            | Binary::Greater
            | Binary::GreaterEqual
    );
    if matches!(common, SINGLE | DOUBLE) {
        let left = as_real(&left)?;
        let right = as_real(&right)?;
        let result = match op {
            Binary::Eq => {
                return Ok((BOOLEAN, Number::Integer(if left == right { -1 } else { 0 })))
            }
            Binary::NotEqual => {
                return Ok((BOOLEAN, Number::Integer(if left != right { -1 } else { 0 })))
            }
            Binary::Less => {
                return Ok((BOOLEAN, Number::Integer(if left < right { -1 } else { 0 })))
            }
            Binary::LessEqual => {
                return Ok((BOOLEAN, Number::Integer(if left <= right { -1 } else { 0 })))
            }
            Binary::Greater => {
                return Ok((BOOLEAN, Number::Integer(if left > right { -1 } else { 0 })))
            }
            Binary::GreaterEqual => {
                return Ok((BOOLEAN, Number::Integer(if left >= right { -1 } else { 0 })))
            }
            Binary::Add => left + right,
            Binary::Subtract => left - right,
            Binary::Multiply => left * right,
            Binary::Divide => left / right,
            Binary::Power => left.powf(right),
            _ => {
                return Err(SemanticError {
                    message: "integral operator has a floating constant".into(),
                })
            }
        };
        if !result.is_finite() {
            return Err(SemanticError {
                message: "non-finite constant expression".into(),
            });
        }
        return Ok((common, real(result, common)));
    }
    let Number::Integer(left) = left else {
        unreachable!()
    };
    let Number::Integer(right) = right else {
        unreachable!()
    };
    if comparison {
        let answer = match op {
            Binary::Eq => left == right,
            Binary::NotEqual => left != right,
            Binary::Less => left < right,
            Binary::LessEqual => left <= right,
            Binary::Greater => left > right,
            Binary::GreaterEqual => left >= right,
            _ => unreachable!(),
        };
        return Ok((BOOLEAN, Number::Integer(if answer { -1 } else { 0 })));
    }
    if matches!(op, Binary::Divide | Binary::Power) {
        let result = if op == Binary::Divide {
            left as f64 / right as f64
        } else {
            (left as f64).powf(right as f64)
        };
        return Ok((common, real(result, common)));
    }
    if right == 0 && matches!(op, Binary::Modulo | Binary::IntegerDivide) {
        return Err(SemanticError {
            message: "division by zero in constant expression".into(),
        });
    }
    let value = match op {
        Binary::Add => left.wrapping_add(right),
        Binary::Subtract => left.wrapping_sub(right),
        Binary::Multiply => left.wrapping_mul(right),
        Binary::Modulo => left.wrapping_rem(right),
        Binary::IntegerDivide => left.wrapping_div(right),
        Binary::And => left & right,
        Binary::Or => left | right,
        Binary::Xor => left ^ right,
        Binary::Imp => (!left) | right,
        Binary::Eqv => !(left ^ right),
        _ => unreachable!(),
    };
    Ok((common, Number::Integer(narrow(value, common))))
}

fn common_type(left: u32, right: u32, op: Binary) -> Result<u32, SemanticError> {
    if matches!(
        op,
        Binary::And
            | Binary::Or
            | Binary::Xor
            | Binary::Eqv
            | Binary::Imp
            | Binary::Modulo
            | Binary::IntegerDivide
    ) && (!matches!(left, INTEGER | LONG | BOOLEAN)
        || !matches!(right, INTEGER | LONG | BOOLEAN))
    {
        return Err(SemanticError {
            message: "integral operator has a floating operand".into(),
        });
    }
    if matches!(op, Binary::Divide | Binary::Power) {
        return Ok(if left == DOUBLE || right == DOUBLE {
            DOUBLE
        } else {
            SINGLE
        });
    }
    Ok(if left == DOUBLE || right == DOUBLE {
        DOUBLE
    } else if left == SINGLE || right == SINGLE {
        SINGLE
    } else if left == LONG || right == LONG {
        LONG
    } else {
        INTEGER
    })
}

fn binary_name(op: Binary) -> &'static str {
    match op {
        Binary::Imp => "or",
        Binary::Eqv => "not",
        Binary::Xor => "xor",
        Binary::Or => "or",
        Binary::And => "and",
        Binary::Eq => "eq",
        Binary::NotEqual => "ne",
        Binary::Less => "lt",
        Binary::LessEqual => "le",
        Binary::Greater => "gt",
        Binary::GreaterEqual => "ge",
        Binary::Add => "add",
        Binary::Subtract => "sub",
        Binary::Modulo => "rem",
        Binary::IntegerDivide => "div",
        Binary::Multiply => "mul",
        Binary::Divide => "fdiv",
        Binary::Power => "call",
    }
}

fn contains_not(expression: &Expr) -> bool {
    match expression {
        Expr::Unary { op, operand, .. } => *op == Unary::Not || contains_not(operand),
        Expr::Binary { left, right, .. } => contains_not(left) || contains_not(right),
        Expr::Apply { arguments, .. } => arguments.iter().any(contains_not),
        Expr::Index { base, indices, .. } => {
            contains_not(base) || indices.iter().any(contains_not)
        }
        Expr::Field { base, .. } => contains_not(base),
        Expr::Literal(..) | Expr::Name(..) => false,
    }
}

fn dialect_name(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::QBasic11 => "qbasic11",
        Dialect::QuickBasic45 => "qb45",
        Dialect::Pds71 => "pds71",
        Dialect::VbDos => "vbdos",
    }
}

fn string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            one => out.push(one),
        }
    }
    out.push('"');
}

fn numbers(out: &mut String, values: &[u32]) {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "{value}").unwrap();
    }
}

fn operand_json(out: &mut String, operand: &Operand) {
    match operand {
        Operand::Value(value) => write!(out, "{{\"tag\":\"value\",\"value\":{value}}}").unwrap(),
        Operand::Constant(type_id, Number::Integer(value)) => write!(
            out,
            "{{\"tag\":\"constant\",\"type\":{type_id},\"value\":{value}}}"
        )
        .unwrap(),
        Operand::Constant(type_id, Number::Real(value)) => write!(
            out,
            "{{\"tag\":\"constant\",\"type\":{type_id},\"value\":{value}}}"
        )
        .unwrap(),
        Operand::Place(place) => write!(out, "{{\"place\":{place},\"tag\":\"place\"}}").unwrap(),
        Operand::Element(place, indices) => {
            write!(out, "{{\"indices\":[").unwrap();
            for (index, operand) in indices.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                operand_json(out, operand);
            }
            write!(out, "],\"place\":{place},\"tag\":\"array_element\"}}").unwrap();
        }
        Operand::Projection {
            place,
            indices,
            offset,
            type_id,
        } => {
            out.push_str("{\"indices\":[");
            for (index, operand) in indices.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                operand_json(out, operand);
            }
            write!(
                out,
                "],\"offset\":{offset},\"place\":{place},\"tag\":\"projection\",\"type\":{type_id}}}"
            )
            .unwrap();
        }
        Operand::Indirect {
            base,
            offset,
            type_id,
        } => write!(
            out,
            "{{\"base\":{base},\"offset\":{offset},\"tag\":\"indirect\",\"type\":{type_id}}}"
        )
        .unwrap(),
    }
}

fn type_json(out: &mut String, type_: &Type) {
    write!(out, "{{\"address\":\"{}\",\"bounds\":[", type_.address).unwrap();
    for (index, (lower, upper)) in type_.bounds.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        write!(out, "[{lower},{upper}]").unwrap();
    }
    write!(out, "],\"element\":").unwrap();
    match type_.element {
        Some(element) => write!(out, "{element}").unwrap(),
        None => out.push_str("null"),
    }
    write!(
        out,
        ",\"evaluation\":\"{}\",\"id\":{},\"kind\":\"{}\",\"name\":",
        type_.evaluation, type_.id, type_.kind
    )
    .unwrap();
    string(out, &type_.name);
    write!(out, ",\"rank\":{},\"signed\":", type_.bounds.len()).unwrap();
    match type_.signed {
        Some(value) => out.push_str(if value { "true" } else { "false" }),
        None => out.push_str("null"),
    }
    write!(out, ",\"width\":{}}}", type_.width).unwrap();
}
