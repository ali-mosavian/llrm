//! Generic functions: a template is instantiated once per set of type
//! arguments, which a call's argument types decide. The instance is an
//! ordinary function named `name[T, ...]`, compiled after the module's own.

use super::*;
use crate::frontends::modern::arguments::{self, Formal};
use crate::frontends::modern::syntax::{GenericParameter, Protocol};

pub(super) struct Templates {
    functions: BTreeMap<String, Function>,
    /// Generators, inlined where a `for` consumes them.
    generators: BTreeMap<String, Function>,
    protocols: BTreeMap<String, Protocol>,
    /// The library's methods, instantiated as they are called.
    library: BTreeMap<String, Function>,
    instances: BTreeMap<String, Signature>,
    pending: Vec<Function>,
    next_id: u32,
}

impl Templates {
    pub(super) fn new(
        functions: Vec<&Function>,
        generators: Vec<&Function>,
        protocols: &[Protocol],
        library: &[Function],
        next_id: u32,
    ) -> Self {
        Self {
            functions: functions
                .into_iter()
                .map(|one| (one.name.clone(), one.clone()))
                .collect(),
            generators: generators
                .into_iter()
                .map(|one| (one.name.clone(), one.clone()))
                .collect(),
            protocols: protocols
                .iter()
                .map(|one| (one.name.clone(), one.clone()))
                .collect(),
            library: library.iter().map(|one| (one.name.clone(), one.clone())).collect(),
            instances: BTreeMap::new(),
            pending: Vec::new(),
            next_id,
        }
    }

    pub(super) fn is_template(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub(super) fn generator(&self, name: &str) -> Option<&Function> {
        self.generators.get(name)
    }

    /// A generator the compiler makes, as a generator expression's.
    pub(super) fn add_generator(&mut self, function: Function) {
        self.generators.insert(function.name.clone(), function);
    }

    pub(super) fn instance(&self, name: &str) -> Option<Signature> {
        self.instances.get(name).cloned()
    }

    /// An instance still to compile, with its signature.
    pub(super) fn next_pending(&mut self) -> Option<(Function, Signature)> {
        let function = self.pending.pop()?;
        let signature = self.instances[&function.name].clone();
        Some((function, signature))
    }

    /// A function the compiler made, such as a lifted lambda, to compile
    /// after the module's own.
    pub(super) fn generated(&mut self, function: Function, types: &mut TypeRegistry) -> Result<(), Diagnostic> {
        self.declared(function.clone(), types)?;
        self.pending.push(function);
        Ok(())
    }

    /// A function the compiler makes whose body comes later: callable now.
    pub(super) fn declared(&mut self, header: Function, types: &mut TypeRegistry) -> Result<(), Diagnostic> {
        let signature = signature(types, &header, self.next_id)?;
        self.next_id += 1;
        self.instances.insert(header.name.clone(), signature);
        Ok(())
    }

    pub(super) fn callables(&self, types: &mut TypeRegistry) -> Vec<hir::Callable> {
        let mut signatures: Vec<&Signature> = self.instances.values().collect();
        signatures.sort_by_key(|one| one.id);
        signatures
            .into_iter()
            .map(|signature| signature.callable(types))
            .collect()
    }
}

impl FunctionCompiler<'_> {
    /// Readies `statement`'s own expressions, innermost first: registers
    /// each tuple literal's type, and renames each generic call to the
    /// instance its arguments select. `None` when nothing was renamed.
    pub(super) fn prepared(
        &mut self,
        statement: &Statement,
    ) -> Result<Option<Statement>, Diagnostic> {
        let mut rewritten = statement.clone();
        let mut renamed = false;
        self.consumed = consumed_generators(&rewritten);
        // `p[i] = v` through a raw pointer writes `*(p.offset(i))`.
        if let Statement::Assign { target: target @ AssignTarget::Index { .. }, span, .. } = &mut rewritten {
            let AssignTarget::Index { base, indices } = target else { unreachable!() };
            if let Some(offset) = self.pointer_index(base, indices, *span) {
                let Expr::Unary { operand, .. } = offset else { unreachable!("a dereference") };
                *target = AssignTarget::Deref(*operand);
                renamed = true;
            } else if let Some(Expr::Member { base, field, .. }) = self.tuple_element(base, indices, *span) {
                *target = AssignTarget::Member { base: *base, field };
                renamed = true;
            }
        }
        for expression in rewritten.own_expressions_mut() {
            renamed |= self.prepare_expression(expression)?;
        }
        Ok(renamed.then_some(rewritten))
    }

