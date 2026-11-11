//! DEVIATE UNROLL-O1-TRIAL (beyond the pin; owner x86-m32-c, exit 2026-12-05) and DEVIATE
//! ROTATION-TRIAL (beyond the pin; owner greedy-os-causes, exit 2026-12-05): a module compiled
//! again with one decision of a pass reversed, kept only if the finished object prices better.
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
//!
//! At -Os a target that states `Tti::rotation_trial_at_size` rotates, as RS1 does, a counted loop
//! whose copied guard does not fold, and notes the function. If the compile noted any, the module
//! is compiled again with those loops unrotated (gcc's `-Os` shape) and `keeps_at_size` keeps it
//! only if the object is smaller. What rotating costs or frees is in the passes after it
//! (x_prefix -12 B unrotated, x_transpose +34 B), so the finished code decides.

use llrm_transforms::trial;

use super::{Options, compiled_module};
use crate::backend::{executed, masm};

/// Which decision the second compile reverses.
enum Trial {
    /// Unroll the loops under this size that the -O1 rule refused.
    Unroll(u32),
    /// Leave unrotated the counted loops RS1 rotated.
    Rotation,
}

impl Trial {
    /// The trial `options` asks for, if its target has one.
    fn of(options: &Options) -> Option<Trial> {
        let tti = options.cpu().ok().and_then(|cpu| cpu.target().tti())?;
        if options.pipeline.at_o1() {
            return tti.unroll_trial_threshold().map(Trial::Unroll);
        }
        (options.pipeline.for_size && tti.rotation_guard_must_fold_at_size() && tti.rotation_trial_at_size())
            .then_some(Trial::Rotation)
    }

    /// Whether the compile just made noted a decision to reverse.
    fn noted(&self) -> bool {
        match self {
            Trial::Unroll(_) => !trial::take().0.is_empty(),
            Trial::Rotation => !trial::take_rotated().is_empty(),
        }
    }

    /// Discards what an earlier compile noted.
    fn clear(&self) {
        let _ = trial::take();
        let _ = trial::take_rotated();
    }

    /// Compiles with the decision reversed.
    fn compile<T>(
        &self,
        body: impl FnOnce() -> T,
    ) -> T {
        match self {
            Trial::Unroll(limit) => trial::with(*limit, body),
            Trial::Rotation => trial::with_unrotated(body),
        }
    }

    /// Whether `other` is kept over `kept`, `bytes` and `work` being the finished modules'.
    fn keeps(
        &self,
        (kept_bytes, kept_work): (i64, f64),
        (other_bytes, other_work): (i64, f64),
    ) -> bool {
        let added = other_bytes - kept_bytes;
        let saved = (kept_work - other_work).round() as i64;
        let pays = match self {
            Trial::Unroll(_) => trial::keeps_at_o1(saved, added),
            Trial::Rotation => trial::keeps_at_size(added),
        };
        if llrm_support::debug::enabled("trial") {
            let what = if matches!(self, Trial::Unroll(_)) { "unroll" } else { "unrotated" };
            eprintln!("trial {what}: {added:+} bytes, {saved:+} work saved: {}", if pays { "kept" } else { "put back" });
        }
        pays
    }
}

/// `module` compiled as `compiled_module` does, then, on a target with a trial and a noted decision,
/// again with the decision reversed: whichever of the two the trial's rule prefers. `bytes`
/// is the size of a compiled module as the caller will write it, `unpadded` the same without
/// the padding between functions, which the rotation trial compares (a layout shift of one
/// function is not a code-size win); `None` leaves the first compile.
pub fn compiled_module_trying(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    bytes: &dyn Fn(&masm::Module) -> Option<i64>,
    unpadded: &dyn Fn(&masm::Module) -> Option<i64>,
) -> Result<masm::Module, String> {
    let Some(kind) = Trial::of(options) else { return compiled_module(module, name, options) };
    kind.clear();
    let kept = compiled_module(module.clone(), name, options)?;
    if !kind.noted() {
        return Ok(kept);
    }
    let mut again = options.clone();
    again.dump = None;
    let other = kind.compile(|| compiled_module(module, name, &again))?;
    let size = if matches!(kind, Trial::Rotation) { unpadded } else { bytes };
    let price = |built: &masm::Module| -> Option<(i64, f64)> {
        let work = built.procedures.iter().map(|one| executed::work(&one.body)).sum::<Option<f64>>()?;
        Some((size(built)?, work))
    };
    let (Some(kept_price), Some(other_price)) = (price(&kept), price(&other)) else { return Ok(kept) };
    Ok(if kind.keeps(kept_price, other_price) { other } else { kept })
}
