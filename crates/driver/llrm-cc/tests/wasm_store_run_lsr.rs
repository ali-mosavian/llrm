//! Eight byte stores at `a + i + 0..7` (sieve's cleared array, `memset`
//! expanded) merge into one `i64.store`. LSR's beyond-pin end-biased formula
//! hoisted `a + 1031` out of the loop and addressed them at `-7..0` from it;
//! it saved no counter and cost 8 bytes of setup: sieve -O2 wasm32 507 B
//! against the pin's 500. Needs `LLRM_LLC` (the `llrm-wasm-llc` to test) and
//! `LLRM_CLANG_WASM`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_cleared_array_keeps_the_pins_base_not_a_hoisted_end_base_on_wasm() {
    for name in ["LLRM_LLC", "LLRM_CLANG_WASM"] {
        assert!(std::env::var_os(name).is_some(), "{name} is not set: this test needs it and does not skip");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("wasm-store-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/sieve_store_run.c");
    // The driver looks for `llrm-wasm-llc` beside itself: a copy of both, so
    // the backend under test is the one that runs.
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_llrm-cc-bin"), bin.join("llrm-cc-bin")).unwrap();
    std::fs::copy(std::env::var_os("LLRM_LLC").unwrap(), bin.join("llrm-wasm-llc")).unwrap();
    let run = Command::new(bin.join("llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "wasm32")
        .env("LLRM_WASM_DUMP", dir.join("stages"))
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join("d.o"))
        .arg(source)
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let lsr = std::fs::read_dir(dir.join("stages"))
        .unwrap()
        .map(|one| one.unwrap().path())
        .find(|path| path.file_name().unwrap().to_string_lossy().ends_with("-lsr.ll"))
        .expect("no lsr stage dumped");
    let ir = std::fs::read_to_string(lsr).unwrap();
    let hoisted = ir.lines().find(|line| line.contains("getelementptr i8, ptr %1, i32 1031"));
    assert!(hoisted.is_none(), "{hoisted:?}");
}