    /// The same for one expression; whether it renamed a call.
    pub(super) fn prepare_expression(&mut self, expression: &mut Expr) -> Result<bool, Diagnostic> {
        let mut renamed = false;
        expression.walk_mut(&mut |one| {
            if let Expr::MethodCall { receiver, name, type_arguments, span, .. } = one {
                self.visible_method(receiver, name, *span)?;
                self.declare_cast(receiver, name, type_arguments, *span)?;
            }
            if let Expr::Index { base, indices, span } = one {
                if let Some(read) = self.pointer_index(base, indices, *span).or_else(|| self.tuple_element(base, indices, *span)) {
                    *one = read;
                    renamed = true;
                }
            }
            // `.iter()` of an array, vec or view, which declares none (section 12).
            if let Expr::MethodCall { receiver, name, arguments, span, .. } = one {
                let declared = self.receiver_type(receiver).is_some_and(|owner| self.known_signature(&format!("{owner}.iter")).is_some());
                if name == "iter" && arguments.is_empty() && !declared && self.iterated_item(receiver).is_some() {
                    *one = Expr::Call { name: "elements".into(), type_arguments: Vec::new(), arguments: vec![(**receiver).clone()], span: *span };
                    renamed = true;
                }
            }
            // A generator no loop consumes escapes: it is its state.
            if let Expr::Generator { element, clauses, span } = one {
                if !self.consumed.contains(span) {
                    *one = self.escaping_generator_expression(element, clauses, *span)?;
                    renamed = true;
                }
            }
            if let Expr::Call { name, arguments, span, .. } = one {
                if self.is_generator_call(name) && !self.consumed.contains(span) {
                    *one = self.escaping_generator(name, arguments, *span)?;
                    renamed = true;
                }
            }
            if let Some(call) = self.generic_method_call(one) {
                *one = call;
            }
            if let Some(literal) = self.generic_literal(one)? {
                *one = literal;
                renamed = true;
            }
            if let Expr::Variant { enum_name: Some(enum_name), name, arguments, span } = one {
                if let Some(instance) = self.generic_variant(enum_name, name, arguments, *span)? {
                    *enum_name = instance;
                    renamed = true;
                }
            }
            if let Expr::Call { name, arguments, span, .. } = one {
                if let Some(call) = self.dispatched_call(name, arguments, *span)? {
                    *one = call;
                    renamed = true;
                }
            }
            if let Expr::Call {
                name,
                type_arguments,
                arguments,
                span,
            } = one
            {
                if self
                    .templates
                    .borrow()
                    .functions
                    .contains_key(name.as_str())
                {
                    (*name, *arguments) = self.instance_for(name, type_arguments, arguments, *span)?;
                    type_arguments.clear();
                    renamed = true;
                } else if !type_arguments.is_empty() && name != super::calls::SIZE_OF {
                    return Err(Diagnostic::new(*span, format!("{name} takes no type arguments")));
                }
            }
            if let Expr::MethodCall { receiver, name, type_arguments, .. } = one {
                // `x.checked_to[i8]()` names the method `checked_to[i8]`; a
                // raw pointer's `cast[U]` keeps its argument.
                let raw = self.expression_type_hint(receiver).and_then(|one| self.types.raw_target(one)).is_some();
                if !type_arguments.is_empty() && !raw {
                    *name = instance_name(name, type_arguments);
                    type_arguments.clear();
                    renamed = true;
                }
                if let Some(owner) = self.receiver_type(receiver) {
                    self.library_method(&format!("{owner}.{name}"))?;
                }
            }
            self.declare_tuple(one)
        })?;
        Ok(renamed)
    }

