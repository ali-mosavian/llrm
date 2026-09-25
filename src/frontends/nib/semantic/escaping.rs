//! A generator that escapes -- stored, passed on, or returned -- rather than
//! consumed by a `for` (section 12): a state struct, its frame, holding its
//! parameters, the locals that live across a `yield`, and a resume point,
//! and a `next` method that resumes it. Everything that iterates by the
//! `Iterator` protocol then iterates it. The body's lowering is `resumable`'s.
//!
//! A frame's fields are its body's locals: a value moves out of one as out
//! of a local, leaving it null. A field holding a type with a `drop` method
//! has no null, so it carries a live flag, another field. A borrowed
//! parameter is kept as its view or reference, so the frame borrows what the
//! argument did and cannot outlive it.

use super::*;
use crate::frontends::nib::resumable::{self, HiddenType, Kept, Lowering, RESUME, STATE};
use crate::frontends::nib::syntax::{Clause, MatchArm, Pattern, StructField, Struct};

/// A generator instance's state type, the fields it starts zero, and the
/// live flags of its parameters, which it starts holding.
#[derive(Clone, Debug)]
pub(super) struct GeneratorState {
    name: String,
    zeroed: Vec<String>,
    live: Vec<String>,
}

/// An escaping generator's frame: what it yields, and each field's live flag.
#[derive(Clone, Debug)]
pub(super) struct Frame {
    item: ElementType,
    flags: BTreeMap<String, String>,
}

/// A frame's field: its name, its type, and its shape when an array.
struct Field {
    name: String,
    element: ElementType,
    shape: Option<Shape>,
}

