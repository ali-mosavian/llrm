//! `Option`, `Result`, and `?` (draft section 10).

use super::enums::EnumLayout;
use super::enums::VariantLayout;
use super::*;

/// The success and failure variants of an `Option` or `Result` instance.
fn outcome(layout: &EnumLayout) -> Option<(VariantLayout, VariantLayout)> {
    let names: Vec<&str> = layout
        .variants
        .iter()
        .map(|one| one.name.as_str())
        .collect();
    matches!(names.as_slice(), ["some", "none"] | ["ok", "err"])
        .then(|| (layout.variants[0].clone(), layout.variants[1].clone()))
}

impl FunctionCompiler<'_> {
    /// `operand?` as a scalar: the success payload, or `void` when there is none.
    /// `value` bound to a hidden local, when it holds a `?`: a statement
    /// makes the place it writes, such as a pushed element or a dict entry,
    /// only after its value can no longer return early.
    pub(super) fn settled_failure(&mut self, value: &Expr, span: Span) -> Result<Option<Expr>, Diagnostic> {
        let mut propagates = false;
        let Ok(()) = value.clone().walk_mut(&mut |one| -> Result<(), std::convert::Infallible> {
            propagates |= matches!(one, Expr::Try { .. });
            Ok(())
        });
        if !propagates {
            return Ok(None);
        }
        let name = self.hidden("settled");
        self.statement(&Statement::Bind { mutable: false, name: name.clone(), annotation: None, value: value.clone(), span })?;
        Ok(Some(Expr::Name(name, span)))
    }

    pub(super) fn try_value(
        &mut self,
        operand: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let (view, payload) = self.unwrap(operand, span)?;
        let Some(field) = payload else {
            return Ok(TypedOperand {
                operand: None,
                type_name: TypeName::Void,
            });
        };
        let ElementType::Scalar(type_name) = field.type_ else {
            return Err(Diagnostic::new(
                span,
                "this '?' yields a struct; bind it with 'let'",
            ));
        };
        if expected.is_some_and(|one| one != type_name) {
            return Err(type_mismatch(span, expected.expect("checked"), type_name));
        }
        let value = self.value(type_name);
        let place = self.projected_place(&view, field.offset, type_name);
        self.emit("load", vec![value], vec![place], None);
        // The payload leaves the outcome, which nothing drops: it is this
        // statement's to move or drop.
        Ok(TypedOperand {
            operand: Some(self.temporary_owned(hir::Operand::Value(value), type_name)),
            type_name,
        })
    }

    /// `operand?` as an aggregate: where its success payload is.
    pub(super) fn try_view(
        &mut self,
        operand: &Expr,
        span: Span,
    ) -> Result<StructView, Diagnostic> {
        let (view, payload) = self.unwrap(operand, span)?;
        match payload.map(|one| (one.type_, one.offset)) {
            Some((ElementType::Struct(struct_id), offset)) => Ok(StructView {
                struct_id,
                offset: view.offset + offset,
                ..view
            }),
            _ => Err(Diagnostic::new(span, "this '?' yields no struct")),
        }
    }

    /// The payload type a `?` yields, when it is an aggregate.
    pub(super) fn try_struct_type(
        &self,
        operand: &Expr,
        span: Span,
    ) -> Result<Option<u32>, Diagnostic> {
        let Some(struct_id) = self.struct_expression_type(operand, span)? else {
            return Ok(None);
        };
        let Some((success, _)) = self
            .types
            .enum_of(ElementType::Struct(struct_id))
            .and_then(outcome)
        else {
            return Ok(None);
        };
        Ok(match success.fields.first().map(|(_, one)| one.type_) {
            Some(ElementType::Struct(id)) => Some(id),
            _ => None,
        })
    }

    /// Evaluates `operand`, returns its failure from this function, and
    /// continues with its storage and success payload field.
    fn unwrap(
        &mut self,
        operand: &Expr,
        span: Span,
    ) -> Result<(StructView, Option<FieldLayout>), Diagnostic> {
        if let Some(call) = self.method_as_call(operand) {
            return self.unwrap(&call, span);
        }
        let struct_id = self
            .struct_expression_type(operand, span)?
            .ok_or_else(|| Diagnostic::new(span, "'?' requires an Option or Result"))?;
        let layout = self.types.enum_of(ElementType::Struct(struct_id)).cloned();
        let Some((success, failure)) = layout.as_ref().and_then(outcome) else {
            return Err(Diagnostic::new(span, "'?' requires an Option or Result"));
        };
        let layout = layout.expect("checked");
        let view = match operand {
            Expr::Call {
                name,
                arguments,
                span,
                ..
            } => self.call_into(name, arguments, *span)?,
            other => match self.struct_view(other, span) {
                Ok(view) => view,
                Err(_) => {
                    let view = self.temporary(struct_id);
                    self.store_struct_expression(&view, other)?;
                    view
                }
            },
        };
        let tag = self.value(layout.tag);
        let place = self.projected_place(&view, 0, layout.tag);
        self.emit("load", vec![tag], vec![place], None);
        let failed = self.value(TypeName::Bool);
        self.emit(
            "eq",
            vec![failed],
            vec![
                hir::Operand::Value(tag),
                hir::Operand::Constant(type_id(layout.tag), failure.tag),
            ],
            None,
        );
        let (propagate, next) = (self.block(), self.block());
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(failed)],
            targets: vec![propagate, next],
        });
        self.current = propagate;
        self.propagate(&view, &failure, span)?;
        self.current = next;
        Ok((view, success.fields.first().map(|(_, one)| *one)))
    }

    /// Returns `failure`, read from `source`, as this function's own failure.
    fn propagate(
        &mut self,
        source: &StructView,
        failure: &VariantLayout,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let own = self
            .signature
            .slot
            .and_then(|one| self.types.enum_of(ElementType::Struct(one)).cloned());
        let Some((_, own_failure)) = own.as_ref().and_then(outcome) else {
            return Err(Diagnostic::new(
                span,
                "'?' needs a function that returns Option or Result",
            ));
        };
        let own = own.expect("checked");
        let destination = self.struct_view(&Expr::Name(RESULT.into(), span), span)?;
        let mut stores = vec![Store::One(
            self.projected_place(&destination, 0, own.tag),
            hir::Operand::Constant(type_id(own.tag), own_failure.tag),
        )];
        let (target, fields) = if own_failure.name == failure.name && same_types(&own_failure, failure) {
            (destination, own_failure.fields.clone())
        } else if let Some((wrapper, wrapping)) = (own_failure.name == failure.name).then(|| self.wrapping(&own_failure, failure)).flatten() {
            // `?` of `.err(e)` returns `.err(.wrapping(e))`.
            let (_, field) = &own_failure.fields[0];
            let target = StructView { struct_id: wrapper, offset: destination.offset + field.offset, ..destination };
            let tag = self.types.enum_of(ElementType::Struct(wrapper)).expect("an enum").tag;
            stores.push(Store::One(self.projected_place(&target, 0, tag), hir::Operand::Constant(type_id(tag), wrapping.tag)));
            (target, wrapping.fields)
        } else {
            return Err(Diagnostic::new(
                span,
                format!("'?' cannot return this failure from a function returning {}", own.name),
            ));
        };
        for ((_, to), (_, from)) in fields.iter().zip(&failure.fields) {
            let at = |view: &StructView, field: &FieldLayout| StructView {
                offset: view.offset + field.offset,
                ..view.clone()
            };
            match to.type_ {
                ElementType::Scalar(type_name) => {
                    let value = self.value(type_name);
                    let place = self.projected_place(source, from.offset, type_name);
                    self.emit("load", vec![value], vec![place], None);
                    stores.push(Store::One(
                        self.projected_place(&target, to.offset, type_name),
                        hir::Operand::Value(value),
                    ));
                }
                ElementType::Struct(struct_id) => {
                    let (to, from) = (
                        StructView {
                            struct_id,
                            ..at(&target, to)
                        },
                        StructView {
                            struct_id,
                            ..at(source, from)
                        },
                    );
                    self.prepare_struct_copy(&to, &from, &mut stores)?;
                }
            }
        }
        self.emit_stores(stores, span)?;
        self.drop_pending();
        self.drop_scopes(0);
        self.return_aggregate(span)
    }

    /// When `own` holds one enum with exactly one variant whose fields have
    /// `failure`'s types: that enum and variant.
    fn wrapping(&self, own: &VariantLayout, failure: &VariantLayout) -> Option<(u32, VariantLayout)> {
        let [(_, field)] = own.fields.as_slice() else {
            return None;
        };
        let ElementType::Struct(wrapper) = field.type_ else {
            return None;
        };
        let layout = self.types.enum_of(field.type_)?;
        let mut holding = layout.variants.iter().filter(|one| same_types(one, failure));
        match (holding.next(), holding.next()) {
            (Some(one), None) => Some((wrapper, one.clone())),
            _ => None,
        }
    }
}

fn same_types(one: &VariantLayout, other: &VariantLayout) -> bool {
    one.fields.iter().map(|(_, field)| field.type_).eq(other.fields.iter().map(|(_, field)| field.type_))
}
