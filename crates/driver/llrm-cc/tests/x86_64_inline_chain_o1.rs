//! x86-64 -O1: a trial that inlines `line` into `main` also inlines the calls
//! its copy brings, priced by the ordinary rules (`Tuning::inline_chain_trial`,
//! as the pin's callee-first order gives). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(
    source: &str,
    level: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inline-chain-{}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// `main` called `line` twice (frame_address_stack_args -O1: 276 B against
/// clang's 238 B): inlined, its two calls fold to constants, which no step
/// shows alone.
#[test]
fn a_call_chain_whose_copy_folds_to_constants_is_inlined_at_o1() {
    let text = assembly("inline_chain_o1.c", "-O1");
    let main =
        text.split("main proc near").nth(1).and_then(|rest| rest.split("main endp").next()).expect("a main body");
    assert!(!main.contains("call line"), "main still calls line:\n{main}");
}
