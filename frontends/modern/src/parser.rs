use std::collections::BTreeMap;

use crate::error::Diagnostic;
use crate::lexer::lex;
use crate::lexer::Token;
use crate::lexer::TokenKind;
use crate::syntax::AssignTarget;
use crate::syntax::BinaryOp;
use crate::syntax::Expr;
use crate::syntax::FStringPart;
use crate::syntax::FixedStorage;
use crate::syntax::FixedType;
use crate::syntax::Function;
use crate::syntax::IterationMode;
use crate::syntax::Module;
use crate::syntax::Parameter;
use crate::syntax::Span;
use crate::syntax::Statement;
use crate::syntax::Struct;
use crate::syntax::StructField;
use crate::syntax::StructLiteralFields;
use crate::syntax::TypeAnnotation;
use crate::syntax::TypeName;
use crate::syntax::TypeSpec;
use crate::syntax::UnaryOp;

pub fn parse(tokens: Vec<Token>) -> Result<Module, Diagnostic> {
    Parser {
        tokens,
        at: 0,
        fixed_types: BTreeMap::new(),
    }
    .module()
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    fixed_types: BTreeMap<String, TypeName>,
}

impl Parser {
    fn module(&mut self) -> Result<Module, Diagnostic> {
        let mut fixed_types = Vec::new();
        let mut structs = Vec::new();
        let mut functions = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Eof) {
            if self
                .take(|kind| matches!(kind, TokenKind::Newline))
                .is_some()
            {
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Type) {
                fixed_types.push(self.fixed_type()?);
            } else if matches!(self.peek().kind, TokenKind::Struct) {
                structs.push(self.structure()?);
            } else {
                functions.push(self.function()?);
            }
        }
        if functions.is_empty() {
            return Err(Diagnostic::new(
                self.peek().span,
                "module contains no functions",
            ));
        }
        Ok(Module {
            fixed_types,
            structs,
            functions,
        })
    }

    fn fixed_type(&mut self) -> Result<FixedType, Diagnostic> {
        let span = self.bump().span;
        let (name, name_span) = self.identifier("expected fixed-point type name")?;
        if self.fixed_types.contains_key(&name) {
            return Err(Diagnostic::new(
                name_span,
                format!("type {name:?} is declared more than once"),
            ));
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Equal),
            "expected '=' after type name",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Fixed),
            "expected 'fixed' numeric type",
        )?;
        let storage = match self.bump().clone() {
            Token {
                kind: TokenKind::I16,
                ..
            } => FixedStorage::I16,
            Token {
                kind: TokenKind::I32,
                ..
            } => FixedStorage::I32,
            token => {
                return Err(Diagnostic::new(
                    token.span,
                    "fixed-point storage must be i16 or i32",
                ))
            }
        };
        self.expect(
            |kind| matches!(kind, TokenKind::Comma),
            "expected ',' before fixed-point fraction",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Fraction),
            "expected 'fraction'",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Equal),
            "expected '=' after 'fraction'",
        )?;
        let fraction_token = self.bump().clone();
        let TokenKind::Integer(fraction) = fraction_token.kind else {
            return Err(Diagnostic::new(
                fraction_token.span,
                "fraction must be an integer literal",
            ));
        };
        let storage_bits = match storage {
            FixedStorage::I16 => 16,
            FixedStorage::I32 => 32,
        };
        let fraction = u8::try_from(fraction)
            .ok()
            .filter(|one| *one > 0 && u32::from(*one) < storage_bits)
            .ok_or_else(|| {
                Diagnostic::new(
                    fraction_token.span,
                    format!("fraction must be between 1 and {}", storage_bits - 1),
                )
            })?;
        self.line_end()?;
        let declaration = u16::try_from(self.fixed_types.len())
            .map_err(|_| Diagnostic::new(span, "too many fixed-point types"))?;
        let type_name = TypeName::Fixed {
            storage,
            fraction,
            declaration,
        };
        self.fixed_types.insert(name.clone(), type_name);
        Ok(FixedType {
            name,
            type_name,
            span,
        })
    }

    fn structure(&mut self) -> Result<Struct, Diagnostic> {
        let span = self.bump().span;
        let (name, _) = self.identifier("expected struct name")?;
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' after struct name",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before struct fields",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected indented struct fields",
        )?;
        let mut fields = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            let (field_name, field_span) = self.identifier("expected field name")?;
            self.expect(
                |kind| matches!(kind, TokenKind::Colon),
                "expected ':' after field name",
            )?;
            let type_spec = self.type_spec()?;
            if type_spec == TypeSpec::Primitive(TypeName::Void) {
                return Err(Diagnostic::new(field_span, "a struct field cannot be void"));
            }
            self.line_end()?;
            fields.push(StructField {
                name: field_name,
                type_spec,
                span: field_span,
            });
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated struct",
        )?;
        if fields.is_empty() {
            return Err(Diagnostic::new(span, "struct must have at least one field"));
        }
        Ok(Struct { name, fields, span })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let start = self
            .expect(|kind| matches!(kind, TokenKind::Fn), "expected 'fn'")?
            .span;
        let (name, _) = self.identifier("expected function name")?;
        self.expect(
            |kind| matches!(kind, TokenKind::LeftParen),
            "expected '(' after function name",
        )?;
        let mut parameters = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RightParen) {
            loop {
                let (parameter_name, span) = self.identifier("expected parameter name")?;
                self.expect(
                    |kind| matches!(kind, TokenKind::Colon),
                    "expected ':' after parameter name",
                )?;
                let type_name = self.type_name()?;
                if type_name == TypeName::Void {
                    return Err(Diagnostic::new(span, "a parameter cannot have type void"));
                }
                parameters.push(Parameter {
                    name: parameter_name,
                    type_name,
                    span,
                });
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                    break;
                }
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RightParen),
            "expected ')' after parameters",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Arrow),
            "expected '->' and a result type",
        )?;
        let result = self.type_name()?;
        let body = self.suite()?;
        Ok(Function {
            name,
            parameters,
            result,
            body,
            span: start,
        })
    }

    fn suite(&mut self) -> Result<Vec<Statement>, Diagnostic> {
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' before block",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before block",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected an indented block",
        )?;
        let mut statements = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            statements.push(self.statement()?);
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated block",
        )?;
        if statements.is_empty() {
            return Err(Diagnostic::new(self.peek().span, "block cannot be empty"));
        }
        Ok(statements)
    }

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        match self.peek().kind {
            TokenKind::Let | TokenKind::Var => self.binding(),
            TokenKind::Return => self.return_statement(),
            TokenKind::If => self.if_statement(),
            TokenKind::While => self.while_statement(),
            TokenKind::For => self.for_statement(),
            TokenKind::Break => {
                let span = self.bump().span;
                self.line_end()?;
                Ok(Statement::Break(span))
            }
            TokenKind::Continue => {
                let span = self.bump().span;
                self.line_end()?;
                Ok(Statement::Continue(span))
            }
            _ => {
                let expression = self.expression(0)?;
                let operation = match self.peek().kind {
                    TokenKind::Equal => Some(None),
                    TokenKind::PlusEqual => Some(Some(BinaryOp::Add)),
                    TokenKind::MinusEqual => Some(Some(BinaryOp::Subtract)),
                    TokenKind::StarEqual => Some(Some(BinaryOp::Multiply)),
                    TokenKind::SlashEqual => Some(Some(BinaryOp::Divide)),
                    TokenKind::PercentEqual => Some(Some(BinaryOp::Remainder)),
                    _ => None,
                };
                if let Some(operation) = operation {
                    self.bump();
                    let span = expression.span();
                    let target = match expression {
                        Expr::Name(name, _) => AssignTarget::Name(name),
                        Expr::Index { base, index, .. } => {
                            let Expr::Name(base, _) = *base else {
                                return Err(Diagnostic::new(
                                    span,
                                    "assignment target must be a named place",
                                ));
                            };
                            AssignTarget::Index {
                                base,
                                index: *index,
                            }
                        }
                        Expr::Member { base, field, .. } => {
                            AssignTarget::Member { base: *base, field }
                        }
                        _ => return Err(Diagnostic::new(span, "expression is not assignable")),
                    };
                    let value = self.expression(0)?;
                    self.line_end()?;
                    Ok(Statement::Assign {
                        target,
                        operation,
                        value,
                        span,
                    })
                } else {
                    self.line_end()?;
                    Ok(Statement::Expr(expression))
                }
            }
        }
    }

    fn binding(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.bump().clone();
        let mutable = matches!(token.kind, TokenKind::Var);
        let (name, _) = self.identifier("expected binding name")?;
        let annotation = if self.take(|kind| matches!(kind, TokenKind::Colon)).is_some() {
            let annotation = self.type_annotation()?;
            if annotation == TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Void)) {
                return Err(Diagnostic::new(
                    token.span,
                    "a binding cannot have type void",
                ));
            }
            Some(annotation)
        } else {
            None
        };
        self.expect(
            |kind| matches!(kind, TokenKind::Equal),
            "a binding requires an initializer",
        )?;
        let value = self.expression(0)?;
        self.line_end()?;
        Ok(Statement::Bind {
            mutable,
            name,
            annotation,
            value,
            span: token.span,
        })
    }

    fn return_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let value = if matches!(self.peek().kind, TokenKind::Newline) {
            None
        } else {
            Some(self.expression(0)?)
        };
        self.line_end()?;
        Ok(Statement::Return { value, span })
    }

    fn if_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let condition = self.expression(0)?;
        let then_branch = self.suite()?;
        let else_branch = if self.take(|kind| matches!(kind, TokenKind::Else)).is_some() {
            self.suite()?
        } else {
            Vec::new()
        };
        Ok(Statement::If {
            condition,
            then_branch,
            else_branch,
            span,
        })
    }

    fn while_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let condition = self.expression(0)?;
        let body = self.suite()?;
        Ok(Statement::While {
            condition,
            body,
            span,
        })
    }

    fn for_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let (name, _) = self.identifier("expected loop binding")?;
        self.expect(
            |kind| matches!(kind, TokenKind::In),
            "expected 'in' after loop binding",
        )?;
        let mode = if self
            .take(|kind| matches!(kind, TokenKind::Ampersand))
            .is_some()
        {
            if self.take(|kind| matches!(kind, TokenKind::Mut)).is_some() {
                IterationMode::Mutable
            } else {
                IterationMode::Shared
            }
        } else {
            IterationMode::Value
        };
        let iterable = self.expression(0)?;
        if mode == IterationMode::Value
            && self.take(|kind| matches!(kind, TokenKind::Range)).is_some()
        {
            let end = self.expression(0)?;
            let body = self.suite()?;
            return Ok(Statement::ForRange {
                name,
                start: iterable,
                end,
                body,
                span,
            });
        }
        let body = self.suite()?;
        Ok(Statement::For {
            mode,
            name,
            iterable,
            body,
            span,
        })
    }

    fn expression(&mut self, minimum_binding: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.prefix()?;
        loop {
            if matches!(self.peek().kind, TokenKind::LeftParen) {
                if 30 < minimum_binding {
                    break;
                }
                left = self.call(left)?;
                continue;
            }
            if matches!(self.peek().kind, TokenKind::LeftBracket) {
                if 30 < minimum_binding {
                    break;
                }
                left = self.index(left)?;
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Dot) {
                if 30 < minimum_binding {
                    break;
                }
                left = self.member(left)?;
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Is) {
                if 5 < minimum_binding {
                    break;
                }
                self.bump();
                let operation = if self.take(|kind| matches!(kind, TokenKind::Not)).is_some() {
                    BinaryOp::IsNot
                } else {
                    BinaryOp::Is
                };
                let right = self.expression(6)?;
                let left_span = left.span();
                let right_span = right.span();
                left = Expr::Binary {
                    op: operation,
                    left: Box::new(left),
                    right: Box::new(right),
                    span: Span::new(left_span.line, left_span.column, right_span.end_column),
                };
                continue;
            }
            let Some((left_binding, right_binding, operation)) = infix(&self.peek().kind) else {
                break;
            };
            if left_binding < minimum_binding {
                break;
            }
            self.bump();
            let right = self.expression(right_binding)?;
            let left_span = left.span();
            let right_span = right.span();
            left = Expr::Binary {
                op: operation,
                left: Box::new(left),
                right: Box::new(right),
                span: Span::new(left_span.line, left_span.column, right_span.end_column),
            };
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer(value, token.span)),
            TokenKind::Float(value) => Ok(Expr::Float(value, token.span)),
            TokenKind::Character(value) => Ok(Expr::Character(value, token.span)),
            TokenKind::String(value) => Ok(Expr::String(value, token.span)),
            TokenKind::FString(value) => self.fstring(value, token.span),
            TokenKind::True => Ok(Expr::Boolean(true, token.span)),
            TokenKind::False => Ok(Expr::Boolean(false, token.span)),
            TokenKind::Identifier(name) => {
                if matches!(self.peek().kind, TokenKind::LeftBrace) {
                    self.bump();
                    self.struct_literal(Some(name), token.span)
                } else {
                    Ok(Expr::Name(name, token.span))
                }
            }
            TokenKind::LeftBrace => self.struct_literal(None, token.span),
            TokenKind::LeftBracket => {
                let mut values = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RightBracket) {
                    loop {
                        values.push(self.expression(0)?);
                        if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                            break;
                        }
                        if matches!(self.peek().kind, TokenKind::RightBracket) {
                            break;
                        }
                    }
                }
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RightBracket),
                    "expected ']' after array literal",
                )?;
                Ok(Expr::Array(
                    values,
                    Span::new(token.span.line, token.span.column, close.span.end_column),
                ))
            }
            TokenKind::Minus | TokenKind::Not => {
                let operation = if matches!(token.kind, TokenKind::Minus) {
                    UnaryOp::Negative
                } else {
                    UnaryOp::Not
                };
                let operand = self.expression(25)?;
                let end = operand.span().end_column;
                Ok(Expr::Unary {
                    op: operation,
                    operand: Box::new(operand),
                    span: Span::new(token.span.line, token.span.column, end),
                })
            }
            TokenKind::LeftParen => {
                let expression = self.expression(0)?;
                self.expect(
                    |kind| matches!(kind, TokenKind::RightParen),
                    "expected ')' after expression",
                )?;
                Ok(expression)
            }
            _ => Err(Diagnostic::new(token.span, "expected expression")),
        }
    }

    fn call(&mut self, callee: Expr) -> Result<Expr, Diagnostic> {
        let (name, start) = match callee {
            Expr::Name(name, span) => (name, span),
            other => {
                return Err(Diagnostic::new(
                    other.span(),
                    "only named functions can be called",
                ))
            }
        };
        self.bump();
        let mut arguments = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RightParen) {
            loop {
                arguments.push(self.expression(0)?);
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                    break;
                }
                if matches!(self.peek().kind, TokenKind::RightParen) {
                    break;
                }
            }
        }
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RightParen),
            "expected ')' after arguments",
        )?;
        Ok(Expr::Call {
            name,
            arguments,
            span: Span::new(start.line, start.column, close.span.end_column),
        })
    }

    fn index(&mut self, base: Expr) -> Result<Expr, Diagnostic> {
        let start = base.span();
        self.bump();
        let index = self.expression(0)?;
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RightBracket),
            "expected ']' after index",
        )?;
        Ok(Expr::Index {
            base: Box::new(base),
            index: Box::new(index),
            span: Span::new(start.line, start.column, close.span.end_column),
        })
    }

    fn member(&mut self, base: Expr) -> Result<Expr, Diagnostic> {
        let start = base.span();
        self.bump();
        let (field, field_span) = self.identifier("expected field name after '.'")?;
        Ok(Expr::Member {
            base: Box::new(base),
            field,
            span: Span::new(start.line, start.column, field_span.end_column),
        })
    }

    fn struct_literal(&mut self, name: Option<String>, start: Span) -> Result<Expr, Diagnostic> {
        let named = matches!(self.peek().kind, TokenKind::Identifier(_))
            && self
                .tokens
                .get(self.at + 1)
                .is_some_and(|token| matches!(token.kind, TokenKind::Colon));
        let fields = if named {
            let mut fields = Vec::new();
            loop {
                let (field, span) = self.identifier("expected field name")?;
                self.expect(
                    |kind| matches!(kind, TokenKind::Colon),
                    "expected ':' after field name",
                )?;
                let value = self.expression(0)?;
                fields.push((field, value, span));
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none()
                    || matches!(self.peek().kind, TokenKind::RightBrace)
                {
                    break;
                }
                if !matches!(self.peek().kind, TokenKind::Identifier(_))
                    || !self
                        .tokens
                        .get(self.at + 1)
                        .is_some_and(|token| matches!(token.kind, TokenKind::Colon))
                {
                    return Err(Diagnostic::new(
                        self.peek().span,
                        "cannot mix named and positional struct fields",
                    ));
                }
            }
            StructLiteralFields::Named(fields)
        } else {
            let mut fields = Vec::new();
            if !matches!(self.peek().kind, TokenKind::RightBrace) {
                loop {
                    fields.push(self.expression(0)?);
                    if matches!(self.peek().kind, TokenKind::Colon) {
                        return Err(Diagnostic::new(
                            self.peek().span,
                            "cannot mix positional and named struct fields",
                        ));
                    }
                    if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none()
                        || matches!(self.peek().kind, TokenKind::RightBrace)
                    {
                        break;
                    }
                }
            }
            StructLiteralFields::Positional(fields)
        };
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RightBrace),
            "expected '}' after struct literal",
        )?;
        Ok(Expr::StructLiteral {
            name,
            fields,
            span: Span::new(start.line, start.column, close.span.end_column),
        })
    }

    fn fstring(&self, value: Vec<u8>, span: Span) -> Result<Expr, Diagnostic> {
        let mut parts = Vec::new();
        let mut text = Vec::new();
        let mut at = 0;
        while at < value.len() {
            match value[at] {
                b'{' if value.get(at + 1) == Some(&b'{') => {
                    text.push(b'{');
                    at += 2;
                }
                b'}' if value.get(at + 1) == Some(&b'}') => {
                    text.push(b'}');
                    at += 2;
                }
                b'{' => {
                    if !text.is_empty() {
                        parts.push(FStringPart::Text(std::mem::take(&mut text)));
                    }
                    let Some(close) = value[at + 1..].iter().position(|one| *one == b'}') else {
                        return Err(Diagnostic::new(
                            span,
                            "f-string has an unclosed interpolation",
                        ));
                    };
                    let close = at + 1 + close;
                    let source = std::str::from_utf8(&value[at + 1..close])
                        .map_err(|_| Diagnostic::new(span, "f-string interpolation must be ASCII"))?
                        .trim();
                    if source.is_empty() {
                        return Err(Diagnostic::new(
                            span,
                            "f-string interpolation cannot be empty",
                        ));
                    }
                    parts.push(FStringPart::Value(parse_inline_expression(source, span)?));
                    at = close + 1;
                }
                b'}' => return Err(Diagnostic::new(span, "f-string has an unmatched '}'")),
                byte => {
                    text.push(byte);
                    at += 1;
                }
            }
        }
        if !text.is_empty() {
            parts.push(FStringPart::Text(text));
        }
        Ok(Expr::FString { parts, span })
    }

    fn type_annotation(&mut self) -> Result<TypeAnnotation, Diagnostic> {
        if self
            .take(|kind| matches!(kind, TokenKind::LeftBracket))
            .is_some()
        {
            let element = self.type_spec()?;
            if element == TypeSpec::Primitive(TypeName::Void) {
                return Err(Diagnostic::new(
                    self.peek().span,
                    "an array element cannot be void",
                ));
            }
            self.expect(
                |kind| matches!(kind, TokenKind::Semicolon),
                "expected ';' and an array length",
            )?;
            let length_token = self.bump().clone();
            let TokenKind::Integer(length_value) = length_token.kind else {
                return Err(Diagnostic::new(
                    length_token.span,
                    "array length must be an integer literal",
                ));
            };
            let length = u32::try_from(length_value)
                .ok()
                .filter(|one| *one > 0)
                .ok_or_else(|| {
                    Diagnostic::new(
                        length_token.span,
                        "array length must be positive and fit u32",
                    )
                })?;
            self.expect(
                |kind| matches!(kind, TokenKind::RightBracket),
                "expected ']' after array type",
            )?;
            Ok(TypeAnnotation::Array { element, length })
        } else {
            self.type_spec().map(TypeAnnotation::Value)
        }
    }

    fn type_name(&mut self) -> Result<TypeName, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Char => Ok(TypeName::Char),
            TokenKind::I8 => Ok(TypeName::I8),
            TokenKind::U8 => Ok(TypeName::U8),
            TokenKind::I16 => Ok(TypeName::I16),
            TokenKind::U16 => Ok(TypeName::U16),
            TokenKind::I32 => Ok(TypeName::I32),
            TokenKind::U32 => Ok(TypeName::U32),
            TokenKind::F32 => Ok(TypeName::F32),
            TokenKind::F64 => Ok(TypeName::F64),
            TokenKind::StringType => Ok(TypeName::String),
            TokenKind::Bool => Ok(TypeName::Bool),
            TokenKind::Void => Ok(TypeName::Void),
            TokenKind::Identifier(name) => self.fixed_types.get(&name).copied().ok_or_else(|| {
                Diagnostic::new(token.span, format!("unknown scalar type {name:?}"))
            }),
            _ => Err(Diagnostic::new(token.span, "expected a type name")),
        }
    }

    fn type_spec(&mut self) -> Result<TypeSpec, Diagnostic> {
        if let TokenKind::Identifier(name) = &self.peek().kind {
            let name = name.clone();
            self.bump();
            Ok(self
                .fixed_types
                .get(&name)
                .copied()
                .map(TypeSpec::Primitive)
                .unwrap_or(TypeSpec::Named(name)))
        } else {
            self.type_name().map(TypeSpec::Primitive)
        }
    }

    fn line_end(&mut self) -> Result<(), Diagnostic> {
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected end of line",
        )?;
        Ok(())
    }

    fn identifier(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.bump();
        if let TokenKind::Identifier(name) = &token.kind {
            Ok((name.clone(), token.span))
        } else {
            Err(Diagnostic::new(token.span, message))
        }
    }

    fn expect(
        &mut self,
        predicate: impl FnOnce(&TokenKind) -> bool,
        message: &str,
    ) -> Result<&Token, Diagnostic> {
        if predicate(&self.peek().kind) {
            Ok(self.bump())
        } else {
            Err(Diagnostic::new(self.peek().span, message))
        }
    }

    fn take(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> Option<&Token> {
        predicate(&self.peek().kind).then(|| self.bump())
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.at]
    }

    fn bump(&mut self) -> &Token {
        let index = self.at;
        if !matches!(self.tokens[index].kind, TokenKind::Eof) {
            self.at += 1;
        }
        &self.tokens[index]
    }
}

