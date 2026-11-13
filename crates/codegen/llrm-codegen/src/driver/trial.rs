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

/// The -O1 trial's loop-size limit, on a target that states one.
fn o1_unroll_limit(options: &Options) -> Option<u32> {
    if !options.pipeline.at_o1() {
        return None;
    }
    options.cpu().ok().and_then(|cpu| cpu.target().tti())?.unroll_trial_threshold()
}

/// A reversible decision of the -Os mid-end. The module is compiled again with one reversed and the finished code
/// decides whether it stays.
#[derive(Clone, Copy, PartialEq)]
enum Dimension {
    /// DEVIATE ROTATION-TRIAL: leave unrotated the counted loops RS1 rotated. The flag: a rotation that executes
    /// less for a few bytes a loop stays (DEVIATE ROTATION-HOT, `Tti::rotation_hot_at_size`).
    Unrotate { hot: bool, bytes: i64 },
    /// DEVIATE ROTATION-HOT: rotate the loops of an unfolded guard and uncounted trips RS1 left.
    RotateMore { bytes: i64 },
    /// DEVIATE UNROLL-OS-TRIAL: unroll the loops under this size that the -Os threshold refused.
    Unroll { limit: u32, bytes_strict: bool },
}

/// What the base compile noted for each dimension.
struct Noted {
    unroll: (Vec<String>, u64),
    rotated: Vec<String>,
    rotated_loops: usize,
    left: (Vec<String>, usize),
}

impl Noted {
    /// Takes what the compile just made noted and forgets it.
    fn take() -> Noted {
        let noted = Noted {
            unroll: trial::take(),
            rotated: trial::take_rotated(),
            rotated_loops: trial::rotated_loops(),
            left: trial::take_left(),
        };
        trial::clear_rotated_loops();
        noted
    }

    fn has(
        &self,
        dimension: Dimension,
    ) -> bool {
        match dimension {
            Dimension::Unrotate { .. } => !self.rotated.is_empty(),
            Dimension::RotateMore { .. } => !self.left.0.is_empty(),
            Dimension::Unroll { .. } => !self.unroll.0.is_empty(),
        }
    }
}

impl Dimension {
    /// The dimensions `options` has, in the order they are decided.
    fn of(options: &Options) -> Vec<Dimension> {
        if options.pipeline.at_o1() || !options.pipeline.for_size {
            return Vec::new();
        }
        let Some(tti) = options.cpu().ok().and_then(|cpu| cpu.target().tti()) else { return Vec::new() };
        let mut all = Vec::new();
        if tti.rotation_guard_must_fold_at_size() && tti.rotation_trial_at_size() {
            all.push(Dimension::Unrotate { hot: tti.rotation_hot_at_size(), bytes: tti.rotation_hot_bytes_per_loop() });
            if tti.rotation_hot_at_size() {
                all.push(Dimension::RotateMore { bytes: tti.rotation_hot_bytes_per_loop() });
            }
        }
        if let Some(limit) = tti.unroll_trial_at_size().filter(|_| !NO_SIZE_UNROLL.with(|one| one.get())) {
            all.push(Dimension::Unroll { limit, bytes_strict: tti.unroll_trial_bytes_strict() });
        }
        all
    }

    /// Runs `body` with this decision reversed.
    fn compile<T>(
        self,
        body: impl FnOnce() -> T,
    ) -> T {
        match self {
            Dimension::Unrotate { .. } => trial::with_unrotated(body),
            Dimension::RotateMore { .. } => trial::with_more_rotation(body),
            Dimension::Unroll { limit, .. } => trial::with(limit, body),
        }
    }

