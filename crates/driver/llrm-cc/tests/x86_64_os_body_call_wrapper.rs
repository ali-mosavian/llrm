//! At -Os a one-call wrapper that stays (external) was inlined into its caller:
//! the inline cost priced the call inside the body flat (args + 1) and the
//! site's own call at 150%, so a 3-argument call plus a store looked smaller
//! than a 2-argument call. gcc keeps the call; comp.c -Os grew 218 B over six
//! such helpers. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn listing() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("x86_64-os-body-call-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-Os", "-w", "-ffreestanding", "-fno-pic", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/os_body_call_wrapper.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let dump = Command::new("objdump").args(["-d", "--no-show-raw-insn", "-r"]).arg(&object).output().unwrap();
    String::from_utf8_lossy(&dump.stdout).into_owned()
}

fn body_of<'a>(
    text: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let header = format!("<{name}>:");
    text.lines().skip_while(|line| !line.contains(&header)).skip(1).take_while(|line| !line.is_empty()).collect()
}

#[test]
fn a_wrapper_that_stays_is_called_not_copied_at_os() {
    let text = listing();
    let user = body_of(&text, "user");
    let calls = user.iter().filter(|line| line.contains("<wrap_int>") || line.contains("<wrap_op>")).count();
    assert_eq!(calls, 2, "user calls both wrappers, none copied into it:\n{text}");
}

#[test]
fn a_callee_that_goes_with_its_only_call_is_still_inlined_at_os() {
    let text = listing();
    assert!(!text.contains("<inner>:"), "inner is gone:\n{text}");
    let body = body_of(&text, "user_dies");
    assert!(body.iter().all(|line| !line.contains("<inner>")), "user_dies has inner's body:\n{text}");
}
