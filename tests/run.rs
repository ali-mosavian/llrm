//! tests/run: every program there compiles with its own compiler, runs in
//! DOSBox and prints what its `.out` says. One launch for all of them.

use std::path::Path;
use std::process::Command;

/// Files git sees in the tree that it does not track or ignore.
fn untracked(root: &Path) -> Option<String> {
    let done = Command::new("git").args(["status", "--porcelain", "--untracked-files=all"]).current_dir(root).output().ok()?;
    done.status.success().then(|| String::from_utf8_lossy(&done.stdout).into_owned())
}

/// Running the examples on the host wrote LEVEL.DAT, LOG.TXT and SCRATCH.TXT into the
/// repo root, and they were committed. A run must leave the tree as it found it.
#[test]
fn test_every_program_under_tests_run_prints_its_out() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let before = untracked(root);
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let done = Command::new("python3")
        .arg(root.join("tools/dosbatch/run_tests.py"))
        .env("LLRM_BIN", bin)
        .current_dir(root)
        .output()
        .expect("python3 runs tools/dosbatch/run_tests.py");
    let report = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success(), "{report}{}", String::from_utf8_lossy(&done.stderr));
    println!("{report}");
    assert_eq!(untracked(root), before, "the run left files in the tree");
}
