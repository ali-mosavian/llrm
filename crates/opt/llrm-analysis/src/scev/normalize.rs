//! `ScalarEvolutionNormalization.cpp`: an expression with its add
//! recurrences decremented, or incremented again, with respect to a set of
//! loops.
//!
//! # How it works
//! Normalizing `{S(n-1),+,S(n-2),+,...,+,S0}` subtracts the normalized step
//! recurrence from each operand, starting from the least significant, so the
//! result is the recurrence one trip earlier; denormalizing adds each operand
//! to the next (`getPostIncExpr`). Only add recurrences of the given loops
//! change; the rest of the expression is rebuilt around them.
//!
//! # Example
//! `{%a,+,%b}<h>` normalized for `h` is `{(%a + (-1 * %b)),+,%b}<h>`.
//!
//! # Used by
//! The expander's post-increment mode.
//!
//! # See also
//! `ScalarEvolutionNormalization.h`.

use llrm_mir::hash::HashMap;
use llrm_mir::module::BlockId;

use super::expr::{Kind, Scev, Use};
use super::{Cx, Flags, ScalarEvolution};

/// `TransformKind`.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Transform {
    Normalize,
    Denormalize,
}

impl ScalarEvolution {
    /// `NormalizeDenormalizeRewriter::visit`.
    fn normalize_denormalize(
        &mut self,
        cx: &Cx,
        s: Scev,
        kind: Transform,
        pred: &dyn Fn(&ScalarEvolution, Scev) -> bool,
        memo: &mut HashMap<Scev, Scev>,
    ) -> Scev {
        if let Some(&done) = memo.get(&s) {
            return done;
        }
        let rewritten = match self.node(s).kind.clone() {
            Kind::AddRec(ops, header) => {
                let mut operands: Vec<Scev> =
                    ops.iter().map(|op| self.normalize_denormalize(cx, op.scev, kind, pred, memo)).collect();
                if pred(self, s) {
                    if kind == Transform::Denormalize {
                        // The partial increment, as `getPostIncExpr`.
                        for i in 0..operands.len() - 1 {
                            operands[i] = self
                                .get_add_expr(
                                    cx,
                                    vec![Use::of(operands[i]), Use::of(operands[i + 1])],
                                    Flags::default(),
                                    0,
                                )
                                .scev;
                        }
                    } else {
                        // The step of the very expression being computed, so
                        // from the least significant operand up.
                        for i in (0..operands.len() - 1).rev() {
                            operands[i] = self.get_minus_scev(cx, Use::of(operands[i]), Use::of(operands[i + 1]), 0, 0);
                        }
                    }
                }
                self.get_add_rec_expr(cx, operands.into_iter().map(Use::of).collect(), header, Flags::default()).scev
            }
            _ => {
                let memo_cell = std::cell::RefCell::new(&mut *memo);
                self.rewrite_operands(cx, s, &mut |se, op| {
                    se.normalize_denormalize(cx, op, kind, pred, &mut memo_cell.borrow_mut())
                })
            }
        };
        memo.insert(s, rewritten);
        rewritten
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolutionNormalization.cpp#L99
    /// `normalizeForPostIncUse`: none where the result is not invertible and
    /// `check_invertible` asks.
    pub fn normalize_for_post_inc_use(
        &mut self,
        cx: &Cx,
        s: Scev,
        loops: &[BlockId],
        check_invertible: bool,
    ) -> Option<Scev> {
        if loops.is_empty() {
            return Some(s);
        }
        let pred =
            |se: &ScalarEvolution, ar: Scev| matches!(se.node(ar).kind, Kind::AddRec(_, l) if loops.contains(&l));
        let normalized = self.normalize_denormalize(cx, s, Transform::Normalize, &pred, &mut HashMap::default());
        let denormalized = self.denormalize_for_post_inc_use(cx, normalized, loops);
        // The normalized expression is not invertible.
        if check_invertible && denormalized != s {
            return None;
        }
        Some(normalized)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolutionNormalization.cpp#L108
    /// `normalizeForPostIncUseIf`: `s` with the add recurrences `pred` accepts
    /// normalized. `pred` sees each recurrence once per visit.
    pub fn normalize_for_post_inc_use_if(
        &mut self,
        cx: &Cx,
        s: Scev,
        pred: &dyn Fn(&ScalarEvolution, Scev) -> bool,
    ) -> Scev {
        self.normalize_denormalize(cx, s, Transform::Normalize, pred, &mut HashMap::default())
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolutionNormalization.cpp#L119
    /// `denormalizeForPostIncUse`.
    pub fn denormalize_for_post_inc_use(
        &mut self,
        cx: &Cx,
        s: Scev,
        loops: &[BlockId],
    ) -> Scev {
        if loops.is_empty() {
            return s;
        }
        let pred =
            |se: &ScalarEvolution, ar: Scev| matches!(se.node(ar).kind, Kind::AddRec(_, l) if loops.contains(&l));
        self.normalize_denormalize(cx, s, Transform::Denormalize, &pred, &mut HashMap::default())
    }
}
