//! A divide by a constant in a loop at m16 -O2 stayed `idiv` (SD446 weighed its
//! bytes against one execution's clocks; the block frequency never reached the
//! comparator): recmany ran 292554 clocks against 229703 before. Needs the
//! pinned clang (`LLRM_PIN_CLANG`) and the Open Watcom headers (`{ow}`,
//! tools/ow-fetch.sh).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn has(
    code: &[u8],
    pattern: &[u8],
) -> bool {
    code.windows(pattern.len()).any(|window| window == pattern)
}

#[test]
fn a_remainder_by_a_constant_in_a_loop_is_not_an_idiv_at_m16_o2() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_PIN_CLANG is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16divhot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("d.c"),
        "long f(long *a, int n) { long s = 0; int i; for (i = 0; i < n; ++i) s += a[i] % 5 + a[i] % 7; return s; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let code = std::fs::read(dir.join("d.o")).unwrap();
    // `66 f7 /7` is `idiv r32`.
    let idiv = (0xf8..=0xff).any(|modrm| has(&code, &[0x66, 0xf7, modrm]));
    assert!(!idiv, "the loop divides by idiv");
}
