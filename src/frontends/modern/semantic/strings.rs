//! `string` operators and methods (section 13): the compiler emits length,
//! indexing, and iteration inline; joining, comparing, and copying call the runtime.

use crate::abi::modern as rt;
use super::*;

impl FunctionCompiler<'_> {
    /// `+` joins; a comparison orders by bytes.
    pub(super) fn string_binary(
        &mut self,
        operation: BinaryOp,
        left: TypedOperand,
        right: TypedOperand,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        if right.type_name != TypeName::String {
            return Err(type_mismatch(span, TypeName::String, right.type_name));
        }
        let (left, right) = (required(left, span)?, required(right, span)?);
        if operation == BinaryOp::Add {
            let joined = self
                .emit_builtin(rt::TEXT_CONCAT, vec![left, right])
                .expect("a string");
            let joined = self.temporary_owned(joined, TypeName::String);
            return Ok(TypedOperand {
                operand: Some(joined),
                type_name: TypeName::String,
            });
        }
        let compare = comparison(operation)
            .ok_or_else(|| Diagnostic::new(span, "strings support '+' and comparisons"))?;
        let order = self
            .emit_builtin(rt::TEXT_COMPARE, vec![left, right])
            .expect("an order");
        self.ordered(compare, order)
    }

    /// A comparison where either side is a `&string` view: both are compared
    /// as views, a string value through a view of it.
    pub(super) fn view_comparison(
        &mut self,
        operation: BinaryOp,
        left: &Expr,
        right: &Expr,
        span: Span,
    ) -> Result<Option<TypedOperand>, Diagnostic> {
        if !self.is_char_view(left) && !self.is_char_view(right) {
            return Ok(None);
        }
        let compare = comparison(operation)
            .ok_or_else(|| Diagnostic::new(span, "string views support comparisons"))?;
        let mut parts = Vec::new();
        for side in [left, right] {
            let descriptor = match self.view_of(side)? {
                Some((descriptor, ..)) => descriptor,
                None => {
                    let element = ElementType::Scalar(TypeName::Char);
                    let pointer = self.types.slice_pointer(element, 1);
                    let (hir::Operand::Value(descriptor), _) =
                        self.value_view(side, element, pointer)?
                    else {
                        unreachable!("a view is a value")
                    };
                    descriptor
                }
            };
            parts.extend(self.view_parts(descriptor));
        }
        let order = self
            .emit_builtin(rt::VIEW_COMPARE, parts)
            .expect("an order");
        self.ordered(compare, order).map(Some)
    }

    /// Whether the runtime's -1, 0, or 1 `order` satisfies `compare`.
    fn ordered(
        &mut self,
        compare: &'static str,
        order: hir::Operand,
    ) -> Result<TypedOperand, Diagnostic> {
        let result = self.value(TypeName::Bool);
        self.emit(
            compare,
            vec![result],
            vec![order, hir::Operand::Constant(I8, 0)],
            None,
        );
        Ok(TypedOperand {
            operand: Some(hir::Operand::Value(result)),
            type_name: TypeName::Bool,
        })
    }

    /// `s.append(more)` and `s.copy()`; `None` for any other method.
    pub(super) fn string_method(
        &mut self,
        receiver: &Expr,
        name: &str,
        arguments: &[Expr],
        span: Span,
    ) -> Result<Option<TypedOperand>, Diagnostic> {
        if let (true, "copy", []) = (self.is_char_view(receiver), name, arguments) {
            let (descriptor, ..) = self.view_of(receiver)?.expect("a view");
            let parts = self.view_parts(descriptor).to_vec();
            let copy = self.emit_builtin(rt::VIEW_COPY, parts).expect("a string");
            let copy = self.temporary_owned(copy, TypeName::String);
            return Ok(Some(TypedOperand {
                operand: Some(copy),
                type_name: TypeName::String,
            }));
        }
        if self.expression_type_hint(receiver) != Some(TypeName::String) {
            return Ok(None);
        }
        match (name, arguments) {
            ("copy", []) => {
                let text = self.coerced(receiver, TypeName::String)?;
                let copy = self
                    .emit_builtin(
                        rt::BUFFER_CLONE,
                        vec![required(text, span)?, hir::Operand::Constant(U16, 1)],
                    )
                    .expect("a string");
                let copy = self.temporary_owned(copy, TypeName::String);
                Ok(Some(TypedOperand {
                    operand: Some(copy),
                    type_name: TypeName::String,
                }))
            }
            ("append", [more]) => {
                let place = self.sequence_place(receiver, span)?;
                let more = self.coerced(more, TypeName::String)?;
                let current = self.value(TypeName::String);
                self.emit("load", vec![current], vec![place.clone()], None);
                let grown = self
                    .emit_builtin(
                        rt::TEXT_APPEND,
                        vec![hir::Operand::Value(current), required(more, span)?],
                    )
                    .expect("a string");
                self.emit("store", Vec::new(), vec![place, grown], None);
                Ok(Some(TypedOperand {
                    operand: None,
                    type_name: TypeName::Void,
                }))
            }
            ("copy" | "append", _) => Err(Diagnostic::new(
                span,
                format!("wrong arguments to string.{name}"),
            )),
            _ => Ok(None),
        }
    }

    /// A `&string` view's far data pointer and length, as the runtime takes them.
    pub(super) fn view_parts(&mut self, descriptor: u32) -> [hir::Operand; 2] {
        let (data, length) = self.view_parts_of(descriptor, ElementType::Scalar(TypeName::Char));
        [hir::Operand::Value(data), length]
    }

    /// A one-dimensional view's far data pointer and length.
    pub(super) fn view_parts_of(
        &mut self,
        descriptor: u32,
        element: ElementType,
    ) -> (u32, hir::Operand) {
        let data = self.slice_data_pointer(descriptor, element, 1);
        let length = self.value(TypeName::U16);
        let place = hir::Operand::IndirectPlace {
            base: descriptor,
            offset: 0,
            type_id: U16,
            inbounds: false,
        };
        self.emit("load", vec![length], vec![place], None);
        (data, hir::Operand::Value(length))
    }

    /// `s[i]` as a destination: `s` is first made a writable heap copy if it may not be.
    pub(super) fn string_element_target(
        &mut self,
        name: &str,
        indices: &[Expr],
        span: Span,
    ) -> Result<AssignmentPlace, Diagnostic> {
        let place = self.sequence_place(&Expr::Name(name.to_owned(), span), span)?;
        let current = self.value(TypeName::String);
        self.emit("load", vec![current], vec![place.clone()], None);
        let owned = self
            .emit_builtin(
                rt::BUFFER_RESERVE,
                vec![
                    hir::Operand::Value(current),
                    hir::Operand::Constant(U16, 0),
                    hir::Operand::Constant(U16, 1),
                ],
            )
            .expect("a string");
        self.emit("store", Vec::new(), vec![place, owned.clone()], None);
        let hir::Operand::Value(owned) = owned else {
            unreachable!("a call result is a value")
        };
        let element =
            self.sequence_element_at(owned, ElementType::Scalar(TypeName::Char), indices, span)?;
        Ok(AssignmentPlace::Scalar(
            element.operand(CHAR),
            TypeName::Char,
        ))
    }

    /// Where `text[index]` is.
    /// Element `indices[0]` of the string or vec `pointer` points to.
    pub(super) fn sequence_element_at(
        &mut self,
        pointer: u32,
        element: ElementType,
        indices: &[Expr],
        span: Span,
    ) -> Result<ElementAt, Diagnostic> {
        let [index] = indices else {
            return Err(Diagnostic::new(span, "a string or vec has one index"));
        };
        let index = self.coerced(index, TypeName::U16)?;
        let length = self.length(pointer);
        self.check_bounds(&required(index.clone(), span)?, length, span)?;
        let width = self.types.width(element.id());
        Ok(ElementAt::Pointer(self.indexed_pointer(
            pointer,
            required(index, span)?,
            width,
            span,
        )?))
    }

}

/// The HIR comparison `operation` is, if it is one.
fn comparison(operation: BinaryOp) -> Option<&'static str> {
    Some(match operation {
        BinaryOp::Equal => "eq",
        BinaryOp::NotEqual => "ne",
        BinaryOp::Less => "lt",
        BinaryOp::LessEqual => "le",
        BinaryOp::Greater => "gt",
        BinaryOp::GreaterEqual => "ge",
        _ => return None,
    })
}
