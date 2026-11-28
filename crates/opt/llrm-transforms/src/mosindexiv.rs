//! A pointer recurrence whose step and offset fit a byte becomes a base plus a
//! byte counter. Translates llvm-mos's `MOSIndexIV`
//! ([MOSIndexIV.cpp](https://github.com/llvm-mos/llvm-mos/blob/main/llvm/lib/Target/MOS/MOSIndexIV.cpp)).
//!
//! # How it works
//! For each loop, each `getelementptr` whose SCEV is an add recurrence of the
//! loop in address space 0 (`Base + {0,+,Step}`) with the signed range of
//! `Step` and of `{0,+,Step}` both non-negative and below the index width is
//! rewritten to `gep i8 Base, zext(trunc {0,+,Step})`. The byte counter is a
//! dedicated phi; the high byte of the index is zero, so the backend can use
//! the byte-indexed addressing modes.
//!
//! # Example
//! `sum += buf[i]` for `i` in `0..200` over a 300-byte global, 16-bit
//! pointers, `index_iv_width` 8. Before:
//!
//! ```mir
//! loop:
//!   %i = phi i16 [ 0, %entry ], [ %next, %loop ]
//!   %p = getelementptr inbounds i8, ptr @buf, i16 %i
//!   %b = load i8, ptr %p
//!   ...
//!   %next = add nuw nsw i16 %i, 1
//!   %c = icmp ult i16 %next, 200
//! ```
//!
//! After:
//!
//! ```mir
//! loop:
//!   %0 = phi i8 [ %3, %loop ], [ 0, %entry ]
//!   %i = phi i16 [ 0, %entry ], [ %next, %loop ]
//!   %1 = zext i8 %0 to i16
//!   %2 = getelementptr i8, ptr @buf, i16 %1
//!   %b = load i8, ptr %2
//!   ...
//!   %next = add nuw nsw i16 %i, 1
//!   %c = icmp ult i16 %next, 200
//!   %3 = add nuw i8 %0, 1
//! ```
//!
//! The address is the global plus a byte counter, so the backend indexes with
//! `lda buf,x`. A 300-trip walk reaches offset 299 and is left as it was.
//!
//! From unit test `test_a_byte_range_walk_is_a_base_and_a_byte_index` in
//! `mosindexiv_tests.rs`.
//!
//! # Used by
//! [`crate::pipeline`] runs it before the late `indvars`, where
//! `Tti::index_iv_width` is set.
//!
//! # See also
//! [`crate::narrowiv`] (a counter, not an address).

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::{Kind, Ty};
use llrm_analysis::scev::{Cx, Flags as ScevFlags};
use llrm_mir::apint::Word;
use llrm_mir::constantrange::Range;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, Outer, PreservedAnalyses};

/// `LLRM_NO_MOSINDEXIV`, read once; set, the pass is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_MOSINDEXIV")
}

/// `MOSIndexIV`.
pub struct MosIndexIv;

