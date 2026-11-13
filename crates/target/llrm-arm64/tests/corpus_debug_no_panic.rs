//! Every arm64 corpus `.ll` compiles at -O1 and -O2 without a panic in a debug
//! build. A release build wraps where a debug build panics on overflow, so a
//! release-only run hid `promote.rs` `span.high - span.low` (vla.ll -O1, rc=101).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn corpus() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut files = Vec::new();
    for dir in ["data", "darwin"] {
        for entry in std::fs::read_dir(root.join(dir)).expect("a directory reads").flatten() {
            if entry.path().extension().is_some_and(|extension| extension == "ll") {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

#[test]
fn no_arm64_corpus_file_panics_at_o1_or_o2() {
    let files = corpus();
    assert!(files.len() > 100, "the corpus is {} files", files.len());
    let mut panicked = Vec::new();
    for path in &files {
        let source = std::fs::read_to_string(path).expect("a source reads");
        let platform = if source.contains("apple") { Platform::Darwin } else { Platform::Linux };
        for level in [Level::O1, Level::O2] {
            let result = std::panic::catch_unwind(|| {
                let _ = driver::assembly(&source, platform, level);
            });
            if result.is_err() {
                panicked.push(format!("{} {level:?}", path.file_name().unwrap().to_string_lossy()));
            }
        }
    }
    assert!(panicked.is_empty(), "panicked in a debug build: {panicked:?}");
}
