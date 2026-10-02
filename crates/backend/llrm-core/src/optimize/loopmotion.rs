//! Sink loop stores nothing inside the loop observes to its single exit.
//!
//! Port of `qbopt/optimize/loopmotion.py`.

// Its callers live in transform.py, not yet ported.

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// Interval maps built, one per block, and the blocks `sunk_stores` was handed.
    pub static MAPPED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub static SEEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
