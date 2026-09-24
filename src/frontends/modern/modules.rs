//! Modules: each file is one, named by its path under the source root, as
//! `graphics.sprite`. Loading follows the imports from the main module;
//! linking makes one module of them all, each imported module's declarations
//! qualified with its name and every reference to one resolved.

use std::collections::{BTreeMap, BTreeSet};

use super::error::Diagnostic;
use super::lexer::lex;
use super::parser::parse;
use super::syntax::{Expr, Import, Module, Pattern, Span, Statement, TypeAnnotation, TypeSpec};

/// A diagnostic and the module it is in; the main module is `""`.
pub type Located = (String, Diagnostic);

/// The main module `source` and every module it imports, linked as one.
/// `read` gives the source of a module by its name.
pub fn load(
    source: &str,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<Module, Located> {
    let mut loaded = BTreeMap::new();
    let main = parsed("", source)?;
    let mut order = Vec::new();
    visit("", &main, &mut Vec::new(), &mut loaded, &mut order, read)?;
    loaded.insert(String::new(), main);
    link(loaded, &order)
}

fn parsed(name: &str, source: &str) -> Result<Module, Located> {
    lex(source)
        .and_then(parse)
        .map_err(|error| (name.to_owned(), error))
}

/// Loads what `module` imports, depth first; `order` lists each once,
/// imported before importer.
fn visit(
    name: &str,
    module: &Module,
    chain: &mut Vec<String>,
    loaded: &mut BTreeMap<String, Module>,
    order: &mut Vec<String>,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<(), Located> {
    chain.push(name.to_owned());
    for import in &module.imports {
        if let Some(at) = chain.iter().position(|one| one == &import.module) {
            let cycle = [&chain[at..], &[import.module.clone()]]
                .concat()
                .join(" -> ");
            return Err((
                name.to_owned(),
                Diagnostic::new(import.span, format!("import cycle: {cycle}")),
            ));
        }
        if loaded.contains_key(&import.module) {
            continue;
        }
        let source = read(&import.module).map_err(|error| {
            (
                name.to_owned(),
                Diagnostic::new(
                    import.span,
                    format!("cannot import {}: {error}", import.module),
                ),
            )
        })?;
        let imported = parsed(&import.module, &source)?;
        visit(&import.module, &imported, chain, loaded, order, read)?;
        loaded.insert(import.module.clone(), imported);
    }
    chain.pop();
    order.push(name.to_owned());
    Ok(())
}

/// What a module declares, by the name it declares it under.
struct Declared {
    names: BTreeSet<String>,
    /// Functions only a value calls.
    methods: BTreeSet<String>,
    /// Types and constants, which an expression may name as a value.
    values: BTreeSet<String>,
    public: BTreeSet<String>,
    /// Functions other objects call by name, which no module qualifies.
    exported: BTreeSet<String>,
}

impl Declared {
    fn of(module: &Module) -> Self {
        let values: BTreeSet<String> = module
            .structs
            .iter()
            .map(|one| &one.name)
            .chain(module.enums.iter().map(|one| &one.name))
            .chain(module.protocols.iter().map(|one| &one.name))
            .chain(module.consts.iter().map(|one| &one.name))
            .cloned()
            .collect();
        let functions = module
            .functions
            .iter()
            .map(|one| &one.name)
            .chain(module.externs.iter().map(|one| &one.function.name));
        let names = values.iter().cloned().chain(functions.cloned()).collect();
        let methods = module.functions.iter().filter(|one| one.is_method()).map(|one| one.name.clone()).collect();
        Self {
            names,
            methods,
            values,
            public: module.public.clone(),
            exported: module.exports.keys().cloned().collect(),
        }
    }

    /// The name `module` declares `name` under, in the linked program.
    fn linked(&self, module: &str, name: &str) -> String {
        if module.is_empty() || self.exported.contains(name) {
            name.to_owned()
        } else {
            format!("{module}.{name}")
        }
    }
}

fn link(mut loaded: BTreeMap<String, Module>, order: &[String]) -> Result<Module, Located> {
    let declared: BTreeMap<String, Declared> = loaded
        .iter()
        .map(|(name, module)| (name.clone(), Declared::of(module)))
        .collect();
    let mut linked = Module::default();
    for name in order {
        let mut module = loaded.remove(name).expect("loaded");
        let resolver = Resolver {
            module: name,
            imports: &module.imports.clone(),
            declared: &declared,
        };
        resolver
            .module(&mut module)
            .map_err(|error| (name.clone(), error))?;
        linked.fixed_types.extend(module.fixed_types);
        linked.consts.extend(module.consts);
        linked.structs.extend(module.structs);
        linked.enums.extend(module.enums);
        linked.protocols.extend(module.protocols);
        linked.functions.extend(module.functions);
        linked.externs.extend(module.externs);
        linked.exports.extend(module.exports);
    }
    Ok(linked)
}

/// Resolves the names one module uses to the declarations they mean.
struct Resolver<'a> {
    module: &'a str,
    imports: &'a [Import],
    declared: &'a BTreeMap<String, Declared>,
}

impl Resolver<'_> {
    /// `path`'s declaration, qualified: one of this module's own, or
    /// `alias.name` for a public one of a module it imports.
    fn resolve(&self, path: &str, span: Span) -> Result<Option<String>, Diagnostic> {
        if self.declared[self.module].names.contains(path) {
            return Ok(Some(self.declared[self.module].linked(self.module, path)));
        }
        for import in self.imports {
            let Some(rest) = path.strip_prefix(&format!("{}.", import.name)) else {
                continue;
            };
            let target = &self.declared[&import.module];
            if !target.names.contains(rest) {
                continue;
            }
            if !target.public.contains(rest) {
                return Err(Diagnostic::new(
                    span,
                    format!("{rest} is private to module {}", import.module),
                ));
            }
            return Ok(Some(target.linked(&import.module, rest)));
        }
        Ok(None)
    }

    /// Whether `path` names a method, `Type.name`: only a value calls it (section 5).
    fn method(&self, path: &str) -> bool {
        let own = self.declared[self.module].methods.contains(path);
        own || self.imports.iter().any(|import| {
            path.strip_prefix(&format!("{}.", import.name))
                .is_some_and(|rest| self.declared[&import.module].methods.contains(rest))
        })
    }

    fn rename(&self, name: &mut String, span: Span) -> Result<(), Diagnostic> {
        if let Some(resolved) = self.resolve(name, span)? {
            *name = resolved;
        }
        Ok(())
    }

    fn module(&self, module: &mut Module) -> Result<(), Diagnostic> {
        let declared = &self.declared[self.module];
        let own = |name: &mut String| *name = declared.linked(self.module, name);
        for one in &mut module.structs {
            own(&mut one.name);
            for field in &mut one.fields {
                self.spec(&mut field.type_spec, field.span)?;
            }
        }
        for one in &mut module.enums {
            own(&mut one.name);
            for field in one
                .variants
                .iter_mut()
                .flat_map(|variant| &mut variant.fields)
            {
                self.spec(&mut field.type_spec, field.span)?;
            }
        }
        for one in &mut module.protocols {
            own(&mut one.name);
        }
        for one in &mut module.consts {
            own(&mut one.name);
        }
        for function in module
            .functions
            .iter_mut()
            .chain(module.externs.iter_mut().map(|one| &mut one.function))
        {
            own(&mut function.name);
            let span = function.span;
            for generic in &mut function.generics {
                if let Some(bound) = &mut generic.bound {
                    self.rename(bound, span)?;
                }
            }
            for parameter in &mut function.parameters {
                match &mut parameter.type_ {
                    super::syntax::ParameterType::Owned(annotation)
                    | super::syntax::ParameterType::Borrowed {
                        target: annotation, ..
                    } => self.annotation(annotation, span)?,
                }
                if let Some(default) = &mut parameter.default {
                    self.expression(default)?;
                }
            }
            self.annotation(&mut function.result, span)?;
            for statement in &mut function.body {
                self.statement(statement)?;
            }
        }
        Ok(())
    }

    fn statement(&self, statement: &mut Statement) -> Result<(), Diagnostic> {
        let mut result = Ok(());
        statement.each_mut(&mut |one| {
            if result.is_ok() {
                result = self.own_parts(one);
            }
        });
        result
    }

    /// A statement's annotation, patterns, and expressions, not its blocks'.
    fn own_parts(&self, statement: &mut Statement) -> Result<(), Diagnostic> {
        let span = statement.span();
        match statement {
            Statement::Bind {
                annotation: Some(annotation),
                ..
            } => self.annotation(annotation, span)?,
            Statement::Destructure { pattern, .. } => self.pattern(pattern)?,
            Statement::Match { arms, .. } => arms
                .iter_mut()
                .try_for_each(|arm| self.pattern(&mut arm.pattern))?,
            _ => {}
        }
        statement
            .own_expressions_mut()
            .into_iter()
            .try_for_each(|expression| self.expression(expression))
    }

    fn expression(&self, expression: &mut Expr) -> Result<(), Diagnostic> {
        expression.walk_mut(&mut |one| self.reference(one))
    }

    /// Resolves the one expression `expression` is, its parts already done.
    fn reference(&self, expression: &mut Expr) -> Result<(), Diagnostic> {
        match expression {
            Expr::Call { name, span, .. } | Expr::StructLiteral { name, span, .. } => {
                self.rename(name, *span)?
            }
            Expr::Variant {
                enum_name: Some(name),
                span,
                ..
            } => self.rename(name, *span)?,
            // A type used as a value, as `Mode` in `Mode.text`; any other
            // name is a local.
            Expr::Name(name, span) if self.declared[self.module].values.contains(name.as_str()) => {
                self.rename(name, *span)?
            }
            // `alias.name`: a path through an import.
            Expr::Member { base, field, span } => {
                if let Some(path) = dotted(base) {
                    if let Some(resolved) = self.resolve(&format!("{path}.{field}"), *span)? {
                        *expression = Expr::Name(resolved, *span);
                    }
                }
            }
            Expr::MethodCall {
                receiver,
                name,
                type_arguments,
                arguments,
                span,
            } => {
                if let Some(path) = dotted(receiver) {
                    if self.method(&format!("{path}.{name}")) {
                        return Err(method_without_value(&path, name, *span));
                    }
                    if let Some(resolved) = self.resolve(&format!("{path}.{name}"), *span)? {
                        *expression = Expr::Call {
                            name: resolved,
                            type_arguments: std::mem::take(type_arguments),
                            arguments: std::mem::take(arguments),
                            span: *span,
                        };
                    }
                }
            }
            // A lambda's body is its own expression, compiled where it is called.
            Expr::Lambda { body, .. } => self.expression(body)?,
            _ => {}
        }
        Ok(())
    }

    fn pattern(&self, pattern: &mut Pattern) -> Result<(), Diagnostic> {
        match pattern {
            Pattern::Variant {
                enum_name: Some(name),
                fields,
                span,
                ..
            }
            | Pattern::Struct { name, fields, span } => {
                self.rename(name, *span)?;
                fields.iter_mut().try_for_each(|one| self.pattern(one))
            }
            Pattern::Variant { fields, .. } | Pattern::Tuple(fields, _) => {
                fields.iter_mut().try_for_each(|one| self.pattern(one))
            }
            Pattern::Sequence { before, rest, after, .. } => before
                .iter_mut()
                .chain(rest.as_deref_mut())
                .chain(after)
                .try_for_each(|one| self.pattern(one)),
            Pattern::Wildcard(_) | Pattern::Binding(..) | Pattern::Literal(_) => Ok(()),
        }
    }

    fn annotation(&self, annotation: &mut TypeAnnotation, span: Span) -> Result<(), Diagnostic> {
        match annotation {
            TypeAnnotation::Value(spec)
            | TypeAnnotation::Slice { element: spec, .. }
            | TypeAnnotation::Array { element: spec, .. } => self.spec(spec, span),
        }
    }

    fn spec(&self, spec: &mut TypeSpec, span: Span) -> Result<(), Diagnostic> {
        match spec {
            TypeSpec::Primitive(_) => Ok(()),
            TypeSpec::Named(name) => self.rename(name, span),
            TypeSpec::Applied { name, args } => {
                self.rename(name, span)?;
                args.iter_mut()
                    .try_for_each(|one| self.annotation(one, span))
            }
        }
    }
}

/// `a.b.c` for a chain of names, as a receiver or member base spells it.
/// `Type.method(...)`: a method called with no value (section 5).
pub(crate) fn method_without_value(path: &str, name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(span, format!("{path}.{name} is a method; call it on a value: value.{name}(...)"))
}

fn dotted(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name, _) => Some(name.clone()),
        Expr::Member { base, field, .. } => Some(format!("{}.{field}", dotted(base)?)),
        _ => None,
    }
}
