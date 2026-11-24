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
//!
//! A rotation is priced with the heuristic's trips in both shapes
//! (`executed_unproven`): the copied guard gives a rotated nest two exits,
//! which neither SCEV nor the induction proof counts, so priced with its proven
//! trips the rotated shape weighed 3.9x the unrotated one (x_prefix -Os, 12,810
//! against 3,297) while it ran 14% faster, and ROTATION-HOT never kept it. An
//! unroll takes the loop away and keeps its proven counts.

use std::collections::BTreeSet;

use llrm_mir::module::Module;
use llrm_mir::program::Program;
use llrm_transforms::trial::{self, differing, only};

use super::{Options, assembled, assembled_module, compiled_module, optimized_program, stage_files};
use crate::backend::cpu::Profile;
use crate::backend::{executed, masm};

/// The -O1 trial's loop-size limit, on a target that states one.
fn o1_unroll_limit(options: &Options) -> Option<u32> {
    if !options.pipeline.at_o1() {
        return None;
    }
    options.cpu().ok().and_then(|cpu| cpu.target().tti())?.unroll_trial_threshold()
}

/// A reversible decision of the -Os mid-end. The module is compiled again with
/// one reversed and the finished code decides whether it stays.
#[derive(Clone, Copy, PartialEq)]
enum Dimension {
    /// DEVIATE ROTATION-TRIAL: leave unrotated the counted loops RS1 rotated.
    /// The flag: a rotation that executes less for a few bytes a loop stays
    /// (DEVIATE ROTATION-HOT, `Tti::rotation_hot_at_size`).
    Unrotate { hot: bool, bytes: i64 },
    /// DEVIATE ROTATION-HOT: rotate the loops of an unfolded guard and
    /// uncounted trips RS1 left.
    RotateMore { bytes: i64 },
    /// DEVIATE UNROLL-OS-TRIAL: unroll the loops under this size that the -Os
    /// threshold refused.
    Unroll { limit: u32, bytes_strict: bool },
    /// DEVIATE JT-LATCH-TRIAL: JumpThreading does not copy a loop latch.
    KeepLatch,
    /// DEVIATE SWITCH-LOOKUP-TRIAL: no switch becomes a lookup table.
    KeepSwitch,
    /// DEVIATE LIR-COPY-TRIAL: a copy loop of an unknown count stays a loop.
    KeepCopyLoop,
    /// DEVIATE INLINEHINT-GROWTH-TRIAL: the -Os size guard binds an
    /// `inlinehint` callee too.
    KeepHintedCalls,
    /// DEVIATE INLINEHINT-LOOP-TRIAL: the same at a site in a loop, its own
    /// bundle under the same cap.
    KeepLoopCalls,
    /// DEVIATE HOT-INLINE-TRIAL: the -Os size bound holds at a locally hot
    /// site too.
    KeepHotCalls,
    /// DEVIATE OS-OUTLINE-TRIAL: a callee the size bound let inline, that goes
    /// with its calls, stays a call.
    OutlineCalls,
}

/// What the base compile noted for each dimension.
struct Noted {
    unroll: (Vec<String>, u64),
    rotated: Vec<String>,
    rotated_loops: usize,
    left: (Vec<String>, usize),
    latched: Vec<String>,
    lookups: usize,
    copied: Vec<String>,
    hinted: Vec<String>,
    looped: Vec<String>,
    hot: Vec<String>,
    /// The distinct callees of the cold sites the size bound accepted, which
    /// the trial could keep as calls.
    accepted: usize,
}

