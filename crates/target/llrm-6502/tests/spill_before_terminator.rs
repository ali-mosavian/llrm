//! A spill of a value that shares a bundle with its terminator goes before it.

use llrm_6502::driver::{Level, Request, assembly};

/// `pack_ref` at -O1 panicked in tailmerge (`LDI v; BCC; SPILL v`: the store
/// after the branch, a block merged into its predecessor while the branch
/// still named it).
#[test]
fn a_spill_of_a_flag_setters_value_is_not_stored_after_the_branch() {
    let text = assembly(include_str!("data/pack_ref_spill_after_branch.ll"), &Request::new(Level::O1))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("pack_ref:"), "{text}");
}
