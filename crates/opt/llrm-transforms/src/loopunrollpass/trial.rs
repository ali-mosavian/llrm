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

use std::cell::{Cell, RefCell};

thread_local! {
    /// The unrolled size under which every loop is unrolled, set while the
    /// trial compile runs.
    static TRIAL: Cell<Option<u32>> = const { Cell::new(None) };
    /// The functions with a loop the threshold refused that a trial may take,
    /// and the unrolled size of those loops together.
    static ELIGIBLE: RefCell<(Vec<String>, u64)> = const { RefCell::new((Vec::new(), 0)) };
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
pub(super) fn note(
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

/// Whether a trial unroll is kept at -O1, where each axis is strict: it adds no bytes (`added`
/// <= 0) and costs no work (`saved` >= 0, work being what the code is expected to execute). The
/// one rule of every target's -O1 trial (DEVIATE UNROLL-TRIAL, 6502-UNROLL-O1, UNROLL-O1-TRIAL).
pub fn keeps_at_o1(
    saved: i64,
    added: i64,
) -> bool {
    added <= 0 && saved >= 0
}
