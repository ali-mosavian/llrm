//! `LLRM_CHECK_MODULES` panicked ("declarations: the result kept is not what
//! working it out again gives") where the second IPSCCP restated the attributes
//! of two bodies and re-ran the first before the other was told changed
//! (`tests/run/c/memory_expansion.c`, 1 of ~210 sources). Needs the Open Watcom
//! tree (tools/ow-fetch.sh) and `LLRM_PIN_CLANG`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static unsigned char src[700];\nstatic int bad;\n__attribute__((noinline)) static void ref_copy(unsigned char *d, const unsigned char *s, int n) {\n    volatile unsigned char *vd = d;\n    for (int i = 0; i < n; i++) vd[i] = s[i];\n}\n__attribute__((noinline)) static void lr1(unsigned char *out, const unsigned char *pattern) {\n    unsigned char a[700], b[700];\n    for (int i = 0; i < 700; i++) a[i] = pattern[i], b[i] = 0xEE;\n    ref_copy(b + 3, a + 1, 1);\n}\nstatic void check_locals(void (*reference)(unsigned char *, const unsigned char *)) {\n    static unsigned char y[700];\n    reference(y, src);\n    if (y[0]) bad = 1;\n}\nint main(void) {\n    check_locals(lr1);\n    return bad;\n}\n";

#[test]
fn bodies_the_second_ipsccp_restated_are_all_told_changed_before_the_first_runs_again() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_PIN_CLANG").is_some(),
        "LLRM_PIN_CLANG is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("checkmodules-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .env("LLRM_CHECK_MODULES", "1")
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
