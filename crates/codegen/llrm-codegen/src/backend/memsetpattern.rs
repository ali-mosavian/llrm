//! Which selectors take `memset.pattern` as a loop. The expansion itself is
//! [`llrm_transforms::memsetpattern`].
//!
//! # How it works
//! The DAG path has no node for the intrinsic, so a selector that is not the
//! old one gets its input expanded into a loop by the transform; the old
//! selector lowers the intrinsic itself.
//!
//! # Used by
//! The compile driver (`driver/mod.rs`), which asks `expands_for` of its
//! selector and expands each module with `expanded`.
//!
//! # Example
//! `memset.pattern(p, pat, n)` reaches the DAG selector as the loop `for i in
//! 0..n { p[i] = pat }`.

pub use llrm_transforms::memsetpattern::expanded;

/// Whether `isel` takes `memset.pattern` as a loop: the DAG path has no node
/// for it, so its input is expanded, and so is the shadow run's (it tries the
/// DAG path, and the intrinsic made 9 corpus functions fall back to the old
/// selector).
pub fn expands_for(isel: crate::backend::shadow::Selector) -> bool {
    isel != crate::backend::shadow::Selector::Old
}