impl FunctionCompiler<'_> {
    /// `callee(arguments)` that no `for` consumes: its state, at the start.
    pub(super) fn escaping_generator(&mut self, callee: &str, arguments: &[Expr], span: Span) -> Result<Expr, Diagnostic> {
        let template = self.templates.borrow().generator(callee).cloned().expect("a generator");
        let inferred = self.inferred(&template, arguments, span)?;
        if !inferred.lambdas.is_empty() {
            return Err(Diagnostic::new(span, format!("{callee} escapes, so it cannot keep a lambda argument")));
        }
        self.chosen(&template, &inferred.bound, span)?;
        let key = format!("{callee}{:?}", inferred.bound);
        let function = instances::substituted_function(&template, callee, &inferred.bound);
        let state = match self.types.generator_states.get(&key) {
            Some(found) => found.clone(),
            None => {
                self.check_in_place(callee, &function, span)?;
                let made = self.generator_state(function.clone(), span)?;
                self.types.generator_states.insert(key, made.clone());
                made
            }
        };
        let mut fields = vec![(RESUME.to_string(), Expr::Integer(0, span), span)];
        fields.extend(function.parameters.iter().zip(inferred.passed).map(|(parameter, argument)| (parameter.name.clone(), argument, span)));
        fields.extend(state.zeroed.into_iter().map(|name| (name, Expr::Zero(span), span)));
        fields.extend(state.live.into_iter().map(|name| (name, Expr::Boolean(true, span), span)));
        Ok(Expr::StructLiteral { name: state.name, fields, span })
    }

    /// Checks `function`, an instance of the generator `callee`, by compiling
    /// a `for` that consumes it in place, and drops what that compiled. That
    /// compile is the one check of a generator body's moves and borrows; the
    /// state machine's control flow, a loop around every state, cannot say
    /// which of its moves precede which use.
    fn check_in_place(&mut self, callee: &str, function: &Function, span: Span) -> Result<(), Diagnostic> {
        let arguments = function.parameters.iter().map(|one| Expr::Name(one.name.clone(), span)).collect();
        let iterable = Expr::Call { name: callee.into(), type_arguments: Vec::new(), arguments, span };
        let consume = Statement::For { mode: IterationMode::Value, name: "$checked".into(), iterable, body: Vec::new(), span };
        let check = Function {
            name: format!("$check_{callee}"),
            generics: Vec::new(),
            parameters: function.parameters.clone(),
            result: TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Void)),
            body: vec![consume],
            span,
        };
        let signature = signature(self.types, &check, 0)?;
        let compiler = FunctionCompiler::new(&check, &signature, self.signatures, self.templates, self.builtin_ids, self.private_methods, self.literals, self.types, self.facts)?;
        compiler.compile(&check).map(|_| ())
    }

    /// `(element for ... in ...)` that no `for` consumes: a generator of its
    /// own, taking each name it reads from here -- a scalar by value,
    /// anything else borrowed -- started with them.
    pub(super) fn escaping_generator_expression(&mut self, element: &Expr, clauses: &[Clause], span: Span) -> Result<Expr, Diagnostic> {
        let depth = self.scopes.len();
        let item = self.clause_scopes(clauses).and_then(|()| self.value_element(element, span));
        self.scopes.truncate(depth);
        let item = item.ok_or_else(|| Diagnostic::new(span, "the generator's item must have a known type"))?;
        let bound: BTreeSet<String> = clauses
            .iter()
            .filter_map(|clause| match clause {
                Clause::For { pattern, .. } => Some(pattern.names().into_iter().map(str::to_string)),
                Clause::If(_) => None,
            })
            .flatten()
            .collect();
        let mut read = element.names();
        for clause in clauses {
            match clause {
                Clause::For { iterable, end, .. } => read.extend(iterable.names().into_iter().chain(end.iter().flat_map(Expr::names))),
                Clause::If(condition) => read.extend(condition.names()),
            }
        }
        let mut captured: Vec<String> = Vec::new();
        for name in read {
            if !bound.contains(&name) && !captured.contains(&name) && self.visible(&name).is_some() {
                captured.push(name);
            }
        }
        let parameters = captured
            .iter()
            .map(|name| {
                let binding = self.binding(name, span)?.clone();
                let borrowed = |target| ParameterType::Borrowed { mutable: false, target };
                let type_ = match binding.type_ {
                    BindingType::Scalar(type_name) if !ownership::needs_drop(type_name) => ParameterType::Owned(TypeAnnotation::Value(TypeSpec::Primitive(type_name))),
                    BindingType::Scalar(type_name) => borrowed(TypeAnnotation::Value(TypeSpec::Primitive(type_name))),
                    BindingType::Struct(id) => borrowed(TypeAnnotation::Value(self.types.spec_of(ElementType::Struct(id)))),
                    BindingType::Array { element, shape } => borrowed(TypeAnnotation::Slice { element: self.types.spec_of(element), rank: shape.rank }),
                    BindingType::Slice { element, rank } => borrowed(TypeAnnotation::Slice { element: self.types.spec_of(element), rank }),
                };
                Ok(Parameter { name: name.clone(), type_, default: None, span })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        // Named per state: a generic body's expression is one per instance.
        let name = format!("$generator{}", self.types.generator_states.len());
        let body = Clause::loops(clauses, vec![Statement::Yield { value: element.clone(), span }]);
        let result = TypeAnnotation::Value(TypeSpec::Applied { name: "iter".into(), args: vec![TypeAnnotation::Value(self.types.spec_of(item))] });
        self.templates.borrow_mut().add_generator(Function { name: name.clone(), generics: Vec::new(), parameters, result, body, span });
        let arguments: Vec<Expr> = captured.into_iter().map(|one| Expr::Name(one, span)).collect();
        self.escaping_generator(&name, &arguments, span)
    }

    /// Declares `function`'s state type and its `next`.
    fn generator_state(&mut self, mut function: Function, span: Span) -> Result<GeneratorState, Diagnostic> {
        let (mut fields, mut borrowed) = self.kept_parameters(&function, span)?;
        let parameters = fields.len();
        self.hold_computed_iterables(&mut function.body);
        resumable::unique_bindings(&mut function.body, &fields.iter().map(|field| field.name.clone()).collect::<Vec<_>>());
        let names: BTreeSet<String> = function
            .parameters
            .iter()
            .map(|one| one.name.clone())
            .filter(|name| !borrowed.iter().any(|(view, _)| view == name))
            .chain(resumable::split_bindings(&function.body).into_iter().map(|(name, _)| name))
            .collect();
        let TypeAnnotation::Value(TypeSpec::Applied { args, .. }) = &function.result else {
            unreachable!("a generator returns iter[T]")
        };
        let [TypeAnnotation::Value(item)] = args.as_slice() else {
            return Err(Diagnostic::new(span, "a generator yields one type: iter[T]"));
        };
        let item = self.types.resolve_element(item, span)?;
        let depth = self.scopes.len();
        let mut scope: BTreeMap<String, Binding> = fields.iter().map(|field| (field.name.clone(), placeholder(field.element, field.shape))).collect();
        scope.extend(borrowed.iter().cloned());
        self.scopes.push(scope);
        self.hidden.push(BODY..depth);
        // The body's borrows root in its own names, not the caller's.
        let caller_borrows = std::mem::take(&mut self.borrowed_from);
        let typed = self.typed_locals(&mut function.body, &mut fields, &mut borrowed);
        self.borrowed_from = caller_borrows;
        let lowered = typed.and_then(|()| {
            let kept = Kept {
                owning: fields.iter().filter(|field| names.contains(&field.name) && self.element_needs_drop(field.element)).map(|field| field.name.clone()).collect(),
                iterators: fields.iter().filter(|field| self.frame_of(field.element).is_some()).map(|field| field.name.clone()).collect(),
                fields: names.iter().filter(|name| !borrowed.iter().any(|(view, _)| view == *name)).cloned().collect(),
                views: borrowed.iter().map(|(view, _)| view.clone()).filter(|view| names.contains(view)).collect(),
            };
            let parameter_names: Vec<String> = fields[..parameters].iter().map(|field| field.name.clone()).collect();
            let (arms, hidden) = Lowering::new(&kept, span).arms(function.body.clone(), &parameter_names)?;
            for (name, type_) in hidden {
                let element = match type_ {
                    HiddenType::Counter => ElementType::Scalar(TypeName::U16),
                    HiddenType::SameAs(field) => fields.iter().find(|one| one.name == field).expect("a field").element,
                    HiddenType::Named(spec) => self.types.resolve_element(&spec, span)?,
                };
                fields.push(Field { name, element, shape: None });
            }
            Ok(arms)
        });
        self.hidden.pop();
        self.scopes.truncate(depth);
        let arms = lowered?;
        // A field with no null is live only while its flag is set.
        let flags: BTreeMap<String, String> = fields
            .iter()
            .filter(|field| matches!(field.element, ElementType::Struct(_)) && self.holds_user_drop(field.element))
            .map(|field| (field.name.clone(), format!("$live{}", field.name)))
            .collect();
        let live: Vec<String> = fields[..parameters].iter().filter_map(|field| flags.get(&field.name).cloned()).collect();
        fields.extend(flags.values().map(|flag| Field { name: flag.clone(), element: ElementType::Scalar(TypeName::Bool), shape: None }));
        let name = self.declare_frame(&fields, span)?;
        let id = self.types.structs[&name].id;
        self.types.frames.insert(id, Frame { item, flags });
        let next = self.next_method(&name, &fields, &borrowed, arms, args.clone(), span);
        self.templates.borrow_mut().generated(next, self.types)?;
        let zeroed = fields[parameters..].iter().map(|field| field.name.clone()).filter(|field| !live.contains(field)).collect();
        Ok(GeneratorState { name, zeroed, live })
    }

    /// `function`'s parameters as fields, and those borrowed: each kept as
    /// its view or reference, and read in `next` by its name.
    fn kept_parameters(&mut self, function: &Function, span: Span) -> Result<(Vec<Field>, Vec<(String, Binding)>), Diagnostic> {
        let mut fields = Vec::new();
        let mut borrowed = Vec::new();
        for parameter in &function.parameters {
            let (element, binding) = match parameter_kind(self.types, parameter)? {
                SignatureParameter::Scalar(type_name) => (ElementType::Scalar(type_name), None),
                SignatureParameter::Owned { struct_id, .. } => (ElementType::Struct(struct_id), None),
                SignatureParameter::Borrowed { mutable, target, .. } => match target.ranked() {
                    Some((element, rank, _)) => {
                        let kept = self.types.kept_view(element, rank, mutable, span)?;
                        let view = Binding { type_: BindingType::Slice { element, rank }, mutable, storage: Storage::Slice(0) };
                        (ElementType::Struct(kept), Some(view))
                    }
                    None => {
                        let target = match target {
                            BindingType::Scalar(type_name) => ElementType::Scalar(type_name),
                            BindingType::Struct(id) => ElementType::Struct(id),
                            _ => unreachable!("an array is ranked"),
                        };
                        let reference = Binding { mutable, storage: Storage::Reference(0), ..placeholder(target, None) };
                        (ElementType::Scalar(self.types.reference(target, mutable)), Some(reference))
                    }
                },
                SignatureParameter::Adapter { .. } => {
                    return Err(Diagnostic::new(span, format!("a generator cannot take the BASIC parameter {:?}", parameter.name)));
                }
            };
            if let Some(binding) = binding {
                borrowed.push((parameter.name.clone(), binding));
            }
            fields.push(Field { name: parameter.name.clone(), element, shape: None });
        }
        Ok((fields, borrowed))
    }

    /// Registers the state struct of `fields`, after the resume point; its name.
    fn declare_frame(&mut self, fields: &[Field], span: Span) -> Result<String, Diagnostic> {
        let name = format!("{STATE}{}", self.types.generator_states.len());
        let resume = StructField { name: RESUME.into(), mutable: true, type_spec: TypeSpec::Primitive(TypeName::U16), dims: Vec::new(), span };
        let declared = Struct {
            name: name.clone(),
            generics: Vec::new(),
            bits: None,
            pack: None,
            fields: std::iter::once(resume)
                .chain(fields.iter().map(|field| {
                    let kept = matches!(field.element, ElementType::Struct(id) if self.types.kept_views.contains_key(&id));
                    let dims = field.shape.map(|shape| shape.dims().to_vec()).unwrap_or_default();
                    StructField { name: field.name.clone(), mutable: !kept, type_spec: self.types.spec_of(field.element), dims, span }
                }))
                .collect(),
            span,
        };
        self.types.register_struct(&declared)?;
        Ok(name)
    }

    /// The state `name`'s `next`: each borrowed name bound, then one loop
    /// over a `match` of the resume point, whose arms are `arms`.
    fn next_method(&self, name: &str, fields: &[Field], borrowed: &[(String, Binding)], arms: Vec<MatchArm>, item: Vec<TypeAnnotation>, span: Span) -> Function {
        let this = || Expr::Name("self".into(), span);
        let resume = Expr::Member { base: Box::new(this()), field: RESUME.into(), span };
        // A reference is bound as itself, a view as another name for the kept one.
        let bound = borrowed.iter().map(|(kept, binding)| Statement::Bind {
            mutable: false,
            name: kept.clone(),
            annotation: matches!(binding.storage, Storage::Reference(_)).then(|| {
                let field = fields.iter().find(|one| &one.name == kept).expect("a field");
                TypeAnnotation::Value(self.types.spec_of(field.element))
            }),
            value: Expr::Member { base: Box::new(this()), field: kept.clone(), span },
            span,
        });
        let resumed = Statement::While { condition: Expr::Boolean(true, span), body: vec![Statement::Match { subject: resume, arms, span }], span };
        Function {
            name: format!("{name}.next"),
            generics: Vec::new(),
            parameters: vec![Parameter {
                name: "self".into(),
                type_: ParameterType::Borrowed { mutable: true, target: TypeAnnotation::Value(TypeSpec::Named(name.into())) },
                default: None,
                span,
            }],
            result: TypeAnnotation::Value(TypeSpec::Applied { name: "Option".into(), args: item }),
            body: bound.chain([resumed]).collect(),
            span,
        }
    }

    /// The type of each name a split block of `body` binds, in order, each
    /// seen by those after it; a generator a split block starts is its state.
    /// A view kept is `borrowed`: read by its name, as a borrowed parameter is.
    fn typed_locals(&mut self, body: &mut [Statement], fields: &mut Vec<Field>, borrowed: &mut Vec<(String, Binding)>) -> Result<(), Diagnostic> {
        for statement in body {
            let split = resumable::holds_yield(statement);
            match statement {
                Statement::Bind { name, annotation: None, value, span, .. } if self.view_value(value).is_some() => {
                    let (element, rank, mutable) = self.view_value(value).expect("a view");
                    self.keep_view(name, element, rank, mutable, value, *span, fields, borrowed)?;
                }
                Statement::Bind { name, annotation, value, span, .. } => {
                    self.started_generators(value)?;
                    let (element, shape) = match annotation {
                        Some(TypeAnnotation::Value(spec)) => (self.types.resolve_element(spec, *span)?, None),
                        Some(TypeAnnotation::Array { element, dims }) => (self.types.resolve_element(element, *span)?, Some(Shape::new(dims))),
                        Some(TypeAnnotation::Slice { .. }) => return Err(Diagnostic::new(*span, "an owned array needs a fixed length: 'T[N]'")),
                        None => (self.local_type(name, value, *span)?, None),
                    };
                    self.check_lent(name, element, value, *span, fields, borrowed)?;
                    self.keep(name, element, shape, fields);
                }
                Statement::With { name, value, span, .. } if split => {
                    self.started_generators(value)?;
                    let element = self.local_type(name, value, *span)?;
                    self.check_lent(name, element, value, *span, fields, borrowed)?;
                    self.keep(name, element, None, fields);
                }
                Statement::Destructure { pattern, value, span, .. } => {
                    self.started_generators(value)?;
                    self.keep_pattern(pattern, value, *span, fields, borrowed)?;
                }
                Statement::ForRange { name, start, end, span, .. } if split => {
                    let element = self.range_hint(start, end).map(ElementType::Scalar).ok_or_else(|| Diagnostic::new(*span, "range bounds must be integers with a common type"))?;
                    self.keep(name, element, None, fields);
                }
                Statement::For { name, iterable, span, .. } if split => {
                    self.started_generators(iterable)?;
                    // An iterator's item is kept; a sequence's is its element, read in place.
                    if let Some(frame) = self.frame_of_expression(iterable) {
                        self.check_lent(name, frame.item, iterable, *span, fields, borrowed)?;
                        self.keep(name, frame.item, None, fields);
                    } else {
                        let element = self.iterated_item(iterable).ok_or_else(|| Diagnostic::new(*span, "a generator that escapes iterates an iterator or a sequence"))?;
                        self.scopes.last_mut().expect("scope").insert(name.clone(), placeholder(element, None));
                    }
                }
                Statement::Match { subject, arms, .. } if split => {
                    for arm in arms.iter() {
                        self.keep_pattern(&arm.pattern, subject, arm.span, fields, borrowed)?;
                    }
                }
                _ => {}
            }
            if split {
                for block in statement.blocks_mut() {
                    self.typed_locals(block, fields, borrowed)?;
                }
            }
        }
        Ok(())
    }

    /// Evaluates once each value a split `for` computes to iterate: `for x in
    /// f(): ...` is `with $iterable = f(): for x in $iterable: ...`.
    fn hold_computed_iterables(&self, body: &mut [Statement]) {
        for statement in body.iter_mut().filter(|one| resumable::holds_yield(one)) {
            for block in statement.blocks_mut() {
                self.hold_computed_iterables(block);
            }
            let Statement::For { iterable, span, .. } = statement else {
                continue;
            };
            let computed = match iterable {
                Expr::Call { name, .. } => !self.is_generator_call(name) && self.known_signature(name).is_some_and(|one| one.view.is_none()),
                Expr::Array(..) | Expr::Repeat { .. } | Expr::Comprehension { .. } | Expr::FString { .. } => true,
                _ => false,
            };
            if computed {
                let (name, span) = (format!("$iterable{}_{}", span.line, span.column), *span);
                let value = std::mem::replace(iterable, Expr::Name(name.clone(), span));
                let body = vec![statement.clone()];
                *statement = Statement::With { mutable: false, name, value, body, span };
            }
        }
    }

    /// `expression` readied as a statement's is, so each generator it starts
    /// that no `for` consumes is its state.
    fn started_generators(&mut self, expression: &mut Expr) -> Result<(), Diagnostic> {
        let consumed = instances::consumed_generators(&Statement::Expr(expression.clone()));
        let outer = std::mem::replace(&mut self.consumed, consumed);
        let prepared = self.prepare_expression(expression);
        self.consumed = outer;
        prepared.map(|_| ())
    }

    /// Keeps each name `pattern` binds in `subject`: a sequence pattern's
    /// starred name views it.
    fn keep_pattern(&mut self, pattern: &Pattern, subject: &Expr, span: Span, fields: &mut Vec<Field>, borrowed: &mut Vec<(String, Binding)>) -> Result<(), Diagnostic> {
        let unknown = || Diagnostic::new(span, "the pattern's names must have known types");
        let bound = match pattern {
            Pattern::Sequence { before, rest, after, .. } => {
                let element = self.view_value(subject).map(|(element, _, _)| element).or_else(|| self.iterated_item(subject)).ok_or_else(unknown)?;
                let mut bound = Vec::new();
                for one in before.iter().chain(after) {
                    bound.extend(self.pattern_bindings(one, element).ok_or_else(unknown)?);
                }
                if let Some(Pattern::Binding(name, _)) = rest.as_deref() {
                    bound.push((name.clone(), BindingType::Slice { element, rank: 1 }));
                }
                bound
            }
            _ => {
                let item = self.value_element(subject, span).ok_or_else(unknown)?;
                self.pattern_bindings(pattern, item).ok_or_else(unknown)?
            }
        };
        for (name, binding) in bound {
            let element = match binding {
                BindingType::Scalar(type_name) => ElementType::Scalar(type_name),
                BindingType::Struct(id) => ElementType::Struct(id),
                BindingType::Slice { element, rank } => {
                    self.keep_view(&name, element, rank, false, subject, span, fields, borrowed)?;
                    continue;
                }
                BindingType::Array { .. } => unreachable!("a pattern binds no array"),
            };
            self.check_lent(&name, element, subject, span, fields, borrowed)?;
            self.keep(&name, element, None, fields);
        }
        Ok(())
    }

    /// Errs when `name`, a value of `element` made from `source`, would keep
    /// a borrow of the frame's own values: the frame moves between
    /// resumptions, so what it keeps borrows only what its caller lent it --
    /// through its borrowed parameters, views and references it keeps, or
    /// the generators it holds, which keep only the same.
    #[allow(clippy::too_many_arguments)]
    fn check_lent(&self, name: &str, element: ElementType, source: &Expr, span: Span, fields: &[Field], borrowed: &[(String, Binding)]) -> Result<(), Diagnostic> {
        if !self.holds_reference(element) && !matches!(element, ElementType::Struct(id) if self.types.kept_views.contains_key(&id)) {
            return Ok(());
        }
        let own = |root: &String| {
            !borrowed.iter().any(|(lent, _)| lent == root)
                && fields.iter().any(|field| &field.name == root && self.frame_of(field.element).is_none() && !self.holds_reference(field.element))
        };
        match self.roots(source).iter().find(|root| own(root)) {
            Some(root) => Err(Diagnostic::new(span, format!("a generator that escapes keeps only borrows of what its caller lent it; {name:?} borrows its own {root:?}"))),
            None => Ok(()),
        }
    }

    /// The view `value` makes: its element, rank, and whether it writes.
    fn view_value(&self, value: &Expr) -> Option<(ElementType, u8, bool)> {
        match value {
            Expr::Borrow { mutable, operand, .. } => self.borrowed_view_type(operand, *mutable).map(|(element, rank)| (element, rank, *mutable)),
            Expr::Slice { base, .. } => self.indexed_hint(base).map(|element| (element, 1, false)),
            _ => self.view_type_of(value).map(|(element, rank)| (element, rank, false)),
        }
    }

    /// Keeps `name`, a view of what `source` borrows. The frame moves between
    /// resumptions, so the view may borrow only what the caller lent.
    #[allow(clippy::too_many_arguments)]
    fn keep_view(&mut self, name: &str, element: ElementType, rank: u8, mutable: bool, source: &Expr, span: Span, fields: &mut Vec<Field>, borrowed: &mut Vec<(String, Binding)>) -> Result<(), Diagnostic> {
        let kept = ElementType::Struct(self.types.kept_view(element, rank, mutable, span)?);
        self.check_lent(name, kept, source, span, fields, borrowed)?;
        fields.push(Field { name: name.into(), element: kept, shape: None });
        let view = Binding { type_: BindingType::Slice { element, rank }, mutable, storage: Storage::Slice(0) };
        self.scopes.last_mut().expect("scope").insert(name.into(), view.clone());
        borrowed.push((name.into(), view));
        Ok(())
    }

    /// `name` as a field, and in scope for the names after it.
    fn keep(&mut self, name: &str, element: ElementType, shape: Option<Shape>, fields: &mut Vec<Field>) {
        fields.push(Field { name: name.into(), element, shape });
        self.scopes.last_mut().expect("scope").insert(name.into(), placeholder(element, shape));
    }

    /// The type of a local bound to `value` with none written: a borrow of a
    /// place is a reference to it, and any other value has its own type.
    fn local_type(&mut self, name: &str, value: &Expr, span: Span) -> Result<ElementType, Diagnostic> {
        if let Expr::Borrow { mutable, operand, .. } = value {
            if let Some(target) = self.value_element(operand, span) {
                return Ok(ElementType::Scalar(self.types.reference(target, *mutable)));
            }
        }
        self.value_element(value, span)
            .ok_or_else(|| Diagnostic::new(span, format!("the type of {name:?} is not evident from its value, and the generator's state keeps it: write it")))
    }

    /// The type of the value `expression` gives, when it is known.
    fn value_element(&mut self, expression: &Expr, span: Span) -> Option<ElementType> {
        match self.struct_expression_type(expression, span) {
            Ok(Some(id)) => Some(ElementType::Struct(id)),
            _ => self.element_hint(expression),
        }
    }

    /// The frame a value of `element` is, when it is one.
    pub(super) fn frame_of(&self, element: ElementType) -> Option<&Frame> {
        match element {
            ElementType::Struct(id) => self.types.frames.get(&id),
            ElementType::Scalar(_) => None,
        }
    }

    fn frame_of_expression(&self, expression: &Expr) -> Option<Frame> {
        let id = self.struct_expression_type(expression, expression.span()).ok()??;
        self.frame_of(ElementType::Struct(id)).cloned()
    }

    /// Whether `base.field` is a field of a frame, named as `next` names its
    /// own, and if so its live flag's place.
    pub(super) fn frame_field(&mut self, base: &Expr, field: &str, span: Span) -> Result<Option<Option<hir::Operand>>, Diagnostic> {
        let (Expr::Name(..), Some(id)) = (base, self.struct_expression_type(base, span)?) else {
            return Ok(None);
        };
        if !self.types.frames.contains_key(&id) {
            return Ok(None);
        }
        let parent = self.struct_view(base, span)?;
        Ok(Some(self.frame_flag(&parent, field)))
    }

    /// The live flag of `field` of the frame `parent` views, if it has one.
    pub(super) fn frame_flag(&self, parent: &StructView, field: &str) -> Option<hir::Operand> {
        let flag = self.types.frames.get(&parent.struct_id)?.flags.get(field)?;
        let offset = self.types.structure(parent.struct_id)?.fields[flag].offset;
        Some(self.projected_place(parent, offset, TypeName::Bool))
    }
}

/// A name for type checking only, bound to nothing.
fn placeholder(element: ElementType, shape: Option<Shape>) -> Binding {
    Binding {
        type_: match (element, shape) {
            (element, Some(shape)) => BindingType::Array { element, shape },
            (ElementType::Scalar(type_name), None) => BindingType::Scalar(type_name),
            (ElementType::Struct(id), None) => BindingType::Struct(id),
        },
        mutable: true,
        storage: Storage::Place(0),
    }
}
