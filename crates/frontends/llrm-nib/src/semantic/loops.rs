//! Loops and comprehensions bound to a name (sections 7 and 12).

use super::*;

impl<'a> FunctionCompiler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn comprehension_binding(
        &mut self,
        mutable: bool,
        name: &str,
        annotation: Option<&TypeAnnotation>,
        expression: &Expr,
        item_name: &str,
        mode: IterationMode,
        iterable: &Expr,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Expr::Name(source_name, _) = iterable else {
            return Err(Diagnostic::new(
                iterable.span(),
                "a bounded comprehension currently requires a named fixed array",
            ));
        };
        if source_name == name {
            return Err(Diagnostic::new(span, "a comprehension cannot replace its own source"));
        }
        let source = self.binding(source_name, iterable.span())?.clone();
        let Some((source_element, Some(length))) = source.type_.array() else {
            let ranked = source.type_.ranked().is_some_and(|(_, rank, _)| rank > 1);
            return Err(Diagnostic::new(
                iterable.span(),
                if ranked {
                    "a comprehension reads a one-dimensional array"
                } else {
                    "a materialized comprehension needs a statically bounded source"
                },
            ));
        };
        let source_scalar = match source_element {
            ElementType::Scalar(type_name) => type_name,
            ElementType::Struct(_) => {
                return Err(Diagnostic::new(span, "struct comprehension elements must select a scalar field"));
            }
        };

        self.scopes.push(BTreeMap::from([(
            item_name.into(),
            Binding {
                type_: BindingType::Scalar(source_scalar),
                mutable: mode == IterationMode::Mutable,
                storage: Storage::Parameter(0),
            },
        )]));
        let inferred = self
            .expression_type_hint(expression)
            .or_else(
                || match expression {
                    Expr::Integer(value, _) => self.rules.literal(*value),
                    _ => None,
                },
            );
        self.scopes.pop();

        let annotated = match annotation {
            None => None,
            Some(TypeAnnotation::Array { element, dims }) => {
                let annotated_length = dims.iter().product::<u32>();
                if dims.len() != 1 || annotated_length != length {
                    return Err(Diagnostic::new(
                        span,
                        format!("comprehension has {length} elements, annotation expects {annotated_length}"),
                    ));
                }
                Some(self.types.resolve_element(element, span)?)
            }
            Some(_) => {
                return Err(Diagnostic::new(span, "a comprehension binding needs an array annotation or inference"));
            }
        };
        let result_type = match (annotated, inferred) {
            (Some(ElementType::Scalar(expected)), Some(actual)) if expected != actual => {
                return Err(type_mismatch(span, expected, actual));
            }
            (Some(ElementType::Scalar(type_name)), _) | (None, Some(type_name)) => type_name,
            (Some(ElementType::Struct(_)), _) => {
                return Err(Diagnostic::new(span, "struct-valued comprehensions are not in this slice"));
            }
            (None, None) => {
                return Err(Diagnostic::new(
                    expression.span(),
                    "cannot infer comprehension element type; add an array annotation",
                ));
            }
        };
        let result_element = ElementType::Scalar(result_type);
        let shape = Shape::new(&[length]);
        let type_id = self.types.array(result_element, shape);
        let place = self.array_place(name, type_id, result_element, shape, mutable);
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(
                name.into(),
                Binding {
                    type_: BindingType::Array { element: result_element, shape: Shape::new(&[length]) },
                    mutable: true,
                    storage: Storage::Place(place),
                },
            );
        let counter_name = format!("$comprehension_{name}");
        let counter = self.place(&counter_name, self.word(), true);
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(counter), hir::Operand::Constant(self.word_id(), 0)],
            None,
        );
        let word = self.word();
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(
                counter_name.clone(),
                Binding { type_: BindingType::Scalar(word), mutable: true, storage: Storage::Place(counter) },
            );
        let body = vec![
            Statement::Assign {
                target: AssignTarget::Index {
                    base: Expr::Name(name.into(), span),
                    indices: vec![Expr::Name(counter_name.clone(), span)],
                },
                operation: None,
                value: expression.clone(),
                span,
            },
            Statement::Assign {
                target: AssignTarget::Name(counter_name),
                operation: Some(BinaryOp::Add),
                value: Expr::Integer(1, span),
                span,
            },
        ];
        self.for_statement(mode, item_name, iterable, &body, span)?;
        self.scopes.last_mut().expect("scope").get_mut(name).expect("comprehension result").mutable = mutable;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_statement(
        &mut self,
        mode: IterationMode,
        name: &str,
        iterable: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        // The sequence a loop walks is borrowed until the loop ends; an
        // iterator it consumes is not.
        let walked = match borrows::expression_owner(iterable) {
            Some(_) if !self.loop_consumes(iterable) => self.roots(iterable),
            _ => BTreeSet::new(),
        };
        if !walked.is_empty() {
            match mode {
                IterationMode::Mutable => self.check_unborrowed(iterable, iterable.span())?,
                IterationMode::Shared | IterationMode::Value => self.check_shareable(iterable, iterable.span())?,
            }
        }
        let depth = self.iterated.len();
        self.iterated.extend(walked);
        let result = self.for_walk(mode, name, iterable, body, span);
        self.iterated.truncate(depth);
        result
    }

    pub(super) fn for_walk(
        &mut self,
        mode: IterationMode,
        name: &str,
        iterable: &Expr,
        body: &[Statement],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if let Some(call) = self.method_as_call(iterable) {
            return self.for_walk(mode, name, &call, body, span);
        }
        if self.for_generated(mode, name, iterable, body, span)?
            || self.for_protocol(mode, name, iterable, body, span)?
        {
            return Ok(());
        }
        let array_name = match iterable {
            Expr::Name(name, _) => name.as_str(),
            // Any other sequence is held by a hidden local while the loop
            // runs: a view of a place or a range, or the value anything else
            // makes.
            _ => {
                let held = self.hidden("sequence");
                let value = match iterable {
                    Expr::Member { .. } | Expr::Index { .. } | Expr::Slice { .. } => Expr::Borrow {
                        mutable: mode == IterationMode::Mutable,
                        operand: Box::new(iterable.clone()),
                        span,
                    },
                    _ => iterable.clone(),
                };
                let bind = Statement::Bind {
                    mutable: mode == IterationMode::Mutable,
                    name: held.clone(),
                    annotation: None,
                    value,
                    span,
                };
                return self.in_scope(|this| {
                    this.statement(&bind)?;
                    this.drop_temporaries();
                    this.for_statement(mode, name, &Expr::Name(held, span), body, span)
                });
            }
        };
        let array = self.binding(array_name, iterable.span())?.clone();
        let heap = self.heap_sequence(&array);
        let string = array.type_ == BindingType::Scalar(TypeName::String);
        let (element, length) = if let Some(element) = heap {
            (element, None)
        } else {
            array.type_.array().ok_or_else(|| {
                let ranked = array.type_.ranked().is_some();
                Diagnostic::new(
                    iterable.span(),
                    if ranked { "a ranked array is iterated by index" } else { "for requires an array or string" },
                )
            })?
        };
        if string && mode == IterationMode::Mutable {
            return Err(Diagnostic::new(span, "strings are immutable byte sequences"));
        }
        if mode == IterationMode::Mutable {
            self.place_writable(iterable, span)?;
        }
        let string_pointer = if heap.is_some() { Some(self.string_pointer(&array, iterable.span())?) } else { None };
        let length = match length {
            Some(length) => hir::Operand::Constant(
                self.word_id(),
                i64::try_from(length)
                    .ok()
                    .filter(|&length| length >> (8 * self.word_bytes()) == 0)
                    .ok_or_else(|| Diagnostic::new(iterable.span(), "for array length exceeds the target's word"))?,
            ),
            None => {
                let pointer = if let Some(pointer) = string_pointer {
                    pointer
                } else if let Storage::Slice(pointer) = &array.storage {
                    *pointer
                } else {
                    return Err(Diagnostic::new(iterable.span(), "view has no descriptor"));
                };
                let value = self.value(self.word());
                self.emit(
                    "load",
                    vec![value],
                    vec![hir::Operand::DescriptorPlace { base: pointer, field: "length", type_id: self.word_id() }],
                    None,
                );
                hir::Operand::Value(value)
            }
        };

        let index_place = self.place(&format!("$for_{array_name}"), self.word(), true);
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(index_place), hir::Operand::Constant(self.word_id(), 0)],
            None,
        );
        let condition_block = self.block();
        let body_block = self.block();
        let increment_block = self.block();
        let exit_block = self.block();
        self.terminate(jump(condition_block));

        self.current = condition_block;
        let index = self.value(self.word());
        self.emit("load", vec![index], vec![hir::Operand::Place(index_place)], None);
        let condition = self.value(TypeName::Bool);
        self.emit("below", vec![condition], vec![hir::Operand::Value(index), length], None);
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(condition)],
            targets: vec![body_block, exit_block],
        });

        self.current = body_block;
        let element_width = self.types.width(element.id());
        let view_storage = if let Some(pointer) = string_pointer {
            Storage::Reference(self.indexed_pointer(pointer, hir::Operand::Value(index), element_width, span)?)
        } else {
            match array.storage {
                Storage::Place(place) => Storage::ArrayView { place, index: hir::Operand::Value(index) },
                Storage::Slice(descriptor) => Storage::Reference(self.view_element(
                    descriptor,
                    element,
                    1,
                    vec![hir::Operand::Value(index)],
                    span,
                )?),
                // A fixed array reached through its address.
                Storage::Reference(pointer) => Storage::Reference(self.indexed_pointer(
                    pointer,
                    hir::Operand::Value(index),
                    element_width,
                    span,
                )?),
                Storage::Parameter(_) | Storage::ArrayView { .. } => {
                    unreachable!("checked above")
                }
                Storage::Lambda(_) => {
                    unreachable!("sequence is not a dictionary")
                }
            }
        };
        // An element reached by reference lies in what the loop walks.
        if let Storage::Reference(pointer) = view_storage {
            let walked = self.roots(iterable);
            self.borrowed_from.insert(borrows::BorrowKey::Value(pointer), walked);
            self.walking.insert(borrows::BorrowKey::Value(pointer));
        }
        self.scopes.push(BTreeMap::new());
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(
                name.into(),
                Binding {
                    type_: match element {
                        ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                        ElementType::Struct(id) => BindingType::Struct(id),
                    },
                    mutable: mode == IterationMode::Mutable,
                    storage: view_storage,
                },
            );
        self.loops.push(Loop::new(exit_block, increment_block, self.scopes.len()));
        let result = self.scoped(body);
        self.loops.pop();
        self.scopes.pop();
        result?;
        if self.open() {
            self.terminate(jump(increment_block));
        }

        self.current = increment_block;
        let old_index = self.value(self.word());
        self.emit("load", vec![old_index], vec![hir::Operand::Place(index_place)], None);
        let next_index = self.value(self.word());
        self.emit(
            "add",
            vec![next_index],
            vec![hir::Operand::Value(old_index), hir::Operand::Constant(self.word_id(), 1)],
            None,
        );
        self.emit("store", Vec::new(), vec![hir::Operand::Place(index_place), hir::Operand::Value(next_index)], None);
        self.terminate(jump(condition_block));
        self.current = exit_block;
        Ok(())
    }

    pub(super) fn range_statement(
        &mut self,
        name: &str,
        start: &Expr,
        end: &Expr,
        body: &[Statement],
        _span: Span,
    ) -> Result<(), Diagnostic> {
        let (start_value, end_value) = self.operand_pair(start, end)?;
        let type_name = self
            .rules
            .common(start_value.type_name, end_value.type_name)
            .filter(|one| is_integer(*one))
            .ok_or_else(|| Diagnostic::new(end.span(), "range bounds must be integers with a common type"))?;
        let start_value = self.implicit(start_value, type_name, start.span())?;
        let end_value = self.implicit(end_value, type_name, end.span())?;
        let counter_place = self.place(&format!("$range_{name}"), type_name, true);
        let limit_place = self.place(&format!("$range_limit_{name}"), type_name, false);
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(counter_place), required(start_value, start.span())?],
            None,
        );
        self.emit("store", Vec::new(), vec![hir::Operand::Place(limit_place), required(end_value, end.span())?], None);

        let condition_block = self.block();
        let body_block = self.block();
        let increment_block = self.block();
        let exit_block = self.block();
        self.terminate(jump(condition_block));

        self.current = condition_block;
        let current = self.value(type_name);
        self.emit("load", vec![current], vec![hir::Operand::Place(counter_place)], None);
        let limit = self.value(type_name);
        self.emit("load", vec![limit], vec![hir::Operand::Place(limit_place)], None);
        let condition = self.value(TypeName::Bool);
        self.emit(
            if is_unsigned(type_name) { "below" } else { "lt" },
            vec![condition],
            vec![hir::Operand::Value(current), hir::Operand::Value(limit)],
            None,
        );
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![hir::Operand::Value(condition)],
            targets: vec![body_block, exit_block],
        });

        self.current = body_block;
        self.scopes.push(BTreeMap::new());
        self.scopes.last_mut().expect("scope").insert(
            name.into(),
            Binding { type_: BindingType::Scalar(type_name), mutable: false, storage: Storage::Place(counter_place) },
        );
        self.loops.push(Loop::new(exit_block, increment_block, self.scopes.len()));
        let result = self.scoped(body);
        self.loops.pop();
        self.scopes.pop();
        result?;
        if self.open() {
            self.terminate(jump(increment_block));
        }

        self.current = increment_block;
        let old_value = self.value(type_name);
        self.emit("load", vec![old_value], vec![hir::Operand::Place(counter_place)], None);
        let next_value = self.value(type_name);
        let add = self.emit(
            "add",
            vec![next_value],
            vec![hir::Operand::Value(old_value), hir::Operand::Constant(type_id(type_name), 1)],
            None,
        );
        // The counter was below the limit before this body ran and nothing
        // else writes it, so one more stays within its type.
        let wrap = if is_unsigned(type_name) {
            llrm_mir::facts::Fact::NoUnsignedWrap
        } else {
            llrm_mir::facts::Fact::NoSignedWrap
        };
        self.stated.state(
            llrm_core::hir::facts::Subject::Instruction { function: i64::from(self.signature.id), id: i64::from(add) },
            wrap,
        );
        self.emit("store", Vec::new(), vec![hir::Operand::Place(counter_place), hir::Operand::Value(next_value)], None);
        self.terminate(jump(condition_block));
        self.current = exit_block;
        Ok(())
    }

    pub(super) fn scoped(
        &mut self,
        statements: &[Statement],
    ) -> Result<(), Diagnostic> {
        self.in_scope(|this| this.statements(statements))
    }

    /// Runs `body` in a new scope whose owners drop when it ends.
    pub(super) fn in_scope(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        self.scopes.push(BTreeMap::new());
        let result = body(self);
        if result.is_ok() && self.open() {
            self.drop_scopes(self.scopes.len() - 1);
        }
        self.scopes.pop();
        result
    }
}
