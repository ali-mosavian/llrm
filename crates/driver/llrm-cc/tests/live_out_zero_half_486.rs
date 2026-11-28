//! A value another block made with a high half of zero (`m = mb & 0x1FFFFF`, an
//! `i64`) is that to the block that multiplies by it (`FunctionLoweringInfo`
//! live-out registers, `getCopyFromRegs`' assertions). Before: the product
//! `d * m` there was `xor; imul; mul; imul`, the widening multiply lost; now
//! one `mul`. Needs the Open Watcom tree (tools/ow-fetch.sh) and the m16 clang
//! (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "typedef unsigned long long u64; typedef unsigned u32;\nextern u64 gv;\nu64 h(u64 mb, u32 d, u32 e, int c)\n{\n    u64 m = mb & 0x1FFFFF;\n    u64 r = (u64)d * m;\n    if (c) { gv = m; r ^= (u64)e * m; }\n    return r;\n}\n";

#[test]
fn a_product_with_a_zero_extended_value_from_another_block_is_one_mul() {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("liveout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("h.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args(["-O2", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("h.s"))
        .arg(dir.join("h.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("h.s")).unwrap();
    assert_eq!(text.lines().filter(|line| line.trim_start().starts_with("imul ")).count(), 0, "{text}");
}