fn parse_inline_expression(source: &str, outer: Span) -> Result<Expr, Diagnostic> {
    let mut parser = Parser {
        tokens: lex(source).map_err(|error| Diagnostic::new(outer, error.message))?,
        at: 0,
        fixed_types: BTreeMap::new(),
    };
    let expression = parser
        .expression(0)
        .map_err(|error| Diagnostic::new(outer, error.message))?;
    if !matches!(parser.peek().kind, TokenKind::Newline | TokenKind::Eof) {
        return Err(Diagnostic::new(outer, "invalid f-string interpolation"));
    }
    Ok(expression)
}

fn infix(kind: &TokenKind) -> Option<(u8, u8, BinaryOp)> {
    Some(match kind {
        TokenKind::EqualEqual => (5, 6, BinaryOp::Equal),
        TokenKind::NotEqual => (5, 6, BinaryOp::NotEqual),
        TokenKind::Less => (5, 6, BinaryOp::Less),
        TokenKind::LessEqual => (5, 6, BinaryOp::LessEqual),
        TokenKind::Greater => (5, 6, BinaryOp::Greater),
        TokenKind::GreaterEqual => (5, 6, BinaryOp::GreaterEqual),
        TokenKind::Plus => (10, 11, BinaryOp::Add),
        TokenKind::Minus => (10, 11, BinaryOp::Subtract),
        TokenKind::Star => (20, 21, BinaryOp::Multiply),
        TokenKind::Slash => (20, 21, BinaryOp::Divide),
        TokenKind::Percent => (20, 21, BinaryOp::Remainder),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::lexer::lex;

    use super::*;

    #[test]
    fn parses_precedence_and_nested_blocks() {
        let module = parse(
            lex("fn choose(value: i16) -> i16:\n\
                 \x20\x20\x20\x20if value < 0:\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20return -value\n\
                 \x20\x20\x20\x20else:\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20return value + 2 * 3\n")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].body.len(), 1);
        let Statement::If { else_branch, .. } = &module.functions[0].body[0] else {
            panic!("expected if")
        };
        let Statement::Return {
            value: Some(Expr::Binary { op, right, .. }),
            ..
        } = &else_branch[0]
        else {
            panic!("expected return expression")
        };
        assert_eq!(*op, BinaryOp::Add);
        assert!(matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn parses_fixed_array_assignment_and_f_string_interpolation() {
        let module = parse(
            lex("fn show() -> void:\n\
                 \x20\x20\x20\x20var values: [i32; 2] = [10, 20]\n\
                 \x20\x20\x20\x20values[1] = 30\n\
                 \x20\x20\x20\x20print(f\"value={values[1]}\")\n")
            .unwrap(),
        )
        .unwrap();
        let Statement::Bind {
            annotation:
                Some(TypeAnnotation::Array {
                    element: TypeSpec::Primitive(TypeName::I32),
                    length: 2,
                }),
            ..
        } = &module.functions[0].body[0]
        else {
            panic!("expected fixed-array binding")
        };
        assert!(matches!(
            &module.functions[0].body[1],
            Statement::Assign {
                target: AssignTarget::Index { .. },
                ..
            }
        ));
        let Statement::Expr(Expr::Call { arguments, .. }) = &module.functions[0].body[2] else {
            panic!("expected print call")
        };
        assert!(matches!(arguments[0], Expr::FString { .. }));
    }

    #[test]
    fn parses_a_half_open_range_loop() {
        let module = parse(
            lex("fn count(step_count: i32) -> i32:\n\
                 \x20\x20\x20\x20var total: i32 = 0\n\
                 \x20\x20\x20\x20for step_no in 0..step_count - 1:\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20total = total + step_no\n\
                 \x20\x20\x20\x20return total\n")
            .unwrap(),
        )
        .unwrap();
        let Statement::ForRange {
            name, start, end, ..
        } = &module.functions[0].body[1]
        else {
            panic!("expected range loop")
        };
        assert_eq!(name, "step_no");
        assert!(matches!(start, Expr::Integer(0, _)));
        assert!(matches!(
            end,
            Expr::Binary {
                op: BinaryOp::Subtract,
                ..
            }
        ));
    }

    #[test]
    fn parses_named_fixed_point_types() {
        let module = parse(
            lex("type fixed8 = fixed i16, fraction=8\n\
                 fn scale(value: fixed8) -> fixed8:\n\
                 \x20\x20\x20\x20return value * 1.5\n")
            .unwrap(),
        )
        .unwrap();
        assert_eq!(module.fixed_types.len(), 1);
        assert_eq!(module.fixed_types[0].name, "fixed8");
        assert!(matches!(
            module.fixed_types[0].type_name,
            TypeName::Fixed {
                storage: FixedStorage::I16,
                fraction: 8,
                declaration: 0,
            }
        ));
        assert_eq!(
            module.functions[0].parameters[0].type_name,
            module.fixed_types[0].type_name
        );
        assert_eq!(module.functions[0].result, module.fixed_types[0].type_name);
    }
}
