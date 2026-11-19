//! x86-64: the address of a global (`lea r,[rip+sym]`, `LEA64r`) is made again where a split
//! piece starts and copied where it returns (DEVIATE SPLIT-REMAT-ENTER). The pin copies it
//! (`isAsCheapAsAMove`, SplitKit.cpp#L658); remade at both ends, ten globals live across 80
//! call sequences gave 134 `lea`s against the pin's 122, an object 424 B larger (13680 B
//! against 13256 B). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_symbol_address_is_not_remade_where_a_split_piece_returns_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("split64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-fPIE", "-O1", "-w", "-c", "-o"])
        .arg(dir.join("s.o"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/split_lea_symbols.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let size = std::fs::metadata(dir.join("s.o")).unwrap().len();
    assert!(size <= 13400, "object is {size} B; remade at both ends 13680, at the start only 13256");
}
