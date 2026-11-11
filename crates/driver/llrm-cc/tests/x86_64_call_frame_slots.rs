//! x86-64: the allocator's numbering has the instructions of the pin's. The pin brackets every call
//! with `ADJCALLSTACKDOWN64`/`UP64`, a zero-byte pair too, and removes them after allocation; ours
//! emitted the pair only for a call that passes stack bytes, so a function with two calls was
//! numbered 11 where the pin has 14 (15 with the argument-definition entry ours carries): every
//! live range was 0.82x as long, and its spill weight, queue priority and local-split estimate
//! with it. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_call_without_stack_arguments_holds_its_call_frame_pair_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("slots64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .env("LLRM_SLOT_COUNT", "1")
        .args(["-O1", "-w", "-c", "-o"])
        .arg(dir.join("s.o"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/slots_calls.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let log = String::from_utf8_lossy(&run.stderr);
    let slots: usize = log
        .lines()
        .find_map(|line| line.strip_prefix("SLOTS f "))
        .unwrap_or_else(|| panic!("no SLOTS line for f:\n{log}"))
        .parse()
        .unwrap();
    assert_eq!(slots, 15, "the pin's 14 and the entry definition; 11 without the pairs");
}
