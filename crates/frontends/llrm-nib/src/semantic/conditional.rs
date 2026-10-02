//! `c ? a : b`: only the chosen arm is evaluated.

use super::*;

impl FunctionCompiler<'_> {
    pub(super) fn conditional(
        &mut self,
        condition: &Expr,
        then: &Expr,
        otherwise: &Expr,
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let type_name = expected
            .or_else(|| self.conditional_type_hint(then, otherwise))
            .ok_or_else(|| Diagnostic::new(span, "the arms of '?:' need a known type"))?;
        let condition = self.expression(condition, Some(TypeName::Bool))?;
        let condition = required(condition, span)?;
        let name = self.hidden("conditional");
        let result = self.place(&name, type_name, true);
        let (then_block, otherwise_block, join) = (self.block(), self.block(), self.block());
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![condition],
            targets: vec![then_block, otherwise_block],
        });
        let mut all_static = true;
        for (block, arm) in [(then_block, then), (otherwise_block, otherwise)] {
            self.current = block;
            let value = self.coerced(arm, type_name)?;
            // An owning result owns whichever arm it took.
            all_static &= self.is_static(&value);
            self.consume(&value, arm.span())?;
            self.emit(
                "store",
                Vec::new(),
                vec![hir::Operand::Place(result), required(value, arm.span())?],
                None,
            );
            self.terminate(jump(join));
        }
        self.current = join;
        let value = self.value(type_name);
        self.emit("load", vec![value], vec![hir::Operand::Place(result)], None);
        let operand = if all_static {
            self.origins.insert(value, ownership::Origin::Static);
            hir::Operand::Value(value)
        } else {
            self.temporary_owned(hir::Operand::Value(value), type_name)
        };
        Ok(TypedOperand {
            operand: Some(operand),
            type_name,
        })
    }

    /// `condition ? then : otherwise` of struct or enum `struct_id`: each arm
    /// is built, on its own branch, in one temporary.
    pub(super) fn conditional_view(
        &mut self,
        condition: &Expr,
        then: &Expr,
        otherwise: &Expr,
        struct_id: u32,
        span: Span,
    ) -> Result<StructView, Diagnostic> {
        let condition = self.expression(condition, Some(TypeName::Bool))?;
        let condition = required(condition, span)?;
        let result = self.temporary(struct_id);
        let (then_block, otherwise_block, join) = (self.block(), self.block(), self.block());
        self.terminate(hir::Terminator {
            kind: "branch",
            operands: vec![condition],
            targets: vec![then_block, otherwise_block],
        });
        for (block, arm) in [(then_block, then), (otherwise_block, otherwise)] {
            self.current = block;
            self.store_struct_expression(&result, arm)?;
            self.terminate(jump(join));
        }
        self.current = join;
        Ok(result)
    }

    /// The common type of both arms, as a binary operator would pick it.
    pub(super) fn conditional_type_hint(&self, then: &Expr, otherwise: &Expr) -> Option<TypeName> {
        match (
            self.expression_type_hint(then),
            self.expression_type_hint(otherwise),
        ) {
            (Some(left), Some(right)) if left == right => Some(left),
            (Some(left), Some(right)) => self.rules.common(left, right),
            (Some(one), None) | (None, Some(one)) => Some(one),
            // Two literals take the type each would alone.
            (None, None) => match (self.literal_type(then), self.literal_type(otherwise)) {
                (Some(left), Some(right)) if left == right => Some(left),
                (Some(left), Some(right)) => self.rules.common(left, right),
                _ => None,
            },
        }
    }

    /// The type an integer literal takes where nothing expects one.
    fn literal_type(&self, expression: &Expr) -> Option<TypeName> {
        let value = match expression {
            Expr::Integer(value, _) => *value,
            Expr::Unary {
                op: UnaryOp::Negative,
                operand,
                ..
            } => match operand.as_ref() {
                Expr::Integer(value, _) => -value,
                _ => return None,
            },
            _ => return None,
        };
        self.integer(value, None, expression.span())
            .ok()
            .map(|one| one.type_name)
    }
}
