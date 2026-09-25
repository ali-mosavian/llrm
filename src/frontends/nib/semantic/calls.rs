//! Calls and their arguments (section 9).

use super::*;

/// `size_of[T]()`: the bytes a `T` takes as laid out, a `u16` known when
/// compiled.
pub(super) const SIZE_OF: &str = "size_of";

impl<'a> FunctionCompiler<'a> {
    pub(super) fn size_of(&mut self, types: &[TypeSpec], arguments: &[Expr], span: Span) -> Result<TypedOperand, Diagnostic> {
        let ([spec], []) = (types, arguments) else {
            return Err(Diagnostic::new(span, "size_of takes one type and no values: size_of[T]()"));
        };
        let element = self.types.resolve_element(spec, span)?;
        let bytes = self.types.width(element.id());
        Ok(TypedOperand { operand: Some(hir::Operand::Constant(U16, i64::from(bytes))), type_name: TypeName::U16 })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn call(
        &mut self,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if name == "print" {
            return self.print(arguments, expected, span);
        }
        if let Some(lambda) = self.lambda_named(name) {
            return self.inline_lambda(lambda, arguments, expected, span);
        }
        if self.is_generator_call(name) {
            return Err(Diagnostic::new(
                span,
                format!("generator {name:?} is consumed by a 'for', not called"),
            ));
        }
        if self.types.bits.contains_key(name) {
            let [value] = arguments else {
                return Err(Diagnostic::new(
                    span,
                    format!("{name}(raw) takes one integer"),
                ));
            };
            return self.bits_from(name, value, span);
        }
        let signature = self.signature_of(name, span)?;
        if expected.is_some_and(|one| one != signature.result) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                signature.result,
            ));
        }
        if signature.view.is_some() {
            return Err(Diagnostic::new(span, format!("{name} returns a view: bind it, pass it, or print it")));
        }
        if signature.slot.is_some() {
            // Called for its effect: the result lands in a temporary.
            let result = self.call_into(name, arguments, span)?;
            self.statement_temporary(result);
            return Ok(TypedOperand {
                operand: None,
                type_name: TypeName::Void,
            });
        }
        let result = self.emit_call(&signature, arguments, None, span)?;
        Ok(TypedOperand {
            operand: result.map(hir::Operand::Value),
            type_name: signature.result,
        })
    }

    /// Calls a function whose result is an aggregate, into a fresh temporary.
    pub(super) fn call_into(
        &mut self,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) -> Result<StructView, Diagnostic> {
        let signature = self.signature_of(name, span)?;
        let Some(struct_id) = signature.slot else {
            return Err(Diagnostic::new(
                span,
                format!("{name} does not return a struct or enum"),
            ));
        };
        let view = self.temporary(struct_id);
        self.call_aggregate(&signature, arguments, &view, span)?;
        Ok(view)
    }

    pub(super) fn signature_of(&self, name: &str, span: Span) -> Result<Signature, Diagnostic> {
        self.known_signature(name)
            .ok_or_else(|| Diagnostic::new(span, format!("unknown function {name:?}")))
    }

    /// A module function's signature, or a generic instance's.
    pub(super) fn known_signature(&self, name: &str) -> Option<Signature> {
        self.signatures
            .get(name)
            .cloned()
            .or_else(|| self.templates.borrow().instance(name))
    }

    /// Storage for an intermediate aggregate, uninitialized.
    pub(super) fn temporary(&mut self, struct_id: u32) -> StructView {
        let width = self.types.width(struct_id);
        let name = self.hidden("temporary");
        let place = self.local_place(&name, struct_id, width, true);
        StructView {
            struct_id,
            place,
            pointer: None,
            indices: Vec::new(),
            offset: 0,
            mutable: true,
            owner: name,
        }
    }

    pub(super) fn address_of(&mut self, view: &StructView) -> hir::Operand {
        let pointer_type = self.types.pointer(view.struct_id, 0);
        self.address_as(view, pointer_type)
    }

    /// `view`'s address as a `pointer_type`, near or far.
    pub(super) fn address_as(&mut self, view: &StructView, pointer_type: u32) -> hir::Operand {
        // A pointer to the whole struct of the same width is its address;
        // any other, such as a vec element's near one, is taken again.
        if let (Some(pointer), 0) = (view.pointer, view.offset) {
            let found = self.type_of(pointer);
            if found == pointer_type {
                return hir::Operand::Value(pointer);
            }
            if self.types.width(found) == self.types.width(pointer_type) {
                let result = self.value_type(pointer_type);
                self.emit("copy", vec![result], vec![hir::Operand::Value(pointer)], None);
                return hir::Operand::Value(result);
            }
        }
        let result = self.value_type(pointer_type);
        let place = self.projected_place(view, 0, TypeName::U8);
        let place = match place {
            hir::Operand::ProjectedPlace {
                place,
                indices,
                offset: 0,
                ..
            } if indices.is_empty() => hir::Operand::Place(place),
            other => other,
        };
        self.emit("address", vec![result], vec![place], None);
        hir::Operand::Value(result)
    }

    /// Emits the call; the result's value, when it has one in registers.
    pub(super) fn emit_call(
        &mut self,
        signature: &Signature,
        arguments: &[Expr],
        slot: Option<hir::Operand>,
        span: Span,
    ) -> Result<Option<u32>, Diagnostic> {
        let formals: Vec<_> = signature
            .formals
            .iter()
            .map(|(name, default)| Formal {
                name,
                default: default.as_ref(),
            })
            .collect();
        if self.is_drop_method(&signature.name) {
            return Err(Diagnostic::new(span, "drop runs when its owner ends; it cannot be called"));
        }
        if signature.abi.interrupt() {
            return Err(Diagnostic::new(span, format!("{} is an interrupt16 function: only an interrupt enters it", signature.name)));
        }
        if signature.foreign {
            self.require_unsafe(
                &format!("calling the foreign function {}", signature.name),
                span,
            )?;
        }
        let arguments = &arguments::bind(&signature.name, &formals, arguments.to_vec(), span)?;
        let mut operands: Vec<hir::Operand> = slot.into_iter().collect();
        let mut borrowed = BTreeMap::new();
        for (argument, parameter) in arguments.iter().zip(&signature.parameters) {
            let operand = self.argument_operand(argument, parameter, &mut borrowed)?;
            operands.push(operand);
        }
        operands.extend(signature.result_pointer.map(|pointer| self.result_pointer(pointer)));
        let returned = signature.returned(self.types);
        let results = if returned == TypeName::Void { Vec::new() } else { vec![self.value(returned)] };
        let count = operands.len() as u32;
        let instruction = self.emit(
            "call",
            results.clone(),
            operands,
            Some(signature.name.clone()),
        );
        self.calls.push(hir::CallSite::new(instruction, signature.id, count, signature.abi));
        if let (Some(result), None) = (results.first(), signature.slot) {
            // The caller owns a result (section 9.5).
            self.temporary_owned(hir::Operand::Value(*result), signature.result);
        }
        Ok(results.first().copied())
    }

    /// What a call passes for `parameter`. `borrowed` tracks the call's
    /// borrows so that a mutable one aliases nothing.
    pub(super) fn argument_operand(
        &mut self,
        argument: &Expr,
        parameter: &SignatureParameter,
        borrowed: &mut BTreeMap<String, bool>,
    ) -> Result<hir::Operand, Diagnostic> {
        match parameter {
            // A BASIC procedure takes the near pointer the adapter is.
            SignatureParameter::Adapter { pointer, .. } => {
                let value = self.coerced(argument, *pointer)?;
                required(value, argument.span())
            }
            SignatureParameter::Scalar(type_name) => {
                let value = self.coerced(argument, *type_name)?;
                self.consume(&value, argument.span())?;
                let value = required(value, argument.span())?;
                if !is_float(*type_name) || !matches!(value, hir::Operand::Value(_)) {
                    return Ok(value);
                }
                // A float is evaluated wider than it is stored; the callee
                // receives its stored form.
                let stored = self.place("$argument", *type_name, true);
                self.emit("store", Vec::new(), vec![hir::Operand::Place(stored), value], None);
                Ok(hir::Operand::Place(stored))
            }
            SignatureParameter::Owned { struct_id, .. } => {
                // The callee owns a copy; the caller's value stays its own.
                let copy = self.temporary(*struct_id);
                self.store_struct_expression(&copy, argument)?;
                Ok(self.address_of(&copy))
            }
            SignatureParameter::Borrowed {
                mutable,
                target,
                pointer,
            } => {
                let (operand, owner) =
                    self.borrow_argument(argument, *mutable, *target, *pointer)?;
                if let Some(previously_mutable) = borrowed.insert(owner.clone(), *mutable) {
                    if *mutable || previously_mutable {
                        return Err(Diagnostic::new(
                            argument.span(),
                            format!("borrow of {owner:?} aliases a mutable argument"),
                        ));
                    }
                }
                Ok(operand)
            }
        }
    }

    pub(super) fn borrow_argument(
        &mut self,
        argument: &Expr,
        required_mutable: bool,
        target: BindingType,
        pointer_type: u32,
    ) -> Result<(hir::Operand, String), Diagnostic> {
        let Expr::Borrow {
            mutable,
            operand,
            span,
        } = argument
        else {
            // The parameter says it borrows, so the call site may write the argument alone.
            let borrow = Expr::Borrow {
                mutable: required_mutable,
                operand: Box::new(argument.clone()),
                span: argument.span(),
            };
            return self.borrow_argument(&borrow, required_mutable, target, pointer_type);
        };
        if required_mutable && !mutable {
            return Err(Diagnostic::new(*span, "mutable parameter requires '&mut'"));
        }
        if let (true, Some(owner)) = (*mutable, borrows::expression_owner(operand)) {
            self.check_unborrowed(owner, *span)?;
        }
        if *mutable {
            self.check_mutable_fields(operand)?;
        }
        // A struct is borrowed through its view, whose address is far.
        if let BindingType::Struct(struct_id) = target {
            let view = self.struct_view(operand, *span)?;
            if view.struct_id != struct_id {
                return Err(Diagnostic::new(*span, "borrowed struct has the wrong type"));
            }
            if *mutable && !view.mutable {
                return Err(Diagnostic::new(
                    *span,
                    format!("cannot borrow {:?} mutably", view.owner),
                ));
            }
            let owner = view.owner.clone();
            return Ok((self.address_as(&view, pointer_type), owner));
        }
        let (binding, name, range) = match operand.as_ref() {
            Expr::Name(name, name_span) => (self.binding(name, *name_span)?.clone(), name.clone(), None),
            Expr::Slice {
                base,
                start,
                end,
                span: range_span,
            } => {
                let (binding, name) = self.sequence_of(base)?;
                (binding, name, Some((start.as_deref(), end.as_deref(), *range_span)))
            }
            // An array field, or a call's result.
            other if self.fixed_array_hint(other).is_some() => {
                let (binding, name) = self.sequence_of(operand)?;
                (binding, name, None)
            }
            _ => {
                if let (Some((element, rank)), BindingType::Slice { element: wanted, rank: wanted_rank }) = (self.view_type_of(operand), target) {
                    if (element, rank) != (wanted, wanted_rank) {
                        return Err(Diagnostic::new(operand.span(), "the view has the wrong element type or rank"));
                    }
                    let (descriptor, ..) = self.view_of(operand)?.expect("a view");
                    return Ok((hir::Operand::Value(descriptor), format!("$view{descriptor}")));
                }
                if let (BindingType::Slice { element, rank: 1 }, false) = (target, *mutable) {
                    return self.value_view(operand, element, pointer_type);
                }
                // `&s.field` or `&items[i]`: a place's address.
                if let BindingType::Scalar(type_name) = target {
                    if let Some((place, actual, owner)) = self.place_of(operand, *mutable, *span)? {
                        if actual != type_name {
                            return Err(Diagnostic::new(*span, format!("borrow of {owner:?}'s {} has the wrong type", type_name_text(actual))));
                        }
                        let pointer = self.value_type(pointer_type);
                        self.emit("address", vec![pointer], vec![place], None);
                        return Ok((hir::Operand::Value(pointer), owner));
                    }
                }
                return Err(Diagnostic::new(
                    operand.span(),
                    "only a place or a sequence can be borrowed",
                ));
            }
        };
        let compatible = binding.type_ == target
            || matches!(
                (binding.type_, target),
                (
                    BindingType::Array { element: actual, shape },
                    BindingType::Slice { element: expected, rank }
                ) if actual == expected && shape.rank == rank
            )
            || matches!(target, BindingType::Slice { element, rank: 1 } if self.heap_sequence(&binding) == Some(element));
        if !compatible {
            return Err(Diagnostic::new(
                *span,
                format!("borrow of {name:?} has the wrong type"),
            ));
        }
        if *mutable && !binding.mutable {
            return Err(Diagnostic::new(
                *span,
                format!("cannot mutably borrow immutable binding {name:?}"),
            ));
        }
        if let BindingType::Slice { element, rank } = target {
            if let Storage::Slice(descriptor) = binding.storage {
                if range.is_none() {
                    return Ok((hir::Operand::Value(descriptor), name.clone()));
                }
                // A range of a view is a view of its own.
                if rank == 1 {
                    let (data, length) = self.view_parts_of(descriptor, element);
                    let view = self.ranged_view(
                        &name,
                        data,
                        length,
                        element,
                        range,
                        pointer_type,
                        operand.span(),
                    )?;
                    return Ok((view, name.clone()));
                }
            }
            if self.heap_sequence(&binding).is_some() {
                let view = self.sequence_view(
                    &binding,
                    &name,
                    element,
                    range,
                    pointer_type,
                    operand.span(),
                )?;
                return Ok((view, name.clone()));
            }
            let BindingType::Array {
                element: actual,
                shape,
            } = binding.type_
            else {
                return Err(Diagnostic::new(
                    operand.span(),
                    "a ranged borrow currently requires a fixed array",
                ));
            };
            debug_assert_eq!(actual, element);
            if range.is_some() && rank != 1 {
                return Err(Diagnostic::new(
                    operand.span(),
                    "only a one-dimensional array can be sliced",
                ));
            }
            let data = self.array_data(&binding, element, operand.span())?;
            if rank == 1 {
                let length = hir::Operand::Constant(U16, i64::from(shape.len()));
                let view = self.ranged_view(&name, data, length, element, range, pointer_type, operand.span())?;
                return Ok((view, name.clone()));
            }
            // A ranked view describes the whole array.
            let words = shape
                .descriptor()
                .into_iter()
                .map(|(_, value)| hir::Operand::Constant(U16, i64::from(value)))
                .collect();
            let view = self.view_descriptor(&name, pointer_type, words, data);
            return Ok((view, name.clone()));
        }
        if range.is_some() {
            return Err(Diagnostic::new(
                operand.span(),
                "a range can only be borrowed as a slice",
            ));
        }
        let place = match binding.storage {
            Storage::Place(place) => hir::Operand::Place(place),
            Storage::ArrayView { place, index } => hir::Operand::ArrayElement(place, vec![index]),
            Storage::Reference(pointer) => return Ok((hir::Operand::Value(pointer), name.clone())),
            Storage::Slice(_) => unreachable!("slice target handled above"),
            Storage::Lambda(_) => {
                unreachable!("borrow target is not a dictionary")
            }
            Storage::Parameter(_) => {
                return Err(Diagnostic::new(
                    *span,
                    "a by-value parameter has no borrowable storage",
                ));
            }
        };
        let result = self.value_type(pointer_type);
        self.emit("address", vec![result], vec![place], None);
        Ok((hir::Operand::Value(result), name.clone()))
    }

    /// A `&[T]` of `words` -- its dimensions, then capacity -- and far `data`,
    /// in a descriptor on this frame.
    pub(super) fn view_descriptor(
        &mut self,
        name: &str,
        pointer_type: u32,
        words: Vec<hir::Operand>,
        data: u32,
    ) -> hir::Operand {
        let descriptor_type = self
            .types
            .types
            .iter()
            .find(|one| one.id == pointer_type)
            .and_then(|one| one.element)
            .expect("slice pointer has a descriptor pointee");
        let size = 2 * words.len() as u32;
        let descriptor =
            self.local_place(&format!("$slice_{name}"), descriptor_type, size + 4, false);
        let data_type = self.type_of(data);
        let stores = words
            .into_iter()
            .enumerate()
            .map(|(word, value)| (2 * word as u32, U16, value));
        for (offset, type_id, value) in stores.chain([(size, data_type, hir::Operand::Value(data))])
        {
            let place = hir::Operand::ProjectedPlace {
                place: descriptor,
                indices: Vec::new(),
                offset,
                type_id,
            };
            self.emit("store", Vec::new(), vec![place, value], None);
        }
        let result = self.value_type(pointer_type);
        self.emit(
            "address",
            vec![result],
            vec![hir::Operand::Place(descriptor)],
            None,
        );
        hir::Operand::Value(result)
    }
}