    /// Whether the reversed compile is kept over the base, `bytes` and `work` being the finished modules'.
    fn keeps(
        self,
        noted: &Noted,
        (kept_sizes, kept_work): (Sizes, Option<f64>),
        (other_sizes, other_work): (Sizes, Option<f64>),
    ) -> bool {
        // A bytes-strict unroll trial compares the code alone: the constants a loop's removal frees are data, which
        // a code-size column does not count.
        let strict = matches!(self, Dimension::Unroll { bytes_strict: true, .. });
        let (kept_bytes, other_bytes) =
            if strict { (kept_sizes.code, other_sizes.code) } else { (kept_sizes.total(), other_sizes.total()) };
        let added = other_bytes - kept_bytes;
        let work = |one: Option<f64>| one.unwrap_or(0.0);
        let saved = (work(kept_work) - work(other_work)).round() as i64;
        let measured = kept_work.is_some() && other_work.is_some();
        let (what, pays) = match self {
            // The unrotated object is smaller; the rotated one stays if it executes less for a few bytes a loop.
            Dimension::Unrotate { hot, bytes } => (
                "unrotated",
                trial::keeps_at_size(added)
                    && !(hot && trial::keeps_rotation_hot(-saved, -added, noted.rotated_loops, bytes)),
            ),
            Dimension::RotateMore { bytes } => ("rotation-hot", measured && trial::keeps_rotation_hot(saved, added, noted.left.1, bytes)),
            Dimension::Unroll { bytes_strict, .. } => {
                ("unroll-os", measured && trial::keeps_unroll_at_size(saved, added, noted.unroll.0.len(), bytes_strict))
            }
        };
        if llrm_support::debug::enabled("trial") {
            eprintln!("trial {what}: {added:+} bytes, {saved:+} work saved: {}", if pays { "kept" } else { "put back" });
        }
        pays
    }
}

/// `body` with every dimension of `all` reversed at once.
fn compiled_with<T>(
    all: &[Dimension],
    body: impl FnOnce() -> T,
) -> T {
    match all.split_first() {
        None => body(),
        Some((one, rest)) => one.compile(|| compiled_with(rest, body)),
    }
}

/// What a finished module is expected to execute: instructions and memory operands as `executed::work`, the
/// branches and jumps at their clocks on the CPU (a rotation trades a taken `jmp` for a copied guard, which the
/// work alone does not see). `None` where the model has no answer.
fn rotation_work(
    built: &masm::Module,
    options: &Options,
) -> Option<f64> {
    let cpu = options.cpu().ok()?;
    built
        .procedures
        .iter()
        .map(|one| {
            let done = executed::executed(&one.body)?;
            let jumps = done.jump_cycles(cpu).ok()?;
            Some(done.instructions - done.branches - done.jumps + done.memory + jumps)
        })
        .sum::<Option<f64>>()
}

/// The unpadded bytes of a finished module: its functions' code and its data sections' images.
#[derive(Clone, Copy)]
pub struct Sizes {
    pub code: i64,
    pub data: i64,
}

impl Sizes {
    fn total(self) -> i64 {
        self.code + self.data
    }
}

/// A compiled variant of the module: its code, the program the backend took (the leaf-fold pair assembles it again)
/// and whether the backend folded a leaf.
struct Variant {
    built: masm::Module,
    program: Program,
    folded: bool,
}

/// `program` assembled, noting whether the backend folded a leaf.
fn built(
    program: Program,
    name: &str,
    options: &Options,
) -> Result<Variant, String> {
    let _ = crate::backend::leaffold::take_folded();
    let built = assembled(&program, name, options)?;
    Ok(Variant { built, program, folded: crate::backend::leaffold::take_folded() })
}

/// DEVIATE LEAF-FOLD-TRIAL: where the backend folded a leaf's constant into the branching block, the module is
/// assembled again without the fold and the smaller one kept. Decided once, on the variant the other trials chose,
/// not in each of them: the pair doubled every variant's assembly.
fn with_leaf_trial(
    variant: Variant,
    name: &str,
    options: &Options,
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    if !variant.folded || !leaf_fold_trial(options) {
        return Ok(variant.built);
    }
    let second = crate::backend::leaffold::without(|| assembled(&variant.program, name, options))?;
    let smaller = matches!((unpadded(&variant.built), unpadded(&second)), (Some(a), Some(b)) if b.total() < a.total());
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial leaffold: {}", if smaller { "put back" } else { "kept" });
    }
    let chosen = if smaller { second } else { variant.built };
    stage_files(options, "", &chosen)?;
    Ok(chosen)
}

