//! DEVIATE ROTATION-TRIAL and ROTATION-HOT on arm64 (beyond the pin; owner w11,
//! exit 2026-12-05): the x86 driver's trial (`llrm_codegen::driver::trial`) for
//! the arm64 driver.
//!
//! # How it works
//! At -Os the pass rotates a counted loop whose copied guard does not fold, and
//! leaves an uncounted one (`Tti::rotation_guard_must_fold_at_size`), noting
//! both. A module with a noted loop is compiled again with the decision
//! reversed, and the finished code decides:
//! - the unrotated compile is kept if it is smaller (`keeps_at_size`) and
//!   executes no more;
//! - the compile that also rotates the left loops is kept if it executes less
//!   for at most `ROTATION_HOT_BYTES_PER_LOOP` bytes a loop
//!   (`keeps_rotation_hot`).
//!
//! # Price
//! [`price`]: bytes are four a word plus the pool; work is `executed` of each
//! function after allocation: instructions, memory operands, taken branches and
//! jumps, by the branch odds and proven trips. The count the harness reports is
//! what the keep is checked against, not this model.

use llrm_codegen::backend::executed;
use llrm_transforms::trial;

use crate::emit::Function;
use crate::machine::Platform;

/// What a finished module costs: its bytes and the work it is expected to
/// execute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    pub bytes: i64,
    pub work: f64,
}

thread_local! {
    static PRICED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static UNMERGED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static MERGED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WORK: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Whether the compile that merges no global of an `optsize` function is
/// running.
pub fn unmerged() -> bool {
    UNMERGED.with(std::cell::Cell::get)
}

/// Runs `body` with GlobalMerge as the pin has it: nothing at -Os.
pub fn without_merge<T>(body: impl FnOnce() -> T) -> T {
    let before = UNMERGED.with(|cell| cell.replace(true));
    let value = body();
    UNMERGED.with(|cell| cell.set(before));
    value
}

/// Notes `count` globals GM-OPTSIZE merged that the pin would not.
pub fn note_merged(count: usize) {
    MERGED.with(|cell| cell.set(cell.get() + count));
}

/// The globals noted since the last call.
pub fn take_merged() -> usize {
    MERGED.with(|cell| cell.replace(0))
}

/// Bytes one unmerged global costs: its own `adrp` and `add`.
const BYTES_PER_UNMERGED_GLOBAL: i64 = 8;

/// The model's frequencies are estimates: a gain under this share of the work
/// is noise (jpeg_fixed -Os modelled 0.004% and ran 5,497 instructions more).
const MIN_GAIN: f64 = 0.001;

/// DEVIATE GM-TRIAL: whether the compile that merges nothing at -Os is kept
/// over `kept`, which merged `merged` globals GM-OPTSIZE takes: it executes
/// less, for no more than the `adrp`+`add` each global needs of its own. A
/// merged base reaches its members by offset, so a loop that indexes them pays
/// an `add` a trip (particle -Os: 6.5% more instructions than gcc and clang, 36
/// B smaller).
pub fn keeps_unmerged(
    kept: Price,
    other: Price,
    merged: usize,
) -> bool {
    !(kept.work.is_nan() || other.work.is_nan())
        && kept.work - other.work >= MIN_GAIN * kept.work
        && other.bytes - kept.bytes <= BYTES_PER_UNMERGED_GLOBAL * merged as i64
}

/// Starts summing the work of the functions lowered on this thread.
pub fn start() {
    WORK.with(|work| work.set(Some(0.0)));
}

/// Adds a lowered function's work, where [`start`] was called. The
/// post-allocation body goes through the shared `executed` (branch odds, proven
/// trips), so spill code and reloads are counted.
pub fn add(function: &crate::mfunc::MFunc) {
    if WORK.with(|work| work.get()).is_none() {
        return;
    }
    PRICED.with(|count| count.set(count.get() + 1));
    let done = crate::lir_body::to_lir(function).ok().and_then(|lowered| executed::executed(&lowered.body));
    WORK.with(|work| {
        // A function with no finite estimate poisons the sum: no work saved is
        // claimed.
        work.set(
            work.get()
                .map(|sum| done.map_or(f64::NAN, |one| sum + one.instructions + one.memory + one.taken + one.jumps)),
        );
    });
}

/// How many function bodies this thread has priced: the cost of the trials'
/// pricing, which a module that noted nothing must not pay (compile time x2.5
/// on nbody -Os before).
pub fn priced() -> usize {
    PRICED.with(std::cell::Cell::get)
}

/// The work summed since [`start`].
fn finish() -> f64 {
    WORK.with(|work| work.replace(None)).unwrap_or(f64::NAN)
}

/// What `functions` cost together: bytes as written, and the work [`add`]
/// summed.
pub fn price(functions: &[Function]) -> Price {
    let bytes = functions
        .iter()
        .map(|function| {
            let words: usize = function.blocks.iter().map(crate::emit::words).sum();
            let pool: usize = function.pool.iter().map(|entry| entry.bytes.len()).sum();
            4 * words as i64 + pool as i64
        })
        .sum();
    Price { bytes, work: finish() }
}

/// Whether `other` is kept over `kept` after a compile that left the loops
/// unrotated.
pub fn keeps_unrotated(
    kept: Price,
    other: Price,
    hot: bool,
    loops: usize,
) -> bool {
    if kept.work.is_nan() || other.work.is_nan() {
        return false;
    }
    let added = other.bytes - kept.bytes;
    let saved = (kept.work - other.work).round() as i64;
    // Strict at -Os on both axes: smaller, and not slower.
    trial::keeps_at_size(added)
        && saved >= 0
        && !(hot && trial::keeps_rotation_hot(-saved, -added, loops, trial::ROTATION_HOT_BYTES_PER_LOOP))
}

/// Whether the compile that also rotates the left loops is kept over `kept`.
pub fn keeps_rotated(
    kept: Price,
    other: Price,
    loops: usize,
) -> bool {
    !(kept.work.is_nan() || other.work.is_nan())
        && trial::keeps_rotation_hot(
            (kept.work - other.work).round() as i64,
            other.bytes - kept.bytes,
            loops,
            trial::ROTATION_HOT_BYTES_PER_LOOP,
        )
}

/// Whether `platform`'s target asks for the trial at `size` builds.
pub fn asked(
    platform: Platform,
    size: bool,
) -> Option<bool> {
    if llrm_support::env_flag!("LLRM_ARM64_NO_TRIAL") {
        return None;
    }
    let tti = crate::machine::Arm64Machine { platform };
    let tti = llrm_mir::target::Machine::tti(&tti)?;
    (size && tti.rotation_guard_must_fold_at_size() && tti.rotation_trial_at_size()).then(|| tti.rotation_hot_at_size())
}
