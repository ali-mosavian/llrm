//! m32 -Os (SZ2-COLD): `x == 0 ? 1 : x` became `umax` (#9902), which saved the
//! `unpack` decode two IR units, so the trial inlined it at all six sites of
//! the soft-float kernel and every sfm16 cell grew 50 B (1763 -> 1813).
//! A copy that runs once per run and leaves its callee in place stays a call.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compiled(source: &str) -> String {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m32cold-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("d.s"))
        .arg(root.join(source))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(dir.join("d.s")).unwrap()
}

#[test]
fn test_a_cold_out_parameter_decode_with_several_sites_stays_a_call() {
    let asm = compiled("bench/sfm16_float_mul/sfm16_float_mul.c");
    let calls = asm.lines().filter(|line| line.trim().starts_with("call") && line.contains("unpack")).count();
    assert!(
        asm.lines().any(|line| line.trim() == "unpack proc near") && calls >= 4,
        "unpack was inlined at every site ({calls} calls left), +50 B"
    );
}
