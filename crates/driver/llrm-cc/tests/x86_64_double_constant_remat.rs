//! x86-64: a double constant read from the pool and used on both sides of a call is read again, not spilled, and the
//! load folds into the multiply after the call. The remat predicate knew `movsd` but the scalar double load is
//! `movsd_x`, so the constant was stored to the stack and reloaded (`mulsd xmm0, [rsp]`, soft_double_add_sub -O1
//! `main` 113 B against the pin's 107); remade alone it was a load beside the multiply (115 B), where the pin
//! (`canFoldAsLoad`) folds it: `mulsd xmm0, [pool]`. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

#[test]
fn a_double_constant_used_across_a_call_is_read_from_the_pool_again_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("double-remat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O1", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/double_constant_across_calls.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body = text.split("f proc near").nth(1).and_then(|rest| rest.split("f endp").next()).expect("an f body");
    let multiplies: Vec<&str> = body.lines().filter(|line| line.contains("mulsd")).collect();
    assert!(multiplies.iter().all(|line| !line.contains("[rsp")), "the constant was read back from the stack:\n{body}");
    assert!(multiplies.iter().any(|line| line.contains("$K")), "no multiply reads the pool operand:\n{body}");
}