    /// Readies the library method `name`, if it is one, to be called.
    fn library_method(&mut self, name: &str) -> Result<(), Diagnostic> {
        let mut templates = self.templates.borrow_mut();
        let Some(function) = templates.library.get(name).cloned() else {
            return Ok(());
        };
        if templates.instances.contains_key(name) {
            return Ok(());
        }
        let signature = signature(self.types, &function, templates.next_id)?;
        templates.next_id += 1;
        templates.instances.insert(name.into(), signature);
        templates.pending.push(function);
        Ok(())
    }

    /// The instance of template `name` that `arguments` call, and the
    /// arguments it takes: a lambda is not passed but built into it.
    fn instance_for(
        &mut self,
        name: &str,
        given: &[TypeSpec],
        arguments: &[Expr],
        span: Span,
    ) -> Result<(String, Vec<Expr>), Diagnostic> {
        let mut template = self.templates.borrow().functions[name].clone();
        if given.len() > template.generics.len() {
            return Err(Diagnostic::new(
                span,
                format!("{name} takes {} type arguments", template.generics.len()),
            ));
        }
        let Inferred {
            mut bound,
            passed,
            lambdas,
        } = self.inferred(&template, arguments, span)?;
        for (generic, spec) in template.generics.iter().zip(given) {
            bound.insert(generic.name.clone(), spec.clone());
        }
        // Each lambda is a local of the instance, bound before its body.
        for (_, lambda, scopes) in &lambdas {
            if let Some(name) = lambdas::captured(lambda, scopes) {
                return Err(Diagnostic::new(
                    lambda.span(),
                    format!("a lambda passed to a function cannot capture {name:?}"),
                ));
            }
        }
        template
            .parameters
            .retain(|one| !lambdas.iter().any(|(name, _, _)| name == &one.name));
        let bindings = lambdas.into_iter().map(|(name, value, _)| Statement::Bind {
            mutable: false,
            name,
            annotation: None,
            value,
            span,
        });
        template.body = bindings.chain(template.body).collect();
        let chosen = self.chosen(&template, &bound, span)?;
        let instance = instance_name(name, &chosen);
        if self.templates.borrow().instances.contains_key(&instance) {
            return Ok((instance, passed));
        }
        let function = substituted_function(&template, &instance, &bound);
        let id = self.templates.borrow().next_id;
        let signature = signature(self.types, &function, id)?;
        let mut templates = self.templates.borrow_mut();
        templates.next_id += 1;
        templates.instances.insert(instance.clone(), signature);
        templates.pending.push(function);
        Ok((instance, passed))
    }

