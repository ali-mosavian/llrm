//! Expressions and literals.

use crate::abi::nib as rt;
use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn expression(
        &mut self,
        expression: &Expr,
        expected: Option<TypeName>,
    ) -> Result<TypedOperand, Diagnostic> {
        match expression {
            Expr::Zero(span) => {
                let type_name = expected.ok_or_else(|| Diagnostic::new(*span, "a zero value has the type expected of it"))?;
                Ok(TypedOperand { operand: Some(hir::Operand::Constant(type_id(type_name), 0)), type_name })
            }
            // A place where a reference goes is borrowed; a conditional or a
            // call that already makes the reference is compiled as it is.
            _ if expected.is_some_and(|one| self.types.referent(one).is_some())
                && !matches!(expression, Expr::Conditional { .. })
                && self.expression_type_hint(expression) != expected =>
            {
                self.reference_to(expression, expected.expect("checked"), expression.span())
            }
            Expr::Lambda { parameters, body, span } if matches!(expected, Some(TypeName::Function { .. })) => {
                let scopes = self.scopes.clone();
                self.lifted_lambda(parameters, body, &scopes, expected.expect("matched"), *span)
            }
            Expr::Lambda { span, .. } => Err(Diagnostic::new(
                *span,
                "a lambda is bound or passed, not used as a value",
            )),
            Expr::Integer(value, span) => self.integer(*value, expected, *span),
            Expr::Float(spelling, span) => self.float(spelling, expected, *span),
            Expr::Character(value, span) => {
                if expected.is_some_and(|one| one != TypeName::Char) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        TypeName::Char,
                    ));
                }
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Constant(CHAR, i64::from(*value))),
                    type_name: TypeName::Char,
                })
            }
            Expr::String(value, span) => self.string_literal(value, expected, *span),
            Expr::FString { parts, span } => {
                if expected.is_some_and(|one| one != TypeName::String) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        TypeName::String,
                    ));
                }
                // The formatters write into a new string instead of the console.
                let parts = self.settled_parts(parts, false)?;
                self.emit_builtin(rt::PRINT_BEGIN, Vec::new());
                self.print_parts(&parts, *span)?;
                let built = self.emit_builtin(rt::PRINT_END, Vec::new()).expect("a string");
                Ok(TypedOperand {
                    operand: Some(self.temporary_owned(built, TypeName::String)),
                    type_name: TypeName::String,
                })
            }
            Expr::Array(_, span) | Expr::Repeat { span, .. } | Expr::Comprehension { span, .. } => {
                self.vector_literal(expression, expected, *span)
            }
            Expr::Conversion {
                target,
                value,
                span,
            } => self.conversion(*target, value, expected, *span),
            Expr::Generator { span, .. } => Err(Diagnostic::new(
                *span,
                "a generator is non-escaping and must be consumed by a for loop",
            )),
            Expr::Dict(_, span) | Expr::DictComprehension { span, .. } => self.dictionary_literal(expression, expected, *span),
            Expr::StructLiteral { name, fields, span } if self.types.bits.contains_key(name) => {
                let value = self.bits_literal(name, fields, *span)?;
                let wanted = expected.unwrap_or(value.type_name);
                self.implicit(value, wanted, *span)
            }
            Expr::StructLiteral { span, .. } | Expr::Tuple(_, span) => Err(Diagnostic::new(
                *span,
                "a struct literal requires an expected struct type",
            )),
            Expr::Borrow {
                mutable,
                operand,
                span,
            } if matches!(expected, Some(TypeName::Pointer { .. })) => {
                self.raw_address(operand, *mutable, expected.expect("matched"), *span)
            }
            Expr::Borrow { span, .. } => Err(Diagnostic::new(
                *span,
                "a borrow is valid only as a borrowed function argument",
            )),
            Expr::Boolean(value, span) => {
                if expected.is_some_and(|one| one != TypeName::Bool) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        TypeName::Bool,
                    ));
                }
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Constant(BOOL, if *value { -1 } else { 0 })),
                    type_name: TypeName::Bool,
                })
            }
            Expr::Name(name, span) if self.visible(name).is_none() && self.signatures.contains_key(name) => {
                self.function_value(name, expected, *span).expect("a function")
            }
            Expr::Name(name, span) if matches!((self.lambda_named(name), expected), (Some(_), Some(TypeName::Function { .. }))) => {
                let lambda = self.lambdas[self.lambda_named(name).expect("matched") as usize].clone();
                lambda.lifted(self, expected.expect("matched"), *span)
            }
            Expr::Name(name, span) => {
                let binding = self.binding(name, *span)?.clone();
                let BindingType::Scalar(type_name) = binding.type_ else {
                    let message = if matches!(binding.type_, BindingType::Slice { element: ElementType::Scalar(TypeName::Char), rank: 1 }) {
                        format!("{name:?} is a borrowed view of a string; .copy() it to own it")
                    } else {
                        format!("aggregate {name:?} requires an index or field")
                    };
                    return Err(Diagnostic::new(*span, message));
                };
                if expected.is_some_and(|one| one != type_name) {
                    return Err(type_mismatch(*span, expected.expect("checked"), type_name));
                }
                let operand = match binding.storage.clone() {
                    Storage::Parameter(value) => hir::Operand::Value(value),
                    Storage::Place(place) => {
                        let value = self.value(type_name);
                        self.emit("load", vec![value], vec![hir::Operand::Place(place)], None);
                        hir::Operand::Value(value)
                    }
                    Storage::ArrayView { place, index } => {
                        let value = self.value(type_name);
                        self.emit(
                            "load",
                            vec![value],
                            vec![hir::Operand::ArrayElement(place, vec![index])],
                            None,
                        );
                        hir::Operand::Value(value)
                    }
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
                        hir::Operand::Value(value)
                    }
                    Storage::Slice(_) => unreachable!("a scalar binding is not a slice"),
                    Storage::Lambda(_) => {
                        return Err(Diagnostic::new(
                            *span,
                            format!("lambda {name:?} is called, not read"),
                        ));
                    }
                };
                self.record_origin(&operand, type_name, &binding.storage);
                Ok(TypedOperand {
                    operand: Some(operand),
                    type_name,
                })
            }
            Expr::Index { span, .. } | Expr::Member { span, .. }
                if self.sequence_property(expression).is_some() =>
            {
                let (receiver, name, arguments) =
                    self.sequence_property(expression).expect("checked");
                self.array_method(receiver, name, arguments, expected, *span)
            }
            Expr::MethodCall { name, span, .. } if Self::is_property_method(name) => Err(
                Diagnostic::new(*span, format!("{name} is a field, not a method")),
            ),
            Expr::Index {
                base,
                indices,
                span,
            } => self.index_expression(base, indices, expected, *span),
            Expr::Slice { span, .. } => Err(Diagnostic::new(
                *span,
                "a slice is a scoped view and cannot be used as a scalar value",
            )),
            Expr::Member { base, field, span } if self.bits_type(base).is_some() => {
                let value = self.bits_read(base, field, *span)?;
                let wanted = expected.unwrap_or(value.type_name);
                self.implicit(value, wanted, *span)
            }
            Expr::Member { base, field, span } => {
                let (place, type_name, _, _) = self.member_place(base, field, *span)?;
                // A reference read where no reference is expected reads what it refers to.
                if let (Some(ElementType::Scalar(target)), false) = (self.types.referent(type_name), expected == Some(type_name)) {
                    let pointer = self.value(type_name);
                    self.emit("load", vec![pointer], vec![place], None);
                    if expected.is_some_and(|one| one != target) {
                        return Err(type_mismatch(*span, expected.expect("checked"), target));
                    }
                    let through = hir::Operand::IndirectPlace { base: pointer, offset: 0, type_id: type_id(target), inbounds: false };
                    let result = self.value(target);
                    self.emit("load", vec![result], vec![through], None);
                    return Ok(TypedOperand { operand: Some(hir::Operand::Value(result)), type_name: target });
                }
                if expected.is_some_and(|one| one != type_name) {
                    return Err(type_mismatch(*span, expected.expect("checked"), type_name));
                }
                let result = self.value(type_name);
                self.emit("load", vec![result], vec![place.clone()], None);
                if ownership::needs_drop(type_name) && self.frame_field(base, field, *span)?.is_some() {
                    self.origins.insert(result, ownership::Origin::Frame(place));
                }
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Value(result)),
                    type_name,
                })
            }
            Expr::Unary { op: UnaryOp::Deref, operand, span } => {
                let name = self.dereferenced(operand, *span)?;
                self.expression(&Expr::Name(name, *span), expected)
            }
            Expr::Unary { op, operand, span } => {
                if *op == UnaryOp::Negative {
                    if let Expr::Integer(value, _) = operand.as_ref() {
                        let wanted = expected.unwrap_or({
                            if *value <= 32768 {
                                TypeName::I16
                            } else {
                                TypeName::I32
                            }
                        });
                        return self.integer(-*value, Some(wanted), *span);
                    }
                }
                // Only a literal takes its type from context; anything else has its own.
                let wanted = match op {
                    UnaryOp::Not => Some(TypeName::Bool),
                    _ if is_literal(operand) => expected,
                    _ => None,
                };
                let operand = self.expression(operand, wanted)?;
                let operand = match op {
                    UnaryOp::Not if operand.type_name != TypeName::Bool => {
                        return Err(Diagnostic::new(*span, "not requires bool"));
                    }
                    UnaryOp::Not => operand,
                    UnaryOp::Complement if !is_integer(operand.type_name) => {
                        return Err(Diagnostic::new(*span, "'~' requires an integer"));
                    }
                    UnaryOp::Negative
                        if !is_numeric(operand.type_name)
                            || (is_fixed(operand.type_name) && !is_signed(operand.type_name)) =>
                    {
                        return Err(Diagnostic::new(*span, "unary '-' requires a number"));
                    }
                    _ => {
                        let promoted = self.rules.promoted(operand.type_name);
                        self.implicit(operand, promoted, *span)?
                    }
                };
                let result = self.value(operand.type_name);
                self.emit(
                    match op {
                        UnaryOp::Negative if is_float(operand.type_name) => "fneg",
                        UnaryOp::Negative => "neg",
                        UnaryOp::Not | UnaryOp::Complement => "not",
                        UnaryOp::Deref => unreachable!("a place, compiled above"),
                    },
                    vec![result],
                    vec![required(operand.clone(), *span)?],
                    None,
                );
                if expected.is_some_and(|one| one != operand.type_name) {
                    return Err(type_mismatch(
                        *span,
                        expected.expect("checked"),
                        operand.type_name,
                    ));
                }
                Ok(TypedOperand {
                    operand: Some(hir::Operand::Value(result)),
                    type_name: operand.type_name,
                })
            }
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => self.binary(*op, left, right, expected, *span),
            Expr::Chain { operands, operations, span } => self.chain(operands, operations, expected, *span),
            Expr::Call {
                name,
                type_arguments,
                arguments,
                span,
            } if name == calls::SIZE_OF => self.size_of(type_arguments, arguments, *span),
            Expr::Call {
                name,
                arguments,
                span,
                ..
            } => self.call(name, arguments, expected, *span),
            Expr::MethodCall {
                receiver,
                name,
                type_arguments,
                arguments,
                span,
            } => {
                if let Some(call) = self.method_as_call(expression) {
                    return self.expression(&call, expected);
                }
                if let Some(result) = self.pointer_method(receiver, name, type_arguments, arguments, *span)? {
                    return Ok(result);
                }
                if let Expr::Name(owner, _) = receiver.as_ref() {
                    if self.visible(owner).is_none() && self.known_signature(&format!("{owner}.{name}")).is_some() {
                        return Err(super::super::modules::method_without_value(owner, name, *span));
                    }
                }
                if let Some(test) = self.emptiness(expression) {
                    return self.expression(&test, expected);
                }
                if let Some(result) = self.vector_method(receiver, name, arguments, *span)? {
                    return Ok(result);
                }
                match self.string_method(receiver, name, arguments, *span)? {
                    Some(result) => Ok(result),
                    None => self.array_method(receiver, name, arguments, expected, *span),
                }
            }
            Expr::Conditional {
                condition,
                then,
                otherwise,
                span,
            } => self.conditional(condition, then, otherwise, expected, *span),
            Expr::Variant {
                enum_name,
                name,
                arguments,
                span,
            } => self.scalar_variant(enum_name.as_deref(), name, arguments, expected, *span),
            Expr::NamedArgument { span, .. } => Err(Diagnostic::new(
                *span,
                "a named argument is valid only in a call",
            )),
            Expr::Try { operand, span } => self.try_value(operand, expected, *span),
        }
    }

    pub(super) fn integer(
        &self,
        value: i64,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if let Some(type_name @ TypeName::Fixed { fraction, .. }) = expected {
            let scaled = i128::from(value) << fraction;
            let value = fixed_storage_value(scaled, type_name, span)?;
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Constant(type_id(type_name), value)),
                type_name,
            });
        }
        // `0` is a raw pointer that points nowhere.
        if let Some(pointer) = expected.filter(|one| value == 0 && self.types.raw_target(*one).is_some()) {
            return Ok(TypedOperand { operand: Some(hir::Operand::Constant(type_id(pointer), 0)), type_name: pointer });
        }
        let type_name = match expected {
            Some(type_name) if is_integer(type_name) || type_name == TypeName::Char => type_name,
            Some(other) => return Err(type_mismatch(span, other, TypeName::I16)),
            None if i16::try_from(value).is_ok() => TypeName::I16,
            None if i32::try_from(value).is_ok() => TypeName::I32,
            None => return Err(Diagnostic::new(span, "integer literal does not fit i32")),
        };
        let fits = match type_name {
            TypeName::Char | TypeName::U8 => u8::try_from(value).is_ok(),
            TypeName::I8 => i8::try_from(value).is_ok(),
            TypeName::I16 => i16::try_from(value).is_ok(),
            TypeName::U16 => u16::try_from(value).is_ok(),
            TypeName::I32 => i32::try_from(value).is_ok(),
            TypeName::U32 => u32::try_from(value).is_ok(),
            _ => false,
        };
        if !fits {
            return Err(Diagnostic::new(
                span,
                format!(
                    "integer literal {value} does not fit {}",
                    type_name_text(type_name)
                ),
            ));
        }
        Ok(TypedOperand {
            operand: Some(hir::Operand::Constant(type_id(type_name), value)),
            type_name,
        })
    }

    pub(super) fn float(
        &mut self,
        spelling: &str,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if let Some(type_name @ TypeName::Fixed { fraction, .. }) = expected {
            let scaled = scaled_decimal(spelling, fraction, span)?;
            let value = fixed_storage_value(scaled, type_name, span)?;
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Constant(type_id(type_name), value)),
                type_name,
            });
        }
        let type_name = match expected {
            Some(type_name) if is_float(type_name) => type_name,
            Some(other) => return Err(type_mismatch(span, other, TypeName::F64)),
            None => TypeName::F64,
        };
        let parsed = spelling
            .parse::<f64>()
            .map_err(|_| Diagnostic::new(span, "invalid floating literal"))?;
        let bits = match type_name {
            TypeName::F32 => {
                let rounded = parsed as f32;
                if !rounded.is_finite() {
                    return Err(Diagnostic::new(span, "floating literal does not fit f32"));
                }
                u64::from(rounded.to_bits())
            }
            TypeName::F64 => {
                if !parsed.is_finite() {
                    return Err(Diagnostic::new(span, "floating literal does not fit f64"));
                }
                parsed.to_bits()
            }
            _ => unreachable!(),
        };
        let symbol = self.literals.float(type_name, bits);
        let place = if let Some(place) = self.constant_places.get(&symbol) {
            *place
        } else {
            let place = self.static_place(symbol, type_name);
            self.constant_places.insert(symbol, place);
            place
        };
        let result = self.value(type_name);
        self.emit("load", vec![result], vec![hir::Operand::Place(place)], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name,
        })
    }

    pub(super) fn string_literal(
        &mut self,
        bytes: &[u8],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::String) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::String,
            ));
        }
        if bytes.len() > u16::MAX as usize {
            return Err(Diagnostic::new(
                span,
                "string literal exceeds the 16-bit descriptor",
            ));
        }
        let symbol = self.literals.string(bytes);
        let place = if let Some(place) = self.constant_places.get(&symbol) {
            *place
        } else {
            let place = self.static_string_place(symbol, bytes.len() as u32 + 1);
            self.constant_places.insert(symbol, place);
            place
        };
        let result = self.value(TypeName::String);
        self.emit(
            "address",
            vec![result],
            vec![hir::Operand::Place(place)],
            None,
        );
        self.origins.insert(result, ownership::Origin::Static);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: TypeName::String,
        })
    }

    pub(super) fn array_index(
        &mut self,
        expression: &Expr,
        length: Option<u32>,
    ) -> Result<hir::Operand, Diagnostic> {
        if let (Some(length), Expr::Integer(value, span)) = (length, expression) {
            if *value < 0 || *value >= i64::from(length) {
                return Err(Diagnostic::new(
                    *span,
                    format!("array index {value} is outside 0..{length}"),
                ));
            }
        }
        let index = self.expression(expression, None)?;
        if !is_integer(index.type_name) {
            return Err(Diagnostic::new(
                expression.span(),
                "array index must be an integer",
            ));
        }
        required(index, expression.span())
    }
}
