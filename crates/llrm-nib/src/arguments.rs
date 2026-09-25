//! Binds a call's positional and named arguments to the callee's parameters.

use super::error::Diagnostic;
use super::syntax::Expr;
use super::syntax::Span;

pub struct Formal<'a> {
    pub name: &'a str,
    pub default: Option<&'a Expr>,
}

/// The arguments in parameter order. Positional arguments come first; a
/// parameter left unnamed takes its default.
pub fn bind(
    callee: &str,
    formals: &[Formal],
    arguments: Vec<Expr>,
    span: Span,
) -> Result<Vec<Expr>, Diagnostic> {
    let mut bound: Vec<Option<Expr>> = vec![None; formals.len()];
    let mut named = false;
    for (position, argument) in arguments.into_iter().enumerate() {
        let (index, value) = match argument {
            Expr::NamedArgument { name, value, span } => {
                named = true;
                let index = formals
                    .iter()
                    .position(|formal| formal.name == name)
                    .ok_or_else(|| {
                        Diagnostic::new(span, format!("{callee} has no parameter {name:?}"))
                    })?;
                (index, *value)
            }
            positional if named => {
                return Err(Diagnostic::new(
                    positional.span(),
                    "a positional argument cannot follow a named one",
                ));
            }
            positional if position >= formals.len() => {
                return Err(Diagnostic::new(
                    positional.span(),
                    format!("{callee} expects {} arguments", formals.len()),
                ));
            }
            positional => (position, positional),
        };
        if bound[index].is_some() {
            return Err(Diagnostic::new(
                value.span(),
                format!("{:?} is given more than once", formals[index].name),
            ));
        }
        bound[index] = Some(value);
    }
    bound
        .into_iter()
        .zip(formals)
        .map(|(value, formal)| {
            value.or_else(|| formal.default.cloned()).ok_or_else(|| {
                Diagnostic::new(span, format!("{callee} requires {:?}", formal.name))
            })
        })
        .collect()
}
