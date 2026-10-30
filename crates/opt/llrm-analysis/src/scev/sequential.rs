//! `getSequentialMinMaxExpr`: `umin_seq`, with the visitor that keeps only the
//! first instance of an operand.

use std::borrow::Cow;

use llrm_mir::hash::HashSet;
use llrm_mir::opcode::IntPredicate;

use super::expr::{Kind, Scev, Type, Use};
use super::{Cx, Key, KeyLike, ScalarEvolution};

/// `SCEVSequentialMinMaxExpr::getEquivalentNonSequentialSCEVType`.
fn equivalent_non_sequential_scev_type(kind: Type) -> Type {
    match kind {
        Type::SequentialUMin => Type::UMin,
        _ => unreachable!("Not a sequential min/max type."),
    }
}

/// `SCEVSequentialMinMaxDeduplicatingVisitor`.
struct Deduplicator {
    /// Must be a sequential min/max expression.
    root_kind: Type,
    /// Non-sequential variant of `root_kind`.
    non_sequential_root_kind: Type,
    seen_ops: HashSet<Scev>,
}

impl Deduplicator {
    fn new(root_kind: Type) -> Self {
        Self {
            root_kind,
            non_sequential_root_kind: equivalent_non_sequential_scev_type(root_kind),
            seen_ops: HashSet::default(),
        }
    }

    /// We can only recurse into the SCEV expression of the same effective type
    /// as the type of our root SCEV expression.
    fn can_recurse_into(
        &self,
        kind: Type,
    ) -> bool {
        self.root_kind == kind || self.non_sequential_root_kind == kind
    }

    /// `visit(const SCEV *)`: `None` is `std::nullopt`, drop the operand.
    fn visit_scev(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        s: Scev,
    ) -> Option<Scev> {
        // Has the whole operand been seen already?
        if !self.seen_ops.insert(s) {
            return None;
        }
        if matches!(
            se.node(s).kind,
            Kind::UMax(_) | Kind::SMax(_) | Kind::UMin(_) | Kind::SMin(_) | Kind::SequentialUMin(_)
        ) {
            let kind = se.scev_type(s);

            if !self.can_recurse_into(kind) {
                return Some(s);
            }

            let operands = se.operands(s).to_vec();
            let mut new_ops: Vec<Use> = Vec::new();
            let changed = self.visit(se, cx, &operands, &mut new_ops);

            if !changed {
                return Some(s);
            }
            if new_ops.is_empty() {
                return None;
            }

            return Some(if matches!(se.node(s).kind, Kind::SequentialUMin(_)) {
                se.get_sequential_min_max_expr(cx, kind, new_ops)
            } else {
                se.get_min_max_expr(cx, kind, new_ops)
            });
        }
        Some(s)
    }

    /// `visit(Kind, OrigOps, NewOps)`: whether anything changed.
    fn visit(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        orig_ops: &[Use],
        new_ops: &mut Vec<Use>,
    ) -> bool {
        let mut changed = false;
        let mut ops: Vec<Use> = Vec::with_capacity(orig_ops.len());

        for op in orig_ops {
            let new_op = self.visit_scev(se, cx, op.scev);
            if new_op != Some(op.scev) {
                changed = true;
            }
            if let Some(new_op) = new_op {
                ops.push(Use::of(new_op));
            }
        }

        if changed {
            *new_ops = ops;
        }
        changed
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4233
    /// `ScalarEvolution::getSequentialMinMaxExpr`.
    pub fn get_sequential_min_max_expr(
        &mut self,
        cx: &Cx,
        kind: Type,
        mut ops: Vec<Use>,
    ) -> Scev {
        debug_assert!(kind == Type::SequentialUMin, "Not a SCEVSequentialMinMaxExpr!");
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
            }
        }

        // Note that SCEVSequentialMinMaxExpr is *NOT* commutative, so we can
        // *NOT* do any kind of sorting of the expressions!

        // Check if we have created the same expression before.
        let key = Key { tag: kind, ops: Cow::Borrowed(&ops), ty: None, constant: None, value: None, header: None };
        if let Some(&s) = self.unique.get(&key as &dyn KeyLike) {
            return s;
        }

        // FIXME: there are *some* simplifications that we can do here.

        // Keep only the first instance of an operand.
        {
            let mut deduplicator = Deduplicator::new(kind);
            let original = ops.clone();
            let changed = deduplicator.visit(self, cx, &original, &mut ops);
            if changed {
                return self.get_sequential_min_max_expr(cx, kind, ops);
            }
        }

        // Check to see if one of the operands is of the same kind. If so,
        // expand its operands onto our operand list, and recurse to simplify.
        {
            let mut idx = 0;
            let mut deleted_any = false;
            while idx < ops.len() {
                if self.scev_type(ops[idx].scev) != kind {
                    idx += 1;
                    continue;
                }
                let smme = ops[idx].scev;
                ops.remove(idx);
                let inner = self.operands(smme).to_vec();
                for (at, op) in inner.into_iter().enumerate() {
                    ops.insert(idx + at, op);
                }
                deleted_any = true;
            }

            if deleted_any {
                return self.get_sequential_min_max_expr(cx, kind, ops);
            }
        }

        let ty = self.type_of(ops[0].scev);
        let saturation_point = match kind {
            Type::SequentialUMin => self.get_zero(cx, ty),
            _ => unreachable!("Not a sequential min/max type."),
        };
        let pred = IntPredicate::Ule;

        for i in 1..ops.len() {
            if !self.is_guaranteed_not_to_cause_ub(cx, ops[i].scev) {
                continue;
            }
            // We can replace %x umin_seq %y with %x umin %y if either:
            //  * %y being poison implies %x is also poison.
            //  * %x cannot be the saturating value (e.g. zero for umin).
            if self.implies_poison(cx, ops[i].scev, ops[i - 1].scev)
                || self.is_known_via_non_recursive_reasoning(cx, IntPredicate::Ne, ops[i - 1].scev, saturation_point)
            {
                let seq_ops = vec![ops[i - 1], ops[i]];
                ops[i - 1] = Use::of(self.get_min_max_expr(cx, equivalent_non_sequential_scev_type(kind), seq_ops));
                ops.remove(i);
                return self.get_sequential_min_max_expr(cx, kind, ops);
            }
            // Fold %x umin_seq %y to %x if %x ule %y.
            // TODO: We might be able to prove the predicate for a later
            // operand.
            if self.is_known_via_non_recursive_reasoning(cx, pred, ops[i - 1].scev, ops[i].scev) {
                ops.remove(i);
                return self.get_sequential_min_max_expr(cx, kind, ops);
            }
        }

        // Okay, it looks like we really DO need an expr. Check to see if we
        // already have one, otherwise create a new one.
        let size = self.compute_expression_size(&ops);
        let key = Key { tag: kind, ops: Cow::Borrowed(&ops), ty: None, constant: None, value: None, header: None };
        self.intern(key, || Kind::SequentialUMin(ops.clone()), Some(ty), size)
    }
}
