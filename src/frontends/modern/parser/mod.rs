use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::consts;
use super::error::Diagnostic;
use super::lexer::Token;
use super::lexer::TokenKind;
use super::lexer::lex;
use super::syntax::Abi;
use super::syntax::AssignTarget;
use super::syntax::BinaryOp;
use super::syntax::Clause;
use super::syntax::Const;
use super::syntax::Enum;
use super::syntax::Expr;
use super::syntax::Extern;
use super::syntax::FixedStorage;
use super::syntax::FixedType;
use super::syntax::Function;
use super::syntax::GenericParameter;
use super::syntax::Import;
use super::syntax::IterationMode;
use super::syntax::LambdaParameter;
use super::syntax::MAX_RANK;
use super::syntax::MatchArm;
use super::syntax::Module;
use super::syntax::Parameter;
use super::syntax::ParameterType;
use super::syntax::Pattern;
use super::syntax::Protocol;
use super::syntax::Span;
use super::syntax::Statement;
use super::syntax::Struct;
use super::syntax::StructField;
use super::syntax::TUPLE;
use super::syntax::FUNCTION;
use super::syntax::TypeAnnotation;
use super::syntax::TypeName;
use super::syntax::TypeSpec;
use super::syntax::UnaryOp;
use super::syntax::Variant;
use super::syntax::{FStringPart, Format};

mod enums;
mod patterns;

pub fn parse(tokens: Vec<Token>) -> Result<Module, Diagnostic> {
    Parser {
        tokens,
        at: 0,
        fixed_types: BTreeMap::new(),
        consts: BTreeMap::new(),
    }
    .module()
    .map(|mut module| {
        super::desugar::local_declarations(&mut module);
        module
    })
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    fixed_types: BTreeMap<String, TypeName>,
    /// Each constant declared so far, as the literal it stands for.
    consts: BTreeMap<String, Expr>,
}

impl Parser {
    fn module(&mut self) -> Result<Module, Diagnostic> {
        let mut fixed_types = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut protocols = Vec::new();
        let mut functions = Vec::new();
        let mut imports = Vec::new();
        let mut public = BTreeSet::new();
        let mut consts = Vec::new();
        let mut externs = Vec::new();
        let mut exports = BTreeMap::new();
        while !matches!(self.peek().kind, TokenKind::Eof) {
            if self
                .take(|kind| matches!(kind, TokenKind::Newline))
                .is_some()
            {
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Import) {
                imports.push(self.import()?);
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Extern) {
                externs.extend(self.extern_block()?);
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Export) {
                let (abi, defined) = self.foreign_block(|parser| parser.function())?;
                exports.extend(defined.iter().map(|one| (one.name.clone(), abi)));
                functions.extend(defined);
                continue;
            }
            let attributes = self.attributes()?;
            // Only a struct takes one, `@repr`.
            let struct_next = matches!(self.peek().kind, TokenKind::Struct)
                || (matches!(self.peek().kind, TokenKind::Pub)
                    && matches!(
                        self.tokens.get(self.at + 1).map(|one| &one.kind),
                        Some(TokenKind::Struct)
                    ));
            if let Some(attribute) = attributes
                .iter()
                .find(|one| one.name != "repr" || !struct_next)
            {
                return Err(Diagnostic::new(
                    attribute.span,
                    format!("@{} does not apply here", attribute.name),
                ));
            }
            // `pub` names the declaration that follows, whichever kind it is.
            let exported = self.take(|kind| matches!(kind, TokenKind::Pub)).is_some();
            let name = match self.peek().kind.clone() {
                TokenKind::Type if !exported => {
                    fixed_types.push(self.fixed_type()?);
                    continue;
                }
                TokenKind::Struct => {
                    let mut structure = self.structure(false)?;
                    structure.pack = repr_pack(&attributes)?;
                    structs.push(structure);
                    structs.last().map(|one| one.name.clone())
                }
                TokenKind::Bits => {
                    self.bump();
                    if !matches!(self.peek().kind, TokenKind::Struct) {
                        return Err(Diagnostic::new(
                            self.peek().span,
                            "expected 'struct' after 'bits'",
                        ));
                    }
                    structs.push(self.structure(true)?);
                    structs.last().map(|one| one.name.clone())
                }
                TokenKind::Enum => {
                    enums.push(self.enumeration()?);
                    enums.last().map(|one| one.name.clone())
                }
                TokenKind::Protocol => {
                    protocols.push(self.protocol()?);
                    protocols.last().map(|one| one.name.clone())
                }
                TokenKind::Const => {
                    consts.push(self.constant()?);
                    consts.last().map(|one| one.name.clone())
                }
                TokenKind::Type => {
                    return Err(Diagnostic::new(
                        self.peek().span,
                        "a fixed-point type cannot be 'pub' yet",
                    ));
                }
                _ => {
                    functions.push(self.function()?);
                    functions.last().map(|one| one.name.clone())
                }
            };
            if exported {
                public.extend(name);
            }
        }
        Ok(Module {
            imports,
            public,
            externs,
            exports,
            consts,
            fixed_types,
            structs,
            enums,
            protocols,
            functions,
            library: Vec::new(),
        })
    }

    /// `const NAME[: T] = value`, the `const` next; the value must fold.
    fn constant(&mut self) -> Result<Const, Diagnostic> {
        let span = self.bump().span;
        let (name, _) = self.identifier("expected a constant name")?;
        let annotation = if self.take(|kind| matches!(kind, TokenKind::Colon)).is_some() {
            Some(self.type_annotation()?)
        } else {
            None
        };
        self.expect(
            |kind| matches!(kind, TokenKind::Equal),
            "a constant requires a value",
        )?;
        let value = self.expression(0)?;
        self.line_end()?;
        let literal = consts::folded(&value, &self.consts).ok_or_else(|| {
            Diagnostic::new(value.span(), format!("{name} is not a compile-time value"))
        })?;
        let value = consts::typed(literal, annotation.as_ref());
        self.consts.insert(name.clone(), value.clone());
        Ok(Const { name, value, span })
    }

    /// `@name(arguments)` lines before a declaration.
    fn attributes(&mut self) -> Result<Vec<Attribute>, Diagnostic> {
        let mut attributes = Vec::new();
        while let Some(at) = self
            .take(|kind| matches!(kind, TokenKind::At))
            .map(|one| one.span)
        {
            let (name, _) = self.identifier("expected an attribute name after '@'")?;
            let arguments = if matches!(self.peek().kind, TokenKind::LeftParen) {
                match self.call(Expr::Name(name.clone(), at), Vec::new())? {
                    Expr::Call { arguments, .. } => arguments,
                    _ => unreachable!("a named call"),
                }
            } else {
                Vec::new()
            };
            self.line_end()?;
            attributes.push(Attribute {
                name,
                arguments,
                span: at,
            });
        }
        Ok(attributes)
    }

