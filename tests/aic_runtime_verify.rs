//! The QB runtime at -Os m16 through the IR verifier: the fold of consecutive
//! byte loads put a wide load before the definition of its address and
//! `B$BLOD` (bload.c) failed "load in block 6 uses a value whose definition
//! does not dominate it", then isel "value with no register" (b0231531b7).

mod common;

use std::path::Path;

#[test]
fn no_runtime_source_leaves_a_use_before_its_definition_at_os_m16() {
    let qb = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/qb");
    let mut sources: Vec<_> = std::fs::read_dir(&qb)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .collect();
    sources.sort();
    let scratch = tempfile::tempdir().unwrap();
    let mut broken = Vec::new();
    let mut compiled = 0;
    for source in &sources {
        let mut command = common::cc::llrm_c();
        command.args(["-m16", "-Os", "-c", "-mbss-begin"]);
        for include in ["", "dos/m16", "dev", "dev/dos/m16", "dev/dos"] {
            command.arg("-I").arg(qb.join(include));
        }
        let output =
            command.env("LLRM_VERIFY", "1").arg(source).arg("-o").arg(scratch.path().join("a.obj")).output().unwrap();
        let text = String::from_utf8_lossy(&output.stderr);
        compiled += usize::from(output.status.success());
        // Other refusals (a header this flag set lacks) are not this test's.
        if text.contains("does not dominate") || text.contains("no register") {
            broken.push(format!(
                "{}: {}",
                source.display(),
                text.lines().find(|l| l.contains("dominate") || l.contains("register")).unwrap_or("")
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
    // A pass on sources that all refused would prove nothing.
    assert!(compiled * 2 > sources.len(), "only {compiled} of {} sources compiled", sources.len());
}