impl FunctionPass for MosIndexIv {
    fn name(&self) -> &'static str {
        "mosindexiv"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("mos-indexiv")
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if llrm_support::env_set("LLRM_EXP_SKIP_RUN") {
            return PreservedAnalyses::all();
        }
        // A session of its own: the expressions made to test a GEP that is not
        // rewritten must not enter the cache the loop passes share, whose
        // numbering orders LSR's ties.
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let changed = !llrm_support::env_set("LLRM_EXP_SKIP_REWRITE")
            && rewritten_in(&mut session, unit.context, unit.layout, unit.function, unit.declared, &outer);
        if changed {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// The loops of `function` rewritten where the target asks
/// (`Tti::index_iv_width`); whether anything changed.
pub fn rewritten_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let Some(width) = outer.target().tti().and_then(|tti| tti.index_iv_width()) else { return false };
    if !enabled() {
        return false;
    }
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = env.loops.get_loops_in_visit_order().iter().map(|one| one.get_header()).collect();
    let mut rewriter = Expander::new("mos-indexiv");
    // `Rewriter.disableCanonicalMode()`: the counters come from the
    // subexpressions already there.
    rewriter.set_canonical_mode(false);
    let mut changed = false;
    for header in headers {
        changed |= rewrite_loop(se, &env, context, layout, function, declared, &mut rewriter, header, width);
    }
    changed
}

/// Whether `range` lies in `0..=2^width - 1` (`InRange`: the upper bound is
/// exclusive, so the largest member the test admits is `2^width - 2`).
fn in_range<W: Word>(
    range: &Range<W>,
    width: u32,
) -> bool {
    range.is_all_non_negative() && range.upper <= W::from_u128((1u128 << width) - 1)
}

/// `MOSIndexIV::run` over the loop at `header`.
#[allow(clippy::too_many_arguments)]
fn rewrite_loop(
    se: &mut ScalarEvolution,
    env: &Env,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    rewriter: &mut Expander,
    header: BlockId,
    width: u32,
) -> bool {
    let geps: Vec<InstId> = {
        let Some(l) = env.loops.get_loop_for(header) else { return false };
        l.get_blocks()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::GetElementPtr { .. }))
            .collect()
    };
    let mut changed = false;
    for gep in geps {
        let Some(plan) = plan(se, env, context, function, header, gep, width) else { continue };
        let ptr_ty = function.instruction(gep).ty;
        let index_ty = context.types.int(layout.pointer(0).index_bits);
        let byte = context.types.int(width);
        rewriter.set_insert_point(gep);
        // The 16-bit base and the byte index.
        let base = rewriter.expand_code_for(env, context, declared, function, se, plan.start, None);
        let index = rewriter.expand_code_for(env, context, declared, function, se, plan.index, None);
        // An "uglygep": no real GEP calculation has to be found that leads to
        // the SCEV, and some aliasing information stays.
        let extended = function.create_instruction(
            Opcode::Cast(CastOp::ZExt),
            index_ty,
            vec![index],
            Flags::default(),
            None,
        );
        function.insert(extended, Position::Before(gep)).expect("a placed extension");
        let extended = Operand::Value(function.instruction(extended).result.expect("a value"));
        let i8 = byte_type(context);
        let uglygep = function.create_instruction(
            Opcode::GetElementPtr { source: i8 },
            ptr_ty,
            vec![base, extended],
            Flags::default(),
            None,
        );
        function.insert(uglygep, Position::Before(gep)).expect("a placed gep");
        let new = function.instruction(uglygep).result.expect("a value");
        let old = function.instruction(gep).result.expect("a value");
        function.replace_all_uses_with(old, Operand::Value(new));
        function.erase(gep).expect("an unused gep");
        let _ = byte;
        changed = true;
    }
    changed
}

/// `i8`, the type the rewritten address is scaled by.
fn byte_type(context: &mut Context) -> llrm_mir::types::TypeId {
    context.types.int(8)
}

/// What to place for one GEP.
struct Plan {
    /// The recurrence's start, the base.
    start: llrm_analysis::scev::expr::Scev,
    /// The byte index: `trunc {0,+,Step}`.
    index: llrm_analysis::scev::expr::Scev,
}

/// The GEP's rewrite, where its recurrence qualifies.
fn plan(
    se: &mut ScalarEvolution,
    env: &Env,
    context: &Context,
    function: &Function,
    header: BlockId,
    gep: InstId,
    width: u32,
) -> Option<Plan> {
    let cx: Cx = env.cx(context, function);
    let value = function.instruction(gep).result?;
    // Only pointer values with an additive recurrence can be made into
    // Base+Index.
    let r = se.get_scev(&cx, Operand::Value(value));
    let Kind::AddRec(operands, rec_header) = se.node(r).kind.clone() else { return None };
    if rec_header != header {
        return None;
    }
    // Only pointers of address space 0 (16-bit on the targets that ask).
    if se.type_of(r) != Ty::Ptr(0) {
        return None;
    }
    // If the step doesn't fit the index, incrementing it takes a wider add, so
    // there's no point to the optimization.
    let step = se.get_step_recurrence(&cx, r);
    if !in_range(&se.get_signed_range(&cx, step), width) {
        return None;
    }
    // The index must itself fit.
    let zero = se.get_zero(&cx, se.type_of(r));
    let step_use = llrm_analysis::scev::expr::Use { scev: step, bits: 0 };
    let start_use = llrm_analysis::scev::expr::Use { scev: zero, bits: 0 };
    let flags = ScevFlags::expr(se.get_no_wrap_flags(r, llrm_analysis::scev::expr::flags::MASK));
    let index = se.get_add_rec_expr_two(&cx, start_use, step_use, header, flags);
    if !in_range(&se.get_signed_range(&cx, index.scev), width) {
        return None;
    }
    // Once the step and index are both known to fit, a 16-bit base plus a byte
    // index always works.
    let narrow = se.get_truncate_expr(&cx, index, Ty::Int(width), 0);
    Some(Plan { start: operands[0].scev, index: narrow })
}

#[cfg(test)]
#[path = "mosindexiv_tests.rs"]
mod tests;
