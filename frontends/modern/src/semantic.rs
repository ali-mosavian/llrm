use std::collections::BTreeMap;

use crate::error::Diagnostic;
use crate::hir;
use crate::syntax::BinaryOp;
use crate::syntax::Expr;
use crate::syntax::Function;
use crate::syntax::Module;
use crate::syntax::Span;
use crate::syntax::Statement;
use crate::syntax::TypeName;
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

#[derive(Default)]
struct FloatPool {
    symbols: BTreeMap<(TypeName, u64), u32>,
    data: Vec<hir::DataObject>,
}

impl FloatPool {
    fn intern(&mut self, type_name: TypeName, bits: u64) -> u32 {
        if let Some(symbol) = self.symbols.get(&(type_name, bits)) {
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
        self.symbols.insert((type_name, bits), id);
        self.data.push(hir::DataObject { id, name, bytes });
        id
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
}

#[derive(Clone, Debug)]
struct Binding {
    type_name: TypeName,
    mutable: bool,
    storage: Storage,
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

    let callables = signatures
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
        })
        .collect();
    let mut functions = Vec::new();
    let mut float_pool = FloatPool::default();
    for function in &module.functions {
        let signature = signatures.get(&function.name).expect("collected function");
        functions.push(
            FunctionCompiler::new(function, signature, &signatures, &mut float_pool)?
                .compile(function)?,
        );
    }
    let program = hir::Program {
        module_name: module_name.into(),
        types: vec![
            hir::Type {
                id: VOID,
                name: "void",
                kind: "void",
                width: 0,
                signed: None,
                evaluation: "none",
            },
            hir::Type {
                id: BOOL,
                name: "bool",
                kind: "boolean",
                width: 1,
                signed: None,
                evaluation: "none",
            },
            hir::Type {
                id: CHAR,
                name: "char",
                kind: "integer",
                width: 1,
                signed: Some(false),
                evaluation: "none",
            },
            hir::Type {
                id: I8,
                name: "i8",
                kind: "integer",
                width: 1,
                signed: Some(true),
                evaluation: "none",
            },
            integer_type(U8, "u8", 1, false),
            integer_type(I16, "i16", 2, true),
            integer_type(U16, "u16", 2, false),
            integer_type(I32, "i32", 4, true),
            integer_type(U32, "u32", 4, false),
            hir::Type {
                id: F32,
                name: "f32",
                kind: "float",
                width: 4,
                signed: None,
                evaluation: "binary32",
            },
            hir::Type {
                id: F64,
                name: "f64",
                kind: "float",
                width: 8,
                signed: None,
                evaluation: "binary64",
            },
        ],
        functions,
        callables,
        data: float_pool.data,
    };
    Ok(program.json())
}

fn integer_type(id: u32, name: &'static str, width: u32, signed: bool) -> hir::Type {
    hir::Type {
        id,
        name,
        kind: "integer",
        width,
        signed: Some(signed),
        evaluation: "none",
    }
}

struct FunctionCompiler<'a> {
    signature: &'a Signature,
    signatures: &'a BTreeMap<String, Signature>,
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
    float_pool: &'a mut FloatPool,
    constant_places: BTreeMap<u32, u32>,
}

