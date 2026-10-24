//! `getMinMaxExpr` and the `getSMaxExpr`... family.

use std::borrow::Cow;

use llrm_mir::opcode::IntPredicate;

use super::ap::Ap;
use super::expr::{Kind, Scev, Type, Use};
use super::{Cx, Key, KeyLike, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3888
    /// `ScalarEvolution::getMinMaxExpr`.
    pub fn get_min_max_expr(
        &mut self,
        cx: &Cx,
        kind: Type,
        mut ops: Vec<Use>,
    ) -> Scev {
        debug_assert!(matches!(kind, Type::SMax | Type::UMax | Type::SMin | Type::UMin), "Not a SCEVMinMaxExpr!");
        debug_assert!(!ops.is_empty(), "Cannot get empty (u|s)(min|max)!");
        if ops.len() == 1 {
            return ops[0].scev;
        }
        #[cfg(debug_assertions)]
        {
            let ety = self.get_effective_scev_type(cx, self.type_of(ops[0].scev));
            for op in &ops[1..] {
                debug_assert!(
                    self.get_effective_scev_type(cx, self.type_of(op.scev)) == ety,
                    "Operand types don't match!"
                );
                debug_assert!(
                    self.type_of(ops[0].scev).is_pointer() == self.type_of(op.scev).is_pointer(),
                    "min/max should be consistently pointerish"
                );
            }
        }

        let is_signed = matches!(kind, Type::SMax | Type::SMin);
        let is_max = matches!(kind, Type::SMax | Type::UMax);

        let folded = self.constant_fold_and_group_ops(
            cx,
            &mut ops,
            &|c1, c2| match kind {
                Type::SMax => c1.smax(c2),
                Type::SMin => c1.smin(c2),
                Type::UMax => c1.umax(c2),
                Type::UMin => c1.umin(c2),
                _ => unreachable!("Unknown SCEV min/max opcode"),
            },
            &|c| {
                // identity
                if is_max {
                    if is_signed { c.is_signed_min() } else { c.is_zero() }
                } else if is_signed {
                    c == Ap::signed_max(c.width)
                } else {
                    c.is_all_ones()
                }
            },
            &|c| {
                // absorber
                if is_max {
                    if is_signed { c == Ap::signed_max(c.width) } else { c.is_all_ones() }
                } else if is_signed {
                    c.is_signed_min()
                } else {
                    c.is_zero()
                }
            },
        );
        if let Some(folded) = folded {
            return folded;
        }

        // Check if we have created the same expression before.
        let key = Key { tag: kind, ops: Cow::Borrowed(&ops), ty: None, constant: None, value: None, header: None };
        if let Some(&s) = self.unique.get(&key as &dyn KeyLike) {
            return s;
        }

        // Find the first operation of the same kind
        let mut idx = 0;
        while idx < ops.len() && (self.scev_type(ops[idx].scev) as u8) < (kind as u8) {
            idx += 1;
        }

        // Check to see if one of the operands is of the same kind. If so,
        // expand its operands onto our operand list, and recurse to simplify.
        if idx < ops.len() {
            let mut deleted_any = false;
            while idx < ops.len() && self.scev_type(ops[idx].scev) == kind {
                let smme = ops[idx].scev;
                ops.remove(idx);
                ops.extend_from_slice(&self.operands(smme).to_vec());
                deleted_any = true;
            }

            if deleted_any {
                return self.get_min_max_expr(cx, kind, ops);
            }
        }

        // Okay, check to see if the same value occurs in the operand list
        // twice. If so, delete one. Since we sorted the list, these values are
        // required to be adjacent.
        let ge_pred = if is_signed { IntPredicate::Sge } else { IntPredicate::Uge };
        let le_pred = if is_signed { IntPredicate::Sle } else { IntPredicate::Ule };
        let first_pred = if is_max { ge_pred } else { le_pred };
        let second_pred = if is_max { le_pred } else { ge_pred };
        let mut i = 0usize;
        let mut e = ops.len() - 1;
        while i != e {
            if ops[i] == ops[i + 1]
                || self.is_known_via_non_recursive_reasoning(cx, first_pred, ops[i].scev, ops[i + 1].scev)
            {
                //  X op Y op Y  -->  X op Y
                //  X op Y       -->  X, if we know X, Y are ordered
                // appropriately
                ops.remove(i + 1);
                e -= 1;
                if i == 0 {
                    continue;
                }
                i -= 1;
            } else if self.is_known_via_non_recursive_reasoning(cx, second_pred, ops[i].scev, ops[i + 1].scev) {
                //  X op Y       -->  Y, if we know X, Y are ordered
                // appropriately
                ops.remove(i);
                e -= 1;
                if i == 0 {
                    continue;
                }
                i -= 1;
            }
            i += 1;
        }

        if ops.len() == 1 {
            return ops[0].scev;
        }

        debug_assert!(!ops.is_empty(), "Reduced smax down to nothing!");

        // Okay, it looks like we really DO need an expr. Check to see if we
        // already have one, otherwise create a new one.
        let key = Key { tag: kind, ops: Cow::Borrowed(&ops), ty: None, constant: None, value: None, header: None };
        if let Some(&existing) = self.unique.get(&key as &dyn KeyLike) {
            return existing;
        }
        let ty = self.type_of(ops[0].scev);
        let size = self.compute_expression_size(&ops);
        let node = || match kind {
            Type::SMax => Kind::SMax(ops.clone()),
            Type::UMax => Kind::UMax(ops.clone()),
            Type::SMin => Kind::SMin(ops.clone()),
            Type::UMin => Kind::UMin(ops.clone()),
            _ => unreachable!(),
        };
        let s = self.intern(key, node, Some(ty), size);
        // Min and max never overflow
        self.nodes[s.0 as usize].flags |= super::expr::flags::NUW | super::expr::flags::NSW;
        s
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3883
    /// `getAbsExpr`: `smax(Op, -Op)`.
    pub fn get_abs_expr(
        &mut self,
        cx: &Cx,
        op: Scev,
        is_nsw: bool,
    ) -> Scev {
        let flags = if is_nsw { super::expr::flags::NSW } else { super::expr::flags::NONE };
        let negated = self.get_negative_scev(cx, op, flags);
        self.get_smax_expr(cx, vec![Use::of(op), Use::of(negated)])
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4356
    /// `getSMaxExpr`.
    pub fn get_smax_expr(
        &mut self,
        cx: &Cx,
        ops: Vec<Use>,
    ) -> Scev {
        self.get_min_max_expr(cx, Type::SMax, ops)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4365
    /// `getUMaxExpr`.
    pub fn get_umax_expr(
        &mut self,
        cx: &Cx,
        ops: Vec<Use>,
    ) -> Scev {
        self.get_min_max_expr(cx, Type::UMax, ops)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4374
    /// `getSMinExpr`.
    pub fn get_smin_expr(
        &mut self,
        cx: &Cx,
        ops: Vec<Use>,
    ) -> Scev {
        self.get_min_max_expr(cx, Type::SMin, ops)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4383
    /// `getUMinExpr`.
    pub fn get_umin_expr(
        &mut self,
        cx: &Cx,
        ops: Vec<Use>,
        sequential: bool,
    ) -> Scev {
        if sequential {
            return self.get_sequential_min_max_expr(cx, Type::SequentialUMin, ops);
        }
        self.get_min_max_expr(cx, Type::UMin, ops)
    }
}
