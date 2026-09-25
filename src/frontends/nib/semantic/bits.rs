//! `bits struct`: fields packed into one backing integer, from bit 0 upward.
//! A field is read with a shift and a mask, and written by merging it back.

use super::*;

#[derive(Clone, Debug)]
pub(super) struct BitsLayout {
    pub(super) name: String,
    pub(super) type_name: TypeName,
    pub(super) backing: TypeName,
    pub(super) fields: Vec<(String, BitField)>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BitField {
    pub(super) low: u32,
    pub(super) bits: u32,
    /// What a read gives: the smallest standard type holding the field.
    pub(super) read: TypeName,
}

impl BitField {
    fn mask(self) -> i64 {
        (1_i64 << self.bits) - 1
    }

    fn signed(self) -> bool {
        matches!(self.read, TypeName::I8 | TypeName::I16 | TypeName::I32)
    }
}

impl TypeRegistry {
    pub(super) fn register_bits(
        &mut self,
        declaration: &Struct,
        backing: TypeName,
    ) -> Result<(), Diagnostic> {
        let mut fields = Vec::new();
        let mut low = 0;
        for field in &declaration.fields {
            let (bits, read) = self.bit_field_type(&field.type_spec, field.span)?;
            fields.push((field.name.clone(), BitField { low, bits, read }));
            low += bits;
        }
        if low > width(backing) * 8 {
            return Err(Diagnostic::new(
                declaration.span,
                format!(
                    "{}'s fields take {low} bits, more than its {}",
                    declaration.name,
                    type_name_text(backing)
                ),
            ));
        }
        let type_id = self.types.len() as u32 + 1;
        self.types.push(plain_type(
            type_id,
            &declaration.name,
            "integer",
            width(backing),
            Some(false),
            "none",
        ));
        let type_name = TypeName::Bits {
            type_id,
            width: width(backing) as u8,
        };
        self.bits.insert(
            declaration.name.clone(),
            BitsLayout {
                name: declaration.name.clone(),
                type_name,
                backing,
                fields,
            },
        );
        Ok(())
    }

    /// A field's width, and the type a read of it gives.
    fn bit_field_type(
        &mut self,
        spec: &TypeSpec,
        span: Span,
    ) -> Result<(u32, TypeName), Diagnostic> {
        if let TypeSpec::Named(name) = spec {
            let sized = name
                .strip_prefix('u')
                .map(|bits| (bits, false))
                .or_else(|| name.strip_prefix('i').map(|bits| (bits, true)))
                .and_then(|(bits, signed)| Some((bits.parse::<u32>().ok()?, signed)));
            if let Some((bits, signed)) = sized {
                let read = match (bits, signed) {
                    (1..=8, false) => TypeName::U8,
                    (9..=16, false) => TypeName::U16,
                    (17..=32, false) => TypeName::U32,
                    (2..=8, true) => TypeName::I8,
                    (9..=16, true) => TypeName::I16,
                    (17..=32, true) => TypeName::I32,
                    _ => {
                        return Err(Diagnostic::new(
                            span,
                            format!("{name} is not a field width"),
                        ));
                    }
                };
                return Ok((bits, read));
            }
            if let Some(layout) = self.enums.get(name) {
                return match layout.element {
                    ElementType::Scalar(read) => Ok((layout.bits, read)),
                    ElementType::Struct(_) => Err(Diagnostic::new(
                        span,
                        "an enum with payloads cannot be a bit field",
                    )),
                };
            }
            // A bits struct, like an enum, is as wide as it is declared.
            if let Some(layout) = self.bits.get(name) {
                return Ok((8 * width(layout.backing), layout.type_name));
            }
        }
        match self.resolve_element(spec, span)? {
            ElementType::Scalar(TypeName::Bool) => Ok((1, TypeName::Bool)),
            ElementType::Scalar(read @ (TypeName::U8 | TypeName::I8)) => Ok((8, read)),
            ElementType::Scalar(read @ (TypeName::U16 | TypeName::I16)) => Ok((16, read)),
            ElementType::Scalar(read @ (TypeName::U32 | TypeName::I32)) => Ok((32, read)),
            _ => Err(Diagnostic::new(
                span,
                "a bit field is bool, uN, iN, a sized enum, or a bits struct",
            )),
        }
    }

