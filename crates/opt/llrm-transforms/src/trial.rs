//! The state a target's driver and a pass share for a trial compile: the module
//! is compiled again with one decision of the pass reversed, and the driver
//! keeps whichever result it prices smaller or cheaper. Two trials use it,
//! UNROLL-TRIAL and ROTATION-TRIAL.
//!
//! DEVIATE UNROLL-TRIAL (beyond the pin; owner 6502, exit 2026-12-05): the
//! state the target's driver and the pass share for a trial unroll.
//!
//! The pin decides a full unroll by a threshold and by what the loop's own
//! body folds (`analyzeLoopUnrollCost`, at most ten trips). A loop whose
//! copies fold the loads of a later loop (an array filled by one loop and read
//! by the next) is seen by neither: the pin unrolls it only because its
//! threshold is large in its own units. Here the pass notes that it refused a
//! loop the target's trial size admits; the driver then compiles the program
//! again with every loop under that size unrolled and keeps whichever
//! assembly it prices cheaper.
//!
//! # Example
//! A loop the pass refused to unroll is noted; the driver compiles again with
//! it unrolled and keeps the cheaper assembly.
//!
//! ```text
//! pass:   loop refused by the threshold -> noted (ELIGIBLE)
//! driver: compile 1 (loop kept) vs compile 2 (loop unrolled)
//!         -> keeps whichever assembly it prices cheaper
//! ```

use std::cell::{Cell, RefCell};

thread_local! {
    /// The unrolled size under which every loop is unrolled, set while the
    /// trial compile runs.
    static TRIAL: Cell<Option<u32>> = const { Cell::new(None) };
    /// The functions with a loop the threshold refused that a trial may take,
    /// and the unrolled size of those loops together.
    static ELIGIBLE: RefCell<(Vec<String>, u64)> = const { RefCell::new((Vec::new(), 0)) };
    /// Set while the trial compile of ROTATION-TRIAL runs: a counted loop whose
    /// copied guard does not fold stays unrotated.
    static UNROTATED: Cell<bool> = const { Cell::new(false) };
    /// The functions with a loop that ROTATION-TRIAL rotated and a trial may
    /// leave unrotated.
    static ROTATED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The loops ROTATION-TRIAL noted, counted apart from the functions they
    /// are in.
    static ROTATED_LOOPS: Cell<usize> = const { Cell::new(0) };
    /// Set while the trial compile of ROTATION-HOT runs: a loop whose copied
    /// guard does not fold and whose trips are not counted is rotated as well.
    static MORE: Cell<bool> = const { Cell::new(false) };
    /// The functions, and loops together, ROTATION-HOT left unrotated.
    static LEFT: RefCell<(Vec<String>, usize)> = const { RefCell::new((Vec::new(), 0)) };
    /// Set while the trial compile of JT-LATCH-TRIAL runs: JumpThreading does
    /// not copy a loop latch.
    static LATCH_KEPT: Cell<bool> = const { Cell::new(false) };
    /// The functions in which JumpThreading copied a latch.
    static LATCH_COPIED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Set while the trial compile of SWITCH-LOOKUP-TRIAL runs: no switch is
    /// turned into a lookup table.
    static LOOKUPS_REFUSED: Cell<bool> = const { Cell::new(false) };
    /// The lookup tables the base compile built.
    static LOOKUPS_BUILT: Cell<usize> = const { Cell::new(0) };
    /// Set while the trial compile of LIR-COPY-TRIAL runs: LoopIdiom leaves a
    /// copy loop of an unknown count as a loop.
    static COPY_LOOPS_KEPT: Cell<bool> = const { Cell::new(false) };
    /// The functions in which LoopIdiom turned such a loop into a `memcpy`.
    static COPY_CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Set while the trial compile of INLINEHINT-GROWTH-TRIAL runs: the -Os
    /// size guard binds an `inlinehint` callee as it does any other.
    static HINTED_CALLS_KEPT: Cell<bool> = const { Cell::new(false) };
    static NEGATIVE_CALLS_KEPT: Cell<bool> = const { Cell::new(false) };
    static NEGATIVE_YIELDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The callers with a call of an `inlinehint` callee that only the hint's
    /// yield let the size guard pass.
    static HINT_YIELDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Set while the trial compile of HOT-INLINE-TRIAL runs: the -Os size
    /// bound holds at a locally hot site too.
    static HOT_CALLS_KEPT: Cell<bool> = const { Cell::new(false) };
    /// The callers with a call that only the hot-site yield let the size bound
    /// pass.
    static HOT_YIELDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Set while the trial compile of IPRA-OS-TRIAL runs: a function that
    /// binds locally takes part in the callee-written-register facts.
    static IPRA_BINDS_LOCAL: Cell<bool> = const { Cell::new(false) };
    /// Whether a module compiled since the last take had a locally binding
    /// function a caller in it calls.
    static IPRA_MATTERS: Cell<bool> = const { Cell::new(false) };
}

/// DEVIATE IPRA-OS-TRIAL: whether the trial compile that lets locally binding
/// functions take part in the callee-written-register facts is running.
pub fn ipra_binds_local() -> bool {
    IPRA_BINDS_LOCAL.with(Cell::get)
}

/// Runs `body` with those functions taking part.
pub fn with_ipra_binds_local<T>(body: impl FnOnce() -> T) -> T {
    let before = IPRA_BINDS_LOCAL.with(|cell| cell.replace(true));
    let value = body();
    IPRA_BINDS_LOCAL.with(|cell| cell.set(before));
    value
}

/// Notes that the facts would differ for the module being compiled.
pub fn note_ipra_matters() {
    IPRA_MATTERS.with(|cell| cell.set(true));
}

/// Whether [`note_ipra_matters`] was called since the last call.
pub fn take_ipra_matters() -> bool {
    IPRA_MATTERS.with(|cell| cell.replace(false))
}

