//! `match`: each arm's pattern is a chain of tests that falls through to the
//! next arm, then bindings in the arm's own scope.

use super::enums::EnumLayout;
use super::*;
use crate::frontends::modern::syntax::MatchArm;
use crate::frontends::modern::syntax::Pattern;

/// What a pattern is matched against.
#[derive(Clone)]
pub(super) enum Subject {
    /// A value, and the place it was read from when that is behind a
    /// pointer: a binding then names the part, not a copy of it.
    Scalar(hir::Operand, TypeName, Option<hir::Operand>),
    Aggregate(StructView),
    /// A borrowed one-dimensional view: its descriptor, far data and length.
    Sequence {
        descriptor: u32,
        data: u32,
        length: hir::Operand,
        element: ElementType,
    },
}

impl FunctionCompiler<'_> {
    pub(super) fn match_statement(
        &mut self,
        subject_expression: &Expr,
        arms: &[MatchArm],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let subject = if arms.iter().any(|arm| matches!(arm.pattern, Pattern::Sequence { .. })) {
            self.sequence_subject(subject_expression, span)?
        } else {
            self.subject(subject_expression, span)?
        };
        let complete = self.check_exhaustive(&subject, arms, span)?;
        let join = self.block();
        let mut falls = !complete;
        for (index, arm) in arms.iter().enumerate() {
            let next = self.block();
            // A value that reaches the last arm of a complete match matches it.
            if !(complete && index + 1 == arms.len()) {
                self.test(&arm.pattern, &subject, next)?;
            }
            self.in_scope(|this| {
                this.bind(&arm.pattern, &subject)?;
                this.record_pattern_borrows(subject_expression);
                this.statements(&arm.body)
            })?;
            if self.open() {
                falls = true;
                self.terminate(jump(join));
            }
            self.current = next;
        }
        // Past the last arm: a value no arm of an integer match names.
        self.terminate(jump(join));
        self.current = join;
        if !falls {
            self.terminate(hir::Terminator {
                kind: "unreachable",
                operands: Vec::new(),
                targets: Vec::new(),
            });
        }
        Ok(())
    }

    fn subject(&mut self, expression: &Expr, span: Span) -> Result<Subject, Diagnostic> {
        if let Some(call) = self.method_as_call(expression) {
            return self.subject(&call, span);
        }
        if let Some(struct_id) = self.struct_expression_type(expression, span)? {
            if let Expr::Call {
                name,
                arguments,
                span,
                ..
            } = expression
            {
                // The result is the statement's, dropped when the match ends.
                let result = self.call_into(name, arguments, *span)?;
                return Ok(Subject::Aggregate(self.statement_temporary(result)));
            }
            if let Ok(view) = self.struct_view(expression, span) {
                return Ok(Subject::Aggregate(view));
            }
            // A temporary, such as a variant literal.
            let view = self.temporary(struct_id);
            self.store_struct_expression(&view, expression)?;
            return Ok(Subject::Aggregate(self.statement_temporary(view)));
        }
        let value = self.expression(expression, None)?;
        let type_name = value.type_name;
        Ok(Subject::Scalar(required(value, span)?, type_name, None))
    }

    fn enum_layout(&self, subject: &Subject) -> Option<EnumLayout> {
        let element = match subject {
            Subject::Scalar(_, type_name, _) => ElementType::Scalar(*type_name),
            Subject::Aggregate(view) => ElementType::Struct(view.struct_id),
            Subject::Sequence { .. } => return None,
        };
        self.types.enum_of(element).cloned()
    }

    /// Branches to `fail` unless `pattern` matches; continues in a new block.
    pub(super) fn test(&mut self, pattern: &Pattern, subject: &Subject, fail: u32) -> Result<(), Diagnostic> {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Binding(..) => Ok(()),
            Pattern::Sequence { before, rest, after, span } => {
                if !matches!(subject, Subject::Sequence { .. }) {
                    return Err(Diagnostic::new(*span, "a sequence pattern needs a vec, array or view"));
                }
                self.test_sequence((before, rest.is_some(), after), subject, fail)
            }
            Pattern::Literal(literal) => {
                let Subject::Scalar(operand, type_name, _) = subject else {
                    return Err(Diagnostic::new(
                        literal.span(),
                        "a literal pattern needs a scalar value",
                    ));
                };
                let literal = self.coerced(literal, *type_name)?;
                let literal = required(literal, pattern.span())?;
                self.branch_unless("eq", operand.clone(), literal, fail);
                Ok(())
            }
            Pattern::Variant {
                enum_name,
                name,
                fields,
                span,
            } => {
                let layout = self.enum_layout(subject).ok_or_else(|| {
                    Diagnostic::new(*span, "a variant pattern needs an enum value")
                })?;
                if enum_name.as_ref().is_some_and(|one| one != &layout.name) {
                    return Err(Diagnostic::new(
                        *span,
                        format!("expected a {} variant", layout.name),
                    ));
                }
                let variant = layout.variant(name, *span)?.clone();
                // `.ok(_)` of a `Result[void, E]`: `_` matches the void payload.
                let fields = match fields.as_slice() {
                    [Pattern::Wildcard(_)] if variant.fields.is_empty() => &[][..],
                    _ => fields.as_slice(),
                };
                if !fields.is_empty() && fields.len() != variant.fields.len() {
                    return Err(Diagnostic::new(
                        *span,
                        format!("{}.{name} has {} fields", layout.name, variant.fields.len()),
                    ));
                }
                let tag = self.tag(subject, &layout);
                let expected =
                    hir::Operand::Constant(type_id(layout.tag_type(subject)), variant.tag);
                self.branch_unless("eq", tag, expected, fail);
                for (field, (_, layout)) in fields.iter().zip(&variant.fields) {
                    let inner = self.field_subject(subject, *layout);
                    self.test(field, &inner, fail)?;
                }
                Ok(())
            }
            Pattern::Struct { fields, span, .. } | Pattern::Tuple(fields, span) => {
                for (field, inner) in fields
                    .iter()
                    .zip(self.struct_fields(pattern, subject, *span)?)
                {
                    self.test(field, &inner, fail)?;
                }
                Ok(())
            }
        }
    }

    /// Binds the names in a pattern already known to match.
    pub(super) fn bind(&mut self, pattern: &Pattern, subject: &Subject) -> Result<(), Diagnostic> {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Literal(_) => Ok(()),
            Pattern::Binding(name, span) => self.bind_copy(name, subject, *span),
            Pattern::Sequence { before, rest, after, .. } => {
                self.bind_sequence((before, rest.as_deref(), after), subject)
            }
            Pattern::Variant {
                name, fields, span, ..
            } => {
                let layout = self.enum_layout(subject).expect("tested");
                let variant = layout.variant(name, *span)?.clone();
                for (field, (_, layout)) in fields.iter().zip(&variant.fields) {
                    let inner = self.field_subject(subject, *layout);
                    self.bind(field, &inner)?;
                }
                Ok(())
            }
            Pattern::Struct { fields, span, .. } | Pattern::Tuple(fields, span) => {
                for (field, inner) in fields
                    .iter()
                    .zip(self.struct_fields(pattern, subject, *span)?)
                {
                    self.bind(field, &inner)?;
                }
                Ok(())
            }
        }
    }

    /// The names `pattern` binds on an `item`, and their types, without
    /// compiling a match: what `bind` would bind. `None` when one is unknown.
    pub(super) fn pattern_bindings(&self, pattern: &Pattern, item: ElementType) -> Option<Vec<(String, BindingType)>> {
        let binding = |element| match element {
            ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
            ElementType::Struct(id) => BindingType::Struct(id),
        };
        let all = |pairs: Vec<(&Pattern, ElementType)>| -> Option<Vec<(String, BindingType)>> {
            let parts: Option<Vec<_>> = pairs.into_iter().map(|(one, element)| self.pattern_bindings(one, element)).collect();
            Some(parts?.concat())
        };
        match pattern {
            Pattern::Wildcard(_) | Pattern::Literal(_) => Some(Vec::new()),
            Pattern::Binding(name, _) => Some(vec![(name.clone(), binding(item))]),
            Pattern::Variant { name, fields, .. } => {
                let variant = self.types.enum_of(item)?.variants.iter().find(|one| &one.name == name)?;
                all(fields.iter().zip(variant.fields.iter().map(|(_, field)| field.type_)).collect())
            }
            Pattern::Struct { fields, .. } | Pattern::Tuple(fields, _) => {
                let ElementType::Struct(id) = item else { return None };
                let layout = self.types.structure(id)?;
                all(fields.iter().zip(layout.order.iter().map(|field| layout.fields[field].type_)).collect())
            }
            Pattern::Sequence { before, rest, after, .. } => {
                let ElementType::Scalar(sequence) = item else { return None };
                let element = self.types.sequence_element(sequence)?;
                let mut bound = all(before.iter().chain(after).map(|one| (one, element)).collect())?;
                if let Some(Pattern::Binding(name, _)) = rest.as_deref() {
                    bound.push((name.clone(), BindingType::Slice { element, rank: 1 }));
                }
                Some(bound)
            }
        }
    }

    /// A struct pattern's field subjects, positionally.
    fn struct_fields(
        &mut self,
        pattern: &Pattern,
        subject: &Subject,
        span: Span,
    ) -> Result<Vec<Subject>, Diagnostic> {
        let (name, fields) = match pattern {
            Pattern::Struct { name, fields, .. } => (Some(name), fields),
            Pattern::Tuple(fields, _) => (None, fields),
            _ => unreachable!("a struct or tuple pattern"),
        };
        let Subject::Aggregate(view) = subject else {
            return Err(Diagnostic::new(
                span,
                "a struct pattern needs a struct value",
            ));
        };
        let layout = self
            .types
            .structure(view.struct_id)
            .expect("resolved layout")
            .clone();
        if name.is_some_and(|name| &layout.name != name) {
            return Err(Diagnostic::new(
                span,
                format!("expected {}, found {}", layout.name, name.expect("checked")),
            ));
        }
        if fields.len() != layout.order.len() {
            return Err(Diagnostic::new(
                span,
                format!("{} has {} fields", layout.name, layout.order.len()),
            ));
        }
        Ok(layout
            .order
            .iter()
            .map(|field| self.field_subject(subject, layout.fields[field]))
            .collect())
    }

    fn field_subject(&mut self, subject: &Subject, field: FieldLayout) -> Subject {
        let Subject::Aggregate(view) = subject else {
            unreachable!("only aggregates have fields")
        };
        match field.type_ {
            ElementType::Scalar(type_name) => {
                let value = self.value(type_name);
                let place = self.projected_place(view, field.offset, type_name);
                self.emit("load", vec![value], vec![place.clone()], None);
                Subject::Scalar(hir::Operand::Value(value), type_name, view.pointer.map(|_| place))
            }
            ElementType::Struct(struct_id) => Subject::Aggregate(StructView {
                struct_id,
                offset: view.offset + field.offset,
                ..view.clone()
            }),
        }
    }

    fn tag(&mut self, subject: &Subject, layout: &EnumLayout) -> hir::Operand {
        match subject {
            Subject::Scalar(operand, ..) => operand.clone(),
            Subject::Aggregate(view) => {
                let value = self.value(layout.tag);
                let place = self.projected_place(view, 0, layout.tag);
                self.emit("load", vec![value], vec![place], None);
                hir::Operand::Value(value)
            }
            Subject::Sequence { .. } => unreachable!("a sequence has no tag"),
        }
    }

    pub(super) fn branch_unless(
        &mut self,
        compare: &'static str,
        left: hir::Operand,
        right: hir::Operand,
        fail: u32,
    ) {
        let condition = self.value(TypeName::Bool);
        self.emit(compare, vec![condition], vec![left, right], None);
        let pass = self.block();
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(condition)],
            targets: vec![pass, fail],
        });
        self.current = pass;
    }

    /// `name` holds its own copy of what it matched; a sequence, a view of it.
    fn bind_copy(&mut self, name: &str, subject: &Subject, span: Span) -> Result<(), Diagnostic> {
        let binding = match subject {
            Subject::Sequence { descriptor, element, .. } => Binding {
                type_: BindingType::Slice { element: *element, rank: 1 },
                mutable: false,
                storage: Storage::Slice(*descriptor),
            },
            Subject::Scalar(operand, type_name, _) if self.types.referent(*type_name).is_some() => {
                self.reference_binding(operand.clone(), *type_name).expect("a reference")
            }
            Subject::Scalar(_, type_name, Some(place)) => {
                let pointer = match place {
                    hir::Operand::IndirectPlace { base, offset: 0, .. } => *base,
                    _ => {
                        let pointer_type = self.types.pointer(type_id(*type_name), 0);
                        let pointer = self.value_type(pointer_type);
                        self.emit("address", vec![pointer], vec![place.clone()], None);
                        pointer
                    }
                };
                Binding { type_: BindingType::Scalar(*type_name), mutable: false, storage: Storage::Reference(pointer) }
            }
            Subject::Aggregate(source) if source.pointer.is_some() => {
                let hir::Operand::Value(pointer) = self.address_of(source) else {
                    unreachable!("an address is a value")
                };
                Binding { type_: BindingType::Struct(source.struct_id), mutable: false, storage: Storage::Reference(pointer) }
            }
            Subject::Scalar(operand, type_name, None) => {
                let place = self.local_place(name, type_id(*type_name), width(*type_name), false);
                self.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(place), operand.clone()],
                    None,
                );
                Binding {
                    type_: BindingType::Scalar(*type_name),
                    mutable: false,
                    storage: Storage::Place(place),
                }
            }
            Subject::Aggregate(source) => {
                let width = self.types.width(source.struct_id);
                let place = self.local_place(name, source.struct_id, width, false);
                let destination = StructView {
                    struct_id: source.struct_id,
                    place,
                    pointer: None,
                    indices: Vec::new(),
                    offset: 0,
                    mutable: false,
                    owner: name.into(),
                };
                let mut stores = Vec::new();
                self.prepare_struct_copy(&destination, source, &mut stores)?;
                for (place, value) in stores {
                    self.emit("store", Vec::new(), vec![place, value], None);
                }
                Binding {
                    type_: BindingType::Struct(source.struct_id),
                    mutable: false,
                    storage: Storage::Place(place),
                }
            }
        };
        let scope = self.scopes.last_mut().expect("scope");
        if scope.insert(name.to_owned(), binding).is_some() {
            return Err(Diagnostic::new(
                span,
                format!("{name:?} is bound twice in one pattern"),
            ));
        }
        Ok(())
    }

    /// Enums and booleans must be covered completely; whether every value is.
    fn check_exhaustive(
        &self,
        subject: &Subject,
        arms: &[MatchArm],
        span: Span,
    ) -> Result<bool, Diagnostic> {
        let element = match subject {
            Subject::Scalar(_, type_name, _) => ElementType::Scalar(*type_name),
            Subject::Aggregate(view) => ElementType::Struct(view.struct_id),
            // A sequence match need not be complete, but may be.
            Subject::Sequence { .. } => return Ok(covers_every_length(arms)),
        };
        let rows: Vec<Vec<&Pattern>> = arms.iter().map(|arm| vec![&arm.pattern]).collect();
        let Some(witness) = self.uncovered(&rows, &[element]) else {
            return Ok(true);
        };
        if self.enum_layout(subject).is_some() || element == ElementType::Scalar(TypeName::Bool) {
            return Err(Diagnostic::new(
                span,
                format!("match does not cover {}", witness.join(", ")),
            ));
        }
        Ok(false)
    }
}

impl EnumLayout {
    /// The type a subject's tag is compared as.
    fn tag_type(&self, subject: &Subject) -> TypeName {
        match subject {
            Subject::Scalar(_, type_name, _) => *type_name,
            Subject::Aggregate(_) => self.tag,
            Subject::Sequence { .. } => unreachable!("a sequence has no tag"),
        }
    }
}

/// Whether `arms` match a sequence of every length: some arm with a rest
/// takes every length from its count up, and exact arms the ones below.
fn covers_every_length(arms: &[MatchArm]) -> bool {
    let mut exact = BTreeSet::new();
    let mut from = None::<usize>;
    for arm in arms {
        match &arm.pattern {
            Pattern::Wildcard(_) | Pattern::Binding(..) => return true,
            Pattern::Sequence { before, rest, after, .. }
                if before.iter().chain(after).all(tuples::irrefutable) =>
            {
                let count = before.len() + after.len();
                if rest.is_some() {
                    from = Some(from.map_or(count, |one: usize| one.min(count)));
                } else {
                    exact.insert(count);
                }
            }
            _ => {}
        }
    }
    from.is_some_and(|from| (0..from).all(|count| exact.contains(&count)))
}
