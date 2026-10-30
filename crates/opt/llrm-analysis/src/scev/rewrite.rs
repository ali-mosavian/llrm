//! `SCEVRewriteVisitor`: rebuild an expression with its operands rewritten,
//! and the `SCEVCastSinkingRewriter` that `getPtrToAddrExpr` runs.

use llrm_mir::context::ConstantKind;
use llrm_mir::hash::HashMap;
use llrm_mir::module::Operand;

use super::expr::{Kind, Scev, Ty, Type, Use};
use super::{Cx, Flags, ScalarEvolution};

impl ScalarEvolution {
    /// `SCEVRewriteVisitor`'s default `visit*Expr` for `s`: rewrite each
    /// operand with `visit` and rebuild the expression with the folders if
    /// any changed. The leaves (`visitConstant`, `visitUnknown`) are `s`.
    pub fn rewrite_operands(
        &mut self,
        cx: &Cx,
        s: Scev,
        visit: &mut dyn FnMut(&mut Self, Scev) -> Scev,
    ) -> Scev {
        let kind = self.node(s).kind.clone();
        match &kind {
            Kind::Constant(_) | Kind::Unknown(_) => s,
            Kind::CouldNotCompute => unreachable!("Invalid use of SCEVCouldNotCompute!"),
            Kind::PtrToAddr(op) => {
                let (op, operand) = (*op, visit(self, op.scev));
                if operand == op.scev { s } else { self.get_ptr_to_addr_expr(cx, operand) }
            }
            Kind::Truncate(op) => {
                let (op, operand) = (*op, visit(self, op.scev));
                if operand == op.scev { s } else { self.get_truncate_expr(cx, Use::of(operand), self.type_of(s), 0) }
            }
            Kind::ZeroExtend(op) => {
                let (op, operand) = (*op, visit(self, op.scev));
                if operand == op.scev { s } else { self.get_zero_extend_expr(cx, Use::of(operand), self.type_of(s), 0) }
            }
            Kind::SignExtend(op) => {
                let (op, operand) = (*op, visit(self, op.scev));
                if operand == op.scev { s } else { self.get_sign_extend_expr(cx, Use::of(operand), self.type_of(s), 0) }
            }
            Kind::UDiv(lhs, rhs) => {
                let (new_lhs, new_rhs) = (visit(self, lhs.scev), visit(self, rhs.scev));
                if new_lhs == lhs.scev && new_rhs == rhs.scev {
                    s
                } else {
                    self.get_udiv_expr(cx, Use::of(new_lhs), Use::of(new_rhs))
                }
            }
            Kind::Add(ops)
            | Kind::Mul(ops)
            | Kind::AddRec(ops, _)
            | Kind::SMax(ops)
            | Kind::UMax(ops)
            | Kind::SMin(ops)
            | Kind::UMin(ops)
            | Kind::SequentialUMin(ops) => {
                let mut operands: Vec<Use> = Vec::new();
                let mut changed = false;
                for op in ops {
                    let new_op = visit(self, op.scev);
                    changed |= op.scev != new_op;
                    operands.push(Use::of(new_op));
                }
                if !changed {
                    return s;
                }
                match &kind {
                    Kind::Add(_) => self.get_add_expr(cx, operands, Flags::default(), 0).scev,
                    Kind::Mul(_) => self.get_mul_expr(cx, operands, Flags::default(), 0).scev,
                    Kind::AddRec(_, header) => {
                        let node_flags = self.get_no_wrap_flags(s, super::expr::flags::MASK);
                        self.get_add_rec_expr(cx, operands, *header, Flags::expr(node_flags)).scev
                    }
                    Kind::SMax(_) => self.get_smax_expr(cx, operands),
                    Kind::UMax(_) => self.get_umax_expr(cx, operands),
                    Kind::SMin(_) => self.get_smin_expr(cx, operands),
                    Kind::UMin(_) => self.get_umin_expr(cx, operands, false),
                    Kind::SequentialUMin(_) => self.get_umin_expr(cx, operands, true),
                    _ => unreachable!(),
                }
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1034
    /// `SCEVCastSinkingRewriter::visit`: rewrite the pointer-typed expression
    /// `s` so that all the computations are done on integers, and the only
    /// pointer-typed operands are unknowns, each turned into an integer by
    /// `create_ptr_cast`.
    fn sink_cast(
        &mut self,
        cx: &Cx,
        s: Scev,
        target_ty: Ty,
        memo: &mut HashMap<Scev, Scev>,
        create_ptr_cast: &mut dyn FnMut(&mut Self, Scev) -> Scev,
    ) -> Scev {
        if let Some(&done) = memo.get(&s) {
            return done;
        }
        // If the expression is not pointer-typed, just keep it as-is.
        let result = if !self.type_of(s).is_pointer() {
            s
        } else {
            match self.node(s).kind.clone() {
                Kind::Add(ops) => {
                    // Preserve wrap flags on rewritten SCEVAddExpr, which the
                    // default implementation drops.
                    let mut operands: Vec<Use> = Vec::new();
                    let mut changed = false;
                    for op in &ops {
                        let new_op = self.sink_cast(cx, op.scev, target_ty, memo, create_ptr_cast);
                        changed |= op.scev != new_op;
                        operands.push(Use::of(new_op));
                    }
                    if !changed {
                        s
                    } else {
                        let node_flags = self.get_no_wrap_flags(s, super::expr::flags::MASK);
                        self.get_add_expr(cx, operands, Flags::expr(node_flags), 0).scev
                    }
                }
                Kind::Unknown(value) => {
                    debug_assert!(self.type_of(s).is_pointer(), "Should only reach pointer-typed SCEVUnknown's.");
                    // Perform some basic constant folding. If the operand of
                    // the cast is a null pointer, don't create a cast SCEV
                    // expression (that will be left as-is), but produce a zero
                    // constant.
                    if let Operand::Constant(id) = value
                        && matches!(cx.context.get(id).kind, ConstantKind::Null)
                    {
                        self.get_zero(cx, target_ty)
                    } else {
                        create_ptr_cast(self, s)
                    }
                }
                _ => {
                    let mut recurse = |se: &mut Self, op: Scev| se.sink_cast(cx, op, target_ty, memo, create_ptr_cast);
                    self.rewrite_operands(cx, s, &mut recurse)
                }
            }
        };
        memo.insert(s, result);
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1080
    /// `ScalarEvolution::getPtrToAddrExpr`.
    pub fn get_ptr_to_addr_expr(
        &mut self,
        cx: &Cx,
        op: Scev,
    ) -> Scev {
        debug_assert!(self.type_of(op).is_pointer(), "Op must be a pointer");

        // The address type of a pointer is its index type here (the offset
        // arithmetic wraps at it): `DL.getAddressType`.
        let Ty::Ptr(space) = self.type_of(op) else { unreachable!() };
        // DEVIATE: a huge pointer's offset carries into its selector, so its
        // bits are no address to subtract.
        if cx.layout.has_unstable_representation(space) {
            return self.get_could_not_compute();
        }
        let ty = Ty::Int(cx.layout.pointer(space).index_bits);

        // Use the rewriter to sink the cast down to SCEVUnknown leaves. The
        // rewriter handles null pointer constant folding.
        let mut memo = HashMap::default();
        let int_op = self.sink_cast(cx, op, ty, &mut memo, &mut |se, u| {
            let Kind::Unknown(_) = se.node(u).kind else { unreachable!("an unknown") };
            let one = [Use::of(u)];
            let key = super::Key {
                tag: Type::PtrToAddr,
                ops: std::borrow::Cow::Borrowed(&one),
                ty: Some(ty),
                constant: None,
                value: None,
                header: None,
            };
            let size = se.compute_expression_size(&[Use::of(u)]);
            se.intern(key, || Kind::PtrToAddr(Use::of(u)), Some(ty), size)
        });
        debug_assert!(
            self.type_of(int_op).is_integer(),
            "We must have succeeded in sinking the cast, and ending up with an integer-typed expression!"
        );
        int_op
    }
}
