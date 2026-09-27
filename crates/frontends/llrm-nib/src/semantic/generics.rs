//! Generic structs and enums, instantiated once per distinct argument list.

use super::*;
use crate::arguments::{self, Formal};
use crate::syntax::{Enum, GenericParameter, StructField};

#[derive(Clone, Debug)]
pub(super) enum Template {
    Struct(Struct),
    Enum(Enum),
}

impl TypeRegistry {
    /// `name[args]`, registered under its spelling on first use.
    pub(super) fn instantiate(
        &mut self,
        spec: &TypeSpec,
        span: Span,
    ) -> Result<ElementType, Diagnostic> {
        let TypeSpec::Applied { name, args } = spec else {
            unreachable!("only an applied type is instantiated")
        };
        let text = spec.text();
        if !self.declared(&text) {
            let template =
                self.templates.get(name).cloned().ok_or_else(|| {
                    Diagnostic::new(span, format!("{name:?} is not a generic type"))
                })?;
            let parameters = match &template {
                Template::Struct(one) => &one.generics,
                Template::Enum(one) => &one.generics,
            };
            if parameters.len() != args.len() {
                return Err(Diagnostic::new(
                    span,
                    format!("{name} takes {} type arguments", parameters.len()),
                ));
            }
            let bound: BTreeMap<String, TypeAnnotation> = parameters.iter().cloned().zip(args.iter().cloned()).collect();
            match template {
                Template::Struct(mut one) => {
                    one.name = text.clone();
                    one.generics.clear();
                    one.fields = substituted(one.fields, &bound)?;
                    self.register_struct(&one)?;
                }
                Template::Enum(mut one) => {
                    one.name = text.clone();
                    one.generics.clear();
                    for variant in &mut one.variants {
                        variant.fields = substituted(std::mem::take(&mut variant.fields), &bound)?;
                    }
                    self.register_enum(&one)?;
                }
            }
            self.applied.insert(text.clone(), spec.clone());
        }
        self.resolve_element(&TypeSpec::Named(text), span)
    }

    /// The generic type `name` instantiates, or `name` itself.
    pub(super) fn template_of<'n>(&'n self, name: &'n str) -> &'n str {
        match self.applied.get(name) {
            Some(TypeSpec::Applied { name, .. }) => name,
            _ => name,
        }
    }

    /// `function`, generic also in its owner's parameters when it is a
    /// method of a generic type.
    pub(super) fn with_owner_generics(&self, function: &Function) -> Function {
        let mut function = function.clone();
        let parameters = match function.name.split_once('.').and_then(|(owner, _)| self.templates.get(owner)) {
            Some(Template::Struct(one)) => one.generics.clone(),
            Some(Template::Enum(one)) => one.generics.clone(),
            None => return function,
        };
        for name in parameters {
            if !function.generics.iter().any(|one| one.name == name) {
                function.generics.push(GenericParameter { name, bound: None });
            }
        }
        function
    }

    /// The applied type `element` is an instance of, if it is one.
    pub(super) fn applied_of(&self, element: ElementType) -> Option<&TypeSpec> {
        let ElementType::Struct(id) = element else {
            return None;
        };
        self.applied.get(&self.structure(id)?.name)
    }
}

/// Fields with type parameters replaced; a `void` field, as in
/// `Result[void, E]`, has no storage, and a field whose type is a parameter
/// bound to a fixed array is an array field.
fn substituted(fields: Vec<StructField>, bound: &BTreeMap<String, TypeAnnotation>) -> Result<Vec<StructField>, Diagnostic> {
    let mut out = Vec::new();
    for mut field in fields {
        match substitute_in(&TypeAnnotation::Value(field.type_spec.clone()), bound) {
            TypeAnnotation::Value(spec) => field.type_spec = spec,
            TypeAnnotation::Array { element, dims } if field.dims.is_empty() => {
                field.type_spec = element;
                field.dims = dims;
            }
            _ => return Err(Diagnostic::new(field.span, "an array of arrays is written as one ranked array")),
        }
        if field.type_spec != TypeSpec::Primitive(TypeName::Void) {
            out.push(field);
        }
    }
    Ok(out)
}

/// `spec` with each type parameter replaced by the type it is bound to.
pub(super) fn substitute(spec: &TypeSpec, bound: &BTreeMap<String, TypeSpec>) -> TypeSpec {
    let bound = bound.iter().map(|(name, one)| (name.clone(), TypeAnnotation::Value(one.clone()))).collect();
    let TypeAnnotation::Value(spec) = substitute_in(&TypeAnnotation::Value(spec.clone()), &bound) else {
        unreachable!("a type bound to types")
    };
    spec
}