/// Whether the module is assembled with and without LEAF-FOLD-OS: a size build on a target that folds.
fn leaf_fold_trial(options: &Options) -> bool {
    options.cpu().is_ok_and(|cpu| cpu.size) && options.arch.leaf_fold_at_size()
}

/// `module` compiled as `compiled_module` does and, where the target has trials, again with one decision reversed
/// at a time, each kept if the finished code prices better: at -O1 UNROLL-O1-TRIAL (`Tti::unroll_trial_threshold`),
/// at -Os the dimensions of [`Dimension`]. `bytes` is the size of a compiled module as the caller will write it,
/// `unpadded` the same without the padding between functions (a layout shift of one function is not a code-size
/// win); `None` leaves the first compile.
pub fn compiled_module_trying(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    bytes: &dyn Fn(&masm::Module) -> Option<i64>,
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    if let Some(limit) = o1_unroll_limit(options) {
        return compiled_module_at_o1(module, name, options, limit, bytes, unpadded);
    }
    let dimensions = Dimension::of(options);
    if dimensions.is_empty() {
        if !leaf_fold_trial(options) {
            return compiled_module(module, name, options);
        }
        let variant = built(optimized_program(module, options)?, name, options)?;
        return with_leaf_trial(variant, name, options, unpadded);
    }
    compiled_module_at_size(module, name, options, &dimensions, unpadded)
}

/// UNROLL-O1-TRIAL: the loops under the trial size unrolled by the pin's threshold, kept if the object is no
/// larger and the code does no more work.
fn compiled_module_at_o1(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    limit: u32,
    bytes: &dyn Fn(&masm::Module) -> Option<i64>,
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    // A bytes-strict target (`unroll_trial_bytes_strict`) compares the code alone, as at -Os: the constants a
    // loop's removal frees are data, which a code-size column does not count (call_cleanup_keeps_pair +13 B).
    let strict = options.cpu().ok().and_then(|cpu| cpu.target().tti()).is_some_and(|tti| tti.unroll_trial_bytes_strict());
    let _ = trial::take();
    let kept = optimized_program(module.clone(), options)?;
    if trial::take().0.is_empty() {
        return assembled(&kept, name, options);
    }
    let mut again = options.clone();
    again.dump = None;
    let other = trial::with(limit, || optimized_program(module, &again))?;
    let kept = assembled(&kept, name, options)?;
    let other = assembled(&other, name, &again)?;
    let price = |built: &masm::Module| -> Option<(i64, f64)> {
        let work = built.procedures.iter().map(|one| executed::work(&one.body)).sum::<Option<f64>>()?;
        Some((if strict { unpadded(built)?.code } else { bytes(built)? }, work))
    };
    let (Some(kept_price), Some(other_price)) = (price(&kept), price(&other)) else { return Ok(kept) };
    let added = other_price.0 - kept_price.0;
    let saved = (kept_price.1 - other_price.1).round() as i64;
    let pays = trial::keeps_at_o1(saved, added);
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial unroll: {added:+} bytes, {saved:+} work saved: {}", if pays { "kept" } else { "put back" });
    }
    Ok(if pays { other } else { kept })
}

