//! A 6-byte copy was stored as a dword and a word. The reader of the packed
//! pair loads a word at 0 and a dword at 2, which spans both stores and cannot
//! forward: the load waits for the stores to commit. Stored as an overlapping
//! dword pair, 0 then 2, both loads forward.
//! Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing(target: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86-copy-tail-{target}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/copy_tail_straddle.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

fn stores(text: &str) -> Vec<&str> {
    text.lines().filter(|line| line.contains("mov") && line.contains("PTR [") && line.contains("],")).collect()
}

fn assert_overlapping_dwords(target: &str) {
    let text = listing(target);
    let stores = stores(&text);
    assert_eq!(stores.len(), 2, "{target}: two stores\n{text}");
    assert!(
        stores.iter().all(|line| line.contains("DWORD PTR")),
        "{target}: a 6-byte copy is two dword stores at 0 and 2, not a dword and a word\n{text}"
    );
    // The tail goes last: stored first, the other store would overwrite its
    // low half and a dword load at 2 would span both again.
    assert!(
        !stores[0].contains("+0x2]") && stores[1].contains("+0x2]"),
        "{target}: the store at 2 follows the store at 0\n{text}"
    );
}

#[test]
fn a_six_byte_copy_is_two_overlapping_dwords_at_m64() {
    assert_overlapping_dwords("x86_64-linux");
}

#[test]
fn a_six_byte_copy_is_two_overlapping_dwords_at_m32() {
    assert_overlapping_dwords("x86_32-linux");
}
