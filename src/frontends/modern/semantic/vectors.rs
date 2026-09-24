//! `vec[T]`: an owned, growable sequence. Its value is a near pointer to its
//! elements behind the descriptor a string has, so one runtime serves both.

use crate::abi::modern as rt;
use super::*;
use crate::frontends::modern::syntax::Pattern;

impl TypeRegistry {
    pub(super) fn vector(&mut self, element: ElementType) -> TypeName {
        if let Some((&type_id, _)) = self.vectors.iter().find(|(_, one)| **one == element) {
            return TypeName::Vector { type_id };
        }
        let element_id = element.id();
        let type_id = self.types.len() as u32 + 1;
        let name = format!("vec[{}]", self.types[(element_id - 1) as usize].name);
        self.types.push(hir::Type {
            id: type_id,
            name,
            kind: "pointer",
            width: 2,
            signed: None,
            evaluation: "none",
            element: Some(element_id),
            rank: 0,
            bounds: Vec::new(),
            address: "near",
        });
        self.vectors.insert(type_id, element);
        TypeName::Vector { type_id }
    }

    /// A heap sequence's element: a string's `char`, a vec's `T`.
    pub(super) fn sequence_element(&self, type_name: TypeName) -> Option<ElementType> {
        match type_name {
            TypeName::String => Some(ElementType::Scalar(TypeName::Char)),
            TypeName::Vector { type_id } => self.vectors.get(&type_id).copied(),
            _ => None,
        }
    }
}

/// Code the compiler made, which cannot fail on a user's account.
pub(super) const GENERATED: Span = Span::new(0, 0, 0);

pub(super) fn builds_vector(expression: &Expr) -> bool {
    matches!(
        expression,
        Expr::Array(..) | Expr::Repeat { .. } | Expr::Comprehension { .. }
    )
}

