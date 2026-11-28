//! `--target=x86_32-wat -mabi=sysv`: clang writes every function
//! `x86_regcallcc` with `inreg` parameters (the Watcom register convention),
//! and the System V backend placed `inreg` in EAX since `-mregparm=N` was
//! honoured (#10316), so `call _report` passed its argument in EAX where a
//! System V extern read the stack: every m32 FPU-table cell failed (fib
//! returned 28672, expected 6765). Only `ccc` with `inreg` (what `-mregparm=N`
//! makes clang write) is placed; regcall's is dropped.

use std::path::Path;
use std::process::Command;

#[test]
fn test_a_regcall_function_under_the_sysv_abi_passes_its_arguments_on_the_stack() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sysv-regcall-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), "extern void report(int);\nint f(int n) { report(n + 1); return n; }\n").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args(["-mabi=sysv", "-O1", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    let call = listing.find("call _report").expect("a call of _report");
    let before: Vec<&str> = listing[..call].lines().rev().take(3).collect();
    assert!(before.iter().any(|line| line.trim_start().starts_with("push")), "the argument is not pushed:\n{listing}");
    assert!(
        listing[call..].lines().nth(1).is_some_and(|line| line.contains("add esp")),
        "no stack cleanup:\n{listing}"
    );
}
