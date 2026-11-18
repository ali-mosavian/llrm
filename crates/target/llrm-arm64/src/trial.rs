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
//!   executes no more, laid out (`Price::laid_out`);
//! - the compile that also rotates the left loops is kept if it executes less
//!   for at most `ROTATION_HOT_BYTES_PER_LOOP` bytes a loop
//!   (`keeps_rotation_hot`).
//!
//! # Price
//! [`price`]: bytes are four a word plus the pool; work is `executed` of each
//! function after allocation: instructions, memory operands, taken branches and
//! jumps, by the branch odds and proven trips. The unrotated decision reads it
//! as laid out ([`add`]): by the odds placement read, a loop's guard entering
//! as often as the loop continues ([`guards`]). Priced before layout, the
//! unrotated recsum read 293 against 136.5 for the same 10005 instructions: a
//! `b` to the next block per trip that placement removes. ROTATION-HOT and
//! GM-TRIAL keep the price before layout their limits were set on. The count
//! the harness reports is what the keep is checked against, not this model.

use llrm_codegen::backend::executed;
use llrm_codegen::model::lir::{BlockOdds, LirBlock, LirBody};
use llrm_support::hash::HashMap;
use llrm_transforms::trial;

use crate::emit::Function;
use crate::frame::Lowered;
use crate::layout::Likelihood;
use crate::machine::Platform;
use crate::mc::inst::Inst;
use crate::mfunc::{MBlock, MFunc, Op};

/// What a finished module costs: its bytes and the work it is expected to
/// execute, before layout (`work`: ROTATION-HOT and GM-TRIAL keep what their
/// limits were set on) and as laid out (`laid_out`: the unrotated trial).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Price {
    pub bytes: i64,
    pub work: f64,
    pub laid_out: f64,
}

thread_local! {
    static PRICED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static UNMERGED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static MERGED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WORK: std::cell::Cell<Option<(f64, f64)>> = const { std::cell::Cell::new(None) };
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
    WORK.with(|work| work.set(Some((0.0, 0.0))));
}

/// Adds a finished function's work, where [`start`] was called, through the
/// shared `executed` (spill code and reloads counted): the machine function
/// before layout, and the blocks layout placed with the odds it placed them by.
pub fn add(
    function: &MFunc,
    blocks: &[Lowered],
    likelihood: &Likelihood,
) {
    if WORK.with(|work| work.get()).is_none() {
        return;
    }
    PRICED.with(|count| count.set(count.get() + 1));
    let work =
        |body: &LirBody| executed::executed(body).map(|one| one.instructions + one.memory + one.taken + one.jumps);
    let before = crate::lir_body::to_lir(function).ok().map(|lowered| lowered.body);
    let early = before.as_ref().and_then(work);
    // A function the layout cannot be read for is priced as it was.
    let placed = finished(function, blocks, likelihood).as_ref().and_then(work).or(early);
    WORK.with(|cell| {
        // A function with no finite estimate poisons the sum: no work saved is
        // claimed.
        cell.set(
            cell.get()
                .map(|(one, two)| (early.map_or(f64::NAN, |now| one + now), placed.map_or(f64::NAN, |now| two + now))),
        );
    });
}

/// The finished `blocks` of `function` as a body: what runs is the placed
/// code's, a loop's test copied into its latch and a `b` to the next block
/// gone, not the selector's order (the unrotated recsum -Os read 293 against
/// the rotated 136.5 for the same 10005 instructions). `None` for a function
/// that jumps through a table, whose edges the blocks do not state, or whose
/// layout kept the selector's order and read no odds.
fn finished(
    function: &MFunc,
    blocks: &[Lowered],
    likelihood: &Likelihood,
) -> Option<LirBody> {
    let indirect = |inst: &Inst| matches!(inst, Inst::Label(_) | Inst::BranchReg { link: false, .. });
    if likelihood.is_empty() || blocks.iter().flat_map(|block| &block.insts).any(indirect) {
        return None;
    }
    let guards = guards(function);
    let mut placed = function.clone();
    placed.alloc_odds = None;
    placed.blocks = blocks
        .iter()
        .map(|block| MBlock {
            label: block.label.clone(),
            ops: block.insts.iter().filter(|inst| !inst.is_directive()).cloned().map(Op::I).collect::<Vec<_>>().into(),
        })
        .collect();
    let mut body = crate::lir_body::to_lir(&placed).ok()?.body;
    let at_of: HashMap<&str, usize> = blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let mut shaped = Vec::with_capacity(blocks.len());
    for (at, block) in blocks.iter().enumerate() {
        let mut succ: Vec<i64> = block
            .insts
            .iter()
            .filter_map(crate::layout::label_of)
            .filter_map(|label| at_of.get(label))
            .map(|&to| to as i64)
            .collect();
        let ends =
            matches!(
                block.insts.last(),
                Some(Inst::Branch { link: false, .. } | Inst::Ret { .. } | Inst::Brk { .. })
            );
        if !ends && at + 1 < blocks.len() {
            succ.push(at as i64 + 1);
        }
        for &to in &succ {
            let edge = (block.label.clone(), blocks[to as usize].label.clone());
            if let Some(&numerator) = likelihood.get(&edge) {
                body.odds.taken.insert((at as i64, to), numerator);
            }
        }
        // The guard may have taken its preheader in: its edge goes to the
        // preheader or the header itself.
        if let Some(guard) = guards.iter().find(|guard| guard.block == block.label)
            && let Some(&continues) = likelihood.get(&guard.latch)
            && let Some(&into) = succ.iter().find(|&&to| guard.enters.contains(&blocks[to as usize].label))
            && let [one, two] = succ[..]
        {
            let out = if one == into { two } else { one };
            body.odds.taken.insert((at as i64, into), continues);
            body.odds.taken.insert((at as i64, out), (BlockOdds::CERTAIN as u32).saturating_sub(continues));
        }
        let was = &body.blocks[at];
        shaped.push(LirBlock { succ: succ.into(), insns: was.insns.clone(), ..LirBlock::new(was.at, Vec::new()) });
    }
    body.blocks = llrm_codegen::model::lir::shared(shaped);
    Some(body)
}

