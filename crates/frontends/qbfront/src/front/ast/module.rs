//! The module: its lines and the statements that start a line.

use super::*;

impl Ast<'_> {
    pub(super) fn module(&mut self) -> R<Module> {
        while self.i < self.tokens.len() {
            while self.eat(Kw::NewLine) || self.eat(Kw::Colon) {}
            if self.i >= self.tokens.len() {
                break;
            }
            let before = self.out.len();
            let procedures_before = self.procedures.len();
            let open_before = self.open_procedure;
            self.statement()?;
            self.route(before, procedures_before, open_before);
            if !self.ends_statement() {
                return self.fail();
            }
            self.colon_pending = false;
        }
        if self.open_procedure.is_some() {
            return self.fail();
        }
        Ok(Module {
            statements: std::mem::take(&mut self.out),
            procedures: std::mem::take(&mut self.procedures),
            prelude: self.prelude || self.tokens.iter().any(|token| matches!(token.kind, TokenKind::FormatString(_))),
        })
    }

    /// A statement read while a procedure is open belongs to its body.
    pub(super) fn route(
        &mut self,
        before: usize,
        procedures_before: usize,
        open_before: Option<usize>,
    ) {
        if let Some(index) = open_before {
            if self.open_procedure == Some(index)
                && self.procedures.len() == procedures_before
                && self.out.len() > before
            {
                let body = self.out.split_off(before);
                self.procedures[index].body.extend(body);
            }
        }
    }

    pub(super) fn ends_statement(&self) -> bool {
        self.colon_pending || self.i >= self.tokens.len() || self.is(Kw::NewLine) || self.is(Kw::Colon)
    }

    /// `:` between two statements, or the one a line number stands for.
    pub(super) fn separator(&mut self) -> bool {
        if self.colon_pending {
            self.colon_pending = false;
            true
        } else {
            self.eat(Kw::Colon)
        }
    }

    pub(super) fn skip_separators(&mut self) {
        while self.eat(Kw::NewLine) || self.separator() {}
    }

    pub(super) fn statement(&mut self) -> R<()> {
        let at = self.i;
        let private = self.private_at.contains(&at);
        self.statement_inner()?;
        if private {
            match self.procedures.last_mut() {
                Some(procedure) if !procedure.declaration => procedure.private = true,
                _ => return Err(at),
            }
        }
        Ok(())
    }

    fn statement_inner(&mut self) -> R<()> {
        let at = self.i;
        match &self.tokens[at].kind {
            TokenKind::Integer(value, None) => {
                if !(0..=65_529).contains(value)
                    || (at > 0 && matches!(self.tokens[at - 1].kind, TokenKind::Integer(_, None)))
                {
                    return self.fail();
                }
                // `1a:` is a single label, a line number run into a name
                if let (Some(TokenKind::Identifier(name)), Some(Tok::Kw(Kw::Colon))) =
                    (self.tokens.get(at + 1).map(|t| &t.kind), self.kinds.get(at + 2))
                {
                    if self.tokens[at + 1].span.start == self.tokens[at].span.end && without_type_character(name) {
                        self.out.push(Statement::Label(format!("{value}{name}"), self.span(at)));
                        self.i += 2;
                        return Ok(());
                    }
                }
                self.out.push(Statement::Label(value.to_string(), self.span(at)));
                self.i += 1;
                if !(self.is(Kw::NewLine) || self.is(Kw::Colon)) {
                    self.colon_pending = true;
                }
                return Ok(());
            }
            // BC reads `1.5` at the start of a line as a line number of its own
            TokenKind::Real(text, None) if !text.contains('E') && self.line_start() && !self.is_prev_integer() => {
                self.out.push(Statement::Label(text.clone(), self.span(at)));
                self.i += 1;
                if !(self.is(Kw::NewLine) || self.is(Kw::Colon)) {
                    self.colon_pending = true;
                }
                return Ok(());
            }
            TokenKind::FormDeclaration(name) => {
                self.out.push(Statement::FormDeclaration(name.clone(), self.span(at)));
                self.i += 1;
                return Ok(());
            }
            TokenKind::ArrayDynamic | TokenKind::ArrayStatic => {
                self.dynamic_arrays = matches!(self.tokens[at].kind, TokenKind::ArrayDynamic);
                self.out.push(Statement::Comment(self.span(at)));
                self.i += 1;
                return Ok(());
            }
            _ => {}
        }
        match self.kinds[at].clone() {
            Tok::Ident | Tok::FnWord => self.name_statement(),
            Tok::Kw(k) => self.keyword_statement(k),
            Tok::Ext => self.extension_statement(),
            _ => self.fail(),
        }
    }

    /// `DEFBYTE` and the `DEFU*` statements: extension words before letter
    /// ranges.
    pub(super) fn extension_statement(&mut self) -> R<()> {
        let TokenKind::ExtensionKeyword(word) = self.tokens[self.i].kind else { return self.fail() };
        let (width, signed) = match word {
            "DEFBYTE" => (1, true),
            "DEFUBYTE" => (1, false),
            "DEFUINT" => (2, false),
            "DEFULNG" => (4, false),
            _ => return self.fail(),
        };
        self.def_type_of(TypeName::Integral { width, signed })
    }

    pub(super) fn line_start(&self) -> bool {
        // after a line number, a label may follow (`1a:`)
        self.i == 0
            || self.is_prev_newline()
            || (self.i > 0 && matches!(
                self.tokens.get(self.i - 1).map(|t| &t.kind),
                Some(TokenKind::Integer(_, None))
            ))
    }

    fn is_prev_integer(&self) -> bool {
        self.i > 0 && matches!(self.tokens[self.i - 1].kind, TokenKind::Integer(_, None))
    }

    pub(super) fn is_prev_newline(&self) -> bool {
        matches!(
            self.kinds.get(self.i.wrapping_sub(1)),
            Some(Tok::Kw(Kw::NewLine))
        )
    }

    /// A statement that begins with a name: a label, an assignment, or a call.
    pub(super) fn name_statement(&mut self) -> R<()> {
        let start = self.i;
        let start_span = self.span(start);
        if let Some(done) = self.isam_statement()? {
            return Ok(done);
        }
        if matches!(self.kinds[start], Tok::Ident)
            && self.line_start()
            && without_type_character(&self.identifier_text(start))
        {
            // a label: `a:` or the dotted `a.b:`
            let mut end = start + 1;
            while matches!(self.kinds.get(end), Some(Tok::Period))
                && matches!(self.kinds.get(end + 1), Some(Tok::Ident))
            {
                end += 2;
            }
            if self.kinds.get(end) == Some(&Tok::Kw(Kw::Colon)) {
                let mut name = self.identifier()?.0;
                while matches!(self.tok_n(0), Some(Tok::Period)) {
                    self.i += 1;
                    name.push('.');
                    name.push_str(&self.field_name()?.0);
                }
                self.out.push(Statement::Label(name, start_span));
                return Ok(());
            }
        }
        if let Ok(target) = self.name_ref() {
            if self.eat(Kw::EQ) {
                let value = self.exp().map_err(|_| self.i)?;
                let span = span_of(start_span, value.span());
                self.out.push(Statement::Assign { target, value, span });
                return Ok(());
            }
            // `list(i).Print "x"`: a method of an element
            if let Expr::Field { base, name, .. } = &target {
                if dotted_name(base).is_none() {
                    let arguments = self.call_arguments()?;
                    self.out.push(Statement::MethodCall {
                        object: (**base).clone(),
                        method: name.clone(),
                        arguments,
                        span: start_span,
                    });
                    return Ok(());
                }
            }
            // `Graphics.Init` and `obj.Method a, b`: a call by a dotted name
            if let Some(name) = dotted_name(&target) {
                let arguments = self.call_arguments()?;
                self.out.push(Statement::Call { name, arguments, explicit: false, span: start_span });
                return Ok(());
            }
        }
        self.i = start;
        // `a.b.Method (x) \ 2, y`: a dotted name, then arguments that may open
        // with a parenthesis
        if matches!(self.tok_n(1), Some(Tok::Period)) {
            let (mut name, _) = self.identifier()?;
            while matches!(self.tok_n(0), Some(Tok::Period)) {
                self.i += 1;
                name.push('.');
                name.push_str(&self.field_name()?.0);
            }
            let arguments = self.call_arguments()?;
            self.out.push(Statement::Call { name, arguments, explicit: false, span: start_span });
            return Ok(());
        }
        let (name, span) = match self.kinds[start] {
            Tok::FnWord => return self.fail(),
            _ => self.identifier()?,
        };
        let arguments = self.call_arguments()?;
        self.out.push(Statement::Call { name, arguments, explicit: false, span });
        Ok(())
    }

    /// PDS and VBDOS have the ISAM file statements.
    pub(super) fn dialect_has_isam(&self) -> bool {
        matches!(self.dialect, Dialect::Pds71 | Dialect::VbDos)
    }

    /// An ISAM statement (`MOVEFIRST #1`, `SEEKEQ #t, key`, `CHECKPOINT`...),
    /// if the line starts with one.
    pub(super) fn isam_statement(&mut self) -> R<Option<()>> {
        if !matches!(
            self.dialect,
            Dialect::Pds71 | Dialect::VbDos | Dialect::Quickr
        )
            || !matches!(self.kinds[self.i], Tok::Ident)
        {
            return Ok(None);
        }
        let TokenKind::Identifier(word) = self.tokens[self.i].kind.clone() else { return Ok(None) };
        match word.as_str() {
            "CHDRIVE" | "STACK" => {
                let span = self.span(self.i);
                self.i += 1;
                let arguments = if self.starts_exp() { vec![self.exp()?] } else { Vec::new() };
                self.runtime(&word, arguments, span);
                return Ok(Some(()));
            }
            "EVENT" => {
                let span = self.span(self.i);
                self.i += 1;
                let at = self.i;
                let verb = match self.kw() {
                    Some(Kw::ON) => "ON",
                    Some(Kw::OFF) => "OFF",
                    _ => return self.fail(),
                };
                self.i += 1;
                self.runtime("EVENT_SWITCH", vec![Expr::Name(verb.into(), self.span(at))], span);
                return Ok(Some(()));
            }
            "LOAD" | "UNLOAD" if matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) => {
                let span = self.span(self.i);
                self.i += 1;
                let form = self.name_ref()?;
                self.runtime(&word, vec![form], span);
                return Ok(Some(()));
            }
            "DEFCUR" => {
                self.def_type_of(TypeName::Currency)?;
                return Ok(Some(()));
            }
            _ => {}
        }
        let name = match word.as_str() {
            "BEGINTRANS" | "COMMITTRANS" | "CHECKPOINT" | "ROLLBACK" | "CREATEINDEX" | "DELETE" | "DELETEINDEX"
            | "DELETETABLE" | "INSERT" | "RETRIEVE" | "UPDATE" | "MOVEFIRST" | "MOVELAST" | "MOVENEXT"
            | "MOVEPREVIOUS" | "SEEKEQ" | "SEEKGE" | "SEEKGT" | "SETINDEX" => word,
            _ => return Ok(None),
        };
        let span = self.span(self.i);
        self.i += 1;
        let mut arguments = Vec::new();
        match name.as_str() {
            "BEGINTRANS" | "COMMITTRANS" | "CHECKPOINT" => {}
            "ROLLBACK" => {
                // ROLLBACK ALL undoes every open transaction
                if matches!(
                    self.tokens.get(self.i).map(|t| &t.kind),
                    Some(TokenKind::Identifier(word)) if word == "ALL"
                ) {
                    self.i += 1;
                    self.runtime("ROLLBACK_ALL", Vec::new(), span);
                    return Ok(Some(()));
                }
                if self.starts_exp() {
                    arguments.push(self.exp()?);
                }
            }
            "DELETETABLE" => {
                arguments.push(self.exp()?);
                self.expect(Kw::Comma)?;
                arguments.push(self.exp()?);
            }
            _ => {
                // [#]table, then the statement's own list
                self.eat(Kw::Lbs);
                arguments.push(self.exp()?);
                let list_required = !matches!(
                    name.as_str(),
                    "DELETE" | "MOVEFIRST" | "MOVELAST" | "MOVENEXT" | "MOVEPREVIOUS" | "SETINDEX"
                );
                if list_required || self.is(Kw::Comma) {
                    self.expect(Kw::Comma)?;
                    if matches!(name.as_str(), "INSERT" | "RETRIEVE" | "UPDATE") {
                        arguments.push(self.name_ref()?);
                    } else {
                        loop {
                            arguments.push(self.exp()?);
                            if !self.eat(Kw::Comma) {
                                break;
                            }
                        }
                    }
                }
            }
        }
        self.runtime(&name, arguments, span);
        Ok(Some(()))
    }

    pub(super) fn call_arguments(&mut self) -> R<Vec<Expr>> {
        let mut arguments = Vec::new();
        while !(self.ends_statement() || self.is(Kw::ELSE)) {
            arguments.push(self.call_argument()?);
            if !self.eat(Kw::Comma) {
                break;
            }
        }
        Ok(arguments)
    }
}
