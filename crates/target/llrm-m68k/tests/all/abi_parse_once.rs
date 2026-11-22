//! `abi::c()` parsed `calling.toml` on every call: every call lowering and
//! every `callee_saved` (3.2 M of grep's 132 M compile instructions at -O2).

use llrm_m68k::{abi, frame};

#[test]
fn the_c_convention_is_parsed_once_however_often_it_is_asked_for() {
    for _ in 0..50 {
        let _ = abi::c();
        let _ = frame::callee_saved();
    }
    assert!(abi::parses() <= 1, "calling.toml parsed {} times", abi::parses());
}
