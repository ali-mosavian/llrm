//! Whether every access of a load in a loop stays inside memory the pointer's
//! base makes dereferenceable (`isDereferenceableAndAlignedInLoop`).
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `Loads.cpp`
//! (`isDereferenceableAndAlignedInLoop`, `isReadOnlyLoop`). A pointer that is
//! an affine add recurrence of the loop with a constant step no smaller than
//! the access walks `[start, end)` over the loop's symbolic-maximum trips; the
//! access is safe when that span, from a base of known dereferenceable size
//! and enough alignment, fits in it. A loop-invariant pointer is safe when its
//! own bytes are.
//!
//! # Example
//! `a[i]` over `i < 1200` with `@a = [1200 x i8]`: start `@a`, end `@a + 1200`,
//! span 1200 <= the 1200 bytes of `@a`: safe, so a vector of 16 may load it
//! before the early exit decides.
//!
//! # DEVIATE LD1 (no operand bundles, no predicates)
//! The pin's retained-knowledge path
//! (`isDereferenceableAndAlignedPointerViaAssumption`) and the SCEV predicates
//! are not translated: a base is dereferenceable by its own attributes or
//! object size only. Exit: `llvm.assume` bundles in MIR.

use llrm_mir::module::{BlockId, Operand};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::known::{pointer_alignment, pointer_dereferenceable_bytes};

use crate::runtimecheck::get_start_and_end_for_access_sized;
use crate::scev::backedge::ExitCountKind;
use crate::scev::expr::{Kind, Scev, Ty, Use};
use crate::scev::guards::LoopGuards;
use crate::scev::{Cx, ScalarEvolution};

fn query<'a>(cx: &Cx<'a>) -> Query<'a> {
    Query {
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
    }
}

/// `isDereferenceableAndAlignedInLoop(LI, L, ...)` for a load of `access_ty`
/// at `ptr` with alignment `align`, in the loop headed by `header`.
pub fn is_dereferenceable_and_aligned_in_loop(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: BlockId,
    ptr: Operand,
    access_ty: TypeId,
    align: u64,
) -> bool {
    if !se.is_value_scevable(cx, ptr) {
        return false;
    }
    let q = query(cx);
    let elt_size = cx.layout.store_size(&cx.context.types, access_ty);
    let ptr_scev = se.get_scev(cx, ptr);
    // A uniform address: safe if its own bytes are.
    if se.is_loop_invariant(cx, ptr_scev, Some(header)) {
        let (bytes, _) = pointer_dereferenceable_bytes(&q, ptr);
        return bytes >= elt_size && pointer_alignment(&q, ptr) >= align;
    }
    let Kind::AddRec(_, in_loop) = se.node(ptr_scev).kind else { return false };
    if !se.is_affine(ptr_scev) || in_loop != header {
        return false;
    }
    let step = se.get_step_recurrence(cx, ptr_scev);
    let Some(step) = se.match_apint(step) else { return false };
    // The access size is a multiple of the alignment, and no wider than the
    // step.
    if elt_size % align.max(1) != 0 || u128::from(elt_size) > step.sext_value().unsigned_abs() {
        return false;
    }
    let max_btc = se.get_backedge_taken_count(cx, header, ExitCountKind::SymbolicMaximum);
    let btc = se.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
    if se.is_could_not_compute(max_btc) {
        return false;
    }
    let size = se.get_constant_of_type(cx, ptr_ty_index(se, cx, ptr_scev), i128::from(elt_size));
    let mut guards = None;
    let (start, end) =
        get_start_and_end_for_access_sized(se, cx, header, ptr_scev, size, btc, max_btc, None, &mut guards);
    if se.is_could_not_compute(start) || se.is_could_not_compute(end) {
        return false;
    }
    let diff = se.get_minus_scev(cx, Use::of(end), Use::of(start), 0, 0);
    if se.is_could_not_compute(diff) {
        return false;
    }
    let guards = LoopGuards::collect(se, cx, header);
    let guarded = se.apply_loop_guards_with(cx, diff, &guards);
    let span = se.get_unsigned_range_max(cx, guarded).zext_value();
    // The base, and a constant offset from it.
    let (base, offset) = match se.node(start).kind.clone() {
        Kind::Unknown(..) => (start, 0u128),
        Kind::Add(operands) if operands.len() == 2 => {
            let Some(offset) = se.match_apint(operands[0].scev) else { return false };
            if offset.is_negative() || u64::try_from(offset.zext_value()).is_err() {
                return false;
            }
            (operands[1].scev, offset.zext_value())
        }
        _ => return false,
    };
    let Some(base) = se.unknown_value(base) else { return false };
    if offset % u128::from(align.max(1)) != 0 {
        return false;
    }
    let (bytes, _) = pointer_dereferenceable_bytes(&q, base);
    pointer_alignment(&q, base) >= align && u128::from(bytes) >= span + offset
}

fn ptr_ty_index(
    se: &ScalarEvolution,
    cx: &Cx,
    ptr_scev: Scev,
) -> Ty {
    let Ty::Ptr(space) = se.type_of(ptr_scev) else { unreachable!("a pointer") };
    Ty::Int(cx.layout.pointer(space).index_bits)
}

#[cfg(test)]
#[path = "loopderef_tests.rs"]
mod tests;
