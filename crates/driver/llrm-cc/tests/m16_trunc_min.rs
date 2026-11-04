//! `trunc (umin x, 255)` stayed an i32 min at m16: the clamp's constant was
//! `mov edx, 255` (66 BA FF 00 00 00, six bytes) where the byte select is
//! `mov al, -1` (two). jpeg_fixed -Os grew 14 bytes with the fold that makes
//! the IR this shape.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const IR: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"\n\
declare i32 @llvm.umin.i32(i32, i32)\n\
define i8 @f(i32 %x) {\n  %lt = icmp slt i32 %x, 0\n  %m = call i32 @llvm.umin.i32(i32 %x, i32 255)\n  %a = trunc nuw i32 %m to i8\n  %b = select i1 %lt, i8 0, i8 %a\n  ret i8 %b\n}\n";

#[test]
fn a_truncated_clamp_to_255_is_a_byte_select_at_m16() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let source = dir.join(format!("clamp-{}.ll", std::process::id()));
    let asm = dir.join(format!("clamp-{}.s", std::process::id()));
    std::fs::write(&source, IR).unwrap();
    for level in ["-O2", "-Os"] {
        let out = Command::new(target_dir::of(&root).join("release/llrm-llc"))
            .args(["-march=i386", level, "-S", "-o"])
            .arg(&asm)
            .arg(&source)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = std::fs::read_to_string(&asm).unwrap();
        assert!(text.contains("mov dl, -1") || text.contains("mov al, -1"), "{level}: no byte constant:\n{text}");
        let wide = text.lines().any(|line| line.trim().starts_with("mov e") && line.trim().ends_with(", 255"));
        assert!(!wide, "{level}: a wide 255:\n{text}");
    }
}
