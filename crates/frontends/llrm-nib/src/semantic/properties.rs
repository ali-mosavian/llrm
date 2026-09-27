//! `a.len`, `a.capacity`, and `a.dim[i]`: a sequence's descriptor words, read as fields.

use super::*;

const PROPERTIES: [&str; 3] = ["len", "capacity", "dim"];

impl FunctionCompiler<'_> {
    /// The receiver, property, and axis argument when `expression` reads one.
    pub(super) fn sequence_property<'e>(
        &self,
        expression: &'e Expr,
    ) -> Option<(&'e Expr, &'static str, &'e [Expr])> {
        let (member, arguments) = match expression {
            Expr::Index { base, indices, .. } => (base.as_ref(), indices.as_slice()),
            member => (member, &[][..]),
        };
        let Expr::Member { base, field, .. } = member else {
            return None;
        };
        let name = PROPERTIES.into_iter().find(|one| one == field)?;
        if (name == "dim") == arguments.is_empty() || !self.is_sequence(base) {
            return None;
        }
        Some((base, name, arguments))
    }

    fn is_sequence(&self, expression: &Expr) -> bool {
        let Expr::Name(name, span) = expression else {
            return self.fixed_array_hint(expression).is_some()
                || self.view_type_of(expression).is_some()
                || self.expression_type_hint(expression).is_some_and(|one| self.types.owned_element(one).is_some());
        };
        self.binding(name, *span).is_ok_and(|one| {
            matches!(one.type_, BindingType::Scalar(type_name) if self.types.owned_element(type_name).is_some())
                || matches!(one.type_, BindingType::Array { .. } | BindingType::Slice { .. })
        })
    }

    /// `s.empty()` of a sequence, as the `s.len == 0` it means.
    pub(super) fn emptiness(&self, expression: &Expr) -> Option<Expr> {
        let Expr::MethodCall { receiver, name, arguments, span, .. } = expression else {
            return None;
        };
        if name != "empty" || !arguments.is_empty() || !self.is_sequence(receiver) {
            return None;
        }
        let length = Expr::Member { base: receiver.clone(), field: "len".into(), span: *span };
        Some(Expr::Binary {
            op: BinaryOp::Equal,
            left: Box::new(length),
            right: Box::new(Expr::Integer(0, *span)),
            span: *span,
        })
    }

    pub(super) fn is_property_method(name: &str) -> bool {
        PROPERTIES.contains(&name)
    }
}