    /// `extern "abi":` and the headers of the functions it imports.
    fn extern_block(&mut self) -> Result<Vec<Extern>, Diagnostic> {
        let (abi, declared) = self.foreign_block(|parser| {
            let attributes = parser.attributes()?;
            let mut symbol = None;
            for attribute in &attributes {
                match (attribute.name.as_str(), attribute.arguments.as_slice()) {
                    ("link_name", [Expr::String(name, _)]) => {
                        symbol = Some(String::from_utf8_lossy(name).into_owned())
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            attribute.span,
                            format!("@{} does not apply to a foreign function", attribute.name),
                        ));
                    }
                }
            }
            // `far fn`, the default: a far call to another code segment.
            if let TokenKind::Identifier(distance) = &parser.peek().kind {
                match distance.as_str() {
                    "far" => {
                        parser.bump();
                    }
                    "near" => {
                        return Err(Diagnostic::new(
                            parser.peek().span,
                            "a near foreign function would share this code segment; declare it far",
                        ));
                    }
                    _ => {}
                }
            }
            let function = parser.function_header()?;
            parser.line_end()?;
            Ok((symbol, function))
        })?;
        Ok(declared
            .into_iter()
            .map(|(symbol, function)| Extern {
                abi,
                symbol: symbol.unwrap_or_else(|| abi.symbol(&function.name)),
                function,
            })
            .collect())
    }

    /// `extern "abi":` or `export "abi":` and the items `item` parses in it.
    fn foreign_block<T>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<(Abi, Vec<T>), Diagnostic> {
        self.bump();
        let token = self.bump().clone();
        let TokenKind::String(abi) = token.kind else {
            return Err(Diagnostic::new(
                token.span,
                "expected an ABI name, as \"cdecl16\"",
            ));
        };
        let name = String::from_utf8_lossy(&abi).into_owned();
        let Some(abi) = Abi::named(&name) else {
            return Err(Diagnostic::new(
                token.span,
                format!("ABI {name:?} is not supported yet; use \"cdecl16\" or \"pascal16\""),
            ));
        };
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' after the ABI name",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before block",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected an indented block",
        )?;
        let mut items = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            if self
                .take(|kind| matches!(kind, TokenKind::Newline))
                .is_some()
            {
                continue;
            }
            items.push(item(self)?);
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated block",
        )?;
        Ok((abi, items))
    }

    /// `import a.b` or `import a.b as c`.
    fn import(&mut self) -> Result<Import, Diagnostic> {
        let span = self.bump().span;
        let (mut module, _) = self.identifier("expected a module name after 'import'")?;
        while self.take(|kind| matches!(kind, TokenKind::Dot)).is_some() {
            let (part, _) = self.identifier("expected a module name after '.'")?;
            module = format!("{module}.{part}");
        }
        let name = if self.take(|kind| matches!(kind, TokenKind::As)).is_some() {
            self.identifier("expected an alias after 'as'")?.0
        } else {
            module.clone()
        };
        self.line_end()?;
        Ok(Import { module, name, span })
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
                ));
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

    /// `struct Name:`, or with `packed`, `bits struct Name: u8`.
    fn structure(&mut self, packed: bool) -> Result<Struct, Diagnostic> {
        let span = self.bump().span;
        let (name, _) = self.identifier("expected struct name")?;
        let generics = self.generics()?;
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' after struct name",
        )?;
        let bits = if packed {
            let at = self.peek().span;
            match self.type_name()? {
                one @ (TypeName::U8 | TypeName::U16 | TypeName::U32) => Some(one),
                _ => {
                    return Err(Diagnostic::new(
                        at,
                        "a bits struct is backed by u8, u16, or u32",
                    ));
                }
            }
        } else {
            None
        };
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
            let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
            let (field_name, field_span) = self.identifier("expected field name")?;
            self.expect(
                |kind| matches!(kind, TokenKind::Colon),
                "expected ':' after field name",
            )?;
            let type_spec = self.field_type()?;
            if type_spec == TypeSpec::Primitive(TypeName::Void) {
                return Err(Diagnostic::new(field_span, "a struct field cannot be void"));
            }
            self.line_end()?;
            fields.push(StructField {
                name: field_name,
                mutable,
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
        Ok(Struct {
            name,
            generics,
            bits,
            pack: None,
            fields,
            span,
        })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let mut function = self.function_header()?;
        function.body = self.suite()?;
        Ok(function)
    }

    /// `protocol Name:` and the method headers a type must match.
    fn protocol(&mut self) -> Result<Protocol, Diagnostic> {
        let span = self.bump().span;
        let (name, _) = self.identifier("expected protocol name")?;
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' after protocol name",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before protocol methods",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected indented protocol methods",
        )?;
        let mut methods = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            let header = self.function_header()?;
            self.line_end()?;
            methods.push((header.name, header.parameters.len()));
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated protocol",
        )?;
        Ok(Protocol {
            name,
            methods,
            span,
        })
    }

    /// `[T, W: Writer]` after a function's name.
    fn generic_parameters(&mut self) -> Result<Vec<GenericParameter>, Diagnostic> {
        let mut parameters = Vec::new();
        if self
            .take(|kind| matches!(kind, TokenKind::LeftBracket))
            .is_none()
        {
            return Ok(parameters);
        }
        loop {
            let (name, _) = self.identifier("expected a type parameter")?;
            let bound = if self.take(|kind| matches!(kind, TokenKind::Colon)).is_some() {
                Some(self.identifier("expected a protocol after ':'")?.0)
            } else {
                None
            };
            parameters.push(GenericParameter { name, bound });
            if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                break;
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RightBracket),
            "expected ']' after type parameters",
        )?;
        Ok(parameters)
    }

    /// `fn name[generics](parameters) -> result`, without a body.
    fn function_header(&mut self) -> Result<Function, Diagnostic> {
        let start = self
            .expect(|kind| matches!(kind, TokenKind::Fn), "expected 'fn'")?
            .span;
        // A built-in type's methods are the library's (section 3).
        let mut name = match primitive(&self.peek().kind) {
            Some(type_name) => {
                let span = self.bump().span;
                if !matches!(self.peek().kind, TokenKind::Dot) {
                    return Err(Diagnostic::new(span, "expected function name"));
                }
                TypeSpec::Primitive(type_name).text()
            }
            None => self.identifier("expected function name")?.0,
        };
        // `fn Point.move(...)`: a method, in its type's namespace.
        if self.take(|kind| matches!(kind, TokenKind::Dot)).is_some() {
            let (method, _) = self.identifier("expected method name after '.'")?;
            name = format!("{name}.{method}");
        }
        let generics = self.generic_parameters()?;
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
                let type_ = self.parameter_type(span)?;
                let default = if self.take(|kind| matches!(kind, TokenKind::Equal)).is_some() {
                    Some(self.expression(0)?)
                } else {
                    None
                };
                if default.is_none()
                    && parameters
                        .iter()
                        .any(|one: &Parameter| one.default.is_some())
                {
                    return Err(Diagnostic::new(
                        span,
                        "a parameter without a default follows one with a default",
                    ));
                }
                parameters.push(Parameter {
                    name: parameter_name,
                    type_,
                    default,
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
        let result = self.result_type(start)?;
        Ok(Function {
            name,
            generics,
            parameters,
            result,
            body: Vec::new(),
            span: start,
        })
    }

    /// `-> T`, after a function's parameters or a function type's.
    fn result_type(&mut self, start: Span) -> Result<TypeAnnotation, Diagnostic> {
        self.expect(
            |kind| matches!(kind, TokenKind::Arrow),
            "expected '->' and a result type",
        )?;
        if self.take(|kind| matches!(kind, TokenKind::Ampersand)).is_none() {
            return self.type_annotation();
        }
        // A borrowed result is a view: `&[T]`, `&T[N]` or `&string`;
        // any other is a reference, `&T` or `&mut T`.
        let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
        Ok(match self.borrowed_annotation()? {
            slice @ TypeAnnotation::Slice { .. } if !mutable => slice,
            TypeAnnotation::Value(TypeSpec::Primitive(TypeName::String)) if !mutable => {
                TypeAnnotation::Slice { element: TypeSpec::Primitive(TypeName::Char), rank: 1 }
            }
            TypeAnnotation::Value(target) => TypeAnnotation::Value(TypeSpec::Applied {
                name: if mutable { "&mut" } else { "&" }.into(),
                args: vec![TypeAnnotation::Value(target)],
            }),
            _ => return Err(Diagnostic::new(start, "a view result is shared: '&[T]' or '&string'")),
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
            TokenKind::Let
                if matches!(
                    self.tokens.get(self.at + 1).map(|one| &one.kind),
                    Some(TokenKind::Const)
                ) =>
            {
                self.bump();
                self.local_constant()
            }
            TokenKind::Const => self.local_constant(),
            TokenKind::Fn => Ok(Statement::Function(Box::new(self.function()?))),
            TokenKind::Unsafe => {
                let span = self.bump().span;
                let body = self.suite()?;
                Ok(Statement::Unsafe { body, span })
            }
            TokenKind::Let => self.binding(),
            TokenKind::Loop => {
                let span = self.bump().span;
                let body = self.suite()?;
                Ok(Statement::While {
                    condition: Expr::Boolean(true, span),
                    body,
                    span,
                })
            }
            TokenKind::Return => self.return_statement(),
            TokenKind::Yield => {
                let span = self.bump().span;
                let value = self.expression(0)?;
                self.line_end()?;
                Ok(Statement::Yield { value, span })
            }
            TokenKind::Match => self.match_statement(),
            TokenKind::With => {
                let span = self.bump().span;
                let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
                let (name, _) = self.identifier("expected a name after 'with'")?;
                self.expect(
                    |kind| matches!(kind, TokenKind::Equal),
                    "expected '=' after the 'with' name",
                )?;
                let value = self.expression(0)?;
                let body = self.suite()?;
                Ok(Statement::With {
                    mutable,
                    name,
                    value,
                    body,
                    span,
                })
            }
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
                    TokenKind::SlashSlashEqual => Some(Some(BinaryOp::FloorDivide)),
                    TokenKind::PercentEqual => Some(Some(BinaryOp::Remainder)),
                    TokenKind::AmpersandEqual => Some(Some(BinaryOp::BitAnd)),
                    TokenKind::PipeEqual => Some(Some(BinaryOp::BitOr)),
                    TokenKind::CaretEqual => Some(Some(BinaryOp::BitXor)),
                    TokenKind::ShiftLeftEqual => Some(Some(BinaryOp::ShiftLeft)),
                    TokenKind::ShiftRightEqual => Some(Some(BinaryOp::ShiftRight)),
                    _ => None,
                };
                if let Some(operation) = operation {
                    self.bump();
                    let span = expression.span();
                    let target = AssignTarget::of(expression)
                        .map_err(|message| Diagnostic::new(span, message))?;
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
        // `let [mut] name[: T] = value` binds a name; anything else is a pattern.
        let named = matches!(self.peek().kind, TokenKind::Mut)
            || (matches!(&self.peek().kind, TokenKind::Identifier(name) if name != "_")
                && matches!(self.tokens.get(self.at + 1).map(|one| &one.kind), Some(TokenKind::Colon | TokenKind::Equal)));
        if !named {
            let pattern = self.pattern()?;
            self.expect(
                |kind| matches!(kind, TokenKind::Equal),
                "a binding requires an initializer",
            )?;
            let value = self.expression(0)?;
            let otherwise = if self.take(|kind| matches!(kind, TokenKind::Else)).is_some() {
                Some(self.suite()?)
            } else {
                self.line_end()?;
                None
            };
            // `let _ = value` binds nothing: the value is a statement's temporary.
            if let (Pattern::Wildcard(_), None) = (&pattern, &otherwise) {
                return Ok(Statement::Expr(value));
            }
            return Ok(Statement::Destructure {
                pattern,
                value,
                otherwise,
                span: token.span,
            });
        }
        let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
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

    /// A constant in a function: a binding of its literal.
    fn local_constant(&mut self) -> Result<Statement, Diagnostic> {
        Ok(Statement::Const(self.constant()?))
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
            // `else if` is an `else` holding one `if`.
            if matches!(self.peek().kind, TokenKind::If) {
                vec![self.if_statement()?]
            } else {
                self.suite()?
            }
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
        let Clause::For {
            pattern,
            refutable,
            mode,
            iterable,
            end,
            span,
        } = self.for_clause()?
        else {
            unreachable!("a for clause")
        };
        let body = self.suite()?;
        Ok(Statement::for_pattern(
            &pattern, refutable, mode, iterable, end, body, span,
        ))
    }

    /// `for [case] pattern in [&[mut]] iterable`, or `for name in start..end`.
    fn for_clause(&mut self) -> Result<Clause, Diagnostic> {
        let span = self.bump().span;
        let refutable = self.take(|kind| matches!(kind, TokenKind::Case)).is_some();
        let pattern = self.pattern()?;
        self.expect(
            |kind| matches!(kind, TokenKind::In),
            "expected 'in' after the loop pattern",
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
        let end = if mode == IterationMode::Value
            && self.take(|kind| matches!(kind, TokenKind::Range)).is_some()
        {
            if !matches!(pattern, Pattern::Binding(..)) || refutable {
                return Err(Diagnostic::new(pattern.span(), "a range binds one name"));
            }
            Some(self.expression(0)?)
        } else {
            None
        };
        Ok(Clause::For {
            pattern,
            refutable,
            mode,
            iterable,
            end,
            span,
        })
    }

    /// The clauses after a comprehension's value: a `for`, then any `for`s and `if`s.
    fn comprehension_clauses(&mut self) -> Result<Vec<Clause>, Diagnostic> {
        let mut clauses = vec![self.for_clause()?];
        loop {
            match self.peek().kind {
                TokenKind::For => clauses.push(self.for_clause()?),
                TokenKind::If => {
                    self.bump();
                    clauses.push(Clause::If(self.expression(0)?));
                }
                _ => return Ok(clauses),
            }
        }
    }

    fn expression(&mut self, minimum_binding: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.prefix(minimum_binding)?;
        let mut compared = false;
        loop {
            if matches!(self.peek().kind, TokenKind::Question) {
                if self.question_is_conditional() {
                    if TERNARY < minimum_binding {
                        break;
                    }
                    left = self.conditional(left)?;
                    continue;
                }
                if POSTFIX < minimum_binding {
                    break;
                }
                let at = self.bump().span;
                let start = left.span();
                left = Expr::Try {
                    operand: Box::new(left),
                    span: Span::new(start.line, start.column, at.end_column),
                };
                continue;
            }
            let postfix = matches!(
                self.peek().kind,
                TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::Dot
            );
            if postfix {
                if POSTFIX < minimum_binding {
                    break;
                }
                left = match self.peek().kind {
                    TokenKind::LeftParen => self.call(left, Vec::new())?,
                    TokenKind::LeftBracket => match self.type_arguments(&left) {
                        Some(types) => self.call(left, types)?,
                        None => self.index(left)?,
                    },
                    _ => self.member(left)?,
                };
                continue;
            }
            let operator = if matches!(self.peek().kind, TokenKind::Is) {
                Some((COMPARE, COMPARE + 1, BinaryOp::Is))
            } else {
                infix(&self.peek().kind)
            };
            let Some((left_binding, right_binding, mut operation)) = operator else {
                break;
            };
            if left_binding < minimum_binding {
                break;
            }
            let at = self.bump().span;
            if operation == BinaryOp::Is
                && self
                    .take(|kind| matches!(kind, TokenKind::Identifier(word) if word == "not"))
                    .is_some()
            {
                operation = BinaryOp::IsNot;
            }
            let right = self.expression(right_binding)?;
            let left_span = left.span();
            let right_span = right.span();
            let span = Span::new(left_span.line, left_span.column, right_span.end_column);
            left = if left_binding == COMPARE && compared {
                chained(left, operation, right, span, at)?
            } else {
                Expr::Binary {
                    op: operation,
                    left: Box::new(left),
                    right: Box::new(right),
                    span,
                }
            };
            compared |= left_binding == COMPARE;
        }
        Ok(left)
    }

    fn prefix(&mut self, minimum_binding: u8) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        if matches!(token.kind, TokenKind::Bang) && minimum_binding > NOT {
            return Err(Diagnostic::new(
                token.span,
                "'!' binds looser than this operator; parenthesize it",
            ));
        }
        match token.kind {
            TokenKind::Integer(value) => Ok(Expr::Integer(value, token.span)),
            TokenKind::Float(value) => Ok(Expr::Float(value, token.span)),
            TokenKind::Character(value) => Ok(Expr::Character(value, token.span)),
            TokenKind::String(value) => Ok(Expr::String(value, token.span)),
            TokenKind::FString(value) => self.fstring(value, token.span),
            TokenKind::Dot => {
                let (name, name_span) = self.identifier("expected variant name after '.'")?;
                let mut end = name_span.end_column;
                let arguments = if matches!(self.peek().kind, TokenKind::LeftParen) {
                    let Expr::Call {
                        arguments, span, ..
                    } = self.call(Expr::Name(name.clone(), name_span), Vec::new())?
                    else {
                        unreachable!("a name's call is a call")
                    };
                    end = span.end_column;
                    arguments
                } else {
                    Vec::new()
                };
                Ok(Expr::Variant {
                    enum_name: None,
                    name,
                    arguments,
                    span: Span::new(token.span.line, token.span.column, end),
                })
            }
            TokenKind::True => Ok(Expr::Boolean(true, token.span)),
            TokenKind::False => Ok(Expr::Boolean(false, token.span)),
            TokenKind::Identifier(name) => {
                if let Some(&target) = self.fixed_types.get(&name) {
                    self.conversion(target, token.span)
                } else {
                    Ok(Expr::Name(name, token.span))
                }
            }
            TokenKind::LeftBrace if self.brace_has_comprehension() => {
                let key = self.expression(0)?;
                self.expect(
                    |kind| matches!(kind, TokenKind::Colon),
                    "expected ':' between dictionary key and value",
                )?;
                let value = self.expression(0)?;
                if !matches!(self.peek().kind, TokenKind::For) {
                    return Err(Diagnostic::new(
                        self.peek().span,
                        "expected 'for' in dictionary comprehension",
                    ));
                }
                let clauses = self.comprehension_clauses()?;
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RightBrace),
                    "expected '}' after dictionary comprehension",
                )?;
                Ok(Expr::DictComprehension {
                    key: Box::new(key),
                    value: Box::new(value),
                    clauses,
                    span: Span::new(token.span.line, token.span.column, close.span.end_column),
                })
            }
            TokenKind::LeftBrace => {
                let mut entries = Vec::new();
                while !matches!(self.peek().kind, TokenKind::RightBrace) {
                    let key = self.expression(0)?;
                    self.expect(
                        |kind| matches!(kind, TokenKind::Colon),
                        "expected ':' between dictionary key and value",
                    )?;
                    entries.push((key, self.expression(0)?));
                    if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                        break;
                    }
                }
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RightBrace),
                    "expected '}' after dictionary entries",
                )?;
                Ok(Expr::Dict(entries, Span::new(token.span.line, token.span.column, close.span.end_column)))
            }
            TokenKind::Ampersand => {
                let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
                let operand = self.expression(UNARY)?;
                let end = operand.span().end_column;
                Ok(Expr::Borrow {
                    mutable,
                    operand: Box::new(operand),
                    span: Span::new(token.span.line, token.span.column, end),
                })
            }
            TokenKind::LeftBracket => {
                let mut values = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RightBracket) {
                    let first = self.expression(0)?;
                    if matches!(self.peek().kind, TokenKind::For) {
                        let clauses = self.comprehension_clauses()?;
                        let close = self.expect(
                            |kind| matches!(kind, TokenKind::RightBracket),
                            "expected ']' after comprehension",
                        )?;
                        return Ok(Expr::Comprehension {
                            element: Box::new(first),
                            clauses,
                            span: Span::new(
                                token.span.line,
                                token.span.column,
                                close.span.end_column,
                            ),
                        });
                    }
                    values.push(first);
                    loop {
                        if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                            break;
                        }
                        if matches!(self.peek().kind, TokenKind::RightBracket) {
                            break;
                        }
                        values.push(self.expression(0)?);
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
            TokenKind::Minus | TokenKind::Tilde | TokenKind::Bang | TokenKind::Star => {
                let (operation, binding) = match token.kind {
                    TokenKind::Minus => (UnaryOp::Negative, UNARY),
                    TokenKind::Tilde => (UnaryOp::Complement, UNARY),
                    TokenKind::Star => (UnaryOp::Deref, UNARY),
                    _ => (UnaryOp::Not, NOT),
                };
                let operand = self.expression(binding)?;
                let end = operand.span().end_column;
                Ok(Expr::Unary {
                    op: operation,
                    operand: Box::new(operand),
                    span: Span::new(token.span.line, token.span.column, end),
                })
            }
            kind if primitive(&kind).is_some() => {
                self.conversion(primitive(&kind).expect("matched"), token.span)
            }
            // `|x: i16, y| body`, or `|| body` with no parameters.
            TokenKind::Pipe | TokenKind::OrOr => {
                let mut parameters = Vec::new();
                if matches!(token.kind, TokenKind::Pipe) {
                    while self.take(|kind| matches!(kind, TokenKind::Pipe)).is_none() {
                        let (name, _) = self.identifier("expected a lambda parameter")?;
                        let type_ = if self.take(|kind| matches!(kind, TokenKind::Colon)).is_some()
                        {
                            Some(self.type_spec()?)
                        } else {
                            None
                        };
                        parameters.push(LambdaParameter { name, type_ });
                        if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                            self.expect(
                                |kind| matches!(kind, TokenKind::Pipe),
                                "expected '|' after lambda parameters",
                            )?;
                            break;
                        }
                    }
                }
                let body = self.expression(0)?;
                let end = body.span().end_column;
                Ok(Expr::Lambda {
                    parameters,
                    body: Box::new(body),
                    span: Span::new(token.span.line, token.span.column, end),
                })
            }
            TokenKind::LeftParen => {
                let expression = self.expression(0)?;
                if matches!(self.peek().kind, TokenKind::For) {
                    let clauses = self.comprehension_clauses()?;
                    let close = self.expect(
                        |kind| matches!(kind, TokenKind::RightParen),
                        "expected ')' after generator",
                    )?;
                    return Ok(Expr::Generator {
                        element: Box::new(expression),
                        clauses,
                        span: Span::new(token.span.line, token.span.column, close.span.end_column),
                    });
                }
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
                    let mut items = vec![expression];
                    while !matches!(self.peek().kind, TokenKind::RightParen) {
                        items.push(self.expression(0)?);
                        if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                            break;
                        }
                    }
                    let close = self.expect(
                        |kind| matches!(kind, TokenKind::RightParen),
                        "expected ')' after a tuple",
                    )?;
                    return Ok(Expr::Tuple(
                        items,
                        Span::new(token.span.line, token.span.column, close.span.end_column),
                    ));
                }
                self.expect(
                    |kind| matches!(kind, TokenKind::RightParen),
                    "expected ')' after expression",
                )?;
                Ok(expression)
            }
            _ => Err(Diagnostic::new(token.span, "expected expression")),
        }
    }

    /// `T(value)`, after the type's name.
    fn conversion(&mut self, target: TypeName, start: Span) -> Result<Expr, Diagnostic> {
        self.expect(
            |kind| matches!(kind, TokenKind::LeftParen),
            "expected '(' after a conversion's type",
        )?;
        let value = self.expression(0)?;
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RightParen),
            "expected ')' after a conversion's value",
        )?;
        Ok(Expr::Conversion {
            target,
            value: Box::new(value),
            span: Span::new(start.line, start.column, close.span.end_column),
        })
    }

    fn brace_has_comprehension(&self) -> bool {
        let mut depth = 0_i32;
        for token in &self.tokens[self.at..] {
            match token.kind {
                TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => depth += 1,
                TokenKind::RightParen | TokenKind::RightBracket => depth -= 1,
                TokenKind::RightBrace if depth == 0 => return false,
                TokenKind::RightBrace => depth -= 1,
                TokenKind::For if depth == 0 => return true,
                _ => {}
            }
        }
        false
    }

    /// `[T, ...]` ahead of a named callee's `(`: its type arguments. The
    /// tokens are left as they were when they are not.
    fn type_arguments(&mut self, callee: &Expr) -> Option<Vec<TypeSpec>> {
        if !matches!(callee, Expr::Name(..) | Expr::Member { .. }) {
            return None;
        }
        let start = self.at;
        let mut attempt = || {
            self.bump();
            let mut types = vec![self.type_spec().ok()?];
            while self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
                types.push(self.type_spec().ok()?);
            }
            self.take(|kind| matches!(kind, TokenKind::RightBracket))?;
            matches!(self.peek().kind, TokenKind::LeftParen).then_some(types)
        };
        let types = attempt();
        if types.is_none() {
            self.at = start;
        }
        types
    }

    fn call(&mut self, callee: Expr, type_arguments: Vec<TypeSpec>) -> Result<Expr, Diagnostic> {
        let start = callee.span();
        self.bump();
        let mut arguments = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RightParen) {
            loop {
                arguments.push(self.argument()?);
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
        let span = Span::new(start.line, start.column, close.span.end_column);
        match callee {
            Expr::Name(name, _) => Ok(Expr::Call {
                name,
                type_arguments,
                arguments,
                span,
            }),
            Expr::Member { base, field, .. } => Ok(Expr::MethodCall {
                receiver: base,
                name: field,
                type_arguments,
                arguments,
                span,
            }),
            other => Err(Diagnostic::new(
                other.span(),
                "only named functions and methods can be called",
            )),
        }
    }

    fn index(&mut self, base: Expr) -> Result<Expr, Diagnostic> {
        let start = base.span();
        self.bump();
        let first = if matches!(self.peek().kind, TokenKind::Colon) {
            None
        } else {
            Some(self.expression(0)?)
        };
        if self.take(|kind| matches!(kind, TokenKind::Colon)).is_some() {
            let end = if matches!(self.peek().kind, TokenKind::RightBracket) {
                None
            } else {
                Some(Box::new(self.expression(0)?))
            };
            let close = self.expect(
                |kind| matches!(kind, TokenKind::RightBracket),
                "expected ']' after slice",
            )?;
            return Ok(Expr::Slice {
                base: Box::new(base),
                start: first.map(Box::new),
                end,
                span: Span::new(start.line, start.column, close.span.end_column),
            });
        }
        let mut indices = vec![first.expect("an index with no start is a slice")];
        while self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
            indices.push(self.expression(0)?);
        }
        if indices.len() > MAX_RANK {
            return Err(Diagnostic::new(
                start,
                format!("an array has at most {MAX_RANK} dimensions"),
            ));
        }
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RightBracket),
            "expected ']' after index",
        )?;
        Ok(Expr::Index {
            base: Box::new(base),
            indices,
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
                    parts.push(self.interpolation(source, span)?);
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

    /// `T`, or `T[d0, d1, ...]` for a fixed row-major array.
    /// `[A, B]` after a declared type's name; none when absent.
    fn generics(&mut self) -> Result<Vec<String>, Diagnostic> {
        let mut names = Vec::new();
        if self
            .take(|kind| matches!(kind, TokenKind::LeftBracket))
            .is_none()
        {
            return Ok(names);
        }
        loop {
            names.push(self.identifier("expected a type parameter")?.0);
            if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                break;
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RightBracket),
            "expected ']' after type parameters",
        )?;
        Ok(names)
    }

    fn type_annotation(&mut self) -> Result<TypeAnnotation, Diagnostic> {
        let mut element = if self
            .take(|kind| matches!(kind, TokenKind::LeftParen))
            .is_some()
        {
            // `(A, B)`: a tuple type.
            let mut args = vec![self.type_annotation()?];
            while self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
                args.push(self.type_annotation()?);
            }
            self.expect(
                |kind| matches!(kind, TokenKind::RightParen),
                "expected ')' after tuple element types",
            )?;
            TypeSpec::Applied {
                name: TUPLE.into(),
                args,
            }
        } else {
            self.type_spec()?
        };
        // `Name[T, ...]` applies a generic; `T[N, ...]` is an array.
        while matches!(self.peek().kind, TokenKind::LeftBracket) && !self.dimension_next() {
            let TypeSpec::Named(name) = element else {
                return Err(Diagnostic::new(
                    self.peek().span,
                    "only a named type takes type arguments",
                ));
            };
            self.bump();
            let mut args = vec![self.type_annotation()?];
            while self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
                args.push(self.type_annotation()?);
            }
            self.expect(
                |kind| matches!(kind, TokenKind::RightBracket),
                "expected ']' after type arguments",
            )?;
            element = TypeSpec::Applied { name, args };
        }
        if self
            .take(|kind| matches!(kind, TokenKind::LeftBracket))
            .is_none()
        {
            return Ok(TypeAnnotation::Value(element));
        }
        if element == TypeSpec::Primitive(TypeName::Void) {
            return Err(Diagnostic::new(
                self.peek().span,
                "an array element cannot be void",
            ));
        }
        let mut dims = vec![self.dimension("array length")?];
        while self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
            dims.push(self.dimension("array length")?);
        }
        if dims.len() > MAX_RANK {
            return Err(Diagnostic::new(
                self.peek().span,
                format!("an array has at most {MAX_RANK} dimensions"),
            ));
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RightBracket),
            "expected ']' after array type",
        )?;
        Ok(TypeAnnotation::Array { element, dims })
    }

    /// What follows `&` or `&mut`: `[T]`, `[T, rank]`, `T[d0, ...]`, or `T`.
    fn borrowed_annotation(&mut self) -> Result<TypeAnnotation, Diagnostic> {
        if self
            .take(|kind| matches!(kind, TokenKind::LeftBracket))
            .is_none()
        {
            let target = self.type_annotation()?;
            if let TypeAnnotation::Array { element, dims } = target {
                // A borrowed fixed array is a view whose rank the type fixes.
                return Ok(TypeAnnotation::Slice {
                    element,
                    rank: dims.len() as u8,
                });
            }
            return Ok(target);
        }
        let element = self.type_spec()?;
        if element == TypeSpec::Primitive(TypeName::Void) {
            return Err(Diagnostic::new(
                self.peek().span,
                "an array element cannot be void",
            ));
        }
        let rank = if self.take(|kind| matches!(kind, TokenKind::Comma)).is_some() {
            let rank = self.dimension("a view's rank")?;
            if !(1..=MAX_RANK as u32).contains(&rank) {
                return Err(Diagnostic::new(
                    self.peek().span,
                    format!("a view's rank is 1 to {MAX_RANK}"),
                ));
            }
            rank as u8
        } else {
            1
        };
        self.expect(
            |kind| matches!(kind, TokenKind::RightBracket),
            "expected ']' after a view's element type",
        )?;
        Ok(TypeAnnotation::Slice { element, rank })
    }

    fn parameter_type(&mut self, span: Span) -> Result<ParameterType, Diagnostic> {
        if self
            .take(|kind| matches!(kind, TokenKind::Ampersand))
            .is_some()
        {
            let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
            let target = self.borrowed_annotation()?;
            if target == TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Void)) {
                return Err(Diagnostic::new(span, "a parameter cannot borrow void"));
            }
            return Ok(ParameterType::Borrowed { mutable, target });
        }
        let annotation = self.type_annotation()?;
        if annotation == TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Void)) {
            return Err(Diagnostic::new(span, "a parameter cannot have type void"));
        }
        Ok(ParameterType::Owned(annotation))
    }

    /// A call argument: `value`, or `name=value`.
    fn argument(&mut self) -> Result<Expr, Diagnostic> {
        let named = matches!(self.peek().kind, TokenKind::Identifier(_))
            && self
                .tokens
                .get(self.at + 1)
                .is_some_and(|token| matches!(token.kind, TokenKind::Equal));
        if !named {
            return self.expression(0);
        }
        let (name, span) = self.identifier("expected argument name")?;
        self.bump();
        let value = self.expression(0)?;
        let end = value.span().end_column;
        Ok(Expr::NamedArgument {
            name,
            value: Box::new(value),
            span: Span::new(span.line, span.column, end),
        })
    }

    /// Whether the `?` ahead opens `? then : otherwise` rather than
    /// propagating a failure: a `:` follows it at the same depth on the line,
    /// and not at its end, where it opens a suite (`with x = f()?:`).
    fn question_is_conditional(&self) -> bool {
        let mut depth = 0_i32;
        let rest = &self.tokens[self.at + 1..];
        for (index, token) in rest.iter().enumerate() {
            match token.kind {
                TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => depth += 1,
                TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                TokenKind::Colon if depth == 0 => {
                    return !matches!(rest.get(index + 1).map(|next| &next.kind), Some(TokenKind::Newline | TokenKind::Eof));
                }
                TokenKind::Comma if depth == 0 => return false,
                TokenKind::Newline | TokenKind::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn conditional(&mut self, condition: Expr) -> Result<Expr, Diagnostic> {
        self.bump();
        let then = self.expression(TERNARY + 1)?;
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' in a conditional expression",
        )?;
        let otherwise = self.expression(TERNARY)?;
        let start = condition.span();
        let end = otherwise.span().end_column;
        Ok(Expr::Conditional {
            condition: Box::new(condition),
            then: Box::new(then),
            otherwise: Box::new(otherwise),
            span: Span::new(start.line, start.column, end),
        })
    }

    /// A positive integer literal that fits u32.
    /// Whether the `[` next opens array dimensions: a number or a constant.
    fn dimension_next(&self) -> bool {
        match self.tokens.get(self.at + 1).map(|one| &one.kind) {
            Some(TokenKind::Integer(_)) => true,
            Some(TokenKind::Identifier(name)) => self.consts.contains_key(name),
            _ => false,
        }
    }

    fn dimension(&mut self, what: &str) -> Result<u32, Diagnostic> {
        let token = self.bump().clone();
        let value = match &token.kind {
            TokenKind::Integer(value) => Some(*value),
            TokenKind::Identifier(name) => self.consts.get(name).and_then(consts::integer),
            _ => None,
        };
        let Some(value) = value else {
            return Err(Diagnostic::new(
                token.span,
                format!("{what} must be an integer literal or constant"),
            ));
        };
        u32::try_from(value)
            .ok()
            .filter(|one| *one > 0)
            .ok_or_else(|| {
                Diagnostic::new(token.span, format!("{what} must be positive and fit u32"))
            })
    }

    fn type_name(&mut self) -> Result<TypeName, Diagnostic> {
        let token = self.bump().clone();
        if let Some(type_name) = primitive(&token.kind) {
            return Ok(type_name);
        }
        match token.kind {
            TokenKind::Identifier(name) => self.fixed_types.get(&name).copied().ok_or_else(|| {
                Diagnostic::new(token.span, format!("unknown scalar type {name:?}"))
            }),
            _ => Err(Diagnostic::new(token.span, "expected a type name")),
        }
    }

    /// A struct or variant field's type: a scalar or a named, possibly generic, type.
    fn field_type(&mut self) -> Result<TypeSpec, Diagnostic> {
        let span = self.peek().span;
        match self.type_annotation()? {
            TypeAnnotation::Value(spec) => Ok(spec),
            _ => Err(Diagnostic::new(span, "a field cannot be an array yet")),
        }
    }

    fn type_spec(&mut self) -> Result<TypeSpec, Diagnostic> {
        // `fn(T, &U) -> R`: a function value's type. Its parameters are
        // written as a header's are; a borrowed one is `&` applied.
        if let Some(start) = self.take(|kind| matches!(kind, TokenKind::Fn)).map(|one| one.span) {
            self.expect(|kind| matches!(kind, TokenKind::LeftParen), "expected '(' after 'fn'")?;
            let mut args = Vec::new();
            while !matches!(self.peek().kind, TokenKind::RightParen) {
                args.push(match self.parameter_type(start)? {
                    ParameterType::Owned(one) => one,
                    ParameterType::Borrowed { mutable, target } => TypeAnnotation::Value(TypeSpec::Applied {
                        name: if mutable { "&mut" } else { "&" }.into(),
                        args: vec![target],
                    }),
                });
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                    break;
                }
            }
            self.expect(|kind| matches!(kind, TokenKind::RightParen), "expected ')' after parameter types")?;
            args.push(self.result_type(start)?);
            return Ok(TypeSpec::Applied { name: FUNCTION.into(), args });
        }
        // `&T`, `&mut T`: a reference, as a tuple element or type argument.
        if self.take(|kind| matches!(kind, TokenKind::Ampersand)).is_some() {
            let mutable = self.take(|kind| matches!(kind, TokenKind::Mut)).is_some();
            let target = self.type_spec()?;
            return Ok(TypeSpec::Applied {
                name: if mutable { "&mut" } else { "&" }.into(),
                args: vec![TypeAnnotation::Value(target)],
            });
        }
        // `*far T`, `*near mut T`: a raw pointer.
        if self.take(|kind| matches!(kind, TokenKind::Star)).is_some() {
            let (distance, span) = self.identifier("expected 'near', 'far' or 'huge' after '*'")?;
            if !["near", "far", "huge"].contains(&distance.as_str()) {
                return Err(Diagnostic::new(span, "a raw pointer is '*near', '*far' or '*huge'"));
            }
            let mutable = if self.take(|kind| matches!(kind, TokenKind::Mut)).is_some() {
                " mut"
            } else {
                ""
            };
            let target = self.type_spec()?;
            return Ok(TypeSpec::Applied {
                name: format!("*{distance}{mutable}"),
                args: vec![TypeAnnotation::Value(target)],
            });
        }
        if let TokenKind::Identifier(name) = &self.peek().kind {
            let mut name = name.clone();
            self.bump();
            // `module.Type`, a type another module declares.
            while matches!(self.peek().kind, TokenKind::Dot) {
                self.bump();
                let (part, _) = self.identifier("expected a type name after '.'")?;
                name = format!("{name}.{part}");
            }
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

/// `a < b < c` is `(a < b) && (b < c)`. The middle operand is written twice,
/// so it must be free of effects: a name, a literal, or a field of one.
fn chained(
    left: Expr,
    operation: BinaryOp,
    right: Expr,
    span: Span,
    at: Span,
) -> Result<Expr, Diagnostic> {
    let previous = match &left {
        Expr::Binary {
            op: BinaryOp::And,
            right: last,
            ..
        } => last.as_ref(),
        other => other,
    };
    let Expr::Binary { right: middle, .. } = previous else {
        return Err(Diagnostic::new(
            at,
            "a chained comparison needs a comparison before it",
        ));
    };
    if !is_pure(middle) {
        return Err(Diagnostic::new(
            middle.span(),
            "a chained comparison's middle operand must be a name, literal, or field",
        ));
    }
    let next = Expr::Binary {
        op: operation,
        left: middle.clone(),
        right: Box::new(right),
        span,
    };
    Ok(Expr::Binary {
        op: BinaryOp::And,
        left: Box::new(left),
        right: Box::new(next),
        span,
    })
}

fn is_pure(expression: &Expr) -> bool {
    match expression {
        Expr::Integer(..)
        | Expr::Float(..)
        | Expr::Character(..)
        | Expr::Boolean(..)
        | Expr::Name(..) => true,
        Expr::Member { base, .. } => is_pure(base),
        Expr::Unary { operand, .. } => is_pure(operand),
        _ => false,
    }
}

impl Parser {
    /// `value` or `value:code`. A colon splits off a code only when what
    /// precedes it parses on its own, so a conditional's colon stays.
    fn interpolation(&self, source: &str, span: Span) -> Result<FStringPart, Diagnostic> {
        if let Some((value, code)) = source.rsplit_once(':') {
            if let (Some(format), Ok(value)) = (
                Format::parse(code.trim()),
                parse_inline_expression(value, span, &self.fixed_types),
            ) {
                return Ok(FStringPart::Value(value, format));
            }
        }
        Ok(FStringPart::Value(
            parse_inline_expression(source, span, &self.fixed_types)?,
            Format::default(),
        ))
    }
}

fn parse_inline_expression(
    source: &str,
    outer: Span,
    fixed_types: &BTreeMap<String, TypeName>,
) -> Result<Expr, Diagnostic> {
    let mut parser = Parser {
        tokens: lex(source).map_err(|error| Diagnostic::new(outer, error.message))?,
        at: 0,
        fixed_types: fixed_types.clone(),
        consts: BTreeMap::new(),
    };
    let expression = parser
        .expression(0)
        .map_err(|error| Diagnostic::new(outer, error.message))?;
    if !matches!(parser.peek().kind, TokenKind::Newline | TokenKind::Eof) {
        return Err(Diagnostic::new(outer, "invalid f-string interpolation"));
    }
    Ok(expression)
}

pub(crate) fn primitive(kind: &TokenKind) -> Option<TypeName> {
    Some(match kind {
        TokenKind::Char => TypeName::Char,
        TokenKind::I8 => TypeName::I8,
        TokenKind::U8 => TypeName::U8,
        TokenKind::I16 => TypeName::I16,
        TokenKind::U16 => TypeName::U16,
        TokenKind::I32 => TypeName::I32,
        TokenKind::U32 => TypeName::U32,
        TokenKind::F32 => TypeName::F32,
        TokenKind::F64 => TypeName::F64,
        TokenKind::StringType => TypeName::String,
        TokenKind::Addr => TypeName::Addr,
        TokenKind::Bool => TypeName::Bool,
        TokenKind::Void => TypeName::Void,
        _ => return None,
    })
}

/// Binding powers, loosest first; a binary operator's right side binds one tighter.
const TERNARY: u8 = 1;
const OR: u8 = 2;
const AND: u8 = 4;
const NOT: u8 = 6;
const COMPARE: u8 = 8;
const UNARY: u8 = 25;
const POSTFIX: u8 = 30;

fn infix(kind: &TokenKind) -> Option<(u8, u8, BinaryOp)> {
    let (binding, operation) = match kind {
        TokenKind::OrOr => (OR, BinaryOp::Or),
        TokenKind::AndAnd => (AND, BinaryOp::And),
        TokenKind::EqualEqual => (COMPARE, BinaryOp::Equal),
        TokenKind::NotEqual => (COMPARE, BinaryOp::NotEqual),
        TokenKind::Less => (COMPARE, BinaryOp::Less),
        TokenKind::LessEqual => (COMPARE, BinaryOp::LessEqual),
        TokenKind::Greater => (COMPARE, BinaryOp::Greater),
        TokenKind::GreaterEqual => (COMPARE, BinaryOp::GreaterEqual),
        TokenKind::Pipe => (10, BinaryOp::BitOr),
        TokenKind::Caret => (12, BinaryOp::BitXor),
        TokenKind::Ampersand => (14, BinaryOp::BitAnd),
        TokenKind::ShiftLeft => (16, BinaryOp::ShiftLeft),
        TokenKind::ShiftRight => (16, BinaryOp::ShiftRight),
        TokenKind::Plus => (18, BinaryOp::Add),
        TokenKind::Minus => (18, BinaryOp::Subtract),
        TokenKind::Star => (20, BinaryOp::Multiply),
        TokenKind::Slash => (20, BinaryOp::Divide),
        TokenKind::SlashSlash => (20, BinaryOp::FloorDivide),
        TokenKind::Percent => (20, BinaryOp::Remainder),
        _ => return None,
    };
    Some((binding, binding + 1, operation))
}

#[cfg(test)]
mod tests {
    use super::super::lexer::lex;

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
                 \x20\x20\x20\x20let mut values: i32[2] = [10, 20]\n\
                 \x20\x20\x20\x20values[1] = 30\n\
                 \x20\x20\x20\x20print(f\"value={values[1]}\")\n")
            .unwrap(),
        )
        .unwrap();
        let Statement::Bind {
            annotation:
                Some(TypeAnnotation::Array {
                    element: TypeSpec::Primitive(TypeName::I32),
                    ..
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
                 \x20\x20\x20\x20let mut total: i32 = 0\n\
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
            module.functions[0].parameters[0].type_,
            ParameterType::Owned(TypeAnnotation::Value(TypeSpec::Primitive(
                module.fixed_types[0].type_name
            )))
        );
        assert_eq!(
            module.functions[0].result,
            TypeAnnotation::Value(TypeSpec::Primitive(module.fixed_types[0].type_name))
        );
    }
}

/// `@name(arguments)`, before the declaration it applies to.
struct Attribute {
    name: String,
    arguments: Vec<Expr>,
    span: Span,
}

/// The field alignment `@repr("c16", pack=N)` sets, if the attributes give one.
fn repr_pack(attributes: &[Attribute]) -> Result<Option<u32>, Diagnostic> {
    let Some(repr) = attributes.iter().find(|one| one.name == "repr") else {
        return Ok(None);
    };
    let mut pack = 2;
    for argument in &repr.arguments {
        match argument {
            Expr::String(abi, span) if abi.as_slice() != b"c16" => {
                return Err(Diagnostic::new(*span, "only the \"c16\" layout is known"));
            }
            Expr::String(..) => {}
            Expr::NamedArgument { name, value, span } if name == "pack" => match value.as_ref() {
                Expr::Integer(value @ (1 | 2), _) => pack = *value as u32,
                _ => return Err(Diagnostic::new(*span, "pack is 1 or 2")),
            },
            other => {
                return Err(Diagnostic::new(
                    other.span(),
                    "@repr takes a layout and pack=N",
                ));
            }
        }
    }
    Ok(Some(pack))
}
