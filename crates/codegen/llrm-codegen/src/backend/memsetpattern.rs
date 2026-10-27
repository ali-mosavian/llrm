// Which selectors take `memset.pattern` as a loop. The expansion itself is
// [`llrm_transforms::memsetpattern`].

pub use llrm_transforms::memsetpattern::expanded;

/// Whether `isel` takes `memset.pattern` as a loop: the DAG path has no node
/// for it, so its input is expanded, and so is the shadow run's (it tries the
/// DAG path, and the intrinsic made 9 corpus functions fall back to the old
/// selector).
pub fn expands_for(isel: crate::backend::shadow::Selector) -> bool {
    isel != crate::backend::shadow::Selector::Old
}
