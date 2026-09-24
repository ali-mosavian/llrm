//! References as values (section 6): `&T` inside a tuple, an enum payload or
//! a generator's item is a far pointer, the one a borrowed parameter takes.
//! A name bound to one reads and writes through it.

use super::*;

impl TypeRegistry {
    /// `&T` or `&mut T`.
    pub(super) fn reference(&mut self, target: ElementType, mutable: bool) -> TypeName {
        let type_id = self.pointer(target.id(), 0);
        self.referents.insert(type_id, target);
        TypeName::Pointer { type_id, width: 4, mutable }
    }

    /// What a reference type refers to; `None` for any other type.
    pub(super) fn referent(&self, type_name: TypeName) -> Option<ElementType> {
        let TypeName::Pointer { type_id, .. } = type_name else {
            return None;
        };
        self.referents.get(&type_id).copied()
    }
}

impl FunctionCompiler<'_> {
    /// A name for the reference `operand`: its referent, read through it.
    pub(super) fn reference_binding(&mut self, operand: hir::Operand, type_name: TypeName) -> Option<Binding> {
        let target = self.types.referent(type_name)?;
        let TypeName::Pointer { mutable, .. } = type_name else {
            unreachable!("a reference is a pointer")
        };
        let pointer = self.materialized(operand, type_id(type_name));
        Some(Binding {
            type_: match target {
                ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                ElementType::Struct(id) => BindingType::Struct(id),
            },
            mutable,
            storage: Storage::Reference(pointer),
        })
    }

    /// `expression` as the reference `reference`: a place, borrowed, or a
    /// name already bound to a reference.
    pub(super) fn reference_to(&mut self, expression: &Expr, reference: TypeName, span: Span) -> Result<TypedOperand, Diagnostic> {
        let target = self.types.referent(reference).expect("a reference type");
        let TypeName::Pointer { type_id: pointer_type, mutable, .. } = reference else {
            unreachable!("a reference is a pointer")
        };
        let target_type = match target {
            ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
            ElementType::Struct(id) => BindingType::Struct(id),
        };
        let operand = match expression {
            Expr::Name(name, name_span) => match self.binding(name, *name_span)?.clone() {
                Binding { storage: Storage::Reference(pointer), type_, mutable: writes }
                    if type_ == target_type && (writes || !mutable) =>
                {
                    hir::Operand::Value(pointer)
                }
                // A place where a reference goes is lent, as an argument is.
                _ => self.borrow_argument(expression, mutable, target_type, pointer_type)?.0,
            },
            Expr::Borrow { .. } | Expr::Member { .. } | Expr::Index { .. } => {
                self.borrow_argument(expression, mutable, target_type, pointer_type)?.0
            }
            _ => return Err(Diagnostic::new(span, "a reference is taken with '&'")),
        };
        Ok(TypedOperand { operand: Some(operand), type_name: reference })
    }

    /// The scalar place `expression` names -- a field or an element -- with
    /// its type and owner; `None` when it names none.
    pub(super) fn place_of(&mut self, expression: &Expr, exclusive: bool, span: Span) -> Result<Option<(hir::Operand, TypeName, String)>, Diagnostic> {
        let (place, type_name, writable, owner) = match expression {
            Expr::Member { base, field, .. } if self.bits_type(base).is_none() => self.member_place(base, field, span)?,
            Expr::Index { base, indices, .. } => {
                let (binding, owner) = self.sequence_of(base)?;
                let (element, at) = self.element_at(&binding, &owner, indices, span)?;
                let ElementType::Scalar(type_name) = element else {
                    return Ok(None);
                };
                (at.operand(type_id(type_name)), type_name, binding.mutable, owner)
            }
            _ => return Ok(None),
        };
        if exclusive && !writable {
            return Err(Diagnostic::new(span, format!("cannot mutably borrow immutable binding {owner:?}")));
        }
        Ok(Some((place, type_name, owner)))
    }

    /// `let name = &operand`: a view of a sequence, otherwise a reference.
    /// `&mut` of a whole vec or string is a reference, so it can grow.
    pub(super) fn borrowed_binding(&mut self, borrow: &Expr) -> Result<Binding, Diagnostic> {
        let Expr::Borrow { mutable, operand, span } = borrow else {
            unreachable!("a borrow")
        };
        if let Some((element, rank)) = self.borrowed_view_type(operand, *mutable) {
            let pointer_type = self.types.slice_pointer(element, rank);
            let type_ = BindingType::Slice { element, rank };
            let (hir::Operand::Value(descriptor), _) = self.borrow_argument(borrow, *mutable, type_, pointer_type)? else {
                unreachable!("a view is a descriptor pointer")
            };
            return Ok(Binding { type_, mutable: *mutable, storage: Storage::Slice(descriptor) });
        }
        let target = match self.struct_type_hint(operand, *span) {
            Some(id) => ElementType::Struct(id),
            None => ElementType::Scalar(
                self.expression_type_hint(operand)
                    .ok_or_else(|| Diagnostic::new(*span, "only a place can be borrowed"))?,
            ),
        };
        let reference = self.types.reference(target, *mutable);
        let pointer = self.reference_to(borrow, reference, *span)?.operand.expect("a reference");
        Ok(self.reference_binding(pointer, reference).expect("a reference type"))
    }

    /// The view `&operand` makes, when `operand` is a sequence.
    fn borrowed_view_type(&self, operand: &Expr, exclusive: bool) -> Option<(ElementType, u8)> {
        match operand {
            Expr::Slice { base, .. } => self.indexed_hint(base).map(|element| (element, 1)),
            Expr::Name(name, span) => match self.binding(name, *span).ok()?.type_ {
                BindingType::Array { element, shape } => Some((element, shape.rank)),
                BindingType::Slice { element, rank } => Some((element, rank)),
                BindingType::Scalar(type_name) if !exclusive => self.types.sequence_element(type_name).map(|element| (element, 1)),
                _ => None,
            },
            _ if exclusive => None,
            _ => self
                .view_type_of(operand)
                .or_else(|| self.types.sequence_element(self.expression_type_hint(operand)?).map(|element| (element, 1))),
        }
    }
}