    /// What a call of `template` with `arguments` binds its type parameters
    /// to. A lambda argument is set apart with its parameter's name and the
    /// scopes it sees, and its type parameter bound to a fresh name.
    pub(super) fn inferred(
        &mut self,
        template: &Function,
        arguments: &[Expr],
        span: Span,
    ) -> Result<Inferred, Diagnostic> {
        let formals: Vec<_> = template
            .parameters
            .iter()
            .map(|one| Formal {
                name: &one.name,
                default: one.default.as_ref(),
            })
            .collect();
        let arguments = arguments::bind(&template.name, &formals, arguments.to_vec(), span)?;
        let generics: Vec<&str> = template
            .generics
            .iter()
            .map(|one| one.name.as_str())
            .collect();
        let mut inferred = Inferred {
            bound: BTreeMap::new(),
            passed: Vec::new(),
            lambdas: Vec::new(),
        };
        let mut literals = Vec::new();
        for (parameter, argument) in template.parameters.iter().zip(arguments) {
            if let (
                Some((lambda, scopes)),
                ParameterType::Owned(TypeAnnotation::Value(TypeSpec::Named(generic))),
            ) = (self.lambda_argument_expression(&argument), &parameter.type_)
            {
                let id = self.templates.borrow().next_id;
                self.templates.borrow_mut().next_id += 1;
                inferred
                    .bound
                    .insert(generic.clone(), TypeSpec::Named(format!("lambda{id}")));
                inferred
                    .lambdas
                    .push((parameter.name.clone(), lambda, scopes));
                continue;
            }
            if let Some(found) = self.argument_type(&parameter.type_, &argument) {
                // A literal takes its type from the others, as an operand does.
                if is_literal(&argument) {
                    literals.push(found);
                } else {
                    self.unify(&found.0, found.1, &generics, &mut inferred.bound);
                }
            }
            inferred.passed.push(argument);
        }
        for (pattern, actual) in literals {
            self.unify(&pattern, actual, &generics, &mut inferred.bound);
        }
        Ok(inferred)
    }

    /// A parameter's declared type and the type its argument has, when known.
    fn argument_type(
        &mut self,
        parameter: &ParameterType,
        argument: &Expr,
    ) -> Option<(TypeSpec, ElementType)> {
        let argument = match argument {
            Expr::Borrow { operand, .. } => operand.as_ref(),
            other => other,
        };
        let binding = match argument {
            Expr::Name(name, _) if self.visible(name).is_none() => {
                // A function passed as a value has its function type.
                if let Some(signature) = self.signatures.get(name).cloned() {
                    self.types.function_type(&signature);
                }
                None
            }
            Expr::Name(name, span) => self.binding(name, *span).ok().map(|one| one.type_),
            _ => None,
        };
        match parameter {
            ParameterType::Owned(TypeAnnotation::Value(spec))
            | ParameterType::Borrowed {
                target: TypeAnnotation::Value(spec),
                ..
            } => {
                let actual = match self.struct_expression_type(argument, argument.span()) {
                    Ok(Some(id)) => ElementType::Struct(id),
                    _ => self.element_hint(argument)?,
                };
                Some((spec.clone(), actual))
            }
            ParameterType::Borrowed {
                target:
                    TypeAnnotation::Slice { element, .. } | TypeAnnotation::Array { element, .. },
                ..
            } => {
                let actual = match binding {
                    Some(BindingType::Scalar(type_name)) => self.types.sequence_element(type_name)?,
                    Some(other) => other.ranked()?.0,
                    None => self.iterated_item(argument)?,
                };
                Some((element.clone(), actual))
            }
            _ => None,
        }
    }

    /// Binds the type parameters `pattern` names to the parts of `actual`.
    pub(super) fn unify(
        &self,
        pattern: &TypeSpec,
        actual: ElementType,
        generics: &[&str],
        bound: &mut BTreeMap<String, TypeSpec>,
    ) {
        match pattern {
            TypeSpec::Named(name) if generics.contains(&name.as_str()) => {
                bound
                    .entry(name.clone())
                    .or_insert_with(|| self.types.spec_of(actual));
            }
            TypeSpec::Applied { name, args } if name == "vec" => {
                if let (ElementType::Scalar(vector), [TypeAnnotation::Value(inner)]) =
                    (actual, args.as_slice())
                {
                    if let Some(element) = self.types.sequence_element(vector) {
                        self.unify(inner, element, generics, bound);
                    }
                }
            }
            TypeSpec::Applied { .. } if self.types.applied_of(actual).is_some() => {
                let applied = self.types.applied_of(actual).expect("checked").clone();
                unify_specs(pattern, &applied, generics, bound);
            }
            TypeSpec::Applied { name, args } if name == TUPLE => {
                let ElementType::Struct(id) = actual else {
                    return;
                };
                let layout = self.types.structure(id).expect("registered").clone();
                for (arg, field) in args.iter().zip(&layout.order) {
                    if let TypeAnnotation::Value(inner) = arg {
                        self.unify(inner, layout.fields[field].type_, generics, bound);
                    }
                }
            }
            _ => {}
        }
    }

