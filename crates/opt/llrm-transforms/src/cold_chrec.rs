//! DEVIATE COLD-CHREC: a chained recurrence of an enclosing loop that an inner
//! loop's entry skips on some trips is not expanded for the inner loop.
//!
//! `SCEVExpander` makes one phi a level in the outer loop, stepped on every
//! trip, for a start the written code computes where the inner loop is entered
//! (sieve: `i * i` feeding a prime's marking loop, 11 entries in 1022 trips).
//! The pin expands it in `WidenIV` and in LSR; gcc's ivopts weighs a use's cost
//! by the frequency of its block.
//!
//! # Example
//! A start value `i * i` of an inner loop, a recurrence of the outer loop, is
//! left as the written multiply instead of becoming an outer phi stepped on
//! every trip.
//!
//! ```text
//! outer: for i:  if (flag[i]) { for (j = i*i; j < n; j += i) mark(j); }
//! kept:  j.start = mul i, i     ; computed only where the inner loop is entered
//! not:   %sq = phi [0, pre], [%sq.next, latch]   ; stepped on every outer trip
//! ```

use llrm_analysis::scev::expr::{Kind, Scev};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::module::BlockId;

use crate::lsr_use::is_existing_phi;

/// DEVIATE COLD-CHREC (`Tti::avoids_cold_outer_addrec_expansion`, used by
/// `WidenIV` and LSR): whether expanding `s`, a value the loop of header `l`
/// starts from or adds, makes phis for a chained recurrence of an enclosing
/// loop that no phi carries, and some trip of that loop reaches its latch
/// without passing `l`'s header: the phis run on every trip, the start on the
/// trips that enter `l`.
pub(crate) fn expands_cold_outer_chrec(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
    l: BlockId,
) -> bool {
    match se.node(s).kind.clone() {
        Kind::AddRec(operands, header) if header != l => {
            if is_existing_phi(se, cx, s) {
                false
            } else if operands.len() > 2 {
                cx.loops.get_loop_for(header).is_some_and(|outer| {
                    outer.contains(l)
                        && outer.get_loop_latches(cx.function).into_iter().any(|latch| !cx.tree.dominates(l, latch))
                })
            } else {
                operands.iter().any(|op| expands_cold_outer_chrec(se, cx, op.scev, l))
            }
        }
        Kind::Add(operands) | Kind::Mul(operands) | Kind::AddRec(operands, _) => {
            operands.iter().any(|op| expands_cold_outer_chrec(se, cx, op.scev, l))
        }
        Kind::Truncate(op) | Kind::ZeroExtend(op) | Kind::SignExtend(op) => {
            expands_cold_outer_chrec(se, cx, op.scev, l)
        }
        _ => false,
    }
}
