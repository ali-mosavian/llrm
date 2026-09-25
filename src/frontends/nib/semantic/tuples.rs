//! Tuples: `(A, B)` is an anonymous struct of its elements, with fields
//! `0`, `1`, ..., registered the first time it is named or built.

use super::*;
use crate::frontends::nib::syntax::{Pattern, StructField};

/// A tuple element's type, and its shape when it is a fixed array.
pub(super) type Member = (ElementType, Option<Shape>);

impl TypeRegistry {
    /// How to spell a resolved type in source.
    pub(super) fn spec_of(&self, element: ElementType) -> TypeSpec {
        match element {
            ElementType::Scalar(type_name) => TypeSpec::Primitive(type_name),
            ElementType::Struct(id) => {
                TypeSpec::Named(self.structure(id).expect("registered").name.clone())
            }
        }
    }

    fn tuple_name(&self, elements: &[Member]) -> String {
        let names: Vec<String> = elements
            .iter()
            .map(|(element, shape)| match shape {
                Some(shape) => self.array_name(*element, *shape),
                None => self.types[(element.id() - 1) as usize].name.clone(),
            })
            .collect();
        format!("({})", names.join(", "))
    }

    /// Whether the struct `id` is a tuple, whose elements `t[k]` names.
    pub(super) fn is_tuple(&self, id: u32) -> bool {
        self.structure(id).is_some_and(|one| one.name.starts_with('('))
    }

    pub(super) fn tuple_id(&self, elements: &[Member]) -> Option<u32> {
        self.structs
            .get(&self.tuple_name(elements))
            .map(|one| one.id)
    }

    /// The tuple of `elements`, registered on first use.
    pub(super) fn tuple(
        &mut self,
        elements: &[Member],
        span: Span,
    ) -> Result<u32, Diagnostic> {
        if let Some(id) = self.tuple_id(elements) {
            return Ok(id);
        }
        let fields = elements
            .iter()
            .enumerate()
            .map(|(index, (element, shape))| StructField {
                name: index.to_string(),
                mutable: true,
                type_spec: self.spec_of(*element),
                dims: shape.map_or_else(Vec::new, |one| one.dims().to_vec()),
                span,
            })
            .collect();
        let name = self.tuple_name(elements);
        self.register_struct(&Struct {
            name: name.clone(),
            generics: Vec::new(),
            bits: None,
            pack: None,
            fields,
            span,
        })?;
        Ok(self.structs[&name].id)
    }
}

impl FunctionCompiler<'_> {
    /// What each element of a tuple literal is, when all are known.
    pub(super) fn tuple_elements(&self, items: &[Expr], span: Span) -> Option<Vec<Member>> {
        items
            .iter()
            .map(|item| match self.struct_expression_type(item, span) {
                _ if self.fixed_array_hint(item).is_some() => self.fixed_array_hint(item).map(|(element, shape)| (element, Some(shape))),
                Ok(Some(id)) => Some((ElementType::Struct(id), None)),
                _ => self
                    .expression_type_hint(item)
                    .or_else(|| matches!(item, Expr::Integer(..)).then_some(TypeName::I16))
                    .map(|one| (ElementType::Scalar(one), None)),
            })
            .collect()
    }

    /// `t[k]` of a tuple `t` and a literal `k`: the element's field.
    pub(super) fn tuple_element(&self, base: &Expr, indices: &[Expr], span: Span) -> Option<Expr> {
        let [Expr::Integer(index, _)] = indices else {
            return None;
        };
        let id = self.struct_expression_type(base, span).ok().flatten().or_else(|| self.struct_type_hint(base, span))?;
        let field = index.to_string();
        (self.types.is_tuple(id) && self.types.structure(id)?.fields.contains_key(&field))
            .then(|| Expr::Member { base: Box::new(base.clone()), field, span })
    }

    /// Registers a tuple literal's type, its elements' already registered,
    /// so that typing it later needs no registration. One whose elements are
    /// not yet known is typed by where it goes.
    pub(super) fn declare_tuple(&mut self, expression: &Expr) -> Result<(), Diagnostic> {
        let Expr::Tuple(items, span) = expression else {
            return Ok(());
        };
        if let Some(elements) = self.tuple_elements(items, *span) {
            self.types.tuple(&elements, *span)?;
        }
        Ok(())
    }

    /// `(a, b)` stored into `layout`, as the struct literal it is.
    pub(super) fn tuple_literal(
        items: &[Expr],
        layout: &StructLayout,
        span: Span,
    ) -> Result<Expr, Diagnostic> {
        if items.len() != layout.order.len() {
            return Err(Diagnostic::new(
                span,
                format!("expected {}, found a tuple of {}", layout.name, items.len()),
            ));
        }
        Ok(Expr::StructLiteral {
            name: layout.name.clone(),
            fields: layout
                .order
                .iter()
                .zip(items)
                .map(|(name, item)| (name.clone(), item.clone(), item.span()))
                .collect(),
            span,
        })
    }

    /// `let (q, r) = value`: bound as a `match` arm binds it, so a named
    /// value is borrowed and a temporary taken apart. A pattern that may not
    /// match runs `otherwise` when it does not.
    pub(super) fn destructure(
        &mut self,
        pattern: &Pattern,
        value: &Expr,
        otherwise: Option<&[Statement]>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let length = self.fixed_array_hint(value).filter(|(_, shape)| shape.rank == 1).map(|(_, shape)| shape.len());
        if otherwise.is_none() && !irrefutable_over(pattern, length) {
            return Err(Diagnostic::new(
                pattern.span(),
                "a 'let' pattern that may not match needs 'else:'",
            ));
        }
        let subject = if matches!(pattern, Pattern::Sequence { .. }) {
            self.sequence_subject(value, span)?
        } else {
            self.subject(value, span)?
        };
        let consumed = self.consumed_temporary(&subject);
        if let Some(otherwise) = otherwise {
            let (fail, pass) = (self.block(), self.block());
            self.test(pattern, &subject, fail)?;
            self.terminate(jump(pass));
            self.current = fail;
            if consumed {
                self.drop_subject(&subject);
            }
            self.scoped(otherwise)?;
            if self.open() {
                return Err(Diagnostic::new(span, "a 'let ... else:' block must leave: return, break or continue"));
            }
            self.current = pass;
        }
        self.bind_subject(pattern, &subject, consumed, value)
    }
}

/// Whether `pattern` matches every value; a fixed array's `length` settles
/// a sequence pattern.
fn irrefutable_over(pattern: &Pattern, length: Option<u32>) -> bool {
    match (pattern, length) {
        (Pattern::Sequence { before, rest, after, .. }, Some(length)) => {
            let named = (before.len() + after.len()) as u32;
            before.iter().chain(after).all(irrefutable) && if rest.is_some() { named <= length } else { named == length }
        }
        _ => irrefutable(pattern),
    }
}

pub(super) fn irrefutable(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard(_) | Pattern::Binding(..) => true,
        Pattern::Struct { fields, .. } | Pattern::Tuple(fields, _) => {
            fields.iter().all(irrefutable)
        }
        Pattern::Sequence { before, rest, after, .. } => before.is_empty() && after.is_empty() && rest.is_some(),
        Pattern::Literal(_) | Pattern::Variant { .. } => false,
    }
}