impl Noted {
    /// Takes what the compile just made noted and forgets it.
    fn take() -> Noted {
        let noted = Noted {
            unroll: trial::take(),
            rotated: trial::take_rotated(),
            rotated_loops: trial::rotated_loops(),
            left: trial::take_left(),
            latched: trial::take_latch_copies(),
            lookups: trial::take_lookup_tables(),
            copied: trial::take_copy_calls(),
            hinted: trial::take_hint_yields(),
            looped: trial::take_loop_yields(),
            hot: trial::take_hot_yields(),
            accepted: trial::take_accepted()
                .iter()
                .flat_map(|site| site.callees.iter())
                .collect::<BTreeSet<_>>()
                .len(),
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
            Dimension::KeepLatch => !self.latched.is_empty(),
            Dimension::KeepSwitch => self.lookups > 0,
            Dimension::KeepCopyLoop => !self.copied.is_empty(),
            Dimension::KeepHintedCalls => !self.hinted.is_empty(),
            Dimension::KeepLoopCalls => !self.looped.is_empty(),
            Dimension::KeepHotCalls => !self.hot.is_empty(),
            Dimension::OutlineCalls => self.accepted > 0,
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
        }
        if tti.switch_lookup_trial_at_size() && !llrm_support::env_flag!("LLRM_NO_SWITCH_LOOKUP_TRIAL") {
            all.push(Dimension::KeepSwitch);
        }
        if tti.loop_idiom_copy_trial_at_size() && !llrm_support::env_flag!("LLRM_NO_LIR_COPY_TRIAL") {
            all.push(Dimension::KeepCopyLoop);
        }
        if tti.jump_threading_latch_trial_at_size() && !llrm_support::env_flag!("LLRM_NO_JT_LATCH_TRIAL") {
            all.push(Dimension::KeepLatch);
        }
        if tti.opt_size_inline_guard_yields_to_inlinehint() && !llrm_support::env_flag!("LLRM_NO_INLINEHINT_TRIAL") {
            all.push(Dimension::KeepHintedCalls);
            if !llrm_support::env_flag!("LLRM_NO_INLINEHINT_LOOP_TRIAL") {
                all.push(Dimension::KeepLoopCalls);
            }
        }
        if tti.opt_size_inline_hot_growth_percent().is_some() && !llrm_support::env_flag!("LLRM_NO_HOT_INLINE_TRIAL") {
            all.push(Dimension::KeepHotCalls);
        }
        if let Some(limit) = tti.unroll_trial_at_size().filter(|_| !NO_SIZE_UNROLL.with(|one| one.get())) {
            all.push(Dimension::Unroll { limit, bytes_strict: tti.unroll_trial_bytes_strict() });
        }
        // Last: the trials that save bytes are decided first, so the bytes the
        // hot rotation adds are priced on the smallest code they made and
        // cannot displace one (QB suggest.c, arm64: +64 B kept first, -52 B put
        // back).
        if tti.rotation_guard_must_fold_at_size() && tti.rotation_trial_at_size() && tti.rotation_hot_at_size() {
            all.push(Dimension::RotateMore { bytes: tti.rotation_hot_bytes_per_loop() });
        }
        // After the hot rotation: that trial may add bytes, which this one,
        // priced on the code so far, cannot be left to pay for (QB m32
        // suggest.c: +142 B of rotation after the outline, +29 before
        // it).
        if tti.opt_size_inline_outline_trial() && !llrm_support::env_flag!("LLRM_NO_OUTLINE_TRIAL") {
            all.push(Dimension::OutlineCalls);
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
            Dimension::KeepLatch => trial::with_kept_latches(body),
            Dimension::KeepSwitch => trial::with_refused_lookup_tables(body),
            Dimension::KeepCopyLoop => trial::with_kept_copy_loops(body),
            Dimension::KeepHintedCalls => trial::with_kept_hinted_calls(body),
            Dimension::KeepLoopCalls => trial::with_kept_loop_calls(body),
            Dimension::KeepHotCalls => trial::with_kept_hot_calls(body),
            Dimension::OutlineCalls => trial::with_outlined_all(body),
        }
    }

    /// Whether the two compiles are priced with the loops' proven trip counts.
    /// An unroll takes the loop away, and the count it was unrolled for is
    /// what pays. A rotation leaves the loop and may hide its count
    /// (the copied guard gives it two exits), so a rotation is priced with the
    /// heuristic's trips in both shapes.
    fn prices_proven_trips(self) -> bool {
        matches!(self, Dimension::Unroll { .. })
    }

    /// Whether the two compiles are priced with each callee's body weighted by
    /// the calls that reach it. A trial that moves work between a function and
    /// its callees (a hinted callee kept as a call, a table the callee holds)
    /// is not priced by the function alone.
    fn prices_called_work(self) -> bool {
        matches!(
            self,
            Dimension::KeepLatch
                | Dimension::KeepSwitch
                | Dimension::KeepHintedCalls
                | Dimension::KeepLoopCalls
                | Dimension::KeepHotCalls
                | Dimension::OutlineCalls
        )
    }

