//! Indexing: element places of arrays, strings, vecs and views.

use super::*;

impl<'a> FunctionCompiler<'a> {
    /// Where `name[indices]` lives: an element of an owned array, or an
    /// address reached through a reference or view.
    pub(super) fn element_at(
        &mut self,
        binding: &Binding,
        name: &str,
        indices: &[Expr],
        span: Span,
    ) -> Result<(ElementType, ElementAt), Diagnostic> {
        if let Some(element) = self.heap_sequence(binding) {
            let pointer = self.string_pointer(binding, span)?;
            return Ok((
                element,
                self.sequence_element_at(pointer, element, indices, span)?,
            ));
        }
        let Some((element, rank, shape)) = binding.type_.ranked() else {
            return Err(Diagnostic::new(
                span,
                format!("binding {name:?} is not an array"),
            ));
        };
        if indices.len() != usize::from(rank) {
            return Err(Diagnostic::new(
                span,
                format!(
                    "{name:?} has {rank} dimension{}, indexed with {}",
                    if rank == 1 { "" } else { "s" },
                    indices.len()
                ),
            ));
        }
        let indices = indices
            .iter()
            .enumerate()
            .map(|(axis, index)| self.array_index(index, shape.map(|one| one.dims[axis])))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(shape) = shape {
            for (index, dim) in indices.iter().zip(shape.dims()) {
                self.check_bounds(index, hir::Operand::Constant(U16, i64::from(*dim)), span)?;
            }
        }
        let element_width = self.types.width(element.id());
        let at = match binding.storage {
            Storage::Place(place) => ElementAt::Element(place, indices),
            Storage::Reference(pointer) => {
                let Some(shape) = shape else {
                    return Err(Diagnostic::new(
                        span,
                        "a reference to an array needs its shape",
                    ));
                };
                let strides = shape
                    .strides()
                    .into_iter()
                    .map(|one| hir::Operand::Constant(U16, i64::from(one)))
                    .collect();
                let flat = self.linear(indices, strides, span)?;
                ElementAt::Pointer(self.indexed_pointer(pointer, flat, element_width, span)?)
            }
            Storage::Slice(descriptor) => {
                self.check_view_bounds(descriptor, &indices, span)?;
                ElementAt::Pointer(self.view_element(descriptor, element, rank, indices, span)?)
            }
            Storage::Parameter(_) | Storage::ArrayView { .. } => {
                return Err(Diagnostic::new(span, "array has no indexable storage"));
            }
            Storage::Lambda(_) => {
                unreachable!("array is not a dictionary")
            }
        };
        Ok((element, at))
    }

    /// The address of a view's element: row-major, so the last stride is one
    /// and each other is the product of the dimensions after it.
    pub(super) fn view_element(
        &mut self,
        descriptor: u32,
        element: ElementType,
        rank: u8,
        indices: Vec<hir::Operand>,
        span: Span,
    ) -> Result<u32, Diagnostic> {
        let mut strides = vec![hir::Operand::Constant(U16, 1)];
        for axis in (1..rank).rev() {
            let dim = self.value(TypeName::U16);
            self.emit(
                "load",
                vec![dim],
                vec![hir::Operand::IndirectPlace {
                    base: descriptor,
                    offset: descriptor::dim(axis),
                    type_id: U16,
                    inbounds: false,
                }],
                None,
            );
            let inner = TypedOperand {
                operand: Some(strides[0].clone()),
                type_name: TypeName::U16,
            };
            let dim = TypedOperand {
                operand: Some(hir::Operand::Value(dim)),
                type_name: TypeName::U16,
            };
            strides.insert(0, self.folded("mul", dim, inner, TypeName::U16));
        }
        let flat = self.linear(indices, strides, span)?;
        let data = self.slice_data_pointer(descriptor, element, rank);
        self.indexed_pointer(data, flat, self.types.width(element.id()), span)
    }

    /// `sum(indices[k] * strides[k])` as a u16: a descriptor's u16 counts bound it.
    pub(super) fn linear(
        &mut self,
        indices: Vec<hir::Operand>,
        strides: Vec<hir::Operand>,
        span: Span,
    ) -> Result<hir::Operand, Diagnostic> {
        if let ([index], [hir::Operand::Constant(_, 1)]) = (indices.as_slice(), strides.as_slice())
        {
            return Ok(index.clone());
        }
        let typed = |this: &Self, operand: &hir::Operand| match operand {
            hir::Operand::Constant(type_id, _) => *type_id,
            hir::Operand::Value(value) => this
                .values
                .iter()
                .find(|one| one.id == *value)
                .map(|one| one.type_id)
                .expect("an index value has a type"),
            _ => U16,
        };
        let index_name = TypeName::U16;
        let mut total: Option<hir::Operand> = None;
        for (index, stride) in indices.into_iter().zip(strides) {
            let index = TypedOperand {
                type_name: type_name_of(typed(self, &index)),
                operand: Some(index),
            };
            let stride = TypedOperand {
                type_name: type_name_of(typed(self, &stride)),
                operand: Some(stride),
            };
            let index = self.converted(index, index_name, span)?;
            let stride = self.converted(stride, index_name, span)?;
            let term = self.folded("mul", index, stride, index_name);
            total = Some(match total {
                None => term,
                Some(sum) => self.folded(
                    "add",
                    TypedOperand {
                        operand: Some(sum),
                        type_name: index_name,
                    },
                    TypedOperand {
                        operand: Some(term),
                        type_name: index_name,
                    },
                    index_name,
                ),
            });
        }
        Ok(total.expect("at least one index"))
    }