/// DEVIATE HOT-INLINE-TRIAL (beyond the pin; owner x86-m32-b, exit
/// 2026-12-12): whether the trial compile that keeps the call at a site the
/// hot-site yield (`Tuning::inline_size_hot_growth_percent`) would inline is
/// running. The units' growth credits the callee's death over all its sites,
/// which cold sites that keep it deny, so the finished object decides.
pub fn keeping_hot_calls() -> bool {
    HOT_CALLS_KEPT.with(Cell::get)
}

/// Runs `body` with those calls kept.
pub fn with_kept_hot_calls<T>(body: impl FnOnce() -> T) -> T {
    let before = HOT_CALLS_KEPT.with(|cell| cell.replace(true));
    let value = body();
    HOT_CALLS_KEPT.with(|cell| cell.set(before));
    value
}

/// Notes that the hot-site yield let a call in `function` through the size
/// bound, which a trial may refuse.
pub(crate) fn note_hot_yield(function: &str) {
    HOT_YIELDS.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The callers [`note_hot_yield`] saw since the last call.
pub fn take_hot_yields() -> Vec<String> {
    HOT_YIELDS.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// DEVIATE INLINEHINT-GROWTH-TRIAL (beyond the pin; gcc's `-Os` inliner takes
/// a callee only while the unit does not grow, `ipa-inline.cc`
/// `want_inline_small_function_p` over `estimate_edge_growth`; owner x86-m32,
/// exit 2026-11-30): whether the trial compile that keeps the call of an
/// `inlinehint` callee the size guard would refuse is running. The pin's hint
/// threshold (325) lets the callee in whatever it costs in bytes; the units
/// cannot say what the copies fold to, so the finished object decides.
pub fn keeping_hinted_calls() -> bool {
    HINTED_CALLS_KEPT.with(Cell::get)
}

/// Runs `body` with those calls kept.
pub fn with_kept_hinted_calls<T>(body: impl FnOnce() -> T) -> T {
    let before = HINTED_CALLS_KEPT.with(|cell| cell.replace(true));
    let value = body();
    HINTED_CALLS_KEPT.with(|cell| cell.set(before));
    value
}

/// Whether the trial compile that also keeps the calls the size guard's
/// negative-cost yield lets through is running (see [`keeping_hinted_calls`]):
/// a one-line accessor the pin's cost calls a shrinking inline (`code_byte`,
/// `if (c->out >= c->lim) ...`), and gcc's `-Os` outlines.
pub fn keeping_negative_calls() -> bool {
    NEGATIVE_CALLS_KEPT.with(Cell::get)
}

/// Runs `body` with those calls kept.
pub fn with_kept_negative_calls<T>(body: impl FnOnce() -> T) -> T {
    let before = NEGATIVE_CALLS_KEPT.with(|cell| cell.replace(true));
    let value = body();
    NEGATIVE_CALLS_KEPT.with(|cell| cell.set(before));
    value
}

/// Notes that the negative-cost yield let a call in `function` through the
/// size guard, which a trial may refuse.
pub(crate) fn note_negative_yield(function: &str) {
    NEGATIVE_YIELDS.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The callers [`note_negative_yield`] saw since the last call.
pub fn take_negative_yields() -> Vec<String> {
    NEGATIVE_YIELDS.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Notes that the hint's yield let a call in `function` through the size
/// guard, which a trial may refuse.
pub(crate) fn note_hint_yield(function: &str) {
    HINT_YIELDS.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The callers [`note_hint_yield`] saw since the last call.
pub fn take_hint_yields() -> Vec<String> {
    HINT_YIELDS.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// DEVIATE SWITCH-LOOKUP-TRIAL (beyond the pin; gcc's `-Os` keeps the compares
/// of a sparse switch; owner x86-m32, exit 2026-11-30): whether the trial
/// compile that builds no lookup table is running. The table is data bytes
/// against the compares' code bytes, which only the object prices.
pub fn refusing_lookup_tables() -> bool {
    LOOKUPS_REFUSED.with(Cell::get)
}

/// Runs `body` with no switch turned into a lookup table.
pub fn with_refused_lookup_tables<T>(body: impl FnOnce() -> T) -> T {
    let before = LOOKUPS_REFUSED.with(|cell| cell.replace(true));
    let value = body();
    LOOKUPS_REFUSED.with(|cell| cell.set(before));
    value
}

/// Notes a lookup table the base compile built, which a trial may refuse.
pub(crate) fn note_lookup_table() {
    LOOKUPS_BUILT.with(|count| count.set(count.get() + 1));
}

/// The tables [`note_lookup_table`] saw since the last call.
pub fn take_lookup_tables() -> usize {
    LOOKUPS_BUILT.with(|count| count.replace(0))
}

/// DEVIATE LIR-COPY-TRIAL (beyond the pin; owner x86-m32, exit 2026-11-30):
/// whether the trial compile that keeps copy loops of an unknown count is
/// running. The pin calls `memcpy` for them; whether the call is fewer bytes
/// than the loop depends on the sequence around it, so the object decides.
pub fn keeping_copy_loops() -> bool {
    COPY_LOOPS_KEPT.with(Cell::get)
}

/// Runs `body` with those copy loops kept.
pub fn with_kept_copy_loops<T>(body: impl FnOnce() -> T) -> T {
    let before = COPY_LOOPS_KEPT.with(|cell| cell.replace(true));
    let value = body();
    COPY_LOOPS_KEPT.with(|cell| cell.set(before));
    value
}

/// Notes that LoopIdiom made a `memcpy` of a copy loop of `function`, which a
/// trial may refuse.
pub(crate) fn note_copy_call(function: &str) {
    COPY_CALLS.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The functions [`note_copy_call`] saw since the last call.
pub fn take_copy_calls() -> Vec<String> {
    COPY_CALLS.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// DEVIATE JT-LATCH-TRIAL (beyond the pin; gcc's `-Os` backward threader
/// refuses a path through a loop latch; owner x86-m32, exit 2026-11-30):
/// whether the trial compile that does not copy a loop latch is running.
pub fn keeping_latches() -> bool {
    LATCH_KEPT.with(Cell::get)
}

/// Runs `body` with loop latches kept in one copy.
pub fn with_kept_latches<T>(body: impl FnOnce() -> T) -> T {
    let before = LATCH_KEPT.with(|cell| cell.replace(true));
    let value = body();
    LATCH_KEPT.with(|cell| cell.set(before));
    value
}

/// Notes that JumpThreading copied a loop latch of `function`, which a trial
/// may refuse.
pub(crate) fn note_latch_copy(function: &str) {
    LATCH_COPIED.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The functions [`note_latch_copy`] saw since the last call.
pub fn take_latch_copies() -> Vec<String> {
    LATCH_COPIED.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

thread_local! {
    /// Set while the trial compile of LOOP-COPY-TRIAL runs: a callee that holds
    /// a loop is not inlined at several sites of a loop of its caller.
    static REFUSE_LOOP_COPIES: Cell<bool> = const { Cell::new(false) };
    /// The inlines of that kind the pass made since [`take_loop_copies`].
    static LOOP_COPIES: Cell<usize> = const { Cell::new(0) };
}

/// DEVIATE LOOP-COPY-TRIAL (beyond the pin; owner z80, exit 2026-11-24):
/// whether the trial compile that refuses the copies of a loop-holding callee
/// at several sites inside a loop is running. The pin's InlineCost prices the
/// call away and the callee's body, never the registers the merged loop needs
/// (its `LoopPenalty` binds minsize callers only): on the z80 each copy spilled
/// its pointers and the module came out larger and slower (idioms -O2: 4555 B /
/// 25.58M T against 1993 B / 24.49M T).
pub fn refusing_loop_copies() -> bool {
    REFUSE_LOOP_COPIES.with(Cell::get)
}

/// Runs `body` with those copies refused.
pub fn with_refused_loop_copies<T>(body: impl FnOnce() -> T) -> T {
    let before = REFUSE_LOOP_COPIES.with(|cell| cell.replace(true));
    let value = body();
    REFUSE_LOOP_COPIES.with(|cell| cell.set(before));
    value
}

/// Notes one inline the trial may refuse.
pub(crate) fn note_loop_copy() {
    LOOP_COPIES.with(|count| count.set(count.get() + 1));
}

/// The inlines [`note_loop_copy`] saw since the last call.
pub fn take_loop_copies() -> usize {
    LOOP_COPIES.with(|count| count.replace(0))
}

/// The trial threshold in force, if a trial compile is running.
pub fn active() -> Option<u32> {
    TRIAL.with(Cell::get)
}

/// Runs `body` with the trial threshold `limit` in force.
pub fn with<T>(
    limit: u32,
    body: impl FnOnce() -> T,
) -> T {
    let before = TRIAL.with(|cell| cell.replace(Some(limit)));
    let value = body();
    TRIAL.with(|cell| cell.set(before));
    value
}

/// Notes that `function` has a loop of `size` unrolled that the threshold
/// refused and a trial may take.
pub(crate) fn note(
    function: &str,
    size: u64,
) {
    ELIGIBLE.with(|list| {
        let mut list = list.borrow_mut();
        if !list.0.iter().any(|one| one == function) {
            list.0.push(function.to_owned());
        }
        list.1 += size;
    });
}

/// The functions noted since the last call, and the unrolled size of their
/// loops together.
pub fn take() -> (Vec<String>, u64) {
    ELIGIBLE.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Puts back what [`take`] returned, for the compile whose candidates they
/// were.
pub fn restore(noted: (Vec<String>, u64)) {
    ELIGIBLE.with(|list| *list.borrow_mut() = noted);
}

/// Whether a trial unroll is kept at -O1: the code is expected to execute no
/// more (`saved` >= 0, work being what the code is expected to execute) and
/// adds no bytes (`added` <= 0). Where the target prices bytes in clocks at -O1
/// (`rate`, thousandths of a clock a byte, `Tti::unroll_milliclocks_per_byte`;
/// 0 where it does not), an unroll that adds bytes is kept too if the work it
/// saves repays them, as the -O2 unroll is (`unroll_pays`): -O1 is about clocks
/// since the ruling of 2026-11-14, which replaced the strict rule of each axis.
/// The one rule of every target's -O1 trial (DEVIATE UNROLL-TRIAL,
/// 6502-UNROLL-O1, UNROLL-O1-TRIAL).
pub fn keeps_at_o1(
    saved: i64,
    added: i64,
    rate: i64,
) -> bool {
    if added <= 0 {
        return saved >= 0;
    }
    rate > 0 && saved > 0 && i128::from(saved) * 1000 >= i128::from(added) * i128::from(rate)
}

#[cfg(test)]
mod tests {
    use super::{keeps_at_o1, keeps_smaller_at_size};

    /// jpeg_fixed -Os after an address-pair change: the work model priced one
    /// shape at 2.5e17 against 4e10 for the other, and `-saved * 100`
    /// overflowed i64 (a debug panic; a release wrap kept or dropped the
    /// trial at random). The work an astronomically worse shape adds is
    /// over any cap.
    #[test]
    fn extreme_work_does_not_overflow_the_trial_rules() {
        assert!(!keeps_smaller_at_size(-4, -250_727_428_995_872_928, 39_774_383_718, Some(1)));
        assert!(!keeps_smaller_at_size(-4, i64::MIN, i64::MAX, Some(1)));
        assert!(keeps_smaller_at_size(-4, i64::MAX, i64::MAX, Some(1)), "it runs less");
        assert!(keeps_at_o1(i64::MAX, 4, i64::MAX));
        assert!(!keeps_at_o1(1, i64::MAX, i64::MAX));
    }

    /// x_transpose -Os (m32): the copy loop saved 20 B and added 178,500 of
    /// about 1.1M work (+16%); x_switch_ranges: 35 B for +11%.
    #[test]
    fn a_smaller_shape_is_kept_only_within_the_clock_cap() {
        assert!(!keeps_smaller_at_size(-20, -178_500, 1_100_000, Some(2)));
        assert!(keeps_smaller_at_size(-20, -178_500, 1_100_000, None), "no cap: bytes alone");
        assert!(keeps_smaller_at_size(-23, -1_000, 1_100_000, Some(2)));
        assert!(keeps_smaller_at_size(-4, 15, 1_100_000, Some(2)), "it runs less");
        assert!(!keeps_smaller_at_size(0, 100, 1_100_000, Some(2)), "no byte saved");
        assert!(keeps_smaller_at_size(-4, -50, 0, Some(2)), "no work known: bytes alone");
    }

    /// x_life -O1 (m32): the 3x3 nest unrolled ran half the work (212,914
    /// against 425,704) for 32 B more once the outer loops' counters
    /// spilled; the strict rule put it back and the nest ran twice as long. 312
    /// is the 486's byte rate.
    #[test]
    fn an_unroll_that_halves_the_work_is_kept_for_a_few_bytes_where_bytes_are_priced() {
        assert!(keeps_at_o1(212_790, 32, 312));
        assert!(!keeps_at_o1(212_790, 32, 0), "the strict rule of a target that prices no bytes");
    }

    /// matmul -O1 (m32): +1601 B for a work saving the 486's byte rate does not
    /// repay.
    #[test]
    fn an_unroll_that_does_not_repay_its_bytes_is_put_back() {
        assert!(!keeps_at_o1(400, 1601, 312));
        assert!(keeps_at_o1(0, -3, 312) && keeps_at_o1(0, 0, 0) && !keeps_at_o1(-1, -3, 312));
    }
}

/// DEVIATE ROTATION-TRIAL (beyond the pin; gcc's `-Os` leaves a counted loop
/// whose guard does not fold unrotated, `ch_base::copy_headers`; owner
/// greedy-os-causes, exit 2026-12-05): whether the trial compile that leaves
/// such loops unrotated is running.
pub fn unrotated() -> bool {
    UNROTATED.with(Cell::get)
}

/// Runs `body` with counted loops of an unfolded guard left unrotated.
pub fn with_unrotated<T>(body: impl FnOnce() -> T) -> T {
    let before = UNROTATED.with(|cell| cell.replace(true));
    let value = body();
    UNROTATED.with(|cell| cell.set(before));
    value
}

/// Notes that `function` has a counted loop of an unfolded guard that was
/// rotated and a trial may leave as it was.
pub(crate) fn note_rotated(function: &str) {
    ROTATED_LOOPS.with(|count| count.set(count.get() + 1));
    ROTATED.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The loops [`note_rotated`] saw since [`clear_rotated_loops`].
pub fn rotated_loops() -> usize {
    ROTATED_LOOPS.with(Cell::get)
}

/// Forgets the loops counted for an earlier compile.
pub fn clear_rotated_loops() {
    ROTATED_LOOPS.with(|count| count.set(0));
}

/// The functions noted by [`note_rotated`] since the last call.
pub fn take_rotated() -> Vec<String> {
    ROTATED.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// DEVIATE ROTATION-HOT (beyond the pin; owner x86-m32, exit 2026-11-24):
/// whether the trial compile that also rotates the loops of an unfolded guard
/// and uncounted trips is running. The pin rotates them at -Os; RS1 leaves
/// them, and the finished code decides which pays (hot loops: fewer executed
/// instructions for a few bytes).
pub fn rotates_more() -> bool {
    MORE.with(Cell::get)
}

/// Runs `body` with those loops rotated as well.
pub fn with_more_rotation<T>(body: impl FnOnce() -> T) -> T {
    let before = MORE.with(|cell| cell.replace(true));
    let value = body();
    MORE.with(|cell| cell.set(before));
    value
}

/// Notes that `function` has a loop RS1 left unrotated that
/// [`with_more_rotation`] would rotate.
pub(crate) fn note_left(function: &str) {
    LEFT.with(|list| {
        let mut list = list.borrow_mut();
        if !list.0.iter().any(|one| one == function) {
            list.0.push(function.to_owned());
        }
        list.1 += 1;
    });
}

/// The functions and loops noted by [`note_left`] since the last call.
pub fn take_left() -> (Vec<String>, usize) {
    LEFT.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Bytes a loop that ROTATION-HOT rotates may add (a copied guard: a compare
/// and a jump), where the target (`Tti::rotation_hot_bytes_per_loop`) does not
/// say.
pub const ROTATION_HOT_BYTES_PER_LOOP: i64 = 4;

/// Whether the ROTATION-HOT compile is kept: it executes less, and grows the
/// code by no more than `bytes` for each loop it rotated.
pub fn keeps_rotation_hot(
    saved: i64,
    added: i64,
    loops: usize,
    bytes: i64,
) -> bool {
    saved > 0 && added <= bytes * loops as i64
}

/// A function of more instructions than this is not priced twice: the second
/// compile of a big function costs more than the loops in it can save
/// (known_loads_walks.c -Os: one 6219-instruction `main`, 2.27x the
/// compile time, put back). The functions the trial leaves unrotated hold up to
/// 2038 (x_jpeg_fixed -Os, -213 B), so 1024 gave that away.
pub const ROTATION_TRIAL_MAX_INSTRUCTIONS: usize = 4096;

/// Whether the unrotated trial is kept at -Os, where bytes are the one strict
/// axis: it is smaller. A tie keeps the pass's own result.
pub fn keeps_at_size(added: i64) -> bool {
    added < 0
}

/// DEVIATE OS-TRIAL-CLOCK-CAP: whether a trial that keeps a smaller shape at
/// -Os is kept. The object is smaller (`added` < 0) and, where the target caps
/// the clocks a trial may cost (`cap`, percent of the code's work without it,
/// `base`), the work it adds (`-saved`) stays within the cap. Where the model
/// has no work for either shape (`base` 0) bytes alone decide, as without a
/// cap.
pub fn keeps_smaller_at_size(
    added: i64,
    saved: i64,
    base: i64,
    cap: Option<i64>,
) -> bool {
    if added >= 0 {
        return false;
    }
    match cap {
        Some(cap) if base > 0 && saved < 0 => -i128::from(saved) * 100 <= i128::from(cap) * i128::from(base),
        _ => true,
    }
}

/// DEVIATE UNROLL-OS-TRIAL: whether the -Os unroll trial is kept: the object is
/// smaller, or it grows by at most `ROTATION_HOT_BYTES_PER_LOOP` for each
/// function the trial took and the code runs less. `bytes_strict`
/// (`Tuning::unroll_trial_bytes_strict`): bytes are the one axis at -Os, so
/// only the smaller object is kept.
pub fn keeps_unroll_at_size(
    saved: i64,
    added: i64,
    functions: usize,
    bytes_strict: bool,
) -> bool {
    added < 0 || (!bytes_strict && saved > 0 && added <= ROTATION_HOT_BYTES_PER_LOOP * functions as i64)
}

/// The names of the functions whose finished bodies differ between two modules
/// of one source (one that only one of them defines counts).
pub fn differing(
    one: &llrm_mir::module::Module,
    other: &llrm_mir::module::Module,
) -> std::collections::BTreeSet<String> {
    // Printed, not compared: a constant's id means a value only within its own
    // module.
    let bodies = |module: &llrm_mir::module::Module| -> std::collections::BTreeMap<String, Option<String>> {
        module
            .globals
            .iter()
            .filter_map(|global| {
                let function = global.function().filter(|one| !one.is_declaration());
                Some((
                    global.name.as_deref()?.to_owned(),
                    function.map(|one| llrm_mir::print::body(&module.context, one)),
                ))
            })
            .collect()
    };
    let (one, other) = (bodies(one), bodies(other));
    one.keys().chain(other.keys()).filter(|name| one.get(*name) != other.get(*name)).cloned().collect()
}

/// `module` with every function outside `keep` cut to its declaration: the
/// backend then works on `keep` alone, in a module whose other functions stand
/// as they do in both versions.
pub fn only(
    module: &llrm_mir::module::Module,
    keep: &std::collections::BTreeSet<String>,
) -> llrm_mir::module::Module {
    let mut cut = module.clone();
    for global in &mut cut.globals {
        let named = global.name.as_deref().is_some_and(|name| keep.contains(name));
        if let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind
            && !named
            && !function.is_declaration()
        {
            **function = function.declaration();
        }
    }
    cut
}

/// DEVIATE OS-INLINE-TRIAL (beyond the pin; owner 6502-c, exit 2026-12-05): a
/// call site the -Os byte model refused within its own error, to be inlined in
/// a second compile and kept if the finished functions of its chain are
/// smaller. The model is fit to the bytes the backend writes
/// (`Tti::opt_size_inline_trial_by_bytes`) and is within 25% of them for two
/// functions in three, so a copy it refuses by less than that is the backend's
/// to decide. `caller` and `callees` are names: the compile is deterministic up
/// to the site, so the same site is found again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub caller: String,
    pub callees: Vec<String>,
}

thread_local! {
    /// The sites the second compile inlines, while it runs.
    static INLINED: RefCell<Vec<Site>> = const { RefCell::new(Vec::new()) };
    /// The sites a compile refused within the model's error.
    static REFUSED: RefCell<Vec<Site>> = const { RefCell::new(Vec::new()) };
}

/// Runs `body` with `sites` inlined whenever the pass reaches them.
pub fn with_inlined<T>(
    sites: &[Site],
    body: impl FnOnce() -> T,
) -> T {
    let before = INLINED.with(|cell| cell.replace(sites.to_vec()));
    let value = body();
    INLINED.with(|cell| *cell.borrow_mut() = before);
    value
}

/// Whether the trial compile in force inlines this site.
pub(crate) fn inlines(
    caller: &str,
    callees: &[String],
) -> bool {
    INLINED.with(|list| list.borrow().iter().any(|one| one.caller == caller && one.callees == callees))
}

thread_local! {
    /// Set while the compile that inlines as the pin's threshold alone says
    /// runs.
    static PIN_INLINES: Cell<bool> = const { Cell::new(false) };
}

/// Whether the byte rate that refuses a growing inline is off, for the compile
/// of the pin's own inline decisions.
pub fn pinning_inlines() -> bool {
    PIN_INLINES.with(Cell::get)
}

/// Runs `body` with every inline the pin's threshold takes made, whatever it
/// grows.
pub fn with_pin_inlines<T>(body: impl FnOnce() -> T) -> T {
    let before = PIN_INLINES.with(|cell| cell.replace(true));
    let value = body();
    PIN_INLINES.with(|cell| cell.set(before));
    value
}

thread_local! {
    /// The sites the second compile keeps as calls, while it runs.
    static OUTLINED: RefCell<Vec<Site>> = const { RefCell::new(Vec::new()) };
    /// The sites a compile accepted within the byte model's error, at -Os.
    static ACCEPTED: RefCell<Vec<Site>> = const { RefCell::new(Vec::new()) };
}

/// Runs `body` with `sites` kept as calls whenever the pass reaches them.
pub fn with_outlined<T>(
    sites: &[Site],
    body: impl FnOnce() -> T,
) -> T {
    let before = OUTLINED.with(|cell| cell.replace(sites.to_vec()));
    let value = body();
    OUTLINED.with(|cell| *cell.borrow_mut() = before);
    value
}

/// Whether the trial compile in force keeps this site a call.
pub(crate) fn outlines(
    caller: &str,
    callees: &[String],
) -> bool {
    OUTLINED.with(|list| list.borrow().iter().any(|one| one.caller == caller && one.callees == callees))
}

/// Notes a site accepted at -Os, a candidate for [`with_outlined`].
pub(crate) fn note_accepted(site: Site) {
    ACCEPTED.with(|list| {
        let mut list = list.borrow_mut();
        if !list.contains(&site) {
            list.push(site);
        }
    });
}

/// The accepted sites noted since the last call.
pub fn take_accepted() -> Vec<Site> {
    ACCEPTED.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Sites that share a function (a caller or a callee) are decided together: the
/// functions they name, and which of the refused and the accepted sites (by
/// index) they hold.
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub names: std::collections::BTreeSet<String>,
    pub refused: Vec<usize>,
    pub accepted: Vec<usize>,
}

/// The groups of `refused` and `accepted` sites, sites linked by a shared
/// function name in one group.
pub fn groups(
    refused: &[Site],
    accepted: &[Site],
) -> Vec<Group> {
    let mut found: Vec<Group> = Vec::new();
    for (index, site, is_refused) in refused
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s, true))
        .chain(accepted.iter().enumerate().map(|(i, s)| (i, s, false)))
    {
        let names: Vec<&String> = std::iter::once(&site.caller).chain(&site.callees).collect();
        let (hit, rest): (Vec<Group>, Vec<Group>) =
            found.into_iter().partition(|group| names.iter().any(|name| group.names.contains(*name)));
        found = rest;
        let mut merged = Group::default();
        for group in hit {
            merged.names.extend(group.names);
            merged.refused.extend(group.refused);
            merged.accepted.extend(group.accepted);
        }
        merged.names.extend(names.into_iter().cloned());
        if is_refused {
            merged.refused.push(index)
        } else {
            merged.accepted.push(index)
        }
        found.push(merged);
    }
    found
}

/// Whether a trial compile is running.
pub(crate) fn inlining() -> bool {
    INLINED.with(|list| !list.borrow().is_empty())
}

/// Notes a site refused within the model's error.
pub(crate) fn note_refused(site: Site) {
    REFUSED.with(|list| {
        let mut list = list.borrow_mut();
        if !list.contains(&site) {
            list.push(site);
        }
    });
}

/// The sites noted since the last call.
pub fn take_refused() -> Vec<Site> {
    REFUSED.with(|list| std::mem::take(&mut *list.borrow_mut()))
}

/// Which of the sites' groups the finished code keeps: sites that share a
/// function (a caller or a callee) are one group, kept whole if the bytes of
/// its functions are fewer in the second compile (`other`) than in the first
/// (`kept`; a function a compile does not define is 0). A group whose
/// caller one compile did not define is not priced: another inline took it, and
/// its bytes went with that. Returns one flag per site.
pub fn accepted(
    sites: &[Site],
    kept: &std::collections::BTreeMap<String, i64>,
    other: &std::collections::BTreeMap<String, i64>,
) -> Vec<bool> {
    // union-find by shared name
    let mut group: Vec<usize> = (0..sites.len()).collect();
    for first in 0..sites.len() {
        for second in first + 1..sites.len() {
            if names(&sites[first]).iter().any(|name| names(&sites[second]).contains(name)) {
                let (a, b) = (root(&group, first), root(&group, second));
                group[a.max(b)] = a.min(b);
            }
        }
    }
    let roots: Vec<usize> = (0..sites.len()).map(|one| root(&group, one)).collect();
    (0..sites.len())
        .map(|site| {
            let members: Vec<&Site> =
                (0..sites.len()).filter(|one| roots[*one] == roots[site]).map(|one| &sites[one]).collect();
            if members.iter().any(|one| !kept.contains_key(&one.caller) || !other.contains_key(&one.caller)) {
                return false;
            }
            let mut every: Vec<&str> = members.iter().flat_map(|one| names(one)).collect();
            every.sort_unstable();
            every.dedup();
            let sum = |bytes: &std::collections::BTreeMap<String, i64>| -> i64 {
                every.iter().map(|name| bytes.get(*name).copied().unwrap_or(0)).sum()
            };
            sum(other) < sum(kept)
        })
        .collect()
}

/// The caller and the callees of `site`.
fn names(site: &Site) -> Vec<&str> {
    std::iter::once(site.caller.as_str()).chain(site.callees.iter().map(String::as_str)).collect()
}

fn root(
    group: &[usize],
    mut one: usize,
) -> usize {
    while group[one] != one {
        one = group[one];
    }
    one
}

#[cfg(test)]
mod inline_tests {
    use std::collections::BTreeMap;

    use super::*;

    fn bytes(list: &[(&str, i64)]) -> BTreeMap<String, i64> {
        list.iter().map(|(name, n)| ((*name).to_owned(), *n)).collect()
    }

    fn site(
        caller: &str,
        callees: &[&str],
    ) -> Site {
        Site { caller: caller.to_owned(), callees: callees.iter().map(|one| (*one).to_owned()).collect() }
    }

    /// memcpy_libcall -Os: the model read the copies of `same` as +52 B, the
    /// backend writes -58 B once `same` is gone. The chain's functions are
    /// priced together; the callee's bytes leave with it.
    #[test]
    fn a_chain_whose_functions_are_fewer_bytes_is_kept_with_its_callee_gone() {
        let sites = [site("main", &["same"])];
        let kept = bytes(&[("main", 300), ("same", 80)]);
        let other = bytes(&[("main", 322)]);
        assert_eq!(accepted(&sites, &kept, &other), [true]);
        assert_eq!(accepted(&sites, &kept, &bytes(&[("main", 380)])), [false], "a tie or more is put back");
    }

    /// Two callers of one callee are one group: priced apart, each would count
    /// the callee's bytes as its own saving.
    #[test]
    fn callers_of_one_callee_are_priced_as_one_group() {
        let sites = [site("a", &["h"]), site("b", &["h"])];
        let kept = bytes(&[("a", 100), ("b", 100), ("h", 50)]);
        let other = bytes(&[("a", 130), ("b", 130)]);
        assert_eq!(accepted(&sites, &kept, &other), [false, false], "a +60 B, h -50 B: larger");
        let cheaper = bytes(&[("a", 120), ("b", 120)]);
        assert_eq!(accepted(&sites, &kept, &cheaper), [true, true]);
    }

    /// A caller another inline absorbed has no bytes of its own in one compile:
    /// that is not a saving.
    #[test]
    fn a_caller_a_compile_does_not_define_is_not_priced() {
        let sites = [site("line", &["kernel"])];
        let kept = bytes(&[("line", 100), ("kernel", 90)]);
        assert_eq!(accepted(&sites, &kept, &bytes(&[])), [false]);
    }

    /// Sites of different chains are decided apart.
    #[test]
    fn unrelated_chains_are_decided_apart() {
        let sites = [site("a", &["x"]), site("b", &["y"])];
        let kept = bytes(&[("a", 100), ("x", 20), ("b", 100), ("y", 20)]);
        let other = bytes(&[("a", 110), ("b", 130)]);
        assert_eq!(accepted(&sites, &kept, &other), [true, false]);
    }
}

#[cfg(test)]
mod moved_tests {
    use std::collections::BTreeSet;

    use llrm_mir::module::Module;

    use super::*;

    fn module(first_returns: &str) -> Module {
        llrm_mir::parse::module(&format!(
            "define i32 @f() {{\n  ret i32 {first_returns}\n}}\ndefine i32 @g() {{\n  ret i32 7\n}}\ndeclare i32 @h()\n"
        ))
        .unwrap()
    }

    /// The trial's second backend run once covered the whole module, so a file
    /// with one counted loop and 400 other functions compiled its backend
    /// twice (memory_expansion.c -Os: 2.09x). It covers the functions whose
    /// bodies differ, the others standing as declarations.
    #[test]
    fn the_second_backend_run_covers_only_the_functions_that_differ() {
        let (one, other) = (module("1"), module("2"));
        let cut = differing(&one, &other);
        assert_eq!(cut, BTreeSet::from(["f".to_owned()]));
        let kept = only(&one, &cut);
        let defined = |name: &str| {
            kept.globals
                .iter()
                .find(|g| g.name.as_deref() == Some(name))
                .and_then(|g| g.function())
                .is_some_and(|f| !f.is_declaration())
        };
        assert!(defined("f") && !defined("g"));
        assert!(differing(&one, &module("1")).is_empty());
    }
}

#[cfg(test)]
mod unroll_keep_tests {
    use super::*;

    /// x86_64 -Os kept an unroll that grew long_double_union_pun by 13 B (53 ->
    /// 66; clang 53) because the code ran less: the one strict axis at -Os
    /// is bytes. A target that says so keeps only the smaller object.
    #[test]
    fn a_bytes_strict_unroll_trial_keeps_only_a_smaller_object() {
        assert!(keeps_unroll_at_size(10, 3, 1, false), "m32 keeps a few bytes for less work");
        assert!(!keeps_unroll_at_size(10, 3, 1, true), "strict: a grown object is put back");
        assert!(keeps_unroll_at_size(0, -1, 1, true), "strict: a smaller one stays");
        assert!(!keeps_unroll_at_size(10, 0, 1, true), "strict: a tie keeps the pass's own result");
    }
}

/// DEVIATE UNROLL-SPEED-TRIAL (beyond the pin; owner z80, exit 2026-11-24): a
/// decision of the full unroll at -O2 and -O3 that a driver compiles the module
/// again without, or with, and keeps the smaller object. Neither the pin's
/// threshold nor the price (bytes added, clocks saved) can see what an unroll
/// frees after it: one loop's copies fold the loads of the next and a whole
/// array is deleted (z80 loop_fusion -O2: priced "adds 108 B" a loop, the
/// object 831 B smaller), or its copies fold nothing and are copied again by
/// the inliner (memmove_overlap -O3: `fill`, 32 trips, 589 B rolled, 1329 B
/// unrolled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedUnroll {
    /// A full unroll is made only where it adds no bytes, and the
    /// inlinable-callee bypass of the price is off.
    NoGrow,
    /// An inline the speed caller's byte rate would take is made only where it
    /// adds no bytes: the rate is [`NO_GROW_RATE`]. Independent of
    /// `NoGrow`: the two answer different decisions, and a module can be
    /// smaller with one and not the other (idioms -O3: 1856 B with the
    /// unrolls refused, 3229 B with the inlines refused too).
    NoInlineGrowth,
    /// [`Self::NoInlineGrowth`], and the SZ2 trial (`walk.rs`) refuses a copy
    /// that adds bytes too, which it otherwise takes by the byte rate: the
    /// module with the calls where the trial inlines what the policy refused
    /// (shift_int64_masked_amount -O2: `mix` inlined at three hot sites,
    /// +81 B and +20% T; shrink_wrapped_frame -O3 is 374 B only through that
    /// inline, which `NoInlineGrowth` keeps).
    NoInlineGrowthAll,
    /// Both of the above: switch_table_in_code -O2 is 138 B smaller with both,
    /// 78 B with the inline alone.
    NoGrowBoth,
    /// The full unroll's threshold is this one, where the pin's -O2 is 150 and
    /// its -O3 300.
    Wide(u32),
    /// No non-trivial unswitch (-O3): the loop is not copied for a condition
    /// that does not change in it. queens -O3: `place` +93 B and +108745 T
    /// over -O2, 90 B over the SDCC bar, for one branch a trip.
    NoUnswitch,
}

impl SpeedUnroll {
    /// Whether the variant makes no non-trivial unswitch.
    pub fn refuses_unswitch(self) -> bool {
        matches!(self, Self::NoUnswitch)
    }

    /// Whether the variant makes only the full unrolls that add no bytes.
    pub fn refuses_unroll_growth(self) -> bool {
        matches!(self, Self::NoGrow | Self::NoGrowBoth)
    }

    /// Whether the variant makes only the inlines that add no bytes.
    pub fn refuses_inline_growth(self) -> bool {
        matches!(
            self,
            Self::NoInlineGrowth | Self::NoInlineGrowthAll | Self::NoGrowBoth
        )
    }

    /// Whether the SZ2 inline trial also refuses a copy that adds bytes.
    pub fn refuses_trial_growth(self) -> bool {
        self == Self::NoInlineGrowthAll
    }
}

/// The byte rate of the no-growth variants: a byte costs more than any clocks
/// can repay.
pub(crate) const NO_GROW_RATE: i64 = 1 << 40;

/// The threshold of [`SpeedUnroll::Wide`]: the pin's at -O3.
pub const WIDE_THRESHOLD: u32 = 300;

/// The loops and call sites of a compile at -O2 and up whose decision a variant
/// changes, asked of each as it is decided: those unrolled that `NoGrow` would
/// not unroll (`made`), those refused that `Wide` would unroll (`refused`), the
/// inlines that `NoInlineGrowth` would not make (`inlined`). A driver
/// compiles a variant only where its count is not 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnrollDecisions {
    pub made: usize,
    pub refused: usize,
    pub inlined: usize,
    /// The non-trivial unswitches that `NoUnswitch` would not make.
    pub unswitched: usize,
    /// The inlines the SZ2 trial took that add bytes, which only
    /// `NoInlineGrowthAll` refuses: a driver tries that variant after a
    /// `NoInlineGrowth` compile with a count over 0.
    pub trial_inlined: usize,
    /// The loops in which the target's alternate LSR order (LSR-KEY-TRIAL)
    /// picks another solution than its own: a driver compiles the module
    /// again under that order only where this is not 0.
    pub lsr_orders: usize,
}

thread_local! {
    /// The variant of UNROLL-SPEED-TRIAL running, if any.
    static SPEED_UNROLL: Cell<Option<SpeedUnroll>> = const { Cell::new(None) };
    /// The decisions since [`take_speed_unrolls`].
    static SPEED_UNROLLS: Cell<UnrollDecisions> = const { Cell::new(UnrollDecisions { made: 0, refused: 0, inlined: 0, unswitched: 0, trial_inlined: 0, lsr_orders: 0 }) };
}

thread_local! {
    /// Set while LSR rates solutions by the target's alternate order.
    static LSR_ALTERNATE: Cell<bool> = const { Cell::new(false) };
}

/// DEVIATE LSR-KEY-TRIAL (beyond the pin; gcc's ivopts totals its costs where
/// LSR ranks `LsrCost` fields in order, `tree-ssa-loop-ivopts.cc`
/// `iv_ca_cost`; owner z80, exit 2026-12-15): whether LSR rates solutions by
/// the target's alternate order (`Tti::lsr_alternate_order`). No order of the
/// fields tells histo's loop (a count-down plus a base add, 291 B) from
/// sum4's (the same deltas, 8 B worse): what differs is what the allocator
/// does with the registers, so the finished module decides.
pub fn lsr_alternate_order() -> bool {
    LSR_ALTERNATE.with(Cell::get)
}

/// Runs `body` with LSR rating by the target's alternate order.
pub fn with_lsr_alternate_order<T>(body: impl FnOnce() -> T) -> T {
    let held = LSR_ALTERNATE.with(|flag| flag.replace(true));
    let made = body();
    LSR_ALTERNATE.with(|flag| flag.set(held));
    made
}

/// A loop whose solution differs under the alternate order.
pub(crate) fn note_lsr_order() {
    SPEED_UNROLLS.with(|held| {
        let mut now = held.get();
        now.lsr_orders += 1;
        held.set(now);
    });
}

/// The UNROLL-SPEED-TRIAL variant in force, if a trial compile is running.
pub fn speed_unroll() -> Option<SpeedUnroll> {
    SPEED_UNROLL.with(Cell::get)
}

/// Runs `body` under `variant` (`None`: the pin's decisions).
pub fn with_speed_unroll<T>(
    variant: Option<SpeedUnroll>,
    body: impl FnOnce() -> T,
) -> T {
    let before = SPEED_UNROLL.with(|cell| cell.replace(variant));
    let value = body();
    SPEED_UNROLL.with(|cell| cell.set(before));
    value
}

/// Notes a call site inlined that adds bytes.
pub(crate) fn note_speed_inline() {
    SPEED_UNROLLS.with(|cell| {
        let mut seen = cell.get();
        seen.inlined += 1;
        cell.set(seen);
    });
}

/// Notes an SZ2 trial inline that adds bytes.
pub(crate) fn note_speed_trial_inline() {
    SPEED_UNROLLS.with(|cell| {
        let mut seen = cell.get();
        seen.trial_inlined += 1;
        cell.set(seen);
    });
}

/// Notes a non-trivial unswitch made.
pub(crate) fn note_speed_unswitch() {
    SPEED_UNROLLS.with(|cell| {
        let mut seen = cell.get();
        seen.unswitched += 1;
        cell.set(seen);
    });
}

/// Notes a loop unrolled (`made`) or, with known trips, refused.
pub(crate) fn note_speed_unroll(made: bool) {
    SPEED_UNROLLS.with(|cell| {
        let mut seen = cell.get();
        if made {
            seen.made += 1;
        } else {
            seen.refused += 1;
        }
        cell.set(seen);
    });
}

/// The decisions [`note_speed_unroll`] saw since the last call.
pub fn take_speed_unrolls() -> UnrollDecisions {
    SPEED_UNROLLS.with(Cell::take)
}

#[cfg(test)]
mod group_tests {
    use super::*;

    fn site(
        caller: &str,
        callee: &str,
    ) -> Site {
        Site { caller: caller.to_owned(), callees: vec![callee.to_owned()] }
    }

    /// Two sites with one caller were two verdicts on the caller's bytes, so a
    /// trial built both and kept the one that paid for the other. Sites
    /// that share a function, a refused and an accepted one included, are one
    /// group.
    #[test]
    fn sites_that_share_a_function_are_one_group() {
        let refused = [site("main", "a"), site("other", "b")];
        let accepted = [site("main", "put32"), site("lone", "c")];
        let mut found = groups(&refused, &accepted);
        found.sort_by_key(|group| group.names.len());
        assert_eq!(found.len(), 3);
        let joined = found.iter().find(|group| group.names.contains("main")).unwrap();
        assert_eq!((joined.refused.clone(), joined.accepted.clone()), (vec![0], vec![0]));
        assert!(joined.names.contains("a") && joined.names.contains("put32") && !joined.names.contains("other"));
    }
}