    /// Whether the reversed compile is kept over the base, `bytes` and `work`
    /// being the finished modules'.
    fn keeps(
        self,
        noted: &Noted,
        (kept_sizes, kept_work): (Sizes, Option<f64>),
        (other_sizes, other_work): (Sizes, Option<f64>),
        cap: Option<i64>,
    ) -> bool {
        // A bytes-strict unroll trial compares the code alone: the constants a
        // loop's removal frees are data, which a code-size column does
        // not count.
        let strict = matches!(self, Dimension::Unroll { bytes_strict: true, .. });
        let (kept_bytes, other_bytes) =
            if strict { (kept_sizes.code, other_sizes.code) } else { (kept_sizes.total(), other_sizes.total()) };
        let added = other_bytes - kept_bytes;
        let work = |one: Option<f64>| one.unwrap_or(0.0);
        let saved = (work(kept_work) - work(other_work)).round() as i64;
        let base_work = work(kept_work).round() as i64;
        let measured = kept_work.is_some() && other_work.is_some();
        let (what, pays) = match self {
            // The unrotated object is smaller; the rotated one stays if it
            // executes less for a few bytes a loop.
            Dimension::Unrotate { hot, bytes } => (
                "unrotated",
                trial::keeps_at_size(added)
                    && !(hot && trial::keeps_rotation_hot(-saved, -added, noted.rotated_loops, bytes)),
            ),
            Dimension::RotateMore { bytes } => {
                ("rotation-hot", measured && trial::keeps_rotation_hot(saved, added, noted.left.1, bytes))
            }
            // The object without the latch copies is smaller.
            Dimension::KeepLatch => ("keep-latch", trial::keeps_smaller_at_size(added, saved, base_work, cap)),
            Dimension::KeepSwitch => ("keep-switch", trial::keeps_smaller_at_size(added, saved, base_work, cap)),
            // The loop is the smaller object; it stays where the call's clocks
            // (priced at the loop's own trip weight, see `library_cycles`) do
            // not beat it past the cap.
            Dimension::KeepCopyLoop => ("keep-copy-loop", trial::keeps_smaller_at_size(added, saved, base_work, cap)),
            Dimension::KeepHintedCalls => {
                ("keep-hinted-calls", trial::keeps_smaller_at_size(added, saved, base_work, cap))
            }
            Dimension::KeepLoopCalls => ("keep-loop-calls", trial::keeps_smaller_at_size(added, saved, base_work, cap)),
            Dimension::KeepHotCalls => ("keep-hot-calls", trial::keeps_smaller_at_size(added, saved, base_work, cap)),
            Dimension::OutlineCalls => ("outline-calls", trial::keeps_smaller_at_size(added, saved, base_work, cap)),
            Dimension::Unroll { bytes_strict, .. } => {
                ("unroll-os", measured && trial::keeps_unroll_at_size(saved, added, noted.unroll.0.len(), bytes_strict))
            }
        };
        if llrm_support::debug::enabled("trial") {
            eprintln!(
                "trial {what}: {added:+} bytes, {saved:+} work saved (base work {base_work}, cap {cap:?}): {}",
                if pays { "kept" } else { "put back" }
            );
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

/// The words a copy of unknown length is priced at: the trips the frequency
/// model gives a loop of unknown count (a branch to stay of 31 in 32).
const UNKNOWN_COPY_WORDS: f64 = 32.0;

/// The clocks the library's copy routines take that `done` calls, which the
/// module does not define and the work would otherwise price as a call and
/// return alone: `memcpy` is `rep movsd` (startup, then a cell a word) over the
/// words a loop of unknown count runs, so a copy loop and the call LoopIdiom
/// made of it are priced alike.
fn library_cycles(
    one: &masm::Procedure,
    done: &executed::Executed,
    cpu: &Profile,
) -> f64 {
    let (Ok(startup), Ok(cell)) = (cpu.cost("rep_movs"), cpu.cost("rep_movs_cell")) else { return 0.0 };
    let runs: f64 = done
        .calls
        .iter()
        .filter_map(|(at, runs)| one.callees.get(at).map(|callee| (callee.name.trim_start_matches('_'), runs)))
        .filter(|(name, _)| matches!(*name, "memcpy" | "memmove"))
        .map(|(_, runs)| runs)
        .sum();
    runs * (startup as f64 + cell as f64 * UNKNOWN_COPY_WORDS)
}

/// What a finished module is expected to execute: instructions and memory
/// operands as `executed::work`, the branches, jumps and x87 exchanges at their
/// clocks on the CPU (a rotation trades a taken `jmp` for a copied guard, and
/// can add `fxch`es to the loop, which the work alone does not see). `None`
/// where the model has no answer.
fn rotation_work(
    built: &masm::Module,
    options: &Options,
    proven: bool,
    called: bool,
) -> Option<f64> {
    let cpu = options.cpu().ok()?;
    let own = |one: &masm::Procedure| -> Option<(f64, Vec<(String, f64)>)> {
        let done = if proven { executed::executed(&one.body)? } else { executed::executed_unproven(&one.body)? };
        let jumps = done.jump_cycles(cpu).ok()?;
        let exchanges = done.exchange_cycles(cpu).ok()?;
        let work =
            done.instructions - done.branches - done.jumps + done.memory + jumps + exchanges + done.call_cycles(cpu);
        let work = work + library_cycles(one, &done, cpu);
        let calls = done
            .calls
            .iter()
            .filter_map(|(at, runs)| one.callees.get(at).map(|callee| (callee.name.clone(), *runs)))
            .collect();
        Some((work, calls))
    };
    let each = built.procedures.iter().map(own).collect::<Option<Vec<_>>>()?;
    if !called {
        return Some(each.iter().map(|(work, _)| work).sum());
    }
    // Each function's work weighted by how often a call of the module's roots
    // runs it: a callee body is paid once for every call that reaches it. A
    // function nothing in the module calls is a root; a call along a cycle
    // adds nothing (the weights are made in one pass, callers first).
    let at: std::collections::BTreeMap<&str, usize> =
        built.procedures.iter().enumerate().map(|(index, one)| (one.name.as_str(), index)).collect();
    let mut callers = vec![0usize; each.len()];
    for (index, (_, calls)) in each.iter().enumerate() {
        for (callee, _) in calls {
            if let Some(&to) = at.get(callee.as_str()).filter(|&&to| to != index) {
                callers[to] += 1;
            }
        }
    }
    // A module that defines `main` is a program: what runs is what `main`
    // reaches, and a function nothing in the module calls is not run at all
    // (a library's other entries: a bench's driver calls one of them).
    let entry = at.get("main").copied();
    let root = |index: usize| entry.map_or(true, |main| index == main);
    let mut weight: Vec<f64> =
        callers.iter().enumerate().map(|(index, &n)| if n == 0 && root(index) { 1.0 } else { 0.0 }).collect();
    let mut ready: Vec<usize> = (0..each.len()).filter(|&index| callers[index] == 0).collect();
    while let Some(index) = ready.pop() {
        for (callee, runs) in &each[index].1 {
            let Some(&to) = at.get(callee.as_str()).filter(|&&to| to != index) else { continue };
            weight[to] += weight[index] * runs;
            callers[to] -= 1;
            if callers[to] == 0 {
                ready.push(to);
            }
        }
    }
    // What a cycle left unreached is paid once.
    let paid = |index: usize, weight: f64| {
        if weight != 0.0 {
            weight
        } else if entry.is_none() || callers[index] > 0 {
            1.0
        } else {
            0.0
        }
    };
    if llrm_support::debug::enabled("trialwork") {
        for (index, ((work, _), weight)) in each.iter().zip(&weight).enumerate() {
            eprintln!(
                "trialwork {} work={work} weight={weight} paid={}",
                built.procedures[index].name,
                paid(index, *weight)
            );
        }
    }
    Some(each.iter().zip(&weight).enumerate().map(|(index, ((work, _), weight))| work * paid(index, *weight)).sum())
}

/// The unpadded bytes of a finished module: its functions' code and its data
/// sections' images.
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

/// A compiled variant of the module: its code, the program the backend took
/// (the leaf-fold pair assembles it again) and whether the backend folded a
/// leaf.
#[derive(Clone)]
struct Variant {
    built: masm::Module,
    program: Program,
    folded: bool,
    /// A function calls a locally binding callee the facts skip.
    ipra: bool,
}

/// `program` assembled, noting whether the backend folded a leaf.
fn built(
    program: Program,
    name: &str,
    options: &Options,
) -> Result<Variant, String> {
    let _ = crate::backend::leaffold::take_folded();
    let _ = trial::take_ipra_matters();
    let built = assembled(&program, name, options)?;
    let ipra = trial::take_ipra_matters();
    Ok(Variant { built, program, folded: crate::backend::leaffold::take_folded(), ipra })
}

/// DEVIATE LEAF-FOLD-TRIAL: where the backend folded a leaf's constant into the
/// branching block, the module is assembled again without the fold and the
/// smaller one kept. Decided once, on the variant the other trials chose,
/// not in each of them: the pair doubled every variant's assembly.
fn with_leaf_trial(
    variant: Variant,
    name: &str,
    options: &Options,
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    let mut variant = variant;
    let smaller_than = |first: &masm::Module, second: &masm::Module| {
        matches!(
            (unpadded(first), unpadded(second)),
            (Some(a), Some(b)) if b.total() < a.total()
        )
    };
    // DEVIATE IPRA-OS-TRIAL: the module assembled with the locally binding
    // callees' register facts, kept where the object is smaller.
    let mut ipra_on = false;
    if variant.ipra && ipra_trial(options) {
        let second = trial::with_ipra_binds_local(|| assembled(&variant.program, name, options))?;
        ipra_on = smaller_than(&variant.built, &second);
        if llrm_support::debug::enabled("trial") {
            eprintln!("trial ipra-local: {}", if ipra_on { "kept" } else { "put back" });
        }
        if ipra_on {
            variant.built = second;
        }
    }
    if !variant.folded || !leaf_fold_trial(options) {
        if ipra_on {
            stage_files(options, "", &variant.built)?;
        }
        return Ok(variant.built);
    }
    let second = crate::backend::leaffold::without(|| {
        if ipra_on {
            trial::with_ipra_binds_local(|| assembled(&variant.program, name, options))
        } else {
            assembled(&variant.program, name, options)
        }
    })?;
    let smaller = smaller_than(&variant.built, &second);
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial leaffold: {}", if smaller { "put back" } else { "kept" });
    }
    let chosen = if smaller { second } else { variant.built };
    stage_files(options, "", &chosen)?;
    Ok(chosen)
}

/// Whether the module is assembled with and without IPRA-OS-TRIAL: a size
/// build on a target that has it.
fn ipra_trial(options: &Options) -> bool {
    options.cpu().is_ok_and(|cpu| cpu.size && cpu.target().tti().is_some_and(|tti| tti.ipra_trial_at_size()))
        && !llrm_support::env_flag!("LLRM_NO_IPRA_TRIAL")
}

/// Whether the module is assembled with and without LEAF-FOLD-OS: a size build
/// on a target that folds.
fn leaf_fold_trial(options: &Options) -> bool {
    options.cpu().is_ok_and(|cpu| cpu.size) && options.arch.leaf_fold_at_size()
}

/// `module` compiled as `compiled_module` does and, where the target has
/// trials, again with one decision reversed at a time, each kept if the
/// finished code prices better: at -O1 UNROLL-O1-TRIAL
/// (`Tti::unroll_trial_threshold`), at -Os the dimensions of [`Dimension`].
/// `bytes` is the size of a compiled module as the caller will write it,
/// `unpadded` the same without the padding between functions (a layout shift of
/// one function is not a code-size win); `None` leaves the first compile.
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
        if !leaf_fold_trial(options) && !ipra_trial(options) {
            return compiled_module(module, name, options);
        }
        let variant = built(optimized_program(module, options)?, name, options)?;
        return with_leaf_trial(variant, name, options, unpadded);
    }
    compiled_module_at_size(module, name, options, &dimensions, unpadded)
}

