//! `-g` keeps no store for a debugger: a variable's stores are the optimiser's
//! like any other, and what a debugger reads is found from the records of what
//! the variable was set to (`#dbg_value`). What the optimiser leaves is read in
//! the last stage the pipeline dumps.

use std::path::{Path, PathBuf};
use std::process::Command;

fn llrm_c() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-c")
}

/// The pipeline's last dumped MIR of `source` compiled with `arguments`.
fn final_mir(
    source: &Path,
    arguments: &[&str],
    dump: &Path,
) -> String {
    let made = Command::new(llrm_c())
        .args(arguments)
        .arg("--dump")
        .arg(dump)
        .arg(source)
        .arg("-o")
        .arg(dump.join("x.o"))
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let mut stages: Vec<PathBuf> = std::fs::read_dir(dump)
        .unwrap()
        .map(|one| one.unwrap().path())
        .filter(|one| one.extension().is_some_and(|ext| ext == "ll"))
        .collect();
    stages.sort();
    std::fs::read_to_string(stages.last().expect("a stage")).unwrap()
}

/// `-g` made every store to a declared variable volatile (and kept a frame
/// register, and a global): no volatile is left in a program that writes none,
/// at any level.
#[test]
fn nothing_is_volatile_with_g_or_without() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    for (level, debug) in [("-O2", false), ("-O2", true), ("-O0", true)] {
        let arguments: Vec<&str> = ["-m32", level].into_iter().chain(debug.then_some("-g")).collect();
        let mir = final_mir(&fixtures.join("observed.c"), &arguments, scratch.path());
        assert!(!mir.contains("store volatile"), "a volatile store with {arguments:?}");
        assert_eq!(mir.contains("#dbg_value"), debug, "{arguments:?}: records");
    }
}
