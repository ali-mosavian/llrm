//! A `lea` of a base and a dword index at m16 is encoded with the shortest
//! displacement. Needs the pinned clang (`LLRM_PIN_CLANG`) and the Open Watcom
//! headers (`LLRM_OW`).

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

/// `lea eax,[eax+eax*4]` was `66 67 8d 84 80 00 00 00 00` (nine bytes, a dword
/// displacement of zero); lru -O2 grew 27 bytes. The pin's and jwasm's form is
/// five, with disp8 and disp32 only where the value needs them.
#[test]
fn a_base_and_scaled_dword_index_takes_the_shortest_displacement() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16lea-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("d.c"),
        "long f(long x) { return x * 5; }\n\
         long g(long x, long y) { return x + y * 8 + 8; }\n\
         long k(long x, long y) { return x + y * 4 + 4000; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-O2", "-w", "-c", "-march=i486", "-o"])
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let code = std::fs::read(dir.join("d.o")).unwrap();
    assert!(has(&code, &[0x66, 0x67, 0x8d, 0x04, 0x80]), "lea eax,[eax+eax*4] is not five bytes");
    assert!(has(&code, &[0x66, 0x67, 0x8d, 0x44, 0xd0, 0x08]), "a disp8 of 8 is not one byte");
    assert!(has(&code, &[0x66, 0x67, 0x8d, 0x84, 0x90, 0xa0, 0x0f, 0x00, 0x00]), "a disp32 of 4000 is not kept");
}
