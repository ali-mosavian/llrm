//! Modules: each file is one, named by its path under the source root, as
//! `graphics.sprite`. Loading follows the imports from the main module;
//! linking makes one module of them all, each imported module's declarations
//! qualified with its name and every reference to one resolved.

use std::collections::{BTreeMap, BTreeSet};

use super::error::Diagnostic;
use super::lexer::{Token, lex};
use super::parser::{imports, parse_after};
use super::scopes;
use super::standard;
use super::syntax::{Expr, Function, Import, Module, Pattern, Span, Statement, TypeAnnotation, TypeName, TypeSpec};

/// A diagnostic and the module it is in; the main module is `""`.
pub type Located = (String, Diagnostic);

/// The extension of a module's file.
pub const EXTENSION: &str = "nib";

/// The file under the source root `root` that module `name` is read from:
/// `a.b` from `a/b.nib`.
pub fn file(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    root.join(format!("{}.{EXTENSION}", name.replace('.', "/")))
}

/// The main module `source` and every module it imports, linked as one.
/// `read` gives the source of a module by its name.
pub fn load(
    source: &str,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<Module, Located> {
    read_all(source, read)?.linked()
}

/// The main module and every module it imports, each parsed as written.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// Each module by name; the main module is `""`.
    pub modules: BTreeMap<String, Module>,
    /// The modules' names, which spans name by index.
    pub sources: Vec<String>,
    /// Each module once, imported before importer.
    order: Vec<String>,
}

