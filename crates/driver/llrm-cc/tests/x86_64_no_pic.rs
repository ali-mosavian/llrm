//! x86-64 `-fno-pic`: external data is reached by absolute address, with or
//! without `-g`. Clang states no module flags at `-fno-pic` without `-g` and
//! without unwind tables (the QuickrBasic flags), and
//! llrm read that as position independent: `[rip + g]` and GOT loads
//! (`copy.c` -O2 had 2 `GOTPCREL`s without `-g`, none with it). Needs
//! `LLRM_LLC` and `readelf`.

use std::path::Path;
use std::process::Command;

fn relocations(debug: bool) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("no-pic-{}-{debug}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let mut run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"));
    run.env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(
            [
                "-O2",
                "-fno-pic",
                "-fno-asynchronous-unwind-tables",
                "-w",
                "-c",
            ],
        );
    if debug {
        run.arg("-g");
    }
    let run = run
        .arg("-o")
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/no_pic_extern.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let read = Command::new("readelf").args(["-r", "-W"]).arg(&object).output().expect("readelf");
    let text = String::from_utf8_lossy(&read.stdout).into_owned();
    // The code's relocations only: `.debug_*` and `.eh_frame` carry their own.
    let start = text.find("'.rela.text'").expect("code relocations");
    text[start..].split("Relocation section").next().unwrap().to_owned()
}

#[test]
fn external_data_at_no_pic_is_absolute_with_and_without_debug_info() {
    for debug in [false, true] {
        let relocs = relocations(debug);
        assert!(relocs.contains("R_X86_64_32 "), "-g {debug}: no absolute address:\n{relocs}");
        assert!(!relocs.contains("PC32") && !relocs.contains("GOT"), "-g {debug}: position independent:\n{relocs}");
    }
}
