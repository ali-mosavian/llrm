//! The `Iterable` and `Iterator` protocols (section 12): a `for` over a
//! value whose type has an `iter` method takes its iterator, and calls the
//! iterator's `next` until it returns `.none`. A value that is itself an
//! iterator is iterated directly.

use super::*;
use crate::syntax::{MatchArm, Pattern};

impl FunctionCompiler<'_> {
    /// Compiles `for name in iterable: body` through the protocols, when
    /// `iterable`'s type implements one; whether it did.
    pub(super) fn for_protocol(&mut self, mode: IterationMode, name: &str, iterable: &Expr, body: &[Statement], span: Span) -> Result<bool, Diagnostic> {
        let Some(type_name) = self.receiver_type(iterable) else {
            return Ok(false);
        };
        let iterator = if self.loop_consumes(iterable) {
            iterable.clone()
        } else if self.known_signature(&format!("{type_name}.iter")).is_some() {
            Expr::MethodCall { receiver: Box::new(iterable.clone()), name: "iter".into(), type_arguments: Vec::new(), arguments: Vec::new(), span }
        } else {
            return Ok(false);
        };
        if mode != IterationMode::Value {
            return Err(Diagnostic::new(span, "an iterator yields values: write 'for item in ...'"));
        }
        let held = format!("$iterator{}", self.next_place);
        let next = Expr::MethodCall { receiver: Box::new(Expr::Name(held.clone(), span)), name: "next".into(), type_arguments: Vec::new(), arguments: Vec::new(), span };
        let variant = |variant: &str, fields| Pattern::Variant { enum_name: None, name: variant.into(), fields, span };
        let arms = vec![
            MatchArm { pattern: variant("some", vec![Pattern::Binding(name.into(), span)]), body: body.to_vec(), span },
            MatchArm { pattern: variant("none", Vec::new()), body: vec![Statement::Break(span)], span },
        ];
        let expansion = [
            Statement::Bind { mutable: true, name: held, annotation: None, value: iterator, span },
            Statement::While { condition: Expr::Boolean(true, span), body: vec![Statement::Match { subject: next, arms, span }], span },
        ];
        self.scoped(&expansion)?;
        Ok(true)
    }

    /// Whether a `for` over `iterable` takes it by value: an iterator, which
    /// has `next` and no `iter`, rather than an iterable it borrows.
    pub(super) fn loop_consumes(&self, iterable: &Expr) -> bool {
        self.receiver_type(iterable).is_some_and(|type_name| {
            self.known_signature(&format!("{type_name}.iter")).is_none() && self.known_signature(&format!("{type_name}.next")).is_some()
        })
    }
}