    /// `left op right`, folded when both are constants.
    pub(super) fn folded(
        &mut self,
        op: &'static str,
        left: TypedOperand,
        right: TypedOperand,
        type_name: TypeName,
    ) -> hir::Operand {
        let (left, right) = (
            left.operand.expect("an operand"),
            right.operand.expect("an operand"),
        );
        if let (hir::Operand::Constant(_, a), hir::Operand::Constant(_, b)) = (&left, &right) {
            let value = if op == "mul" { a * b } else { a + b };
            return hir::Operand::Constant(type_id(type_name), wrapped(value, type_name));
        }
        let result = self.value(type_name);
        self.emit(op, vec![result], vec![left, right], None);
        hir::Operand::Value(result)
    }

    pub(super) fn index_expression(
        &mut self,
        base: &Expr,
        indices: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if let Some(value) = self.dictionary_value(base, indices, span)? {
            return self.expression(&value, expected);
        }
        let (binding, name) = self.sequence_of(base)?;
        let (element, at) = self.element_at(&binding, &name, indices, span)?;
        let ElementType::Scalar(element) = element else {
            return Err(Diagnostic::new(
                span,
                "a struct array element must be used through one of its fields",
            ));
        };
        if expected.is_some_and(|one| one != element) {
            return Err(type_mismatch(span, expected.expect("checked"), element));
        }
        let result = self.value(element);
        self.emit(
            "load",
            vec![result],
            vec![at.operand(type_id(element))],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: element,
        })
    }

    /// The heap sequence's element, when `binding` holds a string or vec.
    pub(super) fn heap_sequence(&self, binding: &Binding) -> Option<ElementType> {
        match binding.type_ {
            BindingType::Scalar(type_name) => self.types.sequence_element(type_name),
            _ => None,
        }
    }

    /// A string's or vec's pointer, read from where `binding` keeps it.
    pub(super) fn string_pointer(&mut self, binding: &Binding, span: Span) -> Result<u32, Diagnostic> {
        let BindingType::Scalar(type_name) = binding.type_ else {
            return Err(Diagnostic::new(span, "not a heap sequence"));
        };
        match binding.storage {
            Storage::Parameter(value) => Ok(value),
            Storage::Reference(pointer) => {
                let value = self.value(type_name);
                self.emit(
                    "load",
                    vec![value],
                    vec![hir::Operand::IndirectPlace {
                        base: pointer,
                        offset: 0,
                        type_id: type_id(type_name),
                        inbounds: false,
                    }],
                    None,
                );
                Ok(value)
            }
            Storage::Place(place) => {
                let value = self.value(type_name);
                self.emit("load", vec![value], vec![hir::Operand::Place(place)], None);
                Ok(value)
            }
            _ => Err(Diagnostic::new(span, "string has no scalar pointer value")),
        }
    }

    pub(super) fn slice_data_pointer(&mut self, descriptor: u32, element: ElementType, rank: u8) -> u32 {
        let pointer_type = self.types.pointer(element.id(), 0);
        let pointer = self.value_type(pointer_type);
        self.emit(
            "load",
            vec![pointer],
            vec![hir::Operand::IndirectPlace {
                base: descriptor,
                offset: descriptor::size(rank),
                type_id: pointer_type,
                inbounds: false,
            }],
            None,
        );
        pointer
    }

    pub(super) fn indexed_pointer(
        &mut self,
        pointer: u32,
        index: hir::Operand,
        element_width: u32,
        span: Span,
    ) -> Result<u32, Diagnostic> {
        let byte_offset = match index {
            hir::Operand::Constant(_, value) => {
                hir::Operand::Constant(U16, value * i64::from(element_width))
            }
            hir::Operand::Value(value) if element_width == 1 => hir::Operand::Value(value),
            hir::Operand::Value(value) => {
                let index_type = self
                    .values
                    .iter()
                    .find(|one| one.id == value)
                    .map(|one| one.type_id)
                    .ok_or_else(|| Diagnostic::new(span, "array index has no type"))?;
                let scaled = self.value_type(index_type);
                self.emit(
                    "mul",
                    vec![scaled],
                    vec![
                        hir::Operand::Value(value),
                        hir::Operand::Constant(index_type, i64::from(element_width)),
                    ],
                    None,
                );
                hir::Operand::Value(scaled)
            }
            _ => return Err(Diagnostic::new(span, "invalid array index operand")),
        };
        let pointer_type = self
            .values
            .iter()
            .find(|one| one.id == pointer)
            .map(|one| one.type_id)
            .ok_or_else(|| Diagnostic::new(span, "array reference has no type"))?;
        let address = self.value_type(pointer_type);
        self.emit(
            "ptr_offset",
            vec![address],
            vec![hir::Operand::Value(pointer), byte_offset],
            None,
        );
        Ok(address)
    }
}
