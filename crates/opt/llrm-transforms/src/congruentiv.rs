//! Phis of a loop header that ScalarEvolution proves equal are one phi.
//! LLVM's `SCEVExpander::replaceCongruentIVs`, as `IndVarSimplify::run` calls
//! it after the exit values.
//!
//! # How it works
//! For each loop in LoopSimplify form, innermost first, the header phis are
//! taken from the widest integer to the narrowest. A phi that simplifies to a
//! value, or whose SCEV is a constant, is replaced by it. A phi whose SCEV an
//! earlier phi has is replaced by that one (a truncation of it where it is
//! narrower), and its increment by the other's. A wide counter that truncates
//! free is also the representative of its truncation, so a narrow counter
//! that is the `trunc` of a wide one reads the wide one.
//!
//! # Example
//! `%i = phi i32 [0,..],[%i1,..]` and `%j = phi i16 [0,..],[%j1,..]` with
//! `%i1 = add %i, 1`, `%j1 = add %j, 1`: the readers of `%j` read `trunc i32
//! %i to i16`.
//!
//! # Used by
//! [`crate::indvars`], which leaves the phis it replaced to `dead`.
//!
//! # See also
//! `docs/architecture/scev-port.md`.

use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function};
use llrm_mir::passes::Outer;

/// `LLRM_NO_SCEV_CONGRUENT`, read once; set, `indvars` keeps its own
/// matcher of equal counters.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_CONGRUENT")
}

/// `replaceCongruentIVs` over every loop of `function` in LoopSimplify form,
/// innermost first; whether any phi was eliminated. The phis are left to
/// `dead`.
pub fn replaced(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    replaced_in(&mut session, context, layout, function, outer)
}

/// [`replaced`] over `session`, whose expressions are valid for `function`.
pub fn replaced_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        crate::loopfilter::retain(&mut headers);
        headers
    };
    // `Rewriter.disableCanonicalMode()`.
    let mut rewriter = llrm_analysis::scev::expander::Expander::new("indvars");
    rewriter.set_canonical_mode(false);
    let tti = outer.target().tti();
    let mut dead = Vec::new();
    let mut eliminated = 0;
    for header in headers {
        let simple = {
            let cx = env.cx(&*context, &*function);
            cx.loops.get_loop_for(header).is_some_and(|found| found.is_loop_simplify_form(cx.function))
        };
        if simple {
            eliminated +=
                rewriter.replace_congruent_ivs(&env, context, function, se, header, &mut dead, tti.as_deref());
        }
    }
    eliminated > 0
}

#[cfg(test)]
#[path = "congruentiv_tests.rs"]
mod tests;
