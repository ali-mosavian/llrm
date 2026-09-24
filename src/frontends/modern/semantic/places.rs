//! Places: where assignment and struct stores write.

use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn initialize_struct(
        &mut self,
        place: u32,
        indices: Vec<hir::Operand>,
        struct_id: u32,
        expression: &Expr,
    ) -> Result<(), Diagnostic> {
        self.store_struct_expression(
            &StructView {
                struct_id,
                place,
                pointer: None,
                indices,
                offset: 0,
                mutable: true,
                owner: "array initializer".into(),
            },
            expression,
        )
    }

    pub(super) fn store_struct_expression(
        &mut self,
        destination: &StructView,
        expression: &Expr,
    ) -> Result<(), Diagnostic> {
        let mut stores = Vec::new();
        self.prepare_struct_stores(destination, expression, &mut stores)?;
        for (place, value) in stores {
            self.emit("store", Vec::new(), vec![place, value], None);
        }
        Ok(())
    }

    pub(super) fn prepare_struct_stores(
        &mut self,
        destination: &StructView,
        expression: &Expr,
        stores: &mut Vec<(hir::Operand, hir::Operand)>,
    ) -> Result<(), Diagnostic> {
        let layout = self
            .types
            .structure(destination.struct_id)
            .cloned()
            .expect("resolved struct type");
        if let Expr::Variant {
            enum_name,
            name,
            arguments,
            span,
        } = expression
        {
            return self.prepare_variant_stores(
                destination,
                enum_name.as_deref(),
                name,
                arguments,
                *span,
                stores,
            );
        }
        if let Some(call) = self.method_as_call(expression) {
            return self.prepare_struct_stores(destination, &call, stores);
        }
        if let Expr::Call {
            name,
            arguments,
            span,
            ..
        } = expression
        {
            let source = self.call_into(name, arguments, *span)?;
            return self.prepare_struct_copy(destination, &source, stores);
        }
        if let Expr::Tuple(items, span) = expression {
            let layout = self
                .types
                .structure(destination.struct_id)
                .expect("resolved struct type")
                .clone();
            let literal = Self::tuple_literal(items, &layout, *span)?;
            return self.prepare_struct_stores(destination, &literal, stores);
        }
        if let Expr::Try { operand, span } = expression {
            let source = self.try_view(operand, *span)?;
            return self.prepare_struct_copy(destination, &source, stores);
        }
        if let Expr::Conditional {
            condition,
            then,
            otherwise,
            span,
        } = expression
        {
            let source =
                self.conditional_view(condition, then, otherwise, destination.struct_id, *span)?;
            return self.prepare_struct_copy(destination, &source, stores);
        }
        let Expr::StructLiteral { name, fields, span } = expression else {
            let source = self.struct_view(expression, expression.span())?;
            if source.struct_id != destination.struct_id {
                let found = &self
                    .types
                    .structure(source.struct_id)
                    .expect("resolved struct type")
                    .name;
                return Err(Diagnostic::new(
                    expression.span(),
                    format!("expected {}, found {found}", layout.name),
                ));
            }
            self.prepare_struct_copy(destination, &source, stores)?;
            return self.consume_aggregate(expression, &source, expression.span());
        };
        // A generic literal builds whichever instance is expected.
        if self.types.template_of(name) != self.types.template_of(&layout.name) {
            return Err(Diagnostic::new(
                *span,
                format!("expected {}, found {name}", layout.name),
            ));
        }
        let mut seen = BTreeMap::new();
        for (name, value, field_span) in fields {
            if seen.insert(name, *field_span).is_some() {
                return Err(Diagnostic::new(
                    *field_span,
                    format!("field {name:?} is initialized more than once"),
                ));
            }
            let field = layout.fields.get(name).ok_or_else(|| {
                Diagnostic::new(
                    *field_span,
                    format!("{} has no field {name:?}", layout.name),
                )
            })?;
            self.prepare_field_store(destination, *field, value, *field_span, stores)?;
        }
        let missing: Vec<_> = layout
            .fields
            .keys()
            .filter(|name| !seen.contains_key(*name))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(Diagnostic::new(
                *span,
                format!(
                    "{} literal is missing fields: {}",
                    layout.name,
                    missing.join(", ")
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn prepare_field_store(
        &mut self,
        destination: &StructView,
        field: FieldLayout,
        value: &Expr,
        span: Span,
        stores: &mut Vec<(hir::Operand, hir::Operand)>,
    ) -> Result<(), Diagnostic> {
        match field.type_ {
            ElementType::Scalar(type_name) => {
                let value = self.coerced(value, type_name)?;
                self.consume(&value, span)?;
                stores.push((
                    self.projected_place(destination, field.offset, type_name),
                    required(value, span)?,
                ));
            }
            ElementType::Struct(field_struct) => {
                let nested = StructView {
                    struct_id: field_struct,
                    place: destination.place,
                    pointer: destination.pointer,
                    indices: destination.indices.clone(),
                    offset: destination.offset + field.offset,
                    mutable: destination.mutable,
                    owner: destination.owner.clone(),
                };
                self.prepare_struct_stores(&nested, value, stores)?;
            }
        }
        Ok(())
    }

    pub(super) fn prepare_struct_copy(
        &mut self,
        destination: &StructView,
        source: &StructView,
        stores: &mut Vec<(hir::Operand, hir::Operand)>,
    ) -> Result<(), Diagnostic> {
        let copy = self
            .types
            .copy_units(ElementType::Struct(destination.struct_id));
        for (offset, type_name) in copy {
            let value = self.value(type_name);
            self.emit(
                "load",
                vec![value],
                vec![self.projected_place(source, offset, type_name)],
                None,
            );
            stores.push((
                self.projected_place(destination, offset, type_name),
                hir::Operand::Value(value),
            ));
        }
        Ok(())
    }

    pub(super) fn projected_place(
        &self,
        view: &StructView,
        field_offset: u32,
        type_name: TypeName,
    ) -> hir::Operand {
        if let Some(pointer) = view.pointer {
            hir::Operand::IndirectPlace {
                base: pointer,
                offset: view.offset + field_offset,
                type_id: type_id(type_name),
                inbounds: false,
            }
        } else {
            hir::Operand::ProjectedPlace {
                place: view.place,
                indices: view.indices.clone(),
                offset: view.offset + field_offset,
                type_id: type_id(type_name),
            }
        }
    }

    pub(super) fn struct_expression_type(
        &self,
        expression: &Expr,
        span: Span,
    ) -> Result<Option<u32>, Diagnostic> {
        if let Some(call) = self.method_as_call(expression) {
            return self.struct_expression_type(&call, span);
        }
        match expression {
            Expr::StructLiteral { name, .. } if self.types.bits.contains_key(name) => Ok(None),
            Expr::Tuple(items, _) => self
                .tuple_elements(items, span)
                .and_then(|elements| self.types.tuple_id(&elements))
                .map(Some)
                .ok_or_else(|| Diagnostic::new(span, "a tuple's element types must be known")),
            Expr::StructLiteral { name, .. } => self
                .types
                .structs
                .get(name)
                .map(|one| Some(one.id))
                .ok_or_else(|| Diagnostic::new(span, format!("unknown struct {name:?}"))),
            // A function's name is a value of its type, not a place.
            Expr::Name(name, _) if self.visible(name).is_none() && self.signatures.contains_key(name) => Ok(None),
            Expr::Name(name, _) => Ok(match self.binding(name, span)?.type_ {
                BindingType::Struct(struct_id) => Some(struct_id),
                _ => None,
            }),
            Expr::Call { name, .. } => Ok(self.known_signature(name).and_then(|one| one.slot)),
            Expr::Unary { op: UnaryOp::Deref, operand, .. } => Ok(match self.expression_type_hint(operand).and_then(|one| self.types.raw_target(one)) {
                Some(ElementType::Struct(struct_id)) => Some(struct_id),
                _ => None,
            }),
            Expr::MethodCall {
                receiver,
                name,
                arguments,
                ..
            } if name == "copy" && arguments.is_empty() => {
                self.struct_expression_type(receiver, span)
            }
            Expr::Try { operand, .. } => self.try_struct_type(operand, span),
            Expr::Variant {
                enum_name: Some(enum_name),
                ..
            } => Ok(
                match self.types.enums.get(enum_name).map(|one| one.element) {
                    Some(ElementType::Struct(struct_id)) => Some(struct_id),
                    _ => None,
                },
            ),
            Expr::Index { base, .. } => {
                let Expr::Name(name, _) = base.as_ref() else {
                    return Ok(None);
                };
                let binding = self.binding(name, span)?;
                Ok(match self.indexed_element(binding) {
                    Some(ElementType::Struct(struct_id)) => Some(struct_id),
                    _ => None,
                })
            }
            Expr::Member { base, field, .. } => {
                let Some(struct_id) = self.struct_expression_type(base, span)? else {
                    return Ok(None);
                };
                let layout = self
                    .types
                    .structure(struct_id)
                    .expect("resolved struct type");
                let field = layout.fields.get(field).ok_or_else(|| {
                    Diagnostic::new(span, format!("{} has no field {field:?}", layout.name))
                })?;
                Ok(match field.type_ {
                    ElementType::Struct(struct_id) => Some(struct_id),
                    ElementType::Scalar(_) => None,
                })
            }
            _ => Ok(None),
        }
    }

    pub(super) fn assignment_target(
        &mut self,
        target: &AssignTarget,
        span: Span,
    ) -> Result<AssignmentPlace, Diagnostic> {
        if let Some(owner) = borrows::written_owner(target) {
            self.check_unborrowed(owner, span)?;
        }
        if let AssignTarget::Member { base, field } = target {
            self.check_mutable_fields(&Expr::Member { base: Box::new(base.clone()), field: field.clone(), span })?;
        }
        match target {
            AssignTarget::Deref(pointer) => {
                let name = self.dereferenced(pointer, span)?;
                self.assignment_target(&AssignTarget::Name(name), span)
            }
            AssignTarget::Member { base, field } if self.bits_type(base).is_some() => {
                let outer = AssignTarget::of(base.clone())
                    .map_err(|message| Diagnostic::new(span, message))?;
                let packed = self.bits_type(base).expect("checked");
                let field = self.types.bit_field(packed, field, span)?;
                Ok(match self.assignment_target(&outer, span)? {
                    AssignmentPlace::Scalar(place, packed) => AssignmentPlace::Bits {
                        place,
                        packed,
                        field,
                    },
                    // A field of a nested bits struct is a narrower range of the outer one.
                    AssignmentPlace::Bits {
                        place,
                        packed,
                        field: parent,
                    } => AssignmentPlace::Bits {
                        place,
                        packed,
                        field: bits::BitField {
                            low: parent.low + field.low,
                            ..field
                        },
                    },
                    AssignmentPlace::Struct(_) => unreachable!("a bits value is a scalar"),
                })
            }
            AssignTarget::Member { base, field } => {
                let parent = self.struct_view(base, span)?;
                if !parent.mutable {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {:?} is immutable", parent.owner),
                    ));
                }
                let layout = self
                    .types
                    .structure(parent.struct_id)
                    .expect("resolved struct type");
                let member = layout.fields.get(field).copied().ok_or_else(|| {
                    Diagnostic::new(span, format!("{} has no field {field:?}", layout.name))
                })?;
                Ok(match member.type_ {
                    ElementType::Scalar(type_name) => AssignmentPlace::Scalar(
                        self.projected_place(&parent, member.offset, type_name),
                        type_name,
                    ),
                    ElementType::Struct(struct_id) => AssignmentPlace::Struct(StructView {
                        struct_id,
                        place: parent.place,
                        pointer: parent.pointer,
                        indices: parent.indices,
                        offset: parent.offset + member.offset,
                        mutable: true,
                        owner: parent.owner,
                    }),
                })
            }
            AssignTarget::Name(name) => {
                let binding = self.binding(name, span)?.clone();
                if !binding.mutable {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {name:?} is immutable"),
                    ));
                }
                match binding.type_ {
                    BindingType::Scalar(type_name) => {
                        let destination = match binding.storage {
                            Storage::Place(place) => hir::Operand::Place(place),
                            Storage::ArrayView { place, index } => {
                                hir::Operand::ArrayElement(place, vec![index])
                            }
                            Storage::Parameter(_) => {
                                return Err(Diagnostic::new(span, "parameters are immutable"));
                            }
                            Storage::Reference(pointer) => hir::Operand::IndirectPlace {
                                base: pointer,
                                offset: 0,
                                type_id: type_id(type_name),
                                inbounds: false,
                            },
                            Storage::Slice(_) => unreachable!("a scalar binding is not a slice"),
                            Storage::Lambda(_) => {
                                return Err(Diagnostic::new(span, "a lambda cannot be assigned"));
                            }
                        };
                        Ok(AssignmentPlace::Scalar(destination, type_name))
                    }
                    BindingType::Struct(struct_id) => {
                        binding_view(struct_id, &binding.storage, true, name)
                            .map(AssignmentPlace::Struct)
                            .ok_or_else(|| Diagnostic::new(span, "parameters are immutable"))
                    }
                    BindingType::Array { .. } | BindingType::Slice { .. } => Err(Diagnostic::new(
                        span,
                        "whole array assignment is not supported",
                    )),
                }
            }
            AssignTarget::Index { base, indices } if self.types.dictionary_parts(self.expression_type_hint(&Expr::Name(base.clone(), span)).unwrap_or(TypeName::Void)).is_some() => {
                let entry = self.dictionary_entry(base, indices, span)?.expect("a dict");
                let target = AssignTarget::of(entry).map_err(|message| Diagnostic::new(span, message))?;
                self.assignment_target(&target, span)
            }
            AssignTarget::Index { base, indices } => {
                let binding = self.binding(base, span)?.clone();
                if !binding.mutable {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {base:?} is immutable"),
                    ));
                }
                if binding.type_ == BindingType::Scalar(TypeName::String) {
                    return self.string_element_target(base, indices, span);
                }
                let (element, at) = self.element_at(&binding, base, indices, span)?;
                Ok(match element {
                    ElementType::Scalar(type_name) => {
                        AssignmentPlace::Scalar(at.operand(type_id(type_name)), type_name)
                    }
                    ElementType::Struct(struct_id) => {
                        let (place, pointer, indices) = at.parts();
                        AssignmentPlace::Struct(StructView {
                            struct_id,
                            place,
                            pointer,
                            indices,
                            offset: 0,
                            mutable: true,
                            owner: base.clone(),
                        })
                    }
                })
            }
        }
    }

    pub(super) fn member_place(
        &mut self,
        base: &Expr,
        field_name: &str,
        span: Span,
    ) -> Result<(hir::Operand, TypeName, bool, String), Diagnostic> {
        let view = self.struct_view(base, span)?;
        let layout = self
            .types
            .structure(view.struct_id)
            .expect("resolved struct type");
        let field = layout.fields.get(field_name).copied().ok_or_else(|| {
            Diagnostic::new(span, format!("{} has no field {field_name:?}", layout.name))
        })?;
        let ElementType::Scalar(type_name) = field.type_ else {
            return Err(Diagnostic::new(
                span,
                "a nested struct value must be used through one of its fields",
            ));
        };
        Ok((
            self.projected_place(&view, field.offset, type_name),
            type_name,
            view.mutable,
            view.owner,
        ))
    }

    pub(super) fn struct_view(&mut self, expression: &Expr, span: Span) -> Result<StructView, Diagnostic> {
        // A reference to a struct, as a call returns one, views its target.
        if let Some(ElementType::Struct(struct_id)) = self.expression_type_hint(expression).and_then(|one| self.types.referent(one)) {
            if !matches!(expression, Expr::Name(..)) {
                let value = self.expression(expression, None)?;
                let pointer_type = self.types.pointer(struct_id, 0);
                let pointer = self.materialized(required(value, span)?, pointer_type);
                return Ok(StructView { struct_id, place: 0, pointer: Some(pointer), indices: Vec::new(), offset: 0, mutable: false, owner: format!("$reference{pointer}") });
            }
        }
        match expression {
            Expr::Unary { op: UnaryOp::Deref, operand, span } => {
                let name = self.dereferenced(operand, *span)?;
                self.struct_view(&Expr::Name(name, *span), *span)
            }
            Expr::Name(name, _) => {
                let binding = self.binding(name, span)?.clone();
                let BindingType::Struct(struct_id) = binding.type_ else {
                    return Err(Diagnostic::new(
                        span,
                        format!("binding {name:?} is not a struct"),
                    ));
                };
                binding_view(struct_id, &binding.storage, binding.mutable, name)
                    .ok_or_else(|| Diagnostic::new(span, "struct has no addressable storage"))
            }
            Expr::Index { base, indices, .. } => {
                if let Some(value) = self.dictionary_value(base, indices, span)? {
                    return self.struct_view(&value, span);
                }
                let (binding, name) = self.sequence_of(base)?;
                let (element, at) = self.element_at(&binding, &name, indices, span)?;
                let ElementType::Struct(struct_id) = element else {
                    return Err(Diagnostic::new(span, "array element is not a struct"));
                };
                let (place, pointer, indices) = at.parts();
                Ok(StructView {
                    struct_id,
                    place,
                    pointer,
                    indices,
                    offset: 0,
                    mutable: binding.mutable,
                    owner: name.clone(),
                })
            }
            Expr::Member {
                base,
                field,
                span: member_span,
            } => {
                let parent = self.struct_view(base, *member_span)?;
                let layout = self
                    .types
                    .structure(parent.struct_id)
                    .expect("resolved struct type");
                let field = layout.fields.get(field).copied().ok_or_else(|| {
                    Diagnostic::new(
                        *member_span,
                        format!("{} has no field {field:?}", layout.name),
                    )
                })?;
                let ElementType::Struct(field_struct) = field.type_ else {
                    return Err(Diagnostic::new(
                        *member_span,
                        "scalar field cannot be used as a struct",
                    ));
                };
                Ok(StructView {
                    struct_id: field_struct,
                    place: parent.place,
                    pointer: parent.pointer,
                    indices: parent.indices,
                    offset: parent.offset + field.offset,
                    mutable: parent.mutable,
                    owner: parent.owner,
                })
            }
            Expr::Call {
                name,
                arguments,
                span,
                ..
            } => {
                let result = self.call_into(name, arguments, *span)?;
                Ok(self.statement_temporary(result))
            }
            Expr::MethodCall { .. } if self.method_as_call(expression).is_some() => {
                let call = self.method_as_call(expression).expect("checked");
                self.struct_view(&call, span)
            }
            // `.copy()`: the bytes, then a copy of each thing they own.
            Expr::MethodCall {
                receiver,
                name,
                arguments,
                ..
            } if name == "copy" && arguments.is_empty() => {
                let source = self.struct_view(receiver, span)?;
                self.check_copyable(ElementType::Struct(source.struct_id), span)?;
                let copy = self.temporary(source.struct_id);
                let mut stores = Vec::new();
                self.prepare_struct_copy(&copy, &source, &mut stores)?;
                for (place, value) in stores {
                    self.emit("store", Vec::new(), vec![place, value], None);
                }
                self.each_owned(&copy, Owned::Duplicate);
                Ok(self.statement_temporary(copy))
            }
            // A literal, variant, tuple or conditional: built in a temporary.
            _ => match self.struct_expression_type(expression, span)? {
                Some(struct_id) => {
                    let view = self.temporary(struct_id);
                    self.store_struct_expression(&view, expression)?;
                    Ok(self.statement_temporary(view))
                }
                None => Err(Diagnostic::new(
                    span,
                    "expression is not an addressable struct",
                )),
            },
        }
    }

    /// An aggregate that lives until the statement ends, then drops what it owns.
    pub(super) fn statement_temporary(&mut self, view: StructView) -> StructView {
        if self.element_needs_drop(ElementType::Struct(view.struct_id)) {
            self.aggregate_temporaries.push(view.clone());
        }
        view
    }
}
