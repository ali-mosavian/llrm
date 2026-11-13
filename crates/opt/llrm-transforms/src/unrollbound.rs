//! The most trips a loop can make, from the size of the object it accesses.
//!
//! A load or store outside the object its pointer is based on is undefined. If
//! one runs on every trip of a loop, and its address is `base + start + step *
//! trip` with a positive constant step, the loop cannot make more trips than
//! fit in the object. gcc reads the same bound from the extents of C arrays
//! (`infer_loop_bounds_from_array`); this reads the extent of the whole object
//! (a global, a static alloca), which LLVM IR guarantees where an inner array
//! extent it does not.
//!
//! # Deviations
//! - OB1 The pin has no such bound: `getSmallConstantMaxTripCount` is
//!   `getConstantMaxBackedgeTakenCount` alone (pin :8419). Beyond it, read only
//!   by the unroller, and only where the target asks
//!   (`Tti::unroll_object_bound`). Owner w12, exit 2026-11-27.
//!
//! # Example
//! `B[k][j]` with `double B[12][12]`, `k` counting from 0 and `j >= 0`, is
//! `@B + 8*j + 96*k`: the loop over `k` makes at most 12 trips.

use llrm_analysis::scev::expr::{Use, flags};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::module::BlockId;
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::objectsize::{ObjectSizeOpts, get_base_object_size};

/// The most trips of the loop headed by `header` that its accesses allow, or
/// none where no access bounds it.
pub fn object_max_trip_count(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: BlockId,
) -> Option<u32> {
    let lp = cx.loops.get_loop_for(header)?;
    let latch = lp.get_loop_latch(cx.function)?;
    let exiting = lp.get_exiting_block(cx.function);
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
    let mut best: Option<i128> = None;
    for &block in lp.get_blocks() {
        // Run on every trip that goes round again.
        if !cx.tree.dominates(block, latch) {
            continue;
        }
        // And on the trip that leaves, where the one exit is after it.
        let on_last_trip = exiting.is_some_and(|exit| cx.tree.dominates(block, exit));
        for inst in cx.function.iter_block(block) {
            let operation = cx.function.instruction(inst);
            let (ptr, ty) = match &operation.opcode {
                Opcode::Load { .. } => (operation.operands[0], operation.ty),
                Opcode::Store { .. } => (operation.operands[1], q.type_of(operation.operands[0])?),
                _ => continue,
            };
            if !se.is_value_scevable(cx, ptr) {
                continue;
            }
            let size = i128::from(cx.layout.store_size(&cx.context.types, ty));
            let address = se.get_scev(cx, ptr);
            let base = se.get_pointer_base(address);
            llrm_support::debug!("unrollbound", "access {inst:?}: base unknown {:?}", se.unknown_value(base));
            let Some(object) = se.unknown_value(base).and_then(|base| get_base_object_size(&q, base, opts)) else {
                continue;
            };
            let offset = se.get_minus_scev(cx, Use::of(address), Use::of(base), 0, 0);
            llrm_support::debug!(
                "unrollbound",
                "access {inst:?}: object {object} size {size} offset header {:?} ops {} flags {}",
                se.addrec_header(offset),
                se.operands(offset).len(),
                se.get_no_wrap_flags(offset, flags::NUW | flags::NSW)
            );
            if se.addrec_header(offset) != Some(header) || se.operands(offset).len() != 2 {
                continue;
            }
            // The offset grows without wrapping, so trip t is `start + step *
            // t`.
            if !grows_without_wrapping(se, address, header) {
                continue;
            }
            let step = se.operands(offset)[1].scev;
            let Some(step) = se.as_constant(step).map(|step| step.sext_value()).filter(|&step| step > 0) else {
                continue;
            };
            let start = se.addrec_start(offset);
            let start = se.get_signed_range_min(cx, start).sext_value();
            let room = i128::from(object) - size - start;
            if room < 0 {
                continue;
            }
            // Trips 0..=last fit; a block that is on the way round again ran
            // `trips - 1` times, and on the last trip as well when it is before
            // the exit.
            let last = room / step;
            let trips = last + if on_last_trip { 1 } else { 2 };
            best = Some(best.map_or(trips, |one| one.min(trips)));
        }
    }
    best.and_then(|trips| u32::try_from(trips).ok()).filter(|&trips| trips > 0)
}

/// Whether the add recurrence of `header` in the address `s` (itself, or an
/// operand of the sum it is in) is known not to wrap: the subtraction of the
/// base drops the flags, which the address's own recurrence has.
fn grows_without_wrapping(
    se: &ScalarEvolution,
    s: llrm_analysis::scev::expr::Scev,
    header: BlockId,
) -> bool {
    let wraps_not =
        |one| se.addrec_header(one) == Some(header) && se.get_no_wrap_flags(one, flags::NUW | flags::NSW) != 0;
    wraps_not(s) || se.operands(s).iter().any(|one| wraps_not(one.scev))
}
