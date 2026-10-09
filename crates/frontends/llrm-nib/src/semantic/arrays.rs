//! Array properties and methods.

use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn array_method(
        &mut self,
        receiver: &Expr,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let (binding, array_name) = self.sequence_of(receiver)?;
        if let BindingType::Scalar(dictionary @ TypeName::Dictionary { .. }) = binding.type_ {
            return self.dictionary_method(receiver, dictionary, name, arguments, expected, span);
        }
        let string = binding.type_ == BindingType::Scalar(TypeName::String);
        let heap = self.heap_sequence(&binding).is_some();
        let (rank, shape) = if heap {
            (1, None)
        } else {
            let Some((_element, rank, shape)) = binding.type_.ranked() else {
                return Err(Diagnostic::new(receiver.span(), format!("{array_name:?} is not an array or string")));
            };
            (rank, shape)
        };
        if name == "data" {
            if !arguments.is_empty() {
                return Err(Diagnostic::new(span, "data() takes no arguments"));
            }
            if string {
                if expected.is_some_and(|one| one != TypeName::String) {
                    return Err(type_mismatch(span, expected.expect("checked"), TypeName::String));
                }
                let pointer = self.string_pointer(&binding, receiver.span())?;
                return Ok(TypedOperand { operand: Some(hir::Operand::Value(pointer)), type_name: TypeName::String });
            }
            if expected.is_some_and(|one| one != TypeName::Addr) {
                return Err(type_mismatch(span, expected.expect("checked"), TypeName::Addr));
            }
            let pointer = match binding.storage {
                Storage::Place(place) => {
                    let result = self.value(TypeName::Addr);
                    self.emit("address", vec![result], vec![hir::Operand::Place(place)], None);
                    result
                }
                Storage::Reference(pointer) => {
                    let result = self.value(TypeName::Addr);
                    self.emit("copy", vec![result], vec![hir::Operand::Value(pointer)], None);
                    result
                }
                Storage::Slice(descriptor) => {
                    let BindingType::Slice { element, rank } = binding.type_ else {
                        unreachable!("slice storage has slice type")
                    };
                    let typed = self.slice_data_pointer(descriptor, element, rank);
                    let result = self.value(TypeName::Addr);
                    self.emit("copy", vec![result], vec![hir::Operand::Value(typed)], None);
                    result
                }
                _ => return Err(Diagnostic::new(span, "sequence has no data pointer")),
            };
            return Ok(TypedOperand { operand: Some(hir::Operand::Value(pointer)), type_name: TypeName::Addr });
        }
        if expected.is_some_and(|one| one != self.word()) {
            return Err(type_mismatch(span, expected.expect("checked"), self.word()));
        }
        // A dimension, or past them all the capacity. A ranked array's length
        // is its element count, which is its capacity.
        let word = match name {
            "len" if arguments.is_empty() => {
                if rank == 1 {
                    0
                } else {
                    rank
                }
            }
            "capacity" if arguments.is_empty() => rank,
            "dim" if arguments.len() == 1 => {
                let Expr::Integer(axis, axis_span) = arguments[0] else {
                    return Err(Diagnostic::new(arguments[0].span(), "dimension index must be an integer literal"));
                };
                if !(0..i64::from(rank)).contains(&axis) {
                    return Err(Diagnostic::new(axis_span, format!("{array_name:?} has dimensions 0..{rank}")));
                }
                axis as u8
            }
            "len" | "capacity" => {
                return Err(Diagnostic::new(span, format!("{name}() takes no arguments")));
            }
            "dim" => return Err(Diagnostic::new(span, "dim() takes one dimension index")),
            _ => {
                return Err(Diagnostic::new(span, format!("array has no method {name:?}")));
            }
        };
        let operand = if let Some(shape) = shape {
            let value = if word < rank { shape.dims[word as usize] } else { shape.len() };
            hir::Operand::Constant(self.word_id(), i64::from(value))
        } else if rank > 1 {
            let Storage::Slice(pointer) = binding.storage else {
                return Err(Diagnostic::new(receiver.span(), "view has no descriptor"));
            };
            let value = self.value(self.word());
            self.emit(
                "load",
                vec![value],
                vec![hir::Operand::IndirectPlace {
                    base: pointer,
                    offset: descriptor::dim(word, self.word_bytes()),
                    type_id: self.word_id(),
                    inbounds: false,
                    member: None,
                }],
                None,
            );
            hir::Operand::Value(value)
        } else {
            let pointer = if heap {
                self.string_pointer(&binding, receiver.span())?
            } else if let Storage::Slice(pointer) = binding.storage {
                pointer
            } else {
                return Err(Diagnostic::new(receiver.span(), "slice has no descriptor"));
            };
            let value = self.value(self.word());
            self.emit(
                "load",
                vec![value],
                vec![hir::Operand::DescriptorPlace {
                    base: pointer,
                    field: if word == 0 { "length" } else { "capacity" },
                    type_id: self.word_id(),
                }],
                None,
            );
            hir::Operand::Value(value)
        };
        Ok(TypedOperand { operand: Some(operand), type_name: self.word() })
    }
}
