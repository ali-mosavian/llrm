//! Each target's `tests/cc/abi/<name>.c` compiles: its `_Static_assert`s state
//! the sizes and layouts clang is told. m16 needs the i8086 clang
//! (`LLRM_CLANG_M16`); without it the check says it skipped.

use std::path::Path;
use std::process::Command;

fn compile(
    target: &str,
    file: &str,
) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", root.join("target/release/llrm-llc"))
        .args(["-c", "-O1", "-o", "/dev/null"])
        .arg(root.join("tests/cc/abi").join(file))
        .output()
        .unwrap();
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).into_owned()) }
}

#[test]
fn m16_sizes_and_layouts_are_the_ones_llrm_expects() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    compile("x86_16-dos", "x86_16.c").unwrap();
}

/// Every module's code went to one `0_TEXT` segment (the IR file was named by
/// its number), so QCport's link failed with "size of segment 0_TEXT exceeds
/// 64k"; and a defined global was not public.
#[test]
fn m16_objects_name_their_code_segment_after_the_source_and_export_globals() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let listing_path = std::env::temp_dir().join(format!("llrm-cc-segments-{}.s", std::process::id()));
    let listing = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env("LLRM_LLC", root.join("target/release/llrm-llc"))
        .args(["-S", "-O1", "-o"])
        .arg(&listing_path)
        .arg(root.join("tests/cc/segments/first.c"))
        .output()
        .unwrap();
    let text = std::fs::read_to_string(&listing_path).unwrap_or_default();
    let _ = std::fs::remove_file(&listing_path);
    assert!(
        text.contains("FIRST_TEXT") || text.contains("first_TEXT"),
        "{text}{}",
        String::from_utf8_lossy(&listing.stderr)
    );
    assert!(text.contains("public _tbl"), "{text}");
}
