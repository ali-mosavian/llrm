//! Views as values (sections 9 and 13): a function returning `&[T]` or
//! `&string` writes the descriptor to the caller's slot. A returned view
//! may borrow only what the caller lent -- a parameter -- never a local.

use super::*;

impl Signature {
    /// The hidden first parameter's type: a far pointer to the slot.
    pub(super) fn slot_pointer(&self, types: &mut TypeRegistry) -> Option<u32> {
        if self.in_registers(types).is_some() {
            return None;
        }
        match (self.slot, self.view) {
            (Some(struct_id), _) => Some(types.pointer(struct_id, 0)),
            (None, Some((element, rank))) => Some(types.slice_pointer(element, rank)),
            (None, None) => None,
        }
    }
}

impl FunctionCompiler<'_> {
    /// The call `expression` is, when it returns a view.
    fn view_call(&self, expression: &Expr) -> Option<(Vec<Expr>, Signature)> {
        let call = self.method_as_call(expression).unwrap_or_else(|| expression.clone());
        let Expr::Call { name, arguments, .. } = call else {
            return None;
        };
        let signature = self.known_signature(&name).filter(|one| one.view.is_some())?;
        Some((arguments, signature))
    }

    /// The element and rank of the view `expression` names or a call returns.
    pub(super) fn view_type_of(&self, expression: &Expr) -> Option<(ElementType, u8)> {
        match expression {
            Expr::Name(name, _) => match self.visible(name)? {
                Binding { type_: BindingType::Slice { element, rank }, storage: Storage::Slice(_), .. } => Some((*element, *rank)),
                _ => None,
            },
            _ if self.string_bytes(expression).is_some() => Some((ElementType::Scalar(TypeName::U8), 1)),
            _ => self.view_call(expression).and_then(|(_, signature)| signature.view),
        }
    }

    /// The string `s.bytes()` views as `&[u8]`.
    fn string_bytes<'e>(&self, expression: &'e Expr) -> Option<&'e Expr> {
        let Expr::MethodCall { receiver, name, arguments, .. } = expression else {
            return None;
        };
        let text = self.expression_type_hint(receiver) == Some(TypeName::String) || self.is_char_view(receiver);
        (name == "bytes" && arguments.is_empty() && text).then_some(receiver.as_ref())
    }

    pub(super) fn is_char_view(&self, expression: &Expr) -> bool {
        self.view_type_of(expression) == Some((ElementType::Scalar(TypeName::Char), 1))
    }

    /// The view `expression` names or a call returns: its descriptor
    /// pointer, element and rank. A call's lands in this frame.
    pub(super) fn view_of(&mut self, expression: &Expr) -> Result<Option<(u32, ElementType, u8)>, Diagnostic> {
        if let Expr::Name(name, span) = expression {
            let binding = self.binding(name, *span)?;
            return Ok(match binding {
                Binding { type_: BindingType::Slice { element, rank }, storage: Storage::Slice(descriptor), .. } => {
                    Some((*descriptor, *element, *rank))
                }
                _ => None,
            });
        }
        if let Some(text) = self.string_bytes(expression) {
            let (binding, name) = self.sequence_of(text)?;
            let element = ElementType::Scalar(TypeName::U8);
            // A char's byte is a u8: a view of chars is one of bytes.
            if let Storage::Slice(descriptor) = binding.storage {
                return Ok(Some((descriptor, element, 1)));
            }
            let pointer_type = self.types.slice_pointer(element, 1);
            let hir::Operand::Value(descriptor) = self.sequence_view(&binding, &name, element, None, pointer_type, expression.span())? else {
                unreachable!("a view is a descriptor pointer")
            };
            return Ok(Some((descriptor, element, 1)));
        }
        let Some((arguments, signature)) = self.view_call(expression) else {
            return Ok(None);
        };
        let (element, rank) = signature.view.expect("a view result");
        let pointer = self.view_slot(element, rank);
        self.emit_call(&signature, &arguments, Some(hir::Operand::Value(pointer)), expression.span())?;
        Ok(Some((pointer, element, rank)))
    }

    /// An uninitialized view descriptor in this frame: its far pointer.
    pub(super) fn view_slot(&mut self, element: ElementType, rank: u8) -> u32 {
        let descriptor_type = self.types.slice_descriptor(element, rank);
        let place = self.local_place(&format!("$view{}", self.next_place), descriptor_type, descriptor::size(rank) + 4, true);
        let pointer_type = self.types.slice_pointer(element, rank);
        let pointer = self.value_type(pointer_type);
        self.emit("address", vec![pointer], vec![hir::Operand::Place(place)], None);
        pointer
    }

    /// Copies the view descriptor `source` points to over `target`'s.
    pub(super) fn copy_view(&mut self, source: u32, target: u32, element: ElementType, rank: u8) {
        let words = (0..descriptor::size(rank)).step_by(2).map(|offset| (offset, U16));
        let data = (descriptor::size(rank), self.types.pointer(element.id(), 0));
        for (offset, type_id) in words.chain([data]) {
            let value = self.value_type(type_id);
            self.emit("load", vec![value], vec![hir::Operand::IndirectPlace { base: source, offset, type_id, inbounds: false }], None);
            let destination = hir::Operand::IndirectPlace { base: target, offset, type_id, inbounds: false };
            self.emit("store", Vec::new(), vec![destination, hir::Operand::Value(value)], None);
        }
    }

    /// What indexing `base` gives: an element of a named array, a view, or a
    /// vec or string any expression reads.
    pub(super) fn indexed_hint(&self, base: &Expr) -> Option<ElementType> {
        match base {
            Expr::Name(name, span) => self.indexed_element(self.binding(name, *span).ok()?),
            _ => self
                .view_type_of(base)
                .map(|(element, _)| element)
                .or_else(|| self.types.indexed(self.expression_type_hint(base)?)),
        }
    }

    /// The sequence `expression` is, as a binding: a named one, a view a call
    /// returns, or a vec or string any other expression reads -- borrowed for
    /// the statement, never moved.
    pub(super) fn sequence_of(&mut self, expression: &Expr) -> Result<(Binding, String), Diagnostic> {
        if let Expr::Name(name, span) = expression {
            return Ok((self.binding(name, *span)?.clone(), name.clone()));
        }
        if let Some((descriptor, element, rank)) = self.view_of(expression)? {
            let binding = Binding { type_: BindingType::Slice { element, rank }, mutable: false, storage: Storage::Slice(descriptor) };
            return Ok((binding, format!("$view{descriptor}")));
        }
        let span = expression.span();
        let value = self.expression(expression, None)?;
        let type_name = value.type_name;
        if self.types.sequence_element(type_name).is_none() {
            return Err(Diagnostic::new(span, format!("a {} is not a sequence", type_name_text(type_name))));
        }
        let value = self.materialized(required(value, span)?, type_id(type_name));
        let binding = Binding { type_: BindingType::Scalar(type_name), mutable: false, storage: Storage::Parameter(value) };
        Ok((binding, format!("$value{value}")))
    }

    /// `return view`: its descriptor, copied to the caller's slot.
    pub(super) fn return_view(&mut self, expression: &Expr, span: Span) -> Result<(), Diagnostic> {
        let (element, rank) = self.signature.view.expect("a view result");
        let source = match self.view_of(expression)? {
            Some((descriptor, ..)) => descriptor,
            None => {
                let pointer = self.types.slice_pointer(element, rank);
                let target = BindingType::Slice { element, rank };
                let (hir::Operand::Value(descriptor), _) = self.borrow_argument(expression, false, target, pointer)? else {
                    unreachable!("a view is a value")
                };
                descriptor
            }
        };
        let Storage::Slice(slot) = self.binding(RESULT, span)?.storage else {
            unreachable!("a view result's slot")
        };
        self.copy_view(source, slot, element, rank);
        Ok(())
    }
}
