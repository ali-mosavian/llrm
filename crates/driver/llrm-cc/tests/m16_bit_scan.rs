//! `cttz` and `ctlz` at m16 were expanded to the popcount shifts and masks
//! (about 35 bytes each, `21845`, `13107`, `3855`) though the 386 has `bsf` and
//! `bsr`, as m32 selects. stft_single -Os (m16) carried one for `N / m`.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
declare i16 @llvm.cttz.i16(i16, i1)\n\
declare i16 @llvm.ctlz.i16(i16, i1)\n\
define i16 @tz(i16 %x) {\n  %c = call i16 @llvm.cttz.i16(i16 %x, i1 true)\n  ret i16 %c\n}\n\
define i16 @lz(i16 %x) {\n  %c = call i16 @llvm.ctlz.i16(i16 %x, i1 true)\n  ret i16 %c\n}\n";

#[test]
fn a_bit_scan_is_bsf_and_bsr_at_m16() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join(format!("scan-{}.ll", std::process::id()));
    let asm = dir.join(format!("scan-{}.s", std::process::id()));
    std::fs::write(&source, IR).unwrap();
    let out = Command::new(target_dir::of(&root).join("release/llrm-llc"))
        .args(["-march=i386", "-S", "-o"])
        .arg(&asm)
        .arg(&source)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(text.contains("bsf") && text.contains("bsr"), "no bit scan:\n{text}");
    assert!(!text.contains("21845"), "popcount mask expansion:\n{text}");
}