/// UNROLL-O1-TRIAL: the loops under the trial size unrolled by the pin's
/// threshold, kept if the object is no larger and the code does no more work.
fn compiled_module_at_o1(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    limit: u32,
    bytes: &dyn Fn(&masm::Module) -> Option<i64>,
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    // A bytes-strict target (`unroll_trial_bytes_strict`) compares the code
    // alone, as at -Os: the constants a loop's removal frees are data,
    // which a code-size column does not count (call_cleanup_keeps_pair +13 B).
    let strict =
        options.cpu().ok().and_then(|cpu| cpu.target().tti()).is_some_and(|tti| tti.unroll_trial_bytes_strict());
    let _ = trial::take();
    let kept = optimized_program(module.clone(), options)?;
    if trial::take().0.is_empty() {
        return assembled(&kept, name, options);
    }
    let mut again = options.clone();
    again.dump = None;
    let other = trial::with(limit, || optimized_program(module.clone(), &again))?;
    let kept = assembled(&kept, name, options)?;
    let other = assembled(&other, name, &again)?;
    let price = |built: &masm::Module| -> Option<(i64, f64)> {
        let work = built.procedures.iter().map(|one| executed::work(&one.body)).sum::<Option<f64>>()?;
        Some((if strict { unpadded(built)?.code } else { bytes(built)? }, work))
    };
    let (Some(kept_price), Some(other_price)) = (price(&kept), price(&other)) else { return Ok(kept) };
    let added = other_price.0 - kept_price.0;
    let saved = (kept_price.1 - other_price.1).round() as i64;
    let rate = options.cpu().ok().and_then(|cpu| cpu.target().tti()).map_or(0, |tti| tti.unroll_milliclocks_per_byte());
    let pays = trial::keeps_at_o1(saved, added, rate);
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial unroll: {added:+} bytes, {saved:+} work saved: {}", if pays { "kept" } else { "put back" });
    }
    if pays && options.dump.is_some() {
        // The stage files are the compile that was put back: the kept one is
        // dumped.
        trial::with(limit, || optimized_program(module, options))?;
    }
    Ok(if pays { other } else { kept })
}

