//! x86-64 PIE: a symbol is `[rip + sym]`, with no index or base register
//! beside it. LSR priced `@sym + reg` as one address and kept it, so the
//! backend formed `lea r,[rip+sym]` at every use in the loop: histo -O2 was
//! 88 B larger (2136 B object against 2048 B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_global_beside_an_index_costs_a_register_at_x86_64_pie() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pie64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-fPIE", "-O2", "-w", "-c", "-o"])
        .arg(dir.join("h.o"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/pie64_histo.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let size = std::fs::metadata(dir.join("h.o")).unwrap().len();
    assert!(size <= 2088, "object is {size} B; with the symbol beside an index 2136, without 2048");
}
