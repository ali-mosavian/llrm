//! x86-64 -Os: a callee of two fills and a move of an unknown length stays a
//! function called from three places. The size guard counted each of the three
//! calls as one instruction, under the 9 units of its own call, so
//! QuickrBasic's `justify` was copied into `set` three times (`file.c`, 445 B
//! against gcc's 212). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inline-fill-call-size-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/inline_fill_call_size.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn three_callers_of_two_fills_and_a_move_keep_the_call() {
    let text = assembly();
    assert!(text.contains("justify proc"), "justify was inlined into its three callers:\n{text}");
}
