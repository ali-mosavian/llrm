//! Type hints: what an expression is, without compiling it.

use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn member_type_hint(&self, base: &Expr, field: &str, span: Span) -> Option<TypeName> {
        if let Some(read) = self.bit_field_hint(base, field) {
            return Some(read);
        }
        let struct_id = self.struct_type_hint(base, span)?;
        let field = self.types.structure(struct_id)?.fields.get(field)?;
        match field.type_ {
            ElementType::Scalar(type_name) => Some(type_name),
            ElementType::Struct(_) => None,
        }
    }

    pub(super) fn struct_type_hint(&self, expression: &Expr, span: Span) -> Option<u32> {
        match expression {
            Expr::Name(name, _) => match self.binding(name, span).ok()?.type_ {
                BindingType::Struct(id) => id,
                _ => return None,
            },
            Expr::Index { base, .. } => match self.indexed_hint(base)? {
                ElementType::Struct(id) => id,
                ElementType::Scalar(_) => return None,
            },
            Expr::Member { base, field, .. } => {
                let parent = self.struct_type_hint(base, span)?;
                match self.types.structure(parent)?.fields.get(field)?.type_ {
                    ElementType::Struct(id) => id,
                    ElementType::Scalar(_) => return None,
                }
            }
            _ => match self.types.referent(self.expression_type_hint(expression)?)? {
                ElementType::Struct(id) => id,
                ElementType::Scalar(_) => return None,
            },
        }
        .into()
    }

    pub(super) fn expression_type_hint(&self, expression: &Expr) -> Option<TypeName> {
        if self.sequence_property(expression).is_some() {
            return Some(TypeName::U16);
        }
        match expression {
            Expr::Lambda { .. } => None,
            Expr::Float(..) => Some(TypeName::F64),
            Expr::Character(..) => Some(TypeName::Char),
            Expr::String(..) => Some(TypeName::String),
            Expr::Boolean(..) => Some(TypeName::Bool),
            Expr::Name(name, _) if self.visible(name).is_none() => self.function_value_hint(name),
            Expr::Name(name, _) => {
                self.binding(name, expression.span())
                    .ok()
                    .and_then(|one| match one.type_ {
                        BindingType::Scalar(type_name) => Some(type_name),
                        BindingType::Array { .. }
                        | BindingType::Slice { .. }
                        | BindingType::Struct(_)
=> None,
                    })
            }
            Expr::Call { name, .. } => self.known_signature(name).map(|one| one.result),
            Expr::MethodCall { .. } if self.method_as_call(expression).is_some() => {
                self.expression_type_hint(&self.method_as_call(expression).expect("checked"))
            }
            Expr::MethodCall { receiver, name, .. } if name == "copy" => {
                self.expression_type_hint(receiver)
            }
            Expr::MethodCall { .. } if self.emptiness(expression).is_some() => Some(TypeName::Bool),
            Expr::MethodCall { receiver, name, .. } if name == "pop" => self
                .expression_type_hint(receiver)
                .and_then(|one| self.types.sequence_element(one))
                .and_then(|one| match one {
                    ElementType::Scalar(type_name) => Some(type_name),
                    ElementType::Struct(_) => None,
                }),
            Expr::MethodCall { .. } => Some(TypeName::U16),
            Expr::Unary {
                op: UnaryOp::Not, ..
            } => Some(TypeName::Bool),
            Expr::Unary { op: UnaryOp::Deref, operand, .. } => match self.types.raw_target(self.expression_type_hint(operand)?)? {
                ElementType::Scalar(type_name) => Some(type_name),
                ElementType::Struct(_) => None,
            },
            Expr::Unary { operand, .. } => self
                .expression_type_hint(operand)
                .map(|one| self.rules.promoted(one)),
            Expr::Conversion { target, .. } => Some(*target),
            Expr::Index { base, .. } => match self.indexed_hint(base)? {
                ElementType::Scalar(element) => Some(element),
                ElementType::Struct(_) => None,
            },
            Expr::Member { base, field, span } => self.member_type_hint(base, field, *span),
            Expr::Binary {
                op, left, right, ..
            } => {
                if matches!(
                    op,
                    BinaryOp::Equal
                        | BinaryOp::NotEqual
                        | BinaryOp::Less
                        | BinaryOp::LessEqual
                        | BinaryOp::Greater
                        | BinaryOp::GreaterEqual
                        | BinaryOp::Is
                        | BinaryOp::IsNot
                        | BinaryOp::And
                        | BinaryOp::Or
                ) {
                    return Some(TypeName::Bool);
                }
                if is_shift(*op) {
                    return self
                        .expression_type_hint(left)
                        .map(|one| self.rules.promoted(one));
                }
                match (
                    self.expression_type_hint(left),
                    self.expression_type_hint(right),
                ) {
                    (Some(left), Some(right)) => self.rules.common(left, right),
                    (Some(type_name), None) | (None, Some(type_name)) => {
                        Some(self.rules.promoted(type_name))
                    }
                    (None, None) => None,
                }
            }
            Expr::Conditional {
                then, otherwise, ..
            } => self.conditional_type_hint(then, otherwise),
            Expr::Variant {
                enum_name: Some(enum_name),
                ..
            } => self.types.enums.get(enum_name).and_then(|one| one.scalar()),
            Expr::Variant { .. } => None,
            Expr::Integer(..)
            | Expr::NamedArgument { .. }
            | Expr::Try { .. }
            | Expr::FString { .. }
            | Expr::Array(..)
            | Expr::Repeat { .. }
            | Expr::Comprehension { .. }
            | Expr::Generator { .. }
            | Expr::DictComprehension { .. }
            | Expr::Dict(..)
            | Expr::Slice { .. }
            | Expr::StructLiteral { .. }
            | Expr::Tuple(..)
            | Expr::Borrow { .. } => None,
        }
    }
}
