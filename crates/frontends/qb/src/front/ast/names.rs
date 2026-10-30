//! Names: identifiers, fields, applied names and argument lists.

use super::*;

/// The parts of QuickrBASIC's `(start:end:step)` slice.
type SliceParts = (Option<Box<Expr>>, Option<Box<Expr>>, Option<Box<Expr>>);

impl Ast<'_> {
    pub(super) fn identifier(&mut self) -> R<(String, Span)> {
        match &self.tokens.get(self.i).map(|token| &token.kind) {
            Some(TokenKind::Identifier(name)) => {
                let out = (name.clone(), self.span(self.i));
                self.i += 1;
                Ok(out)
            }
            _ => self.fail(),
        }
    }

    /// The name a word spells where a name stands: an identifier, or a reserved
    /// word that is no structural keyword.
    pub(super) fn name_at(
        &self,
        at: usize,
    ) -> Option<String> {
        match &self.tokens.get(at)?.kind {
            TokenKind::Identifier(name) => Some(name.clone()),
            TokenKind::Reserved(word) => contextual_reserved_name(*word),
            _ => None,
        }
    }

    /// The name of a function word or a contextual one.
    pub(super) fn word_name(
        &self,
        at: usize,
    ) -> Option<String> {
        match &self.tokens.get(at)?.kind {
            TokenKind::Reserved(word) => {
                if word.is_function_word() {
                    Some(word.spelling().to_ascii_uppercase())
                } else {
                    contextual_reserved_name(*word)
                }
            }
            _ => self.name_at(at),
        }
    }

    /// A function word before a parenthesis: `None` where the word is no
    /// function of that shape.
    pub(super) fn function(
        &mut self,
        k: Kw,
        c: u16,
    ) -> R<Option<Expr>> {
        let at = self.i;
        let shaped = c & (FN1 | FN2 | FN23 | FN_BOUND | FN_REF) != 0
            || matches!(k, Kw::POINT | Kw::ERROR | Kw::INPUT_ | Kw::IOCTL_);
        if !shaped {
            return Ok(None);
        }
        let mut name = self.word_name(at).ok_or(at)?;
        if name.eq_ignore_ascii_case("VARPTR$") {
            name = "$VARPTR".to_owned();
        }
        self.i += 2;
        let mut arguments = Vec::new();
        if c & (FN_BOUND | FN_REF) != 0 {
            arguments.push(self.name_ref()?);
            if c & FN_BOUND != 0 && self.eat(Kw::Comma) {
                arguments.push(self.exp()?);
            }
        } else if matches!(k, Kw::IOCTL_) {
            self.eat(Kw::Lbs);
            arguments.push(self.exp()?);
        } else {
            arguments.push(self.exp()?);
            while self.eat(Kw::Comma) {
                if k == Kw::INPUT_ {
                    self.eat(Kw::Lbs);
                }
                arguments.push(self.exp()?);
            }
        }
        self.expect(Kw::RParen)?;
        let span = Span { line: self.span(at).line, start: self.span(at).start, end: self.previous_end() };
        Ok(Some(Expr::Apply { name, arguments, span }))
    }

    /// A field's name: any word, reserved or not (the checker refuses the ones
    /// the family refuses).
    pub(super) fn any_word_at(
        &self,
        at: usize,
    ) -> Option<String> {
        match &self.tokens.get(at)?.kind {
            TokenKind::Identifier(name) => Some(name.clone()),
            TokenKind::Reserved(word) => {
                let spelling = word.spelling().to_ascii_uppercase();
                spelling.as_bytes().first().is_some_and(u8::is_ascii_alphabetic).then_some(spelling)
            }
            TokenKind::ExtensionKeyword(word) => Some((*word).to_owned()),
            _ => None,
        }
    }

    pub(super) fn identifier_text(
        &self,
        at: usize,
    ) -> String {
        match &self.tokens[at].kind {
            TokenKind::Identifier(name) => name.clone(),
            _ => String::new(),
        }
    }

    pub(super) fn field_name(&mut self) -> R<(String, Span)> {
        let at = self.i;
        match self.tok_n(0) {
            Some(Tok::Ident) | Some(Tok::Kw(_)) | Some(Tok::Ext) => {
                let name = self.any_word_at(at).ok_or(at)?;
                self.i += 1;
                Ok((name, self.span(at)))
            }
            _ => self.fail(),
        }
    }

    /// The head of a NameRef, and whether it is already an applied form
    /// (`name(...)`).
    /// A DEF FN written without a parameter list takes no arguments: the
    /// parenthesis after it starts the next item (`PRINT FNa (1)`).
    pub(super) fn takes_no_arguments(
        &self,
        name: &str,
    ) -> bool {
        name.starts_with("FN")
            && self
                .procedures
                .iter()
                .any(|one| one.name == name && one.module_scope && one.parameters.is_empty() && !one.empty_list)
    }

    pub(super) fn name_base(&mut self) -> R<(Expr, bool)> {
        let at = self.i;
        let fn_word = matches!(self.tok_n(0), Some(Tok::FnWord));
        if fn_word || matches!(self.tok_n(0), Some(Tok::Ident)) {
            let (name, start) = if fn_word {
                // `FN twice`: the name is FNTWICE
                let fn_span = self.span(self.i);
                self.i += 1;
                let (name, ident) = self.identifier()?;
                (format!("FN{name}"), span_of(fn_span, ident))
            } else {
                self.identifier()?
            };
            // a DEF FN written without a parameter list takes no arguments: in
            // `PRINT FNa (1)` the parenthesis starts the next item
            if !self.takes_no_arguments(&name) && self.eat(Kw::LParen) {
                let base = Expr::Name(name.clone(), start);
                let arguments = match self.slice_or_arguments()? {
                    Ok(arguments) => arguments,
                    Err(parts) => {
                        let span = Span { line: start.line, start: start.start, end: self.previous_end() };
                        return Ok((self.slice(base, parts, span), true));
                    }
                };
                let span = Span { line: start.line, start: start.start, end: self.previous_end() };
                return Ok((Expr::Apply { name, arguments, span }, true));
            }
            return Ok((Expr::Name(name, start), false));
        }
        match self.tok_n(0) {
            Some(Tok::Kw(k)) => {
                let c = class(*k);
                if c & (OBJ_WORD | FN_WORD | CTX_WORD) != 0 {
                    let name = self.word_name(at).ok_or(at)?;
                    self.i += 1;
                    return Ok((Expr::Name(name, self.span(at)), false));
                }
                self.fail()
            }
            _ => self.fail(),
        }
    }

    /// An argument of a call: `BYVAL x` and `SEG x` keep their marks.
    pub(super) fn call_argument(&mut self) -> R<Expr> {
        let op = if self.eat(Kw::BYVAL) {
            Some(Unary::ByValue)
        } else if self.eat(Kw::SEG) {
            Some(Unary::Segmented)
        } else {
            None
        };
        let value = self.exp()?;
        Ok(match op {
            Some(op) => {
                let span = value.span();
                Expr::Unary { op, operand: Box::new(value), span }
            }
            None => value,
        })
    }

    /// A QuickrBASIC slice `base(start:end:step)`; the program then needs the
    /// prelude.
    fn slice(
        &mut self,
        base: Expr,
        (start, end, step): SliceParts,
        span: Span,
    ) -> Expr {
        self.prelude = true;
        Expr::Slice { base: Box::new(base), start, end, step, span }
    }

    /// After `(`: the arguments to the `)`, or under QuickrBASIC a slice, which
    /// has a colon where a list has a comma. Inside parentheses a colon cannot
    /// end a statement.
    fn slice_or_arguments(&mut self) -> R<Result<Vec<Expr>, SliceParts>> {
        if !self.dialect.python_expressions() {
            return self.arguments().map(Ok);
        }
        let part = |ast: &mut Self| -> R<Option<Box<Expr>>> {
            if ast.is(Kw::Colon) || ast.is(Kw::RParen) { Ok(None) } else { ast.exp().map(|one| Some(Box::new(one))) }
        };
        if self.eat(Kw::RParen) {
            return Ok(Ok(Vec::new()));
        }
        let first = part(self)?;
        if self.eat(Kw::Colon) {
            let end = part(self)?;
            let step = if self.eat(Kw::Colon) { part(self)? } else { None };
            self.expect(Kw::RParen)?;
            return Ok(Err((first, end, step)));
        }
        let Some(first) = first else { return self.fail() };
        let mut values = vec![*first];
        while self.eat(Kw::Comma) {
            values.push(self.exp()?);
        }
        self.expect(Kw::RParen)?;
        Ok(Ok(values))
    }

    /// After `(`: the arguments to the `)`.
    pub(super) fn arguments(&mut self) -> R<Vec<Expr>> {
        let mut values = Vec::new();
        if self.eat(Kw::RParen) {
            return Ok(values);
        }
        loop {
            values.push(self.exp()?);
            if self.eat(Kw::Comma) {
                continue;
            }
            self.expect(Kw::RParen)?;
            return Ok(values);
        }
    }

    /// A NameRef in an expression: every `.field` and `(args)` belongs to it.
    pub(super) fn name_ref(&mut self) -> R<Expr> {
        let (mut value, _) = self.name_base()?;
        loop {
            if matches!(self.tok_n(0), Some(Tok::Period)) {
                self.i += 1;
                let (name, field) = self.field_name()?;
                let span = span_of(value.span(), field);
                value = Expr::Field { base: Box::new(value), name, span };
            } else if self.is(Kw::LParen) && !matches!(&value, Expr::Name(name, _) if self.takes_no_arguments(name)) {
                self.i += 1;
                let indices = match self.slice_or_arguments()? {
                    Ok(indices) => indices,
                    Err(parts) => {
                        let span =
                            Span { line: value.span().line, start: value.span().start, end: self.previous_end() };
                        value = self.slice(value, parts, span);
                        continue;
                    }
                };
                let span = Span { line: value.span().line, start: value.span().start, end: self.previous_end() };
                value = match value {
                    Expr::Name(name, _) => Expr::Apply { name, arguments: indices, span },
                    base => Expr::Index { base: Box::new(base), indices, span },
                };
            } else {
                return Ok(value);
            }
        }
    }
}
