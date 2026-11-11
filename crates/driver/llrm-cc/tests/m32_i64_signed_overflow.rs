//! m32: a signed 64-bit add or subtract with overflow is `add; adc; seto` (and
//! `sub; sbb; seto`), the pin's `SADDO_CARRY`. It was expanded to
//! `xor; not; test; sets`: +17 B a use (overflow_builtins), 2026-11-09.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(level: &str) -> String {
    // one directory per call: the tests run in parallel and shared `d.s` raced
    static CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32sovf-{level}-{}-{call}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args([level, "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/m32_i64_signed_overflow.c"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

fn function<'a>(
    asm: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let start = asm.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    asm.lines().skip(start).take_while(|line| line.trim() != format!("{name} endp")).map(str::trim).collect()
}

#[test]
fn test_a_signed_i64_overflow_is_the_carry_chain_flag() {
    for level in ["-O1", "-O2", "-Os"] {
        let asm = compiled(level);
        for (name, chain) in [("sadd", "adc"), ("ssub", "sbb")] {
            let body = function(&asm, name);
            let has = |prefix: &str| body.iter().any(|line| line.starts_with(prefix));
            assert!(has(chain) && has("seto"), "{level} {name}: no {chain}/seto:\n{}", body.join("\n"));
            assert!(!has("sets") && !has("not "), "{level} {name}: sign-test expansion:\n{}", body.join("\n"));
        }
    }
}
