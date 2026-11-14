//! DEVIATE LM2 (beyond the pin; owner w11, exit 2026-11-24; target opt-in `Tti::licm_range_deref`): a load through a
//! variable index into an object of known size is dereferenceable when SCEV
//! bounds the offset inside it.
//!
//! The pin's `isDereferenceableAndAlignedPointer` reads constant offsets only,
//! and `isDereferenceableAndAlignedInLoop` (SCEV) only a pointer that is an
//! add recurrence of the loop being asked. A pointer that is invariant in the
//! inner loop but runs with an outer loop's counter (`pos_x[body]` in an
//! `other` loop, `body < 6`) is neither, so a guarded load of it is not
//! hoisted. gcc hoists it (`-ftree-loop-im` with VRP ranges).
//!
//! # How it works
//! `base + offset`: the offset's signed range must lie in `0..=size - bytes`.

use llrm_analysis::scev::expr::Use;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::objectsize::{ObjectSizeOpts, get_base_object_size};

/// Whether `bytes` bytes at `pointer` lie inside the object it is based on, for
/// every value its offset takes.
pub fn dereferenceable_by_range(
    se: &mut ScalarEvolution,
    cx: &Cx,
    pointer: Operand,
    bytes: u64,
) -> bool {
    if !se.is_value_scevable(cx, pointer) {
        return false;
    }
    let q = Query {
        context: cx.context,
        globals: cx.globals,
        metadata: cx.metadata,
        function: cx.function,
        layout: cx.layout,
        dt: Some(cx.tree),
        assumptions: Some(cx.assumptions),
        dom_conditions: None,
        cond_context: None,
        cxt: None,
        pending_intrinsics: &[],
    };
    let opts = ObjectSizeOpts { round_to_align: false, null_is_unknown_size: true };
    let address = se.get_scev(cx, pointer);
    let base = se.get_pointer_base(address);
    let Some(object) = se.unknown_value(base).and_then(|base| get_base_object_size(&q, base, opts)) else {
        return false;
    };
    let offset = se.get_minus_scev(cx, Use::of(address), Use::of(base), 0, 0);
    let low = se.get_signed_range_min(cx, offset).sext_value();
    let high = se.get_signed_range_max(cx, offset).sext_value();
    low >= 0 && high + i128::from(bytes) <= i128::from(object)
}