    /// The type each of `template`'s parameters is bound to, in order;
    /// each must be bound, and satisfy its protocol.
    pub(super) fn chosen(
        &mut self,
        template: &Function,
        bound: &BTreeMap<String, TypeSpec>,
        span: Span,
    ) -> Result<Vec<TypeSpec>, Diagnostic> {
        let mut chosen = Vec::new();
        for generic in &template.generics {
            let spec = bound.get(&generic.name).cloned().ok_or_else(|| {
                Diagnostic::new(
                    span,
                    format!("cannot infer {} of {} from its arguments", generic.name, template.name),
                )
            })?;
            self.satisfies(generic, &spec, span)?;
            chosen.push(spec);
        }
        Ok(chosen)
    }

    /// Checks that the type chosen for `generic` has each method of its
    /// protocol, taking and giving the same types, with `Self` and the
    /// protocol's own type parameters bound.
    fn satisfies(
        &mut self,
        generic: &GenericParameter,
        spec: &TypeSpec,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let (name, arguments) = match &generic.bound {
            None => return Ok(()),
            Some(TypeSpec::Applied { name, args }) => (name, args.as_slice()),
            Some(TypeSpec::Named(name)) => (name, [].as_slice()),
            Some(other) => return Err(Diagnostic::new(span, format!("{} is not a protocol", other.text()))),
        };
        let protocol = self
            .templates
            .borrow()
            .protocols
            .get(name)
            .cloned()
            .ok_or_else(|| Diagnostic::new(span, format!("unknown protocol {name:?}")))?;
        if arguments.len() != protocol.generics.len() {
            return Err(Diagnostic::new(span, format!("{name} takes {} type arguments", protocol.generics.len())));
        }
        let mut bound = BTreeMap::from([("Self".to_owned(), spec.clone())]);
        for (parameter, argument) in protocol.generics.iter().zip(arguments) {
            let TypeAnnotation::Value(argument) = argument else {
                return Err(Diagnostic::new(span, format!("{name} takes a type, not an array")));
            };
            bound.insert(parameter.clone(), argument.clone());
        }
        let type_name = spec.text();
        for method in &protocol.methods {
            let wanted = signature(self.types, &substituted_function(method, &method.name, &bound), 0)?;
            let found = self.method_signature(&format!("{type_name}.{}", method.name))?;
            if !found.is_some_and(|one| one.matches(&wanted)) {
                return Err(Diagnostic::new(
                    span,
                    format!("{type_name} is not a {name}: it has no method {} as {name} declares it", method.name),
                ));
            }
        }
        Ok(())
    }

    /// The method `name`'s signature: a declared one's, or the library's.
    fn method_signature(&mut self, name: &str) -> Result<Option<Signature>, Diagnostic> {
        if let Some(found) = self.known_signature(name) {
            return Ok(Some(found));
        }
        let library = self.templates.borrow().library.get(name).cloned();
        library.map(|function| signature(self.types, &function, 0)).transpose()
    }
}

pub(super) struct Inferred {
    pub(super) bound: BTreeMap<String, TypeSpec>,
    /// The arguments that are not lambdas, in parameter order.
    pub(super) passed: Vec<Expr>,
    pub(super) lambdas: Vec<(String, Expr, Vec<BTreeMap<String, Binding>>)>,
}

