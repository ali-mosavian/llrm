//! m32 -Os: LSR ranks formulae by bytes. Four fill/copy loops over globals were
//! 383 B with the pin's order (a pointer IV per loop) and 373 B ranked by
//! bytes. Needs the Open Watcom tree (tools/ow-fetch.sh) and the m16 clang
//! (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "long L[40], M[40];
short a[64], b[64];
void fill_long(short n, long v) { short i; for (i = 0; i < n; ++i) L[i] = v; }
void copy_long(short n) { short i; for (i = 0; i < n; ++i) M[i] = L[i]; }
void fill_w(short n, short v) { short i; for (i = 0; i < n; ++i) a[i] = v; }
void copy_w(short n) { short i; for (i = 0; i < n; ++i) b[i] = a[i]; }
";

#[test]
fn test_m32_os_lsr_picks_the_smaller_loop_formulae() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lsrsize-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-c", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("f.o"))
        .arg(dir.join("f.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let size = std::fs::metadata(dir.join("f.o")).unwrap().len();
    assert!(size <= 377, "object is {size} B; the pin's order gave 383, bytes-ranked 373");
}
