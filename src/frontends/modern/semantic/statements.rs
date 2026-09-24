//! Statements: each form of section 7 but loops.

use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn statements(&mut self, statements: &[Statement]) -> Result<(), Diagnostic> {
        for statement in statements {
            if !self.open() {
                return Err(Diagnostic::new(
                    statement.span(),
                    "statement is unreachable",
                ));
            }
            let since = self.calls.len();
            match self.prepared(statement)? {
                Some(rewritten) => self.statement(&rewritten)?,
                None => self.statement(statement)?,
            }
            if let Some(error) = self.moves.error.take() {
                return Err(Diagnostic::new(statement.span(), error.message));
            }
            if self.open() {
                self.drop_temporaries();
            } else {
                self.temporaries.clear();
                self.aggregate_temporaries.clear();
            }
            self.refuse_runtime(since, statement.span())?;
        }
        Ok(())
    }

    pub(super) fn statement(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        // BASIC copies a string result before the frame it may view is gone.
        let copied = self.signature.string_result.is_some();
        if let (Statement::Return { value: Some(value), span }, true, false) = (statement, self.consumers.is_empty(), copied) {
            self.check_returned_borrows(value, *span)?;
        }
        match statement {
            Statement::Destructure {
                pattern,
                value,
                otherwise,
                span,
            } => self.destructure(pattern, value, otherwise.as_deref(), *span)?,
            Statement::Yield { value, span } => self.yield_statement(value, *span)?,
            Statement::Unsafe { body, .. } => self.unsafe_block(body)?,
            Statement::Asm(asm) => self.asm_statement(asm)?,
            Statement::Return { value, span } if !self.consumers.is_empty() => {
                self.generator_return(value.as_ref(), *span)?
            }
            Statement::Bind {
                mutable,
                name,
                annotation,
                value,
                span,
            } => {
                if self.scopes.last().expect("scope").contains_key(name) {
                    return Err(Diagnostic::new(
                        *span,
                        format!("binding {name:?} is already declared in this scope"),
                    ));
                }
                // An annotated lambda is a value of its annotation's type.
                if let (Expr::Lambda { .. }, None) = (value, annotation) {
                    self.bind_lambda(name, value);
                    return Ok(());
                }
                // A view a call returns, or another name for one.
                if annotation.is_none() && self.view_type_of(value).is_some() {
                    let writes = self.struct_expression_type(value, *span)?.is_some_and(|id| self.types.writable_views.contains(&id));
                    let (descriptor, element, rank) = self.view_of(value)?.expect("a view");
                    let binding = Binding { type_: BindingType::Slice { element, rank }, mutable: writes, storage: Storage::Slice(descriptor) };
                    self.bind_borrow(name, *mutable, binding, value);
                    return Ok(());
                }
                // Another name for a reference borrows what it borrows; a
                // mutable one is a copy of its own.
                if let (None, Expr::Name(source, _), false) = (annotation, value, *mutable) {
                    if let Some(binding @ Binding { storage: Storage::Reference(_), .. }) = self.visible(source).cloned() {
                        if !self.owns(&binding.storage) {
                            self.bind_borrow(name, false, Binding { mutable: false, ..binding }, value);
                            return Ok(());
                        }
                    }
                }
                if let (None, Expr::Borrow { .. }) = (annotation, value) {
                    let binding = self.borrowed_binding(value)?;
                    self.bind_borrow(name, *mutable, binding, value);
                    return Ok(());
                }
                if let Expr::Comprehension {
                    element,
                    clauses,
                    span: comprehension_span,
                } = value
                {
                    // Annotated as a fixed array, it materializes in place;
                    // otherwise it is a vec (section 12).
                    let simple = Clause::simple(clauses);
                    if let Some(TypeAnnotation::Array { .. }) = annotation {
                        let (binding, mode, iterable) = simple.ok_or_else(|| {
                            Diagnostic::new(
                                *comprehension_span,
                                "a fixed array is built from one 'for' clause and no 'if'",
                            )
                        })?;
                        return self.comprehension_binding(
                            *mutable,
                            name,
                            annotation.as_ref(),
                            element,
                            binding,
                            mode,
                            iterable,
                            *comprehension_span,
                        );
                    }
                }
                // Another array's name: a copy, or a move of owned values.
                if annotation.is_none() {
                    if let Some((element, shape)) = self.fixed_array_hint(value) {
                        let array = self.types.array(element, shape);
                        let place = self.array_place(name, array, element, shape, *mutable);
                        return self.bind_array(name, *mutable, place, element, shape, value, *span);
                    }
                }
                if let Some(TypeAnnotation::Array { element, dims }) = annotation {
                    let shape = Shape::new(dims);
                    let repeated = repeated_literal(value, shape.dims());
                    let value = repeated.as_ref().unwrap_or(value);
                    match value {
                        Expr::Array(..) => {
                            literal_elements(value, shape.dims(), *span)?;
                        }
                        Expr::Repeat { counts, .. } => {
                            let counts = repeat_counts(counts)?;
                            if counts != shape.dims() {
                                return Err(Diagnostic::new(
                                    *span,
                                    format!(
                                        "array expects dimensions {:?}, got {counts:?}",
                                        shape.dims()
                                    ),
                                ));
                            }
                        }
                        _ => {}
                    }
                    if let Expr::Repeat { value, .. } = value {
                        // Evaluated before the new name exists, which it may shadow.
                        self.statement(&Statement::Bind {
                            mutable: false,
                            name: format!("${name}_fill"),
                            annotation: Some(TypeAnnotation::Value(element.clone())),
                            value: value.as_ref().clone(),
                            span: *span,
                        })?;
                    }
                    let element = self.types.resolve_element(element, *span)?;
                    let type_id = self.types.array(element, shape);
                    let place = self.array_place(name, type_id, element, shape, *mutable);
                    if !matches!(value, Expr::Repeat { .. }) {
                        return self.bind_array(name, *mutable, place, element, shape, value, *span);
                    }
                    self.check_repeatable(element, *span)?;
                    let binding = |mutable| Binding {
                        type_: BindingType::Array { element, shape },
                        mutable,
                        storage: Storage::Place(place),
                    };
                    // Filling stores through the name, which a `let` would refuse.
                    self.scopes
                        .last_mut()
                        .expect("scope")
                        .insert(name.clone(), binding(true));
                    self.fill(name, shape, *span)?;
                    self.scopes
                        .last_mut()
                        .expect("scope")
                        .insert(name.clone(), binding(*mutable));
                    return Ok(());
                }
                let annotated = match annotation {
                    Some(TypeAnnotation::Value(spec)) => {
                        Some(self.types.resolve_element(spec, *span)?)
                    }
                    Some(TypeAnnotation::Slice { .. }) => {
                        return Err(Diagnostic::new(
                            *span,
                            "an owned array needs a fixed length: 'T[N]'",
                        ));
                    }
                    Some(TypeAnnotation::Array { .. }) => unreachable!(),
                    None => None,
                };
                let struct_id = match annotated {
                    Some(ElementType::Struct(struct_id)) => Some(struct_id),
                    Some(ElementType::Scalar(_)) => None,
                    None => self.struct_expression_type(value, *span)?,
                };
                if let Some(struct_id) = struct_id {
                    let type_id = self
                        .types
                        .structure(struct_id)
                        .expect("resolved struct type")
                        .id;
                    let extent = self.types.width(type_id);
                    let place = self.local_place(name, type_id, extent, *mutable);
                    let destination = StructView {
                        struct_id,
                        place,
                        pointer: None,
                        indices: Vec::new(),
                        offset: 0,
                        mutable: *mutable,
                        owner: name.clone(),
                    };
                    self.store_struct_expression(&destination, value)?;
                    if self.element_needs_drop(ElementType::Struct(struct_id)) {
                        self.own_aggregate(&Storage::Place(place), struct_id);
                    }
                    self.keep_borrows(place, value);
                    self.scopes.last_mut().expect("scope").insert(
                        name.clone(),
                        Binding {
                            type_: BindingType::Struct(struct_id),
                            mutable: *mutable,
                            storage: Storage::Place(place),
                        },
                    );
                    return Ok(());
                }
                let expected = match annotated {
                    Some(ElementType::Scalar(type_name)) => Some(type_name),
                    Some(ElementType::Struct(_)) => unreachable!(),
                    None => None,
                };
                let source = value;
                let value = match expected {
                    Some(type_name) => self.coerced(value, type_name)?,
                    None => self.expression(value, None)?,
                };
                if value.type_name == TypeName::Void {
                    return Err(Diagnostic::new(*span, "cannot bind a void expression"));
                }
                let binding_type = value.type_name;
                if let Some(binding) = self.reference_binding(required(value.clone(), *span)?, binding_type) {
                    self.bind_borrow(name, false, binding, source);
                    return Ok(());
                }
                self.consume(&value, *span)?;
                let place = self.place(name, binding_type, *mutable);
                if ownership::needs_drop(binding_type) {
                    self.own(place);
                }
                self.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(place), required(value, *span)?],
                    None,
                );
                self.scopes.last_mut().expect("scope").insert(
                    name.clone(),
                    Binding {
                        type_: BindingType::Scalar(binding_type),
                        mutable: *mutable,
                        storage: Storage::Place(place),
                    },
                );
            }
            Statement::Assign {
                target,
                operation,
                value,
                span,
            } => {
                if let Some(value) = self.settled_failure(value, *span)? {
                    let settled = Statement::Assign { target: target.clone(), operation: *operation, value, span: *span };
                    return self.statement(&settled);
                }
                if let (AssignTarget::Name(name), None) = (target, operation) {
                    if self.reseat(name, value, *span)? {
                        return Ok(());
                    }
                }
                // A plain assignment gives a moved binding a value again.
                let reinitialized = match (target, operation) {
                    (AssignTarget::Name(name), None) => {
                        self.visible(name).map(|one| one.storage.clone())
                    }
                    _ => None,
                };
                // A frame's field with no null is dropped only while live.
                let flag = match target {
                    AssignTarget::Member { base, field } => self.frame_field(base, field, *span)?.flatten(),
                    _ => None,
                };
                self.moves.writing = reinitialized.is_some();
                let place = self.assignment_target(target, *span);
                self.moves.writing = false;
                let place = self.written_through(place?, *operation, value);
                let written = match &place {
                    AssignmentPlace::Scalar(_, element) => Some(ElementType::Scalar(*element)),
                    AssignmentPlace::Struct(view) => Some(ElementType::Struct(view.struct_id)),
                    AssignmentPlace::Bits { .. } | AssignmentPlace::Array(..) => None,
                };
                if let (Some(element), None) = (written, operation) {
                    self.check_assigned_borrows(target, value, element, *span)?;
                }
                match place {
                    AssignmentPlace::Scalar(destination, element) => {
                        let value = if let Some(operation) = operation {
                            let current = self.value(element);
                            self.emit("load", vec![current], vec![destination.clone()], None);
                            let right = self.beside(value, element)?;
                            let result = self.arithmetic(
                                *operation,
                                TypedOperand {
                                    operand: Some(hir::Operand::Value(current)),
                                    type_name: element,
                                },
                                right,
                                *span,
                            )?;
                            self.implicit(result, element, *span)?
                        } else {
                            self.coerced(value, element)?
                        };
                        if ownership::needs_drop(element) {
                            // The new value first, then the old one is dropped (section 9.5).
                            self.consume(&value, *span)?;
                            let old = self.value(element);
                            self.emit("load", vec![old], vec![destination.clone()], None);
                            self.emit_drop(hir::Operand::Value(old), element);
                        }
                        self.emit(
                            "store",
                            Vec::new(),
                            vec![destination, required(value, *span)?],
                            None,
                        );
                    }
                    AssignmentPlace::Bits {
                        place,
                        packed,
                        field,
                    } => {
                        let backing = self.types.bits_of(packed).expect("a bits type").backing;
                        let current = self.value(packed);
                        self.emit("load", vec![current], vec![place.clone()], None);
                        let current = self.bits_backing(
                            TypedOperand {
                                operand: Some(hir::Operand::Value(current)),
                                type_name: packed,
                            },
                            *span,
                        )?;
                        let current = required(current, *span)?;
                        let value = if let Some(operation) = operation {
                            let old = self.extracted(current.clone(), backing, field);
                            let right = self.beside(value, field.read)?;
                            let result = self.arithmetic(*operation, old, right, *span)?;
                            self.implicit(result, field.read, *span)?
                        } else {
                            self.coerced(value, field.read)?
                        };
                        let merged =
                            self.inserted(current, backing, field, required(value, *span)?, *span)?;
                        let merged = self.resized(merged, backing, packed);
                        self.emit("store", Vec::new(), vec![place, merged], None);
                    }
                    AssignmentPlace::Struct(destination) => {
                        if operation.is_some() {
                            return Err(Diagnostic::new(
                                *span,
                                "compound assignment requires a numeric scalar",
                            ));
                        }
                        let mut stores = Vec::new();
                        // A run reads its cells as it is stored, after the
                        // batch's earlier stores: a value reading its own
                        // target is built aside first.
                        let runs = self.types.copy_units(ElementType::Struct(destination.struct_id)).iter().any(|(_, _, count)| *count > 1);
                        let reads_target = borrows::written_owner(target).is_some_and(|owner| value.names().iter().any(|one| one == owner));
                        if runs && reads_target {
                            let staged = self.temporary(destination.struct_id);
                            self.store_struct_expression(&staged, value)?;
                            self.prepare_struct_copy(&destination, &staged, &mut stores)?;
                        } else {
                            self.prepare_struct_stores(&destination, value, &mut stores)?;
                        }
                        if self.element_needs_drop(ElementType::Struct(destination.struct_id)) {
                            match flag.clone() {
                                Some(flag) => self.when_live(Some(flag), |this| this.drop_view(&destination)),
                                None => self.drop_owner(&destination),
                            }
                        }
                        self.emit_stores(stores, *span)?;
                        if let Some(key) = Self::whole_owner(&destination) {
                            self.set_live(key, true);
                        }
                    }
                    AssignmentPlace::Array(destination, element, shape) => {
                        if operation.is_some() {
                            return Err(Diagnostic::new(
                                *span,
                                "compound assignment requires a numeric scalar",
                            ));
                        }
                        let mut stores = Vec::new();
                        self.prepare_array_stores(&destination, element, shape, value, *span, &mut stores)?;
                        // The new elements first, then the old ones are dropped (section 9.5).
                        self.when_live(flag.clone(), |this| {
                            if this.element_needs_drop(element) {
                                this.drop_owner(&destination);
                            }
                        });
                        self.emit_stores(stores, *span)?;
                        if let Some(key) = Self::whole_owner(&destination) {
                            self.set_live(key, true);
                        }
                    }
                }
                if let Some(flag) = flag {
                    let live = hir::Operand::Constant(BOOL, if matches!(value, Expr::Zero(_)) { 0 } else { -1 });
                    self.emit("store", Vec::new(), vec![flag, live], None);
                }
                if let Some(storage) = reinitialized {
                    self.reinitialized(&storage);
                }
            }
            Statement::Expr(expression) => {
                if !matches!(
                    expression,
                    Expr::Call { .. } | Expr::MethodCall { .. } | Expr::Try { .. }
                ) {
                    return Err(Diagnostic::new(
                        expression.span(),
                        "only a function call may be used as an expression statement",
                    ));
                }
                self.expression(expression, None)?;
            }
            Statement::Return {
                value: Some(expression),
                span,
            } if self.signature.view.is_some() => {
                self.return_view(expression, *span)?;
                let operands = match self.signature.string_result {
                    Some(_) => vec![self.string_result(*span)?],
                    None => Vec::new(),
                };
                self.drop_temporaries();
                self.drop_scopes(0);
                self.terminate(hir::Terminator { kind: "return", operands, targets: Vec::new() });
            }
            Statement::Return {
                value: Some(expression),
                span,
            } if self.signature.slot.is_some() => {
                let destination = self.result_view(*span)?;
                self.store_struct_expression(&destination, expression)?;
                self.drop_temporaries();
                self.drop_scopes(0);
                self.return_aggregate(*span)?;
            }
            Statement::Return { value, span } => {
                let operands = match (self.signature.result, value) {
                    (TypeName::Void, None) if self.signature.slot.is_some() || self.signature.view.is_some() => {
                        return Err(Diagnostic::new(*span, "return value is required"));
                    }
                    (TypeName::Void, None) => Vec::new(),
                    (TypeName::Void, Some(_)) => {
                        return Err(Diagnostic::new(
                            *span,
                            "void function cannot return a value",
                        ));
                    }
                    (_, None) => return Err(Diagnostic::new(*span, "return value is required")),
                    (result, Some(expression)) => {
                        let value = self.coerced(expression, result)?;
                        self.consume(&value, *span)?;
                        vec![required(value, *span)?]
                    }
                };
                self.drop_temporaries();
                self.drop_scopes(0);
                self.terminate(hir::Terminator {
                    kind: "return",
                    operands,
                    targets: Vec::new(),
                });
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => self.if_statement(condition, then_branch, else_branch, *span)?,
            Statement::While {
                condition,
                body,
                span,
            } => self.while_statement(condition, body, *span)?,
            Statement::For {
                mode,
                name,
                iterable,
                body,
                span,
            } => self.for_statement(*mode, name, iterable, body, *span)?,
            Statement::ForRange {
                name,
                start,
                end,
                body,
                span,
            } => self.range_statement(name, start, end, body, *span)?,
            Statement::Match {
                subject,
                arms,
                span,
            } => self.match_statement(subject, arms, *span)?,
            Statement::With {
                mutable,
                name,
                value,
                body,
                span,
            } => {
                let bind = Statement::Bind {
                    mutable: *mutable,
                    name: name.clone(),
                    annotation: None,
                    value: value.clone(),
                    span: *span,
                };
                self.in_scope(|this| {
                    this.statement(&bind)?;
                    this.drop_temporaries();
                    this.statements(body)
                })?;
            }
            Statement::Const(_) | Statement::Function(_) => unreachable!("desugaring takes out local declarations"),
            Statement::Break(span) => {
                let Some(Loop {
                    exit: target,
                    exit_depth: depth,
                    ..
                }) = self.loops.last().copied()
                else {
                    return Err(Diagnostic::new(*span, "break is only valid inside a loop"));
                };
                self.drop_scopes(depth);
                self.terminate(jump(target));
            }
            Statement::Continue(span) => {
                let Some(Loop {
                    next: target,
                    next_depth: depth,
                    ..
                }) = self.loops.last().copied()
                else {
                    return Err(Diagnostic::new(
                        *span,
                        "continue is only valid inside a loop",
                    ));
                };
                self.drop_scopes(depth);
                self.terminate(jump(target));
            }
        }
        Ok(())
    }

    /// Stores `$name_fill`, bound beforehand, to each of `name`'s `length` elements.
    pub(super) fn fill(&mut self, name: &str, shape: Shape, span: Span) -> Result<(), Diagnostic> {
        let at = |axis: usize| format!("${name}_at{axis}");
        let indices = (0..shape.dims().len())
            .map(|axis| Expr::Name(at(axis), span))
            .collect();
        let mut body = vec![Statement::Assign {
            target: AssignTarget::Index {
                base: Expr::Name(name.into(), span),
                indices,
            },
            operation: None,
            value: Expr::Name(format!("${name}_fill"), span),
            span,
        }];
        for (axis, length) in shape.dims().iter().enumerate().rev() {
            body = vec![Statement::ForRange {
                name: at(axis),
                start: Expr::Integer(0, span),
                end: Expr::Integer(i64::from(*length), span),
                body,
                span,
            }];
        }
        // Its indices run over the dimensions, so none is checked.
        self.statement(&Statement::Unsafe { body, span })
    }

    pub(super) fn if_statement(
        &mut self,
        condition: &Expr,
        then_branch: &[Statement],
        else_branch: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let condition = self.expression(condition, Some(TypeName::Bool))?;
        self.drop_temporaries();
        let then_block = self.block();
        let else_block = self.block();
        let join_block = self.block();
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![required(condition, span)?],
            targets: vec![then_block, else_block],
        });

        self.current = then_block;
        self.scoped(then_branch)?;
        let then_falls = self.open();
        if then_falls {
            self.terminate(jump(join_block));
        }

        self.current = else_block;
        self.scoped(else_branch)?;
        let else_falls = self.open();
        if else_falls {
            self.terminate(jump(join_block));
        }

        self.current = join_block;
        if !then_falls && !else_falls {
            self.terminate(hir::Terminator {
                kind: "unreachable",
                operands: Vec::new(),
                targets: Vec::new(),
            });
        }
        Ok(())
    }

    pub(super) fn while_statement(
        &mut self,
        condition: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let condition_block = self.block();
        let body_block = self.block();
        let exit_block = self.block();
        self.terminate(jump(condition_block));

        self.current = condition_block;
        // `loop:` leaves only by `break`.
        let endless = matches!(condition, Expr::Boolean(true, _));
        if endless {
            self.terminate(jump(body_block));
        } else {
            let condition = self.expression(condition, Some(TypeName::Bool))?;
            self.drop_temporaries();
            self.terminate(hir::Terminator {
                kind: "branch",
                operands: vec![required(condition, span)?],
                targets: vec![body_block, exit_block],
            });
        }

        self.current = body_block;
        self.loops
            .push(Loop::new(exit_block, condition_block, self.scopes.len()));
        self.scoped(body)?;
        self.loops.pop();
        if self.open() {
            self.terminate(jump(condition_block));
        }
        self.current = exit_block;
        Ok(())
    }
}
