//! `table[id >> 10]` is `shr 10` and the entry size as the scale (8, 4, 2;
//! a\n//! load or a store; a 32- or 64-bit index). The
//! pin's `matchAddressRecursively` (`zext(and(srl))` into `foldMaskAndShiftTo
//! Scale`) restores it from the combiner's `(id >> 7) & 0x1fffff8`; ours had no
//! such case and kept the mask: `shr 7; and 0x1fffff8; [r+i*1]`, 40 times in
//! QuickrBasic's m64 Os modules (clang and gcc: none). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-chunk-table-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_64_chunk_table_index.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

#[test]
fn a_shifted_index_into_a_table_is_a_shift_and_the_entry_size_as_a_scale() {
    let text = listing();
    assert!(!text.contains("and "), "an index was masked, not scaled\n{text}");
    for (scale, count) in [("*8]", 3), ("*4]", 2), ("*2]", 1)] {
        assert_eq!(text.matches(scale).count(), count, "{scale}\n{text}");
    }
}
