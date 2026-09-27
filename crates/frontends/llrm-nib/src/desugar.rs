//! Rewrites that need the whole module before type checking.

use std::collections::BTreeMap;

use super::arguments;
use super::arguments::Formal;
use super::error::Diagnostic;
use super::syntax::BinaryOp;
use super::syntax::Expr;
use super::syntax::Function;
use super::syntax::GenericParameter;
use super::syntax::IterationMode;
use super::syntax::ParameterType;
use super::syntax::TypeAnnotation;
use super::syntax::TypeSpec;
use super::syntax::UnaryOp;
use super::scopes;
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
    for one in &mut module.statics {
        one.value.walk_mut(&mut |expression| {
            constant(expression, &consts, &[]);
            repeat(expression);
            constructor(expression, &structs)
        })?;
    }
    for function in &mut module.functions {
        iterator_parameters(function);
        if matches!(&function.result, TypeAnnotation::Value(TypeSpec::Applied { name, .. }) if name == "iter") {
            hand_over_returned(&mut function.body);
        }
        for default in function.parameters.iter_mut().filter_map(|one| one.default.as_mut()) {
            default.walk_mut(&mut |expression| {
                constant(expression, &consts, &[]);
                Ok::<(), Diagnostic>(())
            })?;
        }
        let mut locals = function.parameters.iter().map(|one| one.name.clone()).collect();
        scopes::walk_mut(&mut function.body, &mut locals, &mut |expression, locals| {
            constant(expression, &consts, locals);
            Ok::<(), Diagnostic>(())
        })?;
        for statement in &mut function.body {
            statement.walk_mut(&mut |expression| {
                untyped_arithmetic(expression);
                repeat(expression);
                qualified_variant(expression, &enums);
                constructor(expression, &structs)
            })?;
        }
    }
    Ok(())
}

/// Each `iter[T]` parameter as a type parameter of its own: a function
/// taking a generator is instantiated for the state of each passed to it
/// (section 12).
fn iterator_parameters(function: &mut Function) {
    for parameter in &mut function.parameters {
        let spec = match &mut parameter.type_ {
            ParameterType::Owned(TypeAnnotation::Value(spec)) | ParameterType::Borrowed { target: TypeAnnotation::Value(spec), .. } => spec,
            _ => continue,
        };
        if matches!(spec, TypeSpec::Applied { name, .. } if name == "iter") {
            let name = format!("$Iterator{}", function.generics.len());
            function.generics.push(GenericParameter { name: name.clone(), bound: None });
            *spec = TypeSpec::Named(name);
        }
    }
}

/// In a function returning `iter[T]`, `return items` hands `items` over: its
/// items are yielded, then the function ends (section 12). So a function
/// that returns an iterator is a generator, and escapes or is consumed in
/// place as any is.
fn hand_over_returned(body: &mut Vec<Statement>) {
    let mut at = 0;
    while at < body.len() {
        for block in body[at].blocks_mut() {
            hand_over_returned(block);
        }
        if let Statement::Return { value: value @ Some(_), span } = &mut body[at] {
            let (iterable, span) = (value.take().expect("matched"), *span);
            let name = format!("$returned{}_{}", span.line, span.column);
            let each = Statement::Yield { value: Expr::Name(name.clone(), span), span };
            let items = Statement::For { mode: IterationMode::Value, name, iterable, body: vec![each], span };
            body.insert(at, items);
            at += 1;
        }
        at += 1;
    }
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
        if let Statement::Asm(asm) = statement {
            for (_, target, _) in &asm.outputs {
                if let crate::syntax::AsmTarget::Bind { name, .. } = target {
                    scope.remove(name);
                }
            }
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

/// A constant's name stands for its literal, where no local hides it.
/// Arithmetic on untyped integer literals is one, so it takes its type
/// from context as they do (section 3).
fn untyped_arithmetic(expression: &mut Expr) {
    let untyped = |one: &Expr| match one {
        Expr::Integer(..) => true,
        Expr::Unary { op: UnaryOp::Negative, operand, .. } => matches!(operand.as_ref(), Expr::Integer(..)),
        _ => false,
    };
    let operands_untyped = match expression {
        Expr::Binary { left, right, .. } => untyped(left) && untyped(right),
        Expr::Unary { operand, .. } => untyped(operand),
        _ => false,
    };
    if operands_untyped {
        if let Some(value @ Expr::Integer(..)) = super::consts::folded(expression, &BTreeMap::new()) {
            *expression = value;
        }
    }
}

fn constant(expression: &mut Expr, consts: &BTreeMap<&str, &Expr>, locals: &[String]) {
    if let Expr::Name(name, _) = expression {
        if let Some(value) = consts.get(name.as_str()).filter(|_| !locals.contains(name)) {
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
