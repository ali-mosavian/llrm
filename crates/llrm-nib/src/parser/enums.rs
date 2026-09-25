//! `enum Name[: backing]:` and its variants.

use super::*;

impl Parser {
    pub(super) fn enumeration(&mut self) -> Result<Enum, Diagnostic> {
        let span = self.bump().span;
        let (name, _) = self.identifier("expected enum name")?;
        let generics = self.generics()?;
        // `enum Name:` or, with a tag type, `enum Name: u8`.
        self.expect(
            |kind| matches!(kind, TokenKind::Colon),
            "expected ':' after enum name",
        )?;
        let backing = if matches!(self.peek().kind, TokenKind::Newline) {
            None
        } else {
            Some(self.tag_type()?)
        };
        self.expect(
            |kind| matches!(kind, TokenKind::Newline),
            "expected newline before variants",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Indent),
            "expected indented variants",
        )?;
        let mut variants = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Dedent | TokenKind::Eof) {
            variants.push(self.variant()?);
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Dedent),
            "unterminated enum",
        )?;
        if variants.is_empty() {
            return Err(Diagnostic::new(span, "enum must have at least one variant"));
        }
        Ok(Enum {
            name,
            generics,
            backing,
            variants,
            span,
        })
    }

    /// `u8`, `u16`, or a field width `uN`, stored in the smallest type holding it.
    fn tag_type(&mut self) -> Result<(TypeName, u32), Diagnostic> {
        let token = self.peek().clone();
        if let TokenKind::Identifier(name) = &token.kind {
            let bits = name
                .strip_prefix('u')
                .and_then(|one| one.parse::<u32>().ok());
            let Some(bits @ 1..=16) = bits else {
                return Err(Diagnostic::new(
                    token.span,
                    "an enum's tag type is u1 to u16",
                ));
            };
            self.bump();
            return Ok((
                if bits <= 8 {
                    TypeName::U8
                } else {
                    TypeName::U16
                },
                bits,
            ));
        }
        match self.type_name()? {
            TypeName::U8 => Ok((TypeName::U8, 8)),
            TypeName::U16 => Ok((TypeName::U16, 16)),
            _ => Err(Diagnostic::new(
                token.span,
                "an enum's tag type is u1 to u16",
            )),
        }
    }

    fn variant(&mut self) -> Result<Variant, Diagnostic> {
        let (name, span) = self.identifier("expected variant name")?;
        let mut fields = Vec::new();
        if self
            .take(|kind| matches!(kind, TokenKind::LeftParen))
            .is_some()
        {
            loop {
                let field_span = self.peek().span;
                let named = matches!(self.peek().kind, TokenKind::Identifier(_))
                    && self
                        .tokens
                        .get(self.at + 1)
                        .is_some_and(|one| matches!(one.kind, TokenKind::Colon));
                let field_name = if named {
                    let (field_name, _) = self.identifier("expected field name")?;
                    self.bump();
                    field_name
                } else {
                    format!("_{}", fields.len())
                };
                let (type_spec, dims) = self.field_type()?;
                fields.push(StructField {
                    name: field_name,
                    mutable: false,
                    type_spec,
                    dims,
                    span: field_span,
                });
                if self.take(|kind| matches!(kind, TokenKind::Comma)).is_none() {
                    break;
                }
            }
            self.expect(
                |kind| matches!(kind, TokenKind::RightParen),
                "expected ')' after variant fields",
            )?;
        }
        let tag = if self.take(|kind| matches!(kind, TokenKind::Equal)).is_some() {
            let token = self.bump().clone();
            let TokenKind::Integer(value) = token.kind else {
                return Err(Diagnostic::new(
                    token.span,
                    "a variant's tag must be an integer literal",
                ));
            };
            Some(value)
        } else {
            None
        };
        self.line_end()?;
        Ok(Variant {
            name,
            fields,
            tag,
            span,
        })
    }
}
