//! `(a << 1) + (x >= y)` is `add; cmp; sbb eax, -1`, not `xor; cmp; setae;
//! lea`: the generic combiner makes the add a disjoint `or`, and the pin's
//! `combineOr` ends in `combineAddOrSubToADCOrSBB`. Soft-float division's
//! quotient loop (`q = q << 1 | (r >= d)`) had the setcc and the lea in all 28
//! trips (sfm16_dbg -Os +207 B against gcc). Needs the Open Watcom tree
//! (tools/ow-fetch.sh) and the m16 clang (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned g(unsigned r, unsigned mb) { return (r << 1) + (r >= mb); }
unsigned h(unsigned r, unsigned mb) { return (r << 1) | (r < mb); }
";

#[test]
fn test_m32_shifted_value_plus_a_compare_is_adc_or_sbb() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("orcarry-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-march=i486", "-mabi=sysv", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(!listing.contains("setae") && !listing.contains("setb"), "a compare is materialized:\n{listing}");
    assert!(listing.contains("sbb") && listing.contains("adc"), "no carry add:\n{listing}");
}
