//! Which names are locals where an expression stands. A pass that rewrites
//! module-level names asks this walk, so that a local of the same name hides
//! the declaration, as the semantic scopes will.

use super::syntax::{AsmTarget, AssignTarget, Clause, Expr, Pattern, Span, Statement};

/// A local as a caller of the walk keeps it: its name, or also where it is bound.
pub trait Local {
    fn bound(name: &str, span: Span) -> Self;
}

impl Local for String {
    fn bound(name: &str, _: Span) -> Self {
        name.to_owned()
    }
}

impl Local for (String, Span) {
    fn bound(name: &str, span: Span) -> Self {
        (name.to_owned(), span)
    }
}

/// Calls `visit` on each expression of `body`, innermost first, with the
/// locals bound where it stands: `locals`, then each binding before it.
/// An assignment's named target is visited as the name it is.
pub fn walk_mut<E, L: Local>(
    body: &mut [Statement],
    locals: &mut Vec<L>,
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    let depth = locals.len();
    for statement in body {
        statement_mut(statement, locals, visit)?;
    }
    locals.truncate(depth);
    Ok(())
}

fn statement_mut<E, L: Local>(
    statement: &mut Statement,
    locals: &mut Vec<L>,
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    match statement {
        Statement::Bind { name, value, span, .. } => {
            expression_mut(value, locals, visit)?;
            locals.push(L::bound(name, *span));
        }
        Statement::Destructure { pattern, value, otherwise, .. } => {
            expression_mut(value, locals, visit)?;
            if let Some(otherwise) = otherwise {
                walk_mut(otherwise, locals, visit)?;
            }
            locals.extend(bound(pattern));
        }
        Statement::Assign { target, value, span, .. } => {
            match target {
                AssignTarget::Name(name) => assigned(name, *span, locals, visit)?,
                AssignTarget::Member { base, .. } | AssignTarget::Deref(base) | AssignTarget::Index { base, .. } => {
                    expression_mut(base, locals, visit)?
                }
            }
            if let AssignTarget::Index { indices, .. } = target {
                for index in indices {
                    expression_mut(index, locals, visit)?;
                }
            }
            expression_mut(value, locals, visit)?;
        }
        Statement::For { name, iterable, body, span, .. } => {
            expression_mut(iterable, locals, visit)?;
            scoped(name, *span, body, locals, visit)?;
        }
        Statement::ForRange { name, start, end, body, span } => {
            expression_mut(start, locals, visit)?;
            expression_mut(end, locals, visit)?;
            scoped(name, *span, body, locals, visit)?;
        }
        Statement::With { name, value, body, span, .. } => {
            expression_mut(value, locals, visit)?;
            scoped(name, *span, body, locals, visit)?;
        }
        Statement::Asm(asm) => {
            for one in asm.expressions_mut() {
                expression_mut(one, locals, visit)?;
            }
            let span = asm.span;
            for (_, target, _) in &mut asm.outputs {
                if let AsmTarget::Place(AssignTarget::Name(name)) = target {
                    assigned(name, span, locals, visit)?;
                }
            }
            locals.extend(asm.outputs.iter().filter_map(|(_, target, span)| match target {
                AsmTarget::Bind { name, .. } => Some(L::bound(name, *span)),
                AsmTarget::Place(_) => None,
            }));
        }
        Statement::Match { subject, arms, .. } => {
            expression_mut(subject, locals, visit)?;
            for arm in arms {
                let depth = locals.len();
                locals.extend(bound(&arm.pattern));
                walk_mut(&mut arm.body, locals, visit)?;
                locals.truncate(depth);
            }
        }
        _ => {
            for expression in statement.own_expressions_mut() {
                expression_mut(expression, locals, visit)?;
            }
            for block in statement.blocks_mut() {
                walk_mut(block, locals, visit)?;
            }
        }
    }
    Ok(())
}

fn bound<L: Local>(pattern: &Pattern) -> impl Iterator<Item = L> + '_ {
    pattern.bindings().into_iter().map(|(name, span)| L::bound(name, span))
}

/// An assigned `name`, visited as the name it is.
fn assigned<E, L: Local>(
    name: &mut String,
    span: Span,
    locals: &[L],
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    let mut named = Expr::Name(std::mem::take(name), span);
    visit(&mut named, locals)?;
    let Expr::Name(renamed, _) = named else {
        unreachable!("a name stays a name")
    };
    *name = renamed;
    Ok(())
}

/// `body` with `name`, bound at `span`, in it.
fn scoped<E, L: Local>(
    name: &str,
    span: Span,
    body: &mut [Statement],
    locals: &mut Vec<L>,
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    locals.push(L::bound(name, span));
    walk_mut(body, locals, visit)?;
    locals.pop();
    Ok(())
}

/// `walk_mut` for one expression.
pub fn expression_walk_mut<E, L: Local>(
    expression: &mut Expr,
    locals: &mut Vec<L>,
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    expression_mut(expression, locals, visit)
}

fn expression_mut<E, L: Local>(
    expression: &mut Expr,
    locals: &mut Vec<L>,
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    let depth = locals.len();
    match expression {
        Expr::Lambda { parameters, body, span } => {
            locals.extend(parameters.iter().map(|one| L::bound(&one.name, *span)));
            expression_mut(body, locals, visit)?;
        }
        // Each clause's names are bound in the clauses after it and in the element.
        Expr::Comprehension { element, clauses, .. } | Expr::Generator { element, clauses, .. } => {
            clauses_mut(clauses, locals, visit)?;
            expression_mut(element, locals, visit)?;
        }
        Expr::DictComprehension { key, value, clauses, .. } => {
            clauses_mut(clauses, locals, visit)?;
            expression_mut(key, locals, visit)?;
            expression_mut(value, locals, visit)?;
        }
        _ => {
            for child in expression.children_mut() {
                expression_mut(child, locals, visit)?;
            }
        }
    }
    locals.truncate(depth);
    visit(expression, locals)
}

fn clauses_mut<E, L: Local>(
    clauses: &mut [Clause],
    locals: &mut Vec<L>,
    visit: &mut impl FnMut(&mut Expr, &[L]) -> Result<(), E>,
) -> Result<(), E> {
    for clause in clauses {
        match clause {
            Clause::If(condition) => expression_mut(condition, locals, visit)?,
            Clause::For { pattern, iterable, end, .. } => {
                expression_mut(iterable, locals, visit)?;
                if let Some(end) = end {
                    expression_mut(end, locals, visit)?;
                }
                locals.extend(bound(pattern));
            }
        }
    }
    Ok(())
}
