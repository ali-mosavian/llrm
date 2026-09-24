//! Rewrites that need the whole module before type checking.

use std::collections::BTreeMap;

use super::arguments;
use super::arguments::Formal;
use super::error::Diagnostic;
use super::syntax::BinaryOp;
use super::syntax::Expr;
use super::syntax::Function;
use super::syntax::Module;
use super::syntax::Statement;
use super::syntax::Struct;

pub fn desugar(module: &mut Module) -> Result<(), Diagnostic> {
    let structs = module.structs.clone();
    let enums: Vec<String> = module.enums.iter().map(|one| one.name.clone()).collect();
    let consts: BTreeMap<&str, &Expr> = module
        .consts
        .iter()
        .map(|one| (one.name.as_str(), &one.value))
        .collect();
    for function in &mut module.functions {
        for statement in &mut function.body {
            statement.walk_mut(&mut |expression| {
                constant(expression, &consts);
                repeat(expression);
                qualified_variant(expression, &enums);
                constructor(expression, &structs)
            })?;
        }
    }
    Ok(())
}

/// A declaration in a block, seen from there to the block's end.
#[derive(Clone)]
enum Local {
    Constant(Expr),
    /// The hidden name of a local `fn`.
    Function(String),
}

/// Each local `const` put in where its name is read (section 2), and each
/// local `fn` made a module function whose calls use its hidden name
/// (section 4). Runs on the parsed module, before names are resolved
/// across modules.
pub fn local_declarations(module: &mut Module) {
    let mut hoisted = Vec::new();
    for function in &mut module.functions {
        let owner = function.name.clone();
        locals(&owner, &mut function.body, &BTreeMap::new(), &mut hoisted);
    }
    module.functions.extend(hoisted);
}

fn locals(owner: &str, body: &mut Vec<Statement>, outer: &BTreeMap<String, Local>, hoisted: &mut Vec<Function>) {
    let mut scope = outer.clone();
    body.retain_mut(|statement| {
        match statement {
            Statement::Const(one) => {
                scope.insert(one.name.clone(), Local::Constant(one.value.clone()));
                return false;
            }
            Statement::Function(function) => {
                let hidden = format!("{}${}", owner.replace('.', "$"), function.name);
                // Seen in its own body, so that it may call itself.
                scope.insert(function.name.clone(), Local::Function(hidden.clone()));
                let mut inner = scope.clone();
                for parameter in &function.parameters {
                    inner.remove(&parameter.name);
                }
                function.name = hidden.clone();
                locals(&hidden, &mut function.body, &inner, hoisted);
                hoisted.push(function.as_ref().clone());
                return false;
            }
            _ => {}
        }
        for expression in statement.own_expressions_mut() {
            let Ok(()) = expression.walk_mut(&mut |one| -> Result<(), std::convert::Infallible> {
                local(one, &scope);
                Ok(())
            });
        }
        for block in statement.blocks_mut() {
            locals(owner, block, &scope, hoisted);
        }
        // A `let` of the name hides the declaration from here on.
        if let Statement::Bind { name, .. } = statement {
            scope.remove(name);
        }
        true
    });
}

/// A local declaration's name, read or called, as what it stands for.
fn local(expression: &mut Expr, scope: &BTreeMap<String, Local>) {
    match expression {
        Expr::Name(name, _) => match scope.get(name) {
            Some(Local::Constant(value)) => *expression = value.clone(),
            Some(Local::Function(hidden)) => *name = hidden.clone(),
            None => {}
        },
        Expr::Call { name, .. } => {
            if let Some(Local::Function(hidden)) = scope.get(name) {
                *name = hidden.clone();
            }
        }
        _ => {}
    }
}

/// A constant's name stands for its literal.
fn constant(expression: &mut Expr, consts: &BTreeMap<&str, &Expr>) {
    if let Expr::Name(name, _) = expression {
        if let Some(value) = consts.get(name.as_str()) {
            *expression = (*value).clone();
        }
    }
}

/// `[v] * n` gives every element one value; `[[v] * m] * n` is ranked `[n, m]`.
fn repeat(expression: &mut Expr) {
    let Expr::Binary {
        op: BinaryOp::Multiply,
        left,
        right,
        span,
    } = expression
    else {
        return;
    };
    let Expr::Array(items, _) = left.as_mut() else {
        return;
    };
    if items.len() != 1 {
        return;
    }
    let count = std::mem::replace(right.as_mut(), Expr::Boolean(false, *span));
    let (value, counts) = match items.pop().expect("one item") {
        Expr::Repeat { value, counts, .. } => (value, [vec![count], counts].concat()),
        value => (Box::new(value), vec![count]),
    };
    *expression = Expr::Repeat {
        value,
        counts,
        span: *span,
    };
}

/// `Shape.circle(...)` and `Mode.text` name a variant, not a method or field.
fn qualified_variant(expression: &mut Expr, enums: &[String]) {
    let (receiver, name, span) = match expression {
        Expr::MethodCall {
            receiver,
            name,
            span,
            ..
        } => (receiver, name, *span),
        Expr::Member { base, field, span } => (base, field, *span),
        _ => return,
    };
    let Expr::Name(enum_name, _) = receiver.as_ref() else {
        return;
    };
    if !enums.contains(enum_name) {
        return;
    }
    let (enum_name, name) = (enum_name.clone(), name.clone());
    let arguments = match expression {
        Expr::MethodCall { arguments, .. } => std::mem::take(arguments),
        _ => Vec::new(),
    };
    *expression = Expr::Variant {
        enum_name: Some(enum_name),
        name,
        arguments,
        span,
    };
}

/// `Point(0, y=1)` calls no function: it builds a `Point`.
fn constructor(expression: &mut Expr, structs: &[Struct]) -> Result<(), Diagnostic> {
    let Expr::Call {
        name,
        arguments: given,
        span,
        ..
    } = expression
    else {
        return Ok(());
    };
    // A generic struct's instance is chosen where types are known.
    let Some(layout) = structs.iter().find(|one| &one.name == name && one.generics.is_empty()) else {
        return Ok(());
    };
    // `Attr(raw)` reads a bits struct from its backing integer.
    if layout.bits.is_some()
        && matches!(given.as_slice(), [one] if !matches!(one, Expr::NamedArgument { .. }))
    {
        return Ok(());
    }
    let formals: Vec<_> = layout
        .fields
        .iter()
        .map(|field| Formal {
            name: &field.name,
            default: None,
        })
        .collect();
    let values = arguments::bind(name, &formals, std::mem::take(given), *span)?;
    *expression = Expr::StructLiteral {
        name: name.clone(),
        fields: layout
            .fields
            .iter()
            .zip(values)
            .map(|(field, value)| {
                let span = value.span();
                (field.name.clone(), value, span)
            })
            .collect(),
        span: *span,
    };
    Ok(())
}
