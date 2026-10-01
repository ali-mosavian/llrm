//! Whether a place may be written (section 5): the one rule every write
//! and every `&mut` asks. An owner is written only if bound `let mut`, a
//! field only if declared `mut`, and nothing reached through a `&` is.

use super::*;
use crate::syntax::UnaryOp;

impl FunctionCompiler<'_> {
    /// Errs unless `place` may be written, or borrowed `&mut`.
    pub(super) fn place_writable(&self, place: &Expr, span: Span) -> Result<(), Diagnostic> {
        match place {
            Expr::Name(name, _) => match self.visible(name) {
                Some(binding) if !binding.mutable => Err(Diagnostic::new(span, format!("binding {name:?} is immutable"))),
                _ => Ok(()),
            },
            Expr::Member { base, field, .. } => {
                if let Some(owner) = self.receiver_type(base) {
                    // An instance of a generic type is declared by its template.
                    let declared = owner.split('[').next().unwrap_or(&owner);
                    if self.types.fixed_fields.contains(&(declared.to_owned(), field.clone())) {
                        return Err(Diagnostic::new(span, format!("field {field:?} of {owner} is not declared 'mut'")));
                    }
                }
                self.container_writable(base, span)
            }
            Expr::Index { base, .. } | Expr::Slice { base, .. } => self.container_writable(base, span),
            Expr::Unary { op: UnaryOp::Deref, operand, .. } => match self.expression_type_hint(operand) {
                Some(TypeName::Pointer { mutable: false, .. }) => Err(Diagnostic::new(span, format!("binding \"*{}\" is immutable", path_text(operand)))),
                _ => Ok(()),
            },
            _ => match self.reference_at(place) {
                Some(_) => self.referent_writable(place, span),
                None => Ok(()),
            },
        }
    }

    /// Errs unless assigning `value` through `target` may write what it
    /// writes: the place, or what a reference there refers to.
    pub(super) fn check_written(&self, target: &AssignTarget, operation: Option<BinaryOp>, value: &Expr, span: Span) -> Result<(), Diagnostic> {
        let place = target.expression(span);
        match self.expression_type_hint(&place) {
            Some(reference) if self.reference_at(&place).is_some() && !self.seats(reference, operation, value) => self.referent_writable(&place, span),
            _ => self.place_writable(&place, span),
        }
    }

    /// Errs unless what the reference `place` holds may be written: it is
    /// `&mut`, and the path to it goes through no `&`.
    pub(super) fn referent_writable(&self, place: &Expr, span: Span) -> Result<(), Diagnostic> {
        match self.reference_at(place) {
            Some(false) => Err(Diagnostic::new(span, format!("cannot write through {:?}, a '&' reference", path_text(place)))),
            _ => self.exclusive_path(place, span),
        }
    }

    /// Errs unless `container`, which holds the place written, may be
    /// written: through it when it is a reference.
    fn container_writable(&self, container: &Expr, span: Span) -> Result<(), Diagnostic> {
        match self.reference_at(container) {
            Some(_) => self.referent_writable(container, span),
            None => self.place_writable(container, span),
        }
    }

    /// Errs when the path to `place` goes through a `&`.
    fn exclusive_path(&self, place: &Expr, span: Span) -> Result<(), Diagnostic> {
        match place {
            Expr::Name(name, _) => match self.visible(name) {
                Some(binding @ Binding { storage: Storage::Reference(_) | Storage::Slice(_), .. })
                    if !binding.mutable && borrows::identity(&binding.storage).and_then(|one| self.parameter_lives.get(&one)) != Some(&borrows::Life::Frame) =>
                {
                    Err(Diagnostic::new(span, format!("cannot write through {name:?}, a '&' reference")))
                }
                _ => Ok(()),
            },
            Expr::Member { base, .. } | Expr::Index { base, .. } | Expr::Slice { base, .. } => match self.reference_at(base) {
                Some(_) => self.referent_writable(base, span),
                None => self.exclusive_path(base, span),
            },
            _ => Ok(()),
        }
    }

    /// Whether `place` -- a field, element or result, not a name, which is
    /// bound as what it refers to -- holds a reference: `Some(mutable)`.
    fn reference_at(&self, place: &Expr) -> Option<bool> {
        match self.expression_type_hint(place)? {
            TypeName::Pointer { mutable, .. } if self.reference_type(place) => Some(mutable),
            _ => None,
        }
    }
}

/// `place` as the source writes it, for a diagnostic.
fn path_text(place: &Expr) -> String {
    match place {
        Expr::Name(name, _) => name.clone(),
        Expr::Member { base, field, .. } => format!("{}.{field}", path_text(base)),
        Expr::Index { base, .. } | Expr::Slice { base, .. } => format!("{}[...]", path_text(base)),
        Expr::Unary { op: UnaryOp::Deref, operand, .. } => format!("*{}", path_text(operand)),
        _ => "a value".into(),
    }
}