/// `annotation` with each type parameter replaced by the type, possibly a
/// fixed array, it is bound to.
pub(super) fn substitute_in(annotation: &TypeAnnotation, bound: &BTreeMap<String, TypeAnnotation>) -> TypeAnnotation {
    let element_of = |element: &TypeSpec| match substitute_in(&TypeAnnotation::Value(element.clone()), bound) {
        TypeAnnotation::Value(one) => one,
        _ => element.clone(),
    };
    match annotation {
        TypeAnnotation::Value(TypeSpec::Named(name)) => bound.get(name).cloned().unwrap_or_else(|| annotation.clone()),
        TypeAnnotation::Value(TypeSpec::Applied { name, args }) => TypeAnnotation::Value(TypeSpec::Applied {
            name: name.clone(),
            args: args.iter().map(|arg| substitute_in(arg, bound)).collect(),
        }),
        TypeAnnotation::Value(TypeSpec::Primitive(_)) => annotation.clone(),
        TypeAnnotation::Slice { element, rank } => TypeAnnotation::Slice { element: element_of(element), rank: *rank },
        TypeAnnotation::Array { element, dims } => TypeAnnotation::Array { element: element_of(element), dims: dims.clone() },
    }
}

/// The named types a field type refers to, its type arguments included.
pub(super) fn named_in(spec: &TypeSpec) -> Vec<&str> {
    match spec {
        TypeSpec::Named(name) => vec![name.as_str()],
        TypeSpec::Applied { args, .. } => args
            .iter()
            .flat_map(|arg| match arg {
                TypeAnnotation::Value(one)
                | TypeAnnotation::Slice { element: one, .. }
                | TypeAnnotation::Array { element: one, .. } => named_in(one),
            })
            .collect(),
        TypeSpec::Primitive(_) => Vec::new(),
    }
}

impl FunctionCompiler<'_> {
    /// `Pair(1, "two")` or `Pair[u8, bool](...)`: a literal of the instance
    /// its type arguments select, given or implied by its field values.
    /// `None` when `call` builds no generic struct.
    pub(super) fn generic_literal(&mut self, call: &Expr) -> Result<Option<Expr>, Diagnostic> {
        let Expr::Call { name, type_arguments, arguments, span } = call else {
            return Ok(None);
        };
        let Some(Template::Struct(template)) = self.types.templates.get(name).cloned() else {
            return Ok(None);
        };
        let values = bound_fields(name, &template.fields, arguments, *span)?;
        let patterns = template.fields.iter().map(|field| &field.type_spec).zip(&values);
        let instance = self.instance_named(name, &template.generics, type_arguments, patterns, *span)?;
        let fields = template
            .fields
            .iter()
            .zip(values.iter().cloned())
            .map(|(field, value)| (field.name.clone(), value.clone(), value.span()))
            .collect();
        Ok(Some(Expr::StructLiteral { name: instance.unwrap_or_else(|| name.clone()), fields, span: *span }))
    }

    /// The instance of the generic enum `enum_name` a variant's payload
    /// implies, when it implies one.
    pub(super) fn generic_variant(&mut self, enum_name: &str, variant: &str, arguments: &[Expr], span: Span) -> Result<Option<String>, Diagnostic> {
        let Some(Template::Enum(template)) = self.types.templates.get(enum_name).cloned() else {
            return Ok(None);
        };
        let Some(variant) = template.variants.iter().find(|one| one.name == variant) else {
            return Ok(None);
        };
        // `Result.ok()` of a `Result[void, E]` has no payload to bind.
        let Ok(values) = bound_fields(enum_name, &variant.fields, arguments, span) else {
            return Ok(None);
        };
        let patterns = variant.fields.iter().map(|field| &field.type_spec).zip(&values);
        self.instance_named(enum_name, &template.generics, &[], patterns, span)
    }

    /// `name[...]` of the `given` types, else of those `values` imply;
    /// `None` when one cannot be inferred, so an expected type decides.
    fn instance_named<'e>(
        &mut self,
        name: &str,
        parameters: &[String],
        given: &[TypeSpec],
        values: impl Iterator<Item = (&'e TypeSpec, &'e Expr)>,
        span: Span,
    ) -> Result<Option<String>, Diagnostic> {
        let args = if given.is_empty() {
            let generics: Vec<&str> = parameters.iter().map(String::as_str).collect();
            let mut bound = BTreeMap::new();
            for (pattern, value) in values {
                if let Some(actual) = self.element_hint(value) {
                    self.unify(pattern, actual, &generics, &mut bound);
                }
            }
            let Some(args) = parameters.iter().map(|one| bound.get(one).cloned()).collect::<Option<Vec<_>>>() else {
                return Ok(None);
            };
            args
        } else {
            given.to_vec()
        };
        let spec = TypeSpec::Applied { name: name.to_owned(), args: args.into_iter().map(TypeAnnotation::Value).collect() };
        self.types.instantiate(&spec, span)?;
        Ok(Some(spec.text()))
    }
}

/// `arguments`, positional or named, in the order of `fields`.
fn bound_fields(name: &str, fields: &[StructField], arguments: &[Expr], span: Span) -> Result<Vec<Expr>, Diagnostic> {
    let formals: Vec<_> = fields.iter().map(|field| Formal { name: &field.name, default: None }).collect();
    arguments::bind(name, &formals, arguments.to_vec(), span)
}
