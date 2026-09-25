//! `match` and the pattern language of draft section 6.

use super::*;

impl Parser {
    pub(super) fn match_statement(&mut self) -> Result<Statement, Diagnostic> {
        let span = self.bump().span;
        let subject = self.expression(0)?;
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' after the matched value",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before match arms",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected indented match arms",
        )?;
        let mut arms = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            let pattern = self.pattern()?;
            let arm_span = pattern.span();
            arms.push(MatchArm {
                pattern,
                body: self.suite()?,
                span: arm_span,
            });
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated match",
        )?;
        if arms.is_empty() {
            return Err(Diagnostic::new(span, "match must have at least one arm"));
        }
        Ok(Statement::Match {
            subject,
            arms,
            span,
        })
    }

    pub(super) fn pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Dot => {
                self.bump();
                let (name, name_span) = self.identifier("expected variant name after '.'")?;
                let fields = self.pattern_fields()?;
                Ok(Pattern::Variant {
                    enum_name: None,
                    name,
                    fields,
                    span: token.span.to(name_span.end_column),
                })
            }
            TokenKind::LeftParen => {
                let fields = self.pattern_fields()?;
                Ok(Pattern::Tuple(fields, token.span))
            }
            TokenKind::LeftBracket => self.sequence_pattern(),
            TokenKind::Identifier(name) if name == "_" => {
                self.bump();
                Ok(Pattern::Wildcard(token.span))
            }
            TokenKind::Identifier(name) => {
                self.bump();
                if self.take(|kind| matches!(kind, TokenKind::Dot)).is_some() {
                    // `Enum.variant`, or `module.Enum.variant`.
                    let (mut enum_name, mut variant, mut variant_span) = (
                        name,
                        self.identifier("expected variant name after '.'")?.0,
                        token.span,
                    );
                    while self.take(|kind| matches!(kind, TokenKind::Dot)).is_some() {
                        let (next, next_span) =
                            self.identifier("expected variant name after '.'")?;
                        enum_name = format!("{enum_name}.{variant}");
                        (variant, variant_span) = (next, next_span);
                    }
                    let variant_span = if variant_span == token.span {
                        self.tokens[self.at - 1].span
                    } else {
                        variant_span
                    };
                    let fields = self.pattern_fields()?;
                    return Ok(Pattern::Variant {
                        enum_name: Some(enum_name),
                        name: variant,
                        fields,
                        span: token.span.to(variant_span.end_column),
                    });
                }
                if matches!(self.peek().kind, TokenKind::LeftParen) {
                    let fields = self.pattern_fields()?;
                    return Ok(Pattern::Struct {
                        name,
                        fields,
                        span: token.span,
                    });
                }
                Ok(Pattern::Binding(name, token.span))
            }
            TokenKind::Integer(_)
            | TokenKind::Character(_)
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Minus => Ok(Pattern::Literal(self.expression(UNARY)?)),
            _ => Err(Diagnostic::new(token.span, "expected a pattern")),
        }
    }

    /// `[a, *rest, z]`, with at most one starred binding.
    fn sequence_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let span = self.bump().span;
        let (mut before, mut rest, mut after) = (Vec::new(), None, Vec::new());
        while self.take(|kind| matches!(kind, TokenKind::RightBracket)).is_none() {
            if let Some(star) = self.take(|kind| matches!(kind, TokenKind::Star)).map(|one| one.span) {
                if rest.is_some() {
                    return Err(Diagnostic::new(star, "a sequence pattern has at most one starred binding"));
                }
                let binding = self.pattern()?;
                if !matches!(binding, Pattern::Binding(..) | Pattern::Wildcard(_)) {
                    return Err(Diagnostic::new(binding.span(), "a starred pattern is a name or '_'"));
                }
                rest = Some(Box::new(binding));
            } else if rest.is_some() {
                after.push(self.pattern()?);
            } else {
                before.push(self.pattern()?);
            }
            if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                self.expect(|kind| matches!(kind, TokenKind::RightBracket), "expected ']' after patterns")?;
                break;
            }
        }
        Ok(Pattern::Sequence { before, rest, after, span })
    }

    /// `(pattern, ...)` after a variant or struct name; none when absent.
    fn pattern_fields(&mut self) -> Result<Vec<Pattern>, Diagnostic> {
        let mut fields = Vec::new();
        if self
            .take(|kind| matches!(kind, TokenKind::LeftParen))
            .is_none()
        {
            return Ok(fields);
        }
        if self
            .take(|kind| matches!(kind, TokenKind::RightParen))
            .is_some()
        {
            return Ok(fields);
        }
        loop {
            fields.push(self.pattern()?);
            if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                break;
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RightParen),
            "expected ')' after patterns",
        )?;
        Ok(fields)
    }
}
