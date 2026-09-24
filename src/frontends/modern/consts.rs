//! Constants: `const W: u16 = 320` is a compile-time value. Its initializer
//! folds to a literal, and each use of the name stands for that literal.

use std::collections::BTreeMap;

use super::error::Diagnostic;
use super::syntax::{BinaryOp, Expr, TypeAnnotation, TypeSpec, UnaryOp};

/// Each of `declared`'s constants as its literal, each folded after the
/// constants it names, whatever their order; a cycle is an error. `imported`
/// are other modules' constants, already folded, which the result keeps.
pub fn folded_all(
    declared: &BTreeMap<String, (Option<TypeAnnotation>, Expr)>,
    imported: &BTreeMap<String, Expr>,
) -> Result<BTreeMap<String, Expr>, Diagnostic> {
    let mut known = imported.clone();
    for name in declared.keys() {
        fold_declared(name, declared, &mut known, &mut Vec::new())?;
    }
    Ok(known)
}

fn fold_declared(
    name: &str,
    declared: &BTreeMap<String, (Option<TypeAnnotation>, Expr)>,
    known: &mut BTreeMap<String, Expr>,
    pending: &mut Vec<String>,
) -> Result<(), Diagnostic> {
    if known.contains_key(name) {
        return Ok(());
    }
    let (annotation, value) = &declared[name];
    if pending.iter().any(|one| one == name) {
        return Err(Diagnostic::new(value.span(), format!("constant {name} depends on itself")));
    }
    pending.push(name.to_owned());
    let named = value.names();
    for used in named.iter().filter(|one| declared.contains_key(*one)) {
        fold_declared(used, declared, known, pending)?;
    }
    pending.pop();
    let literal = folded(value, known).ok_or_else(|| {
        let reason = match named.iter().find(|one| !known.contains_key(*one)) {
            Some(unknown) => format!(": {unknown} is not a constant"),
            None => String::new(),
        };
        Diagnostic::new(value.span(), format!("{name} is not a compile-time value{reason}"))
    })?;
    known.insert(name.to_owned(), typed(literal, annotation.as_ref()));
    Ok(())
}

/// `expression` as a literal, the constants in `known` substituted; `None`
/// when it is not a compile-time value.
pub fn folded(expression: &Expr, known: &BTreeMap<String, Expr>) -> Option<Expr> {
    match expression {
        Expr::Integer(..)
        | Expr::Float(..)
        | Expr::String(..)
        | Expr::Boolean(..)
        | Expr::Character(..) => Some(expression.clone()),
        Expr::Name(name, _) => known.get(name).cloned(),
        Expr::Member { .. } => known.get(&expression.dotted()?).cloned(),
        Expr::Conversion {
            target,
            value,
            span,
        } => Some(Expr::Conversion {
            target: *target,
            value: Box::new(folded(value, known)?),
            span: *span,
        }),
        Expr::Unary { op, operand, span } => match (op, folded(operand, known)?) {
            (UnaryOp::Negative, Expr::Float(spelling, _)) => {
                Some(Expr::Float(format!("-{spelling}"), *span))
            }
            (UnaryOp::Not, Expr::Boolean(value, _)) => Some(Expr::Boolean(!value, *span)),
            (op, operand) => {
                let value = integer(&operand)?;
                let result = match op {
                    UnaryOp::Negative => value.checked_neg()?,
                    UnaryOp::Complement => !value,
                    UnaryOp::Not | UnaryOp::Deref => return None,
                };
                Some(Expr::Integer(result, *span))
            }
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            let (left, right) = (
                integer(&folded(left, known)?)?,
                integer(&folded(right, known)?)?,
            );
            let result = match op {
                BinaryOp::Add => left.checked_add(right)?,
                BinaryOp::Subtract => left.checked_sub(right)?,
                BinaryOp::Multiply => left.checked_mul(right)?,
                BinaryOp::FloorDivide => floor_divide(left, right)?,
                BinaryOp::Remainder => left.checked_rem(right)?,
                BinaryOp::ShiftLeft => left.checked_shl(u32::try_from(right).ok()?)?,
                BinaryOp::ShiftRight => left.checked_shr(u32::try_from(right).ok()?)?,
                BinaryOp::BitAnd => left & right,
                BinaryOp::BitOr => left | right,
                BinaryOp::BitXor => left ^ right,
                _ => return None,
            };
            Some(Expr::Integer(result, *span))
        }
        _ => None,
    }
}

/// `//`: the truncated quotient, one lower when the remainder's sign
/// differs from the divisor's.
fn floor_divide(left: i64, right: i64) -> Option<i64> {
    let quotient = left.checked_div(right)?;
    let remainder = left % right;
    Some(if remainder != 0 && (remainder < 0) != (right < 0) { quotient - 1 } else { quotient })
}

/// An integer constant's value.
pub fn integer(expression: &Expr) -> Option<i64> {
    match expression {
        Expr::Integer(value, _) => Some(*value),
        Expr::Conversion { value, .. } => integer(value),
        _ => None,
    }
}

/// The literal a constant of `annotation` stands for: a number typed as declared.
pub fn typed(literal: Expr, annotation: Option<&TypeAnnotation>) -> Expr {
    match (annotation, &literal) {
        (
            Some(TypeAnnotation::Value(TypeSpec::Primitive(target))),
            Expr::Integer(_, span) | Expr::Float(_, span),
        ) => Expr::Conversion {
            target: *target,
            value: Box::new(literal.clone()),
            span: *span,
        },
        _ => literal,
    }
}
