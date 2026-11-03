//! At m16 a bit field is allocated in units of its declared type, as Watcom,
//! Borland and MSVC do (`-mms-bitfields`), not packed across them as the
//! Itanium ABI would. Needs the patched clang (`LLRM_CLANG_M16`) and the Open
//! Watcom headers (`LLRM_OW`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

/// volatile_bitfield.c -Os was 283 B against 230 B built the DOS compilers'
/// way: `unsigned a:3, f:15` shared one 32-bit container, so each access was a
/// dword read-modify-write instead of a byte or a word.
#[test]
fn bit_fields_are_laid_out_in_units_of_their_declared_type() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16bf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("b.c"),
        "struct A { unsigned a : 3; unsigned f : 15; };\n\
         struct B { unsigned char c : 3; unsigned int i : 5; };\n\
         struct C { unsigned a : 3; unsigned f : 15; unsigned long g : 14; };\n\
         _Static_assert(sizeof(struct A) == 4, \"a:3 f:15 do not share a word\");\n\
         _Static_assert(sizeof(struct B) == 3, \"a char field and an int field do not share a unit\");\n\
         _Static_assert(sizeof(struct C) == 8, \"g starts its own unit\");\n\
         int x;\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-c", "-march=i486", "-o"])
        .arg(dir.join("b.o"))
        .arg(dir.join("b.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}
