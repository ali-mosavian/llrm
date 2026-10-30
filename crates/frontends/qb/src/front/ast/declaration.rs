//! Declarations: DIM, TYPE, CONST, DEF, procedures and their parameters.

use super::*;

impl Ast<'_> {
    /// One DATA item: a number where it is plainly one, else the text (a quoted
    /// item without its quotes).
    pub(super) fn data_item(
        &self,
        item: &str,
        span: Span,
    ) -> Expr {
        if let Some(rest) = item.strip_prefix('"') {
            return Expr::Literal(Literal::String(rest.split('"').next().unwrap_or("").to_owned()), span);
        }
        let (sign, digits) = match item.as_bytes().first() {
            Some(b'-') => (Some(Unary::Negative), &item[1..]),
            Some(b'+') => (Some(Unary::Positive), &item[1..]),
            _ => (None, item),
        };
        if digits.as_bytes().first().is_some_and(|b| b.is_ascii_digit() || *b == b'.' || *b == b'&') {
            if let Ok(tokens) = crate::front::scan::scan(digits, self.dialect) {
                if let [Token { kind, .. }, Token { kind: TokenKind::Reserved(_), .. }] = tokens.as_slice() {
                    let literal = match kind {
                        TokenKind::Integer(value, suffix) => Some(Expr::Literal(
                            Literal::Integer(
                                *value,
                                if *suffix == Some('&') { TypeName::Long } else { TypeName::Integer },
                            ),
                            span,
                        )),
                        TokenKind::Real(text, suffix) => {
                            Some(Expr::Literal(Literal::Real(text.clone(), real_type(*suffix)), span))
                        }
                        _ => None,
                    };
                    if let Some(literal) = literal {
                        return match sign {
                            Some(op) => Expr::Unary { op, operand: Box::new(literal), span },
                            None => literal,
                        };
                    }
                }
            }
        }
        Expr::Literal(Literal::String(item.to_owned()), span)
    }

    /// DATA's text as written: the items between its commas, a quoted one with
    /// its quotes dropped.
    pub(super) fn data_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let raw_at = self.i;
        let TokenKind::String(raw) = &self.tokens[raw_at].kind else { return self.fail() };
        self.i += 1;
        let origin = self.span(raw_at);
        let mut values = Vec::new();
        let bytes = raw.as_bytes();
        let mut from = 0;
        loop {
            let mut at = from;
            let mut quoted = false;
            while at < bytes.len() && (quoted || bytes[at] != b',') {
                if bytes[at] == b'"' {
                    quoted = !quoted;
                }
                at += 1;
            }
            let item = raw[from..at].trim();
            let span = Span { line: origin.line, start: origin.start + from, end: origin.start + at };
            values.push(self.data_item(item, span));
            if at >= bytes.len() {
                break;
            }
            from = at + 1;
        }
        self.out.push(Statement::Data { values, span: self.span(keyword) });
        Ok(())
    }

    // ---- declarations
    // ---------------------------------------------------------------------------------

    /// `SIGNED`/`UNSIGNED` before `BYTE`, `INTEGER` or `LONG`; both words stay
    /// names elsewhere.
    pub(super) fn signed_type(&mut self) -> Option<TypeName> {
        let TokenKind::Identifier(modifier) = &self.tokens.get(self.i)?.kind else { return None };
        let signed = match modifier.as_str() {
            "SIGNED" => true,
            "UNSIGNED" => false,
            _ => return None,
        };
        let width = match &self.tokens.get(self.i + 1)?.kind {
            TokenKind::Identifier(name) if name == "BYTE" => 1,
            _ if self.kinds.get(self.i + 1) == Some(&Tok::Kw(Kw::INTEGER)) => 2,
            _ if self.kinds.get(self.i + 1) == Some(&Tok::Kw(Kw::LONG)) => 4,
            _ => return None,
        };
        self.i += 2;
        Some(TypeName::Integral { width, signed })
    }

    /// A type after AS. QuickrBASIC adds tuples, `(INTEGER, LONG)`, and arrays,
    /// `LONG()`, for a FUNCTION's result.
    pub(super) fn declaration_type(&mut self) -> R<TypeName> {
        if self.dialect.python_expressions() {
            if self.eat(Kw::LParen) {
                let mut types = vec![self.declaration_type()?];
                while self.eat(Kw::Comma) {
                    types.push(self.declaration_type()?);
                }
                self.expect(Kw::RParen)?;
                return if types.len() > 1 { Ok(TypeName::Tuple(types)) } else { self.fail() };
            }
            let element = self.element_type()?;
            if self.is(Kw::LParen) && self.is_n(1, Kw::RParen) {
                self.i += 2;
                return Ok(TypeName::Array(Box::new(element)));
            }
            return Ok(element);
        }
        self.element_type()
    }

    fn element_type(&mut self) -> R<TypeName> {
        if let Some(integral) = self.signed_type() {
            return Ok(integral);
        }
        let at = self.i;
        let type_name = match &self.tokens[at].kind {
            TokenKind::Reserved(_) => match self.kw() {
                Some(Kw::INTEGER) => TypeName::Integer,
                Some(Kw::LONG) => TypeName::Long,
                Some(Kw::SINGLE) => TypeName::Single,
                Some(Kw::DOUBLE) => TypeName::Double,
                Some(Kw::STRING) => TypeName::String,
                Some(Kw::ANY) => TypeName::Named("ANY".into()),
                // QuickrBASIC lets a record type take a word BC reserves (`AS
                // Point`)
                _ => TypeName::Named(match (self.name_at(at), &self.tokens[at].kind) {
                    (Some(name), _) => name,
                    (None, TokenKind::Reserved(word)) if self.dialect.format_strings() => {
                        word.spelling().to_ascii_uppercase()
                    }
                    _ => return Err(at),
                }),
            },
            TokenKind::Identifier(name) if name == "CURRENCY" && self.dialect.currency_suffix() => TypeName::Currency,
            TokenKind::Identifier(name) => TypeName::Named(name.clone()),
            _ => return self.fail(),
        };
        self.i += 1;
        Ok(type_name)
    }

    pub(super) fn declaration(
        &mut self,
        shared: bool,
        require_array: bool,
    ) -> R<Declaration> {
        let at = self.i;
        let mut name = self.name_at(at).ok_or(at)?;
        if !matches!(self.tok_n(0), Some(Tok::Ident | Tok::Kw(_))) {
            return self.fail();
        }
        self.i += 1;
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            name.push('.');
            name.push_str(&self.field_name()?.0);
        }
        let mut bounds = Vec::new();
        let mut array = false;
        if self.eat(Kw::LParen) {
            array = true;
            if !self.eat(Kw::RParen) {
                loop {
                    let first = self.exp()?;
                    let (lower, upper) = if self.eat(Kw::TO) { (Some(first), self.exp()?) } else { (None, first) };
                    bounds.push(Bound { lower, upper });
                    if self.eat(Kw::Comma) {
                        continue;
                    }
                    self.expect(Kw::RParen)?;
                    break;
                }
            }
        }
        if require_array && !array {
            return self.fail();
        }
        let mut fixed_length = None;
        let type_name = if self.eat(Kw::AS) {
            let type_name = self.declaration_type()?;
            if type_name == TypeName::String && self.eat(Kw::Mult) {
                fixed_length = Some(self.exp()?);
            }
            Some(type_name)
        } else {
            name_of_suffix(&name)
        };
        let token = self.span(at);
        Ok(Declaration {
            name,
            type_name,
            array,
            bounds,
            fixed_length,
            shared,
            dynamic: self.dynamic_arrays,
            volatile: self.volatile_at.contains(&(token.line, token.start)),
            span: Span { line: token.line, start: token.start, end: self.previous_end() },
        })
    }

    pub(super) fn declaration_statement(
        &mut self,
        k: Kw,
    ) -> R<()> {
        self.i += 1;
        let shared = matches!(k, Kw::DIM | Kw::REDIM) && self.eat(Kw::SHARED);
        let preserve = k == Kw::REDIM
            && self.dialect_has_isam()
            && matches!(
                self.tokens.get(self.i).map(|t| &t.kind),
                Some(TokenKind::Identifier(name)) if name == "PRESERVE"
            )
            && {
                self.i += 1;
                true
            };
        let mut declarations = vec![self.declaration(shared, k == Kw::REDIM)?];
        while self.eat(Kw::Comma) {
            declarations.push(self.declaration(shared, k == Kw::REDIM)?);
        }
        // SHARED inside a procedure names module variables: they are shared
        let declarations = if k == Kw::SHARED {
            declarations.into_iter().map(|declaration| Declaration { shared: true, ..declaration }).collect()
        } else {
            declarations
        };
        self.out.push(match k {
            Kw::DIM => Statement::Dim(declarations),
            Kw::REDIM if preserve => Statement::RedimPreserve(declarations),
            Kw::REDIM => Statement::Redim(declarations),
            Kw::STATIC => Statement::Static(declarations),
            _ => Statement::Shared(declarations),
        });
        Ok(())
    }

    /// `COMMON [SHARED] [/block/] a, b AS INTEGER`: variables a CHAINed program
    /// would keep. With no CHAIN to carry them it declares them, shared where
    /// SHARED says so.
    pub(super) fn common(&mut self) -> R<()> {
        self.i += 1;
        let keyword = self.i - 1;
        let shared = self.eat(Kw::SHARED);
        let mut block = None;
        if self.eat(Kw::Div) {
            block = Some(self.identifier()?.0);
            self.expect(Kw::Div)?;
        }
        let mut declarations = vec![self.declaration(shared, false)?];
        while self.eat(Kw::Comma) {
            declarations.push(self.declaration(shared, false)?);
        }
        let span = self.span(keyword);
        self.out.push(Statement::Common { shared, block, declarations, span });
        Ok(())
    }

    pub(super) fn const_statement(&mut self) -> R<()> {
        self.i += 1;
        loop {
            let at = self.i;
            let mut name = if self.is(Kw::MOD) {
                self.i += 1;
                "MOD".to_owned()
            } else {
                let name = self.name_at(at).ok_or(at)?;
                self.i += 1;
                name
            };
            while matches!(self.tok_n(0), Some(Tok::Period)) {
                self.i += 1;
                let (field, _) = self.field_name()?;
                name.push('.');
                name.push_str(&field);
            }
            self.expect(Kw::EQ)?;
            let value = self.exp()?;
            let first = self.span(at);
            self.out.push(Statement::Const {
                name,
                span: Span { line: first.line, start: first.start, end: value.span().end },
                value,
            });
            if !self.eat(Kw::Comma) {
                return Ok(());
            }
        }
    }

    pub(super) fn def_type(
        &mut self,
        k: Kw,
    ) -> R<()> {
        let type_name = match k {
            Kw::DEFINT => TypeName::Integer,
            Kw::DEFLNG => TypeName::Long,
            Kw::DEFSNG => TypeName::Single,
            Kw::DEFDBL => TypeName::Double,
            _ => TypeName::String,
        };
        self.def_type_of(type_name)
    }

    pub(super) fn def_type_of(
        &mut self,
        type_name: TypeName,
    ) -> R<()> {
        let span = self.span(self.i);
        self.i += 1;
        let mut ranges = Vec::new();
        loop {
            let first = self.letter()?;
            let last = if self.eat(Kw::Minus) { self.letter()? } else { first };
            if first > last {
                return self.fail();
            }
            ranges.push((first, last));
            if !self.eat(Kw::Comma) {
                break;
            }
        }
        self.out.push(Statement::DefType { type_name, ranges, span });
        Ok(())
    }

    pub(super) fn letter(&mut self) -> R<char> {
        let (name, _) = self.identifier()?;
        if name.len() != 1 || !name.as_bytes()[0].is_ascii_alphabetic() {
            self.i -= 1;
            return self.fail();
        }
        Ok(name.as_bytes()[0].to_ascii_uppercase() as char)
    }

    pub(super) fn option(&mut self) -> R<()> {
        let first = self.take();
        if self.eat(Kw::BASE) {
            let base = match &self.tokens.get(self.i).map(|token| &token.kind) {
                Some(TokenKind::Integer(value @ (0 | 1), _)) => *value,
                _ => return self.fail(),
            };
            self.i += 1;
            self.out.push(Statement::OptionBase(base, self.span(first)));
            return Ok(());
        }
        if matches!(self.tok_n(0), Some(Tok::Ext)) {
            self.i += 1;
            self.out.push(Statement::OptionExplicit(self.span(first)));
            return Ok(());
        }
        self.fail()
    }

    pub(super) fn type_statement(&mut self) -> R<()> {
        let keyword = self.take();
        let at = self.i;
        let name = self.name_at(at).ok_or(at)?;
        self.i += 1;
        // `TYPE t: a AS INTEGER: END TYPE` on one line is BC's too
        if !self.eat(Kw::Colon) {
            self.expect(Kw::NewLine)?;
        }
        let mut fields = Vec::new();
        self.blocks.push(Open { what: "TYPE", closer: "END TYPE", span: self.span(keyword) });
        loop {
            self.skip_separators();
            if self.is(Kw::END) && self.is_n(1, Kw::TYPE) {
                self.i += 2;
                self.blocks.pop();
                break;
            }
            let at = self.i;
            let field = self.any_word_at(at).ok_or(at)?;
            self.i += 1;
            let mut bounds = Vec::new();
            // BC reads `a (5)` as a name and then something that is not AS
            if self.is(Kw::LParen) && self.tokens[self.i].span.start > self.tokens[at].span.end {
                return self.fail();
            }
            let array = self.eat(Kw::LParen);
            if array && !self.eat(Kw::RParen) {
                loop {
                    let first = self.exp()?;
                    bounds.push(if self.eat(Kw::TO) {
                        Bound { lower: Some(first), upper: self.exp()? }
                    } else {
                        Bound { lower: None, upper: first }
                    });
                    if self.eat(Kw::Comma) {
                        continue;
                    }
                    self.expect(Kw::RParen)?;
                    break;
                }
            }
            self.expect(Kw::AS)?;
            let type_name = self.declaration_type()?;
            let fixed_length =
                if type_name == TypeName::String && self.eat(Kw::Mult) { Some(self.exp()?) } else { None };
            fields.push(Declaration {
                name: field,
                type_name: Some(type_name),
                array,
                bounds,
                fixed_length,
                shared: false,
                dynamic: false,
                volatile: false,
                span: self.span(at),
            });
            if !(self.is(Kw::NewLine) || self.is(Kw::Colon)) {
                return self.fail();
            }
        }
        self.out.push(Statement::TypeDecl { name, fields, span: self.span(keyword) });
        Ok(())
    }

    pub(super) fn end(&mut self) -> R<()> {
        let keyword = self.take();
        match self.kw() {
            Some(k @ (Kw::SUB | Kw::FUNCTION | Kw::DEF)) => {
                self.i += 1;
                let kind = if k == Kw::SUB { ProcedureKind::Sub } else { ProcedureKind::Function };
                match self.open_procedure {
                    Some(index) if self.procedures[index].kind == kind => {
                        self.open_procedure = None;
                        Ok(())
                    }
                    _ => self.fail(),
                }
            }
            Some(Kw::IF | Kw::SELECT | Kw::TYPE) => self.fail(),
            _ => {
                let arguments = if self.dialect.exit_codes() { self.optional_exp()? } else { Vec::new() };
                self.runtime("END", arguments, self.span(keyword));
                Ok(())
            }
        }
    }

    // ---- procedures
    // -----------------------------------------------------------------------------------

    pub(super) fn parameter(&mut self) -> R<Parameter> {
        let by_value = self.eat(Kw::BYVAL);
        let segmented = self.eat(Kw::SEG);
        let (name, token) = self.identifier()?;
        let mut array = false;
        if self.eat(Kw::LParen) {
            array = true;
            if !self.is(Kw::RParen) {
                self.arguments_loose()?;
            } else {
                self.i += 1;
            }
        }
        let mut fixed_length = None;
        let type_name = if self.eat(Kw::AS) {
            let type_name = self.declaration_type()?;
            if type_name == TypeName::String && self.eat(Kw::Mult) {
                fixed_length = Some(self.exp()?);
            }
            Some(type_name)
        } else {
            name_of_suffix(&name)
        };
        Ok(Parameter {
            declaration: Declaration {
                name,
                type_name,
                array,
                bounds: Vec::new(),
                fixed_length,
                shared: false,
                dynamic: false,
                volatile: self.volatile_at.contains(&(token.line, token.start)),
                span: Span { line: token.line, start: token.start, end: self.previous_end() },
            },
            by_value,
            segmented,
        })
    }

    /// Bounds in parentheses after a parameter name, to the `)`: read and
    /// dropped.
    pub(super) fn arguments_loose(&mut self) -> R<()> {
        loop {
            self.exp()?;
            if self.eat(Kw::TO) {
                self.exp()?;
            }
            if self.eat(Kw::Comma) {
                continue;
            }
            return self.expect(Kw::RParen);
        }
    }

    pub(super) fn parameters(&mut self) -> R<Vec<Parameter>> {
        let mut parameters = Vec::new();
        if self.eat(Kw::LParen) && !self.eat(Kw::RParen) {
            loop {
                parameters.push(self.parameter()?);
                if self.eat(Kw::Comma) {
                    continue;
                }
                self.expect(Kw::RParen)?;
                break;
            }
        }
        Ok(parameters)
    }

    /// `SUB name …` or `FUNCTION name …`, a definition or, after DECLARE, a
    /// declaration.
    pub(super) fn procedure(
        &mut self,
        declaration: bool,
        _marker: Option<()>,
        is_static_prefix: bool,
    ) -> R<()> {
        let keyword = self.take();
        let kind = if self.kinds[keyword] == Tok::Kw(Kw::SUB) { ProcedureKind::Sub } else { ProcedureKind::Function };
        let span = self.span(keyword);
        let (mut name, _) = self.identifier()?;
        while matches!(self.tok_n(0), Some(Tok::Period)) {
            self.i += 1;
            name.push('.');
            name.push_str(&self.field_name()?.0);
        }
        let mut result = name_of_suffix(&name);
        if kind == ProcedureKind::Sub && result.is_some() {
            return self.fail();
        }
        let mut cdecl = false;
        let mut alias = None;
        if declaration {
            cdecl = self.eat(Kw::CDECL);
            if self.eat(Kw::ALIAS) {
                alias = match &self.tokens.get(self.i).map(|token| &token.kind) {
                    Some(TokenKind::String(text)) => Some(text.clone()),
                    _ => return self.fail(),
                };
                self.i += 1;
            }
        }
        let unchecked = declaration && !self.is(Kw::LParen);
        let empty_list = self.is(Kw::LParen) && self.is_n(1, Kw::RParen);
        let parameters = self.parameters()?;
        if kind == ProcedureKind::Function && self.eat(Kw::AS) {
            // a type character and an AS clause both name the result: refused;
            // and a record is no result
            if result.is_some() {
                return self.fail();
            }
            let given = self.declaration_type()?;
            if matches!(given, TypeName::Named(_)) && !self.dialect.record_results() {
                return self.fail();
            }
            result = Some(given);
        }
        let is_static = self.eat(Kw::STATIC) || is_static_prefix;
        self.procedures.push(Procedure {
            name,
            alias,
            cdecl,
            kind,
            parameters,
            result,
            body: Vec::new(),
            declaration,
            is_static,
            exported: true,
            private: false,
            module_scope: false,
            unchecked,
            empty_list,
            span,
        });
        if !declaration {
            if self.open_procedure.is_some() {
                return self.fail();
            }
            self.open_procedure = Some(self.procedures.len() - 1);
        }
        Ok(())
    }

    pub(super) fn declare(&mut self) -> R<()> {
        self.i += 1;
        if !matches!(self.kw(), Some(Kw::SUB | Kw::FUNCTION)) {
            return self.fail();
        }
        self.procedure(true, None, false)
    }

    pub(super) fn def(&mut self) -> R<()> {
        let keyword = self.take();
        let span = self.span(keyword);
        if self.eat(Kw::SEG) {
            let value = if self.eat(Kw::EQ) { Some(self.exp()?) } else { None };
            self.out.push(Statement::DefSeg { value, span });
            return Ok(());
        }
        let (name, _) = if matches!(self.tok_n(0), Some(Tok::FnWord)) {
            self.i += 1;
            let (name, at) = self.identifier()?;
            (format!("FN{name}"), at)
        } else {
            self.identifier()?
        };
        if !name.starts_with("FN") || self.open_procedure.is_some() {
            return self.fail();
        }
        let mut result = name_of_suffix(&name);
        let empty_list = self.is(Kw::LParen) && self.is_n(1, Kw::RParen);
        let parameters = self.parameters()?;
        if self.eat(Kw::AS) {
            result = Some(self.declaration_type()?);
        }
        let is_static = self.eat(Kw::STATIC);
        let definition = if self.eat(Kw::EQ) { Some(self.exp()?) } else { None };
        let inline = definition.is_some();
        let body = definition
            .map(|value| vec![Statement::Assign { target: Expr::Name(name.clone(), span), value, span }])
            .unwrap_or_default();
        self.procedures.push(Procedure {
            name,
            alias: None,
            cdecl: false,
            kind: ProcedureKind::Function,
            parameters,
            result,
            body,
            declaration: false,
            is_static,
            exported: !inline,
            private: false,
            module_scope: true,
            unchecked: false,
            empty_list,
            span,
        });
        if !inline {
            if self.open_procedure.is_some() {
                return self.fail();
            }
            self.open_procedure = Some(self.procedures.len() - 1);
        }
        Ok(())
    }
}
