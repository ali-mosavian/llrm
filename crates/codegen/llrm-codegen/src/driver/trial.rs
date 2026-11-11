//! DEVIATE UNROLL-O1-TRIAL (beyond the pin; owner x86-m32-c, exit 2026-12-05)
//! and DEVIATE ROTATION-TRIAL (beyond the pin; owner greedy-os-causes, exit
//! 2026-12-05): a module compiled again with one decision of a pass reversed,
//! kept only if the finished object prices better.
//!
//! # How it works
//! At -O1 a target that states a trial size (`Tti::unroll_trial_threshold`)
//! refuses, in the pass, any full unroll that grows the loop by the analysis'
//! count (`unroll_never_grows_at_o1`), and notes the function. If the compile
//! noted any, the module is compiled again with every loop under the trial size
//! unrolled by the pin's threshold, and `keeps_at_o1` decides between
//! the two on exact numbers: the caller's object bytes and the expected
//! executed work of the finished bodies. The static rule cannot see what an
//! unroll frees: the rolled 3x3 nest of x_life spills to the stack, the
//! unrolled one does not (-15 B, -62% clk), while matmul's (+1601 B) is put
//! back. A module with no refused loop is compiled once.
//!
//! At -Os a target that states `Tti::rotation_trial_at_size` rotates, as RS1
//! does, a counted loop whose copied guard does not fold, and notes the
//! function. If the compile noted any, the module is compiled again with those
//! loops unrotated (gcc's `-Os` shape) and `keeps_at_size` keeps it only if the
//! object is smaller. What rotating costs or frees is in the passes after it
//! (x_prefix -12 B unrotated, x_transpose +34 B), so the finished code decides.


use std::collections::BTreeSet;

use llrm_mir::module::Module;
use llrm_mir::program::Program;
use llrm_transforms::trial::{self, differing, only};

use super::{Options, assembled, assembled_module, compiled_module, optimized_program, stage_files};
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

    /// Whether `other` is kept over `kept`, `bytes` and `work` being the
    /// finished modules'.
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
            eprintln!(
                "trial {what}: {added:+} bytes, {saved:+} work saved: {}",
                if pays { "kept" } else { "put back" }
            );
        }
        pays
    }
}

/// `module` compiled as `compiled_module` does, then, on a target with a trial
/// and a noted decision, again with the decision reversed: whichever of the two
/// the trial's rule prefers. `bytes` is the size of a compiled module as the
/// caller will write it, `unpadded` the same without the padding between
/// functions, which the rotation trial compares (a layout shift of one function
/// is not a code-size win); `None` leaves the first compile.
///
/// The rotation trial decides before the backend runs: the mid-end runs twice,
/// the backend on the functions whose bodies differ twice (cut out of the
/// module) and on the whole module once, so its cost grows with the loops
/// noted, not with the module.
pub fn compiled_module_trying(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    bytes: &dyn Fn(&masm::Module) -> Option<i64>,
    unpadded: &dyn Fn(&masm::Module) -> Option<i64>,
) -> Result<masm::Module, String> {
    let Some(kind) = Trial::of(options) else { return compiled_module(module, name, options) };
    kind.clear();
    let kept = optimized_program(module.clone(), options)?;
    if !kind.noted() {
        return assembled(&kept, name, options);
    }
    let mut again = options.clone();
    again.dump = None;
    let other = kind.compile(|| optimized_program(module, &again))?;
    if matches!(kind, Trial::Rotation) {
        let (one, two) = (&kept.modules[0], &other.modules[0]);
        let cut = differing(one, two);
        if llrm_support::debug::enabled("trial") {
            let defined =
                one.globals.iter().filter(|global| global.function().is_some_and(|f| !f.is_declaration())).count();
            eprintln!("trial unrotated: {} of {defined} functions differ: {cut:?}", cut.len());
        }
        // Pricing the cut twice and assembling the winner costs `2 * cut + all`
        // instructions of backend work; both modules whole cost `2 *
        // all`. The cut is the cheaper only while it is under half.
        let size = |module: &Module, only: Option<&BTreeSet<String>>| -> usize {
            module
                .globals
                .iter()
                .filter_map(|global| {
                    Some((global.name.as_deref()?, global.function().filter(|f| !f.is_declaration())?))
                })
                .filter(|(name, _)| only.is_none_or(|cut| cut.contains(*name)))
                .map(|(_, function)| function.instruction_count())
                .sum()
        };
        if 2 * size(one, Some(&cut)) >= size(one, None) {
            let first = assembled(&kept, name, options)?;
            let second = assembled(&other, name, &again)?;
            let pays = matches!(
                (unpadded(&first), unpadded(&second)),
                (Some(a), Some(b)) if kind.keeps((a, 0.0), (b, 0.0))
            );
            if !pays {
                return Ok(first);
            }
            stage_files(options, "", &second)?;
            return Ok(second);
        }
        let each =
            |module: &Module, program: &Program| assembled_module(&only(module, &cut), program, name, &again).ok();
        let (Some(first), Some(second)) = (each(one, &kept), each(two, &other)) else {
            return assembled(&kept, name, options);
        };
        let pays = matches!(
            (unpadded(&first), unpadded(&second)),
            (Some(a), Some(b)) if kind.keeps((a, 0.0), (b, 0.0))
        );
        // Where every function differs, the cut module is the module: its masm
        // is the answer.
        let chosen = if pays { (two, &other, second) } else { (one, &kept, first) };
        if chosen
            .0
            .globals
            .iter()
            .filter(|global| global.function().is_some_and(|f| !f.is_declaration()))
            .all(|global| global.name.as_deref().is_some_and(|name| cut.contains(name)))
        {
            stage_files(options, "", &chosen.2)?;
            return Ok(chosen.2);
        }
        return assembled(chosen.1, name, options);
    }
    let kept = assembled(&kept, name, options)?;
    let other = assembled(&other, name, &again)?;
    let price = |built: &masm::Module| -> Option<(i64, f64)> {
        let work = built.procedures.iter().map(|one| executed::work(&one.body)).sum::<Option<f64>>()?;
        Some((bytes(built)?, work))
    };
    let (Some(kept_price), Some(other_price)) = (price(&kept), price(&other)) else { return Ok(kept) };
    Ok(if kind.keeps(kept_price, other_price) { other } else { kept })
}
