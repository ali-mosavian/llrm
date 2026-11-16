//! A spill of a value that shares a bundle with its terminator goes before it.

use llrm_6502::driver::{Level, Request, assembly};

/// `pack_ref` at -O1 panicked in tailmerge (`LDI v; BCC; SPILL v`: the store
/// after the branch, a block merged into its predecessor while the branch
/// still named it).
///
/// Since #9950 (a runtime call keeps scratch cells) the word argument also
/// lives in `a`, `x` and `y` at the inline callee-saved pushes: emission
/// refused ("a register is live where the callee-saved cells are saved"). The
/// pushes go through a free cell.
#[test]
fn a_spill_of_a_flag_setters_value_is_not_stored_after_the_branch() {
    let text = assembly(include_str!("data/pack_ref_spill_after_branch.ll"), &Request::new(Level::O1))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("pack_ref:"), "{text}");
}

/// The same function at -O2 and -Os, where the callee-saved cells are pushed
/// one by one: `a` was live with no index register free, and emission refused.
#[test]
fn a_callee_saved_push_with_a_x_and_y_live_goes_through_a_free_cell() {
    for level in [Level::O2, Level::Os] {
        let text = assembly(include_str!("data/pack_ref_spill_after_branch.ll"), &Request::new(level))
            .unwrap_or_else(|e| panic!("{level:?}: {e}"));
        assert!(text.contains("pack_ref:"), "{level:?}: {text}");
    }
}
