//! `//` rounds toward negative infinity; `%` keeps the dividend's sign.

use super::*;

impl FunctionCompiler<'_> {
    pub(super) fn floor_divide(
        &mut self,
        left: TypedOperand,
        right: TypedOperand,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let type_name = left.type_name;
        if !is_integer(type_name) {
            return Err(Diagnostic::new(
                span,
                "'//' requires integer operands; use '/'",
            ));
        }
        let (left, right) = (required(left, span)?, required(right, span)?);
        let operand = if is_unsigned(type_name) {
            self.binary_value("udiv", type_name, left, right)
        } else {
            self.signed_floor_divide(type_name, left, right, span)?
        };
        Ok(TypedOperand {
            operand: Some(operand),
            type_name,
        })
    }

    fn signed_floor_divide(
        &mut self,
        type_name: TypeName,
        left: hir::Operand,
        right: hir::Operand,
        span: Span,
    ) -> Result<hir::Operand, Diagnostic> {
        // The truncated quotient is one too high when the remainder is
        // nonzero and its sign differs from the divisor's.
        let quotient = self.binary_value("div", type_name, left.clone(), right.clone());
        let remainder = self.binary_value("rem", type_name, left, right.clone());
        let zero = hir::Operand::Constant(type_id(type_name), 0);
        let nonzero = self.binary_value("ne", TypeName::Bool, remainder.clone(), zero.clone());
        let signs = self.binary_value("xor", type_name, remainder, right);
        let differ = self.binary_value("lt", TypeName::Bool, signs, zero);
        let both = self.binary_value("and", TypeName::Bool, nonzero, differ);
        let adjust = self.converted(TypedOperand { operand: Some(both), type_name: TypeName::Bool }, type_name, span)?;
        Ok(self.binary_value("sub", type_name, quotient, required(adjust, span)?))
    }

    fn binary_value(
        &mut self,
        op: &'static str,
        type_name: TypeName,
        left: hir::Operand,
        right: hir::Operand,
    ) -> hir::Operand {
        let result = self.value(type_name);
        self.emit(op, vec![result], vec![left, right], None);
        hir::Operand::Value(result)
    }
}
