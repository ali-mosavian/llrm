//! `tools/6502/smoke_divide.sh`: six small divide and multiply programs,
//! compiled at -O2, run on sim65 and compared with the host's gcc. A pair's
//! remainder read after a later divide was wrong on release/next
//! twice (2026-11-15), at every level, and no program ran that shape.

mod common;

use std::process::Command;

#[test]
fn the_divide_smoke_programs_print_what_the_host_prints() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(root.join("tools/6502/smoke_divide.sh"))
        .env("LLRM_CC", common::pipeline::llrm_cc())
        .output()
        .expect("smoke_divide.sh");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "exit {:?}:\n{said}", out.status.code());
}
