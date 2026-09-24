//! Operators and conversions (section 3).

use crate::abi::modern as rt;
use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn binary(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if matches!(operation, BinaryOp::Is | BinaryOp::IsNot) {
            return self.identity(operation, left, right, expected, span);
        }
        if matches!(operation, BinaryOp::And | BinaryOp::Or) {
            return self.logical(operation, left, right, expected, span);
        }
        if let Some(result) = self.view_comparison(operation, left, right, span)? {
            if expected.is_some_and(|one| one != TypeName::Bool) {
                return Err(type_mismatch(
                    span,
                    expected.expect("checked"),
                    TypeName::Bool,
                ));
            }
            return Ok(result);
        }
        let (left, right) = if is_shift(operation) {
            // A shift's operands are typed apart: the result is the left one's.
            (self.expression(left, None)?, self.expression(right, None)?)
        } else {
            self.operand_pair(left, right)?
        };
        let result = self.arithmetic(operation, left, right, span)?;
        if expected.is_some_and(|one| one != result.type_name) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                result.type_name,
            ));
        }
        Ok(result)
    }

    /// Two operands that meet at a common type, in source order.
    pub(super) fn operand_pair(
        &mut self,
        left: &Expr,
        right: &Expr,
    ) -> Result<(TypedOperand, TypedOperand), Diagnostic> {
        // A literal has no effects, so evaluating it second keeps source order.
        if is_literal(left) && !is_literal(right) {
            let right = self.expression(right, None)?;
            return Ok((self.beside(left, right.type_name)?, right));
        }
        let left = self.expression(left, None)?;
        let right = self.beside(right, left.type_name)?;
        Ok((left, right))
    }

    /// An operand next to one of type `other`: a literal takes that type when it fits.
    pub(super) fn beside(&mut self, expression: &Expr, other: TypeName) -> Result<TypedOperand, Diagnostic> {
        if !is_literal(expression) {
            return self.expression(expression, None);
        }
        if is_integer_literal(expression) && is_integer(other) {
            if let Ok(value) = self.expression(expression, Some(self.rules.promoted(other))) {
                return Ok(value);
            }
            return self.expression(expression, None);
        }
        // `0` is also the null pointer of any type.
        if is_float(other) || is_fixed(other) || (is_integer_literal(expression) && self.types.raw_target(other).is_some()) {
            return self.coerced(expression, other);
        }
        self.expression(expression, None)
    }

    /// A value for a destination of type `target`, converted as an assignment converts it.
    pub(super) fn coerced(&mut self, expression: &Expr, target: TypeName) -> Result<TypedOperand, Diagnostic> {
        if is_float(target) {
            let spelled = match expression {
                Expr::Integer(value, span) => Some((value.to_string(), *span)),
                Expr::Unary {
                    op: UnaryOp::Negative,
                    operand,
                    span,
                } => match operand.as_ref() {
                    Expr::Integer(value, _) => Some((format!("-{value}"), *span)),
                    _ => None,
                },
                _ => None,
            };
            if let Some((spelling, span)) = spelled {
                return self.float(&spelling, Some(target), span);
            }
        }
        // A literal or variant takes its type from where it goes.
        if is_literal(expression)
            || matches!(expression, Expr::Variant { .. } | Expr::Conditional { .. })
            || vectors::builds_vector(expression)
            || matches!(expression, Expr::Borrow { .. } | Expr::Dict(..) | Expr::DictComprehension { .. })
            || self.types.referent(target).is_some()
            || matches!(target, TypeName::Function { .. })
        {
            return self.expression(expression, Some(target));
        }
        let value = self.expression(expression, None)?;
        self.implicit(value, target, expression.span())
    }

    /// C's implicit conversion, between integers and floats only.
    pub(super) fn implicit(
        &mut self,
        value: TypedOperand,
        target: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if value.type_name == target {
            return Ok(value);
        }
        if self.types.reads_through(value.type_name, target) {
            let read_only = self.value(target);
            self.emit("copy", vec![read_only], vec![required(value, span)?], None);
            return Ok(TypedOperand { operand: Some(hir::Operand::Value(read_only)), type_name: target });
        }
        if !conversions::implicit(value.type_name) || !conversions::implicit(target) {
            return Err(type_mismatch(span, target, value.type_name));
        }
        self.converted(value, target, span)
    }

    /// `and` and `or` evaluate their right operand only when it decides the result.
    pub(super) fn logical(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let name = format!("$logical{}", self.next_place);
        let result = self.place(&name, TypeName::Bool, true);
        let left = self.expression(left, Some(TypeName::Bool))?;
        let left = required(left, span)?;
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(result), left.clone()],
            None,
        );
        let decide = self.block();
        let join = self.block();
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![left],
            targets: if operation == BinaryOp::And {
                vec![decide, join]
            } else {
                vec![join, decide]
            },
        });
        self.current = decide;
        let right = self.expression(right, Some(TypeName::Bool))?;
        self.emit(
            "store",
            Vec::new(),
            vec![hir::Operand::Place(result), required(right, span)?],
            None,
        );
        self.terminate(jump(join));
        self.current = join;
        let value = self.value(TypeName::Bool);
        self.emit("load", vec![value], vec![hir::Operand::Place(result)], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(value)),
            type_name: TypeName::Bool,
        })
    }

    /// `a < b < c`: `(a < b) && (b < c)`, each operand evaluated once, in order.
    pub(super) fn chain(
        &mut self,
        operands: &[Expr],
        operations: &[BinaryOp],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let name = self.hidden("chain");
        let result = self.place(&name, TypeName::Bool, true);
        let join = self.block();
        self.in_scope(|this| {
            let mut left = this.evaluated_once(&operands[0])?;
            for (index, operation) in operations.iter().enumerate() {
                let right = this.evaluated_once(&operands[index + 1])?;
                let holds = this.binary(*operation, &left, &right, Some(TypeName::Bool), span)?;
                let holds = required(holds, span)?;
                this.emit(
                    "store",
                    Vec::new(),
                    vec![hir::Operand::Place(result), holds.clone()],
                    None,
                );
                if index + 1 < operations.len() {
                    let next = this.block();
                    this.terminate(hir::Terminator {
                        kind: "branch",
                        operands: vec![holds],
                        targets: vec![next, join],
                    });
                    this.current = next;
                }
                left = right;
            }
            Ok(())
        })?;
        self.terminate(jump(join));
        self.current = join;
        let value = self.value(TypeName::Bool);
        self.emit("load", vec![value], vec![hir::Operand::Place(result)], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(value)),
            type_name: TypeName::Bool,
        })
    }

    /// `operand`, evaluated now, as an expression that reads it again without
    /// evaluating it again. A literal stays one, typed by what it meets, and a
    /// struct stays the place it names.
    fn evaluated_once(&mut self, operand: &Expr) -> Result<Expr, Diagnostic> {
        let span = operand.span();
        if is_literal(operand) || self.struct_type_hint(operand, span).is_some() {
            return Ok(operand.clone());
        }
        let binding = if self.is_char_view(operand) {
            self.sequence_of(operand)?.0
        } else {
            let value = self.expression(operand, None)?;
            let type_name = value.type_name;
            let value = self.materialized(required(value, span)?, type_id(type_name));
            Binding { type_: BindingType::Scalar(type_name), mutable: false, storage: Storage::Parameter(value) }
        };
        let name = self.hidden("operand");
        self.scopes.last_mut().expect("scope").insert(name.clone(), binding);
        Ok(Expr::Name(name, span))
    }

    pub(super) fn conversion(
        &mut self,
        target: TypeName,
        value: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != target) {
            return Err(type_mismatch(span, expected.expect("checked"), target));
        }
        // A number literal takes the target type, so `u8(300)` is rejected rather than wrapped.
        let typed = ((is_integer_literal(value) || is_float_literal(value)) && is_fixed(target))
            || (is_integer_literal(value) && conversions::implicit(target))
            || (is_float_literal(value) && is_float(target));
        let value = if typed {
            self.coerced(value, target)?
        } else {
            self.expression(value, None)?
        };
        self.converted(value, target, span)
    }

    pub(super) fn converted(
        &mut self,
        value: TypedOperand,
        target: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let source = value.type_name;
        if source == target {
            return Ok(value);
        }
        if matches!(source, TypeName::Bits { .. }) {
            let backing = self.bits_backing(value, span)?;
            return self.converted(backing, target, span);
        }
        if is_fixed(source) || is_fixed(target) {
            return self.fixed_converted(value, target, span);
        }
        let op = match (source, target) {
            (from, to) if is_float(from) && is_integer(to) => "truncate",
            (from, to) if conversions::implicit(from) && conversions::implicit(to) => "convert",
            (TypeName::Bool, to) if is_integer(to) => "convert",
            (TypeName::Char | TypeName::Enum { .. } | TypeName::Function { .. }, to) if is_integer(to) => "convert",
            (from, TypeName::Char) if is_integer(from) => "convert",
            _ => {
                return Err(Diagnostic::new(
                    span,
                    format!(
                        "no conversion from {} to {}",
                        type_name_text(source),
                        type_name_text(target)
                    ),
                ));
            }
        };
        if let Some(hir::Operand::Constant(_, constant)) = value.operand {
            if is_integer(source) && is_integer(target) {
                return Ok(TypedOperand {
                    operand: Some(hir::Operand::Constant(
                        type_id(target),
                        wrapped(constant, target),
                    )),
                    type_name: target,
                });
            }
        }
        let operand = required(value, span)?;
        if op == "truncate" && self.unsafe_depth == 0 {
            self.check_truncation(&operand, source, target, span)?;
        }
        let result = self.value(target);
        self.emit(op, vec![result], vec![operand], None);
        if source != TypeName::Bool {
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Value(result)),
                type_name: target,
            });
        }
        // `true` is all ones.
        let bit = self.value(target);
        self.emit(
            "and",
            vec![bit],
            vec![
                hir::Operand::Value(result),
                hir::Operand::Constant(type_id(target), 1),
            ],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(bit)),
            type_name: target,
        })
    }

    /// A conversion to or from a fixed-point type, done on its storage integer.
    pub(super) fn fixed_converted(
        &mut self,
        value: TypedOperand,
        target: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let source = value.type_name;
        if !(is_integer(source) || is_fixed(source)) || !(is_integer(target) || is_fixed(target)) {
            return Err(Diagnostic::new(
                span,
                format!(
                    "no conversion from {} to {}",
                    type_name_text(source),
                    type_name_text(target)
                ),
            ));
        }
        let (stored, from) = self.fixed_storage(value);
        let (to_storage, to) = match target {
            TypeName::Fixed {
                storage, fraction, ..
            } => (storage_type(storage), fraction),
            integer => (integer, 0),
        };
        // Scale in the wider storage, so rescaling up loses nothing it keeps.
        let work = if width(to_storage) > width(stored.type_name) {
            to_storage
        } else {
            stored.type_name
        };
        let stored = self.converted(stored, work, span)?;
        let scaled = if to >= from {
            self.shifted("shl", stored, to - from, span)?
        } else {
            self.toward_zero(stored, from - to, span)?
        };
        let narrowed = self.converted(scaled, to_storage, span)?;
        if to_storage == target {
            return Ok(narrowed);
        }
        let result = self.value(target);
        self.emit(
            "convert",
            vec![result],
            vec![required(narrowed, span)?],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: target,
        })
    }

    /// A fixed-point value as its storage integer and fraction; an integer as itself.
    pub(super) fn fixed_storage(&mut self, value: TypedOperand) -> (TypedOperand, u8) {
        let TypeName::Fixed {
            storage, fraction, ..
        } = value.type_name
        else {
            return (value, 0);
        };
        let storage = storage_type(storage);
        let result = self.value(storage);
        let operand = value.operand.expect("a fixed-point value has an operand");
        self.emit("convert", vec![result], vec![operand], None);
        (
            TypedOperand {
                operand: Some(hir::Operand::Value(result)),
                type_name: storage,
            },
            fraction,
        )
    }

    pub(super) fn shifted(
        &mut self,
        op: &'static str,
        value: TypedOperand,
        count: u8,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if count == 0 {
            return Ok(value);
        }
        let result = self.value(value.type_name);
        self.emit(
            op,
            vec![result],
            vec![
                required(value.clone(), span)?,
                hir::Operand::Constant(U8, i64::from(count)),
            ],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: value.type_name,
        })
    }

    /// `value >> count`, rounded toward zero: a negative value is first biased by `2^count - 1`.
    pub(super) fn toward_zero(
        &mut self,
        value: TypedOperand,
        count: u8,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if !is_signed(value.type_name) {
            return self.shifted("shr", value, count, span);
        }
        let type_name = value.type_name;
        let bits = u8::try_from(8 * width(type_name) - 1).expect("an integer is under 256 bits");
        let sign = self.shifted("sar", value.clone(), bits, span)?;
        let bias = self.value(type_name);
        self.emit(
            "and",
            vec![bias],
            vec![
                required(sign, span)?,
                hir::Operand::Constant(type_id(type_name), (1_i64 << count) - 1),
            ],
            None,
        );
        let biased = self.value(type_name);
        self.emit(
            "add",
            vec![biased],
            vec![required(value, span)?, hir::Operand::Value(bias)],
            None,
        );
        self.shifted(
            "sar",
            TypedOperand {
                operand: Some(hir::Operand::Value(biased)),
                type_name,
            },
            count,
            span,
        )
    }

    /// Both operands evaluated: the usual arithmetic conversions, then the operation.
    pub(super) fn arithmetic(
        &mut self,
        operation: BinaryOp,
        left: TypedOperand,
        right: TypedOperand,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if matches!(
            operation,
            BinaryOp::Is | BinaryOp::IsNot | BinaryOp::And | BinaryOp::Or
        ) {
            return Err(Diagnostic::new(
                span,
                "this operator has no compound assignment",
            ));
        }
        if left.type_name == TypeName::String {
            return self.string_binary(operation, left, right, span);
        }
        if let Some(result) = self.pointer_comparison(operation, &left, &right, span) {
            return result;
        }
        let comparison = is_comparison(operation);
        let (left, right) = if is_shift(operation) {
            if !is_integer(left.type_name) || !is_integer(right.type_name) {
                return Err(Diagnostic::new(span, "a shift requires integer operands"));
            }
            let bits = 8 * width(self.rules.promoted(left.type_name));
            if let Some(hir::Operand::Constant(_, count)) = right.operand {
                if !(0..i64::from(bits)).contains(&count) {
                    return Err(Diagnostic::new(
                        span,
                        format!("shift count {count} is outside 0..{bits}"),
                    ));
                }
            } else {
                let count = required(right.clone(), span)?;
                self.check_below(&count, hir::Operand::Constant(U16, i64::from(bits)), rt::ERROR_SHIFT, span)?;
            }
            let (left_type, right_type) = (
                self.rules.promoted(left.type_name),
                self.rules.promoted(right.type_name),
            );
            (
                self.implicit(left, left_type, span)?,
                self.implicit(right, right_type, span)?,
            )
        } else {
            let Some(common) = self.rules.common(left.type_name, right.type_name) else {
                let (left, right) = (left.type_name, right.type_name);
                if is_integer(left) && is_integer(right) {
                    return Err(Diagnostic::new(
                        span,
                        format!(
                            "{} and {} have no common type; convert one explicitly",
                            type_name_text(left),
                            type_name_text(right)
                        ),
                    ));
                }
                return Err(type_mismatch(span, left, right));
            };
            operand_rule(operation, common, span)?;
            (
                self.implicit(left, common, span)?,
                self.implicit(right, common, span)?,
            )
        };
        if is_fixed(left.type_name)
            && matches!(
                operation,
                BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder
            )
        {
            return self.fixed_binary(operation, left, right, span);
        }
        if operation == BinaryOp::Divide && is_integer(left.type_name) {
            return Err(Diagnostic::new(
                span,
                "'/' is not defined for integers; use '//'",
            ));
        }
        if operation == BinaryOp::FloorDivide {
            return self.floor_divide(left, right, span);
        }
        let result_type = if comparison {
            TypeName::Bool
        } else {
            left.type_name
        };
        let result = self.value(result_type);
        let op = match operation {
            BinaryOp::Add if is_float(left.type_name) => "fadd",
            BinaryOp::Subtract if is_float(left.type_name) => "fsub",
            BinaryOp::Multiply if is_float(left.type_name) => "fmul",
            BinaryOp::Divide if is_float(left.type_name) => "fdiv",
            BinaryOp::Remainder if is_float(left.type_name) => {
                return Err(Diagnostic::new(span, "'%' is not defined for floats"));
            }
            BinaryOp::Add => "add",
            BinaryOp::Subtract => "sub",
            BinaryOp::Multiply => "mul",
            BinaryOp::Divide if is_unsigned(left.type_name) => "udiv",
            BinaryOp::Divide => "div",
            BinaryOp::Remainder if is_unsigned(left.type_name) => "urem",
            BinaryOp::Remainder => "rem",
            BinaryOp::Equal => "eq",
            BinaryOp::NotEqual => "ne",
            BinaryOp::Less if is_unsigned(left.type_name) => "below",
            BinaryOp::LessEqual if is_unsigned(left.type_name) => "beloweq",
            BinaryOp::Greater if is_unsigned(left.type_name) => "above",
            BinaryOp::GreaterEqual if is_unsigned(left.type_name) => "aboveeq",
            BinaryOp::Less => "lt",
            BinaryOp::LessEqual => "le",
            BinaryOp::Greater => "gt",
            BinaryOp::GreaterEqual => "ge",
            BinaryOp::BitAnd => "and",
            BinaryOp::BitOr => "or",
            BinaryOp::BitXor => "xor",
            BinaryOp::ShiftLeft => "shl",
            BinaryOp::ShiftRight if is_unsigned(left.type_name) => "shr",
            BinaryOp::ShiftRight => "sar",
            BinaryOp::Is | BinaryOp::IsNot => unreachable!("identity handled above"),
            BinaryOp::FloorDivide => unreachable!("floor division handled above"),
            BinaryOp::And | BinaryOp::Or => {
                return Err(Diagnostic::new(
                    span,
                    "'and' and 'or' have no compound assignment",
                ));
            }
        };
        self.emit(
            op,
            vec![result],
            vec![required(left, span)?, required(right, span)?],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: result_type,
        })
    }

    pub(super) fn fixed_binary(
        &mut self,
        operation: BinaryOp,
        left: TypedOperand,
        right: TypedOperand,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let fixed_type = left.type_name;
        let TypeName::Fixed {
            storage, fraction, ..
        } = fixed_type
        else {
            unreachable!("fixed arithmetic requires a fixed type")
        };
        if operation == BinaryOp::Remainder {
            return Err(Diagnostic::new(
                span,
                "'%' is not defined for fixed-point values",
            ));
        }
        // Keep fixed i32 arithmetic intact through HIR.  Its widened
        // intermediate is a machine operand pair, not a first-class i64:
        // expanding it here made the generic int64 legalizer select complete
        // 64x64 multiply and 64/64 divide helpers for a 32-bit stored value.
        // Target lowering can instead use the native 32x32->64 product and
        // EDX:EAX dividend while preserving the language's wrapping result.
        if storage == FixedStorage::I32 {
            let result = self.value(fixed_type);
            let operation = match operation {
                BinaryOp::Multiply => "fixed_mul",
                BinaryOp::Divide => "fixed_div",
                _ => unreachable!("only scaling fixed operations reach this helper"),
            };
            self.emit(
                operation,
                vec![result],
                vec![
                    required(left, span)?,
                    required(right, span)?,
                    hir::Operand::Constant(U8, i64::from(fraction)),
                ],
                None,
            );
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Value(result)),
                type_name: fixed_type,
            });
        }
        let wide_type = match storage {
            FixedStorage::I16 => TypeName::I32,
            FixedStorage::I32 => TypeName::I64,
        };
        let left = self.convert_value(left, wide_type, span)?;
        let right = self.convert_value(right, wide_type, span)?;
        let left = required(left, span)?;
        let right = required(right, span)?;
        let adjusted = self.value(wide_type);
        match operation {
            BinaryOp::Multiply => {
                let product = self.value(wide_type);
                self.emit("mul", vec![product], vec![left, right], None);
                self.emit(
                    "sar",
                    vec![adjusted],
                    vec![
                        hir::Operand::Value(product),
                        hir::Operand::Constant(U8, i64::from(fraction)),
                    ],
                    None,
                );
            }
            BinaryOp::Divide => {
                let numerator = self.value(wide_type);
                self.emit(
                    "shl",
                    vec![numerator],
                    vec![left, hir::Operand::Constant(U8, i64::from(fraction))],
                    None,
                );
                self.emit(
                    "div",
                    vec![adjusted],
                    vec![hir::Operand::Value(numerator), right],
                    None,
                );
            }
            _ => unreachable!("only scaling fixed operations reach this helper"),
        }
        self.convert_value(
            TypedOperand {
                operand: Some(hir::Operand::Value(adjusted)),
                type_name: wide_type,
            },
            fixed_type,
            span,
        )
    }

    pub(super) fn convert_value(
        &mut self,
        value: TypedOperand,
        target: TypeName,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let result = self.value(target);
        self.emit("convert", vec![result], vec![required(value, span)?], None);
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: target,
        })
    }

    pub(super) fn identity<'e>(
        &mut self,
        operation: BinaryOp,
        left: &'e Expr,
        right: &'e Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if expected.is_some_and(|one| one != TypeName::Bool) {
            return Err(type_mismatch(
                span,
                expected.expect("checked"),
                TypeName::Bool,
            ));
        }
        let unborrowed = |expression: &'e Expr| match expression {
            Expr::Borrow { operand, .. } => operand.as_ref(),
            other => other,
        };
        let (left, right) = (unborrowed(left), unborrowed(right));
        for one in [left, right] {
            if !matches!(self.struct_expression_type(one, span), Ok(Some(_))) {
                return Err(Diagnostic::new(one.span(), "a scalar has no identity; compare it with '=='"));
            }
        }
        let (left_index, right_index, distinct) = match (self.view_identity(left), self.view_identity(right)) {
            // Two elements of arrays a loop views: the same array, and index.
            (Some((left_place, left_index)), Some((right_place, right_index))) => (left_index, right_index, left_place != right_place),
            _ => {
                let (left_view, right_view) = (self.struct_view(left, span)?, self.struct_view(right, span)?);
                let distinct = left_view.pointer.is_none() && right_view.pointer.is_none() && left_view.place != right_view.place;
                (self.address_of(&left_view), self.address_of(&right_view), distinct)
            }
        };
        if distinct {
            return Ok(TypedOperand {
                operand: Some(hir::Operand::Constant(
                    BOOL,
                    if operation == BinaryOp::IsNot { -1 } else { 0 },
                )),
                type_name: TypeName::Bool,
            });
        }
        let result = self.value(TypeName::Bool);
        self.emit(
            if operation == BinaryOp::Is {
                "eq"
            } else {
                "ne"
            },
            vec![result],
            vec![left_index, right_index],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: TypeName::Bool,
        })
    }

    /// A loop's view of an array element: the array's place and the index.
    fn view_identity(&self, expression: &Expr) -> Option<(u32, hir::Operand)> {
        let Expr::Name(name, _) = expression else {
            return None;
        };
        match &self.visible(name)?.storage {
            Storage::ArrayView { place, index } => Some((*place, index.clone())),
            _ => None,
        }
    }
}