/// The -Os trials. The base compile notes what each dimension could reverse. Each noted dimension is compiled
/// against the base alone, priced on the functions that differ (the whole module where they are half of it) and
/// kept or put back; the dimensions that stay are added one at a time and priced against the code so far. The mid-end
/// runs `1 + k` times for `k` noted dimensions, plus one for each that stays after the first, where nesting them ran
/// it `2^k` times.
fn compiled_module_at_size(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    dimensions: &[Dimension],
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    let _ = Noted::take();
    let (base_program, prewalk) = trial::recording_prewalk(|| optimized_program(module.clone(), options));
    let base_program = base_program?;
    let noted = Noted::take();
    // The variants start where the base was before the walk: nothing before it reads what a variant reverses.
    let resumed = |body: &dyn Fn() -> Result<Program, String>| match &prewalk {
        Some(prewalk) => trial::resuming_prewalk(prewalk, body),
        None => body(),
    };
    let active: Vec<Dimension> = dimensions.iter().copied().filter(|one| noted.has(*one)).collect();
    let base = built(base_program, name, options)?;
    if active.is_empty() {
        return with_leaf_trial(base, name, options, unpadded);
    }
    let mut again = options.clone();
    again.dump = None;
    // A dimension that stays, with its variant assembled whole or the program not yet assembled.
    let mut kept: Vec<(Dimension, Result<Variant, Program>)> = Vec::new();
    for dimension in active {
        let program = resumed(&|| dimension.compile(|| optimized_program(module.clone(), &again)))?;
        let _ = Noted::take();
        let cut = differing(&base.program.modules[0], &program.modules[0]);
        if llrm_support::debug::enabled("trial") {
            eprintln!("trial: {} of the functions differ: {cut:?}", cut.len());
        }
        if cut.is_empty() {
            continue;
        }
        // Pricing the cut twice costs `2 * cut` instructions of backend work, the variant whole `all`: the cut is
        // the cheaper only while it is under half.
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
        let price = |one: &masm::Module| Some((unpadded(one)?, rotation_work(one, options)));
        if 2 * size(&base.program.modules[0], Some(&cut)) >= size(&base.program.modules[0], None) {
            let variant = built(program, name, &again)?;
            let pays = matches!((price(&base.built), price(&variant.built)), (Some(a), Some(b)) if dimension.keeps(&noted, a, b));
            if pays {
                kept.push((dimension, Ok(variant)));
            }
            continue;
        }
        let each = |program: &Program| assembled_module(&only(&program.modules[0], &cut), program, name, &again).ok();
        let (Some(first), Some(second)) = (each(&base.program), each(&program)) else { continue };
        let pays = matches!((price(&first), price(&second)), (Some(a), Some(b)) if dimension.keeps(&noted, a, b));
        if pays {
            kept.push((dimension, Err(program)));
        }
    }
    // The first dimension that stayed is the variant already priced against the base. Each further one is added to
    // it and priced against the code so far: a dimension's gain does not carry over when another changed the loops
    // (x_popcount: the unrotated loops and the hot rotations each paid alone and cost clocks together).
    let mut kept = kept.into_iter();
    let Some((first, variant)) = kept.next() else { return with_leaf_trial(base, name, options, unpadded) };
    let mut accepted = vec![first];
    let mut chosen = match variant {
        Ok(variant) => variant,
        Err(program) => built(program, name, &again)?,
    };
    for (dimension, _) in kept {
        accepted.push(dimension);
        let program = resumed(&|| compiled_with(&accepted, || optimized_program(module.clone(), &again)))?;
        let _ = Noted::take();
        let together = built(program, name, &again)?;
        let price = |one: &masm::Module| Some((unpadded(one)?, rotation_work(one, options)));
        let pays =
            matches!((price(&chosen.built), price(&together.built)), (Some(a), Some(b)) if dimension.keeps(&noted, a, b));
        if pays {
            chosen = together;
        } else {
            accepted.pop();
        }
    }
    stage_files(options, "", &chosen.built)?;
    with_leaf_trial(chosen, name, options, unpadded)
}

thread_local! {
    static NO_SIZE_UNROLL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `body` with UNROLL-OS-TRIAL off: the compile it would be compared with.
pub fn without_size_unroll<T>(body: impl FnOnce() -> T) -> T {
    let before = NO_SIZE_UNROLL.with(|one| one.replace(true));
    let made = body();
    NO_SIZE_UNROLL.with(|one| one.set(before));
    made
}