/// A loop's guard: the block whose branch enters the loop or goes around it,
/// the labels its edge into the loop can have (the preheader, the header), and
/// the latch's edge back to the header.
struct Guard {
    block: String,
    enters: [String; 2],
    latch: (String, String),
}

/// DEVIATE ROTATION-TRIAL (beyond the pin; owner w11, exit 2026-12-05): the
/// guard in front of a loop enters it as often as the loop continues. The
/// pin's odds give a copied guard's test its zero heuristic (5 in 8) and the
/// header test of an unrotated loop its loop-exit odds (31 in 32), so one loop
/// read 20 trips rotated and 31 unrotated, and the same instructions weighed
/// 1.5 times as much. A guard is the one branch before a loop's preheader
/// (`Loop::getLoopGuardBranch` without its check that the other edge meets the
/// loop's exit: the exit's code may stay out of the zero-trip path).
fn guards(function: &MFunc) -> Vec<Guard> {
    use llrm_codegen::analysis::loops;
    let Ok(lowered) = crate::lir_body::to_lir(function) else { return Vec::new() };
    let body = lowered.body;
    let mut preds: Vec<Vec<i64>> = vec![Vec::new(); body.blocks.len()];
    for block in &body.blocks {
        for &to in block.succ.iter() {
            preds[to as usize].push(block.at);
        }
    }
    let label = |at: i64| function.blocks[at as usize].label.clone();
    let mut out = Vec::new();
    for found in loops::loops(&body.blocks, Some(body.entry)) {
        let &[latch] = found.latches.iter().copied().collect::<Vec<_>>().as_slice() else { continue };
        let outside: Vec<i64> =
            preds[found.header as usize].iter().copied().filter(|one| !found.body.contains(one)).collect();
        let &[pre] = outside.as_slice() else { continue };
        let (&[guard], true) = (preds[pre as usize].as_slice(), body.blocks[pre as usize].succ[..] == [found.header])
        else {
            continue;
        };
        let mut around: Vec<i64> = body.blocks[guard as usize].succ.to_vec();
        around.dedup();
        if around.len() == 2 && !found.body.contains(&guard) {
            out.push(Guard {
                block: label(guard),
                enters: [label(pre), label(found.header)],
                latch: (label(latch), label(found.header)),
            });
        }
    }
    out
}

/// How many function bodies this thread has priced: the cost of the trials'
/// pricing, which a module that noted nothing must not pay (compile time x2.5
/// on nbody -Os before).
pub fn priced() -> usize {
    PRICED.with(std::cell::Cell::get)
}

/// The work summed since [`start`], before layout and laid out.
fn finish() -> (f64, f64) {
    WORK.with(|work| work.replace(None)).unwrap_or((f64::NAN, f64::NAN))
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
    let (work, laid_out) = finish();
    Price { bytes, work, laid_out }
}

/// Whether `other` is kept over `kept` after a compile that left the loops
/// unrotated.
pub fn keeps_unrotated(
    kept: Price,
    other: Price,
    hot: bool,
    loops: usize,
) -> bool {
    if kept.laid_out.is_nan() || other.laid_out.is_nan() || kept.work.is_nan() || other.work.is_nan() {
        return false;
    }
    let added = other.bytes - kept.bytes;
    let slower = (kept.laid_out - other.laid_out).round() < 0.0;
    let saved = (kept.work - other.work).round() as i64;
    // Strict at -Os on both axes: smaller, and not slower.
    trial::keeps_at_size(added)
        && !slower
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
