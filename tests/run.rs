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

/// A benchmark that exists in one language is a language gap, not a benchmark: every one has
/// all three variants, its `.out` and its `bench.toml`. The gaps below are real ones, each with its reason.
#[test]
fn test_every_benchmark_exists_in_basic_c_and_nib() {
    const GAPS: &[(&str, &str)] = &[("huge", "nib")]; // Nib has no array over 64K and no allocator for one
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench");
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap().flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "parity") {
            dirs.extend(std::fs::read_dir(&path).unwrap().flatten().map(|one| one.path()).filter(|one| one.is_dir()));
        } else if path.is_dir() {
            dirs.push(path);
        }
    }
    assert!(dirs.len() >= 20, "premise: the benchmarks are found: {dirs:?}");
    let mut missing = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_str().unwrap().to_owned();
        for extension in ["bas", "c", "nib", "out", "toml"] {
            let file = if extension == "toml" { dir.join("bench.toml") } else { dir.join(format!("{name}.{extension}")) };
            if !file.exists() && !GAPS.contains(&(name.as_str(), extension)) {
                missing.push(file.strip_prefix(&root).unwrap().display().to_string());
            }
        }
    }
    assert!(missing.is_empty(), "missing: {missing:?}");
}