/// `load` before linking.
pub fn read_all(
    source: &str,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<Loaded, Located> {
    let mut modules = BTreeMap::new();
    let mut sources = Sources::default();
    let main = lexed("", source, &mut sources)?;
    let mut order = Vec::new();
    visit("", main, &mut Vec::new(), &mut modules, &mut order, &mut sources, read)?;
    Ok(Loaded { modules, sources: sources.names, order })
}

impl Loaded {
    /// The modules linked as one.
    pub fn linked(self) -> Result<Module, Located> {
        let mut linked = link(self.modules, &self.order)?;
        linked.sources = self.sources;
        linked.fixed_types.sort_by_key(|one| match one.type_name {
            TypeName::Fixed { declaration, .. } => declaration,
            _ => unreachable!("a fixed-point type"),
        });
        Ok(linked)
    }
}

/// `name`'s tokens, each span marked with its place in `sources`.
fn lexed(name: &str, source: &str, sources: &mut Sources) -> Result<Vec<Token>, Located> {
    let module = u16::try_from(sources.names.len()).expect("fewer modules than a u16 counts");
    sources.names.push(name.to_owned());
    let mut tokens = lex(source).map_err(|error| (name.to_owned(), error))?;
    for token in &mut tokens {
        token.span.module = module;
    }
    Ok(tokens)
}

/// `name`'s module, its fixed-point types numbered after those of the
/// modules parsed before it; `loaded` holds every module it imports.
fn parsed(name: &str, tokens: Vec<Token>, imports: &[Import], loaded: &BTreeMap<String, Module>, sources: &mut Sources) -> Result<Module, Located> {
    // An imported module's public constants, by their path here.
    let imported = imports
        .iter()
        .flat_map(|import| {
            let module = &loaded[&import.module];
            module
                .consts
                .iter()
                .filter(|one| module.public.contains(&one.name))
                .map(|one| (format!("{}.{}", import.name, one.name), one.value.clone()))
        })
        .collect();
    let parsed = parse_after(tokens, sources.fixed_types, &imported).map_err(|error| (name.to_owned(), error))?;
    sources.fixed_types += parsed.fixed_types.len() as u16;
    Ok(parsed)
}

/// The modules parsed so far: their names, which spans name by index, and
/// how many fixed-point types they declare.
#[derive(Default)]
struct Sources {
    names: Vec<String>,
    fixed_types: u16,
}

/// Loads the module `name` and what it imports, depth first, each parsed
/// after what it imports; `order` lists each once, imported before importer.
fn visit(
    name: &str,
    tokens: Vec<Token>,
    chain: &mut Vec<String>,
    loaded: &mut BTreeMap<String, Module>,
    order: &mut Vec<String>,
    sources: &mut Sources,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<(), Located> {
    chain.push(name.to_owned());
    let imports = imports(&tokens).map_err(|error| (name.to_owned(), error))?;
    for import in &imports {
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
        let source = if standard::supplied(&import.module) {
            standard::source(&import.module).map(str::to_owned).ok_or_else(|| "the compiler supplies no such module".to_owned())
        } else {
            read(&import.module)
        }
        .map_err(|error| {
            (
                name.to_owned(),
                Diagnostic::new(
                    import.span,
                    format!("cannot import {}: {error}", import.module),
                ),
            )
        })?;
        let imported = lexed(&import.module, &source, sources)?;
        visit(&import.module, imported, chain, loaded, order, sources, read)?;
    }
    chain.pop();
    let module = parsed(name, tokens, &imports, loaded, sources)?;
    loaded.insert(name.to_owned(), module);
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
            .chain(module.statics.iter().map(|one| &one.name))
            .chain(module.fixed_types.iter().map(|one| &one.name))
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
        }
    }

    /// The name `module` declares `name` under, in the linked program.
    fn linked(&self, module: &str, name: &str) -> String {
        if module.is_empty() {
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
    // A fixed-point type named from another module is the type itself, as
    // the parser makes one its own module names.
    let fixed: BTreeMap<String, TypeName> = loaded
        .iter()
        .flat_map(|(name, module)| module.fixed_types.iter().map(|one| (declared[name].linked(name, &one.name), one.type_name)))
        .collect();
    let mut linked = Module::default();
    for name in order {
        let mut module = loaded.remove(name).expect("loaded");
        let own = &declared[name];
        linked.private_methods.extend(
            module
                .functions
                .iter()
                .filter(|one| one.is_method() && !own.public.contains(&one.name))
                .map(|one| (own.linked(name, &one.name), one.span.module)),
        );
        let resolver = Resolver {
            module: name,
            imports: &module.imports.clone(),
            declared: &declared,
            fixed: &fixed,
        };
        resolver
            .module(&mut module)
            .map_err(|error| (name.clone(), error))?;
        linked.fixed_types.extend(module.fixed_types.into_iter().map(|mut one| {
            one.name = own.linked(name, &one.name);
            one
        }));
        linked.consts.extend(module.consts);
        linked.statics.extend(module.statics);
        linked.structs.extend(module.structs);
        linked.enums.extend(module.enums);
        linked.protocols.extend(module.protocols);
        linked.functions.extend(module.functions);
        linked.externs.extend(module.externs);
        // An export's symbol is its own; only its source name is qualified.
        linked.exports.extend(module.exports.into_iter().map(|(function, export)| (own.linked(name, &function), export)));
    }
    Ok(linked)
}

/// Resolves the names one module uses to the declarations they mean.
struct Resolver<'a> {
    module: &'a str,
    imports: &'a [Import],
    declared: &'a BTreeMap<String, Declared>,
    fixed: &'a BTreeMap<String, TypeName>,
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
            one.methods.iter_mut().try_for_each(|method| self.header(method))?;
        }
        for one in &mut module.consts {
            own(&mut one.name);
        }
        for one in &mut module.statics {
            own(&mut one.name);
            self.annotation(&mut one.annotation, one.span)?;
            self.expression(&mut one.value)?;
        }
        for function in module
            .functions
            .iter_mut()
            .chain(module.externs.iter_mut().map(|one| &mut one.function))
        {
            own(&mut function.name);
            self.header(function)?;
            for parameter in &function.parameters {
                self.unshadowed(&parameter.name, function.span)?;
            }
            for statement in &mut function.body {
                self.statement(statement)?;
            }
            let mut locals = function.parameters.iter().map(|one| one.name.clone()).collect();
            scopes::walk_mut(&mut function.body, &mut locals, &mut |one, locals| self.reference(one, locals))?;
        }
        Ok(())
    }

    /// A function's bounds, parameters and result: all a protocol's method has.
    fn header(&self, function: &mut Function) -> Result<(), Diagnostic> {
        let span = function.span;
        for generic in &mut function.generics {
            if let Some(bound) = &mut generic.bound {
                self.spec(bound, span)?;
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
        self.annotation(&mut function.result, span)
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

    /// Refuses a local named as an import: an import alias cannot be shadowed (section 14).
    fn unshadowed(&self, name: &str, span: Span) -> Result<(), Diagnostic> {
        let root = |import: &str| import.split('.').next().unwrap_or(import).to_owned();
        if self.imports.iter().any(|import| root(&import.name) == name) {
            return Err(Diagnostic::new(span, format!("{name:?} would shadow an import")));
        }
        Ok(())
    }

    /// A statement's annotation, patterns and the names it binds, not its blocks'.
    fn own_parts(&self, statement: &mut Statement) -> Result<(), Diagnostic> {
        let span = statement.span();
        let bound: Vec<String> = match &*statement {
            Statement::Bind { name, .. } | Statement::For { name, .. } | Statement::ForRange { name, .. } => vec![name.clone()],
            Statement::Destructure { pattern, .. } => pattern.names().into_iter().map(str::to_owned).collect(),
            Statement::Match { arms, .. } => arms.iter().flat_map(|arm| arm.pattern.names()).map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        bound.iter().try_for_each(|name| self.unshadowed(name, span))?;
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
        Ok(())
    }

    fn expression(&self, expression: &mut Expr) -> Result<(), Diagnostic> {
        scopes::expression_walk_mut(expression, &mut Vec::new(), &mut |one, locals| self.reference(one, locals))
    }

    /// Resolves the one expression `expression` is, its parts already done.
    /// `expression`'s names as the linked program declares them; `locals`
    /// hide the module's own.
    fn reference(&self, expression: &mut Expr, locals: &[String]) -> Result<(), Diagnostic> {
        match expression {
            // A call keeps naming the module's function: whether a local of
            // its name holds a function value is a type fact this pass lacks.
            Expr::Name(name, _) if locals.contains(name) => {}
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
                if let Some(path) = base.dotted() {
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
                if let Some(path) = receiver.dotted() {
                    if self.method(&format!("{path}.{name}")) {
                        return Err(method_without_value(&path, name, *span));
                    }
                    if let Some(resolved) = self.resolve(&format!("{path}.{name}"), *span)? {
                        if let Some(&target) = self.fixed.get(&resolved) {
                            let [value] = std::mem::take(arguments).try_into().map_err(|_| Diagnostic::new(*span, "a conversion takes one value"))?;
                            *expression = Expr::Conversion { target, value: Box::new(value), span: *span };
                            return Ok(());
                        }
                        *expression = Expr::Call {
                            name: resolved,
                            type_arguments: std::mem::take(type_arguments),
                            arguments: std::mem::take(arguments),
                            span: *span,
                        };
                    }
                }
            }
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
            TypeSpec::Named(name) => {
                self.rename(name, span)?;
                if let Some(&fixed) = self.fixed.get(name.as_str()) {
                    *spec = TypeSpec::Primitive(fixed);
                }
                Ok(())
            }
            TypeSpec::Applied { name, args } => {
                self.rename(name, span)?;
                args.iter_mut()
                    .try_for_each(|one| self.annotation(one, span))
            }
        }
    }
}

/// `Type.method(...)`: a method called with no value (section 5).
pub(crate) fn method_without_value(path: &str, name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(span, format!("{path}.{name} is a method; call it on a value: value.{name}(...)"))
}