impl FunctionCompiler<'_> {
    /// `[a, b]`, `[v] * n`, or `[e for x in xs]`, built on the heap.
    pub(super) fn vector_literal(
        &mut self,
        expression: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let type_name = match expected {
            Some(one @ TypeName::Vector { .. }) => one,
            Some(other) => {
                return Err(Diagnostic::new(
                    span,
                    format!("a list literal is a vec, not {}", type_name_text(other)),
                ));
            }
            None => self
                .vector_type_hint(expression)
                .ok_or_else(|| Diagnostic::new(span, "an empty list needs a vec type"))?,
        };
        let element = self.types.sequence_element(type_name).expect("a vec type");
        let size = hir::Operand::Constant(U16, i64::from(self.types.width(element.id())));
        let vector = match expression {
            Expr::Array(items, _) if items.is_empty() => self.empty(type_name),
            Expr::Array(items, _) => {
                let count = hir::Operand::Constant(U16, items.len() as i64);
                let empty = self.empty(type_name);
                let vector = self.grow(empty, type_name, count, size);
                for (index, item) in items.iter().enumerate() {
                    let at = self.element_pointer(
                        vector,
                        element,
                        hir::Operand::Constant(U16, index as i64),
                    );
                    self.store_element(at, element, item)?;
                }
                vector
            }
            Expr::Repeat { value, counts, .. } => {
                let [count] = counts.as_slice() else {
                    return Err(Diagnostic::new(span, "a vec has one dimension"));
                };
                super::repeat_counts(counts)?;
                if self.element_needs_drop(element) {
                    return Err(Diagnostic::new(
                        value.span(),
                        "a repeated element must be copyable",
                    ));
                }
                let ElementType::Scalar(scalar) = element else {
                    return Err(Diagnostic::new(
                        value.span(),
                        "a repeated vec element is a scalar",
                    ));
                };
                let value = self.coerced(value, scalar)?;
                let value = required(value, span)?;
                let count = self.coerced(count, TypeName::U16)?;
                let count = required(count, span)?;
                let empty = self.empty(type_name);
                let vector = self.grow(empty, type_name, count.clone(), size);
                self.counted(count, |this, index| {
                    let at = this.element_pointer(vector, element, index);
                    this.emit("store", Vec::new(), vec![indirect(at, scalar), value], None);
                });
                vector
            }
            Expr::Comprehension {
                element: item,
                clauses,
                ..
            } => {
                // A hidden vec the loop pushes to, moved out when done.
                let name = format!("$vec{}", self.next_place);
                let place = self.place(&name, type_name, true);
                let empty = self.empty(type_name);
                self.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(place), hir::Operand::Value(empty)],
                    None,
                );
                self.own(place);
                self.scopes.last_mut().expect("scope").insert(
                    name.clone(),
                    Binding {
                        type_: BindingType::Scalar(type_name),
                        mutable: true,
                        storage: Storage::Place(place),
                    },
                );
                let push = Statement::Expr(Expr::MethodCall {
                    receiver: Box::new(Expr::Name(name, span)),
                    name: "push".into(),
                    type_arguments: Vec::new(),
                    arguments: vec![(**item).clone()],
                    span,
                });
                for statement in Clause::loops(clauses, vec![push]) {
                    self.statement(&statement)?;
                }
                let built = self.value(type_name);
                self.emit("load", vec![built], vec![hir::Operand::Place(place)], None);
                self.emit(
                    "store",
                    Vec::new(),
                    vec![
                        hir::Operand::Place(place),
                        hir::Operand::Constant(type_id(type_name), 0),
                    ],
                    None,
                );
                built
            }
            _ => unreachable!("builds_vector"),
        };
        Ok(TypedOperand {
            operand: Some(self.temporary_owned(hir::Operand::Value(vector), type_name)),
            type_name,
        })
    }

    /// The vec type of an unannotated literal, from its first element.
    pub(super) fn vector_type_hint(&mut self, expression: &Expr) -> Option<TypeName> {
        let element = match expression {
            Expr::Array(items, _) => self.element_hint(items.first()?)?,
            Expr::Repeat { value, .. } => self.element_hint(value)?,
            Expr::Comprehension {
                element, clauses, ..
            } => {
                let depth = self.scopes.len();
                let hint = self
                    .clause_scopes(clauses)
                    .and_then(|()| self.element_hint(element));
                self.scopes.truncate(depth);
                hint?
            }
            _ => return None,
        };
        Some(self.types.vector(element))
    }

    /// A scope per `for` clause, binding its name to the type of its items,
    /// for hinting what the clauses' value is. `None` when one is unknown.
    pub(super) fn clause_scopes(&mut self, clauses: &[Clause]) -> Option<()> {
        for clause in clauses {
            let Clause::For { pattern, iterable, end, .. } = clause else {
                continue;
            };
            let item = match end {
                Some(_) => ElementType::Scalar(
                    self.expression_type_hint(iterable).unwrap_or(TypeName::I16),
                ),
                None => self.iterated_item(iterable)?,
            };
            let scope = self
                .pattern_bindings(pattern, item)?
                .into_iter()
                .map(|(name, type_)| (name, Binding { type_, mutable: false, storage: Storage::Parameter(0) }))
                .collect();
            self.scopes.push(scope);
        }
        Some(())
    }

    pub(super) fn element_hint(&mut self, expression: &Expr) -> Option<ElementType> {
        if let Some(name) = self.struct_literal_name(expression) {
            return self
                .types
                .resolve_element(&TypeSpec::Named(name), expression.span())
                .ok();
        }
        if builds_vector(expression) {
            return self.vector_type_hint(expression).map(ElementType::Scalar);
        }
        if let Expr::Tuple(items, span) = expression {
            let elements: Option<Vec<_>> = items.iter().map(|item| Some((self.element_hint(item)?, None))).collect();
            return self.types.tuple(&elements?, *span).ok().map(ElementType::Struct);
        }
        // An integer constant with no type of its own is an i16.
        let constant = || crate::frontends::modern::consts::folded(expression, &BTreeMap::new());
        self.expression_type_hint(expression)
            .or_else(|| matches!(constant(), Some(Expr::Integer(..))).then_some(TypeName::I16))
            .map(ElementType::Scalar)
    }

    fn struct_literal_name(&self, expression: &Expr) -> Option<String> {
        match expression {
            Expr::StructLiteral { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    /// What `for x in source` binds `x` to.
    /// What a `for` over `iterable` binds, when that is known.
    pub(super) fn iterated_item(&mut self, iterable: &Expr) -> Option<ElementType> {
        match iterable {
            Expr::Name(source, span) => {
                let source = self.binding(source, *span).ok()?.clone();
                self.iterated_element(&source)
            }
            Expr::Slice { base, .. } => self.iterated_item(base),
            _ if self.fixed_array_hint(iterable).is_some_and(|(_, shape)| shape.rank == 1) => {
                self.fixed_array_hint(iterable).map(|(element, _)| element)
            }
            _ if builds_vector(iterable) => self
                .vector_type_hint(iterable)
                .and_then(|one| self.types.sequence_element(one)),
            _ => self.generated_item(iterable).or_else(|| {
                let type_name = self.expression_type_hint(iterable)?;
                self.types.sequence_element(type_name)
            }),
        }
    }

    /// What indexing `source` gives, at any rank.
    pub(super) fn indexed_element(&self, source: &Binding) -> Option<ElementType> {
        match source.type_ {
            BindingType::Scalar(type_name) => self.types.indexed(type_name),
            other => other.ranked().map(|(element, _, _)| element),
        }
    }

    pub(super) fn iterated_element(&self, source: &Binding) -> Option<ElementType> {
        match source.type_ {
            BindingType::Scalar(type_name) => self.types.sequence_element(type_name),
            other => other.array().map(|(element, _)| element),
        }
    }

    /// The empty string literal's descriptor, which an empty vec shares:
    /// static, so the first growth allocates.
    pub(super) fn empty(&mut self, type_name: TypeName) -> u32 {
        let text = self
            .string_literal(b"", Some(TypeName::String), GENERATED)
            .expect("a literal");
        self.retyped(required(text, GENERATED).expect("a value"), type_name)
    }

    pub(super) fn retyped(&mut self, operand: hir::Operand, to: TypeName) -> u32 {
        let result = self.value(to);
        self.emit("copy", vec![result], vec![operand], None);
        result
    }

    /// `vector` with `count` more elements; it may move.
    fn grow(
        &mut self,
        vector: u32,
        type_name: TypeName,
        count: hir::Operand,
        size: hir::Operand,
    ) -> u32 {
        let grown = self
            .emit_builtin(rt::BUFFER_GROW, vec![hir::Operand::Value(vector), count, size])
            .expect("a pointer");
        self.retyped(grown, type_name)
    }

    pub(super) fn length(&mut self, vector: u32) -> hir::Operand {
        let length = self.value(TypeName::U16);
        self.emit(
            "load",
            vec![length],
            vec![hir::Operand::DescriptorPlace {
                base: vector,
                field: "length",
                type_id: U16,
            }],
            None,
        );
        hir::Operand::Value(length)
    }

    /// `&v` or `&v[a:b]` for a `&[T]`: a view of the vec's elements.
    /// A view of the string or vec `operand` computes, which lives as long
    /// as the statement's temporaries.
    pub(super) fn value_view(
        &mut self,
        operand: &Expr,
        element: ElementType,
        pointer_type: u32,
    ) -> Result<(hir::Operand, String), Diagnostic> {
        let span = operand.span();
        let value = self.expression(operand, None)?;
        if self.types.sequence_element(value.type_name) != Some(element) {
            return Err(Diagnostic::new(
                span,
                "the borrowed value has the wrong element type",
            ));
        }
        let type_name = value.type_name;
        let value = match required(value, span)? {
            hir::Operand::Value(value) => value,
            other => {
                let copy = self.value(type_name);
                self.emit("copy", vec![copy], vec![other], None);
                copy
            }
        };
        let binding = Binding {
            type_: BindingType::Scalar(type_name),
            mutable: false,
            storage: Storage::Parameter(value),
        };
        let name = format!("$value{value}");
        let view = self.sequence_view(&binding, &name, element, None, pointer_type, span)?;
        Ok((view, name))
    }

    pub(super) fn sequence_view(
        &mut self,
        binding: &Binding,
        name: &str,
        element: ElementType,
        range: Option<(Option<&Expr>, Option<&Expr>, Span)>,
        pointer_type: u32,
        span: Span,
    ) -> Result<hir::Operand, Diagnostic> {
        let vector = self.string_pointer(binding, span)?;
        let length = self.length(vector);
        // The far address of the first element, through the vec's near pointer.
        let data_type = self.types.pointer(element.id(), 0);
        let data = self.value_type(data_type);
        let first = hir::Operand::IndirectPlace {
            base: vector,
            offset: 0,
            type_id: element.id(),
            inbounds: false,
        };
        self.emit("address", vec![data], vec![first], None);
        self.ranged_view(name, data, length, element, range, pointer_type, span)
    }

    /// A view of `range` of the `length` elements from the far `data`:
    /// every slice is made here, and checked to lie within `length`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn ranged_view(
        &mut self,
        name: &str,
        data: u32,
        length: hir::Operand,
        element: ElementType,
        range: Option<(Option<&Expr>, Option<&Expr>, Span)>,
        pointer_type: u32,
        span: Span,
    ) -> Result<hir::Operand, Diagnostic> {
        let (start, end) = match range {
            None => (None, length),
            Some((start, end, range_span)) => {
                let mut bound = |one: &Expr| self.coerced(one, TypeName::U16).and_then(|one| required(one, span));
                let start = start.map(&mut bound).transpose()?;
                let end = match end.map(&mut bound).transpose()? {
                    Some(end) => {
                        self.check_slice_bound(&end, length, range_span)?;
                        end
                    }
                    None => length,
                };
                if let Some(start) = &start {
                    self.check_slice_bound(start, end.clone(), range_span)?;
                }
                (start, end)
            }
        };
        let (data, count) = match start {
            None | Some(hir::Operand::Constant(_, 0)) => (data, end),
            Some(start) => {
                let width = self.types.width(element.id());
                let data = self.indexed_pointer(data, start.clone(), width, span)?;
                let count = match (&start, &end) {
                    (hir::Operand::Constant(_, first), hir::Operand::Constant(_, last)) => hir::Operand::Constant(U16, last - first),
                    _ => {
                        let count = self.value(TypeName::U16);
                        self.emit("sub", vec![count], vec![end, start], None);
                        hir::Operand::Value(count)
                    }
                };
                (data, count)
            }
        };
        Ok(self.view_descriptor(name, pointer_type, vec![count.clone(), count], data))
    }

    /// A string keeps a NUL after its last char (section 9).
    fn terminated(&mut self, sequence: u32, type_name: TypeName) {
        if type_name != TypeName::String {
            return;
        }
        let length = self.length(sequence);
        let end = self.element_pointer(sequence, ElementType::Scalar(TypeName::Char), length);
        self.emit("store", Vec::new(), vec![indirect(end, TypeName::Char), hir::Operand::Constant(type_id(TypeName::Char), 0)], None);
    }

    /// Shrinks the vec `receiver` names by one: the vec, and a pointer to
    /// the element it no longer holds, now its taker's.
    fn popped(&mut self, receiver: &Expr, type_name: TypeName, element: ElementType, span: Span) -> Result<(u32, u32), Diagnostic> {
        let place = self.sequence_place(receiver, span)?;
        let vector = self.value(type_name);
        self.emit("load", vec![vector], vec![place], None);
        let index = self
            .emit_builtin(
                rt::BUFFER_SHRINK,
                vec![hir::Operand::Value(vector), hir::Operand::Constant(U16, 1)],
            )
            .expect("a length");
        Ok((vector, self.element_pointer(vector, element, index)))
    }

    /// The struct element of a vec in `receiver`, if `v.pop()` is one.
    pub(super) fn popped_struct_type(&self, expression: &Expr) -> Option<u32> {
        let Expr::MethodCall { receiver, name, arguments, .. } = expression else {
            return None;
        };
        if name != "pop" || !arguments.is_empty() {
            return None;
        }
        match self.types.sequence_element(self.expression_type_hint(receiver)?)? {
            ElementType::Struct(id) => Some(id),
            ElementType::Scalar(_) => None,
        }
    }

    /// `v.pop()` of a vec of structs: the last element, moved into a
    /// statement temporary.
    pub(super) fn popped_struct(&mut self, receiver: &Expr, span: Span) -> Result<StructView, Diagnostic> {
        let type_name = self.expression_type_hint(receiver).expect("a vec");
        let element = self.types.sequence_element(type_name).expect("a vec");
        let ElementType::Struct(struct_id) = element else {
            unreachable!("a struct element")
        };
        let (_, at) = self.popped(receiver, type_name, element, span)?;
        let taken = self.temporary(struct_id);
        let mut stores = Vec::new();
        self.prepare_struct_copy(&taken, &element_view(struct_id, at), &mut stores)?;
        self.emit_stores(stores, span)?;
        Ok(self.statement_temporary(taken))
    }

    fn element_pointer(&mut self, vector: u32, element: ElementType, index: hir::Operand) -> u32 {
        let width = self.types.width(element.id());
        self.indexed_pointer(vector, index, width, GENERATED)
            .expect("a typed index")
    }

    /// Moves `value` into the element `at` points to.
    fn store_element(
        &mut self,
        at: u32,
        element: ElementType,
        value: &Expr,
    ) -> Result<(), Diagnostic> {
        match element {
            ElementType::Scalar(type_name) => {
                let span = value.span();
                let value = self.coerced(value, type_name)?;
                self.consume(&value, span)?;
                let value = required(value, span)?;
                self.emit(
                    "store",
                    Vec::new(),
                    vec![indirect(at, type_name), value],
                    None,
                );
                Ok(())
            }
            ElementType::Struct(struct_id) => {
                self.store_struct_expression(&element_view(struct_id, at), value)
            }
        }
    }

    /// `v.push(x)`, `v.pop()`, and `v.copy()`; `None` when `receiver` is not a vec.
    pub(super) fn vector_method(
        &mut self,
        receiver: &Expr,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) -> Result<Option<TypedOperand>, Diagnostic> {
        let Some(type_name @ (TypeName::Vector { .. } | TypeName::String)) = self.expression_type_hint(receiver) else {
            return Ok(None);
        };
        let element = self.types.sequence_element(type_name).expect("a sequence type");
        let size = hir::Operand::Constant(U16, i64::from(self.types.width(element.id())));
        match (name, arguments) {
            ("push", [value]) => {
                let settled = self.settled_failure(value, span)?;
                let value = settled.as_ref().unwrap_or(value);
                let place = self.sequence_place(receiver, span)?;
                let vector = self.value(type_name);
                self.emit("load", vec![vector], vec![place.clone()], None);
                let index = self.length(vector);
                let grown = self.grow(vector, type_name, hir::Operand::Constant(U16, 1), size);
                self.emit(
                    "store",
                    Vec::new(),
                    vec![place, hir::Operand::Value(grown)],
                    None,
                );
                let at = self.element_pointer(grown, element, index);
                self.store_element(at, element, value)?;
                self.terminated(grown, type_name);
                Ok(Some(TypedOperand {
                    operand: None,
                    type_name: TypeName::Void,
                }))
            }
            ("pop", []) => {
                let ElementType::Scalar(scalar) = element else {
                    // Popped as a statement: the struct drops when it ends.
                    self.popped_struct(receiver, span)?;
                    return Ok(Some(TypedOperand { operand: None, type_name: TypeName::Void }));
                };
                let (vector, at) = self.popped(receiver, type_name, element, span)?;
                let value = self.value(scalar);
                self.emit("load", vec![value], vec![indirect(at, scalar)], None);
                self.terminated(vector, type_name);
                Ok(Some(TypedOperand {
                    operand: Some(self.temporary_owned(hir::Operand::Value(value), scalar)),
                    type_name: scalar,
                }))
            }
            ("copy", []) => {
                self.check_copyable(ElementType::Scalar(type_name), span)?;
                let vector = self.coerced(receiver, type_name)?;
                let copy = self.emit_copy(required(vector, span)?, type_name);
                Ok(Some(TypedOperand {
                    operand: Some(self.temporary_owned(copy, type_name)),
                    type_name,
                }))
            }
            ("push" | "pop" | "copy", _) => Err(Diagnostic::new(
                span,
                format!("wrong arguments to {name}"),
            )),
            _ => Ok(None),
        }
    }

    /// Where a string, vec or dict being written is kept.
    pub(super) fn sequence_place(&mut self, receiver: &Expr, span: Span) -> Result<hir::Operand, Diagnostic> {
        let target =
            AssignTarget::of(receiver.clone()).map_err(|message| Diagnostic::new(span, message))?;
        if let AssignTarget::Name(name) = &target {
            let binding = self.binding(name, span)?;
            if !binding.mutable {
                return Err(Diagnostic::new(
                    span,
                    format!("binding {name:?} is immutable"),
                ));
            }
        }
        match self.assignment_target(&target, span)? {
            AssignmentPlace::Scalar(place, _) => Ok(place),
            AssignmentPlace::Struct(_) | AssignmentPlace::Array(..) | AssignmentPlace::Bits { .. } => {
                unreachable!("a vec is a scalar")
            }
        }
    }

    /// A heap copy of a string or vec, and of everything its elements own.
    pub(super) fn emit_copy(&mut self, operand: hir::Operand, type_name: TypeName) -> hir::Operand {
        let element = self.types.owned_element(type_name).expect("an owning buffer");
        let size = hir::Operand::Constant(U16, i64::from(self.types.width(element.id())));
        let copy = self
            .emit_builtin(rt::BUFFER_CLONE, vec![operand, size])
            .expect("a pointer");
        let copy = self.retyped(copy, type_name);
        if self.element_needs_drop(element) {
            self.each_element(copy, element, Owned::Duplicate);
        }
        hir::Operand::Value(copy)
    }

    /// Applies `action` to what each element of `vector` owns.
    pub(super) fn each_element(&mut self, vector: u32, element: ElementType, action: Owned) {
        let length = self.length(vector);
        self.counted(length, |this, index| {
            let at = this.element_pointer(vector, element, index);
            match element {
                ElementType::Scalar(type_name) => {
                    this.owned_leaf(indirect(at, type_name), type_name, action)
                }
                ElementType::Struct(struct_id) => {
                    this.each_owned(&element_view(struct_id, at), action)
                }
            }
        });
    }

    /// Emits `body` once per index in `0..count`.
    pub(super) fn counted(
        &mut self,
        count: hir::Operand,
        body: impl FnOnce(&mut Self, hir::Operand),
    ) {
        let name = format!("$index{}", self.next_place);
        let index_place = self.place(&name, TypeName::U16, true);
        self.emit(
            "store",
            Vec::new(),
            vec![
                hir::Operand::Place(index_place),
                hir::Operand::Constant(U16, 0),
            ],
            None,
        );
        let (condition, exit) = (self.block(), self.block());
        self.terminate(jump(condition));
        self.current = condition;
        let index = self.value(TypeName::U16);
        self.emit(
            "load",
            vec![index],
            vec![hir::Operand::Place(index_place)],
            None,
        );
        self.branch_unless("below", hir::Operand::Value(index), count, exit);
        body(self, hir::Operand::Value(index));
        let next = self.value(TypeName::U16);
        self.emit(
            "add",
            vec![next],
            vec![hir::Operand::Value(index), hir::Operand::Constant(U16, 1)],
            None,
        );
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(index_place), hir::Operand::Value(next)],
            None,
        );
        self.terminate(jump(condition));
        self.current = exit;
    }
}

fn indirect(base: u32, type_name: TypeName) -> hir::Operand {
    hir::Operand::IndirectPlace {
        base,
        offset: 0,
        type_id: type_id(type_name),
        inbounds: false,
    }
}

fn element_view(struct_id: u32, at: u32) -> StructView {
    StructView {
        struct_id,
        place: 0,
        pointer: Some(at),
        indices: Vec::new(),
        offset: 0,
        mutable: true,
        owner: "a vec element".into(),
    }
}
