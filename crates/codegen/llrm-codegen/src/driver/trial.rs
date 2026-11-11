//! DEVIATE UNROLL-O1-TRIAL (beyond the pin; owner x86-m32-c, exit 2026-12-05): a module compiled
//! again with the loops the -O1 unroll rule refused unrolled, kept only if it is no larger and
//! runs no more work.
//!
//! # How it works
//! At -O1 a target that states a trial size (`Tti::unroll_trial_threshold`) refuses, in the
//! pass, any full unroll that grows the loop by the analysis' count (`unroll_never_grows_at_o1`),
//! and notes the function. If the compile noted any, the module is compiled again with every
//! loop under the trial size unrolled by the pin's threshold, and `keeps_at_o1` decides between
//! the two on exact numbers: the caller's object bytes and the expected executed work of the
//! finished bodies. The static rule cannot see what an unroll frees: the rolled 3x3 nest of
//! x_life spills to the stack, the unrolled one does not (-15 B, -62% clk), while matmul's
//! (+1601 B) is put back. A module with no refused loop is compiled once.

use llrm_transforms::loopunrollpass::trial;

use super::{Options, compiled_module};
use crate::backend::{executed, masm};

/// `module` compiled as `compiled_module` does, then, at -O1 on a target with a trial size and a
/// refused loop, again with the trial unroll: whichever of the two `keeps_at_o1` prefers. `bytes`
/// is the size of a compiled module as the caller will write it; `None` leaves the first compile.
pub fn compiled_module_trying(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    bytes: &dyn Fn(&masm::Module) -> Option<i64>,
) -> Result<masm::Module, String> {
    let limit = options
        .pipeline
        .at_o1()
        .then(|| options.cpu().ok().and_then(|cpu| cpu.target().tti()).and_then(|tti| tti.unroll_trial_threshold()))
        .flatten();
    let Some(limit) = limit else { return compiled_module(module, name, options) };
    let _ = trial::take();
    let kept = compiled_module(module.clone(), name, options)?;
    if trial::take().0.is_empty() {
        return Ok(kept);
    }
    let mut again = options.clone();
    again.dump = None;
    let other = trial::with(limit, || compiled_module(module, name, &again))?;
    let price = |built: &masm::Module| -> Option<(i64, f64)> {
        let work = built.procedures.iter().map(|one| executed::work(&one.body)).sum::<Option<f64>>()?;
        Some((bytes(built)?, work))
    };
    let (Some((kept_bytes, kept_work)), Some((other_bytes, other_work))) = (price(&kept), price(&other)) else {
        return Ok(kept);
    };
    let (added, saved) = (other_bytes - kept_bytes, (kept_work - other_work).round() as i64);
    let pays = trial::keeps_at_o1(saved, added);
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial unroll: {added:+} bytes, {saved:+} work saved: {}", if pays { "kept" } else { "put back" });
    }
    Ok(if pays { other } else { kept })
}
