//! The QB runtime through the pinned clang front end, the only C frontend
//! (llrm-cc): `file.c` called the static `in_buffer` above its declaration,
//! which host gcc takes and clang refuses (implicit declaration), so the
//! runtime did not build under llrm-cc.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn pin_clang() -> PathBuf {
    let clang = std::env::var_os("LLRM_PIN_CLANG").expect("LLRM_PIN_CLANG names the pinned clang");
    PathBuf::from(clang)
}

#[test]
fn every_portable_runtime_source_passes_the_pinned_clang() {
    let qb = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/qb");
    let mut sources: Vec<PathBuf> = fs::read_dir(&qb)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .collect();
    sources.sort();
    let mut refused = Vec::new();
    for source in &sources {
        let mut clang = Command::new(pin_clang());
        clang.args(["-fsyntax-only", "-std=gnu11", "-w", "-Werror=implicit-function-declaration", "-D__cdecl="]);
        for include in ["posix", ".", "dev", "dev/headless", "dev/host", "../shared/posix"] {
            clang.arg("-I").arg(qb.join(include));
        }
        let done = clang.arg(source).output().expect("the pinned clang runs");
        if !done.status.success() {
            let text = String::from_utf8_lossy(&done.stderr);
            refused.push(format!("{}: {}", source.display(), text.lines().find(|l| l.contains("error")).unwrap_or("")));
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
}