    pub(super) fn bits_of(&self, type_name: TypeName) -> Option<&BitsLayout> {
        self.bits.values().find(|one| one.type_name == type_name)
    }

    pub(super) fn bit_field(
        &self,
        type_name: TypeName,
        field: &str,
        span: Span,
    ) -> Result<BitField, Diagnostic> {
        let layout = self.bits_of(type_name).expect("a bits type");
        layout
            .fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, one)| *one)
            .ok_or_else(|| Diagnostic::new(span, format!("{} has no field {field:?}", layout.name)))
    }
}

impl FunctionCompiler<'_> {
    /// The bits type `expression` has, when it has one.
    pub(super) fn bits_type(&self, expression: &Expr) -> Option<TypeName> {
        self.expression_type_hint(expression)
            .filter(|one| matches!(one, TypeName::Bits { .. }))
    }

    /// The type a read of `base.field` gives, when `base` is a bits value.
    pub(super) fn bit_field_hint(&self, base: &Expr, field: &str) -> Option<TypeName> {
        let type_name = self.bits_type(base)?;
        self.types
            .bit_field(type_name, field, base.span())
            .ok()
            .map(|one| one.read)
    }

    /// `Attr(fg=15, bg=1, blink=false)`: each field shifted into place.
    pub(super) fn bits_literal(
        &mut self,
        name: &str,
        fields: &[(String, Expr, Span)],
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let layout = self.types.bits.get(name).cloned().expect("a bits struct");
        let mut packed = hir::Operand::Constant(type_id(layout.backing), 0);
        for (field_name, field) in &layout.fields {
            let (_, value, value_span) = fields
                .iter()
                .find(|(given, _, _)| given == field_name)
                .ok_or_else(|| {
                    Diagnostic::new(span, format!("{name} needs field {field_name:?}"))
                })?;
            let value = self.coerced(value, field.read)?;
            packed = self.inserted(
                packed,
                layout.backing,
                *field,
                required(value, *value_span)?,
                *value_span,
            )?;
        }
        Ok(TypedOperand {
            operand: Some(self.resized(packed, layout.backing, layout.type_name)),
            type_name: layout.type_name,
        })
    }

    /// `Attr(raw)`: the backing integer as the bits struct.
    pub(super) fn bits_from(
        &mut self,
        name: &str,
        value: &Expr,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let layout = self.types.bits.get(name).cloned().expect("a bits struct");
        let value = self.coerced(value, layout.backing)?;
        let value = required(value, span)?;
        Ok(TypedOperand {
            operand: Some(self.resized(value, layout.backing, layout.type_name)),
            type_name: layout.type_name,
        })
    }

    /// `u8(a)`: the bits struct as its backing integer, then converted.
    pub(super) fn bits_backing(
        &mut self,
        value: TypedOperand,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let backing = self
            .types
            .bits_of(value.type_name)
            .expect("a bits type")
            .backing;
        let operand = required(value.clone(), span)?;
        Ok(TypedOperand {
            operand: Some(self.resized(operand, value.type_name, backing)),
            type_name: backing,
        })
    }

    /// `a.bg`.
    pub(super) fn bits_read(
        &mut self,
        base: &Expr,
        field: &str,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let value = self.expression(base, None)?;
        let type_name = value.type_name;
        let field = self.types.bit_field(type_name, field, span)?;
        let backing = self.types.bits_of(type_name).expect("a bits type").backing;
        let packed = self.resized(required(value, span)?, type_name, backing);
        Ok(self.extracted(packed, backing, field))
    }

    /// The field `field` of `packed`, as its read type.
    pub(super) fn extracted(
        &mut self,
        packed: hir::Operand,
        backing: TypeName,
        field: BitField,
    ) -> TypedOperand {
        let backing_bits = width(backing) * 8;
        let value = if field.signed() {
            // To the top, then an arithmetic shift down brings the sign.
            let signed = signed_of(backing);
            let packed = self.resized(packed, backing, signed);
            let up = self.bit_shift("shl", packed, backing_bits - field.low - field.bits, signed);
            let down = self.bit_shift("sar", up, backing_bits - field.bits, signed);
            self.resized(down, signed, field.read)
        } else {
            let shifted = self.bit_shift("shr", packed, field.low, backing);
            let masked = if field.low + field.bits < backing_bits {
                self.binary_op(
                    "and",
                    shifted,
                    hir::Operand::Constant(type_id(backing), field.mask()),
                    backing,
                )
            } else {
                shifted
            };
            let integer = integer_of(field.read, backing);
            let resized = self.resized(masked, backing, integer);
            self.resized(resized, integer, field.read)
        };
        TypedOperand {
            operand: Some(value),
            type_name: field.read,
        }
    }

    /// `packed` with `field` replaced by `value`, wrapped to the field's width.
    pub(super) fn inserted(
        &mut self,
        packed: hir::Operand,
        backing: TypeName,
        field: BitField,
        value: hir::Operand,
        span: Span,
    ) -> Result<hir::Operand, Diagnostic> {
        let integer = integer_of(field.read, backing);
        if let (hir::Operand::Constant(_, constant), false) = (&value, field.read == TypeName::Bool)
        {
            let constant = *constant;
            let fits = if field.signed() {
                (-(1_i64 << (field.bits - 1))..1_i64 << (field.bits - 1)).contains(&constant)
            } else {
                (0..=field.mask()).contains(&constant)
            };
            if !fits {
                return Err(Diagnostic::new(
                    span,
                    format!("{constant} does not fit in {} bits", field.bits),
                ));
            }
        }
        let value = self.resized(value, field.read, integer);
        let value = self.resized(value, integer, backing);
        let value = self.binary_op(
            "and",
            value,
            hir::Operand::Constant(type_id(backing), field.mask()),
            backing,
        );
        let value = self.bit_shift("shl", value, field.low, backing);
        let kept = !(field.mask() << field.low) & ((1_i64 << (width(backing) * 8)) - 1);
        let cleared = self.binary_op(
            "and",
            packed,
            hir::Operand::Constant(type_id(backing), kept),
            backing,
        );
        Ok(self.binary_op("or", cleared, value, backing))
    }

    pub(super) fn bit_shift(
        &mut self,
        op: &'static str,
        value: hir::Operand,
        count: u32,
        type_name: TypeName,
    ) -> hir::Operand {
        if count == 0 {
            return value;
        }
        self.binary_op(
            op,
            value,
            hir::Operand::Constant(type_id(type_name), i64::from(count)),
            type_name,
        )
    }

    pub(super) fn binary_op(
        &mut self,
        op: &'static str,
        left: hir::Operand,
        right: hir::Operand,
        type_name: TypeName,
    ) -> hir::Operand {
        let result = self.value(type_name);
        self.emit(op, vec![result], vec![left, right], None);
        hir::Operand::Value(result)
    }

    /// An integer as another integer type: widened, narrowed, or retyped.
    /// A `bool` is made by comparing, so `true` is all ones.
    pub(super) fn resized(
        &mut self,
        value: hir::Operand,
        from: TypeName,
        to: TypeName,
    ) -> hir::Operand {
        if from == to {
            return value;
        }
        let result = self.value(to);
        if to == TypeName::Bool {
            let zero = hir::Operand::Constant(type_id(from), 0);
            self.emit("ne", vec![result], vec![value, zero], None);
        } else {
            self.emit("convert", vec![result], vec![value], None);
        }
        hir::Operand::Value(result)
    }
}

/// The integer a read type's bits travel as: a bool or enum as unsigned.
fn integer_of(read: TypeName, backing: TypeName) -> TypeName {
    match read {
        TypeName::Bool | TypeName::Enum { width: 1, .. } | TypeName::Bits { width: 1, .. } => {
            TypeName::U8
        }
        TypeName::Enum { width: 2, .. } | TypeName::Bits { width: 2, .. } => TypeName::U16,
        TypeName::Bits { width: 4, .. } => TypeName::U32,
        TypeName::Enum { .. } | TypeName::Bits { .. } => backing,
        other => other,
    }
}

fn signed_of(backing: TypeName) -> TypeName {
    match backing {
        TypeName::U8 => TypeName::I8,
        TypeName::U16 => TypeName::I16,
        _ => TypeName::I32,
    }
}
