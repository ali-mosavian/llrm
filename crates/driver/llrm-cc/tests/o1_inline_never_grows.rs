//! m32 -O1 (INL-O1): a growing inline stays only where the call's clocks pay for
//! its bytes at the O1 rate. The pin's threshold copied every soft-float helper
//! into each site: sfm16_dbg 24,284 B against 23,372 B priced. Needs the Open
//! Watcom tree (tools/ow-fetch.sh) and the m16 clang (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn compile(
    name: &str,
    source: &str,
    flag: &str,
) -> (String, u64) {
    assert!(
        llrm_cc::ow_tree().is_dir(),
        "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip"
    );
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inlo1-{name}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = |mode: &str, out: &str| {
        let result = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([flag, "-w", mode, "-march=i486", "-mabi=sysv", "-o"])
            .arg(dir.join(out))
            .arg(dir.join("d.c"))
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    run("-S", "d.s");
    run("-c", "d.o");
    (
        std::fs::read_to_string(dir.join("d.s")).unwrap(),
        std::fs::metadata(dir.join("d.o")).unwrap().len(),
    )
}

#[test]
fn test_m32_o1_straight_line_helpers_are_priced_not_all_copied() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = std::fs::read_to_string(root.join("bench/sfm16_dbg/sfm16_dbg.c")).unwrap();
    let (_, size) = compile("soft", &source, "-O1");
    assert!(size < 23_800, "object is {size} B; copying every helper into each site gave 24,284, pricing them 23,372");
}

/// The inliner's growth is in size units, one per instruction, and a callee's
/// bytes are 3.5 to 5 times that: compared with a per-byte rate it let
/// `sf_cmp_bits` (about 70 B) into each of eight comparison wrappers
/// (sfm16_float 23,432 B against gcc's smaller object and 22,748 B with the unit
/// scaled).
#[test]
fn test_m32_o1_growth_is_priced_in_bytes_not_size_units() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source = std::fs::read_to_string(root.join("bench/sfm16_float/sfm16_float.c")).unwrap();
    let (_, size) = compile("units", &source, "-O1");
    assert!(size < 23_000, "object is {size} B; the unscaled growth gave 23,432");
}

/// The bottom-up walk priced `sf32_pack` in `mul_core` at the site's own
/// frequency (1.0), though the soft-float multiply runs in a loop of the
/// kernel 30 times per call: it stayed a call and x_sf_mul ran 88263 clk
/// against 82587 with it inlined (gcc/clang 88107). The check weighs a site by
/// how often its function runs, up the call graph.
#[test]
fn test_m32_o1_inline_weighs_how_often_the_caller_runs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source =
        std::fs::read_to_string(root.join("crates/target/llrm-x86-m32/vsgcc/kernels/x_sf_mul/x_sf_mul.c")).unwrap();
    let (listing, _) = compile("entry", &source, "-O1");
    let calls = listing.lines().filter(|line| line.contains("call") && line.contains("sf32_pack")).count();
    assert!(calls <= 2, "{calls} calls of sf32_pack: the hot caller's copies were not made (3 without the weight)");
}

/// m32 -O2 (INL-O2 RATE): a growing inline stays where the call sequence's
/// clocks, at the site's run count, pay for the bytes at the 486's rate. The
/// call was priced as one instruction (4 clocks of the 15 it costs, with its
/// pushes, `ret` and `add esp`) and a helper called from a hot loop up the
/// graph kept its call: x_sf_add -O2 object: 22,860 B before, 22,572 B priced.
#[test]
fn test_m32_o2_inline_prices_the_call_sequence_and_the_caller_s_run_count() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source =
        std::fs::read_to_string(root.join("crates/target/llrm-x86-m32/vsgcc/kernels/x_sf_add/x_sf_add.c")).unwrap();
    let (_, size) = compile("o2rate", &source, "-O2");
    assert!(size < 22_700, "object is {size} B; the call priced as one instruction gave 22,860");
}
