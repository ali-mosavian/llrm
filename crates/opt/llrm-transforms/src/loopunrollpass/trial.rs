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
    /// The functions with a loop the threshold refused that a trial may take.
    static ELIGIBLE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
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

/// Notes that `function` has a loop the threshold refused and a trial may take.
pub(super) fn note(function: &str) {
    ELIGIBLE.with(|list| {
        let mut list = list.borrow_mut();
        if !list.iter().any(|one| one == function) {
            list.push(function.to_owned());
        }
    });
}

/// The functions noted since the last call.
pub fn take() -> Vec<String> {
    ELIGIBLE.with(|list| std::mem::take(&mut *list.borrow_mut()))
}
