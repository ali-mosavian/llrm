//! `llvm.experimental.memset.pattern` as a store loop, before selection. The
//! expansion is `llrm_transforms::memsetpattern`; this module says which
//! selectors take it.
//!
//! # Example
//! `--isel=dag` takes the loop; `--isel=old` selects the intrinsic itself.

pub use llrm_transforms::memsetpattern::expanded;

/// Whether `isel` takes `memset.pattern` as a loop: the DAG path has no node
/// for it, so its input is expanded, and so is the shadow run's (it tries the
/// DAG path, and the intrinsic made 9 corpus functions fall back to the old
/// selector).
pub fn expands_for(isel: crate::backend::shadow::Selector) -> bool {
    isel != crate::backend::shadow::Selector::Old
}