/// `template` with its type parameters replaced, as the function `name`.
pub(super) fn substituted_function(
    template: &Function,
    name: &str,
    bound: &BTreeMap<String, TypeSpec>,
) -> Function {
    let mut function = template.clone();
    function.name = name.into();
    function.generics.clear();
    for parameter in &mut function.parameters {
        match &mut parameter.type_ {
            ParameterType::Owned(annotation)
            | ParameterType::Borrowed {
                target: annotation, ..
            } => {
                *annotation = substituted_annotation(annotation, bound);
            }
        }
    }
    function.result = substituted_annotation(&function.result, bound);
    substitute_body(&mut function.body, bound);
    function
}

fn substituted_annotation(
    annotation: &TypeAnnotation,
    bound: &BTreeMap<String, TypeSpec>,
) -> TypeAnnotation {
    let bound = bound.iter().map(|(name, one)| (name.clone(), TypeAnnotation::Value(one.clone()))).collect();
    generics::substitute_in(annotation, &bound)
}

/// Binds the type parameters `pattern` names to the parts of `actual`.
fn unify_specs(pattern: &TypeSpec, actual: &TypeSpec, generics: &[&str], bound: &mut BTreeMap<String, TypeSpec>) {
    match (pattern, actual) {
        (TypeSpec::Named(name), _) if generics.contains(&name.as_str()) => {
            bound.entry(name.clone()).or_insert_with(|| actual.clone());
        }
        (TypeSpec::Applied { name, args }, TypeSpec::Applied { name: other, args: others }) if name == other => {
            for pair in args.iter().zip(others) {
                if let (TypeAnnotation::Value(inner), TypeAnnotation::Value(actual)) = pair {
                    unify_specs(inner, actual, generics, bound);
                }
            }
        }
        _ => {}
    }
}

/// `name[T, ...]`: an instance, or a method its type arguments name.
fn instance_name(name: &str, types: &[TypeSpec]) -> String {
    let types: Vec<String> = types.iter().map(TypeSpec::text).collect();
    format!("{name}[{}]", types.join(", "))
}

/// The types `body` names: its `let`s' annotations, its calls' type
/// arguments, and each `T(x)`, a conversion to what `T` is bound to.
fn substitute_body(body: &mut [Statement], bound: &BTreeMap<String, TypeSpec>) {
    for statement in body {
        statement.each_mut(&mut |one| {
            if let Statement::Bind { annotation: Some(annotation), .. } = one {
                *annotation = substituted_annotation(annotation, bound);
            }
        });
        let Ok(()) = statement.walk_mut(&mut |expression| -> Result<(), std::convert::Infallible> {
            match expression {
                Expr::Call { name, arguments, span, .. } if arguments.len() == 1 => {
                    if let Some(TypeSpec::Primitive(target)) = bound.get(name.as_str()) {
                        *expression = Expr::Conversion { target: *target, value: Box::new(arguments[0].clone()), span: *span };
                        return Ok(());
                    }
                }
                _ => {}
            }
            if let Expr::Call { type_arguments, .. } | Expr::MethodCall { type_arguments, .. } = expression {
                for spec in type_arguments {
                    *spec = generics::substitute(spec, bound);
                }
            }
            Ok(())
        });
    }
}

/// Where `statement` calls a generator that a loop consumes in place: a
/// `for`'s iterable, or a comprehension clause's.
pub(super) fn consumed_generators(statement: &Statement) -> Vec<Span> {
    let mut found = Vec::new();
    let mut statement = statement.clone();
    if let Statement::For { iterable: Expr::Call { span, .. } | Expr::MethodCall { span, .. } | Expr::Generator { span, .. }, .. } = &statement {
        found.push(*span);
    }
    let Ok(()) = statement.walk_mut(&mut |one| -> Result<(), std::convert::Infallible> {
        if let Expr::Comprehension { clauses, .. } | Expr::Generator { clauses, .. } | Expr::DictComprehension { clauses, .. } = one {
            for clause in clauses {
                if let Clause::For { iterable: Expr::Call { span, .. } | Expr::Generator { span, .. }, .. } = clause {
                    found.push(*span);
                }
            }
        }
        Ok(())
    });
    found
}
