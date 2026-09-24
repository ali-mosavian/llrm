//! Tuples: `(A, B)` is an anonymous struct of its elements, with fields
//! `0`, `1`, ..., registered the first time it is named or built.

use super::*;
use crate::frontends::modern::syntax::{Pattern, StructField};

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

    fn tuple_name(&self, elements: &[ElementType]) -> String {
        let names: Vec<&str> = elements
            .iter()
            .map(|one| self.types[(one.id() - 1) as usize].name.as_str())
            .collect();
        format!("({})", names.join(", "))
    }

    pub(super) fn tuple_id(&self, elements: &[ElementType]) -> Option<u32> {
        self.structs
            .get(&self.tuple_name(elements))
            .map(|one| one.id)
    }

    /// The tuple of `elements`, registered on first use.
    pub(super) fn tuple(
        &mut self,
        elements: &[ElementType],
        span: Span,
    ) -> Result<u32, Diagnostic> {
        if let Some(id) = self.tuple_id(elements) {
            return Ok(id);
        }
        let fields = elements
            .iter()
            .enumerate()
            .map(|(index, element)| StructField {
                name: index.to_string(),
                mutable: true,
                type_spec: self.spec_of(*element),
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
    pub(super) fn tuple_elements(&self, items: &[Expr], span: Span) -> Option<Vec<ElementType>> {
        items
            .iter()
            .map(|item| match self.struct_expression_type(item, span) {
                Ok(Some(id)) => Some(ElementType::Struct(id)),
                _ => self
                    .expression_type_hint(item)
                    .or_else(|| matches!(item, Expr::Integer(..)).then_some(TypeName::I16))
                    .map(ElementType::Scalar),
            })
            .collect()
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

    /// `let (q, r) = value`: a hidden owner holds the value, and the
    /// pattern's names are views of its parts. A sequence is borrowed, not
    /// owned. A pattern that may not match runs `otherwise` when it does not.
    pub(super) fn destructure(
        &mut self,
        pattern: &Pattern,
        value: &Expr,
        otherwise: Option<&[Statement]>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if otherwise.is_none() && !irrefutable(pattern) {
            return Err(Diagnostic::new(
                pattern.span(),
                "a 'let' pattern that may not match needs 'else:'",
            ));
        }
        let subject = if matches!(pattern, Pattern::Sequence { .. }) {
            self.sequence_subject(value, span)?
        } else {
            self.destructured(value, span)?
        };
        if let Some(otherwise) = otherwise {
            let (fail, pass) = (self.block(), self.block());
            self.test(pattern, &subject, fail)?;
            self.terminate(jump(pass));
            self.current = fail;
            self.scoped(otherwise)?;
            if self.open() {
                return Err(Diagnostic::new(span, "a 'let ... else:' block must leave: return, break or continue"));
            }
            self.current = pass;
        }
        self.bind(pattern, &subject)
    }

    /// `value` held by a hidden owner, to take apart.
    fn destructured(&mut self, value: &Expr, span: Span) -> Result<matching::Subject, Diagnostic> {
        let owner = format!("$destructured{}", self.next_place);
        self.statement(&Statement::Bind {
            mutable: false,
            name: owner.clone(),
            annotation: None,
            value: value.clone(),
            span,
        })?;
        let binding = self.binding(&owner, span)?.clone();
        match binding.type_ {
            BindingType::Struct(struct_id) => Ok(matching::Subject::Aggregate(
                binding_view(struct_id, &binding.storage, false, &owner).expect("a local"),
            )),
            _ => Err(Diagnostic::new(span, "only a struct, tuple or sequence can be taken apart")),
        }
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
