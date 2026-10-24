//! Expressions by BC's precedence.

use super::*;

impl Ast<'_> {
    pub(super) fn exp(&mut self) -> R<Expr> {
        self.imp(First::Full)
    }

    pub(super) fn binary(
        op: Binary,
        left: Expr,
        right: Expr,
    ) -> Expr {
        let span = span_of(left.span(), right.span());
        Expr::Binary { op, left: Box::new(left), right: Box::new(right), span }
    }

    pub(super) fn imp(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.eqv(f)?;
        while self.eat(Kw::IMP) {
            let r = self.eqv(First::Full)?;
            l = Self::binary(Binary::Imp, l, r);
        }
        Ok(l)
    }

    pub(super) fn eqv(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.xor(f)?;
        while self.eat(Kw::EQV) {
            let r = self.xor(First::Full)?;
            l = Self::binary(Binary::Eqv, l, r);
        }
        Ok(l)
    }

    pub(super) fn xor(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.or(f)?;
        while self.eat(Kw::XOR) {
            let r = self.or(First::Full)?;
            l = Self::binary(Binary::Xor, l, r);
        }
        Ok(l)
    }

    pub(super) fn or(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.and(f)?;
        while self.eat(Kw::OR) {
            let r = self.and(First::Full)?;
            l = Self::binary(Binary::Or, l, r);
        }
        Ok(l)
    }

    pub(super) fn and(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.not(f)?;
        while self.eat(Kw::AND) {
            let r = self.not(First::Full)?;
            l = Self::binary(Binary::And, l, r);
        }
        Ok(l)
    }

    pub(super) fn not(
        &mut self,
        f: First,
    ) -> R<Expr> {
        if f != First::Joined && self.injected.is_none() && self.is(Kw::NOT) {
            let at = self.take();
            let operand = self.not(First::Full)?;
            let span = span_of(self.span(at), operand.span());
            return Ok(Expr::Unary { op: Unary::Not, operand: Box::new(operand), span });
        }
        self.rel(f)
    }

    pub(super) fn rel_op(&self) -> Option<Binary> {
        match self.tok_n(0)? {
            Tok::Le => Some(Binary::LessEqual),
            Tok::Ne => Some(Binary::NotEqual),
            Tok::Ge => Some(Binary::GreaterEqual),
            Tok::Kw(Kw::EQ) => Some(Binary::Eq),
            Tok::Kw(Kw::LT) => Some(Binary::Less),
            Tok::Kw(Kw::GT) => Some(Binary::Greater),
            _ => None,
        }
    }

    pub(super) fn rel(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.add(f)?;
        while let Some(op) = self.rel_op() {
            self.i += 1;
            let r = self.add(First::Full)?;
            l = Self::binary(op, l, r);
        }
        Ok(l)
    }

    pub(super) fn add(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.modulo(f)?;
        loop {
            let op = match self.kw() {
                Some(Kw::Add) => Binary::Add,
                Some(Kw::Minus) => Binary::Subtract,
                _ => return Ok(l),
            };
            self.i += 1;
            let r = self.modulo(First::Full)?;
            l = Self::binary(op, l, r);
        }
    }

    pub(super) fn modulo(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.idiv(f)?;
        while self.eat(Kw::MOD) {
            let r = self.idiv(First::Full)?;
            l = Self::binary(Binary::Modulo, l, r);
        }
        Ok(l)
    }

    pub(super) fn idiv(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.mul(f)?;
        while self.eat(Kw::Idiv) {
            let r = self.mul(First::Full)?;
            l = Self::binary(Binary::IntegerDivide, l, r);
        }
        Ok(l)
    }

    pub(super) fn mul(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.neg(f)?;
        loop {
            let op = match self.kw() {
                Some(Kw::Mult) => Binary::Multiply,
                Some(Kw::Div) => Binary::Divide,
                _ => return Ok(l),
            };
            self.i += 1;
            let r = self.neg(First::Full)?;
            l = Self::binary(op, l, r);
        }
    }

    pub(super) fn sign(&self) -> Option<Unary> {
        match self.kw() {
            Some(Kw::Add) => Some(Unary::Positive),
            Some(Kw::Minus) => Some(Unary::Negative),
            _ => None,
        }
    }

    pub(super) fn neg(
        &mut self,
        f: First,
    ) -> R<Expr> {
        // `5 + NOT 5`: NOT may start the operand of a binary operator, and
        // takes the comparison and arithmetic that follows it
        if f == First::Full && self.injected.is_none() && self.is(Kw::NOT) {
            return self.not(First::Full);
        }
        if f != First::Joined && self.injected.is_none() {
            if let Some(op) = self.sign() {
                let at = self.take();
                let operand = self.neg(First::Full)?;
                let span = span_of(self.span(at), operand.span());
                return Ok(Expr::Unary { op, operand: Box::new(operand), span });
            }
        }
        self.pow(f)
    }

    pub(super) fn pow(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let mut l = self.prim(f)?;
        while self.eat(Kw::Pwr) {
            let r = self.pow_operand()?;
            l = Self::binary(Binary::Power, l, r);
        }
        Ok(l)
    }

    pub(super) fn pow_operand(&mut self) -> R<Expr> {
        if let Some(op) = self.sign() {
            let at = self.take();
            let operand = self.pow_operand()?;
            let span = span_of(self.span(at), operand.span());
            return Ok(Expr::Unary { op, operand: Box::new(operand), span });
        }
        self.prim(First::Full)
    }

    pub(super) fn prim(
        &mut self,
        f: First,
    ) -> R<Expr> {
        if let Some(expr) = self.injected.take() {
            return Ok(expr);
        }
        if self.is(Kw::LParen) {
            if !matches!(f, First::Full | First::NoErr) {
                return self.fail();
            }
            let open = self.take();
            let value = self.exp()?;
            self.expect(Kw::RParen)?;
            // Grouping only changes meaning around a reference.
            return Ok(match value {
                Expr::Name(..) | Expr::Apply { .. } | Expr::Index { .. } | Expr::Field { .. } => Expr::Unary {
                    op: Unary::Grouped,
                    span: Span { line: self.span(open).line, start: self.span(open).start, end: self.previous_end() },
                    operand: Box::new(value),
                },
                value => value,
            });
        }
        self.prim_base(f)
    }

    pub(super) fn literal(&mut self) -> R<Expr> {
        let token = &self.tokens[self.i];
        let span = token.span;
        let expr = match &token.kind {
            TokenKind::Integer(value, suffix) => Expr::Literal(
                Literal::Integer(*value, if *suffix == Some('&') { TypeName::Long } else { TypeName::Integer }),
                span,
            ),
            TokenKind::Real(value, suffix) => Expr::Literal(Literal::Real(value.clone(), real_type(*suffix)), span),
            TokenKind::String(value) => Expr::Literal(Literal::String(value.clone()), span),
            _ => return self.fail(),
        };
        self.i += 1;
        Ok(expr)
    }

    pub(super) fn prim_base(
        &mut self,
        f: First,
    ) -> R<Expr> {
        let k = match self.tok_n(0) {
            Some(Tok::Int | Tok::Real | Tok::Str) => return self.literal(),
            Some(Tok::Ident | Tok::FnWord) => return self.name_ref(),
            Some(Tok::Kw(k)) => *k,
            _ => return self.fail(),
        };
        let c = class(k);
        if c & OBJ_WORD != 0 && self.period_n(1) {
            return self.name_ref();
        }
        if k == Kw::MOD && self.period_n(1) {
            if !matches!(f, First::Full) {
                return self.fail();
            }
            let at = self.take();
            let base = Expr::Name("MOD".into(), self.span(at));
            self.i += 1;
            let (name, field_span) = self.field_name()?;
            let span = span_of(base.span(), field_span);
            return Ok(Expr::Field { base: Box::new(base), name, span });
        }
        if k == Kw::RND {
            let at = self.take();
            let name = "RND".to_owned();
            if self.eat(Kw::LParen) {
                let argument = self.exp()?;
                self.expect(Kw::RParen)?;
                let span = Span { line: self.span(at).line, start: self.span(at).start, end: self.previous_end() };
                return Ok(Expr::Apply { name, arguments: vec![argument], span });
            }
            return Ok(Expr::Name(name, self.span(at)));
        }
        if self.is_n(1, Kw::LParen) {
            if let Some(expr) = self.function(k, c)? {
                return Ok(expr);
            }
        }
        if c & FN0 != 0 || c & (FN_WORD | CTX_WORD) != 0 {
            let at = self.take();
            let name = self.word_name(at).ok_or(at)?;
            return Ok(Expr::Name(name, self.span(at)));
        }
        if k == Kw::ERROR && f != First::NoErr {
            let at = self.take();
            return Ok(Expr::Name("ERROR".into(), self.span(at)));
        }
        self.fail()
    }
}
