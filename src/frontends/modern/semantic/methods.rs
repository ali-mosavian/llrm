//! Methods: `fn Point.move(self: &mut Point, ...)` is the function
//! `Point.move`, and `p.move(...)` calls it with `p` as its first argument.

use super::*;

impl FunctionCompiler<'_> {
    /// The namespace a receiver's methods are declared in.
    pub(super) fn receiver_type(&self, receiver: &Expr) -> Option<String> {
        if let Ok(Some(id)) = self.struct_expression_type(receiver, receiver.span()) {
            return self.types.structure(id).map(|one| one.name.clone());
        }
        if self.is_char_view(receiver) {
            return Some("string".into());
        }
        match self.expression_type_hint(receiver)? {
            bits @ TypeName::Bits { .. } => self.types.bits_of(bits).map(|one| one.name.clone()),
            scalar @ TypeName::Enum { .. } => self
                .types
                .enum_of(ElementType::Scalar(scalar))
                .map(|one| one.name.clone()),
            scalar if is_integer(scalar) || is_float(scalar) => Some(type_name_text(scalar)),
            scalar @ (TypeName::Char | TypeName::Bool | TypeName::String) => Some(type_name_text(scalar)),
            _ => None,
        }
    }

    /// Refuses `receiver.name(...)` of a method another module keeps
    /// private (section 14).
    pub(super) fn visible_method(&self, receiver: &Expr, name: &str, span: Span) -> Result<(), Diagnostic> {
        let Some(owner) = self.receiver_type(receiver) else {
            return Ok(());
        };
        for method in [format!("{owner}.{name}"), format!("{}.{name}", self.types.template_of(&owner))] {
            if self.private_methods.get(&method).is_some_and(|&module| module != span.module) {
                return Err(Diagnostic::new(span, format!("{method} is private to its module")));
            }
        }
        Ok(())
    }

    /// `receiver.name(arguments)` as the call of the method it names, if any.
    pub(super) fn method_as_call(&self, expression: &Expr) -> Option<Expr> {
        let Expr::MethodCall {
            receiver,
            name,
            arguments,
            span,
            ..
        } = expression
        else {
            return None;
        };
        if let Expr::Name(owner, _) = receiver.as_ref() {
            let qualified = format!("{owner}.{name}");
            let associated = self.visible(owner).is_none() && self.known_signature(&qualified).is_some_and(|one| !one.method);
            if associated {
                return Some(Expr::Call { name: qualified, type_arguments: Vec::new(), arguments: arguments.clone(), span: *span });
            }
        }
        let qualified = format!("{}.{name}", self.receiver_type(receiver)?);
        (self.known_signature(&qualified).is_some() || self.is_generator_call(&qualified))
            .then(|| Expr::Call {
                name: qualified,
                type_arguments: Vec::new(),
                arguments: std::iter::once(receiver.as_ref())
                    .chain(arguments)
                    .cloned()
                    .collect(),
                span: *span,
            })
    }

    /// `receiver.name(arguments)` of a generic type's method, as the call of
    /// its template, which a call instantiates.
    pub(super) fn generic_method_call(&self, expression: &Expr) -> Option<Expr> {
        let Expr::MethodCall { receiver, name, type_arguments, arguments, span } = expression else {
            return None;
        };
        let owner = self.receiver_type(receiver)?;
        let qualified = format!("{}.{name}", self.types.template_of(&owner));
        self.templates.borrow().is_template(&qualified).then(|| Expr::Call {
            name: qualified,
            type_arguments: type_arguments.clone(),
            arguments: std::iter::once(receiver.as_ref()).chain(arguments).cloned().collect(),
            span: *span,
        })
    }
}
