//! At m16 the alloca raise to a type's preferred alignment is unbounded, as the pin's is (its m16 datalayout names no `S`). Needs the pinned
//! clang (`LLRM_PIN_CLANG`) and the Open Watcom headers (`{ow}`, tools/ow-fetch.sh).

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

/// With `S16` in m16's datalayout, bench/parity/qbsp -Os zeroed a packed 16-byte local with `mov cx, 16; rep stosb` where the pin's (no `S`)
/// raises the local to 4 and zeroes it with `mov cx, 8; rep stosw`: twice the iterations.
#[test]
fn a_packed_local_is_raised_and_zeroed_by_words() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_PIN_CLANG is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16raise-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("d.c"),
        "typedef struct { float x, y, z; } Vec3;\n\
         typedef struct { Vec3 norm; float dist; } Plane;\n\
         extern void use(Plane *);\n\
         void f(void)\n\
         {\n\
             Plane planes[2];\n\
             planes[0].norm.x = 1.0f; planes[0].norm.y = 0.0f; planes[0].norm.z = 0.0f; planes[0].dist = 0.0f;\n\
             planes[1].norm.x = 0.0f; planes[1].norm.y = 1.0f; planes[1].norm.z = 0.0f; planes[1].dist = 0.0f;\n\
             use(planes);\n\
         }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-wat")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-c", "-march=i386", "-o"])
        .arg(dir.join("d.o"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let code = std::fs::read(dir.join("d.o")).unwrap();
    assert!(has(&code, &[0xf3, 0xab]), "no `rep stosw`");
    assert!(!has(&code, &[0xf3, 0xaa]), "a `rep stosb` zeroes the local");
}
