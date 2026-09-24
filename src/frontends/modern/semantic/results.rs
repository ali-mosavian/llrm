//! Where an aggregate result goes (section 9.2): one of 4 bytes or less comes
//! back in `al`, `ax` or `dx:ax` as the integer its bytes spell; a larger one
//! is written to the caller's slot.

use super::*;

impl TypeRegistry {
    /// Whether `element`'s bytes hold an address. HIR addresses are not
    /// integers, so such an aggregate cannot travel as one.
    fn holds_address(&self, element: ElementType) -> bool {
        match element {
            ElementType::Scalar(type_name) => matches!(
                type_name,
                TypeName::String | TypeName::Addr | TypeName::Vector { .. } | TypeName::Pointer { .. }
            ),
            ElementType::Struct(id) => {
                let fields = self.structure(id).into_iter().flat_map(|layout| layout.fields.values());
                let payloads = self
                    .enum_of(element)
                    .into_iter()
                    .flat_map(|layout| &layout.variants)
                    .flat_map(|variant| variant.fields.iter().map(|(_, field)| field));
                fields.chain(payloads).any(|field| self.holds_address(field.type_))
            }
        }
    }
}

impl Signature {
    /// The integer an aggregate result travels as, when it fits registers.
    pub(super) fn in_registers(&self, types: &TypeRegistry) -> Option<TypeName> {
        let struct_id = self.slot?;
        if types.holds_address(ElementType::Struct(struct_id)) {
            return None;
        }
        match types.width(struct_id) {
            1 => Some(TypeName::U8),
            2 => Some(TypeName::U16),
            3 | 4 => Some(TypeName::U32),
            _ => None,
        }
    }

    /// The result the object code returns.
    pub(super) fn returned(&self, types: &TypeRegistry) -> TypeName {
        self.in_registers(types).unwrap_or(self.result)
    }
}

/// The pieces a register image of `width` bytes is read and written in.
fn pieces(width: u32) -> &'static [(u32, TypeName)] {
    match width {
        1 => &[(0, TypeName::U8)],
        2 => &[(0, TypeName::U16)],
        3 => &[(0, TypeName::U16), (2, TypeName::U8)],
        _ => &[(0, TypeName::U32)],
    }
}

impl FunctionCompiler<'_> {
    /// `view`'s bytes as the integer `image`.
    fn register_image(&mut self, view: &StructView, image: TypeName) -> hir::Operand {
        let mut combined: Option<hir::Operand> = None;
        for &(offset, piece) in pieces(self.types.width(view.struct_id)) {
            let value = self.value(piece);
            let place = self.projected_place(view, offset, piece);
            self.emit("load", vec![value], vec![place], None);
            let widened = self.resized(hir::Operand::Value(value), piece, image);
            let shifted = self.bit_shift("shl", widened, 8 * offset, image);
            combined = Some(match combined {
                Some(low) => self.binary_op("or", low, shifted, image),
                None => shifted,
            });
        }
        combined.expect("an aggregate has bytes")
    }

    /// Stores the integer `image` into `view`'s bytes.
    fn store_image(&mut self, view: &StructView, image: hir::Operand, type_name: TypeName) {
        for &(offset, piece) in pieces(self.types.width(view.struct_id)) {
            let shifted = self.bit_shift("shr", image.clone(), 8 * offset, type_name);
            let value = self.resized(shifted, type_name, piece);
            let place = self.projected_place(view, offset, piece);
            self.emit("store", Vec::new(), vec![place, value], None);
        }
    }

    /// Returns the aggregate `$result` holds: in registers, or already in the slot.
    pub(super) fn return_aggregate(&mut self, span: Span) -> Result<(), Diagnostic> {
        let operands = match self.signature.in_registers(self.types) {
            Some(image) => {
                let result = self.struct_view(&Expr::Name(RESULT.into(), span), span)?;
                vec![self.register_image(&result, image)]
            }
            None => Vec::new(),
        };
        self.terminate(hir::Terminator { kind: "return", operands, targets: Vec::new() });
        Ok(())
    }

    /// Calls `signature`, whose aggregate result lands in `view`.
    pub(super) fn call_aggregate(
        &mut self,
        signature: &Signature,
        arguments: &[Expr],
        view: &StructView,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(image) = signature.in_registers(self.types) else {
            let slot = self.address_of(view);
            self.emit_call(signature, arguments, Some(slot), span)?;
            return Ok(());
        };
        let value = self.emit_call(signature, arguments, None, span)?.expect("a register result");
        self.store_image(view, hir::Operand::Value(value), image);
        Ok(())
    }
}
