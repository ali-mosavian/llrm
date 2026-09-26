//! Fixed arrays as places: a local, or a field inside a struct (section 5).
//! Each is bytes at a view, as a struct is. A fill or copy of one is a run
//! of cells, stored as the counted loop a local fill is, for the optimizer
//! to price; an element indexed at run time is reached through the
//! array's address.

use super::*;

impl FunctionCompiler<'_> {
    /// The element and shape of the fixed array `expression` is: one it
    /// holds, or one a `&T[N]` value it computes refers to.
    pub(super) fn fixed_array_hint(&self, expression: &Expr) -> Option<(ElementType, Shape)> {
        self.held_array(expression).or_else(|| self.referenced_array(expression))
    }

    /// The array a `&T[N]` value `expression` computes refers to.
    fn referenced_array(&self, expression: &Expr) -> Option<(ElementType, Shape)> {
        match self.types.referent(self.expression_type_hint(expression)?)? {
            ElementType::Struct(id) => self.types.array_of(id),
            ElementType::Scalar(_) => None,
        }
    }

    /// A local array, an array field, or a call's array result.
    fn held_array(&self, expression: &Expr) -> Option<(ElementType, Shape)> {
        match expression {
            Expr::Name(name, _) => match self.visible(name)?.type_ {
                BindingType::Array { element, shape } => Some((element, shape)),
                _ => None,
            },
            Expr::Member { base, field, span } => {
                let parent = self.struct_expression_type(base, *span).ok().flatten().or_else(|| self.struct_type_hint(base, *span))?;
                let field = self.types.structure(parent)?.fields.get(field)?;
                Some((field.type_, field.shape?))
            }
            Expr::Call { name, .. } => self.types.array_of(self.known_signature(name)?.slot?),
            Expr::MethodCall { .. } => self.fixed_array_hint(&self.method_as_call(expression)?),
            _ => None,
        }
    }

    /// Where the fixed array `expression` names keeps its elements, with its
    /// element and shape; `None` when it names none.
    pub(super) fn array_view(&mut self, expression: &Expr, span: Span) -> Result<Option<(StructView, ElementType, Shape)>, Diagnostic> {
        let Some((element, shape)) = self.fixed_array_hint(expression) else {
            return Ok(None);
        };
        let array = self.types.array(element, shape);
        if self.held_array(expression).is_none() {
            // A `&T[N]` value: the array behind it.
            let value = self.expression(expression, None)?;
            let TypeName::Pointer { mutable, .. } = value.type_name else {
                unreachable!("a reference is a pointer")
            };
            let pointer = self.materialized(required(value.clone(), span)?, type_id(value.type_name));
            let view = StructView { struct_id: array, place: 0, pointer: Some(pointer), indices: Vec::new(), offset: 0, mutable, owner: format!("$reference{pointer}") };
            return Ok(Some((view, element, shape)));
        }
        let view = match expression {
            Expr::Name(name, _) => {
                let binding = self.binding(name, span)?.clone();
                let (place, pointer) = match binding.storage {
                    Storage::Place(place) => (place, None),
                    Storage::Reference(pointer) => (0, Some(pointer)),
                    _ => return Ok(None),
                };
                StructView { struct_id: array, place, pointer, indices: Vec::new(), offset: 0, mutable: binding.mutable, owner: name.clone() }
            }
            Expr::Member { base, field, .. } => {
                let parent = self.struct_view(base, span)?;
                let offset = self.types.structure(parent.struct_id).expect("a struct").fields[field].offset;
                StructView { struct_id: array, offset: parent.offset + offset, ..parent }
            }
            Expr::Call { name, arguments, .. } => {
                let result = self.call_into(name, arguments, span)?;
                self.statement_temporary(result)
            }
            Expr::MethodCall { .. } => {
                let call = self.method_as_call(expression).expect("a call");
                return self.array_view(&call, span);
            }
            _ => unreachable!("only a name, a field or a call is a fixed array"),
        };
        Ok(Some((view, element, shape)))
    }

    /// The array field or call result `expression` is, as a binding reached
    /// through its address, as a `&T[N]` is.
    pub(super) fn unnamed_array_binding(&mut self, expression: &Expr, span: Span) -> Result<Option<(Binding, String)>, Diagnostic> {
        if matches!(expression, Expr::Name(..)) {
            return Ok(None);
        }
        let Some((view, element, shape)) = self.array_view(expression, span)? else {
            return Ok(None);
        };
        let pointer = self.array_address(&view);
        let binding = Binding { type_: BindingType::Array { element, shape }, mutable: view.mutable, storage: Storage::Reference(pointer) };
        Ok(Some((binding, view.owner)))
    }

    /// The far address of the array at `view`, typed as a `&T[N]`.
    pub(super) fn array_address(&mut self, view: &StructView) -> u32 {
        let hir::Operand::Value(address) = self.address_of(view) else {
            unreachable!("an address is a value")
        };
        address
    }

    /// `view`, projectable at any byte: a whole array place, which is
    /// projected per element, is reached through its address.
    pub(super) fn byte_view(&mut self, view: &StructView) -> StructView {
        if !self.whole_place(view, view.struct_id) {
            return view.clone();
        }
        let pointer = self.array_address(view);
        StructView { place: 0, pointer: Some(pointer), ..view.clone() }
    }

    /// Whether `view` is the whole of a place of type `array`.
    fn whole_place(&self, view: &StructView, array: u32) -> bool {
        view.pointer.is_none()
            && view.indices.is_empty()
            && view.offset == 0
            && self.places.iter().any(|one| one.id == view.place && one.type_id == array)
    }

    /// Element `index`, counted row-major, of the array of `element` and
    /// `shape` at `array`: a view whose `struct_id` is the element's type. A
    /// place that is the array itself is addressed by its indices.
    pub(super) fn element_view(&self, array: &StructView, element: ElementType, shape: Shape, index: u32) -> StructView {
        if self.whole_place(array, array.struct_id) {
            let indices = shape
                .strides()
                .into_iter()
                .zip(shape.dims())
                .map(|(stride, dim)| hir::Operand::Constant(U16, i64::from(index / stride % dim)))
                .collect();
            return StructView { struct_id: element.id(), indices, ..array.clone() };
        }
        let offset = array.offset + index * self.types.width(element.id());
        StructView { struct_id: element.id(), offset, ..array.clone() }
    }

    /// A far pointer to the first element of the fixed array `binding` holds.
    pub(super) fn array_data(&mut self, binding: &Binding, element: ElementType, span: Span) -> Result<u32, Diagnostic> {
        let data_type = self.types.pointer(element.id(), 0);
        let data = self.value_type(data_type);
        match binding.storage {
            Storage::Place(place) => self.emit("address", vec![data], vec![hir::Operand::Place(place)], None),
            Storage::Reference(pointer) => self.emit("copy", vec![data], vec![hir::Operand::Value(pointer)], None),
            _ => return Err(Diagnostic::new(span, "array has no owned payload")),
        };
        Ok(data)
    }

    /// The stores that write `value` -- an array literal, a repeat, or
    /// another fixed array -- over the array of `element` and `shape` at
    /// `destination`. Values are computed now and stored later, as a
    /// struct's are; a copied array is read as it is stored.
    pub(super) fn prepare_array_stores(
        &mut self,
        destination: &StructView,
        element: ElementType,
        shape: Shape,
        value: &Expr,
        span: Span,
        stores: &mut Vec<Store>,
    ) -> Result<(), Diagnostic> {
        match value {
            Expr::Zero(_) => {
                let (type_name, count) = self.types.array_run(element, shape);
                stores.extend(self.zero_stores(destination, vec![(0, type_name, count)]));
            }
            Expr::Array(..) => {
                let slot = FieldLayout { type_: element, offset: 0, shape: None };
                for (index, (_, item)) in literal_elements(value, shape.dims(), span)?.into_iter().enumerate() {
                    let target = self.element_view(destination, element, shape, index as u32);
                    self.prepare_field_store(&target, slot, item, item.span(), stores)?;
                }
            }
            Expr::Repeat { value: item, counts, .. } => {
                let counts = repeat_counts(counts)?;
                if counts != shape.dims() {
                    return Err(Diagnostic::new(span, format!("array expects dimensions {:?}, got {counts:?}", shape.dims())));
                }
                self.check_repeatable(element, span)?;
                // One value, stored to every element.
                let source = match element {
                    ElementType::Scalar(type_name) => RunSource::Value(required(self.coerced(item, type_name)?, span)?),
                    ElementType::Struct(struct_id) => {
                        let one = self.temporary(struct_id);
                        self.store_struct_expression(&one, item)?;
                        RunSource::Struct(one)
                    }
                };
                stores.push(Store::Run { destination: destination.clone(), element, count: shape.len(), source });
            }
            _ => {
                let Some((source, found, found_shape)) = self.array_view(value, value.span())? else {
                    return Err(Diagnostic::new(value.span(), format!("expected an array of dimensions {:?}", shape.dims())));
                };
                if (found, found_shape) != (element, shape) {
                    return Err(Diagnostic::new(value.span(), "the array has the wrong element type or dimensions"));
                }
                let (type_name, count) = self.types.array_run(element, shape);
                stores.push(Store::Run {
                    destination: destination.clone(),
                    element: ElementType::Scalar(type_name),
                    count,
                    source: RunSource::Cells(source.clone()),
                });
                // An array of owned values moves, as a struct holding them does.
                self.consume_aggregate(value, &source, span, stores)?;
            }
        }
        Ok(())
    }

    /// Refuses a repeat of an owned value, which would have many owners.
    pub(super) fn check_repeatable(&self, element: ElementType, span: Span) -> Result<(), Diagnostic> {
        if self.element_needs_drop(element) {
            return Err(Diagnostic::new(span, "a repeated owned value would have many owners; write each element"));
        }
        Ok(())
    }

    /// Binds `name` to the local array at `place`, holding `value`: a literal
    /// or another array. It owns its elements, as a struct owns its fields.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_array(
        &mut self,
        name: &str,
        mutable: bool,
        place: u32,
        element: ElementType,
        shape: Shape,
        value: &Expr,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let array = self.types.array(element, shape);
        let destination = StructView { struct_id: array, place, pointer: None, indices: Vec::new(), offset: 0, mutable: true, owner: name.into() };
        if let Expr::Array(..) = value {
            // No value reads the new place: each element is stored as it is made.
            let slot = FieldLayout { type_: element, offset: 0, shape: None };
            for (index, (_, item)) in literal_elements(value, shape.dims(), span)?.into_iter().enumerate() {
                let mut stores = Vec::new();
                let target = self.element_view(&destination, element, shape, index as u32);
                self.prepare_field_store(&target, slot, item, item.span(), &mut stores)?;
                self.emit_stores(stores, span)?;
            }
        } else {
            let mut stores = Vec::new();
            self.prepare_array_stores(&destination, element, shape, value, span, &mut stores)?;
            self.emit_stores(stores, span)?;
        }
        let storage = Storage::Place(place);
        if self.element_needs_drop(element) {
            self.own_aggregate(&storage, array);
        }
        let binding = Binding { type_: BindingType::Array { element, shape }, mutable, storage };
        self.scopes.last_mut().expect("scope").insert(name.into(), binding);
        Ok(())
    }

    /// Makes the prepared `stores`, in order.
    pub(super) fn emit_stores(&mut self, stores: Vec<Store>, span: Span) -> Result<(), Diagnostic> {
        for store in stores {
            match store {
                Store::One(place, value) => {
                    self.emit("store", Vec::new(), vec![place, value], None);
                }
                Store::Run { destination, element, count, source } => self.emit_run(&destination, element, count, source, span)?,
            }
        }
        Ok(())
    }

    /// Stores a run of cells as a local fill does, a counted loop over
    /// `cells[at] = source`, which the optimizer prices as a string fill,
    /// a loop, or unrolled stores.
    fn emit_run(&mut self, destination: &StructView, element: ElementType, count: u32, source: RunSource, span: Span) -> Result<(), Diagnostic> {
        let (cells, at) = (self.hidden("cells"), self.hidden("cell"));
        let mut bindings = vec![(cells.clone(), self.cells_binding(destination, element, count, true))];
        let named = self.hidden("source");
        let value = match source {
            RunSource::Value(operand) => {
                let value = self.materialized(operand, element.id());
                let ElementType::Scalar(type_name) = element else { unreachable!("a value fills scalar cells") };
                bindings.push((named.clone(), Binding { type_: BindingType::Scalar(type_name), mutable: false, storage: Storage::Parameter(value) }));
                Expr::Name(named, span)
            }
            RunSource::Struct(view) => {
                let hir::Operand::Value(pointer) = self.address_of(&view) else { unreachable!("an address is a value") };
                bindings.push((named.clone(), Binding { type_: BindingType::Struct(view.struct_id), mutable: false, storage: Storage::Reference(pointer) }));
                Expr::Name(named, span)
            }
            RunSource::Cells(view) => {
                bindings.push((named.clone(), self.cells_binding(&view, element, count, false)));
                Expr::Index { base: Box::new(Expr::Name(named, span)), indices: vec![Expr::Name(at.clone(), span)], span }
            }
        };
        let store = Statement::Assign {
            target: AssignTarget::Index { base: Expr::Name(cells, span), indices: vec![Expr::Name(at.clone(), span)] },
            operation: None,
            value,
            span,
        };
        let walk = Statement::ForRange { name: at, start: Expr::Integer(0, span), end: Expr::Integer(i64::from(count), span), body: vec![store], span };
        self.in_scope(|this| {
            this.scopes.last_mut().expect("scope").extend(bindings);
            // Its index runs over the cells, so none is checked.
            this.statement(&Statement::Unsafe { body: vec![walk], span })
        })
    }

    /// `count` cells of `element` from `view` on, as a one-dimensional array.
    fn cells_binding(&mut self, view: &StructView, element: ElementType, count: u32, mutable: bool) -> Binding {
        let shape = Shape::new(&[count]);
        let array = self.types.array(element, shape);
        let storage = if self.whole_place(view, array) {
            Storage::Place(view.place)
        } else {
            Storage::Reference(self.array_address(&StructView { struct_id: array, ..view.clone() }))
        };
        Binding { type_: BindingType::Array { element, shape }, mutable, storage }
    }
}