/// The -Os trials. The base compile notes what each dimension could reverse.
/// Each noted dimension is compiled against the base alone, priced on the
/// functions that differ (the whole module where they are half of it) and
/// kept or put back; the dimensions that stay are added one at a time and
/// priced against the code so far. The mid-end runs `1 + k` times for `k` noted
/// dimensions, plus one for each that stays after the first, where nesting them
/// ran it `2^k` times.
fn compiled_module_at_size(
    module: llrm_mir::module::Module,
    name: &str,
    options: &Options,
    dimensions: &[Dimension],
    unpadded: &dyn Fn(&masm::Module) -> Option<Sizes>,
) -> Result<masm::Module, String> {
    let _ = Noted::take();
    let base_program = optimized_program(module.clone(), options)?;
    let noted = Noted::take();
    // OS-OUTLINE-TRIAL's compile cost grows with its candidates, its gain does
    // not: a module of many candidate callees is not tried
    // (`Tti::opt_size_inline_outline_max_callees`).
    let outline_callees = options
        .cpu()
        .ok()
        .and_then(|cpu| cpu.target().tti())
        .and_then(|tti| tti.opt_size_inline_outline_max_callees());
    let active: Vec<Dimension> = dimensions
        .iter()
        .copied()
        .filter(|one| noted.has(*one))
        .filter(|one| *one != Dimension::OutlineCalls || outline_callees.is_none_or(|most| noted.accepted <= most))
        .collect();
    let base = built(base_program, name, options)?;
    if active.is_empty() {
        return with_leaf_trial(base, name, options, unpadded);
    }
    let cap = options.cpu().ok().and_then(|cpu| cpu.target().tti()).and_then(|tti| tti.os_trial_clock_cap_percent());
    // OS-OUTLINE-TRIAL has a cap of its own where the target states one: it
    // trades clocks on every call it keeps (QB m64 funcs.c: -85 B for +18%
    // of the work, +3.8M guest instructions on a string bench).
    let outline_cap = options
        .cpu()
        .ok()
        .and_then(|cpu| cpu.target().tti())
        .and_then(|tti| tti.opt_size_inline_outline_clock_cap_percent())
        .or(cap);
    let cap_of = |dimension: Dimension| if dimension == Dimension::OutlineCalls { outline_cap } else { cap };
    let mut again = options.clone();
    again.dump = None;
    // A dimension that stays, with its variant assembled whole or the program
    // not yet assembled.
    let mut kept: Vec<(Dimension, Result<Variant, Program>)> = Vec::new();
    for dimension in active {
        let program = dimension.compile(|| optimized_program(module.clone(), &again))?;
        let _ = Noted::take();
        let cut = differing(&base.program.modules[0], &program.modules[0]);
        if llrm_support::debug::enabled("trial") {
            eprintln!("trial: {} of the functions differ: {cut:?}", cut.len());
        }
        if cut.is_empty() {
            continue;
        }
        // Pricing the cut twice costs `2 * cut` instructions of backend work,
        // the variant whole `all`: the cut is the cheaper only while it
        // is under half.
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
        let price = |one: &masm::Module| {
            Some((
                unpadded(one)?,
                rotation_work(one, options, dimension.prices_proven_trips(), dimension.prices_called_work()),
            ))
        };
        if 2 * size(&base.program.modules[0], Some(&cut)) >= size(&base.program.modules[0], None) {
            let variant = built(program, name, &again)?;
            let pays = matches!(
                (price(&base.built), price(&variant.built)),
                (Some(a), Some(b)) if dimension.keeps(&noted, a, b, cap_of(dimension))
            );
            if pays {
                kept.push((dimension, Ok(variant)));
            }
            continue;
        }
        let each = |program: &Program| assembled_module(&only(&program.modules[0], &cut), program, name, &again).ok();
        let (Some(first), Some(second)) = (each(&base.program), each(&program)) else { continue };
        let pays = matches!(
            (price(&first), price(&second)),
            (Some(a), Some(b)) if dimension.keeps(&noted, a, b, cap_of(dimension))
        );
        if pays {
            kept.push((dimension, Err(program)));
        }
    }
    // The first dimension that stayed is the variant already priced against the
    // base. Each further one is added to it and priced against the code so
    // far: a dimension's gain does not carry over when another changed the
    // loops (x_popcount: the unrotated loops and the hot rotations each
    // paid alone and cost clocks together).
    let mut kept = kept.into_iter();
    let Some((first, variant)) = kept.next() else { return with_leaf_trial(base, name, options, unpadded) };
    let mut accepted = vec![first];
    let mut chosen = match variant {
        Ok(variant) => variant,
        Err(program) => built(program, name, &again)?,
    };
    // OS-OUTLINE-TRIAL is priced before the backend's own trials (IPRA,
    // leaf fold) run, which an outlined call changes (QB m64 scope.c: -12 B
    // priced, +39 B finished). The code without it is kept to decide on the
    // finished modules.
    let mut without_outline = (first == Dimension::OutlineCalls).then(|| base.clone());
    for (dimension, _) in kept {
        accepted.push(dimension);
        let program = compiled_with(&accepted, || optimized_program(module.clone(), &again))?;
        let _ = Noted::take();
        let together = built(program, name, &again)?;
        let price = |one: &masm::Module| {
            Some((
                unpadded(one)?,
                rotation_work(one, options, dimension.prices_proven_trips(), dimension.prices_called_work()),
            ))
        };
        let pays = matches!(
            (price(&chosen.built), price(&together.built)),
            (Some(a), Some(b)) if dimension.keeps(&noted, a, b, cap_of(dimension))
        );
        if pays {
            if dimension == Dimension::OutlineCalls {
                without_outline = Some(chosen.clone());
            }
            chosen = together;
        } else {
            accepted.pop();
        }
    }
    // The stage files written so far are the base compile's: the module the
    // kept decisions made is dumped again.
    if options.dump.is_some() {
        compiled_with(&accepted, || optimized_program(module.clone(), options))?;
    }
    stage_files(options, "", &chosen.built)?;
    let Some(without) = without_outline else { return with_leaf_trial(chosen, name, options, unpadded) };
    let (without, with) =
        (with_leaf_trial(without, name, options, unpadded)?, with_leaf_trial(chosen, name, options, unpadded)?);
    let smaller = matches!(
        (unpadded(&without), unpadded(&with)),
        (Some(a), Some(b)) if a.total() < b.total()
    );
    if llrm_support::debug::enabled("trial") {
        eprintln!("trial outline-calls finished: {}", if smaller { "put back" } else { "kept" });
    }
    let picked = if smaller { without } else { with };
    stage_files(options, "", &picked)?;
    Ok(picked)
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
