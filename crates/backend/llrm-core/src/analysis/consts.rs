//! Which values are known, and what they are.
//!
//! Port of `qbopt/analysis/consts.py`.  A fact is a width as well as a
//! number -- "the low `width` bytes of this value are `n`" -- and an
//! operation only folds where the widths agree.

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// Fixed points solved, for the tests that pin cache reuse to Python's.
    pub static SOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// `may_overlap` questions, for the test that pins the cell index.
    pub static MAY_OVERLAP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
