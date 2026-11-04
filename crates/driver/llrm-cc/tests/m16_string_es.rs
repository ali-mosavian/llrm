//! A backward string move takes ES as an input, not a clobber, so a
//! selector already in ES is reused. Needs the pinned clang
//! (`LLRM_PIN_CLANG`) and the Open Watcom headers (`LLRM_OW`).

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

/// Each backward copy loaded ES itself (`push ds; pop es`), so scroll -Os with
/// a forward and a backward copy spent six bytes more than one load.
#[test]
fn backward_copies_load_es_once() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || std::env::var_os("LLRM_OW").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG or LLRM_OW is not set");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16es-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("d.c"),
        "static short a[2000];\n\
         void f(void) {\n\
             short i;\n\
             for (i = 1919; i >= 0; --i) a[i + 80] = a[i];\n\
             for (i = 1919; i >= 0; --i) a[i + 40] = a[i];\n\
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
    let loads = code.windows(2).filter(|window| *window == [0x1e, 0x07]).count();
    assert!(loads == 0, "{loads} `push ds; pop es` in two backward copies");
    assert!(has(&code, &[0xfd]), "no backward string move was emitted");
}
