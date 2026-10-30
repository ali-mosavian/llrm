//! The allocator's holders index and a body renumbered by a rewrite.

use std::process::Command;

/// jpeg_fixed at -Omax -m32 panicked in the allocator, `liveunion.rs:88` (since
/// deleted) "no entry found for key": a rewrite that took a fresh slot
/// numbering moved every interval, yet only the values it named were moved in
/// the index of a register with many holders, so a removed holder's old
/// segments stayed in it.
#[test]
fn a_rewrite_that_renumbers_the_slots_does_not_leave_stale_holders_in_the_index() {
    let scratch = tempfile::tempdir().unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/jpeg_fixed/jpeg_fixed.nib");
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .args(["-Omax", "-m32", "-o"])
        .arg(scratch.path().join("x.o"))
        .arg(source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
