//! Statements that are one keyword and its operands, and calls.

use super::*;

impl Ast<'_> {
    pub(super) fn runtime(
        &mut self,
        name: &str,
        arguments: Vec<Expr>,
        span: Span,
    ) {
        self.out.push(Statement::Runtime { name: name.into(), arguments, span });
    }

    pub(super) fn implicit_call(
        &mut self,
        name: &str,
        arguments: Vec<Expr>,
        span: Span,
    ) {
        self.out.push(Statement::Call { name: name.into(), arguments, explicit: false, span });
    }

    /// `a, , b,`: an expression where the source gave one, Omitted at the comma
    /// that closes an empty slot.
    pub(super) fn argument_list(
        &mut self,
        trailing_ok: bool,
    ) -> R<Vec<Expr>> {
        let mut out = Vec::new();
        while !(self.ends_statement() || self.is(Kw::ELSE)) {
            if self.is(Kw::Comma) {
                let at = self.take();
                out.push(Expr::Omitted(self.span(at)));
                continue;
            }
            out.push(self.exp()?);
            if !self.eat(Kw::Comma) {
                break;
            }
            if self.ends_statement() || self.is(Kw::ELSE) {
                out.push(Expr::Omitted(self.previous()));
                break;
            }
        }
        if !trailing_ok && matches!(out.last(), Some(Expr::Omitted(_))) {
            return self.fail();
        }
        Ok(out)
    }

    pub(super) fn optional_exp(&mut self) -> R<Vec<Expr>> {
        if self.starts_exp() { Ok(vec![self.exp()?]) } else { Ok(Vec::new()) }
    }

    pub(super) fn starts_exp(&self) -> bool {
        match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str | Tok::Format | Tok::Ident | Tok::FnWord) => true,
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                c & (FN0 | FN_WORD | CTX_WORD) != 0
                    || (c & OBJ_WORD != 0 && self.period_n(1))
                    || (c & (FN1 | FN2 | FN23 | FN_BOUND | FN_REF) != 0 && self.is_n(1, Kw::LParen))
                    || matches!(
                        k,
                        Kw::ERROR | Kw::RND | Kw::LParen | Kw::Add | Kw::Minus | Kw::NOT
                    )
                    || (*k == Kw::MOD && self.period_n(1))
            }
            _ => false,
        }
    }

    pub(super) fn keyword_statement(
        &mut self,
        k: Kw,
    ) -> R<()> {
        let at = self.i;
        let span = self.span(at);
        let c = class(k);
        let assigned = c & (CTX_WORD | FN_WORD | FN0_VAR) != 0 || k == Kw::ERROR;
        // (TRON and TROFF swallow the rest of their line, `= 1` and all)
        if (assigned && self.is_n(1, Kw::EQ) && !matches!(k, Kw::TRON | Kw::TROFF))
            || (c & OBJ_WORD != 0 && self.period_n(1))
        {
            return self.name_statement();
        }
        match k {
            Kw::REM => {
                self.i += 1;
                self.out.push(Statement::Comment(span));
            }
            Kw::IF => return self.if_statement(),
            Kw::FOR => return self.for_statement(),
            Kw::WHILE => return self.while_statement(),
            Kw::DO => return self.do_statement(),
            Kw::SELECT => return self.select_statement(),
            Kw::DATA => return self.data_statement(),
            Kw::SUB | Kw::FUNCTION => return self.procedure(false, None, false),
            // a STATIC prefix on SUB and FUNCTION is VBDOS's
            Kw::STATIC if matches!(self.kw_n(1), Some(Kw::SUB | Kw::FUNCTION)) => {
                if !matches!(self.dialect, Dialect::VbDos | Dialect::Quickr) {
                    return self.fail();
                }
                self.i += 1;
                return self.procedure(false, None, true);
            }
            Kw::DECLARE if self.open_procedure.is_none() => return self.declare(),
            Kw::DECLARE | Kw::COMMON | Kw::OPTION if self.open_procedure.is_some() => return self.fail(),
            Kw::DEF => return self.def(),
            Kw::TYPE => return self.type_statement(),
            Kw::END => return self.end(),
            Kw::DIM | Kw::REDIM | Kw::STATIC | Kw::SHARED => return self.declaration_statement(k),
            // PDS and VBDOS: ERR = n sets the error code
            Kw::ERR if self.dialect_has_isam() && self.is_n(1, Kw::EQ) => {
                self.i += 2;
                let value = self.exp()?;
                let target = Expr::Name("ERR".into(), span);
                let whole = span_of(span, value.span());
                self.out.push(Statement::Assign { target, value, span: whole });
            }
            Kw::COMMON => return self.common(),
            Kw::CONST => return self.const_statement(),
            Kw::DEFINT | Kw::DEFLNG | Kw::DEFSNG | Kw::DEFDBL | Kw::DEFSTR => return self.def_type(k),
            Kw::OPTION => return self.option(),
            Kw::GOTO => {
                self.i += 1;
                let (label, _) = self.lab_ln()?;
                self.out.push(Statement::Goto(label, span));
            }
            Kw::GOSUB => {
                self.i += 1;
                let (label, label_span) = self.lab_ln()?;
                self.implicit_call("GOSUB", vec![Expr::Name(label, label_span)], span);
            }
            Kw::RETURN => {
                self.i += 1;
                let arguments = if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) {
                    let (label, label_span) = self.lab_ln()?;
                    vec![Expr::Name(label, label_span)]
                } else {
                    Vec::new()
                };
                self.implicit_call("RETURN", arguments, span);
            }
            Kw::RESTORE => {
                self.i += 1;
                let arguments = if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) {
                    let (label, label_span) = self.lab_ln()?;
                    vec![Expr::Name(label, label_span)]
                } else {
                    Vec::new()
                };
                self.runtime("RESTORE", arguments, span);
            }
            Kw::RESUME => {
                self.i += 1;
                let target = if self.eat(Kw::NEXT) {
                    ResumeTarget::Next
                } else if matches!(self.tok_n(0), Some(Tok::Ident | Tok::Int)) {
                    ResumeTarget::Label(self.lab_ln()?.0)
                } else {
                    ResumeTarget::Current
                };
                self.out.push(Statement::Resume { target, span });
            }
            Kw::EXIT => {
                self.i += 1;
                let target = match self.kw() {
                    Some(Kw::SUB) => ExitTarget::Sub,
                    Some(Kw::FUNCTION) => ExitTarget::Function,
                    Some(Kw::DO) => ExitTarget::Do,
                    Some(Kw::FOR) => ExitTarget::For,
                    _ => return self.fail(),
                };
                self.i += 1;
                self.out.push(Statement::Exit(target, span));
            }
            Kw::ON => return self.on(),
            Kw::CALL => {
                self.i += 1;
                let (mut name, _) = self.identifier()?;
                while matches!(self.tok_n(0), Some(Tok::Period)) {
                    self.i += 1;
                    name.push('.');
                    name.push_str(&self.field_name()?.0);
                }
                let mut arguments = Vec::new();
                if self.eat(Kw::LParen) && !self.eat(Kw::RParen) {
                    loop {
                        arguments.push(self.call_argument()?);
                        if self.eat(Kw::Comma) {
                            continue;
                        }
                        self.expect(Kw::RParen)?;
                        break;
                    }
                }
                self.out.push(Statement::Call { name, arguments, explicit: true, span });
            }
            Kw::LET => {
                self.i += 1;
                let start = self.i;
                let target = self.name_ref()?;
                self.expect(Kw::EQ)?;
                let value = self.exp()?;
                let begin = self.span(start);
                self.out.push(Statement::Assign { target, span: span_of(begin, value.span()), value });
            }
            Kw::MID_ => {
                self.i += 1;
                self.expect(Kw::LParen)?;
                let mut arguments = vec![self.name_ref()?];
                self.expect(Kw::Comma)?;
                arguments.push(self.exp()?);
                if self.eat(Kw::Comma) {
                    arguments.push(self.exp()?);
                }
                self.expect(Kw::RParen)?;
                self.expect(Kw::EQ)?;
                let value = self.exp()?;
                // The table-driven parser ends the target at the last `)` read,
                // the value's included.
                let right_parenthesis = self.kinds[..self.i]
                    .iter()
                    .rposition(|kind| *kind == Tok::Kw(Kw::RParen))
                    .map_or(value.span().end, |at| self.span(at).end);
                let target_span = Span { line: span.line, start: span.start, end: right_parenthesis };
                let assign_span = Span { line: span.line, start: span.start, end: value.span().end };
                self.out.push(Statement::Assign {
                    target: Expr::Apply { name: "MID$".into(), arguments, span: target_span },
                    span: assign_span,
                    value,
                });
            }
            Kw::LSET | Kw::RSET => {
                self.i += 1;
                let left = self.name_ref()?;
                self.expect(Kw::EQ)?;
                let right = self.exp()?;
                let comparison_span = span_of(left.span(), right.span());
                let name = if k == Kw::LSET { "LSET" } else { "RSET" };
                let comparison = Expr::Binary {
                    op: Binary::Eq,
                    left: Box::new(left),
                    right: Box::new(right),
                    span: comparison_span,
                };
                self.implicit_call(name, vec![comparison], span);
            }
            Kw::SWAP => {
                self.i += 1;
                let a = self.name_ref()?;
                self.expect(Kw::Comma)?;
                let b = self.name_ref()?;
                self.runtime("SWAP", vec![a, b], span);
            }
            Kw::READ => {
                self.i += 1;
                let mut destinations = vec![self.name_ref()?];
                while self.eat(Kw::Comma) {
                    destinations.push(self.name_ref()?);
                }
                self.out.push(Statement::Read { destinations, span });
            }
            Kw::ERASE => {
                self.i += 1;
                let mut names = vec![self.name_ref()?];
                while self.eat(Kw::Comma) {
                    names.push(self.name_ref()?);
                }
                self.out.push(Statement::Erase(names));
            }
            Kw::PRINT | Kw::QMark | Kw::LPRINT | Kw::WRITE => {
                return self.print(if k == Kw::QMark { Kw::PRINT } else { k });
            }
            Kw::INPUT => return self.input(),
            Kw::LINE => return self.line(),
            Kw::OPEN => return self.open(),
            Kw::CLOSE => {
                self.i += 1;
                let mut files = Vec::new();
                while !self.ends_statement() && !self.is(Kw::ELSE) {
                    self.eat(Kw::Lbs);
                    files.push(self.exp()?);
                    if !self.eat(Kw::Comma) {
                        break;
                    }
                }
                self.out.push(Statement::Close { files, span });
            }
            Kw::GET | Kw::PUT => return self.get_put(k),
            Kw::SEEK => {
                self.i += 1;
                self.eat(Kw::Lbs);
                let file = self.exp()?;
                self.expect(Kw::Comma)?;
                let position = self.exp()?;
                self.out.push(Statement::Seek { file, position, span });
            }
            Kw::DATE_ | Kw::TIME_ => {
                self.i += 1;
                self.expect(Kw::EQ)?;
                let value = self.exp()?;
                self.runtime(if k == Kw::DATE_ { "DATE$" } else { "TIME$" }, vec![value], span);
            }
            _ => return self.simple_statement(k),
        }
        Ok(())
    }
}
