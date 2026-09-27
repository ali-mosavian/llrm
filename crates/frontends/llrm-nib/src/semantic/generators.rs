//! Generators: a function returning `iter[T]` is never called. A `for` that
//! consumes one compiles the generator's body in place and runs its own body
//! at each `yield`, so no iterator is built. A generator expression is one
//! whose body is its clauses.

use super::*;

pub(super) fn is_generator(function: &Function) -> bool {
    matches!(&function.result, TypeAnnotation::Value(TypeSpec::Applied { name, .. }) if name == "iter")
}

/// A `for` whose generator is being compiled in place.
#[derive(Clone, Debug)]
pub(super) struct Consumer {
    /// The loop's binding and body, run at each `yield`.
    name: String,
    /// The generator's declared item type, which each `yield` binds.
    item: Option<TypeAnnotation>,
    body: Vec<Statement>,
    /// How many scopes the loop's own code sees; the generator's are above.
    depth: usize,
    exit: u32,
}

impl FunctionCompiler<'_> {
    /// `for name in iterable: body`, when `iterable` generates its items.
    /// `false` when it does not.
    pub(super) fn for_generated(
        &mut self,
        mode: IterationMode,
        name: &str,
        iterable: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<bool, Diagnostic> {
        let (parameters, statements, item) = match iterable {
            Expr::Generator {
                element,
                clauses,
                span,
            } => {
                let produce = vec![Statement::Yield {
                    value: (**element).clone(),
                    span: *span,
                }];
                (BTreeMap::new(), Clause::loops(clauses, produce), None)
            }
            Expr::Call {
                name: callee,
                arguments,
                span,
                ..
            } if self.is_generator_call(callee) => {
                let (parameters, body, item) = self.generator_call(callee, arguments, *span)?;
                (parameters, body, Some(item))
            }
            _ => return Ok(false),
        };
        if mode != IterationMode::Value {
            return Err(Diagnostic::new(
                span,
                "a generator's items are iterated by value",
            ));
        }
        let depth = self.scopes.len();
        let exit = self.block();
        self.consumers.push(Consumer {
            name: name.into(),
            item,
            body: body.to_vec(),
            depth,
            exit,
        });
        self.scopes.push(parameters);
        let result = self.statements(&statements);
        if result.is_ok() && self.open() {
            self.drop_scopes(depth);
        }
        self.scopes.truncate(depth);
        self.consumers.pop();
        result?;
        if self.open() {
            self.terminate(jump(exit));
        }
        self.current = exit;
        Ok(true)
    }

    pub(super) fn is_generator_call(&self, name: &str) -> bool {
        self.templates.borrow().generator(name).is_some()
    }

    /// A generator's parameters bound to `arguments` as a call binds them,
    /// and its body, its type parameters inferred from them.
    fn generator_call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) -> Result<(BTreeMap<String, Binding>, Vec<Statement>, TypeAnnotation), Diagnostic> {
        let template = self
            .templates
            .borrow()
            .generator(name)
            .cloned()
            .expect("a generator");
        let inferred = self.inferred(&template, arguments, span)?;
        self.chosen(&template, &inferred.bound, span)?;
        let function = instances::substituted_function(&template, name, &inferred.bound);
        let mut scope = BTreeMap::new();
        // Compiled here, a lambda argument may capture the caller's names.
        for (parameter, lambda, scopes) in inferred.lambdas {
            let binding = self.lambda_binding(&lambda, scopes);
            scope.insert(parameter, binding);
        }
        let parameters: Vec<_> = function
            .parameters
            .iter()
            .filter(|one| !scope.contains_key(&one.name))
            .collect();
        let mut borrowed = BTreeMap::new();
        for (parameter, argument) in parameters.into_iter().zip(&inferred.passed) {
            let kind = parameter_kind(self.types, parameter)?;
            let operand = self.argument_operand(argument, &kind, &mut borrowed)?;
            let value = self.materialized(operand, kind.hir_type());
            let binding = self.parameter_binding(&parameter.name, &kind, value);
            scope.insert(parameter.name.clone(), binding);
        }
        let TypeAnnotation::Value(TypeSpec::Applied { args, .. }) = &function.result else {
            unreachable!("a generator returns iter[T]")
        };
        Ok((scope, function.body, args[0].clone()))
    }

    /// `yield value`: the consuming loop's body, with its binding the value.
    /// A `break` there leaves the generator, a `continue` resumes it.
    pub(super) fn yield_statement(&mut self, value: &Expr, span: Span) -> Result<(), Diagnostic> {
        let consumer = self.consumers.pop().ok_or_else(|| {
            Diagnostic::new(span, "yield is only valid in a generator a 'for' consumes")
        })?;
        let generator = consumer.depth..self.scopes.len();
        let resume = self.block();
        let result = self.in_scope(|this| {
            let depth = this.scopes.len() - 1;
            let bind = Statement::Bind {
                mutable: false,
                name: consumer.name.clone(),
                annotation: consumer.item.clone(),
                value: value.clone(),
                span,
            };
            this.statement(&bind)?;
            this.drop_temporaries();
            this.hidden.push(generator);
            this.loops.push(Loop {
                exit: consumer.exit,
                exit_depth: consumer.depth,
                next: resume,
                next_depth: depth,
            });
            let result = this.statements(&consumer.body);
            this.loops.pop();
            this.hidden.pop();
            result
        });
        self.consumers.push(consumer);
        result?;
        if self.open() {
            self.terminate(jump(resume));
        }
        self.current = resume;
        Ok(())
    }

    /// `return` in a generator: the loop consuming it ends.
    pub(super) fn generator_return(
        &mut self,
        value: Option<&Expr>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if value.is_some() {
            return Err(Diagnostic::new(span, "a generator returns no value"));
        }
        let consumer = self.consumers.last().expect("in a generator");
        let (depth, exit) = (consumer.depth, consumer.exit);
        self.drop_temporaries();
        self.drop_scopes(depth);
        self.terminate(jump(exit));
        Ok(())
    }

    /// What `iterable` generates, when it is a generator and that is known.
    pub(super) fn generated_item(&mut self, iterable: &Expr) -> Option<ElementType> {
        match iterable {
            Expr::Generator {
                element, clauses, ..
            } => {
                let depth = self.scopes.len();
                let hint = self
                    .clause_scopes(clauses)
                    .and_then(|()| self.element_hint(element));
                self.scopes.truncate(depth);
                hint
            }
            Expr::Call {
                name,
                arguments,
                span,
                ..
            } if self.is_generator_call(name) => {
                let template = self
                    .templates
                    .borrow()
                    .generator(name)
                    .cloned()
                    .expect("a generator");
                let bound = self.inferred(&template, arguments, *span).ok()?.bound;
                let TypeAnnotation::Value(TypeSpec::Applied { args, .. }) = &template.result else {
                    return None;
                };
                let [TypeAnnotation::Value(item)] = args.as_slice() else {
                    return None;
                };
                self.types
                    .resolve_element(&generics::substitute(item, &bound), *span)
                    .ok()
            }
            _ => None,
        }
    }
}
