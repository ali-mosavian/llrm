//! DEVIATE UNROLL-TRIAL (beyond the pin; owner 6502, exit 2026-12-05): the
//! state the trial unroll (`crate::trialunroll`) shares with the pass.
//!
//! The pin decides a full unroll by a threshold and by what the loop's own
//! body folds (`analyzeLoopUnrollCost`, at most ten trips). A loop whose
//! copies fold the loads of a later loop (an array filled by one loop and read
//! by the next) is seen by neither: the pin unrolls it only because its
//! threshold is large in its own units. Here the loops the threshold refuses
//! are noted, and the function is later unrolled on trial.

use std::cell::{Cell, RefCell};

thread_local! {
    /// The unrolled size under which every loop is unrolled, set while a trial
    /// runs the simplification list.
    static TRIAL: Cell<Option<u32>> = const { Cell::new(None) };
    /// The functions with a loop the threshold refused that a trial may take.
    static ELIGIBLE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The trips a trial run proved of each loop, by the id of its latch: the
    /// frequencies of the body as it stood price the clocks it ran.
    static TRIPS: RefCell<llrm_support::hash::IndexMap<i64, i64>> = RefCell::new(Default::default());
}

/// A loop of the trial run whose latch is `latch` runs it `trips` times.
pub(super) fn proved(
    latch: i64,
    trips: u32,
) {
    TRIPS.with(|map| {
        map.borrow_mut().insert(latch, i64::from(trips));
    });
}

/// The trips proved since the last call.
pub fn take_trips() -> llrm_support::hash::IndexMap<i64, i64> {
    TRIPS.with(|map| std::mem::take(&mut *map.borrow_mut()))
}

/// The trial threshold in force, if a trial is running.
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
    if llrm_support::env_flag!("LLRM_DEBUG_TRIAL") {
        eprintln!("trial: noted {function}");
    }
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
