//! print and f-string formatting.

use crate::abi::nib as rt;
use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn print(
        &mut self,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Void) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Void,
            ));
        }
        let last_call = arguments.iter().rposition(|one| calls(one));
        let mut settled = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let before_call = last_call.is_some_and(|last| index < last);
            settled.push(match argument {
                Expr::FString { parts, span } => Expr::FString { parts: self.settled_parts(parts, before_call)?, span: *span },
                _ => self.settled(argument, before_call)?,
            });
        }
        for argument in &settled {
            if let Expr::FString { parts, .. } = argument {
                // As a direct argument, an f-string streams its pieces and allocates nothing.
                self.print_parts(parts, argument.span())?;
            } else {
                self.print_value(argument, Format::default())?;
            }
        }
        self.emit_builtin(rt::PRINT_NEWLINE, Vec::new());
        Ok(TypedOperand {
            operand: None,
            type_name: TypeName::Void,
        })
    }

    pub(super) fn print_parts(&mut self, parts: &[FStringPart], span: Span) -> Result<(), Diagnostic> {
        for part in parts {
            match part {
                FStringPart::Text(bytes) if !bytes.is_empty() => {
                    let text = self.string_literal(bytes, Some(TypeName::String), span)?;
                    self.emit_print(TypeName::String, required(text, span)?);
                }
                FStringPart::Text(_) => {}
                FStringPart::Value(expression, format) => self.print_value(expression, *format)?,
            }
        }
        Ok(())
    }

    /// `parts` with each value computed first, as `settled` leaves it;
    /// `before_call` when a call follows them all.
    pub(super) fn settled_parts(&mut self, parts: &[FStringPart], before_call: bool) -> Result<Vec<FStringPart>, Diagnostic> {
        let value = |part: &FStringPart| match part {
            FStringPart::Value(expression, _) => Some(expression.clone()),
            FStringPart::Text(_) => None,
        };
        let last_call = parts.iter().rposition(|part| value(part).is_some_and(|one| calls(&one)));
        parts
            .iter()
            .enumerate()
            .map(|(index, part)| match part {
                FStringPart::Value(expression, format) => {
                    let early = before_call || last_call.is_some_and(|last| index < last);
                    Ok(FStringPart::Value(self.settled(expression, early)?, *format))
                }
                text => Ok(text.clone()),
            })
            .collect()
    }

    /// What prints for `expression` (section 13): what its type's `display`
    /// method returns, when it has one. Every value is computed, left to
    /// right, before any is formatted, since a call may print, build an
    /// f-string, or change what an earlier value reads. So a value that
    /// calls, or precedes one that does (`early`), is bound to a hidden
    /// local first: a copy of a scalar, a borrow of any other place.
    fn settled(&mut self, expression: &Expr, early: bool) -> Result<Expr, Diagnostic> {
        let span = expression.span();
        let shown = Expr::MethodCall {
            receiver: Box::new(expression.clone()),
            name: "display".into(),
            type_arguments: Vec::new(),
            arguments: Vec::new(),
            span,
        };
        let shown = self.method_as_call(&shown).unwrap_or_else(|| expression.clone());
        if !(early || calls(&shown)) || is_literal(&shown) {
            return Ok(shown);
        }
        let place = matches!(shown, Expr::Name(..) | Expr::Member { .. } | Expr::Index { .. } | Expr::Slice { .. });
        let owning = self.expression_type_hint(&shown).is_some_and(ownership::needs_drop)
            || matches!(self.struct_expression_type(&shown, span), Ok(Some(_)));
        let value = if place && owning {
            Expr::Borrow { mutable: false, operand: Box::new(shown), span }
        } else {
            shown
        };
        let name = self.hidden("shown");
        self.statement(&Statement::Bind { mutable: false, name: name.clone(), annotation: None, value, span })?;
        Ok(Expr::Name(name, span))
    }

    /// Formats one value into `format`'s field.
    pub(super) fn print_value(&mut self, expression: &Expr, format: Format) -> Result<(), Diagnostic> {
        if let Some((descriptor, element, rank)) = self.view_of(expression)? {
            if (element, rank) != (ElementType::Scalar(TypeName::Char), 1) {
                return Err(Diagnostic::new(expression.span(), "only a &string view prints"));
            }
            self.field(format, TypeName::String, expression.span())?;
            self.emit_builtin(rt::PRINT_VIEW, vec![hir::Operand::Value(descriptor)]);
            return Ok(());
        }
        let value = self.expression(expression, None)?;
        if value.type_name == TypeName::Void {
            return Err(Diagnostic::new(expression.span(), "cannot format void"));
        }
        let type_name = value.type_name;
        self.field(format, type_name, expression.span())?;
        self.emit_print(type_name, required(value, expression.span())?);
        Ok(())
    }

    /// Sets the field the next formatted value fills, unless it is the default.
    pub(super) fn field(&mut self, format: Format, type_name: TypeName, span: Span) -> Result<(), Diagnostic> {
        if format == Format::default() {
            return Ok(());
        }
        if format.radix != 10 && !is_integer(type_name) {
            return Err(Diagnostic::new(
                span,
                format!("only an integer is formatted in base {}", format.radix),
            ));
        }
        let fill = if format.zero { b'0' } else { b' ' };
        let operands = [format.width, format.radix, fill, u8::from(format.left)]
            .map(|one| hir::Operand::Constant(U8, i64::from(one)));
        self.emit_builtin(rt::PRINT_FIELD, operands.to_vec());
        Ok(())
    }

    pub(super) fn emit_print(&mut self, type_name: TypeName, operand: hir::Operand) {
        if let TypeName::Fixed {
            storage, fraction, ..
        } = type_name
        {
            let storage_type = match storage {
                FixedStorage::I16 => TypeName::I16,
                FixedStorage::I32 => TypeName::I32,
            };
            let raw = self.value(storage_type);
            self.emit("convert", vec![raw], vec![operand], None);
            self.emit_builtin(
                print_name(type_name),
                vec![
                    hir::Operand::Value(raw),
                    hir::Operand::Constant(U8, i64::from(fraction)),
                ],
            );
            return;
        }
        if let TypeName::Enum { width, .. } = type_name {
            let tag = if width == 1 {
                TypeName::U8
            } else {
                TypeName::U16
            };
            let raw = self.value(tag);
            self.emit("convert", vec![raw], vec![operand], None);
            self.emit_builtin(print_name(tag), vec![hir::Operand::Value(raw)]);
            return;
        }
        self.emit_builtin(print_name(type_name), vec![operand]);
    }
}

/// Whether evaluating `expression` calls a function.
fn calls(expression: &Expr) -> bool {
    let mut found = false;
    let Ok(()) = expression.clone().walk_mut(&mut |one| -> Result<(), std::convert::Infallible> {
        found |= matches!(one, Expr::Call { .. } | Expr::MethodCall { .. });
        Ok(())
    });
    found
}