impl<'a> FunctionCompiler<'a> {
    fn new(
        function: &Function,
        signature: &'a Signature,
        signatures: &'a BTreeMap<String, Signature>,
        float_pool: &'a mut FloatPool,
    ) -> Result<Self, Diagnostic> {
        let mut compiler = Self {
            signature,
            signatures,
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
            float_pool,
            constant_places: BTreeMap::new(),
        };
        for parameter in &function.parameters {
            let value = compiler.value(parameter.type_name);
            compiler.parameters.push(value);
            compiler.scopes[0].insert(
                parameter.name.clone(),
                Binding {
                    type_name: parameter.type_name,
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
                let value = self.expression(value, *annotation)?;
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
                        type_name: annotation.unwrap_or(binding_type),
                        mutable: *mutable,
                        storage: Storage::Place(place),
                    },
                );
            }
            Statement::Assign { name, value, span } => {
                let binding = self.binding(name, *span)?.clone();
                if !binding.mutable {
                    return Err(Diagnostic::new(
                        *span,
                        format!("binding {name:?} is immutable"),
                    ));
                }
                let Storage::Place(place) = binding.storage else {
                    return Err(Diagnostic::new(*span, "parameters are immutable"));
                };
                let value = self.expression(value, Some(binding.type_name))?;
                self.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(place), required(value, *span)?],
                    None,
                );
            }
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

    fn scoped(&mut self, statements: &[Statement]) -> Result<(), Diagnostic> {
        self.scopes.push(BTreeMap::new());
        let result = self.statements(statements);
        self.scopes.pop();
        result
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
                if expected.is_some_and(|one| one != binding.type_name) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        binding.type_name,
                    ));
                }
                let operand = match binding.storage {
                    Storage::Parameter(value) => hir::Operand::Value(value),
                    Storage::Place(place) => {
                        let value = self.value(binding.type_name);
                        self.emit("load", vec![value], vec![hir::Operand::Place(place)], None);
                        hir::Operand::Value(value)
                    }
                };
                Ok(TypedOperand {
                    operand: Some(operand),
                    type_name: binding.type_name,
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
        let symbol = self.float_pool.intern(type_name, bits);
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

    fn binary(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
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
        if comparison {
            let hint = self.expression_type_hint(right);
            if (matches!(left, Expr::Integer(..))
                && hint.is_some_and(|one| is_integer(one) || one == TypeName::Char))
                || (matches!(left, Expr::Float(..)) && hint.is_some_and(is_float))
            {
                left_expected = hint;
            }
        }
        let left = self.expression(left, left_expected)?;
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

    fn expression_type_hint(&self, expression: &Expr) -> Option<TypeName> {
        match expression {
            Expr::Float(..) => Some(TypeName::F64),
            Expr::Character(..) => Some(TypeName::Char),
            Expr::Boolean(..) => Some(TypeName::Bool),
            Expr::Name(name, _) => self
                .binding(name, expression.span())
                .ok()
                .map(|one| one.type_name),
            Expr::Call { name, .. } => self.signatures.get(name).map(|one| one.result),
            Expr::Unary { operand, .. } => self.expression_type_hint(operand),
            Expr::Integer(..) | Expr::Binary { .. } => None,
        }
    }

    fn call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
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
            order: (0..arguments.len() as u32).collect(),
            callee: signature.id,
        });
        Ok(TypedOperand {
            operand: results.first().copied().map(hir::Operand::Value),
            type_name: signature.result,
        })
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
        let id = self.next_place;
        self.next_place += 1;
        let extent = width(type_name);
        self.next_frame_offset -= extent as i32;
        self.places.push(hir::Place {
            id,
            name: name.into(),
            type_id: type_id(type_name),
            mutable,
            offset: self.next_frame_offset,
            extent,
            storage: "local",
            symbol: 0,
        });
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

fn is_integer(type_name: TypeName) -> bool {
    matches!(
        type_name,
        TypeName::I8 | TypeName::U8 | TypeName::I16 | TypeName::U16 | TypeName::I32 | TypeName::U32
    )
}

fn is_signed(type_name: TypeName) -> bool {
    matches!(type_name, TypeName::I8 | TypeName::I16 | TypeName::I32)
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
    is_integer(type_name) || is_float(type_name)
}

fn is_ordered(type_name: TypeName) -> bool {
    is_numeric(type_name) || type_name == TypeName::Char
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
    }
}

fn width(type_name: TypeName) -> u32 {
    match type_name {
        TypeName::Void => 0,
        TypeName::Bool | TypeName::Char | TypeName::I8 | TypeName::U8 => 1,
        TypeName::I16 | TypeName::U16 => 2,
        TypeName::I32 | TypeName::U32 | TypeName::F32 => 4,
        TypeName::F64 => 8,
    }
}

fn type_name_text(type_name: TypeName) -> &'static str {
    match type_name {
        TypeName::Void => "void",
        TypeName::Bool => "bool",
        TypeName::Char => "char",
        TypeName::I8 => "i8",
        TypeName::U8 => "u8",
        TypeName::I16 => "i16",
        TypeName::U16 => "u16",
        TypeName::I32 => "i32",
        TypeName::U32 => "u32",
        TypeName::F32 => "f32",
        TypeName::F64 => "f64",
    }
}

fn type_mismatch(span: Span, expected: TypeName, found: TypeName) -> Diagnostic {
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
}
